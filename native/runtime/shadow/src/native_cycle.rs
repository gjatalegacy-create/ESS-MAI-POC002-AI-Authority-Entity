//! Native response durability under the existing Shadow authority.
//! NEW derivative: request identity follows shadow_gateway::commit_decision.
//! This module neither judges evidence nor issues a verification seal.
use crate::shadow_gateway::ShadowResponse;
use crate::sovereign_ffi_gate::VerificationReceipt;
use crate::vault_transaction::{sha256_parts, VaultError, VaultTransaction};
use sha2::{Digest, Sha256};
use shadow_contracts::{
    ShadowCycleRequest, ShadowCycleResponse, ShadowVerdictWire, VerificationReceiptWire,
};

pub(crate) const MAX_NATIVE_RESPONSE_BYTES: usize = 1024 * 1024;

/// R3 pd_mp reference only; all checkpoint, source and receipt material stays in
/// its existing authoritative Vault T1/T2 records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NativePdHead {
    pub(crate) identity: [u8; 32],
    pub(crate) request_sha256: [u8; 32],
    pub(crate) transaction_id: [u8; 32],
    pub(crate) root_identity: [u8; 32],
}
pub(crate) fn cognitive_scope_key(session: &str, territory: &str) -> [u8; 32] {
    sha256_parts(&[
        b"ESSMAI/NATIVE/PD-MAP/SCOPE/V1",
        &(session.len() as u64).to_le_bytes(), session.as_bytes(),
        &(territory.len() as u64).to_le_bytes(), territory.as_bytes(),
    ])
}


/// Query result keeps MODEL_CONDITIONAL origin and actual native receipt apart.
#[derive(Debug,Clone)]
pub(crate) struct KieAdjudication {
    pub(crate) commit: crate::vault_transaction::DurableCommit,
    pub(crate) evidence_family_digest: [u8;32],
}
#[derive(Debug,Clone,Copy,PartialEq,Eq,Hash)]
pub(crate) struct KieHistoryKey {
    pub(crate) scope:[u8;32], pub(crate) proposition:[u8;32], pub(crate) family:[u8;32],
}
/// One pinned model/task is one family, even under different operation/query IDs.
pub(crate) fn kie_family(candidate:&science_domains::CandidateEvidence)->[u8;32] {
    let science_domains::ScienceTask::EarthRotationAngle {jd_part1,jd_part2,time_scale,
        claim_radians,tolerance_radians}=candidate.query.task;
    let scale=match time_scale {science_domains::TimeScale::Ut1=>0,science_domains::TimeScale::Utc=>1,
        science_domains::TimeScale::Tai=>2,science_domains::TimeScale::Tt=>3,science_domains::TimeScale::Tdb=>4};
    sha256_parts(&[b"ESSMAI/KIE/MODEL-FAMILY/1",&candidate.provenance.compiled_model_sha256,
        &[scale],&jd_part1.to_bits().to_le_bytes(),&jd_part2.to_bits().to_le_bytes(),
        &claim_radians.to_bits().to_le_bytes(),&tolerance_radians.to_bits().to_le_bytes()])
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeRequestBinding {
    pub(crate) request_sha256: [u8; 32],
    pub(crate) model_case: Option<crate::case_reasoning::VerifiedModelCase>,
    pub(crate) identity_digest: [u8; 32],
    pub(crate) prepared: Vec<u8>,
    pub(crate) kie_proof: Vec<u8>,
}

impl NativeRequestBinding {
    /// Attach only the Shadow-revalidated proof to this exact current native
    /// request. The proof context is its historical PDS1 origin, not this attempt.
    pub(crate) fn attach_kie(&mut self, proof:&shadow_contracts::kie::KieProof,
        request:&ShadowCycleRequest) -> Result<(),String> {
        proof.validate()?;
        if !self.prepared.is_empty() && shadow_contracts::pd_prepared::decode_prepared(&self.prepared)?.source_iz.is_some() {
            return Err("PD_IZ_KIE_ROUTE_MIX".into());
        }
        let bytes=shadow_contracts::encode_cycle_request(request);
        let expected=Self::from_request(request,&bytes)?;
        if expected.identity_digest!=self.identity_digest || expected.request_sha256!=self.request_sha256
            || request.quantum.final_evidence.light_input_bytes!=proof.canonical_proposition.as_bytes()
            || request.quantum.final_evidence.light_input_sha256.to_ascii_lowercase()
                !=shadow_contracts::pd_memory::hex(&Sha256::digest(proof.canonical_proposition.as_bytes()).into()) {
            return Err("KIE_CURRENT_REQUEST_BINDING".into());
        }
        if !self.prepared.is_empty() {
            let prepared=shadow_contracts::pd_prepared::decode_prepared(&self.prepared)?;
            validate_failure_witness(prepared.failure_witness.as_ref(),Some(request),Some(proof))?;
        }
        let encoded=shadow_contracts::kie::encode_proof(proof)?;
        if !self.kie_proof.is_empty() && self.kie_proof!=encoded {return Err("KIE_ATTACHMENT_CONFLICT".into());}
        self.model_case=crate::case_reasoning::VerifiedModelCase::from_request(proof,request)?;
        self.kie_proof=encoded;Ok(())
    }
    pub(crate) fn from_request(request: &ShadowCycleRequest, bytes: &[u8]) -> Result<Self, String> {
        if shadow_contracts::encode_cycle_request(request) != bytes {
            return Err("NATIVE_REQUEST_NONCANONICAL".into());
        }
        let session = &request.quantum.session_id;
        let parent = request
            .quantum
            .pd_parent_i0
            .as_deref()
            .ok_or("NATIVE_REQUEST_PARENT_MISSING")?;
        let anchor = parent
            .strip_prefix("i0-")
            .and_then(|s| u64::from_str_radix(s, 16).ok())
            .filter(|a| *a != 0)
            .ok_or("NATIVE_REQUEST_ANCHOR_INVALID")?;
        if session.is_empty()
            || session != &request.light.session_id
            || parent != format!("i0-{anchor:016x}")
        {
            return Err("NATIVE_REQUEST_IDENTITY_MISMATCH".into());
        }
        Ok(Self {
            request_sha256: Sha256::digest(bytes).into(),
            model_case: None,
            identity_digest: identity_digest(session, parent, anchor),
            prepared:Vec::new(),
            kie_proof:Vec::new(),
        })
    }
}

pub(crate) fn identity_digest(session: &str, parent: &str, anchor: u64) -> [u8; 32] {
    // Exact source formula. Kept distinct from the progressive-memory identity.
    sha256_parts(&[
        b"ESSMAI_SHADOW_IDENTITY_V1",
        session.as_bytes(),
        parent.as_bytes(),
        &anchor.to_le_bytes(),
    ])
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeCycleSnapshot {
    pub(crate) request_sha256: [u8; 32],
    pub(crate) response: Vec<u8>,
    pub(crate) prepared:Vec<u8>,
    pub(crate) kie_proof:Vec<u8>,
    pub(crate) model_judgment: Option<([u8;32],u64)>,
}
impl NativeCycleSnapshot {
    pub(crate) fn new(
        binding: &NativeRequestBinding,
        response: ShadowCycleResponse,
    ) -> Result<Self, String> {
        let model_judgment = binding.model_case.as_ref().filter(|c| c.uses_model_judgment())
            .map(|case|case.judgment_binding());
        crate::model_judgment_trace::check_binding(&response.verdict.lgc_dsl, model_judgment)?;
        let bytes = shadow_contracts::encode_cycle_response(&response);
        if bytes.len() > MAX_NATIVE_RESPONSE_BYTES {
            return Err("NATIVE_RESPONSE_LIMIT".into());
        }
        Ok(Self {
            request_sha256: binding.request_sha256,
            response: bytes,
            prepared:binding.prepared.clone(),
            kie_proof:binding.kie_proof.clone(),
            model_judgment,
        })
    }
    pub(crate) fn validate(&self, tx: &VaultTransaction) -> Result<(), VaultError> {
        let err =
            || VaultError::CorruptTransaction("native request/response snapshot mismatch".into());
        if self.request_sha256 == [0; 32] || self.response.len() > MAX_NATIVE_RESPONSE_BYTES {
            return Err(err());
        }
        let wire = shadow_contracts::decode_cycle_response(&self.response).map_err(|_| err())?;
        if shadow_contracts::encode_cycle_response(&wire) != self.response
            || wire.session_id != tx.receipt.session_id
            || wire.verification_receipt != receipt_wire(&tx.receipt)
            || wire.verdict.verified != tx.y
            || wire.verdict.primitive != tx.x
            || wire.verdict.living_trust_sha256
                != crate::living_trust_contract::hex_32(&tx.living_trust_sha256)
            || identity_digest(
                &tx.receipt.session_id,
                &tx.receipt.parent_i0,
                tx.receipt.primitive_anchor,
            ) != tx.identity_digest
        {
            return Err(err());
        }
        if !self.prepared.is_empty() {
            let prepared=shadow_contracts::pd_prepared::decode_prepared(&self.prepared).map_err(|_|err())?;
            if prepared.source_request_sha256!=self.request_sha256
                || prepared.pending.session_id!=tx.receipt.session_id
                || prepared.pending.parent_i0!=tx.receipt.parent_i0
                || prepared.pending.binding_digest().map_err(|_|err())?!=tx.receipt.pd_binding_digest {
                return Err(err());
            }
            let proof=if self.kie_proof.is_empty() {None} else {
                Some(shadow_contracts::kie::decode_proof(&self.kie_proof)
                    .map_err(|reason|VaultError::CorruptTransaction(format!("KIE_SNAPSHOT:{reason}")))?)
            };
            validate_failure_witness(prepared.failure_witness.as_ref(),None,proof.as_ref())
                .map_err(|reason|VaultError::CorruptTransaction(format!("PD_FAILURE_WITNESS:{reason}")))?;
            if prepared.source_iz.is_some() && !self.kie_proof.is_empty() {
                return Err(VaultError::CorruptTransaction("PD_IZ_KIE_ROUTE_MIX".into()));
            }
            // Validate every current prerequisite before append, without
            // forming final iZ text. Output digests are checked after durable
            // T1 and before the separately persisted T2 or Light delivery.
            shadow_contracts::pd_prepared::validate_continuation_prerequisites(&prepared, &wire)
                .map_err(|reason| VaultError::CorruptTransaction(format!("PD_T1_CANNOT_FINALIZE:{reason}")))?;
        }
        if self.kie_proof.is_empty() && !self.prepared.is_empty() {
            let envelope=shadow_contracts::pd_prepared::decode_prepared(&self.prepared).map_err(|_|err())?;
            if envelope.source_dialogue.is_some() {return Err(VaultError::CorruptTransaction("DIALOGUE_KIE_PROOF_REQUIRED".into()));}
        }
        if !self.kie_proof.is_empty() {
            let proof=shadow_contracts::kie::decode_proof(&self.kie_proof)
                .map_err(|reason|VaultError::CorruptTransaction(format!("KIE_SNAPSHOT:{reason}")))?;
            if self.prepared.is_empty() {return Err(VaultError::CorruptTransaction("KIE_PREPARED_REQUIRED".into()));}
            let historical=shadow_contracts::decode_selection_request(&proof.historical_request).map_err(|_|err())?;
            if historical.session_id!=tx.receipt.session_id || historical.trace_id!=tx.receipt.session_id {
                return Err(VaultError::CorruptTransaction("KIE_HISTORICAL_CURRENT_ATTEMPT_MISMATCH".into()));
            }
            let envelope=shadow_contracts::pd_prepared::decode_prepared(&self.prepared).map_err(|_|err())?;
            let current=shadow_contracts::pd_checkpoint_data::checkpoint_metadata(
                &envelope.checkpoint,envelope.cognitive_session_id()).map_err(|_|err())?;
            if current.last_user_text!=proof.canonical_proposition {
                return Err(VaultError::CorruptTransaction("KIE_CURRENT_PROPOSITION_MISMATCH".into()));
            }
            if envelope.cognitive_session_id()!=shadow_contracts::pd_memory::hex(&proof.context.identity.key().map_err(|_|err())?)
                || (envelope.continuity.is_none() && envelope.source_dialogue.is_none()) {
                return Err(VaultError::CorruptTransaction("KIE_DIALOGUE_HISTORY_REQUIRED".into()));
            }
            if let Some(link)=&envelope.source_dialogue {
                if link.context!=proof.context {return Err(VaultError::CorruptTransaction("KIE_DIALOGUE_ORIGIN_MISMATCH".into()));}
            }
            validate_kie_verdict(&proof,tx.y,tx.x)?;
            if let Some((binding,_))=self.model_judgment {
                crate::case_reasoning::VerifiedModelCase::verify_persisted_binding(
                    &proof,&tx.receipt.session_id,binding,tx.y==1)
                    .map_err(VaultError::CorruptTransaction)?;
            }
        }
        let expected_kind=match (tx.y,tx.x) {
            (1,1)=>crate::living_trust_contract::TRUST_KIND_CONSTRUCTIVE,
            (0,0)=>crate::living_trust_contract::TRUST_KIND_RIGOROUS_NEGATIVE,
            _=>return Err(err()),
        };
        let trust=crate::living_trust_contract::LivingTrustSeal {
            identity_sha256:tx.receipt.living_trust_sha256,
            intensity:wire.verdict.living_trust_intensity,
            kind:wire.verdict.living_trust_kind,
            sovereign_value:wire.verdict.living_trust_sovereign_value,
        };
        if trust.identity_sha256==[0;32]
            || trust.intensity>crate::living_trust_contract::TRUST_INTENSITY_SCALE
            || trust.kind!=expected_kind
            || trust.sovereign_value!=crate::living_trust_contract::SOVEREIGN_SEAL_PRIMITIVE
            || trust.identity_digest()!=tx.receipt.living_trust_digest {
            return Err(err());
        }
        if self.model_judgment.is_some() && self.kie_proof.is_empty() {
            return Err(VaultError::CorruptTransaction("MODEL_TRACE_PROOF_REQUIRED".into()));
        }
        crate::model_judgment_trace::validate(&wire.verdict.lgc_dsl, &tx.lineage, tx.y, self.model_judgment)
            .map_err(VaultError::CorruptTransaction)?;
        if wire.negative_persisted && !tx.knowledge_writes.iter().any(|w|matches!(w,crate::shadow_commit::KnowledgeWrite::Negative{..})) {
            return Err(err());
        }
        Ok(())
    }
}

/// R10: the witness is data about the actual failure, not an authority token.
/// Live admission supplies the original request; persisted snapshots retain its
/// SHA binding and can recheck the stored model material without inventing input.
/// Legacy envelopes without a witness retain their original validation path.
fn validate_failure_witness(
    witness:Option<&shadow_contracts::pd_failure_witness::PdFailureWitness>,
    request:Option<&ShadowCycleRequest>,
    proof:Option<&shadow_contracts::kie::KieProof>,
)->Result<(),String> {
    let Some(witness)=witness else {return Ok(());};
    if let Some(request)=request {witness.validate_for_request(request)?;}
    if let Some(proof)=proof {crate::kie_selection::validate_proof_model(proof)?;}
    witness.validate_model(proof)?;
    if proof.is_some() && witness.inquiry.as_ref().is_some_and(|frontier|
        frontier.uses_current_processing() && frontier.model_measurement().is_none()) {
        return Err("KIE_MEASUREMENT_OWNER_REQUIRED".into());
    }
    if let Some(measurement) = witness.inquiry.as_ref().and_then(|frontier| frontier.model_measurement()) {
        let proof = proof.ok_or("KIE_MEASUREMENT_OWNER_PROOF")?;
        let expected = crate::kie_selection::measure_current_model(proof)?;
        if measurement != &expected {
            return Err("KIE_MEASUREMENT_OWNER_RECOMPUTE".into());
        }
    }
    Ok(())
}

/// Storage backstop only: a source receipt cannot turn critical supplied proof
/// into a positive T1. This never grants a positive verdict by itself.
fn validate_kie_verdict(proof:&shadow_contracts::kie::KieProof,y:u8,x:u8)->Result<(),VaultError> {
    proof.validate().map_err(|e|VaultError::CorruptTransaction(format!("KIE_PROOF:{e}")))?;
    let combined=shadow_contracts::kie::combined_evaluation(proof)
        .map_err(|e|VaultError::CorruptTransaction(format!("KIE_COMBINED:{e}")))?;
    if (y,x)==(1,1) && !combined.permits_positive {
        return Err(VaultError::CorruptTransaction("KIE_POSITIVE_CONTRADICTS_CRITICAL_PROOF".into()));
    }
    Ok(())
}

fn receipt_wire(receipt: &VerificationReceipt) -> VerificationReceiptWire {
    VerificationReceiptWire {
        session_id: receipt.session_id.clone(),
        parent_i0: receipt.parent_i0.clone(),
        primitive_anchor: receipt.primitive_anchor,
        xy_digest: receipt.xy_digest,
        pd_binding_digest: receipt.pd_binding_digest,
        pd_continuum_activation_digest: receipt.pd_continuum_activation_digest,
        living_trust_digest: receipt.living_trust_digest,
        living_trust_sha256: crate::living_trust_contract::hex_32(&receipt.living_trust_sha256),
        y_verdict: receipt.y_verdict,
        x_verdict: receipt.x_verdict,
        seal: receipt.seal,
        generation: receipt.generation,
        receipt_id: receipt.receipt_id.clone(),
    }
}

pub(crate) fn response_wire(response: ShadowResponse) -> ShadowCycleResponse {
    let receipt = response.verification_receipt;
    let verdict = response.verdict;
    ShadowCycleResponse {
        session_id: response.session_id,
        verdict: ShadowVerdictWire {
            verified: verdict.verified,
            primitive: verdict.primitive,
            knowledge_band: verdict.knowledge_band.as_u8(),
            lgc_law: verdict.lgc_law,
            legacy_score: verdict.legacy_score,
            lgc_dsl: verdict.lgc_dsl,
            living_trust_action_state: verdict.living_trust_action_state,
            living_trust_action_mask: verdict.living_trust_action_mask,
            living_trust_required_action_mask: verdict.living_trust_required_action_mask,
            living_trust_system_laws_seal: verdict.living_trust_system_laws_seal,
            living_trust_sovereign_flags: verdict.living_trust_sovereign_flags,
            living_trust_sovereign_value: verdict.living_trust_sovereign_value,
            living_trust_sha256: crate::living_trust_contract::hex_32(&verdict.living_trust_sha256),
            living_trust_intensity: verdict.living_trust_intensity,
            living_trust_kind: verdict.living_trust_kind,
            project_id: verdict.project_id,
            project_status: verdict.project_status,
            project_context_digest: verdict.project_context_digest,
            project_evidence_digest: verdict.project_evidence_digest,
            project_context_sha256: crate::gcl_project_contract::sha256_hex(
                &verdict.project_context_sha256,
            ),
            project_evidence_sha256: crate::gcl_project_contract::sha256_hex(
                &verdict.project_evidence_sha256,
            ),
            novel_factualized: verdict.novel_factualized,
            novel_trl_level: verdict.novel_trl_level,
            novel_proof_score: verdict.novel_proof_score,
            novel_rejection: verdict.novel_rejection,
        },
        verification_receipt: VerificationReceiptWire {
            session_id: receipt.session_id,
            parent_i0: receipt.parent_i0,
            primitive_anchor: receipt.primitive_anchor,
            xy_digest: receipt.xy_digest,
            pd_binding_digest: receipt.pd_binding_digest,
            pd_continuum_activation_digest: receipt.pd_continuum_activation_digest,
            living_trust_digest: receipt.living_trust_digest,
            living_trust_sha256: crate::living_trust_contract::hex_32(&receipt.living_trust_sha256),
            y_verdict: receipt.y_verdict,
            x_verdict: receipt.x_verdict,
            seal: receipt.seal,
            generation: receipt.generation,
            receipt_id: receipt.receipt_id,
        },
        negative_persisted: response.negative_persisted,
    }
}

#[cfg(test)]
#[path = "native_cycle_tests.rs"]
mod tests;
