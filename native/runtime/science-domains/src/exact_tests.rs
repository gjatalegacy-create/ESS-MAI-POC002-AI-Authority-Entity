//! Bounded conditional Fisher 2x2 calculation; NEW implementation.
//!
//! Mathematical reference (no upstream source copied):
//! R stats, fisher.test, Details (fixed margins and probability-ordered two-sided test):
//! https://www.stat.ethz.ch/R-manual/R-devel/library/stats/html/fisher.test.html
//! This uses the classical hypergeometric formula attributed there to Fisher (1935).
//! Upstream anofox-statistics was inspected as prior art, not imported. Its MIT
//! notice does not license this independently written implementation.
//!
//! For [[a,b],[c,d]], rows are groups and columns are outcomes. Under conditional
//! independence, X=a has mass C(K,x) C(N-K,r-x) / C(N,r), where r=a+b, K=a+c.
//! Greater means excess in cell a (odds ratio >1); Less means the reverse.
//! TwoSided sums tables no more probable than the observed table. It does NOT
//! generally equal twice a one-sided probability.
//!
//! "Exact" describes the discrete conditional test: all feasible tables are
//! enumerated without a large-sample approximation. Arithmetic is f64, not exact
//! rational arithmetic. Log weights, compensated sums and an explicit numerical
//! guard are used. The guard is a conservative decision policy, NOT a formally
//! proved floating-point error enclosure.
//!
//! The caller must establish actual data provenance, category orientation,
//! independent sampling units, conditioning/design assumptions and predeclared
//! direction/alpha. This pure kernel cannot establish those facts. Rejection is
//! against conditional independence under that model; non-rejection is NOT
//! proof of independence, equivalence, causal effect or any scientific theory.
//! This module has no scientific admission, canonical-domain, P/N or Vault authority.

use std::{error::Error, fmt};

pub const MODEL_ID: &str = "fisher-fixed-margins-2x2/ess-mai-original-v1";
pub const PRIMARY_REFERENCE: &str =
    "https://www.stat.ethz.ch/R-manual/R-devel/library/stats/html/fisher.test.html";
pub const IMPLEMENTATION_ORIGIN: &str =
    "Implementation of the classical formula; no third-party code copied";
/// Explicit work and arithmetic bounds, not statistical applicability limits.
pub const MAX_TOTAL_COUNT: u64 = 1_000_000;
pub const MAX_SUPPORT_SIZE: u64 = 100_001;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FisherAlternative {
    Less,
    Greater,
    TwoSided,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FisherOutcome {
    /// Statistical rejection under supplied assumptions, not an epistemic verdict.
    RejectConditionalIndependence,
    /// Does not support equality, independence or equivalence.
    InsufficientEvidenceAgainstConditionalIndependence,
    /// Within the declared numerical decision guard of alpha.
    NumericalBoundaryUnresolved,
    /// Fixed margins permit only one table; there is no association contrast.
    NoContrast,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FisherAuthority {
    NumericalOnly,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FisherResult {
    pub table: [[u64; 2]; 2],
    pub alternative: FisherAlternative,
    /// Caller-supplied. This field cannot prove that it was chosen in advance.
    pub predeclared_alpha: f64,
    pub total_count: u64,
    pub support_min: u64,
    pub support_max: u64,
    pub support_size: u64,
    /// May underflow to 0. Consult p_value_underflow and log_p_value.
    pub p_value: f64,
    /// Natural log, retained even when p_value underflows.
    pub log_p_value: f64,
    pub p_value_underflow: bool,
    pub observed_log_probability: f64,
    /// Two-sided probability ordering includes near ties within this log guard.
    /// Significance decisions within the same log guard remain unresolved.
    /// This is a policy margin, not a certified numerical error bound.
    pub numerical_log_guard: f64,
    pub outcome: FisherOutcome,
    pub authority: FisherAuthority,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FisherError {
    InvalidAlpha,
    CountOverflow,
    EmptyTable,
    TotalExceedsLimit { actual: u64, maximum: u64 },
    SupportExceedsLimit { actual: u64, maximum: u64 },
    AllocationFailed,
    NumericalFailure,
}

impl fmt::Display for FisherError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAlpha => f.write_str("predeclared alpha must be finite and strictly between zero and one"),
            Self::CountOverflow => f.write_str("2x2 count sum overflows u64"),
            Self::EmptyTable => f.write_str("all four counts are zero"),
            Self::TotalExceedsLimit { actual, maximum } =>
                write!(f, "total count {actual} exceeds bounded model limit {maximum}"),
            Self::SupportExceedsLimit { actual, maximum } =>
                write!(f, "conditional support size {actual} exceeds work limit {maximum}"),
            Self::AllocationFailed => f.write_str("could not allocate bounded Fisher support"),
            Self::NumericalFailure => f.write_str("non-finite or inconsistent Fisher calculation"),
        }
    }
}

impl Error for FisherError {}

/// Evaluate the conditional test using supplied counts and predeclared alpha.
///
/// Counts are exact integers; there is no missing-value imputation, pseudo-count,
/// random sampling, simulation or inferred category ordering. Degenerate margins
/// are returned as NoContrast, never as learned positive knowledge.
///
/// Complexity: O(support_size) time and memory. The explicit support/total bounds
/// keep this kernel usable on untrusted operational input without unbounded work.
pub fn fisher_2x2(
    table: [[u64; 2]; 2],
    alternative: FisherAlternative,
    predeclared_alpha: f64,
) -> Result<FisherResult, FisherError> {
    if !predeclared_alpha.is_finite() || !(0.0 < predeclared_alpha && predeclared_alpha < 1.0) {
        return Err(FisherError::InvalidAlpha);
    }
    let [[a, b], [c, d]] = table;
    let row = a.checked_add(b).ok_or(FisherError::CountOverflow)?;
    let row_other = c.checked_add(d).ok_or(FisherError::CountOverflow)?;
    let total = row.checked_add(row_other).ok_or(FisherError::CountOverflow)?;
    let column = a.checked_add(c).ok_or(FisherError::CountOverflow)?;
    if total == 0 {
        return Err(FisherError::EmptyTable);
    }
    if total > MAX_TOTAL_COUNT {
        return Err(FisherError::TotalExceedsLimit {
            actual: total, maximum: MAX_TOTAL_COUNT,
        });
    }
    let other_column = total - column;
    let low = row.saturating_sub(other_column);
    let high = row.min(column);
    let width = high - low + 1;
    if width > MAX_SUPPORT_SIZE {
        return Err(FisherError::SupportExceedsLimit {
            actual: width, maximum: MAX_SUPPORT_SIZE,
        });
    }
    let guard = 1.0e-10 * width as f64;
    let width_usize = usize::try_from(width).map_err(|_| FisherError::AllocationFailed)?;
    let observed_index = usize::try_from(a - low).map_err(|_| FisherError::NumericalFailure)?;

    let mut weights = Vec::new();
    weights.try_reserve_exact(width_usize).map_err(|_| FisherError::AllocationFailed)?;
    // Set the arbitrary log weight of the first feasible table to zero.
    // Normalization cancels the arbitrary scale. Adjacent ratios are:
    // w(x+1)/w(x) = (K-x)(r-x) / ((x+1)(N-K-r+x+1)).
    // Bounded total makes products <=1e12, exactly representable as f64 integers.
    let mut running = CompensatedSum::default();
    weights.push(0.0);
    for x in low..high {
        let numerator = (column - x) * (row - x);
        let denominator = (x + 1) * (other_column + x - row + 1);
        if numerator == 0 || denominator == 0 {
            return Err(FisherError::NumericalFailure);
        }
        let num = numerator as f64;
        let den = denominator as f64;
        // Near unity, log1p preserves information lost by a rounded quotient.
        let ratio = num / den;
        let increment = if (0.5..=2.0).contains(&ratio) {
            ((num - den) / den).ln_1p()
        } else {
            num.ln() - den.ln()
        };
        running.add(increment);
        if !running.sum.is_finite() {
            return Err(FisherError::NumericalFailure);
        }
        weights.push(running.sum);
    }
    if observed_index >= weights.len() {
        return Err(FisherError::NumericalFailure);
    }
    let observed_weight = weights[observed_index];
    let log_normalizer = selected_log_sum(&weights, |_| true)?;
    let mut selected_count = 0usize;
    let log_selected = selected_log_sum(&weights, |index| {
        let x = low + index as u64;
        let include = match alternative {
            FisherAlternative::Less => x <= a,
            FisherAlternative::Greater => x >= a,
            FisherAlternative::TwoSided => weights[index] <= observed_weight + guard,
        };
        if include { selected_count += 1; }
        include
    })?;
    // selected_log_sum visits the selection once to preserve FnMut semantics.
    if selected_count == 0 || selected_count > weights.len() {
        return Err(FisherError::NumericalFailure);
    }
    let raw_log_p = log_selected - log_normalizer;
    if !raw_log_p.is_finite() || raw_log_p > guard {
        return Err(FisherError::NumericalFailure);
    }
    let log_p = if selected_count == weights.len() { 0.0 } else { raw_log_p.min(0.0) };
    let p = log_p.exp();
    let observed_log_probability = weights[observed_index] - log_normalizer;
    if !p.is_finite() || !(0.0..=1.0).contains(&p) || !observed_log_probability.is_finite() {
        return Err(FisherError::NumericalFailure);
    }
    let alpha_log = predeclared_alpha.ln();
    let outcome = if width == 1 {
        FisherOutcome::NoContrast
    } else if (log_p - alpha_log).abs() <= guard {
        FisherOutcome::NumericalBoundaryUnresolved
    } else if log_p < alpha_log {
        FisherOutcome::RejectConditionalIndependence
    } else {
        FisherOutcome::InsufficientEvidenceAgainstConditionalIndependence
    };
    Ok(FisherResult {
        table, alternative, predeclared_alpha,
        total_count: total, support_min: low, support_max: high, support_size: width,
        p_value: p, log_p_value: log_p, p_value_underflow: p == 0.0,
        observed_log_probability, numerical_log_guard: guard, outcome,
        authority: FisherAuthority::NumericalOnly,
    })
}

#[derive(Default)]
struct CompensatedSum { sum: f64, correction: f64 }

impl CompensatedSum {
    fn add(&mut self, value: f64) {
        let corrected = value - self.correction;
        let next = self.sum + corrected;
        self.correction = (next - self.sum) - corrected;
        self.sum = next;
    }
}

/// Stable log-sum-exp with one predicate invocation per entry.
/// Online rescaling keeps even an extreme selected tail away from underflow.
fn selected_log_sum<F: FnMut(usize) -> bool>(
    weights: &[f64], mut select: F,
) -> Result<f64, FisherError> {
    let mut maximum = f64::NEG_INFINITY;
    let mut terms = CompensatedSum::default();
    for (index, &value) in weights.iter().enumerate() {
        if !select(index) { continue; }
        if !value.is_finite() { return Err(FisherError::NumericalFailure); }
        if maximum == f64::NEG_INFINITY {
            maximum = value;
            terms.add(1.0);
        } else if value > maximum {
            let scale = (maximum - value).exp();
            terms.sum *= scale;
            terms.correction *= scale;
            maximum = value;
            terms.add(1.0);
        } else {
            terms.add((value - maximum).exp());
        }
    }
    if !maximum.is_finite() || !terms.sum.is_finite() || terms.sum <= 0.0 {
        return Err(FisherError::NumericalFailure);
    }
    Ok(maximum + terms.sum.ln())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(got: f64, expected: f64) {
        assert!((got - expected).abs() < 2e-11, "{got} != {expected}");
    }

    #[test]
    fn reference_tea_table_probabilities_and_direction() {
        let table = [[3, 1], [1, 3]];
        let two = fisher_2x2(table, FisherAlternative::TwoSided, 0.05).unwrap();
        near(two.p_value, 17.0 / 35.0);
        near(two.observed_log_probability.exp(), 8.0 / 35.0);
        near(fisher_2x2(table, FisherAlternative::Greater, 0.05).unwrap().p_value, 17.0 / 70.0);
        near(fisher_2x2(table, FisherAlternative::Less, 0.05).unwrap().p_value, 69.0 / 70.0);
        assert_eq!(two.authority, FisherAuthority::NumericalOnly);
        assert_eq!(two.outcome, FisherOutcome::InsufficientEvidenceAgainstConditionalIndependence);
    }

    #[test]
    fn exact_small_tail_rejects_only_under_the_declared_model() {
        let r = fisher_2x2([[4, 0], [0, 4]], FisherAlternative::Greater, 0.05).unwrap();
        near(r.p_value, 1.0 / 70.0);
        assert_eq!(r.outcome, FisherOutcome::RejectConditionalIndependence);
        let r = fisher_2x2([[4, 0], [0, 4]], FisherAlternative::Greater, 1.0 / 70.0).unwrap();
        assert_eq!(r.outcome, FisherOutcome::NumericalBoundaryUnresolved);
    }

    #[test]
    fn reference_unbalanced_table_is_not_twice_one_tail_by_definition() {
        let r = fisher_2x2([[2, 7], [8, 2]], FisherAlternative::TwoSided, 0.05).unwrap();
        near(r.p_value, 0.023014137565221154);
        let l = fisher_2x2([[2, 7], [8, 2]], FisherAlternative::Less, 0.05).unwrap();
        assert!((r.p_value - 2.0*l.p_value).abs() > 1e-6);
    }

    #[test]
    fn row_column_relabeling_and_transposition_preserve_required_relations() {
        let table = [[7, 2], [3, 8]];
        let row_swap = [[3, 8], [7, 2]];
        let col_swap = [[2, 7], [8, 3]];
        let transpose = [[7, 3], [2, 8]];
        let p = fisher_2x2(table, FisherAlternative::TwoSided, 0.05).unwrap().p_value;
        for t in [row_swap, col_swap, transpose] {
            near(fisher_2x2(t, FisherAlternative::TwoSided, 0.05).unwrap().p_value, p);
        }
        let upper = fisher_2x2(table, FisherAlternative::Greater, 0.05).unwrap().p_value;
        near(fisher_2x2(row_swap, FisherAlternative::Less, 0.05).unwrap().p_value, upper);
        near(fisher_2x2(col_swap, FisherAlternative::Less, 0.05).unwrap().p_value, upper);
    }

    #[test]
    fn degenerate_margins_are_no_contrast_not_positive_evidence() {
        for t in [[[0,7],[0,3]], [[4,6],[0,0]], [[0,0],[0,2]]] {
            let r = fisher_2x2(t, FisherAlternative::TwoSided, 0.05).unwrap();
            assert_eq!(r.support_size, 1);
            assert_eq!(r.p_value, 1.0);
            assert_eq!(r.log_p_value, 0.0);
            assert_eq!(r.outcome, FisherOutcome::NoContrast);
        }
    }

    #[test]
    fn extreme_finite_tail_retains_log_probability_when_exp_underflows() {
        let r = fisher_2x2([[1000,0],[0,1000]], FisherAlternative::TwoSided, 0.05).unwrap();
        assert!(r.log_p_value.is_finite() && r.log_p_value < -1000.0);
        assert_eq!(r.p_value, 0.0);
        assert!(r.p_value_underflow);
        assert_eq!(r.outcome, FisherOutcome::RejectConditionalIndependence);
    }

    #[test]
    fn large_total_with_small_support_is_still_bounded_and_accurate() {
        let r = fisher_2x2([[0,1],[499_999,500_000]], FisherAlternative::Less, 0.05).unwrap();
        assert_eq!(r.total_count, MAX_TOTAL_COUNT);
        assert_eq!(r.support_size, 2);
        near(r.p_value, 0.500001);
    }

    #[test]
    fn invalid_alpha_empty_overflow_and_work_limits_are_typed() {
        for a in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, 0.0, 1.0, 2.0] {
            assert_eq!(fisher_2x2([[1,1],[1,1]], FisherAlternative::Less, a), Err(FisherError::InvalidAlpha));
        }
        assert_eq!(fisher_2x2([[0,0],[0,0]], FisherAlternative::Less, 0.05), Err(FisherError::EmptyTable));
        assert_eq!(fisher_2x2([[u64::MAX,1],[0,0]], FisherAlternative::Less, 0.05), Err(FisherError::CountOverflow));
        assert_eq!(fisher_2x2([[u64::MAX,0],[1,0]], FisherAlternative::Less, 0.05), Err(FisherError::CountOverflow));
        assert!(matches!(fisher_2x2([[MAX_TOTAL_COUNT+1,0],[0,0]], FisherAlternative::Less, 0.05), Err(FisherError::TotalExceedsLimit { .. })));
        assert!(matches!(fisher_2x2([[50_001,50_001],[50_001,50_001]], FisherAlternative::Less, 0.05), Err(FisherError::SupportExceedsLimit { .. })));
    }

    // Independent small-integer oracle: direct binomial coefficients and integer
    // probability ordering, not the production log-recurrence implementation.
    // Test fixtures certify numerical behavior only, never production evidence.
    fn choose(n: u64, k: u64) -> u128 {
        if k > n { return 0; }
        let k = k.min(n-k);
        let mut v = 1u128;
        for i in 1..=k { v = v * u128::from(n-k+i) / u128::from(i); }
        v
    }

    fn integer_reference(t: [[u64;2];2], direction: FisherAlternative) -> f64 {
        let [[a,b],[c,d]] = t;
        let n = a+b+c+d;
        let r = a+b;
        let k = a+c;
        let lo = r.saturating_sub(n-k);
        let hi = r.min(k);
        let weight = |x| choose(k,x)*choose(n-k,r-x);
        let observed = weight(a);
        let mut numerator = 0u128;
        for x in lo..=hi {
            let w = weight(x);
            if match direction {
                FisherAlternative::Less => x<=a,
                FisherAlternative::Greater => x>=a,
                FisherAlternative::TwoSided => w<=observed,
            } { numerator += w; }
        }
        numerator as f64 / choose(n,r) as f64
    }

    #[test]
    fn exhaustive_small_tables_match_independent_integer_combinatorics() {
        for a in 0..=4 { for b in 0..=4 { for c in 0..=4 { for d in 0..=4 {
            if a+b+c+d == 0 { continue; }
            let t = [[a,b],[c,d]];
            for direction in [FisherAlternative::Less, FisherAlternative::Greater, FisherAlternative::TwoSided] {
                let r = fisher_2x2(t, direction, 0.05).unwrap();
                near(r.p_value, integer_reference(t,direction));
                assert!(r.log_p_value <= 0.0 && r.observed_log_probability <= 1e-12);
            }
        }}}}
    }
}
