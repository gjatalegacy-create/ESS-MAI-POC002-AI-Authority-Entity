//! R23: adapt the source's measured-candidate method, not entire source files.
//! Canonical v1.8.9 ess_mai/quantum/src/pro.rs:205-224 supplies actual scores;
//! npro.rs:182-183 averages actual weakness observations; layer3/mpro.rs:113-118
//! uses fixed-point ratio arithmetic. Each polarity now has its own actual
//! denominator; absent observations stay None instead of using max(1).
//! HPRO/APRO flags remain metadata, not invented candidates. These measurements
//! grant neither semantic counterproof nor execution authority.

use crate::lim_types::Operator;
use crate::npro::NegativeElimination;
use crate::pro_types::PROCandidate;
use sha2::{Digest, Sha256};
use shadow_contracts::candidate_measurement::{
    CandidateLedger, CandidateObservation, CandidatePolarity, CandidateSummary,
};
use shadow_contracts::materialization::condition_measure::ConditionMeasurement;

/// Current version-marked FinalEvidence compatibility scalars. `positives` is
/// a fixed-unit positive sum, not a count of binary flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdmittedCandidateScore {
    pub positives: u32,
    pub total: u32,
    pub mass: u32,
}

#[derive(Debug, Clone)]
pub struct ProducerMeasurement {
    pub ledger: CandidateLedger,
    pub summary: CandidateSummary,
    pub admitted: AdmittedCandidateScore,
    /// R=3 requires measured PRO/NPRO operator outputs, not historical P/N.
    pub conditions: ConditionMeasurement,
}

fn checked_value(value: f32) -> Result<u32, String> {
    match value.is_finite() && (0.0..=1.0).contains(&value) {
        true => shadow_contracts::fixed_mass(value).ok_or_else(|| "MPRO_PRODUCER_SCORE".into()),
        false => Err("MPRO_PRODUCER_SCORE".into()),
    }
}

fn checked_refs(refs: &[usize], fragment_count: usize) -> Result<Vec<u32>, String> {
    refs.iter().map(|reference| match *reference < fragment_count {
        true => u32::try_from(*reference).map_err(|_| "MPRO_PRODUCER_FRAGMENT_RANGE".into()),
        false => Err("MPRO_PRODUCER_FRAGMENT_RANGE".into()),
    }).collect()
}

pub fn measure_current_producer(
    actual_input: &str,
    fragment_count: usize,
    positive: &[PROCandidate],
    negative: &[NegativeElimination],
    family_mask: u8,
) -> Result<ProducerMeasurement, String> {
    let mut observations = Vec::with_capacity(positive.len() + negative.len());
    for candidate in positive {
        let operator = match candidate.operator {
            Operator::Reverse => 1,
            Operator::Mirror => 2,
            Operator::Contrast => 3,
            Operator::Trace => 4,
        };
        observations.push(CandidateObservation {
            polarity: CandidatePolarity::Positive,
            operator,
            fragment_refs: checked_refs(&candidate.fragment_refs, fragment_count)?,
            value: checked_value(candidate.score)?,
        });
    }
    for candidate in negative {
        observations.push(CandidateObservation {
            polarity: CandidatePolarity::NegativeCondition,
            operator: crate::negative_asset::operator_code(&candidate.operator)
                .ok_or("MPRO_PRODUCER_OPERATOR")?,
            fragment_refs: checked_refs(&candidate.fragment_refs, fragment_count)?,
            value: checked_value(candidate.weakness_score)?,
        });
    }
    // Exact input + polarity + operator + reference set identify a candidate.
    // The wire preserves the original reference sequence, including repeats;
    // the constructor rejects duplicate identities rather than dropping them.
    let ledger = CandidateLedger::new(Sha256::digest(actual_input.as_bytes()).into(), observations)?;
    let summary = ledger.summarize(family_mask)?;
    let available = u64::from(summary.positive_observed.count != 0)
        | (u64::from(summary.negative_observed.count != 0) << 1);
    let conditions = ConditionMeasurement::observed(3, available)?;
    let admitted = AdmittedCandidateScore {
        positives: summary.positive_admitted.sum,
        total: summary.positive_admitted.count,
        // Legacy scalar consumers need a number; the ledger retains None.
        mass: summary.positive_admitted.mean.unwrap_or(0),
    };
    Ok(ProducerMeasurement { ledger, summary, admitted, conditions })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn positive(operator: Operator, score: f32, refs: Vec<usize>) -> PROCandidate {
        PROCandidate {
            candidate_id: PROCandidate::make_id(operator, &refs), operator, score,
            confidence: 1.0, reasoning: "local measurement fixture".into(), fragment_refs: refs,
        }
    }

    fn negative(operator: &str, score: f32, refs: Vec<usize>) -> NegativeElimination {
        NegativeElimination {
            elimination_id: format!("{operator}-local"), operator: operator.into(),
            weakness_score: score, cause: "operator concern, not semantic counterproof".into(),
            fragment_refs: refs,
        }
    }

    #[test]
    fn actual_positive_candidates_use_their_own_denominator() {
        let candidates = vec![
            positive(Operator::Reverse, 0.0, vec![0]),
            positive(Operator::Mirror, 0.3, vec![1]),
            positive(Operator::Contrast, 0.6, vec![2]),
        ];
        let observed = measure_current_producer("local input", 3, &candidates, &[], 1).unwrap();
        // Same regression values as extracted old producer: (0+3000+6000)/3.
        assert_eq!(observed.admitted.mass, 3000);
        assert_eq!(observed.admitted.total, 3);
        assert_eq!(observed.admitted.positives, 9000);
        assert_eq!(observed.summary.negative_observed.mean, None);
    }

    #[test]
    fn actual_source_zero_operators_retain_exact_repeated_references() {
        use crate::pro_operator::{Contrast, Mirror};
        use crate::pro_types::vectorize_fragments;
        let fragments = vec!["one actual fragment".to_string()];
        let vectors = vectorize_fragments(&fragments);
        // Both real source operators retain their default pair [0,0] for an
        // insufficient pair. PRO preserves these measured zero candidates.
        for output in [Mirror::compute(&vectors), Contrast::compute(&vectors)] {
            assert_eq!(output.raw_score, 0.0);
            assert_eq!(output.fragment_refs, vec![0, 0]);
            let candidate = positive(output.operator, output.raw_score, output.fragment_refs);
            let measured = measure_current_producer(
                &fragments[0], fragments.len(), &[candidate], &[], 1,
            ).unwrap();
            assert_eq!(measured.summary.positive_observed.count, 1);
            assert_eq!(measured.summary.positive_observed.mean, Some(0));
            assert_eq!(measured.ledger.observations()[0].fragment_refs, vec![0, 0]);
            assert_eq!(CandidateLedger::from_words(&measured.ledger.to_words()).unwrap(), measured.ledger);
        }
    }

    #[test]
    fn negative_measurements_have_their_own_denominator_and_never_raise_positive() {
        let positives = [positive(Operator::Reverse, 0.2, vec![0])];
        let negatives = [negative("Reverse", 0.0, vec![0]), negative("Mirror", 1.0, vec![])];
        let with = measure_current_producer("i0", 1, &positives, &negatives, 3).unwrap();
        let without = measure_current_producer("i0", 1, &positives, &[], 3).unwrap();
        assert_eq!(with.admitted, without.admitted);
        assert_eq!(with.summary.negative_observed.count, 2);
        assert_eq!(with.summary.negative_observed.sum, 10000);
        assert_eq!(with.summary.negative_observed.mean, Some(5000));
        assert_eq!(with.conditions.missing_mask(), Some(0));
        assert_eq!(without.conditions.missing_mask(), Some(2));
        assert_eq!(without.conditions.condition_mass_fixed(), Some(5000));
    }

    #[test]
    fn measured_zero_is_present_absence_is_none_and_admission_preserves_observations() {
        let positives = [positive(Operator::Reverse, 0.0, vec![])];
        let negatives = [negative("Mirror", 0.8, vec![])];
        let blocked = measure_current_producer("i0", 0, &positives, &negatives, 0).unwrap();
        assert_eq!(blocked.summary.positive_observed.mean, Some(0));
        assert_eq!(blocked.summary.negative_observed.mean, Some(8000));
        assert_eq!(blocked.summary.negative_admitted.mean, Some(0));
        assert_eq!(blocked.summary.negative_admitted.count, 1);
        assert_eq!(blocked.conditions.missing_mask(), Some(0));
        let empty = measure_current_producer("i0", 0, &[], &[], 15).unwrap();
        assert_eq!(empty.summary.positive_observed.mean, None);
        assert_eq!(empty.summary.negative_observed.mean, None);
        assert_eq!(empty.admitted.total, 0);
        assert_eq!(empty.conditions.condition_mass_fixed(), Some(10000));
        let auxiliary_only = measure_current_producer("i0", 0, &positives, &negatives, 12).unwrap();
        assert_eq!(auxiliary_only.summary, blocked.summary);
    }

    #[test]
    fn distinct_candidates_from_one_component_count_once_each_and_duplicates_fail() {
        let first = positive(Operator::Reverse, 0.2, vec![0]);
        let second = positive(Operator::Reverse, 0.8, vec![1]);
        let measured = measure_current_producer("i0", 2, &[first.clone(), second], &[], 1).unwrap();
        assert_eq!(measured.admitted.total, 2);
        assert_eq!(measured.admitted.mass, 5000);
        for score in [0.2, 0.8] {
            let repeated = positive(Operator::Reverse, score, vec![0]);
            assert_eq!(measure_current_producer("i0", 2, &[first.clone(), repeated], &[], 1)
                .unwrap_err(), "MPRO_CANDIDATE_DUPLICATE");
        }
    }

    #[test]
    fn ledger_binds_actual_input_refs_and_values_without_changing_the_producer() {
        let positives = [positive(Operator::Contrast, 0.3, vec![0, 1])];
        let negatives = [negative("Trace", 0.7, vec![])];
        let measured = measure_current_producer("a|b,ë", 2, &positives, &negatives, 3).unwrap();
        let decoded = CandidateLedger::from_words(&measured.ledger.to_words()).unwrap();
        assert_eq!(decoded, measured.ledger);
        assert_eq!(decoded.input_sha256(), <[u8; 32]>::from(Sha256::digest("a|b,ë".as_bytes())));
        assert_eq!(decoded.observations()[0].fragment_refs, vec![0, 1]);
        assert_eq!(decoded.observations()[0].value, 3000);
        assert_eq!(positives[0].score, 0.3);
        assert_eq!(negatives[0].weakness_score, 0.7);
        let changed = measure_current_producer("different i0", 2, &positives, &negatives, 3).unwrap();
        assert_ne!(decoded.input_sha256(), changed.ledger.input_sha256());
    }

    #[test]
    fn invalid_scores_references_and_operators_are_not_silently_repaired() {
        for score in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
            assert_eq!(measure_current_producer("i0", 1,
                &[positive(Operator::Reverse, score, vec![0])], &[], 1).unwrap_err(), "MPRO_PRODUCER_SCORE");
            assert_eq!(measure_current_producer("i0", 1,
                &[], &[negative("Reverse", score, vec![0])], 2).unwrap_err(), "MPRO_PRODUCER_SCORE");
        }
        for refs in [vec![1, 0], vec![0, 0]] {
            let measured = measure_current_producer("i0", 2,
                &[positive(Operator::Reverse, 0.5, refs.clone())], &[], 1).unwrap();
            assert_eq!(measured.ledger.observations()[0].fragment_refs,
                refs.iter().map(|value| *value as u32).collect::<Vec<_>>());
            assert_eq!(CandidateLedger::from_words(&measured.ledger.to_words()).unwrap(), measured.ledger);
        }
        let aliases = [
            positive(Operator::Reverse, 0.2, vec![0, 1]),
            positive(Operator::Reverse, 0.8, vec![1, 0, 1]),
        ];
        assert_eq!(measure_current_producer("i0", 2, &aliases, &[], 1)
            .unwrap_err(), "MPRO_CANDIDATE_DUPLICATE");
        assert_eq!(measure_current_producer("i0", 1,
            &[positive(Operator::Reverse, 0.5, vec![1])], &[], 1).unwrap_err(), "MPRO_PRODUCER_FRAGMENT_RANGE");
        assert_eq!(measure_current_producer("i0", 0,
            &[], &[negative("invented", 0.5, vec![])], 2).unwrap_err(), "MPRO_PRODUCER_OPERATOR");
        assert_eq!(measure_current_producer("i0", 0, &[], &[], 16).unwrap_err(), "MPRO_CANDIDATE_FAMILY_MASK");
    }
}
