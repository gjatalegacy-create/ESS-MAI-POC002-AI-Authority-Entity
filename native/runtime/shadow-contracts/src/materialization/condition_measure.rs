//! R22: independently implemented, data-only measurement of required conditions.
//! D = R & !A; gap_count = popcount(D); mass = floor(10000 * gap_count / popcount(R)).
//! The host must supply an actual validated observation before A is known.
//! This describes a missing condition, never a semantic counterproof, candidate
//! weight, final verdict, authority token, or permission to bypass a court.

pub const CONDITION_SCALE: u32 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionStatus {
    Awaiting,
    Hold,
    Complete,
    FailedMissing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ObservedConditions {
    available: u64,
    missing: u64,
    gap_count: u32,
    mass_fixed: u32,
}

/// Immutable mathematical data. Even a valid value does not authenticate the
/// supplied observation; the surrounding existing source/hold checks do that.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConditionMeasurement {
    required: u64,
    status: ConditionStatus,
    observed: Option<ObservedConditions>,
}

impl ConditionMeasurement {
    pub fn awaiting(required: u64) -> Result<Self, String> {
        Self::unobserved(required, ConditionStatus::Awaiting)
    }

    /// A transport error or unresolved provenance is not an observed absence.
    pub fn hold(required: u64) -> Result<Self, String> {
        Self::unobserved(required, ConditionStatus::Hold)
    }

    fn unobserved(required: u64, status: ConditionStatus) -> Result<Self, String> {
        match required {
            0 => Err("DBAB_CONDITION_EMPTY_REQUIRED".into()),
            _ => Ok(Self { required, status, observed: None }),
        }
    }

    /// Call only with a complete, request-bound observation. Available bits
    /// outside the declared requirement are rejected rather than silently lost.
    pub fn observed(required: u64, available: u64) -> Result<Self, String> {
        match (required, available & !required) {
            (0, _) => Err("DBAB_CONDITION_EMPTY_REQUIRED".into()),
            (_, extra) if extra != 0 => Err("DBAB_CONDITION_UNDECLARED_AVAILABLE".into()),
            _ => {
                let missing = required & !available;
                let gap_count = missing.count_ones();
                // popcount(u64) <= 64, so multiplication is bounded by 640000.
                let mass_fixed = CONDITION_SCALE * gap_count / required.count_ones();
                let status = match missing {
                    0 => ConditionStatus::Complete,
                    _ => ConditionStatus::FailedMissing,
                };
                Ok(Self { required, status, observed: Some(ObservedConditions {
                    available, missing, gap_count, mass_fixed,
                }) })
            }
        }
    }

    pub fn status(&self) -> ConditionStatus { self.status }
    pub fn required_mask(&self) -> u64 { self.required }
    pub fn available_mask(&self) -> Option<u64> { self.observed.map(|v| v.available) }
    pub fn missing_mask(&self) -> Option<u64> { self.observed.map(|v| v.missing) }
    pub fn gap_count(&self) -> Option<u32> { self.observed.map(|v| v.gap_count) }
    pub fn condition_mass_fixed(&self) -> Option<u32> { self.observed.map(|v| v.mass_fixed) }

    /// Knowledge that this processing condition was observed missing. This is
    /// deliberately distinct from knowledge that the user's thesis is false.
    pub fn is_negative_condition_knowledge(&self) -> bool {
        self.status == ConditionStatus::FailedMissing && self.observed.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r22_unobserved_conditions_never_predict_absence_or_create_negative_knowledge() {
        for m in [ConditionMeasurement::awaiting(3).unwrap(), ConditionMeasurement::hold(3).unwrap()] {
            assert_eq!(m.required_mask(), 3);
            assert_eq!(m.available_mask(), None);
            assert_eq!(m.missing_mask(), None);
            assert_eq!(m.gap_count(), None);
            assert_eq!(m.condition_mass_fixed(), None);
            assert!(!m.is_negative_condition_knowledge());
        }
        assert!(ConditionMeasurement::awaiting(0).is_err());
        assert!(ConditionMeasurement::hold(0).is_err());
    }

    #[test]
    fn r22_exact_pair_truth_table_measures_only_observed_condition_deficits() {
        for (available, missing, count, mass, status) in [
            (0, 3, 2, 10_000, ConditionStatus::FailedMissing),
            (1, 2, 1, 5_000, ConditionStatus::FailedMissing),
            (2, 1, 1, 5_000, ConditionStatus::FailedMissing),
            (3, 0, 0, 0, ConditionStatus::Complete),
        ] {
            let m = ConditionMeasurement::observed(3, available).unwrap();
            assert_eq!(m.missing_mask(), Some(missing));
            assert_eq!(m.gap_count(), Some(count));
            assert_eq!(m.condition_mass_fixed(), Some(mass));
            assert_eq!(m.status(), status);
            assert_eq!(m.is_negative_condition_knowledge(), missing != 0);
        }
        assert!(ConditionMeasurement::observed(0, 0).is_err());
        assert!(ConditionMeasurement::observed(3, 4).is_err());
    }

    #[test]
    fn r22_bounded_integer_measurement_rounds_down_and_more_evidence_never_increases_gap() {
        assert_eq!(ConditionMeasurement::observed(7, 3).unwrap().condition_mass_fixed(), Some(3_333));
        assert_eq!(ConditionMeasurement::observed(7, 1).unwrap().condition_mass_fixed(), Some(6_666));
        assert_eq!(ConditionMeasurement::observed(u64::MAX, 0).unwrap().condition_mass_fixed(), Some(10_000));
        assert_eq!(ConditionMeasurement::observed(u64::MAX, u64::MAX).unwrap().condition_mass_fixed(), Some(0));
        for first in 0u64..16 {
            for second in first..16 {
                match first & second == first {
                    false => {},
                    true => {
                        let a = ConditionMeasurement::observed(15, first).unwrap();
                        let b = ConditionMeasurement::observed(15, second).unwrap();
                        assert!(b.gap_count() <= a.gap_count());
                        assert!(b.condition_mass_fixed() <= a.condition_mass_fixed());
                    },
                }
            }
        }
    }
}
