//! Current-case model material reconstructed by Shadow, before Spine and Matrix.
//! This is verified data, not a capability or an alternative verdict.
use shadow_contracts::{case_reasoning::ModelReasoning, kie::KieProof, ShadowCycleRequest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VerifiedModelCase {
    model: ModelReasoning,
    current_evidence_digest: u64,
    model_judgment: bool,
    history_permits_positive: bool,
    original_empty: bool,
    conservation_accounted: bool,
}
impl VerifiedModelCase {
    pub(crate) fn from_request(proof: &KieProof, request: &ShadowCycleRequest)
        -> Result<Option<Self>, String> {
        let final_evidence = &request.quantum.final_evidence;
        let ledger = final_evidence.candidate_measurement()?.map(|entry| entry.0);
        let current = ledger.as_ref().is_some_and(|value| value.uses_case_reasoning());
        let has_record = final_evidence.pim_proof_chain.iter()
            .any(|entry| entry.starts_with(shadow_contracts::case_reasoning::PROOF_SOURCE));
        if !current {
            if has_record { return Err("CASE_RECORD_WITHOUT_VERSION".into()); }
            return Ok(None);
        }
        let ledger = ledger.ok_or("CASE_LEDGER_REQUIRED")?;
        ledger.validate_model(proof)?;
        crate::kie_selection::validate_proof_model(proof)?;
        let measured = crate::kie_selection::measure_current_model(proof)?;
        let model = ledger.model_measurements().ok_or("CASE_MODEL_REQUIRED")?;
        if model.measurement() != &measured { return Err("CASE_OWNER_RECOMPUTE".into()); }
        let hcp = final_evidence.quantum_action_evidence.iter()
            .find(|entry| entry.stage == shadow_contracts::ACTION_STAGE_HCP)
            .ok_or("CASE_HCP_REQUIRED")?;
        let performed = shadow_contracts::model_operator_measurement::resource_permitted(proof, &hcp.evidence_words)?;
        if model.pro_permitted() != performed || model.npro_permitted() != performed {
            return Err("CASE_ENDPOINT_PERMISSION_MISMATCH".into());
        }
        if request.quantum.session_id != request.light.session_id
            || final_evidence.light_input_bytes != proof.canonical_proposition.as_bytes() {
            return Err("CASE_CURRENT_INPUT_MISMATCH".into());
        }
        let model = ModelReasoning::new(&request.quantum.session_id, proof, model.clone())?;
        model.verify_record(&final_evidence.pim_proof_chain)?;
        let srk = final_evidence.quantum_action_evidence.iter()
            .find(|entry| entry.stage == shadow_contracts::ACTION_STAGE_SRK);
        let original_empty = ledger.observations().iter().all(|item|
            item.polarity != shadow_contracts::candidate_measurement::CandidatePolarity::Positive)
            && request.quantum.candidate_scores.is_empty()
            && srk.is_some_and(|entry| matches!(entry.evidence_words.as_slice(),
                [0, _, 0, 0, _, _]));
        let conservation_accounted = original_empty || (request.quantum.conservation_ok
            && srk.is_some_and(|entry| matches!(entry.evidence_words.as_slice(),
                [_, _, total, 0, _, _] if *total > 0)));
        Ok(Some(Self { model, current_evidence_digest: final_evidence.recompute_digest(),
            model_judgment: ledger.uses_model_judgment(),
            history_permits_positive: shadow_contracts::kie::combined_evaluation(proof)?.permits_positive,
            original_empty, conservation_accounted }))
    }
    /// Replay verifies the mathematical model/case identity from retained proof.
    /// The historical hardware lease is not reissued; its committed outcome is
    /// one of the two symmetric endpoint states encoded by the original binding.
    pub(crate) fn verify_persisted_binding(proof: &KieProof, cycle: &str,
        binding: [u8;32], positive: bool) -> Result<(), String> {
        crate::kie_selection::validate_proof_model(proof)?;
        let measured = crate::kie_selection::measure_current_model(proof)?;
        for permitted in [false, true] {
            let model = shadow_contracts::model_operator_measurement::ModelOperatorMeasurements::new(
                measured.clone(), permitted, permitted)?;
            let candidate = ModelReasoning::new(cycle, proof, model)?;
            if candidate.binding() == binding && (!positive || permitted) { return Ok(()); }
        }
        Err("MODEL_PERSISTED_CASE_RECOMPUTE".into())
    }
    pub(crate) fn judgment_binding(&self) -> ([u8; 32], u64) {
        (self.model.binding(), self.current_evidence_digest)
    }
    pub(crate) fn uses_model_judgment(&self) -> bool { self.model_judgment }
    pub(crate) fn history_permits_positive(&self) -> bool { self.history_permits_positive }
    pub(crate) fn conservation_accounted(&self) -> bool { self.conservation_accounted }
    pub(crate) fn original_empty(&self) -> bool { self.original_empty }
    pub(crate) fn model_only(&self, pkg: &crate::types::PassPackage) -> bool {
        pkg.scientific_project.is_none()
    }
    pub(crate) fn admissible(&self, pkg: &crate::types::PassPackage) -> bool {
        self.uses_model_judgment() && self.matches(pkg) && self.model_only(pkg)
            && self.conservation_accounted && self.history_permits_positive
            && self.positive_count() == 1 && self.failure_modes().is_empty()
    }
    /// Retain the original scalar as a diagnostic. The categorical verdict is
    /// the conjunction of this node's declared-model obligations, never a score.
    pub(crate) fn review(&self, pkg: &crate::types::PassPackage,
        mut diagnostic: crate::types::ShadowVerdict, predicates: &[bool])
        -> crate::types::ShadowVerdict {
        let scope = self.uses_model_judgment() && self.matches(pkg) && predicates.len() <= 6;
        let missing = predicates.iter().take(6).enumerate().fold(u8::from(!scope),
            |mask, (i, ok)| mask | (u8::from(!ok) << (i + 1)));
        diagnostic.verdict = u8::from(missing == 0);
        diagnostic.reason_code = 0x80 | missing;
        println!("[MODEL_JUDGMENT_V5] node={} missing={missing:02x} bit={} diagnostic={:.6} {}",
            diagnostic.node.label(), diagnostic.verdict, diagnostic.score, self.model.statement());
        diagnostic
    }
    pub(crate) fn model(&self) -> &ModelReasoning { &self.model }
    pub(crate) fn matches(&self, package: &crate::types::PassPackage) -> bool {
        self.model.cycle() == package.package_id && self.current_evidence_digest == package.final_evidence_digest
    }
    pub(crate) fn positive_count(&self) -> u32 {
        self.model.summary().expect("owner-verified model").pro.candidates.count
    }
    pub(crate) fn failure_modes(&self) -> Vec<&'static str> {
        let s = self.model.summary().expect("owner-verified model");
        let mut modes = Vec::new();
        if !s.pro.predicate_tested { modes.push("model_support_not_tested"); }
        if !s.npro.predicate_tested { modes.push("model_counter_not_tested"); }
        if s.npro.candidates.count != 0 { modes.push("model_counterproof_present"); }
        if s.unresolved { modes.push("model_comparison_unresolved"); }
        modes
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use shadow_contracts::{candidate_measurement::CandidateLedger, model_operator_measurement::ModelOperatorMeasurements};
    use sha2::{Digest, Sha256};

    fn captured() -> (KieProof, ShadowCycleRequest) {
        let (bytes,_) = shadow_contracts::pd_cycle::decode_cycle(
            include_bytes!("test_fixtures/late_refusal_r25/attempt.pdt1")).unwrap();
        let proof=include_bytes!("test_fixtures/late_refusal_r25/proof.kie");
        let n=u32::from_le_bytes(proof[4..8].try_into().unwrap()) as usize;
        let response=shadow_contracts::kie::decode_response(&proof[8+n..]).unwrap();
        (response.proof,shadow_contracts::decode_cycle_request(&bytes).unwrap())
    }
    // Reuse captured scoped input only. The ledger/lease below are unit fixture
    // data; no live authority is issued and no Vault is opened.
    pub(crate) fn material() -> (KieProof, ShadowCycleRequest) {
        let (proof,mut request)=captured();
        let model=ModelOperatorMeasurements::new(
            crate::kie_selection::measure_current_model(&proof).unwrap(),true,true).unwrap();
        let ledger=CandidateLedger::new(Sha256::digest(proof.canonical_proposition.as_bytes()).into(),vec![])
            .unwrap().with_case_reasoning(model.clone()).unwrap();
        let case=ModelReasoning::new(&request.quantum.session_id,&proof,model).unwrap();
        let e=&mut request.quantum.final_evidence;
        e.quantum_action_evidence.iter_mut().find(|v|v.stage==shadow_contracts::ACTION_STAGE_MPRO)
            .unwrap().evidence_words=ledger.to_words();
        e.quantum_action_evidence.iter_mut().find(|v|v.stage==shadow_contracts::ACTION_STAGE_HCP)
            .unwrap().evidence_words=vec![1,1,1,0,1,1,0,0,1024*1024,1,1];
        e.pim_proof_chain.push(case.wire_record());
        (proof,request)
    }
    pub(crate) fn judgment_material() -> (KieProof, ShadowCycleRequest) {
        let (proof, mut request) = material();
        request.quantum.final_evidence.quantum_action_evidence.iter_mut()
            .find(|v| v.stage == shadow_contracts::ACTION_STAGE_MPRO).unwrap().evidence_words[1] = 5;
        (proof, request)
    }
    #[test]
    fn r32_shadow_owner_recomputes_exact_model_and_case_before_consumption() {
        let (proof,request)=material();
        let case=VerifiedModelCase::from_request(&proof,&request).unwrap().unwrap();
        assert_eq!(case.positive_count(),1);
        assert!(case.failure_modes().is_empty());
        assert_eq!(case.model().cycle(),request.quantum.session_id);
        let (proof,old)=captured();
        assert!(VerifiedModelCase::from_request(&proof,&old).unwrap().is_none());
    }
    #[test]
    fn r32_shadow_rejects_missing_duplicate_changed_or_foreign_case_record() {
        let (proof,request)=material();
        for scenario in 0..4 {
            let mut wrong=request.clone();
            let chain=&mut wrong.quantum.final_evidence.pim_proof_chain;
            let record=chain.last().unwrap().clone();
            match scenario {
                0=>{chain.pop();},
                1=>chain.push(record),
                2=>chain.last_mut().unwrap().push(','),
                _=>wrong.quantum.session_id.push_str("-other"),
            }
            assert!(VerifiedModelCase::from_request(&proof,&wrong).is_err(),"{scenario}");
        }
    }
    #[test]
    fn r32_shadow_rejects_self_consistent_result_digest_and_false_resource_flags() {
        let (proof,request)=material();
        let mut wrong=request.clone();
        let actual=crate::kie_selection::measure_current_model(&proof).unwrap();
        let mut bytes=actual.encode().unwrap();bytes[128]^=1;
        let model=ModelOperatorMeasurements::new(
            shadow_contracts::kie_measurement::KieMeasurement::decode(&bytes).unwrap(),true,true).unwrap();
        let ledger=CandidateLedger::new(Sha256::digest(proof.canonical_proposition.as_bytes()).into(),vec![])
            .unwrap().with_case_reasoning(model.clone()).unwrap();
        wrong.quantum.final_evidence.quantum_action_evidence.iter_mut()
            .find(|v|v.stage==shadow_contracts::ACTION_STAGE_MPRO).unwrap().evidence_words=ledger.to_words();
        *wrong.quantum.final_evidence.pim_proof_chain.last_mut().unwrap()=
            ModelReasoning::new(&wrong.quantum.session_id,&proof,model).unwrap().wire_record();
        assert_eq!(VerifiedModelCase::from_request(&proof,&wrong).unwrap_err(),"CASE_OWNER_RECOMPUTE");
        for index in [4,5,8,9,10] {
            let mut wrong=request.clone();
            wrong.quantum.final_evidence.quantum_action_evidence.iter_mut()
                .find(|v|v.stage==shadow_contracts::ACTION_STAGE_HCP).unwrap().evidence_words[index]=0;
            assert_eq!(VerifiedModelCase::from_request(&proof,&wrong).unwrap_err(),"CASE_ENDPOINT_PERMISSION_MISMATCH");
        }
    }
    #[test]
    fn r32_case_version_cannot_lose_required_model_record_or_relabel_legacy() {
        let (proof,mut request)=material();
        let e=&mut request.quantum.final_evidence;
        e.quantum_action_evidence.iter_mut().find(|v|v.stage==shadow_contracts::ACTION_STAGE_MPRO)
            .unwrap().evidence_words[1]=3;
        assert_eq!(VerifiedModelCase::from_request(&proof,&request).unwrap_err(),"CASE_RECORD_WITHOUT_VERSION");
    }
    #[test]
    fn r33_missing_model_history_permission_or_conservation_never_admits() {
        let (proof, request)=judgment_material();
        let case=VerifiedModelCase::from_request(&proof,&request).unwrap().unwrap();
        let pkg=crate::types::PassPackage {package_id:request.quantum.session_id.clone(),
            final_evidence_digest:request.quantum.final_evidence.recompute_digest(),..Default::default()};
        assert!(case.admissible(&pkg));
        for deficit in 0..5 {
            let mut missing=case.clone();
            match deficit {
                0=>missing.history_permits_positive=false,
                1=>missing.conservation_accounted=false,
                2=>missing.model_judgment=false,
                3=>missing.current_evidence_digest^=1,
                _=>{
                    let held=ModelOperatorMeasurements::new(missing.model.model().measurement().clone(),true,false).unwrap();
                    missing.model=ModelReasoning::new(&request.quantum.session_id,&proof,held).unwrap();
                }
            }
            assert!(!missing.admissible(&pkg),"{deficit}");
        }
        let diagnostic=crate::types::ShadowVerdict::from_score(crate::types::ShadowNode::Type,1.0,0,1);
        assert_eq!(case.review(&pkg,diagnostic,&[true;7]).verdict,0);
    }
    #[test]
    fn r33_persisted_binding_recomputes_exact_model_and_cycle() {
        let (proof,request)=judgment_material();
        let case=VerifiedModelCase::from_request(&proof,&request).unwrap().unwrap();
        VerifiedModelCase::verify_persisted_binding(&proof,&request.quantum.session_id,case.model.binding(),true).unwrap();
        assert!(VerifiedModelCase::verify_persisted_binding(&proof,&request.quantum.session_id,[7;32],true).is_err());
        assert!(VerifiedModelCase::verify_persisted_binding(&proof,"foreign",case.model.binding(),true).is_err());
    }
    #[test]
    fn r33_conservation_absence_exception_requires_all_empty_witnesses() {
        let (proof,request)=judgment_material();
        let case=VerifiedModelCase::from_request(&proof,&request).unwrap().unwrap();
        assert!(case.original_empty() && case.conservation_accounted());
        for field in 0..4 {
            let mut wrong=request.clone();
            wrong.quantum.conservation_ok=false;
            match field {
                0=>wrong.quantum.candidate_scores.push(0.1),
                _=>wrong.quantum.final_evidence.quantum_action_evidence.iter_mut()
                    .find(|v|v.stage==shadow_contracts::ACTION_STAGE_SRK).unwrap()
                    .evidence_words[[0,0,2,3][field]]=1,
            }
            let blocked=VerifiedModelCase::from_request(&proof,&wrong).unwrap().unwrap();
            assert!(!blocked.original_empty() && !blocked.conservation_accounted(),"{field}");
        }
    }

}
