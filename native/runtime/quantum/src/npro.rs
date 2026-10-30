// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  NPRO.RS — Negative PRO (Eliminimi Paralel)                          ║
// ║  GJATA LEGACY™ — Quantum Platform                                     ║
// ║                                                                          ║
// ║  NPRO punon PARALEL me PRO. Ndërsa PRO kërkon primitivin, NPRO         ║
// ║  kalkulon rrugët e dobëta (eleminimet) me 4 operatorët MBRAPSHT.       ║
// ║                                                                          ║
// ║  DY OUTPUTE:                                                           ║
// ║   1. eliminations → NPIM (paketon me argumenta për Shadow)           ║
// ║   2. WeaknessSignal → LIM (forbidden_paths për kufijtë e PRO)        ║
// ║                                                                          ║
// ║  NPRO NUK vendos, NUK dërgon vetë te Shadow — vetëm kalkulon.          ║
// ║  Zero if/else — formula matematike.                                   ║
// ╚══════════════════════════════════════════════════════════════════════════╝

use crate::candidate_weight::{is_ranked, polarity_mass, ParityEvidence, ParityVerdict};
use crate::npro_operator::{NContrast, NMirror, NReverse, NTrace};
use crate::pro_types::{vectorize_fragments, FragmentVector};
use crate::ultimatum_collapse_law::AshtOperatorDirective;
use shadow_contracts::CandidateStatisticWire;

// ─────────────────────────────────────────────────────────────────────────────
// ELIMINATION — një rrugë e dobët e identifikuar nga NPRO
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct NegativeElimination {
    pub elimination_id: String,
    pub operator: String,    // cili operator mbrapsht e gjeti
    pub weakness_score: f32, // [0,1] sa e dobët
    pub cause: String,       // pse u eliminua
    pub fragment_refs: Vec<usize>,
}

// ─────────────────────────────────────────────────────────────────────────────
// WEAKNESS SIGNAL — sinjali që NPRO i dërgon LIM-it
// ─────────────────────────────────────────────────────────────────────────────

/// Sinjali që vendos kufij për PRO-n (forbidden paths).
/// LIM e merr → PRO s'humbet cikle në rrugë të dobëta.
#[derive(Debug, Clone)]
pub struct WeaknessSignal {
    pub forbidden_paths: Vec<String>, // operatorët/rrugët e ndaluara
    pub weakness_ceiling: f32,        // mbi këtë score → e ndaluar
    pub avg_weakness: f32,            // dobësia mesatare e detektuar
}

impl WeaknessSignal {
    /// A është një rrugë e ndaluar? Zero if — contains() boolean.
    pub fn is_forbidden(&self, path: &str) -> bool {
        self.forbidden_paths.iter().any(|p| p == path)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// NPRO RESULT
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct NproResult {
    pub eliminations: Vec<NegativeElimination>, // → NPIM
    pub weakness_signal: WeaknessSignal,        // → LIM
    pub operators_run: usize,
    /// Phase 1B.1: parity consumed from the request-bound Asht evidence.
    pub candidate_parity: Option<ParityVerdict>,
    pub positive_candidate_mass: u64,
    pub negative_candidate_mass: u64,
    pub ranked_negative_count: usize,
}

// ─────────────────────────────────────────────────────────────────────────────
// NPRO ENGINE
// ─────────────────────────────────────────────────────────────────────────────

/// Pragu mbi të cilin një rrugë konsiderohet e dobët (forbidden).
pub const WEAKNESS_CEILING: f32 = 0.65;

pub struct NproEngine;

impl NproEngine {
    /// Typed KiE counterproof predicate. Separate from original lexical observations.
    /// The result is evidence in radians, never a historical score or verdict.
    pub fn measure_kic<H: crate::pro::HwScaffoldView>(
        hw: &H,
        proof: &shadow_contracts::kie::KieProof,
        directive: &crate::ultimatum_collapse_law::AshtOperatorDirective,
        sealed: bool,
    ) -> Result<Option<shadow_contracts::kie_measurement::KieMeasurement>, String> {
        crate::kie_measurement::measure_endpoint(hw, proof, directive, sealed)
    }


    /// Directed production entrypoint. Asht is verified before any negative
    /// operator consumes the evidence-scoped LIM set.
    pub fn activate_directed(
        fragments: &[String],
        signal: f32,
        causal: f32,
        directive: &AshtOperatorDirective,
        ranked_negative_candidates: &[CandidateStatisticWire],
        parity: ParityEvidence,
    ) -> Result<NproResult, String> {
        match (
            directive.shape_is_valid(),
            signal.is_finite() && (0.0..=1.0).contains(&signal),
            causal.is_finite() && (0.0..=1.0).contains(&causal),
        ) {
            (true, true, true) => {}
            (false, _, _) => return Err("NPRO rejected invalid Asht directive".to_string()),
            (_, false, _) => return Err("NPRO rejected non-finite/out-of-range signal".to_string()),
            (_, _, false) => {
                return Err("NPRO rejected non-finite/out-of-range causal mass".to_string())
            }
        }
        let measured_negative_mass = polarity_mass(ranked_negative_candidates);
        let candidate_scope_is_valid = ranked_negative_candidates
            .iter()
            .all(|candidate| candidate.shape_is_valid() && candidate.polarity == 0)
            && is_ranked(ranked_negative_candidates)
            && measured_negative_mass == parity.negative_mass
            && parity.shape_is_valid(ranked_negative_candidates.len());
        match candidate_scope_is_valid {
            true => {}
            false => {
                return Err(
                    "NPRO rejected non-canonical ranked negative parity evidence".to_string(),
                )
            }
        }

        let mut result = Self::activate(fragments, signal, causal);
        result.candidate_parity = Some(parity.verdict);
        result.positive_candidate_mass = parity.positive_mass;
        result.negative_candidate_mass = parity.negative_mass;
        result.ranked_negative_count = ranked_negative_candidates.len();

        let authorized = result.eliminations.iter().all(|elimination| {
            let normalized = elimination.operator.to_ascii_uppercase();
            directive
                .allowed_operators
                .iter()
                .any(|allowed| normalized.contains(allowed))
        });
        match (
            result.operators_run == 4,
            result.eliminations.len() == 4,
            authorized,
            result.eliminations.iter().all(|value| {
                value.weakness_score.is_finite() && (0.0..=1.0).contains(&value.weakness_score)
            }),
            result.ranked_negative_count == ranked_negative_candidates.len(),
        ) {
            (true, true, true, true, true) => Ok(result),
            _ => Err("NPRO operator output escaped the Asht scope".to_string()),
        }
    }

    /// activate — kalkulon eliminimet me 4 operatorët mbrapsht.
    ///
    /// signal/causal vijnë nga scaffold-i (si te PRO), por interpretohen
    /// për dobësi. Prodhon eliminations (→NPIM) + WeaknessSignal (→LIM).
    pub fn activate(fragments: &[String], signal: f32, causal: f32) -> NproResult {
        let vectors: Vec<FragmentVector> = vectorize_fragments(fragments);

        // Aplikon 4 operatorët MBRAPSHT.
        // A zero-sized admitted workset has no fragment 0. The same weakness
        // computation remains an observation of conditions, not a counterproof.
        let n_reverse = NReverse::compute(signal, causal, (0..fragments.len().min(1)).collect());
        let n_mirror = NMirror::compute(&vectors);
        let n_contrast = NContrast::compute(&vectors);
        let n_trace = NTrace::compute(&vectors);

        let outputs = [n_reverse, n_mirror, n_contrast, n_trace];

        // Ndërto eliminimet (një per operator).
        let eliminations: Vec<NegativeElimination> = outputs
            .iter()
            .enumerate()
            .map(|(i, out)| NegativeElimination {
                elimination_id: format!("NELIM_{}", i),
                operator: format!("{:?}", out.operator),
                weakness_score: out.raw_score,
                cause: out.reasoning.clone(),
                fragment_refs: out.fragment_refs.clone(),
            })
            .collect();

        // Forbidden paths: operatorët me weakness > ceiling.
        let forbidden_paths: Vec<String> = eliminations
            .iter()
            .filter(|e| e.weakness_score > WEAKNESS_CEILING)
            .map(|e| e.operator.clone())
            .collect();

        // Dobësia mesatare.
        let sum: f32 = eliminations.iter().map(|e| e.weakness_score).sum();
        let avg_weakness = sum / eliminations.len().max(1) as f32;

        let weakness_signal = WeaknessSignal {
            forbidden_paths,
            weakness_ceiling: WEAKNESS_CEILING,
            avg_weakness,
        };

        NproResult {
            eliminations,
            weakness_signal,
            operators_run: 4,
            candidate_parity: None,
            positive_candidate_mass: 0,
            negative_candidate_mass: 0,
            ranked_negative_count: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn r29_empty_admitted_workset_has_no_fabricated_fragment_reference() {
        let result=NproEngine::activate(&[],0.2,0.3);
        assert!(result.eliminations.iter().all(|e|e.fragment_refs.is_empty()),
            "no fragment 0 exists in an empty admitted workset");
        let measured=crate::candidate_measurement::measure_current_producer(
            "actual input with no admitted fragment",0,&[],&result.eliminations,0).unwrap();
        assert_eq!(measured.summary.positive_observed.count,0);
        assert_eq!(measured.summary.negative_observed.count,4);
        assert_eq!(measured.summary.negative_admitted.sum,0);
        assert_eq!(result.operators_run,4);
        let populated=NproEngine::activate(&["actual fragment".into()],0.2,0.3);
        assert_eq!(populated.eliminations[0].fragment_refs,vec![0]);
        assert_eq!(result.eliminations[0].weakness_score.to_bits(),
            populated.eliminations[0].weakness_score.to_bits());
        // The strict MPRO bounds check remains in force for foreign references.
        let mut invalid=result.eliminations;
        invalid[0].fragment_refs.push(0);
        assert_eq!(crate::candidate_measurement::measure_current_producer(
            "same scope",0,&[],&invalid,0).unwrap_err(),"MPRO_PRODUCER_FRAGMENT_RANGE");
    }


    fn directive() -> AshtOperatorDirective {
        AshtOperatorDirective::new(
            "trace-1".to_string(),
            "a".repeat(64),
            "b".repeat(64),
            "c".repeat(64),
            Vec::new(),
        )
    }

    fn negative_candidate(id: u64, confidence_fixed: u32) -> CandidateStatisticWire {
        CandidateStatisticWire {
            candidate_id: id,
            polarity: 0,
            domain: "negative".to_string(),
            term_sha256: "e".repeat(64),
            source_sha256: "f".repeat(64),
            security_y: 0,
            trust_x: 0,
            confidence_fixed,
            frequency: 1,
            domain_count: 1,
            legacy_score_fixed: 0,
        }
    }

    fn parity_for(negative: &[CandidateStatisticWire], positive_mass: u64) -> ParityEvidence {
        let negative_mass = polarity_mass(negative);
        let verdict = match negative_mass >= positive_mass {
            true => ParityVerdict::NegativeDominant,
            false => ParityVerdict::PositivePermitted,
        };
        ParityEvidence {
            verdict,
            positive_mass,
            negative_mass,
        }
    }

    #[test]
    fn directed_npro_requires_valid_asht_capability() {
        let fragments = vec!["bounded evidence".to_string()];
        let negatives = vec![negative_candidate(1, 2_000)];
        let parity = parity_for(&negatives, 5_000);
        let result =
            NproEngine::activate_directed(&fragments, 0.5, 0.5, &directive(), &negatives, parity);
        assert!(result.is_ok());
        let value = result.expect("directed NPRO");
        assert_eq!(
            value.candidate_parity,
            Some(ParityVerdict::PositivePermitted)
        );
        assert_eq!(value.ranked_negative_count, 1);

        let mut corrupt = directive();
        corrupt.directive_sha256 = "d".repeat(64);
        assert!(
            NproEngine::activate_directed(&fragments, 0.5, 0.5, &corrupt, &negatives, parity,)
                .is_err()
        );
    }

    #[test]
    fn directed_npro_rejects_unsorted_negative_candidates() {
        let fragments = vec!["bounded evidence".to_string()];
        let negatives = vec![negative_candidate(2, 1_000), negative_candidate(1, 9_000)];
        let parity = parity_for(&negatives, 9_000);
        assert!(NproEngine::activate_directed(
            &fragments,
            0.5,
            0.5,
            &directive(),
            &negatives,
            parity,
        )
        .is_err());
    }

    #[test]
    fn directed_npro_rejects_inconsistent_parity_mass() {
        let fragments = vec!["bounded evidence".to_string()];
        let negatives = vec![negative_candidate(1, 2_000)];
        let parity = ParityEvidence {
            verdict: ParityVerdict::PositivePermitted,
            positive_mass: 5_000,
            negative_mass: 999,
        };
        assert!(NproEngine::activate_directed(
            &fragments,
            0.5,
            0.5,
            &directive(),
            &negatives,
            parity,
        )
        .is_err());
    }

    #[test]
    fn npro_produces_four_eliminations() {
        let frags = vec![
            "the reactor is stable under load".to_string(),
            "the reactor is not stable under load".to_string(),
        ];
        let result = NproEngine::activate(&frags, 0.5, 0.5);
        assert_eq!(result.eliminations.len(), 4);
        assert_eq!(result.operators_run, 4);
    }

    #[test]
    fn weak_causal_raises_weakness() {
        let frags = vec!["a".to_string(), "b".to_string()];
        // Lidhje e dobët → dobësi e lartë te REVERSE⁻¹.
        let weak = NproEngine::activate(&frags, 0.1, 0.1);
        let reverse_elim = weak
            .eliminations
            .iter()
            .find(|e| e.operator.contains("Reverse"))
            .unwrap();
        assert!(reverse_elim.weakness_score > 0.5);
    }

    #[test]
    fn forbidden_paths_above_ceiling() {
        // Fragment i vetëm → CONTRAST⁻¹ dhe TRACE⁻¹ dobësi të larta.
        let frags = vec!["single fragment only".to_string()];
        let result = NproEngine::activate(&frags, 0.5, 0.5);
        // Të paktën një rrugë e ndaluar (contrast=1.0 > 0.65).
        assert!(!result.weakness_signal.forbidden_paths.is_empty());
    }

    #[test]
    fn weakness_signal_detects_forbidden() {
        let frags = vec!["x".to_string()];
        let result = NproEngine::activate(&frags, 0.5, 0.5);
        // CONTRAST⁻¹ jep 1.0 për fragment të vetëm → forbidden.
        assert!(result.weakness_signal.is_forbidden("Contrast"));
    }

    #[test]
    fn avg_weakness_computed() {
        let frags = vec!["a".to_string(), "b".to_string()];
        let result = NproEngine::activate(&frags, 0.5, 0.5);
        assert!((0.0..=1.0).contains(&result.weakness_signal.avg_weakness));
    }

    #[test]
    fn strong_input_few_forbidden() {
        // Input i fortë → pak forbidden paths.
        let frags = vec![
            "deterministic elimination produces verifiable knowledge".to_string(),
            "each candidate passes through filters before acceptance".to_string(),
            "surviving knowledge becomes primitive over time".to_string(),
        ];
        let result = NproEngine::activate(&frags, 0.9, 0.9);
        // Me input të fortë, forbidden paths < të gjithë.
        assert!(result.weakness_signal.forbidden_paths.len() < 4);
    }
}

impl NproEngine {
    /// R21 exact computational proposition only. This is local eligibility,
    /// never Shadow authority or evidence for an unstated physical claim.
    /// All generic operator observations remain in the original trace.
    pub fn kie_exact_model_support(
        proof:&shadow_contracts::kie::KieProof, actual_input:&str,
    )->Result<bool,String> {
        if proof.history_assessment.is_none() {return Ok(false);}
        let parity=Self::kie_parity(proof)?; // pinned provider and exact scope recomputed
        if actual_input.as_bytes()!=proof.canonical_proposition.as_bytes() {
            return Err("KIE_APPLICABILITY_EXACT_INPUT_REQUIRED".into());
        }
        Ok(proof.relation==0 && parity.verdict==ParityVerdict::PositivePermitted)
    }

    /// NEW KiE origin adapter. Original historical weighting is untouched.
    /// KiC exists as an evaluated critical predicate even when false; its
    /// Boolean mass is not historical evidence confidence or an observed fact.
    pub fn kie_parity(proof: &shadow_contracts::kie::KieProof) -> Result<ParityEvidence, String> {
        proof.validate()?;
        let actual = science_domains::kie::evaluate_kie(&proof.query_bytes, &proof.candidate_bytes)
            .map_err(|e| format!("KIE_NPRO_MODEL:{e}"))?;
        let query = science_domains::decode_query(&proof.query_bytes)
            .map_err(|e| format!("KIE_NPRO_QUERY:{e}"))?;
        let id = &proof.context.identity;
        if actual.canonical_proposition != proof.canonical_proposition
            || actual.relation != proof.relation
            || actual.kis_mass != proof.kis_mass
            || actual.kic_mass != proof.kic_mass
            || query.session_id != shadow_contracts::pd_memory::hex(&id.key()?)
            || query.case_id != id.case_id
            || query.scope_id != id.scope
            || query.obligation_id != id.obligation
        {
            return Err("KIE_NPRO_MODEL_SCOPE".into());
        }
        let outcome = shadow_contracts::kie::computed_outcome(proof)?;
        let positive_mass = outcome.combined.positive_mass;
        let negative_mass = outcome.combined.negative_mass;
        let verdict = match outcome.gate {
            shadow_contracts::kie::KieGateOutcome::NegativeDominant => ParityVerdict::NegativeDominant,
            shadow_contracts::kie::KieGateOutcome::PositiveEligible => ParityVerdict::PositivePermitted,
            shadow_contracts::kie::KieGateOutcome::Undetermined => ParityVerdict::Undetermined,
        };
        let parity = ParityEvidence {
            verdict,
            positive_mass,
            negative_mass,
        };
        if !parity.shape_is_valid(1) {
            return Err("KIE_NPRO_PARITY".into());
        }
        Ok(parity)
    }

    /// Same original four NPRO operators and output checks, with typed KiE
    /// predicate validation replacing only historical-candidate decoding.
    pub fn activate_kie_directed(
        fragments: &[String],
        signal: f32,
        causal: f32,
        directive: &AshtOperatorDirective,
        proof: &shadow_contracts::kie::KieProof,
        parity: ParityEvidence,
    ) -> Result<NproResult, String> {
        if !directive.shape_is_valid() {
            return Err("NPRO rejected invalid Asht directive".into());
        }
        if !signal.is_finite() || !(0.0..=1.0).contains(&signal) {
            return Err("NPRO rejected non-finite/out-of-range signal".into());
        }
        if !causal.is_finite() || !(0.0..=1.0).contains(&causal) {
            return Err("NPRO rejected non-finite/out-of-range causal mass".into());
        }
        let actual = Self::kie_parity(proof)?;
        if actual != parity
            || directive.selection_sha256 != proof.selection_sha256()
            || directive.allowed_candidate_sha256 != proof.predicate_references()
        {
            return Err("KIE_NPRO_DIRECTIVE_PROOF_BINDING".into());
        }
        let mut result = Self::activate(fragments, signal, causal);
        result.candidate_parity = Some(parity.verdict);
        result.positive_candidate_mass = parity.positive_mass;
        result.negative_candidate_mass = parity.negative_mass;
        result.ranked_negative_count = 1; // one typed KiC predicate, zero historical candidates.
        let authorized = result.eliminations.iter().all(|e| {
            let op = e.operator.to_ascii_uppercase();
            directive
                .allowed_operators
                .iter()
                .any(|allowed| op.contains(allowed))
        });
        if result.operators_run != 4
            || result.eliminations.len() != 4
            || !authorized
            || !result
                .eliminations
                .iter()
                .all(|e| e.weakness_score.is_finite() && (0.0..=1.0).contains(&e.weakness_score))
        {
            return Err("NPRO operator output escaped the Asht scope".into());
        }
        Ok(result)
    }
}
#[cfg(test)]
mod kie_tests {
    use super::*;
    fn proof(claim: f64) -> shadow_contracts::kie::KieProof {
        use shadow_contracts::{kie::*, pd_memory::*};
        let identity = PdIdentity {
            owner: "owner".into(),
            case_id: "case".into(),
            scope: "scope".into(),
            obligation: "model".into(),
            domain: "science".into(),
            lineage_root: "root".into(),
        };
        let q = science_domains::ScienceQuery {
            adapter: science_domains::AdapterId::Astronomy,
            session_id: shadow_contracts::pd_memory::hex(&identity.key().unwrap()),
            case_id: identity.case_id.clone(),
            scope_id: identity.scope.clone(),
            obligation_id: identity.obligation.clone(),
            query_id: "query".into(),
            input_source_id: "caller".into(),
            task: science_domains::ScienceTask::EarthRotationAngle {
                jd_part1: 2451545.0,
                jd_part2: 0.0,
                time_scale: science_domains::TimeScale::Ut1,
                claim_radians: claim,
                tolerance_radians: 0.1,
            },
        };
        let qb = science_domains::encode_query(&q).unwrap();
        let cb =
            science_domains::encode_candidate(&science_domains::evaluate(&q).unwrap()).unwrap();
        let e = science_domains::kie::evaluate_kie(&qb, &cb).unwrap();
        KieProof {
            context: KieContext {
                identity,
                operation_id: "op".into(),
                record_id: [3; 32],
            },
            query_bytes: qb,
            candidate_bytes: cb,
            historical_request: test_history(&e.canonical_proposition).0,
            historical_response: test_history(&e.canonical_proposition).1,
            canonical_proposition: e.canonical_proposition,
            kis_mass: e.kis_mass,
            kic_mass: e.kic_mass,
            relation: e.relation,
            proof_sha256: [0; 32],
            prior_adjudication: None,
            history_assessment: None,
        }
        .seal_data()
        .unwrap()
    }
    // Actual provider boundary, synthetic scoped hold data only: no native admission.
    fn unresolved_proof() -> shadow_contracts::kie::KieProof {
        let mut evidence = proof(0.0);
        let mut query = science_domains::decode_query(&evidence.query_bytes).unwrap();
        let boundary = science_domains::evaluate(&query).unwrap().circular_residual_radians;
        let science_domains::ScienceTask::EarthRotationAngle { tolerance_radians, .. } = &mut query.task;
        *tolerance_radians = boundary;
        let model = science_domains::evaluate(&query).unwrap();
        assert_eq!(model.relation, science_domains::ModelRelation::Unresolved);
        evidence.query_bytes = science_domains::encode_query(&query).unwrap();
        evidence.candidate_bytes = science_domains::encode_candidate(&model).unwrap();
        let actual = science_domains::kie::evaluate_kie(
            &evidence.query_bytes, &evidence.candidate_bytes,
        ).unwrap();
        evidence.canonical_proposition = actual.canonical_proposition;
        evidence.relation = actual.relation;
        evidence.kis_mass = actual.kis_mass;
        evidence.kic_mass = actual.kic_mass;
        let history = test_history(&evidence.canonical_proposition);
        evidence.historical_request = history.0;
        evidence.historical_response = history.1;
        evidence.seal_data().unwrap()
    }

    #[test]
    fn r10_unresolved_model_without_history_is_not_refutation() {
        let evidence = unresolved_proof();
        let combined = shadow_contracts::kie::combined_evaluation(&evidence).unwrap();
        assert_eq!((combined.historical_positive_count, combined.historical_negative_count), (0, 0));
        assert!(!combined.historical_veto);
        assert_eq!((combined.positive_mass, combined.negative_mass), (0, 0));
        let parity = NproEngine::kie_parity(&evidence).unwrap();
        assert!(!parity.verdict.permits_positive());
        assert_ne!(parity.verdict, ParityVerdict::NegativeDominant,
            "R10_UNRESOLVED_IS_NOT_REFUTING_EVIDENCE: exact model equality with no historical veto");
        assert_eq!(parity.verdict, ParityVerdict::Undetermined);
        assert!(parity.shape_is_valid(1));
    }

    #[test]
    fn r21_model_applicability_requires_actual_strict_compute_and_exact_input() {
        use shadow_contracts::kie::*;
        let seed=proof(0.0);
        let computed=science_domains::decode_candidate(&seed.candidate_bytes).unwrap().computed_radians;
        let legacy=proof(computed);
        assert_eq!(legacy.relation,0);
        assert!(!NproEngine::kie_exact_model_support(&legacy,&legacy.canonical_proposition).unwrap(),
            "KIP2 retains its original operator gate");
        // Actual provider compute with test-only history data; this is not a
        // fabricated admission into Shadow or a second scientific source.
        let classify=|mut p:KieProof| {
            p.history_assessment=Some(KieHistoryAssessment{scope_sha256:p.context.identity.key().unwrap(),
                proposition_sha256:digest(p.canonical_proposition.as_bytes()),entries:vec![],scoped_evidence:vec![]});
            p.seal_data().unwrap()
        };
        let support=classify(legacy);
        assert!(NproEngine::kie_exact_model_support(&support,&support.canonical_proposition).unwrap());
        for p in [classify(seed),classify(unresolved_proof())] {
            assert!(!NproEngine::kie_exact_model_support(&p,&p.canonical_proposition).unwrap());
        }
        assert_eq!(NproEngine::kie_exact_model_support(&support,"physical object actually rotates this way").unwrap_err(),
            "KIE_APPLICABILITY_EXACT_INPUT_REQUIRED");
        let mut foreign=support.clone();foreign.context.identity.case_id.push('x');
        foreign.history_assessment.as_mut().unwrap().scope_sha256=foreign.context.identity.key().unwrap();
        let foreign=foreign.seal_data().unwrap();
        assert_eq!(NproEngine::kie_exact_model_support(&foreign,&foreign.canonical_proposition).unwrap_err(),"KIE_NPRO_MODEL_SCOPE");
        let mut substituted=support;substituted.candidate_bytes[0]^=1;
        let substituted=substituted.seal_data().unwrap();
        assert!(NproEngine::kie_exact_model_support(&substituted,&substituted.canonical_proposition).is_err());
    }

    #[test]
    fn r10_real_model_support_and_counter_remain_distinct_from_equality() {
        let counter = proof(0.0);
        let computed = science_domains::decode_candidate(&counter.candidate_bytes).unwrap().computed_radians;
        let support = proof(computed);
        assert_eq!(NproEngine::kie_parity(&support).unwrap().verdict, ParityVerdict::PositivePermitted);
        assert_eq!(NproEngine::kie_parity(&counter).unwrap().verdict, ParityVerdict::NegativeDominant);
        assert_eq!(NproEngine::kie_parity(&unresolved_proof()).unwrap().verdict, ParityVerdict::Undetermined);
    }

    #[test]
    fn r10_unresolved_model_keeps_actual_relevant_historical_veto() {
        let mut p = unresolved_proof();
        let mut response = shadow_contracts::decode_selection_response(&p.historical_response).unwrap();
        response.negative.push(shadow_contracts::CandidateStatisticWire {
            candidate_id: 2, polarity: 0, domain: "negative".into(),
            term_sha256: shadow_contracts::sha256_hex(&[
                shadow_contracts::pd_memory::failure::relevance::TERM_NEGATIVE_DOMAIN,
                p.canonical_proposition.as_bytes(),
            ]),
            source_sha256: "e".repeat(64), security_y: 0, trust_x: 0,
            confidence_fixed: 6000, frequency: 1, domain_count: 1, legacy_score_fixed: 0,
        });
        response.selection_sha256 = shadow_contracts::pd_memory::failure::relevance::selection_digest(&response);
        p.historical_response = shadow_contracts::encode_selection_response(&response);
        let p = p.seal_data().unwrap();
        let outcome = shadow_contracts::kie::computed_outcome(&p).unwrap();
        assert_eq!(outcome.model, shadow_contracts::kie::KieModelOutcome::Unresolved);
        assert!(outcome.combined.historical_veto);
        assert_eq!(NproEngine::kie_parity(&p).unwrap().verdict, ParityVerdict::NegativeDominant);
        assert!(!outcome.combined.permits_positive);
    }

    #[test]
    fn typed_kic_keeps_original_operators_and_detects_source_mismatch() {
        let proof = proof(0.0);
        let parity = NproEngine::kie_parity(&proof).unwrap();
        assert_eq!(parity.verdict, ParityVerdict::NegativeDominant);
        let d = AshtOperatorDirective::new(
            "attempt".into(),
            "a".repeat(64),
            "b".repeat(64),
            proof.selection_sha256(),
            proof.predicate_references(),
        );
        let f = vec![
            "explicit model premise".into(),
            "critical comparison".into(),
        ];
        let original = NproEngine::activate(&f, 0.7, 0.8);
        let typed = NproEngine::activate_kie_directed(&f, 0.7, 0.8, &d, &proof, parity).unwrap();
        assert_eq!(original.operators_run, typed.operators_run);
        assert_eq!(
            original
                .eliminations
                .iter()
                .map(|e| e.weakness_score.to_bits())
                .collect::<Vec<_>>(),
            typed
                .eliminations
                .iter()
                .map(|e| e.weakness_score.to_bits())
                .collect::<Vec<_>>()
        );
        let mut bad = d.clone();
        bad.allowed_candidate_sha256.clear();
        bad.directive_sha256 = bad.recompute_sha256();
        assert!(NproEngine::activate_kie_directed(&f, 0.7, 0.8, &bad, &proof, parity).is_err());
        assert!(NproEngine::activate_kie_directed(&f, f32::NAN, 0.8, &d, &proof, parity).is_err());
        let mut lie = proof.clone();
        lie.canonical_proposition = "unrelated physical claim".into();
        assert!(lie.seal_data().is_err());
    }

    #[derive(Clone)]
    struct ModelTestHardware {
        scaffold: crate::lim_types::EpistemicScaffold,
        lease: Option<crate::hw_real::HwLease>,
        work: bool,
        operator_mask: u8,
    }
    impl crate::pro::HwScaffoldView for ModelTestHardware {
        fn epistemic(&self) -> &crate::lim_types::EpistemicScaffold { &self.scaffold }
        fn parallel_count(&self) -> u8 { 1 }
        fn runtime_lease(&self) -> Option<crate::hw_real::HwLease> { self.lease }
        fn work_permitted(&self) -> bool { self.work }
        fn operator_allowed(&self, operator: crate::lim_types::Operator) -> bool {
            let bit=match operator {crate::lim_types::Operator::Reverse=>1,
                crate::lim_types::Operator::Mirror=>2,crate::lim_types::Operator::Contrast=>4,
                crate::lim_types::Operator::Trace=>8};
            self.operator_mask & bit != 0
        }
    }
    fn model_test_hardware(proof: &shadow_contracts::kie::KieProof) -> ModelTestHardware {
        let mut lim=crate::lim::LinearInfoMetricEngine::new("unit-local", "science");
        lim.collect("model evidence carries exact input and output");
        lim.collect("scientific proof carries a declared mathematical scope");
        ModelTestHardware { scaffold:lim.pre_pro_scan().unwrap(), work:true,operator_mask:0,
            lease:Some(crate::hw_real::HwLease {lease_id:7,
                granted_bytes:(shadow_contracts::kie_measurement::KieMeasurement::WIRE_BYTES+
                    proof.query_bytes.len()+proof.candidate_bytes.len()) as u64,
                granted_parallel:1,floor_bytes:100,reserved_cores:1,flexed:false,active:true}) }
    }
    fn model_test_directive(proof:&shadow_contracts::kie::KieProof)->AshtOperatorDirective {
        AshtOperatorDirective::new("attempt".into(),"a".repeat(64),"b".repeat(64),
            proof.selection_sha256(),proof.predicate_references())
    }

    #[test]
    fn r28_real_model_endpoints_route_kis_and_kic_separately_and_preserve_original_stop() {
        use crate::pro::{ProEngine,HwScaffoldView};
        use shadow_contracts::model_operator_measurement::{ModelOperatorMeasurements,ModelOperatorOutcome};
        let counter=proof(0.0);
        let computed=science_domains::decode_candidate(&counter.candidate_bytes).unwrap().computed_radians;
        for proof in [proof(computed),counter,unresolved_proof()] {
            let hw=model_test_hardware(&proof);
            let original_lease=hw.lease;
            let directive=model_test_directive(&proof);
            let kis=ProEngine::measure_kis(&hw,&proof,&directive,true).unwrap().unwrap();
            let kic=NproEngine::measure_kic(&hw,&proof,&directive,true).unwrap().unwrap();
            assert_eq!(kis,kic,"independent endpoints must compute the same model evidence");
            assert_eq!(hw.lease,original_lease,"no new or enlarged lease");
            assert_eq!(hw.operator_mask,0);
            for op in [crate::lim_types::Operator::Reverse,crate::lim_types::Operator::Mirror,
                crate::lim_types::Operator::Contrast,crate::lim_types::Operator::Trace] {
                assert!(!hw.operator_allowed(op),"scientific side cannot reopen a lexical stop");
            }
            let summary=ModelOperatorMeasurements::new(kis,true,true).unwrap().summarize().unwrap();
            assert!(summary.pro.predicate_tested && summary.npro.predicate_tested);
            assert_eq!(summary.pro.candidates.count,u32::from(proof.relation==0));
            assert_eq!(summary.npro.candidates.count,u32::from(proof.relation==1));
            assert_eq!(summary.pro.outcome,if proof.relation==0 {ModelOperatorOutcome::Admitted} else {ModelOperatorOutcome::Absent});
            assert_eq!(summary.npro.outcome,if proof.relation==1 {ModelOperatorOutcome::Admitted} else {ModelOperatorOutcome::Absent});
            assert_eq!(summary.unresolved,proof.relation==2);
            assert_eq!(summary.pro.candidates.mean_radians.is_some(),proof.relation==0);
            assert_eq!(summary.npro.candidates.mean_radians.is_some(),proof.relation==1);
        }
    }

    #[test]
    fn r28_real_model_endpoints_hold_on_missing_capacity_without_claiming_tested_zero() {
        let proof=proof(0.0);let good=model_test_hardware(&proof);let d=model_test_directive(&proof);
        for scenario in 0..7 {
            let mut hw=good.clone();let mut sealed=true;
            match scenario {
                0=>hw.lease=None,
                1=>hw.lease.as_mut().unwrap().active=false,
                2=>hw.lease.as_mut().unwrap().granted_bytes-=1,
                3=>hw.lease.as_mut().unwrap().granted_parallel=0,
                4=>hw.lease.as_mut().unwrap().lease_id=0,
                5=>hw.work=false,
                _=>sealed=false,
            }
            assert!(crate::pro::ProEngine::measure_kis(&hw,&proof,&d,sealed).unwrap().is_none(),"scenario {scenario}");
            assert!(NproEngine::measure_kic(&hw,&proof,&d,sealed).unwrap().is_none(),"scenario {scenario}");
        }
    }

    #[test]
    fn r28_real_model_endpoints_reject_resealed_foreign_scope_and_wrong_directive() {
        let proof=proof(0.0);let hw=model_test_hardware(&proof);let good=model_test_directive(&proof);
        for field in 0..3 {
            let mut bad=good.clone();
            match field {0=>bad.selection_sha256="f".repeat(64),
                1=>bad.allowed_candidate_sha256.clear(),_=>bad.allowed_candidate_sha256.reverse()}
            bad.directive_sha256=bad.recompute_sha256();
            assert!(crate::pro::ProEngine::measure_kis(&hw,&proof,&bad,true).is_err());
            assert!(NproEngine::measure_kic(&hw,&proof,&bad,true).is_err());
        }
        let mut foreign=proof.clone();foreign.context.identity.case_id.push_str("-other");
        let foreign=foreign.seal_data().unwrap();
        let foreign_directive=model_test_directive(&foreign);
        let foreign_hw=model_test_hardware(&foreign);
        assert!(crate::pro::ProEngine::measure_kis(&foreign_hw,&foreign,&foreign_directive,true).is_err(),
            "a valid digest does not bind a model query to a foreign case");
        assert!(NproEngine::measure_kic(&foreign_hw,&foreign,&foreign_directive,true).is_err());
    }

}
/// Data-only fixture utility scoped to crate tests; no authoritative hold issued.
#[cfg(test)]
fn test_history(text: &str) -> (Vec<u8>, Vec<u8>) {
    let q = shadow_contracts::KnowledgeSelectionRequestWire {
        version: shadow_contracts::PROTOCOL_VERSION,
        requester: shadow_contracts::SelectionRequester::AshtQuantum,
        purpose: shadow_contracts::SelectionPurpose::EvidenceCandidates,
        session_id: "attempt".into(),
        parent_i0: shadow_contracts::kie::hex(&shadow_contracts::kie::digest(text.as_bytes())),
        primitive_anchor: 1,
        trace_id: "attempt".into(),
        split_sha256: "a".repeat(64),
        xi: vec![text.into()],
        yi: vec![],
        upstream_hold_sha256: Some("b".repeat(64)),
        evidence_sha256: "c".repeat(64),
        max_positive: shadow_contracts::SELECTION_MAX_CANDIDATES,
        max_negative: shadow_contracts::SELECTION_MAX_CANDIDATES,
    };
    let qb = shadow_contracts::encode_selection_request(&q);
    let mut s = shadow_contracts::KnowledgeSelectionResponseWire {
        version: shadow_contracts::PROTOCOL_VERSION,
        requester: q.requester,
        session_id: q.session_id.clone(),
        parent_i0: q.parent_i0.clone(),
        primitive_anchor: q.primitive_anchor,
        split_sha256: q.split_sha256.clone(),
        upstream_hold_sha256: q.upstream_hold_sha256.clone(),
        request_sha256: shadow_contracts::sha256_hex(&[b"ESSMAI/SELECTION/REQUEST/V178", &qb]),
        selection_sha256: String::new(),
        hold_id: "historical-fixture".into(),
        hold_sha256: "d".repeat(64),
        positive: vec![],
        negative: vec![],
        law_seal: 1,
        system_laws_seal: 1,
        expires_at_ns: 1,
    };
    s.selection_sha256 = shadow_contracts::pd_memory::failure::relevance::selection_digest(&s);
    (qb, shadow_contracts::encode_selection_response(&s))
}
