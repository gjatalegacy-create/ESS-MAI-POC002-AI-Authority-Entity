//! NEW R3 KiE source adapter. Only an explicit model-scoped proposition may
//! consume these predicates. It does not admit knowledge or physical truth.
use super::*;

/// Stable rendering from caller-supplied query only; does not execute the model.
/// Exact IEEE-754 task bits and compiled-model digest identify the proposition
/// independently of operation/query IDs. Full query/context remains separately
/// bound in KiEProof; identical propositions do not grant cross-scope authority.
pub fn canonical_proposition(query: &ScienceQuery) -> Result<String, ScienceError> {
    let q = normalized(query)?;
    if q.adapter != AdapterId::Astronomy {
        return Err(ScienceError::Unavailable { adapter: q.adapter });
    }
    let ScienceTask::EarthRotationAngle {
        jd_part1,
        jd_part2,
        claim_radians,
        tolerance_radians,
        ..
    } = q.task;
    let model_sha = hash_hex(&compiled_model_hash());
    Ok(format!(
        "MODEL_CONDITIONAL KiE IAU2000 era00 UT1 rad f64hex jd1={:016x} jd2={:016x} claim={:016x} tol={:016x}. \
KiS residual<tol KiC residual>tol equality unresolved. One dependent model/input and not independent evidence and no observation. \
model_sha256={}.",
        jd_part1.to_bits(),jd_part2.to_bits(),claim_radians.to_bits(),tolerance_radians.to_bits(),model_sha
    ))
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KieEvaluation {
    pub canonical_proposition: String,
    pub relation: u8,
    pub kis_mass: u32,
    pub kic_mass: u32,
}
/// Re-evaluates the real pinned provider; checks full canonical bytes/provenance.
/// Scale endpoints express Boolean predicate outcomes, never probability,
/// confidence, frequency, historical trust, or two independently observed facts.
pub fn evaluate_kie(
    query_bytes: &[u8],
    candidate_bytes: &[u8],
) -> Result<KieEvaluation, ScienceError> {
    let query = decode_query(query_bytes)?;
    let actual = evaluate(&query)?;
    if encode_candidate(&actual)? != candidate_bytes {
        return Err(ScienceError::CandidateMismatch);
    }
    let (relation, kis_mass, kic_mass) = match actual.relation {
        ModelRelation::SupportsClaim => (0, 10_000, 0),
        ModelRelation::CountersClaim => (1, 0, 10_000),
        ModelRelation::Unresolved => (2, 0, 0),
    };
    Ok(KieEvaluation {
        canonical_proposition: canonical_proposition(&query)?,
        relation,
        kis_mass,
        kic_mass,
    })
}
/// Current numerical model evidence, preserving both polarity and provenance.
/// Margins are distances in radians from the declared comparison boundary.
/// They are not confidence, historical mass, independent observations or authority.
#[derive(Debug, Clone, PartialEq)]
pub struct KieModelMeasurement {
    pub candidate: CandidateEvidence,
    pub positive_margin_radians: f64,
    pub counter_margin_radians: f64,
}

/// Recompute the pinned provider against exact canonical query/candidate bytes.
/// Owners must invoke this operation themselves rather than trust a caller's
/// public measurement fields or its hashes. Existing KiE wire data is unchanged.
pub fn measure_kie(
    query_bytes: &[u8],
    candidate_bytes: &[u8],
) -> Result<KieModelMeasurement, ScienceError> {
    let query = decode_query(query_bytes)?;
    let candidate = evaluate(&query)?;
    // Exact serialization from encode_candidate, after our owned computation.
    // Calling that public encoder here would execute the same provider twice.
    let mut canonical = candidate_body(&candidate)?;
    canonical.extend_from_slice(&candidate.result_sha256);
    if bounded(canonical)? != candidate_bytes {
        return Err(ScienceError::CandidateMismatch);
    }
    let ScienceTask::EarthRotationAngle { tolerance_radians, .. } = candidate.query.task;
    let residual = candidate.circular_residual_radians;
    Ok(KieModelMeasurement {
        positive_margin_radians: (tolerance_radians - residual).max(0.0),
        counter_margin_radians: (residual - tolerance_radians).max(0.0),
        candidate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn query(claim: f64, tolerance: f64) -> ScienceQuery {
        ScienceQuery {
            adapter: AdapterId::Astronomy,
            session_id: "session".into(),
            case_id: "case".into(),
            scope_id: "model-scope".into(),
            obligation_id: "predicate".into(),
            query_id: "query".into(),
            input_source_id: "caller-ut1".into(),
            task: ScienceTask::EarthRotationAngle {
                jd_part1: 2451545.0,
                jd_part2: 0.0,
                time_scale: TimeScale::Ut1,
                claim_radians: claim,
                tolerance_radians: tolerance,
            },
        }
    }
    #[test]
    fn support_counter_and_boundary_use_real_model_without_fabricated_history() {
        let base = evaluate(&query(0.0, 0.1)).unwrap();
        for (claim, tolerance, expected) in [
            (base.computed_radians, 0.1, (0, 10000, 0)),
            (0.0, 0.1, (1, 0, 10000)),
            (0.0, base.circular_residual_radians, (2, 0, 0)),
        ] {
            let q = query(claim, tolerance);
            let c = evaluate(&q).unwrap();
            let e =
                evaluate_kie(&encode_query(&q).unwrap(), &encode_candidate(&c).unwrap()).unwrap();
            assert_eq!((e.relation, e.kis_mass, e.kic_mass), expected);
            assert!(e.canonical_proposition.contains("MODEL_CONDITIONAL"));
            assert!(e.canonical_proposition.contains("not independent evidence"));
        }
    }
    #[test]
    fn canonical_scope_cannot_be_reused_for_different_claim_or_user_topic() {
        let q = query(0.0, 0.1);
        let rendered = canonical_proposition(&q).unwrap();
        let mut other = q.clone();
        other.case_id = "other".into();
        assert_eq!(rendered, canonical_proposition(&other).unwrap());
        other.query_id = "new-inquiry".into();
        assert_eq!(rendered, canonical_proposition(&other).unwrap());
        let c = encode_candidate(&evaluate(&q).unwrap()).unwrap();
        assert!(evaluate_kie(&encode_query(&other).unwrap(), &c).is_err());
        other.adapter = AdapterId::Physics;
        assert!(canonical_proposition(&other).is_err());
    }
    #[test]
    fn canonical_claim_respects_original_light_512_byte_gate_for_valid_extremes() {
        let mut q = query(f64::from_bits(TAU.to_bits() - 1), f64::from_bits(1));
        q.task = ScienceTask::EarthRotationAngle {
            jd_part1: MAX_EXACT_INTEGER,
            jd_part2: -MAX_EXACT_INTEGER,
            time_scale: TimeScale::Ut1,
            claim_radians: f64::from_bits(TAU.to_bits() - 1),
            tolerance_radians: f64::from_bits(1),
        };
        q.session_id = "s".repeat(MAX_IDENTITY_BYTES);
        q.case_id = "c".repeat(MAX_IDENTITY_BYTES);
        q.scope_id = "x".repeat(MAX_IDENTITY_BYTES);
        q.obligation_id = "o".repeat(MAX_IDENTITY_BYTES);
        q.query_id = "q".repeat(MAX_IDENTITY_BYTES);
        q.input_source_id = "i".repeat(MAX_IDENTITY_BYTES);
        let text = canonical_proposition(&q).unwrap();
        assert!(
            text.len() <= 400,
            "compact canonical claim length {}",
            text.len()
        );
        assert!(text.contains("MODEL_CONDITIONAL") && text.contains("UT1 rad f64hex"));
        assert!(text.contains("KiS residual<tol KiC residual>tol equality unresolved"));
        assert!(text.contains("no observation") && text.contains("model_sha256="));
        // Original PrimitiveSplit wire uses comma/pipe delimiters without escaping.
        // Lingua retains punctuation, so generated input must avoid both delimiters.
        assert!(!text.contains(',') && !text.contains('|'));
        let mut different = q.clone();
        let ScienceTask::EarthRotationAngle {
            ref mut jd_part2, ..
        } = different.task;
        *jd_part2 = 0.0;
        assert_ne!(text, canonical_proposition(&different).unwrap());
        assert!(canonical_proposition(&different).unwrap().len() <= 512);
        let original = canonical_proposition(&query(0.0, 0.1)).unwrap();
        assert!(original.len() <= 512);
        assert_ne!(original, canonical_proposition(&query(0.1, 0.1)).unwrap());
        assert_ne!(original, canonical_proposition(&query(0.0, 0.2)).unwrap());
    }
}


#[cfg(test)]
mod r27_measurement_tests {
    use super::*;

    fn query(claim: f64, tolerance: f64) -> ScienceQuery {
        ScienceQuery {
            adapter: AdapterId::Astronomy,
            session_id: "r27-session".into(), case_id: "r27-case".into(),
            scope_id: "model-only".into(), obligation_id: "era-comparison".into(),
            query_id: "actual-query".into(), input_source_id: "caller-ut1".into(),
            task: ScienceTask::EarthRotationAngle {
                jd_part1: 2451545.0, jd_part2: 0.0, time_scale: TimeScale::Ut1,
                claim_radians: claim, tolerance_radians: tolerance,
            },
        }
    }

    fn measure(query: &ScienceQuery) -> KieModelMeasurement {
        let actual = evaluate(query).unwrap();
        measure_kie(&encode_query(query).unwrap(), &encode_candidate(&actual).unwrap()).unwrap()
    }

    #[test]
    fn r27_measured_support_retains_actual_provider_and_full_identity() {
        let computed = evaluate(&query(0.0, 0.1)).unwrap().computed_radians;
        let q = query(computed, 0.25);
        let actual = evaluate(&q).unwrap();
        let measured = measure(&q);
        assert_eq!(measured.candidate, actual);
        assert_eq!(measured.candidate.query_sha256, sha(&encode_query(&q).unwrap()));
        assert_eq!(measured.candidate.provenance.compiled_model_sha256, compiled_model_hash());
        assert_eq!(measured.candidate.relation, ModelRelation::SupportsClaim);
        assert_eq!(measured.positive_margin_radians.to_bits(), 0.25f64.to_bits());
        assert_eq!(measured.counter_margin_radians.to_bits(), 0.0f64.to_bits());
        assert_eq!(measured.candidate.authority, CandidateAuthority::None);
        assert_eq!(encode_candidate(&measured.candidate).unwrap(), encode_candidate(&actual).unwrap());
    }

    #[test]
    fn r27_measured_counterproof_keeps_its_actual_residual_strength() {
        let q = query(0.0, 0.1);
        let measured = measure(&q);
        let r = measured.candidate.circular_residual_radians;
        assert!(r > 0.1);
        assert_eq!(measured.candidate.relation, ModelRelation::CountersClaim);
        assert_eq!(measured.positive_margin_radians.to_bits(), 0.0f64.to_bits());
        assert_eq!(measured.counter_margin_radians.to_bits(), (r - 0.1).to_bits());
        assert_eq!(measured.candidate.authority, CandidateAuthority::None);
    }

    #[test]
    fn r27_measured_boundary_and_adjacent_floats_are_strict() {
        let r = evaluate(&query(0.0, 0.1)).unwrap().circular_residual_radians;
        for (tol, relation) in [
            (f64::from_bits(r.to_bits() + 1), ModelRelation::SupportsClaim),
            (r, ModelRelation::Unresolved),
            (f64::from_bits(r.to_bits() - 1), ModelRelation::CountersClaim),
        ] {
            let measured = measure(&query(0.0, tol));
            assert_eq!(measured.candidate.relation, relation);
            assert_eq!(measured.positive_margin_radians.to_bits(), (tol - r).max(0.0).to_bits());
            assert_eq!(measured.counter_margin_radians.to_bits(), (r - tol).max(0.0).to_bits());
            assert_eq!(measured.positive_margin_radians * measured.counter_margin_radians, 0.0);
        }
    }

    #[test]
    fn r27_measured_residual_is_symmetric_around_the_actual_model_value() {
        let computed = evaluate(&query(0.0, 0.1)).unwrap().computed_radians;
        let left = measure(&query(computed - 0.25, 0.125));
        let right = measure(&query(computed + 0.25, 0.125));
        assert_eq!(left.candidate.signed_residual_radians, -right.candidate.signed_residual_radians);
        assert_eq!(left.candidate.circular_residual_radians, right.candidate.circular_residual_radians);
        assert_eq!(left.positive_margin_radians, right.positive_margin_radians);
        assert_eq!(left.counter_margin_radians, right.counter_margin_radians);
        assert_eq!(left.counter_margin_radians, 0.125);
        assert_ne!(left.candidate.query_sha256, right.candidate.query_sha256);
        assert_ne!(left.candidate.result_sha256, right.candidate.result_sha256);
    }

    #[test]
    fn r27_measured_candidate_tamper_is_rejected_even_with_coherent_resealed_data() {
        let q = query(0.0, 0.1);
        let query_bytes = encode_query(&q).unwrap();
        let actual = evaluate(&q).unwrap();
        let bytes = encode_candidate(&actual).unwrap();
        for index in [0, query_bytes.len() / 2, query_bytes.len() - 1] {
            let mut changed_query = query_bytes.clone();
            changed_query[index] ^= 1;
            assert!(measure_kie(&changed_query, &bytes).is_err());
        }
        for index in [0, bytes.len() / 2, bytes.len() - 1] {
            let mut changed = bytes.clone();
            changed[index] ^= 1;
            assert_eq!(measure_kie(&query_bytes, &changed).unwrap_err(), ScienceError::CandidateMismatch);
        }
        for field in 0..3 {
            let mut changed = actual.clone();
            match field {
                0 => changed.provenance.compiled_model_sha256[0] ^= 1,
                1 => changed.provenance.source_sha256 = "0".repeat(64),
                _ => changed.circular_residual_radians = 0.0,
            }
            let mut changed_bytes = candidate_body(&changed).unwrap();
            changed.result_sha256 = sha(&changed_bytes);
            assert_eq!(encode_candidate(&changed).unwrap_err(), ScienceError::CandidateMismatch);
            changed_bytes.extend_from_slice(&changed.result_sha256);
            assert_eq!(measure_kie(&query_bytes, &changed_bytes).unwrap_err(), ScienceError::CandidateMismatch);
        }
    }

    #[test]
    fn r27_measured_same_numbers_do_not_allow_cross_scope_or_input_source_reuse() {
        let q = query(0.0, 0.1);
        let actual = evaluate(&q).unwrap();
        let bytes = encode_candidate(&actual).unwrap();
        for field in 0..6 {
            let mut changed = q.clone();
            match field {
                0 => changed.session_id.push_str("-other"),
                1 => changed.case_id.push_str("-other"),
                2 => changed.scope_id.push_str("-other"),
                3 => changed.obligation_id.push_str("-other"),
                4 => changed.query_id.push_str("-other"),
                _ => changed.input_source_id.push_str("-other"),
            }
            assert_eq!(measure_kie(&encode_query(&changed).unwrap(), &bytes).unwrap_err(), ScienceError::CandidateMismatch);
            let own = measure(&changed);
            assert_eq!(own.candidate.circular_residual_radians, actual.circular_residual_radians);
            assert_ne!(own.candidate.query_sha256, actual.query_sha256);
            assert_ne!(own.candidate.result_sha256, actual.result_sha256);
        }
    }

    #[test]
    fn r27_measured_legacy_predicates_remain_unchanged_and_repeated_compute_adds_no_evidence() {
        let q = query(0.0, 0.1);
        let query_bytes = encode_query(&q).unwrap();
        let candidate_bytes = encode_candidate(&evaluate(&q).unwrap()).unwrap();
        let before = evaluate_kie(&query_bytes, &candidate_bytes).unwrap();
        let first = measure_kie(&query_bytes, &candidate_bytes).unwrap();
        let second = measure_kie(&query_bytes, &candidate_bytes).unwrap();
        assert_eq!(first, second);
        assert_eq!(before, evaluate_kie(&query_bytes, &candidate_bytes).unwrap());
        assert_eq!(before.relation, 1);
        assert_eq!((before.kis_mass, before.kic_mass), (0, 10_000));
        assert_eq!(first.candidate.authority, CandidateAuthority::None);
    }
}
