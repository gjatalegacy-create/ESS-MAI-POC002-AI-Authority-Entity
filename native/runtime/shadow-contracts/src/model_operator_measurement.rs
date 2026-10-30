//! Typed scientific PRO/NPRO observations, kept separate from historical
//! candidates and lexical condition measurements. Each installed model result
//! supplies at most one candidate on its actual side. Radians are not confidence.
//! Permission flags record endpoint work permitted and performed. Owners must
//! independently recompute the model and verify those flags against the cycle.
use crate::{kie::KieProof, kie_measurement::KieMeasurement};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelOperatorOutcome { Held, Absent, Admitted }

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelCandidateMean {
    pub count: u32,
    pub sum_radians: f64,
    pub mean_radians: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelSideSummary {
    /// A tested zero supplies no semantic candidate; a held test is not a zero.
    pub predicate_tested: bool,
    pub outcome: ModelOperatorOutcome,
    pub candidates: ModelCandidateMean,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelOperatorSummary {
    pub pro: ModelSideSummary,
    pub npro: ModelSideSummary,
    pub unresolved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelOperatorMeasurements {
    measurement: KieMeasurement,
    pro_permitted: bool,
    npro_permitted: bool,
}

impl ModelOperatorMeasurements {
    pub const WIRE_BYTES: usize = KieMeasurement::WIRE_BYTES + 8;
    pub const WIRE_WORDS: usize = Self::WIRE_BYTES / 8;

    pub fn new(measurement: KieMeasurement, pro_permitted: bool, npro_permitted: bool)
        -> Result<Self, String> {
        measurement.validate()?;
        Ok(Self { measurement, pro_permitted, npro_permitted })
    }
    pub fn validate(&self) -> Result<(), String> { self.measurement.validate() }
    pub fn measurement(&self) -> &KieMeasurement { &self.measurement }
    pub fn pro_permitted(&self) -> bool { self.pro_permitted }
    pub fn npro_permitted(&self) -> bool { self.npro_permitted }
    pub fn validate_model(&self, proof: &KieProof) -> Result<(), String> {
        self.measurement.validate_model(proof)
    }
    /// Each actual model result is one candidate, never four imitated lexical
    /// operators. The denominator is therefore zero or one for this record.
    /// This is observation data; it grants no verdict or write authority.
    pub fn summarize(&self) -> Result<ModelOperatorSummary, String> {
        self.measurement.validate()?;
        fn side(performed: bool, margin: f64) -> ModelSideSummary {
            let count = u32::from(performed && margin > 0.0);
            let outcome = match (performed, count) {
                (false, _) => ModelOperatorOutcome::Held,
                (true, 0) => ModelOperatorOutcome::Absent,
                _ => ModelOperatorOutcome::Admitted,
            };
            let sum_radians = margin * f64::from(count);
            let mean_radians = (count != 0).then(|| sum_radians / f64::from(count));
            ModelSideSummary { predicate_tested: performed, outcome,
                candidates: ModelCandidateMean { count, sum_radians, mean_radians } }
        }
        Ok(ModelOperatorSummary {
            pro: side(self.pro_permitted, self.measurement.positive_margin_radians()),
            npro: side(self.npro_permitted, self.measurement.counter_margin_radians()),
            unresolved: self.measurement.relation()? == 2,
        })
    }
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        let mut bytes = self.measurement.encode()?;
        bytes.extend([u8::from(self.pro_permitted), u8::from(self.npro_permitted), 0, 0, 0, 0, 0, 0]);
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != Self::WIRE_BYTES { return Err("MODEL_OPERATOR_LENGTH".into()); }
        let tail = &bytes[KieMeasurement::WIRE_BYTES..];
        if tail[0] > 1 || tail[1] > 1 || tail[2..].iter().any(|value| *value != 0) {
            return Err("MODEL_OPERATOR_FLAGS".into());
        }
        Self::new(KieMeasurement::decode(&bytes[..KieMeasurement::WIRE_BYTES])?, tail[0] == 1, tail[1] == 1)
    }
    pub fn to_words(&self) -> Vec<u64> {
        // Construction and decoding validate private state; encoding cannot
        // fail after that boundary, and preserves the exact f64 bit patterns.
        let bytes = self.encode().expect("validated model operator measurement");
        bytes.chunks_exact(8).map(|part| u64::from_le_bytes(part.try_into().expect("eight bytes"))).collect()
    }
    pub fn from_words(words: &[u64]) -> Result<Self, String> {
        if words.len() != Self::WIRE_WORDS { return Err("MODEL_OPERATOR_LENGTH".into()); }
        let bytes: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
        Self::decode(&bytes)
    }
}

/// Replay the recorded physical resource condition, not a scientific verdict.
/// This does not prove that caller-supplied hardware data was authentic; the
/// owner binds the eleven words to the current LGC/HPRO/HCP evidence.
pub fn resource_permitted(proof: &KieProof, hcp_words: &[u64]) -> Result<bool, String> {
    if hcp_words.len() != 11 { return Err("MODEL_OPERATOR_HCP_FORMAT".into()); }
    crate::hcp_family_admission_mask(hcp_words)?;
    let parallel = u32::try_from(hcp_words[9]).map_err(|_| "MODEL_OPERATOR_PARALLEL_RANGE")?;
    resource_capacity_permitted(proof, hcp_words[4] == 1, hcp_words[5], hcp_words[8],
        parallel, hcp_words[10] == 1)
}

/// Same capacity predicate for the real endpoint before the family outcomes
/// exist. It consumes the actual single lease; it neither creates a new lease
/// nor reopens a lexical operator that stopped.
pub fn resource_capacity_permitted(proof: &KieProof, sealed: bool, lease_id: u64,
    granted_bytes: u64, granted_parallel: u32, active: bool) -> Result<bool, String> {
    proof.validate()?;
    let required = KieMeasurement::WIRE_BYTES.checked_add(proof.query_bytes.len())
        .and_then(|n| n.checked_add(proof.candidate_bytes.len()))
        .and_then(|n| u64::try_from(n).ok()).ok_or("MODEL_OPERATOR_RESOURCE_OVERFLOW")?;
    Ok(sealed && lease_id != 0 && granted_bytes >= required && granted_parallel > 0 && active)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pd_memory::iz_science_math::EraComparison;
    use sha2::{Digest, Sha256};

    fn fixture(relation: u8) -> (KieProof, KieMeasurement) {
        let proof = crate::iz_frontier::test_proof(relation);
        let tolerance_radians = match relation { 0 => 0.5, 1 => 0.125, _ => 0.25 };
        let comparison = EraComparison { query_sha256: Sha256::digest(&proof.query_bytes).into(),
            result_sha256: [8;32], model_sha256: [9;32], claim_radians: 0.0,
            computed_radians: 0.25, residual_radians: 0.25, tolerance_radians };
        let measurement = KieMeasurement::from_comparison(&proof, &comparison).unwrap();
        (proof, measurement)
    }

    #[test]
    fn scientific_sides_are_symmetric_and_never_create_a_zero_countercandidate() {
        for relation in 0..3 {
            let (proof, measurement) = fixture(relation);
            for pro in [false, true] { for npro in [false, true] {
                let value = ModelOperatorMeasurements::new(measurement.clone(), pro, npro).unwrap();
                value.validate_model(&proof).unwrap();
                let summary = value.summarize().unwrap();
                assert_eq!(summary.pro.predicate_tested, pro);
                assert_eq!(summary.npro.predicate_tested, npro);
                assert_eq!(summary.unresolved, relation == 2);
                assert_eq!(summary.pro.candidates.count, u32::from(pro && relation == 0));
                assert_eq!(summary.npro.candidates.count, u32::from(npro && relation == 1));
                for side in [summary.pro, summary.npro] {
                    assert_eq!(side.candidates.mean_radians.is_some(), side.candidates.count != 0);
                    assert_eq!(side.candidates.sum_radians > 0.0, side.candidates.count != 0);
                    assert_eq!(side.outcome == ModelOperatorOutcome::Held, !side.predicate_tested);
                }
                if pro && relation == 0 { assert_eq!(summary.pro.candidates.mean_radians, Some(0.25)); }
                if npro && relation == 1 { assert_eq!(summary.npro.candidates.mean_radians, Some(0.125)); }
                assert_eq!(ModelOperatorMeasurements::decode(&value.encode().unwrap()).unwrap(), value);
                assert_eq!(ModelOperatorMeasurements::from_words(&value.to_words()).unwrap(), value);
            }}
        }
    }

    #[test]
    fn neighboring_float_boundaries_retain_radian_candidates_without_fixed_point_rounding() {
        let residual = 0.25f64;
        for (relation, tolerance_radians) in [(0, f64::from_bits(residual.to_bits()+1)),
            (1, f64::from_bits(residual.to_bits()-1)), (2, residual)] {
            let proof = crate::iz_frontier::test_proof(relation);
            let measurement = KieMeasurement::from_comparison(&proof, &EraComparison {
                query_sha256: Sha256::digest(&proof.query_bytes).into(), result_sha256: [8;32],
                model_sha256: [9;32], claim_radians: 0.0, computed_radians: residual,
                residual_radians: residual, tolerance_radians }).unwrap();
            let summary = ModelOperatorMeasurements::new(measurement, true, true).unwrap().summarize().unwrap();
            assert_eq!(summary.pro.candidates.count, u32::from(relation == 0));
            assert_eq!(summary.npro.candidates.count, u32::from(relation == 1));
            assert_eq!(summary.pro.candidates.sum_radians.to_bits(), (tolerance_radians-residual).max(0.0).to_bits());
            assert_eq!(summary.npro.candidates.sum_radians.to_bits(), (residual-tolerance_radians).max(0.0).to_bits());
        }
    }

    #[test]
    fn strict_codec_rejects_alias_flags_padding_lengths_and_margin_tampering() {
        let (_, measurement) = fixture(0);
        let value = ModelOperatorMeasurements::new(measurement, true, true).unwrap();
        let bytes = value.encode().unwrap();
        for end in 0..bytes.len() { assert!(ModelOperatorMeasurements::decode(&bytes[..end]).is_err()); }
        let mut extra = bytes.clone(); extra.push(0);
        assert!(ModelOperatorMeasurements::decode(&extra).is_err());
        for index in KieMeasurement::WIRE_BYTES..bytes.len() {
            let mut altered = bytes.clone(); altered[index] = 2;
            assert_eq!(ModelOperatorMeasurements::decode(&altered).unwrap_err(), "MODEL_OPERATOR_FLAGS");
        }
        for index in [192, 200] {
            let mut altered = bytes.clone(); altered[index] ^= 1;
            assert!(ModelOperatorMeasurements::decode(&altered).is_err());
        }
        assert!(ModelOperatorMeasurements::from_words(&value.to_words()[..26]).is_err());
    }

    #[test]
    fn typed_model_data_remains_bound_to_exact_proof_and_query() {
        let (proof, measurement) = fixture(1);
        let value = ModelOperatorMeasurements::new(measurement, true, true).unwrap();
        let mut other = proof.clone(); other.query_bytes.push(1);
        let other = other.seal_data().unwrap();
        assert!(value.validate_model(&other).is_err());
        value.validate_model(&proof).unwrap();
    }

    #[test]
    fn physical_permission_uses_exact_capacity_not_the_lexical_family_mask() {
        let (proof, _) = fixture(0);
        let needed = (KieMeasurement::WIRE_BYTES + proof.query_bytes.len()+proof.candidate_bytes.len()) as u64;
        let words = vec![1,0,3,0,1,5,0,0,needed,1,1];
        assert!(resource_permitted(&proof, &words).unwrap());
        for (index, value) in [(4,0),(5,0),(8,needed-1),(9,0),(10,0)] {
            let mut blocked = words.clone(); blocked[index]=value;
            assert!(!resource_permitted(&proof, &blocked).unwrap(), "accepted failed resource at {index}");
        }
        for mask in 0u64..16 {
            let mut same = words.clone(); same[6]=u64::from(mask.count_ones()); same[7]=mask;
            assert!(resource_permitted(&proof, &same).unwrap());
        }
        for (index,value) in [(4,2),(6,1),(7,16),(10,2)] {
            let mut invalid=words.clone(); invalid[index]=value;
            assert!(resource_permitted(&proof, &invalid).is_err());
        }
        for size in [7,8,10,12] { assert!(resource_permitted(&proof, &vec![0;size]).is_err()); }
    }

}
