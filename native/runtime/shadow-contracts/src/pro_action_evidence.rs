//! PRO action wire form; validates declared evidence, never grants authority.
//!
//! Formula: [candidate_count, first_score_fixed, last_score_fixed,
//! workers_joined, fragments_admitted, working_set_bytes].
//! Location/purpose: shared Quantum/Shadow action-ledger boundary.
//! Inputs/outputs: six u64 words <-> a validated named value.
//! Failure semantics: reject malformed shape or producer-inconsistent bounds.
//! Authority boundary: resource counters are producer declarations retained in
//! the digest-bound ledger, not independent OS measurements or a Shadow verdict.

use crate::MASS_SCALE;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProActionEvidence {
    pub candidate_count: u64,
    pub first_score_fixed: u64,
    pub last_score_fixed: u64,
    pub workers_joined: u64,
    pub fragments_admitted: u64,
    /// Estimated admitted workload cost, not measured resident RAM.
    pub working_set_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProActionEvidenceError {
    WordCount { actual: usize },
    CandidateLimit,
    CandidatesExceedWorkers,
    ScoreOutOfRange,
    EmptyCandidateScores,
    SingleCandidateScoresDiffer,
    AscendingEndpointScores,
}

impl fmt::Display for ProActionEvidenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WordCount { actual } => write!(f, "PRO requires six words; received {actual}"),
            Self::CandidateLimit => f.write_str("PRO candidate count exceeds five"),
            Self::CandidatesExceedWorkers => f.write_str("PRO candidates exceed joined workers"),
            Self::ScoreOutOfRange => f.write_str("PRO fixed score exceeds MASS_SCALE"),
            Self::EmptyCandidateScores => f.write_str("empty PRO candidates require zero endpoint scores"),
            Self::SingleCandidateScoresDiffer => f.write_str("one PRO candidate requires equal endpoint scores"),
            Self::AscendingEndpointScores => f.write_str("PRO endpoint scores must be descending"),
        }
    }
}

impl std::error::Error for ProActionEvidenceError {}

impl ProActionEvidence {
    pub const WORD_COUNT: usize = 6;
    pub const MAX_CANDIDATES: u64 = 5;

    pub fn try_from_words(words: &[u64]) -> Result<Self, ProActionEvidenceError> {
        let evidence = match words {
            &[candidate_count, first_score_fixed, last_score_fixed, workers_joined, fragments_admitted, working_set_bytes] => Self {
                candidate_count,
                first_score_fixed,
                last_score_fixed,
                workers_joined,
                fragments_admitted,
                working_set_bytes,
            },
            _ => return Err(ProActionEvidenceError::WordCount { actual: words.len() }),
        };
        evidence.validate()?;
        Ok(evidence)
    }

    pub fn validate(&self) -> Result<(), ProActionEvidenceError> {
        match self.candidate_count <= Self::MAX_CANDIDATES {
            true => {}
            false => return Err(ProActionEvidenceError::CandidateLimit),
        }
        match self.candidate_count <= self.workers_joined {
            true => {}
            false => return Err(ProActionEvidenceError::CandidatesExceedWorkers),
        }
        match self.first_score_fixed <= u64::from(MASS_SCALE)
            && self.last_score_fixed <= u64::from(MASS_SCALE)
        {
            true => {}
            false => return Err(ProActionEvidenceError::ScoreOutOfRange),
        }
        match self.candidate_count {
            0 => match self.first_score_fixed == 0 && self.last_score_fixed == 0 {
                true => {}
                false => return Err(ProActionEvidenceError::EmptyCandidateScores),
            },
            1 => match self.first_score_fixed == self.last_score_fixed {
                true => {}
                false => return Err(ProActionEvidenceError::SingleCandidateScoresDiffer),
            },
            _ => {}
        }
        match self.first_score_fixed >= self.last_score_fixed {
            true => Ok(()),
            false => Err(ProActionEvidenceError::AscendingEndpointScores),
        }
    }

    pub const fn to_words(&self) -> [u64; Self::WORD_COUNT] {
        [
            self.candidate_count,
            self.first_score_fixed,
            self.last_score_fixed,
            self.workers_joined,
            self.fragments_admitted,
            self.working_set_bytes,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn six_word_roundtrip_preserves_named_fields_and_resource_observations() {
        for words in [
            [2, 9000, 1000, 2, 3, 120],
            [1, 8000, 8000, 1, 1, 40],
            [5, 10000, 0, 8, 5, 200],
            [0, 0, 0, 0, 1, 40],
            [0, 0, 0, 0, 1, 0],
            [0, 0, 0, u64::MAX, u64::MAX, u64::MAX],
        ] {
            let parsed = ProActionEvidence::try_from_words(&words).expect("valid PRO fixture");
            assert_eq!(parsed.to_words(), words);
            assert_eq!(parsed.candidate_count, words[0]);
            assert_eq!(parsed.workers_joined, words[3]);
            assert_eq!(parsed.fragments_admitted, words[4]);
            assert_eq!(parsed.working_set_bytes, words[5]);
        }
    }

    #[test]
    fn legacy_three_words_and_other_wrong_arities_are_rejected() {
        for length in [0, 1, 2, 3, 4, 5, 7, 64] {
            assert_eq!(
                ProActionEvidence::try_from_words(&vec![0; length]),
                Err(ProActionEvidenceError::WordCount { actual: length }),
            );
        }
    }

    #[test]
    fn semantic_bounds_reject_counts_scores_and_inconsistent_endpoints() {
        for (words, error) in [
            ([6, 9000, 1000, 6, 1, 40], ProActionEvidenceError::CandidateLimit),
            ([2, 9000, 1000, 1, 1, 40], ProActionEvidenceError::CandidatesExceedWorkers),
            ([1, 10001, 10001, 1, 1, 40], ProActionEvidenceError::ScoreOutOfRange),
            ([2, 10000, 10001, 2, 1, 40], ProActionEvidenceError::ScoreOutOfRange),
            ([0, 1, 0, 0, 1, 40], ProActionEvidenceError::EmptyCandidateScores),
            ([0, 0, 1, 0, 1, 40], ProActionEvidenceError::EmptyCandidateScores),
            ([1, 8000, 7000, 1, 1, 40], ProActionEvidenceError::SingleCandidateScoresDiffer),
            ([2, 1000, 9000, 2, 1, 40], ProActionEvidenceError::AscendingEndpointScores),
        ] {
            assert_eq!(ProActionEvidence::try_from_words(&words), Err(error));
        }
    }
}
