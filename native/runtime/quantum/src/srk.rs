// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  SRK.RS — SCIENTIFIC REASONING KERNEL — ORCHESTRATORI                 ║
// ║  GJATA LEGACY™ — Quantum Platform                                     ║
// ║                                                                          ║
// ║  POZICIONI: MES PRO dhe PIM.                                          ║
// ║   PRO → kandidatë                                                     ║
// ║   SRK → filtro (conservation) → IBE score → Bayesian →               ║
// ║         ReasoningPackage                                              ║
// ║   PIM → paketon vetëm ç'kaloi SRK                                     ║
// ║                                                                          ║
// ║  SRK NUK vendos (verdict=None). NUK shkruan memory.                  ║
// ╚══════════════════════════════════════════════════════════════════════════╝

use crate::pro_types::{PROCandidate, ProResult};
use crate::srk_ibe::{ConservationEngine, IbeScorer, UncertaintyEngine};
use crate::srk_types::*;
use crate::ultimatum_collapse_law::AshtOperatorDirective;

pub struct ScientificReasoningKernel {
    session_id: String,
    territory: String,
}

impl ScientificReasoningKernel {
    pub fn new(session_id: &str, territory: &str) -> Self {
        Self {
            session_id: session_id.to_string(),
            territory: territory.to_string(),
        }
    }

    /// Directed production entrypoint. Every PRO candidate must belong to an
    /// operator family authorized by the sealed Asht capability.
    pub fn process_directed(
        &self,
        pro_result: &ProResult,
        directive: &AshtOperatorDirective,
    ) -> Result<ReasoningPackage, String> {
        match directive.shape_is_valid() {
            true => {}
            false => return Err("SRK rejected invalid Asht directive".to_string()),
        }
        match pro_result.candidates.iter().all(|candidate| {
            directive.allows_operator(candidate.operator.name())
                && candidate.score.is_finite()
                && candidate.confidence.is_finite()
                && (0.0..=1.0).contains(&candidate.score)
                && (0.0..=1.0).contains(&candidate.confidence)
        }) {
            true => Ok(self.process(pro_result)),
            false => Err("SRK rejected a candidate outside the Asht scope".to_string()),
        }
    }

    pub fn process_case_directed(
        &self, pro: &ProResult, directive: &AshtOperatorDirective,
        model: Option<(&shadow_contracts::kie::KieProof,
            shadow_contracts::model_operator_measurement::ModelOperatorMeasurements)>,
    ) -> Result<ReasoningPackage, String> {
        let mut package = self.process_directed(pro, directive)?;
        if let Some((proof, measured)) = model {
            if directive.trace_id != self.session_id
                || directive.selection_sha256 != proof.selection_sha256()
                || directive.allowed_candidate_sha256 != proof.predicate_references() {
                return Err("SRK_CASE_DIRECTIVE_MISMATCH".into());
            }
            let actual = crate::kie_measurement::measure(proof)?;
            if measured.measurement() != &actual {
                return Err("SRK_CASE_MODEL_RECOMPUTE".into());
            }
            package.model_reasoning = Some(shadow_contracts::case_reasoning::ModelReasoning::new(
                &self.session_id, proof, measured)?);
        }
        Ok(package)
    }

    /// PROCESS — merr ProResult → prodhon ReasoningPackage.
    ///
    /// Hapat (besnik ndaj srk_core.pseudo):
    ///   1. filter_conservation_violators (eliminon ata që shkelin ligjet)
    ///   2. IBE evaluate për secilin që mbijetoi
    ///   3. rendit sipas ibe_score
    ///   4. uncertainty (Bayesian) mbi shpjegimet
    ///   5. ReasoningPackage me shadow_note
    pub fn process(&self, pro_result: &ProResult) -> ReasoningPackage {
        let candidates = &pro_result.candidates;

        // ── 1. CONSERVATION FILTER ─────────────────────────────────────────
        // Ndaj kandidatët: ata që kalojnë vs ata që shkelin ligjet.
        let mut surviving: Vec<&PROCandidate> = Vec::new();
        let mut all_checks: Vec<ConservationCheck> = Vec::new();
        let mut eliminated = 0usize;

        for c in candidates {
            let checks = ConservationEngine::check(c);
            let passes = ConservationEngine::passes(&checks);
            // Mblidh check-et (për proof chain).
            for ch in &checks {
                all_checks.push(ch.clone());
            }
            // push kushtëzuar përmes match (jo if/else).
            match passes {
                true => surviving.push(c),
                false => eliminated += 1,
            }
        }

        // ── 2. IBE për secilin që mbijetoi ─────────────────────────────────
        let mut explanations: Vec<AbductiveExplanation> = Vec::new();
        for c in &surviving {
            // Parametrat e IBE nga vetitë e kandidatit + konteksti.
            let phenomena_covered = (c.fragment_refs.len() as f32 / 5.0).clamp(0.1, 1.0);
            let causal_complexity = 1.0 + c.fragment_refs.len() as f32 * 0.3;
            let theory_coherence = c.confidence; // proxy
            let new_entities = (c.fragment_refs.len() as f32 * 0.5).max(1.0);
            // conservation_ok për këtë kandidat (e rikontrollojmë lokalisht).
            let conservation_ok = ConservationEngine::passes(&ConservationEngine::check(c));

            let exp = IbeScorer::evaluate(
                c,
                phenomena_covered,
                causal_complexity,
                theory_coherence,
                new_entities,
                conservation_ok,
            );
            explanations.push(exp);
        }

        // ── 3. Rendit sipas ibe_score (zbritës) ────────────────────────────
        explanations.sort_by(|a, b| {
            b.ibe_score
                .partial_cmp(&a.ibe_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let best_explanation = explanations.first().cloned();

        // ── 4. UNCERTAINTY (Bayesian) ──────────────────────────────────────
        let uncertainty = UncertaintyEngine::compute(&explanations);

        // ── 5. Evidence chain (proof-carrying) ─────────────────────────────
        let evidence_chain = Self::build_evidence(&explanations, pro_result);

        // ── 6. Shadow note (orientim, JO vendim) ───────────────────────────
        let shadow_note = Self::build_shadow_note(&best_explanation, &uncertainty, eliminated);

        ReasoningPackage {
            package_id: format!("SRK_{}_{}", self.territory, self.session_id),
            model_reasoning: None,
            explanations,
            best_explanation,
            uncertainty,
            evidence_chain,
            conservation_checks: all_checks,
            conservation_eliminated: eliminated,
            shadow_note,
            origin: "SRK".to_string(),
        }
    }

    fn build_evidence(
        explanations: &[AbductiveExplanation],
        pro_result: &ProResult,
    ) -> Vec<Evidence> {
        let mut chain = Vec::new();
        // Evidencë nga LIM (masa).
        chain.push(Evidence {
            source: "LIM".to_string(),
            claim: format!("epistemic_mass={:.3}", pro_result.source_mass),
            verifiable: true,
        });
        // Evidencë nga secili shpjegim (IBE).
        for e in explanations {
            chain.push(Evidence {
                source: "SRK".to_string(),
                claim: format!("{} IBE={:.3}", e.source_operator.name(), e.ibe_score),
                verifiable: true,
            });
        }
        chain
    }

    fn build_shadow_note(
        best: &Option<AbductiveExplanation>,
        uncertainty: &UncertaintyProfile,
        eliminated: usize,
    ) -> String {
        match best {
            Some(b) => format!(
                "SRK→Shadow: best={} IBE={:.3} | combined_uncertainty={:.3} | conservation_eliminated={}",
                b.source_operator.name(), b.ibe_score,
                uncertainty.combined, eliminated,
            ),
            None => format!(
                "SRK→Shadow: asnjë shpjegim mbijetoi (conservation_eliminated={})",
                eliminated,
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lim_types::Operator;

    fn pro_result(candidates: Vec<PROCandidate>) -> ProResult {
        ProResult {
            candidates,
            operators_run: 4,
            nk_blocked: 0,
            source_mass: 0.5,
            hw_parallel: 4,
            workers_joined: 4,
            fragments_admitted: 1,
            working_set_bytes: 64,
        }
    }

    fn cand(id: &str, op: Operator, score: f32, conf: f32, refs: Vec<usize>) -> PROCandidate {
        PROCandidate {
            candidate_id: id.to_string(),
            operator: op,
            score,
            confidence: conf,
            reasoning: "r".to_string(),
            fragment_refs: refs,
        }
    }

    #[test]
    fn srk_produces_package() {
        let srk = ScientificReasoningKernel::new("s1", "science");
        let pr = pro_result(vec![
            cand("C1", Operator::Reverse, 0.8, 0.75, vec![0, 1]),
            cand("C2", Operator::Trace, 0.6, 0.6, vec![0, 1, 2]),
        ]);
        let pkg = srk.process(&pr);
        assert!(pkg.best_explanation.is_some());
        assert_eq!(pkg.origin, "SRK");
        // Renditur sipas IBE
        for w in pkg.explanations.windows(2) {
            assert!(w[0].ibe_score >= w[1].ibe_score);
        }
    }

    #[test]
    fn srk_eliminates_conservation_violators() {
        let srk = ScientificReasoningKernel::new("s1", "science");
        let pr = pro_result(vec![
            cand("C1", Operator::Reverse, 0.8, 0.75, vec![0, 1]), // OK
            cand("C2", Operator::Mirror, 0.5, 0.5, vec![]),       // pa refs → violated
            cand("C3", Operator::Trace, 0.99, 0.99, vec![0]),     // overconfident → violated
        ]);
        let pkg = srk.process(&pr);
        assert_eq!(pkg.conservation_eliminated, 2);
        assert_eq!(pkg.explanations.len(), 1);
    }

    #[test]
    fn srk_never_decides() {
        // verdict s'ekziston në ReasoningPackage — SRK NUK vendos.
        let srk = ScientificReasoningKernel::new("s1", "x");
        let pkg = srk.process(&pro_result(vec![cand(
            "C1",
            Operator::Reverse,
            0.7,
            0.7,
            vec![0, 1],
        )]));
        assert_eq!(pkg.origin, "SRK");
        assert!(pkg.shadow_note.contains("SRK→Shadow"));
    }
}


#[cfg(test)]
mod case_tests {
    use super::*;
    use shadow_contracts::model_operator_measurement::ModelOperatorMeasurements;
    fn fixture_proof(claim: f64) -> shadow_contracts::kie::KieProof {
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
    #[test]
    fn r32_srk_must_consume_the_actual_scientific_candidate() {
        let counter=fixture_proof(0.0);
        let value=science_domains::decode_candidate(&counter.candidate_bytes).unwrap().computed_radians;
        let proof=fixture_proof(value);
        let model=ModelOperatorMeasurements::new(crate::kie_measurement::measure(&proof).unwrap(),true,true).unwrap();
        let directive=AshtOperatorDirective::new("attempt".into(),"a".repeat(64),"b".repeat(64),
            proof.selection_sha256(),proof.predicate_references());
        let pro=ProResult { candidates:vec![], operators_run:0,nk_blocked:0,source_mass:0.5,
            hw_parallel:0,workers_joined:0,fragments_admitted:0,working_set_bytes:0 };
        let srk=ScientificReasoningKernel::new("attempt","science");
        let pkg=srk.process_case_directed(&pro,&directive,Some((&proof,model))).unwrap();
        assert!(pkg.model_reasoning.is_some(),"KiS was produced but lost before SRK/PIM");
        let summary=pkg.model_reasoning.as_ref().unwrap().summary().unwrap();
        assert_eq!(summary.pro.candidates.count,1);
        assert_eq!(summary.npro.candidates.count,0);
        assert!(pkg.explanations.is_empty(),"a model result must not fabricate a lexical candidate");
        let state=crate::lim_types::EpistemicState::PotentialHypothesis {
            confidence_range:(0.36,0.98),stability:pkg.uncertainty.combined,trl_candidate:2,pro_mode:"deep".into()};
        let pass=crate::pim::PassInfoMetric::measure(&pkg,&state,"attempt");
        let baseline=crate::pim::PassInfoMetric::measure(&srk.process_directed(&pro,&directive).unwrap(),&state,"attempt");
        assert_eq!(pass.potentiality.evidence_density.to_bits(),baseline.potentiality.evidence_density.to_bits());
        assert_eq!(pass.potentiality.logical_coherence.to_bits(),baseline.potentiality.logical_coherence.to_bits());
        let records=pass.proof_chain.iter().map(|p|format!("{}|{}|{}",p.source,p.claim,u8::from(p.verifiable))).collect::<Vec<_>>();
        pass.model_reasoning.as_ref().unwrap().verify_record(&records).unwrap();
        assert!(!pass.shadow_suggestion.is_directive);

    }
    fn empty_pro() -> ProResult {
        ProResult {candidates:vec![],operators_run:0,nk_blocked:0,source_mass:0.5,
            hw_parallel:0,workers_joined:0,fragments_admitted:0,working_set_bytes:0}
    }
    fn directive(proof:&shadow_contracts::kie::KieProof)->AshtOperatorDirective {
        AshtOperatorDirective::new("attempt".into(),"a".repeat(64),"b".repeat(64),
            proof.selection_sha256(),proof.predicate_references())
    }
    #[test]
    fn r32_srk_preserves_counter_and_held_without_positive_inflation() {
        let proof=fixture_proof(0.0);
        for performed in [false,true] {
            let model=ModelOperatorMeasurements::new(crate::kie_measurement::measure(&proof).unwrap(),performed,performed).unwrap();
            let result=ScientificReasoningKernel::new("attempt","science")
                .process_case_directed(&empty_pro(),&directive(&proof),Some((&proof,model))).unwrap();
            let summary=result.model_reasoning.unwrap().summary().unwrap();
            assert_eq!(summary.pro.candidates.count,0);
            assert_eq!(summary.npro.candidates.count,u32::from(performed));
            assert_eq!(summary.npro.predicate_tested,performed);
        }
    }
    #[test]
    fn r32_case_wrong_cycle_directive_and_duplicate_record_are_rejected() {
        let proof=fixture_proof(0.0);
        let model=ModelOperatorMeasurements::new(crate::kie_measurement::measure(&proof).unwrap(),true,true).unwrap();
        assert!(ScientificReasoningKernel::new("other-cycle","science")
            .process_case_directed(&empty_pro(),&directive(&proof),Some((&proof,model.clone()))).is_err());
        let mut wrong=directive(&proof);wrong.selection_sha256="f".repeat(64);
        wrong.directive_sha256=wrong.recompute_sha256();
        assert!(ScientificReasoningKernel::new("attempt","science")
            .process_case_directed(&empty_pro(),&wrong,Some((&proof,model.clone()))).is_err());
        let case=shadow_contracts::case_reasoning::ModelReasoning::new("attempt",&proof,model).unwrap();
        let entry=case.wire_record();
        assert!(case.verify_record(&[]).is_err());
        assert!(case.verify_record(&[entry.clone(),entry.clone()]).is_err());
        case.verify_record(&[entry]).unwrap();
    }
    #[test]
    fn r32_srk_recomputes_model_even_when_self_consistent_hashes_are_supplied() {
        let counter=fixture_proof(0.0);
        let output=science_domains::decode_candidate(&counter.candidate_bytes).unwrap().computed_radians;
        let proof=fixture_proof(output);
        let actual=crate::kie_measurement::measure(&proof).unwrap();
        let mut bytes=actual.encode().unwrap();
        // An altered result identity passes local arithmetic shape checks; it
        // must still fail the producer's independent provider comparison.
        bytes[128]^=1;
        let forged=shadow_contracts::kie_measurement::KieMeasurement::decode(&bytes).unwrap();
        let model=ModelOperatorMeasurements::new(forged,true,true).unwrap();
        assert_eq!(ScientificReasoningKernel::new("attempt","science")
            .process_case_directed(&empty_pro(),&directive(&proof),Some((&proof,model)))
            .unwrap_err(),"SRK_CASE_MODEL_RECOMPUTE");
    }

}
