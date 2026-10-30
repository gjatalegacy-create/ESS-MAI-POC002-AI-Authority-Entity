//! Data-only mathematical synchronization of KiE and iZ.
//! The residual formula is copied selectively from science-domains/src/lib.rs
//! `evaluate`: circular distance in radians, followed by strict <, >, =.
//! No model runs here. Quantum supplies actual computed data; Shadow must
//! independently reconstruct it from the bound query before admitting a record.
//! Hashes bind exact data; this public formatter does not authenticate authority.
use super::{hex, iz_science::IzScienceInput, text_valid, Hash, MAX_TEXT_BYTES};
use sha2::{Digest, Sha256};
use std::f64::consts::{PI, TAU};

#[derive(Debug, Clone, PartialEq)]
pub struct EraComparison {
    pub query_sha256: Hash,
    pub result_sha256: Hash,
    pub model_sha256: Hash,
    pub claim_radians: f64,
    pub computed_radians: f64,
    pub residual_radians: f64,
    pub tolerance_radians: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Relation {
    Support,
    Counter,
    Boundary,
}

/// Arithmetic/data coherence only, never scientific or historical admission.
fn comparison(result: &EraComparison) -> Result<Relation, String> {
    if [result.query_sha256, result.result_sha256, result.model_sha256]
        .iter()
        .any(|h| *h == [0; 32])
    {
        return Err("IZ_SCIENCE_MATH_UNBOUND_HASH".into());
    }
    for value in [
        result.claim_radians,
        result.computed_radians,
        result.residual_radians,
        result.tolerance_radians,
    ] {
        if !value.is_finite() || (value == 0.0 && value.is_sign_negative()) {
            return Err("IZ_SCIENCE_MATH_NONCANONICAL_NUMBER".into());
        }
    }
    if !(0.0..TAU).contains(&result.claim_radians)
        || !(0.0..TAU).contains(&result.computed_radians)
        || !(0.0..=PI).contains(&result.residual_radians)
        || result.tolerance_radians <= 0.0
        || result.tolerance_radians > PI
    {
        return Err("IZ_SCIENCE_MATH_DOMAIN".into());
    }
    // Source-exact order matters at floating-point boundaries. Do not replace
    // this with a different algebraic distance or introduce an epsilon.
    let mut signed = (result.computed_radians - result.claim_radians).rem_euclid(TAU);
    if signed > PI {
        signed -= TAU;
    }
    if signed == 0.0 {
        signed = 0.0;
    }
    let residual = signed.abs();
    if residual.to_bits() != result.residual_radians.to_bits() {
        return Err("IZ_SCIENCE_MATH_RESIDUAL_BINDING".into());
    }
    Ok(if residual < result.tolerance_radians {
        Relation::Support
    } else if residual > result.tolerance_radians {
        Relation::Counter
    } else {
        Relation::Boundary
    })
}

/// Shared strict arithmetic classification; this does not execute a provider.
pub(crate) fn comparison_code(result: &EraComparison) -> Result<u8, String> {
    Ok(match comparison(result)? {
        Relation::Support => 0, Relation::Counter => 1, Relation::Boundary => 2,
    })
}
fn relation_text(relation: Relation) -> (&'static str, &'static str) {
    match relation {
        Relation::Support => (
            "KiS=1 KiC=0 U=0; r < toleranca: mbështetje vetëm nën modelin dhe hyrjet e deklaruara.",
            "Cilat burime të kontrollueshme provojnë hyrjet UT1 dhe zbatueshmërinë e modelit ndaj këtij rasti? Cila provë e pavarur do ta mbështeste ose kundërshtonte këtë lidhje?",
        ),
        Relation::Counter => (
            "KiS=0 KiC=1 U=0; r > toleranca: kundërshtim i pretendimit numerik nën modelin dhe hyrjet e deklaruara.",
            "Cila evidencë e kontrollueshme sqaron mospërputhjen ndërmjet pretendimit dhe rezultatit? Jep burimin e hyrjeve UT1, pretendimit dhe tolerancës së deklaruar; ruaj mospërputhjen derisa kjo evidencë të verifikohet.",
        ),
        Relation::Boundary => (
            "KiS=0 KiC=0 U=1; r = toleranca: kufi i pazgjidhur, jo kundërprovë dhe jo pranim.",
            "Cila matje ose evidencë e pavarur, me precizion dhe pasiguri të deklaruar, e dallon këtë rast kufitar? Barazia nuk zgjidhet duke përshtatur tolerancën për të prodhuar një pranim.",
        ),
    }
}

/// Bounded actual result plus an evidence question. The result is MODEL_COMPUTED;
/// relevance is USER_REPORTED. Neither supplies constitutional authority.
pub fn computed_question(
    input: &IzScienceInput,
    result: &EraComparison,
) -> Result<String, String> {
    input.validate()?;
    let actual_query_sha256: Hash = Sha256::digest(&input.science_query).into();
    if actual_query_sha256 != result.query_sha256 {
        return Err("IZ_SCIENCE_MATH_QUERY_BINDING".into());
    }
    let relation = comparison(result)?;
    let (outcome, ask) = relation_text(relation);
    let parent = input.context.parent.as_ref().ok_or("IZ_SCIENCE_MATH_PARENT")?;
    let result_text = format!(
        "Operational_iZ KiE-MATH-1 — Rasti {}; fusha {}; detyrimi {}; operacioni {}. \
MODEL_RESULT MODEL_COMPUTED: ERA={:.17e} rad; pretendimi={:.17e} rad; residual_rad={:.17e}; toleranca={:.17e} rad. \
F64hex ERA={:016x} pretendimi={:016x} r={:016x} toleranca={:016x}. {} \
KiS/KiC/U janë tregues logjikë të krahasimit, jo probabilitet ose masë historike. \
Relevanca e propozuar USER_REPORTED: {}. {} \
Mungesa historike e dëshmuar në prind ruhet si referencë e veçantë; ky llogaritim nuk pretendon një përzgjedhje të re ASHT dhe nuk e shndërron mungesën në kundërprovë shkencore. \
model_sha256={}; query_sha256={}; result_sha256={}; prind_record={}; prind_pyetje_sha256={}; lgc_prind={}. \
OperationalNoVerdict; pa autoritet të ri; pa pranim K+/K- ose 0:0/1:1. Pyetja kërkon evidencë nga përdoruesi; nuk parashikon i0 pasues.",
        input.context.identity.case_id,
        input.context.identity.scope,
        input.context.identity.obligation,
        input.context.operation_id,
        result.computed_radians,
        result.claim_radians,
        result.residual_radians,
        result.tolerance_radians,
        result.computed_radians.to_bits(),
        result.claim_radians.to_bits(),
        result.residual_radians.to_bits(),
        result.tolerance_radians.to_bits(),
        outcome,
        input.relevance_statement,
        ask,
        hex(&result.model_sha256),
        hex(&result.query_sha256),
        hex(&result.result_sha256),
        hex(&parent.record_id),
        hex(&parent.question_sha256),
        hex(&input.prior_lgc_head),
    );
    // Oversize is explicit; no silent truncation of evidence or question text.
    text_valid(&result_text, MAX_TEXT_BYTES)?;
    Ok(result_text)
}

#[cfg(test)]
mod tests {
    use super::*;

    // These are arithmetic fixtures, not scientific candidates or GCL evidence.
    fn arithmetic(claim: f64, computed: f64, tolerance: f64) -> EraComparison {
        let mut signed = (computed - claim).rem_euclid(TAU);
        if signed > PI {
            signed -= TAU;
        }
        if signed == 0.0 {
            signed = 0.0;
        }
        EraComparison {
            query_sha256: [1; 32],
            result_sha256: [2; 32],
            model_sha256: [3; 32],
            claim_radians: claim,
            computed_radians: computed,
            residual_radians: signed.abs(),
            tolerance_radians: tolerance,
        }
    }

    #[test]
    fn exact_comparison_distinguishes_support_counter_and_equality() {
        let mut result = arithmetic(0.0, 0.25, 0.5);
        assert_eq!(comparison(&result).unwrap(), Relation::Support);
        result.tolerance_radians = 0.125;
        assert_eq!(comparison(&result).unwrap(), Relation::Counter);
        result.tolerance_radians = 0.25;
        assert_eq!(comparison(&result).unwrap(), Relation::Boundary);
        result.tolerance_radians = f64::from_bits(0.25f64.to_bits() + 1);
        assert_eq!(comparison(&result).unwrap(), Relation::Support);
        result.tolerance_radians = f64::from_bits(0.25f64.to_bits() - 1);
        assert_eq!(comparison(&result).unwrap(), Relation::Counter);
    }

    #[test]
    fn circular_wrap_preserves_source_order_at_zero_and_pi() {
        let result = arithmetic(TAU - 0.125, 0.125, 0.5);
        assert_eq!(result.residual_radians, 0.25);
        assert_eq!(comparison(&result).unwrap(), Relation::Support);
        let reverse = arithmetic(0.125, TAU - 0.125, 0.25);
        assert_eq!(comparison(&reverse).unwrap(), Relation::Boundary);
        assert_eq!(comparison(&arithmetic(0.0, PI, PI)).unwrap(), Relation::Boundary);
        let same = arithmetic(0.0, 0.0, f64::MIN_POSITIVE);
        assert_eq!(same.residual_radians.to_bits(), 0.0f64.to_bits());
        assert_eq!(comparison(&same).unwrap(), Relation::Support);
    }

    #[test]
    fn invalid_or_altered_numbers_and_unbound_hashes_are_rejected() {
        let valid = arithmetic(0.0, 0.25, 0.5);
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.0] {
            for field in 0..4 {
                let mut result = valid.clone();
                match field {
                    0 => result.claim_radians = invalid,
                    1 => result.computed_radians = invalid,
                    2 => result.residual_radians = invalid,
                    _ => result.tolerance_radians = invalid,
                }
                assert!(comparison(&result).is_err());
            }
        }
        for field in 0..4 {
            let mut result = valid.clone();
            match field {
                0 => result.claim_radians = TAU,
                1 => result.computed_radians = -0.25,
                2 => result.residual_radians = PI + 0.25,
                _ => result.tolerance_radians = 0.0,
            }
            assert!(comparison(&result).is_err());
        }
        let mut altered = valid.clone();
        altered.residual_radians = f64::from_bits(altered.residual_radians.to_bits() + 1);
        assert_eq!(comparison(&altered).unwrap_err(), "IZ_SCIENCE_MATH_RESIDUAL_BINDING");
        for field in 0..3 {
            let mut result = valid.clone();
            match field {
                0 => result.query_sha256 = [0; 32],
                1 => result.result_sha256 = [0; 32],
                _ => result.model_sha256 = [0; 32],
            }
            assert_eq!(comparison(&result).unwrap_err(), "IZ_SCIENCE_MATH_UNBOUND_HASH");
        }
    }

    #[test]
    fn relation_questions_do_not_turn_support_or_equality_into_admission() {
        let (support, support_question) = relation_text(Relation::Support);
        let (counter, counter_question) = relation_text(Relation::Counter);
        let (boundary, boundary_question) = relation_text(Relation::Boundary);
        assert!(support.contains("KiS=1 KiC=0 U=0"));
        assert!(support_question.contains("zbatueshmërinë"));
        assert!(counter.contains("KiS=0 KiC=1 U=0"));
        assert!(counter_question.contains("mospërputhjen"));
        assert!(boundary.contains("KiS=0 KiC=0 U=1"));
        assert!(boundary.contains("jo kundërprovë"));
        assert!(boundary_question.contains("precizion dhe pasiguri"));
        assert_ne!(support_question, counter_question);
        assert_ne!(counter_question, boundary_question);
    }
}
