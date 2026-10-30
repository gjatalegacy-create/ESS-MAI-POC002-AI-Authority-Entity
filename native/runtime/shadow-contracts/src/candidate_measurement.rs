//! R23: data-only, candidate-sized MPRO measurements.
//!
//! Every unique measured candidate contributes once, including a measured zero.
//! Positive and negative-condition values have separate sums and denominators.
//! A negative-condition measurement describes an operator concern; it is not a
//! semantic counterproof. Neither hashes nor these means grant authority.
//!
//! The legacy 21-word MPRO action is unchanged. The new marker selects this
//! strict format; malformed marked words must never fall back to legacy parsing.

use std::collections::BTreeSet;

pub const CANDIDATE_MEASUREMENT_MAGIC: u64 = 0x4553_4d50_524f_4331; // ESMPROC1
pub const CANDIDATE_MEASUREMENT_VERSION: u64 = 1;
pub const CANDIDATE_MEASUREMENT_MATRIX_VERSION: u64 = 2;
pub const CANDIDATE_MEASUREMENT_MODEL_VERSION: u64 = 3;
pub const CANDIDATE_MEASUREMENT_CASE_VERSION: u64 = 4;
pub const CANDIDATE_MEASUREMENT_JUDGMENT_VERSION: u64 = 5;
pub const CANDIDATE_MEASUREMENT_SCALE: u32 = 10_000;
pub const MAX_CANDIDATE_OBSERVATIONS: usize = 128;
pub const MAX_CANDIDATE_FRAGMENT_REFS: usize = 128;
const HEADER_WORDS: usize = 7; // marker, version, SHA-256 (4 words), count
const RECORD_WORDS: usize = 4; // polarity, operator, value, reference count
pub const MAX_CANDIDATE_LEGACY_WORDS: usize = HEADER_WORDS
    + MAX_CANDIDATE_OBSERVATIONS * (RECORD_WORDS + MAX_CANDIDATE_FRAGMENT_REFS);
pub const MAX_CANDIDATE_WORDS: usize = MAX_CANDIDATE_LEGACY_WORDS
    + crate::model_operator_measurement::ModelOperatorMeasurements::WIRE_WORDS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CandidatePolarity {
    Positive,
    NegativeCondition,
}

impl CandidatePolarity {
    fn to_word(self) -> u64 {
        match self {
            Self::Positive => 1,
            Self::NegativeCondition => 2,
        }
    }

    fn from_word(word: u64) -> Result<Self, String> {
        match word {
            1 => Ok(Self::Positive),
            2 => Ok(Self::NegativeCondition),
            _ => Err("MPRO_CANDIDATE_POLARITY".into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateObservation {
    pub polarity: CandidatePolarity,
    /// Canonical producer mapping: 1 Reverse, 2 Mirror, 3 Contrast, 4 Trace.
    pub operator: u8,
    /// Exact source reference sequence, including order and repeated roles.
    /// MIRROR/CONTRAST can legitimately return [0, 0] with a measured zero.
    /// Empty is allowed for an actual computation without fragment references.
    pub fragment_refs: Vec<u32>,
    pub value: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CandidateMean {
    pub count: u32,
    pub sum: u32,
    pub mean: Option<u32>,
}

impl CandidateMean {
    fn empty() -> Self {
        Self { count: 0, sum: 0, mean: None }
    }

    fn include(&mut self, value: u32) -> Result<(), String> {
        self.count = self.count.checked_add(1).ok_or("MPRO_CANDIDATE_COUNT_OVERFLOW")?;
        self.sum = self.sum.checked_add(value).ok_or("MPRO_CANDIDATE_SUM_OVERFLOW")?;
        self.mean = Some(self.sum / self.count);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CandidateSummary {
    pub positive_observed: CandidateMean,
    pub positive_admitted: CandidateMean,
    pub negative_observed: CandidateMean,
    pub negative_admitted: CandidateMean,
}

/// Immutable after validated construction; to_words cannot encode an invalid
/// ledger. The input hash binds identity, not execution or admission authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateLedger {
    version: u64,
    input_sha256: [u8; 32],
    observations: Vec<CandidateObservation>,
    model_measurements: Option<crate::model_operator_measurement::ModelOperatorMeasurements>,
}

impl CandidateLedger {
    pub fn new(
        input_sha256: [u8; 32],
        observations: Vec<CandidateObservation>,
    ) -> Result<Self, String> {
        if input_sha256 == [0; 32] {
            return Err("MPRO_CANDIDATE_INPUT_HASH_ZERO".into());
        }
        if observations.len() > MAX_CANDIDATE_OBSERVATIONS {
            return Err("MPRO_CANDIDATE_COUNT_LIMIT".into());
        }
        let mut identities = BTreeSet::new();
        for item in &observations {
            if !(1..=4).contains(&item.operator) {
                return Err("MPRO_CANDIDATE_OPERATOR".into());
            }
            if item.value > CANDIDATE_MEASUREMENT_SCALE {
                return Err("MPRO_CANDIDATE_VALUE".into());
            }
            if item.fragment_refs.len() > MAX_CANDIDATE_FRAGMENT_REFS {
                return Err("MPRO_CANDIDATE_REF_LIMIT".into());
            }
            // Value is deliberately absent from identity: changing the score
            // does not turn the same measured candidate into another candidate.
            // Reference roles/order remain exact in observations and on wire.
            // Only counting identity uses the reference set, so rearranging or
            // repeating a reference cannot inflate the number of candidates.
            let mut reference_identity = item.fragment_refs.clone();
            reference_identity.sort_unstable();
            reference_identity.dedup();
            if !identities.insert((item.polarity, item.operator, reference_identity)) {
                return Err("MPRO_CANDIDATE_DUPLICATE".into());
            }
        }
        drop(identities);
        Ok(Self { version: CANDIDATE_MEASUREMENT_VERSION, input_sha256, observations, model_measurements: None })
    }

    /// R24 is an explicit policy opt-in, not a grant of authority. The owner
    /// must independently validate the exact scoped KiE proof before using it.
    /// Generic historical GCL and decoded R23 ledgers retain version 1.
    pub fn into_current_matrix(mut self) -> Self {
        if self.model_measurements.is_none() { self.version = CANDIDATE_MEASUREMENT_MATRIX_VERSION; }
        self
    }

    pub fn uses_current_matrix(&self) -> bool {
        matches!(self.version, CANDIDATE_MEASUREMENT_MATRIX_VERSION | CANDIDATE_MEASUREMENT_MODEL_VERSION | CANDIDATE_MEASUREMENT_CASE_VERSION | CANDIDATE_MEASUREMENT_JUDGMENT_VERSION)
    }

    /// Scientific measurements are a separate typed record, never converted
    /// into lexical scores or the historical PRO/NPRO family mask.
    pub fn with_model_measurements(mut self,
        model: crate::model_operator_measurement::ModelOperatorMeasurements) -> Result<Self, String> {
        if self.model_measurements.is_some() { return Err("MPRO_MODEL_DUPLICATE".into()); }
        model.measurement().validate()?;
        self.model_measurements = Some(model);
        self.version = CANDIDATE_MEASUREMENT_MODEL_VERSION;
        Ok(self)
    }
    pub fn with_case_reasoning(self,
        model: crate::model_operator_measurement::ModelOperatorMeasurements) -> Result<Self, String> {
        let mut ledger = self.with_model_measurements(model)?;
        ledger.version = CANDIDATE_MEASUREMENT_CASE_VERSION;
        Ok(ledger)
    }
    pub fn uses_case_reasoning(&self) -> bool { matches!(self.version,
        CANDIDATE_MEASUREMENT_CASE_VERSION | CANDIDATE_MEASUREMENT_JUDGMENT_VERSION) }
    pub fn uses_model_judgment(&self) -> bool { self.version == CANDIDATE_MEASUREMENT_JUDGMENT_VERSION }
    pub fn with_model_judgment(self,
        model: crate::model_operator_measurement::ModelOperatorMeasurements) -> Result<Self, String> {
        let mut ledger = self.with_case_reasoning(model)?;
        ledger.version = CANDIDATE_MEASUREMENT_JUDGMENT_VERSION;
        Ok(ledger)
    }
    pub fn model_measurements(&self) -> Option<&crate::model_operator_measurement::ModelOperatorMeasurements> {
        self.model_measurements.as_ref()
    }
    pub fn uses_model_operators(&self) -> bool { matches!(self.version, CANDIDATE_MEASUREMENT_MODEL_VERSION | CANDIDATE_MEASUREMENT_CASE_VERSION | CANDIDATE_MEASUREMENT_JUDGMENT_VERSION) }
    pub fn validate_model(&self, proof: &crate::kie::KieProof) -> Result<(), String> {
        use sha2::{Digest, Sha256};
        let expected: [u8;32] = Sha256::digest(proof.canonical_proposition.as_bytes()).into();
        if self.input_sha256 != expected { return Err("MPRO_MODEL_INPUT_MISMATCH".into()); }
        self.model_measurements.as_ref().ok_or("MPRO_MODEL_MISSING")?.validate_model(proof)
    }

    pub fn input_sha256(&self) -> [u8; 32] {
        self.input_sha256
    }

    pub fn observations(&self) -> &[CandidateObservation] {
        &self.observations
    }

    /// Marker detection only. A marked but incomplete/malformed action still
    /// belongs to this decoder and must fail here, never downgrade to legacy.
    pub fn is_current_words(words: &[u64]) -> bool {
        words.first() == Some(&CANDIDATE_MEASUREMENT_MAGIC)
    }

    pub fn to_words(&self) -> Vec<u64> {
        let capacity = HEADER_WORDS + self.observations.iter()
            .map(|item| RECORD_WORDS + item.fragment_refs.len()).sum::<usize>();
        let mut words = Vec::with_capacity(capacity);
        words.extend([CANDIDATE_MEASUREMENT_MAGIC, self.version]);
        for bytes in self.input_sha256.chunks_exact(8) {
            let mut word = [0; 8];
            word.copy_from_slice(bytes);
            words.push(u64::from_le_bytes(word));
        }
        words.push(self.observations.len() as u64);
        for item in &self.observations {
            words.extend([
                item.polarity.to_word(),
                u64::from(item.operator),
                u64::from(item.value),
                item.fragment_refs.len() as u64,
            ]);
            words.extend(item.fragment_refs.iter().map(|value| u64::from(*value)));
        }
        if let Some(model) = &self.model_measurements { words.extend(model.to_words()); }
        words
    }

    pub fn from_words(words: &[u64]) -> Result<Self, String> {
        if words.len() < HEADER_WORDS || words.len() > MAX_CANDIDATE_WORDS {
            return Err("MPRO_CANDIDATE_WORD_LIMIT".into());
        }
        if !Self::is_current_words(words) {
            return Err("MPRO_CANDIDATE_MAGIC".into());
        }
        let version = words[1];
        if !matches!(version, CANDIDATE_MEASUREMENT_VERSION | CANDIDATE_MEASUREMENT_MATRIX_VERSION | CANDIDATE_MEASUREMENT_MODEL_VERSION | CANDIDATE_MEASUREMENT_CASE_VERSION | CANDIDATE_MEASUREMENT_JUDGMENT_VERSION) {
            return Err("MPRO_CANDIDATE_VERSION".into());
        }
        let mut input_sha256 = [0; 32];
        for (index, word) in words[2..6].iter().enumerate() {
            input_sha256[index * 8..(index + 1) * 8].copy_from_slice(&word.to_le_bytes());
        }
        let count = usize::try_from(words[6]).map_err(|_| "MPRO_CANDIDATE_COUNT_LIMIT")?;
        if count > MAX_CANDIDATE_OBSERVATIONS {
            return Err("MPRO_CANDIDATE_COUNT_LIMIT".into());
        }
        let mut observations = Vec::with_capacity(count);
        let mut position = HEADER_WORDS;
        for _ in 0..count {
            let record = words.get(position..position + RECORD_WORDS)
                .ok_or("MPRO_CANDIDATE_TRUNCATED")?;
            position += RECORD_WORDS;
            let polarity = CandidatePolarity::from_word(record[0])?;
            let operator = u8::try_from(record[1]).map_err(|_| "MPRO_CANDIDATE_OPERATOR")?;
            let value = u32::try_from(record[2]).map_err(|_| "MPRO_CANDIDATE_VALUE")?;
            let ref_count = usize::try_from(record[3]).map_err(|_| "MPRO_CANDIDATE_REF_LIMIT")?;
            if ref_count > MAX_CANDIDATE_FRAGMENT_REFS {
                return Err("MPRO_CANDIDATE_REF_LIMIT".into());
            }
            let raw_refs = words.get(position..position + ref_count)
                .ok_or("MPRO_CANDIDATE_TRUNCATED")?;
            let fragment_refs = raw_refs.iter().map(|word| {
                u32::try_from(*word).map_err(|_| "MPRO_CANDIDATE_REF_RANGE".to_string())
            }).collect::<Result<Vec<_>, _>>()?;
            position += ref_count;
            observations.push(CandidateObservation { polarity, operator, fragment_refs, value });
        }
        let model = if matches!(version, CANDIDATE_MEASUREMENT_MODEL_VERSION | CANDIDATE_MEASUREMENT_CASE_VERSION | CANDIDATE_MEASUREMENT_JUDGMENT_VERSION) {
            Some(crate::model_operator_measurement::ModelOperatorMeasurements::from_words(&words[position..])?)
        } else {
            if position != words.len() { return Err("MPRO_CANDIDATE_TRAILING_WORDS".into()); }
            None
        };
        let mut ledger = Self::new(input_sha256, observations)?;
        ledger.version = version;
        ledger.model_measurements = model;
        Ok(ledger)
    }

    /// Own denominators for each polarity; a denied family contributes zero
    /// admitted value while retaining the actual observed candidate count.
    /// Empty side => None. Measured zero => Some(0). No combined mean exists.
    pub fn summarize(&self, family_mask: u8) -> Result<CandidateSummary, String> {
        if family_mask & !0x0f != 0 {
            return Err("MPRO_CANDIDATE_FAMILY_MASK".into());
        }
        let mut summary = CandidateSummary {
            positive_observed: CandidateMean::empty(),
            positive_admitted: CandidateMean::empty(),
            negative_observed: CandidateMean::empty(),
            negative_admitted: CandidateMean::empty(),
        };
        for item in &self.observations {
            let (observed, admitted, family_bit) = match item.polarity {
                CandidatePolarity::Positive => (&mut summary.positive_observed, &mut summary.positive_admitted, 1),
                CandidatePolarity::NegativeCondition => (&mut summary.negative_observed, &mut summary.negative_admitted, 2),
            };
            observed.include(item.value)?;
            let admitted_value = if family_mask & family_bit != 0 { item.value } else { 0 };
            admitted.include(admitted_value)?;
        }
        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(polarity: CandidatePolarity, reference: u32, value: u32) -> CandidateObservation {
        CandidateObservation { polarity, operator: 1, fragment_refs: vec![reference], value }
    }

    #[test]
    fn actual_candidate_counts_keep_zeros_and_polarities_separate() {
        let ledger = CandidateLedger::new([0x35; 32], vec![
            observation(CandidatePolarity::Positive, 0, 0),
            observation(CandidatePolarity::Positive, 1, 10_000),
            observation(CandidatePolarity::Positive, 2, 1),
            observation(CandidatePolarity::NegativeCondition, 0, 0),
            observation(CandidatePolarity::NegativeCondition, 1, 9_999),
        ]).unwrap();
        let summary = ledger.summarize(15).unwrap();
        assert_eq!(summary.positive_observed, CandidateMean { count: 3, sum: 10_001, mean: Some(3_333) });
        assert_eq!(summary.negative_observed, CandidateMean { count: 2, sum: 9_999, mean: Some(4_999) });
        assert_eq!(summary.positive_admitted, summary.positive_observed);
        assert_eq!(summary.negative_admitted, summary.negative_observed);
    }

    #[test]
    fn absent_side_is_unknown_while_measured_zero_is_counted() {
        let empty = CandidateLedger::new([1; 32], vec![]).unwrap().summarize(0).unwrap();
        assert_eq!(empty.positive_observed, CandidateMean { count: 0, sum: 0, mean: None });
        assert_eq!(empty.negative_observed, empty.positive_observed);
        let measured = CandidateLedger::new([1; 32], vec![observation(CandidatePolarity::Positive, 0, 0)])
            .unwrap().summarize(0).unwrap();
        assert_eq!(measured.positive_observed, CandidateMean { count: 1, sum: 0, mean: Some(0) });
        assert_eq!(measured.positive_admitted, measured.positive_observed);
        assert_eq!(measured.negative_observed.mean, None);
    }

    #[test]
    fn masks_change_only_admission_not_counts_or_observations() {
        let ledger = CandidateLedger::new([2; 32], vec![
            observation(CandidatePolarity::Positive, 0, 7_001),
            observation(CandidatePolarity::Positive, 1, 2),
            observation(CandidatePolarity::NegativeCondition, 0, 5_001),
        ]).unwrap();
        for mask in 0u8..16 {
            let summary = ledger.summarize(mask).unwrap();
            assert_eq!(summary.positive_observed, CandidateMean { count: 2, sum: 7_003, mean: Some(3_501) });
            assert_eq!(summary.negative_observed, CandidateMean { count: 1, sum: 5_001, mean: Some(5_001) });
            assert_eq!(summary.positive_admitted.count, 2);
            assert_eq!(summary.negative_admitted.count, 1);
            assert_eq!(summary.positive_admitted.sum, 7_003 * u32::from(mask & 1 != 0));
            assert_eq!(summary.negative_admitted.sum, 5_001 * u32::from(mask & 2 != 0));
            assert_eq!(summary.positive_admitted.mean, Some(summary.positive_admitted.sum / 2));
            assert_eq!(summary.negative_admitted.mean, Some(summary.negative_admitted.sum));
            assert!(summary.positive_admitted.mean <= summary.positive_observed.mean);
            assert!(summary.negative_admitted.mean <= summary.negative_observed.mean);
        }
        for mask in 16u8..=255 { assert!(ledger.summarize(mask).is_err()); }
    }

    #[test]
    fn identity_rejects_duplicate_candidates_without_rewriting_source_references() {
        let first = observation(CandidatePolarity::Positive, 7, 0);
        let mut changed_score = first.clone();
        changed_score.value = 10_000;
        assert_eq!(CandidateLedger::new([1; 32], vec![first.clone(), changed_score]).unwrap_err(), "MPRO_CANDIDATE_DUPLICATE");
        for refs in [vec![0, 0], vec![2, 1], vec![1, 2, 1]] {
            let mut item = first.clone(); item.fragment_refs = refs;
            let ledger = CandidateLedger::new([1; 32], vec![item.clone()]).unwrap();
            assert_eq!(ledger.observations(), &[item]);
            assert_eq!(CandidateLedger::from_words(&ledger.to_words()).unwrap(), ledger);
            assert_eq!(ledger.summarize(1).unwrap().positive_observed,
                CandidateMean { count: 1, sum: 0, mean: Some(0) });
        }
        for refs in [vec![1, 2], vec![2, 1], vec![1, 2, 1], vec![2, 2, 1]] {
            let mut original = first.clone(); original.fragment_refs = vec![1, 2];
            let mut alias = first.clone(); alias.fragment_refs = refs; alias.value = 9000;
            assert_eq!(CandidateLedger::new([1; 32], vec![original, alias]).unwrap_err(), "MPRO_CANDIDATE_DUPLICATE");
        }
        let mut repeated_role = first.clone(); repeated_role.fragment_refs = vec![7, 7];
        assert_eq!(CandidateLedger::new([1; 32], vec![first.clone(), repeated_role]).unwrap_err(), "MPRO_CANDIDATE_DUPLICATE");
        let mut other_operator = first.clone(); other_operator.operator = 2;
        let mut other_polarity = first.clone(); other_polarity.polarity = CandidatePolarity::NegativeCondition;
        assert!(CandidateLedger::new([1; 32], vec![first, other_operator, other_polarity]).is_ok());
        let no_fragments = CandidateObservation { polarity: CandidatePolarity::Positive, operator: 1, fragment_refs: vec![], value: 0 };
        assert_eq!(CandidateLedger::new([1; 32], vec![no_fragments]).unwrap().summarize(1).unwrap().positive_observed.count, 1);
    }

    #[test]
    fn counts_and_reference_limits_are_exact_without_silent_truncation() {
        for count in [0, 1, 3, 17, MAX_CANDIDATE_OBSERVATIONS] {
            let observations = (0..count).map(|i| observation(CandidatePolarity::Positive, i as u32, (i as u32 * 73) % 10_001)).collect();
            let ledger = CandidateLedger::new([9; 32], observations).unwrap();
            let summary = ledger.summarize(1).unwrap();
            let expected_sum: u32 = (0..count).map(|i| (i as u32 * 73) % 10_001).sum();
            assert_eq!(summary.positive_observed.count, count as u32);
            assert_eq!(summary.positive_observed.sum, expected_sum);
            assert_eq!(summary.positive_observed.mean, if count == 0 { None } else { Some(expected_sum / count as u32) });
            assert_eq!(CandidateLedger::from_words(&ledger.to_words()).unwrap(), ledger);
        }
        let over_count = (0..=MAX_CANDIDATE_OBSERVATIONS).map(|i| observation(CandidatePolarity::Positive, i as u32, 1)).collect();
        assert_eq!(CandidateLedger::new([9; 32], over_count).unwrap_err(), "MPRO_CANDIDATE_COUNT_LIMIT");
        let max_records = (0..MAX_CANDIDATE_OBSERVATIONS).map(|i| CandidateObservation {
            polarity: CandidatePolarity::Positive, operator: 1,
            fragment_refs: (0..MAX_CANDIDATE_FRAGMENT_REFS).map(|j| (i * MAX_CANDIDATE_FRAGMENT_REFS + j) as u32).collect(),
            value: 10_000,
        }).collect();
        let maximum = CandidateLedger::new([9; 32], max_records).unwrap();
        assert_eq!(maximum.to_words().len(), MAX_CANDIDATE_LEGACY_WORDS);
        assert_eq!(CandidateLedger::from_words(&maximum.to_words()).unwrap(), maximum);
        assert_eq!(maximum.summarize(1).unwrap().positive_observed.sum, 1_280_000);
        let mut too_many_refs = observation(CandidatePolarity::Positive, 0, 1);
        too_many_refs.fragment_refs = (0..=MAX_CANDIDATE_FRAGMENT_REFS as u32).collect();
        assert_eq!(CandidateLedger::new([9; 32], vec![too_many_refs]).unwrap_err(), "MPRO_CANDIDATE_REF_LIMIT");
    }

    #[test]
    fn strict_words_reject_truncation_aliases_extra_data_and_legacy_reinterpretation() {
        let ledger = CandidateLedger::new([0x5a; 32], vec![observation(CandidatePolarity::Positive, 4, 0)]).unwrap();
        let words = ledger.to_words();
        assert_eq!(CandidateLedger::from_words(&words).unwrap().to_words(), words);
        assert_eq!(ledger.input_sha256(), [0x5a; 32]);
        assert_eq!(ledger.observations().len(), 1);
        for end in 0..words.len() { assert!(CandidateLedger::from_words(&words[..end]).is_err()); }
        assert!(CandidateLedger::is_current_words(&words[..1]));
        assert!(!CandidateLedger::is_current_words(&[1; 21]));
        assert!(CandidateLedger::from_words(&[1; 21]).is_err());
        for (index, invalid) in [(0, 0), (1, 3), (6, 129), (6, u64::MAX), (7, 0), (7, 3), (8, 0), (8, 5), (8, 257), (9, 10_001), (9, u64::MAX), (10, 129), (10, u64::MAX), (11, u64::from(u32::MAX) + 1)] {
            let mut altered = words.clone(); altered[index] = invalid;
            assert!(CandidateLedger::from_words(&altered).is_err(), "accepted mutation at {index}");
        }
        let mut trailing = words.clone(); trailing.push(0);
        assert_eq!(CandidateLedger::from_words(&trailing).unwrap_err(), "MPRO_CANDIDATE_TRAILING_WORDS");
        assert!(CandidateLedger::from_words(&vec![0; MAX_CANDIDATE_WORDS + 1]).is_err());
        assert!(CandidateLedger::new([0; 32], vec![]).is_err());
        for operator in [0, 5, 255] {
            let mut item = observation(CandidatePolarity::Positive, 0, 0); item.operator = operator;
            assert!(CandidateLedger::new([1; 32], vec![item]).is_err());
        }
    }

    #[test]
    fn matrix_policy_is_explicit_and_legacy_words_replay_exactly() {
        // Fixed R23 wire fixture, including the real zero-result [0, 0] roles.
        // Reading it must neither upgrade policy nor normalize source bytes.
        let legacy_words = vec![
            0x4553_4d50_524f_4331, 1,
            0x1111_1111_1111_1111, 0x1111_1111_1111_1111,
            0x1111_1111_1111_1111, 0x1111_1111_1111_1111,
            1, 1, 2, 0, 2, 0, 0,
        ];
        let legacy = CandidateLedger::from_words(&legacy_words).unwrap();
        assert!(!legacy.uses_current_matrix());
        assert_eq!(legacy.to_words(), legacy_words);
        let default = CandidateLedger::new([0x11; 32], vec![CandidateObservation {
            polarity: CandidatePolarity::Positive, operator: 2,
            fragment_refs: vec![0, 0], value: 0,
        }]).unwrap();
        assert_eq!(default, legacy, "new generic GCL must retain the R23 policy");

        let current = default.into_current_matrix();
        assert!(current.uses_current_matrix());
        assert_eq!(current.to_words()[1], 2);
        let mut expected_current = legacy_words.clone();
        expected_current[1] = 2;
        assert_eq!(current.to_words(), expected_current,
            "opting in changes only the bound policy version, never measurements");
        for mask in 0..=15 {
            assert_eq!(current.summarize(mask).unwrap(), legacy.summarize(mask).unwrap());
        }
        let decoded = CandidateLedger::from_words(&expected_current).unwrap();
        assert!(decoded.uses_current_matrix());
        assert_eq!(decoded.to_words(), expected_current);
        assert_eq!(decoded.into_current_matrix(), current, "explicit opt-in is idempotent");
    }

    #[test]
    fn unknown_matrix_policy_never_falls_back_to_legacy() {
        let current = CandidateLedger::new([0x31; 32], vec![]).unwrap().into_current_matrix();
        let words = current.to_words();
        for unknown in [0, 6, u64::MAX] {
            let mut altered = words.clone();
            altered[1] = unknown;
            assert!(CandidateLedger::is_current_words(&altered));
            assert_eq!(CandidateLedger::from_words(&altered).unwrap_err(), "MPRO_CANDIDATE_VERSION");
        }
        for end in 1..words.len() {
            assert!(CandidateLedger::is_current_words(&words[..end]));
            assert!(CandidateLedger::from_words(&words[..end]).is_err());
        }
        let mut trailing = words;
        trailing.push(0);
        assert_eq!(CandidateLedger::from_words(&trailing).unwrap_err(), "MPRO_CANDIDATE_TRAILING_WORDS");
    }

    #[test]
    fn model_v3_is_explicit_and_never_changes_original_measurements_or_legacy_bytes() {
        use crate::{model_operator_measurement::ModelOperatorMeasurements, kie_measurement::KieMeasurement,
            pd_memory::iz_science_math::EraComparison};
        use sha2::{Digest, Sha256};
        let proof = crate::iz_frontier::test_proof(0);
        let input_sha = Sha256::digest(proof.canonical_proposition.as_bytes()).into();
        let measurement = KieMeasurement::from_comparison(&proof, &EraComparison {
            query_sha256: Sha256::digest(&proof.query_bytes).into(), result_sha256: [8;32],
            model_sha256: [9;32], claim_radians: 0.0, computed_radians: 0.25,
            residual_radians: 0.25, tolerance_radians: 0.5 }).unwrap();
        let model = ModelOperatorMeasurements::new(measurement, true, true).unwrap();
        let legacy = CandidateLedger::new(input_sha, vec![
            observation(CandidatePolarity::Positive, 0, 3000),
            observation(CandidatePolarity::NegativeCondition, 0, 7000),
        ]).unwrap();
        let v2 = legacy.clone().into_current_matrix();
        for old in [&legacy, &v2] {
            let bytes = old.to_words();
            assert_eq!(CandidateLedger::from_words(&bytes).unwrap().to_words(), bytes);
            assert!(old.model_measurements().is_none());
            let current = old.clone().with_model_measurements(model.clone()).unwrap();
            current.validate_model(&proof).unwrap();
            assert!(current.uses_current_matrix() && current.uses_model_operators());
            for mask in 0..16 { assert_eq!(current.summarize(mask).unwrap(), old.summarize(mask).unwrap()); }
            assert_eq!(current.model_measurements().unwrap().summarize().unwrap().pro.candidates.count, 1);
            assert_eq!(current.model_measurements().unwrap().summarize().unwrap().npro.candidates.count, 0);
            let words = current.to_words();
            assert_eq!(words.len(), bytes.len()+ModelOperatorMeasurements::WIRE_WORDS);
            assert_eq!(words[0], bytes[0]); assert_eq!(&words[2..bytes.len()], &bytes[2..]);
            assert_eq!(words[1], 3);
            assert_eq!(CandidateLedger::from_words(&words).unwrap(), current);
            assert_eq!(current.clone().into_current_matrix(), current, "cannot silently drop typed scientific data");
            assert_eq!(current.clone().with_model_measurements(model.clone()).unwrap_err(), "MPRO_MODEL_DUPLICATE");
            for end in bytes.len()..words.len() { assert!(CandidateLedger::from_words(&words[..end]).is_err()); }
            let mut extra=words.clone(); extra.push(0); assert!(CandidateLedger::from_words(&extra).is_err());
            for old_version in [1,2] { let mut downgraded=words.clone(); downgraded[1]=old_version;
                assert_eq!(CandidateLedger::from_words(&downgraded).unwrap_err(), "MPRO_CANDIDATE_TRAILING_WORDS"); }
        }
        let wrong_input = CandidateLedger::new([1;32], vec![]).unwrap().with_model_measurements(model).unwrap();
        assert_eq!(wrong_input.validate_model(&proof).unwrap_err(), "MPRO_MODEL_INPUT_MISMATCH");
    }

}
