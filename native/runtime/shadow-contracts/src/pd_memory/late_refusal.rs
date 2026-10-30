//! R25: data for one actually observed, late Shadow structural refusal.
//! PLF1 is an operational NoVerdict event, never a native receipt, counterproof,
//! user turn, or permission to append. Only the Shadow owner may admit it.
//! The original PDS record and its LGC remain in the same immutable Vault.
use super::*;

pub const CONDITION: &str = "SHADOW_MISSING_NEGATIVE_PROOF";
pub const OUTPUT: &str = "Shadow did not issue a verdict: the structural condition requiring a real negative proof for (0,0) was not satisfied. The claim has not thereby been refuted.";
const SITE: &str = "shadow::late_structural_refusal";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LateRefusalSource {
    pub context: crate::kie::KieContext,
    pub parent_revision: u64,
    pub parent_state: Hash,
    pub parent_source_sha256: Hash,
    pub parent_lgc_tip: Hash,
    /// Exact original PDS source bytes, so an ensuing real reply can recover
    /// its own context without a recursive or global-history lookup.
    pub original_source: Vec<u8>,
    pub attempt_sha256: Hash,
    /// Exact original base native frame, before any attempted verdict.
    pub attempt: Vec<u8>,
    pub prepared: Vec<u8>,
    /// Shadow-revalidated current proof, not a new model or knowledge entry.
    pub kie_proof: Vec<u8>,
    /// Validated targets only; question prose is formed after owner persistence.
    pub frontier: Vec<u8>,
}

pub fn operation_id(attempt_sha256: &Hash) -> String {
    format!("late-shadow-{}", hex(attempt_sha256))
}
fn sha(bytes: &[u8]) -> Hash { Sha256::digest(bytes).into() }
pub fn source_is_late(bytes: &[u8]) -> bool { bytes.starts_with(b"PLF1") }

/// Retains the originating real inquiry. This does not create a new user turn,
/// copy a successful ASHT observation, or fabricate a field-obligation frontier.
pub fn original_context(source:&LateRefusalSource) -> Result<iz::IzContext,String> {
    source.validate()?;
    Ok(iz_science::decode_input(&source.original_source)?.context)
}
pub fn checkpoint_raw(request:&PdOperationRequest) -> Result<&[u8],String> {
    let source=validate_operation(request)?;
    let input=iz_science::decode_input(&source.original_source)?;
    request.checkpoint.strip_prefix(input.checkpoint_prefix()).ok_or_else(||"PD_LATE_CHECKPOINT_POLICY".into())
}

impl LateRefusalSource {
    pub fn validate(&self) -> Result<(), String> {
        self.context.validate()?;
        if self.parent_revision == 0 || [self.parent_state, self.parent_source_sha256,
            self.parent_lgc_tip, self.attempt_sha256].contains(&[0;32]) {
            return Err("PD_LATE_PARENT_UNBOUND".into());
        }
        if self.attempt.is_empty() || self.prepared.is_empty() || self.kie_proof.is_empty()
            || self.original_source.is_empty()
            || self.attempt.len().saturating_add(self.prepared.len()).saturating_add(self.kie_proof.len()).saturating_add(self.original_source.len()).saturating_add(self.frontier.len()) > MAX_SOURCE_BYTES {
            return Err("PD_LATE_MATERIAL_LIMIT".into());
        }
        if sha(&self.attempt) != self.attempt_sha256 { return Err("PD_LATE_ATTEMPT_SHA".into()); }
        if sha(&self.original_source) != self.parent_source_sha256
            || !iz_science::source_is_science(&self.original_source) {
            return Err("PD_LATE_ORIGINAL_SOURCE".into());
        }
        let original=iz_science::decode_input(&self.original_source)?;
        if original.context.identity != self.context.identity || original.context.operation_id != self.context.operation_id {
            return Err("PD_LATE_ORIGINAL_CONTEXT".into());
        }
        let attempt = crate::decode_cycle_request(&self.attempt).map_err(|e|e.to_string())?;
        if crate::encode_cycle_request(&attempt) != self.attempt { return Err("PD_LATE_ATTEMPT_CANONICAL".into()); }
        let prepared = crate::pd_prepared::decode_prepared(&self.prepared)?;
        prepared.validate_for_request(&self.attempt)?;
        let proof = crate::kie::decode_proof(&self.kie_proof)?;
        if prepared.source_dialogue.as_ref().map(|link|&link.context) != Some(&self.context)
            || prepared.source_iz.is_some() || proof.context != self.context
            || proof.history_assessment.is_none()
            || attempt.quantum.final_evidence.light_input_bytes != proof.canonical_proposition.as_bytes()
            || attempt.quantum.final_evidence.light_input_sha256.to_ascii_lowercase() != hex(&sha(proof.canonical_proposition.as_bytes()))
            || attempt.quantum.territory != self.context.identity.scope {
            return Err("PD_LATE_CURRENT_SOURCE_BINDING".into());
        }
        let frontier=crate::iz_frontier::IzFrontier::decode(&self.frontier)?;
        frontier.validate_refusal(Some(CONDITION))?;
        frontier.validate_model(Some(&proof))?;
        frontier.validate_current_request(&attempt)?;
        if frontier!=current_frontier(&attempt,&prepared,&proof)? {
            return Err("PD_LATE_FRONTIER_BINDING".into());
        }
        // Bound the eventual current output plus question without forming it
        // before persistence. Reject excess; never truncate a proof target.
        let frontier_bound=frontier.rendered_byte_bound()?;
        let final_bound=OUTPUT.len().checked_add(1)
            .and_then(|bytes|bytes.checked_add(frontier_bound)).ok_or("PD_LATE_RENDER_OVERFLOW")?;
        if final_bound>crate::iz_frontier::MAX_RENDER_BYTES { return Err("PD_LATE_RENDER_LIMIT".into()); }
        Ok(())
    }
}

pub fn encode_source(source: &LateRefusalSource) -> Result<Vec<u8>, String> {
    source.validate()?;
    let mut out = b"PLF1".to_vec();
    put_bytes(&mut out, &crate::kie::encode_context(&source.context)?, MAX_TEXT_BYTES)?;
    out.extend_from_slice(&source.parent_revision.to_le_bytes());
    for value in [source.parent_state, source.parent_source_sha256, source.parent_lgc_tip, source.attempt_sha256] {
        out.extend_from_slice(&value);
    }
    for value in [&source.original_source, &source.attempt, &source.prepared, &source.kie_proof, &source.frontier] {
        put_bytes(&mut out, value, MAX_SOURCE_BYTES)?;
    }
    if out.len() > MAX_SOURCE_BYTES { return Err("PD_LATE_SOURCE_LIMIT".into()); }
    Ok(out)
}
pub fn decode_source(bytes: &[u8]) -> Result<LateRefusalSource, String> {
    if bytes.len() > MAX_SOURCE_BYTES { return Err("PD_LATE_SOURCE_LIMIT".into()); }
    let mut r=Reader{bytes,pos:0};
    if r.take(4)? != b"PLF1" { return Err("PD_LATE_SOURCE_VERSION".into()); }
    let source=LateRefusalSource {
        context: crate::kie::decode_context(&r.bytes(MAX_TEXT_BYTES)?)?,
        parent_revision:r.u64()?, parent_state:r.hash()?, parent_source_sha256:r.hash()?,
        parent_lgc_tip:r.hash()?, attempt_sha256:r.hash()?,
        original_source:r.bytes(MAX_SOURCE_BYTES)?,attempt:r.bytes(MAX_SOURCE_BYTES)?, prepared:r.bytes(MAX_SOURCE_BYTES)?, kie_proof:r.bytes(MAX_SOURCE_BYTES)?,frontier:r.bytes(MAX_SOURCE_BYTES)?,
    };
    r.end()?;
    if encode_source(&source)? != bytes { return Err("PD_LATE_SOURCE_NONCANONICAL".into()); }
    Ok(source)
}

fn operation_data(source:&LateRefusalSource, checkpoint:Vec<u8>) -> Result<PdOperationRequest,String> {
    let source_input=encode_source(source)?;
    let input_sha256=sha(&source_input);
    let first=LgcStep::new(0,input_sha256,SITE,"bind exact attempted native request",source.attempt_sha256)?;
    let second=LgcStep::new(1,first.step_digest,SITE,"observe MissingNegativeProof; no verdict issued",sha(CONDITION.as_bytes()))?;
    // PDQ1 requires this field. For explicit PLF1 only it binds pending target
    // data, not question prose. Map/LGC question hashes bind this descriptor.
    let question=format!("PLF1/IZ-PENDING/{}",hex(&sha(&source.frontier)));
    Ok(PdOperationRequest {
        identity:source.context.identity.clone(), operation_id:operation_id(&source.attempt_sha256),
        source_input, expected_revision:source.parent_revision, previous_state:source.parent_state,
        witness:ConditionWitness {
            predecessor:hex(&source.context.record_id), parent_authority:"Shadow".into(),
            failed_action:"construct law-confirmed native verdict".into(), condition_code:CONDITION.into(),
            reason:OUTPUT.into(), source_component:SITE.into(), source_sha256:source.attempt_sha256,
            attempt_trace:format!("native-request={} source-record={} source-lgc={}",hex(&source.attempt_sha256),hex(&source.context.record_id),hex(&source.parent_lgc_tip)),
            input_sha256,lgc_parent:second.step_digest,report_class:ReportClass::MissingConditions,
            permitted_next:NextAction::AcquireScopedEvidence,
        },
        checkpoint,scoped_question:question,candidate:Vec::new(),candidate_key:[0;32],lgc_steps:vec![first,second],
    })
}

/// Data construction only. Public possession of this result does not authorize
/// a live append; the external Shadow operational endpoint rejects PLF1.
fn current_frontier(attempt:&crate::ShadowCycleRequest,prepared:&crate::pd_prepared::PdPreparedEnvelope,
    proof:&crate::kie::KieProof)->Result<crate::iz_frontier::IzFrontier,String> {
    // The prepared witness chooses the explicit inquiry policy. Retained v1
    // attempts keep their original frontier bytes and rendered question.
    let current=prepared.failure_witness.as_ref().and_then(|w|w.inquiry.as_ref())
        .is_some_and(|frontier|frontier.uses_current_processing());
    if current {
        let mut frontier=crate::iz_frontier::IzFrontier::from_current_request(Some(CONDITION),attempt,Some(proof))?;
        if prepared.failure_witness.as_ref().and_then(|w|w.inquiry.as_ref())
            .is_some_and(|f|f.uses_historical_branch()) {
            frontier=frontier.with_historical_branch(proof)?;
        }
        match prepared.failure_witness.as_ref().and_then(|w|w.inquiry.as_ref())
            .and_then(|frontier|frontier.model_measurement()) {
            Some(value)=>frontier.with_model_measurement(proof,value.clone()),
            None=>Ok(frontier),
        }
    } else {
        crate::iz_frontier::IzFrontier::from_observation(Some(CONDITION),Some(proof))
    }
}
pub fn request_from_observation(parent:&PdOperationRecord, attempt:Vec<u8>, prepared:Vec<u8>, kie_proof:Vec<u8>)
    -> Result<PdOperationRequest,String> {
    let proof=crate::kie::decode_proof(&kie_proof)?;
    let wire=crate::decode_cycle_request(&attempt).map_err(|e|e.to_string())?;
    let envelope=crate::pd_prepared::decode_prepared(&prepared)?;
    let frontier=current_frontier(&wire,&envelope,&proof)?.encode()?;
    let source=LateRefusalSource {
        context:proof.context,parent_revision:parent.revision,parent_state:parent.state_digest,
        parent_source_sha256:sha(&parent.request.source_input),parent_lgc_tip:parent.request.witness.lgc_parent,
        original_source:parent.request.source_input.clone(),
        attempt_sha256:sha(&attempt),attempt,prepared,kie_proof,frontier,
    };
    let request=operation_data(&source,parent.request.checkpoint.clone())?;
    validate_transition(&request,parent)?;
    Ok(request)
}

pub fn validate_operation(request:&PdOperationRequest) -> Result<LateRefusalSource,String> {
    let source=decode_source(&request.source_input)?;
    // Every output, question, witness field and LGC step is fixed by this exact
    // event. Checkpoint equality is checked against the owner-held parent below.
    if operation_data(&source,request.checkpoint.clone())? != *request {
        return Err("PD_LATE_OPERATION_BINDING".into());
    }
    Ok(source)
}
pub fn validate_transition(request:&PdOperationRequest,parent:&PdOperationRecord) -> Result<(),String> {
    let source=validate_operation(request)?;
    parent.validate()?;
    if parent.record_id != source.context.record_id || parent.revision != source.parent_revision
        || parent.state_digest != source.parent_state || parent.request.identity != source.context.identity
        || parent.request.operation_id != source.context.operation_id
        || sha(&parent.request.source_input) != source.parent_source_sha256
        || parent.request.source_input != source.original_source
        || parent.request.witness.lgc_parent != source.parent_lgc_tip
        || request.checkpoint != parent.request.checkpoint {
        return Err("PD_LATE_PARENT_OR_CHECKPOINT_CHANGED".into());
    }
    linked_science::validate_science_record(parent)?;
    let proof=crate::kie::decode_proof(&source.kie_proof)?;
    if linked_science::science_query_from_source(&parent.request.source_input)? != proof.query_bytes
        || parent.request.candidate != proof.candidate_bytes {
        return Err("PD_LATE_ORIGINAL_SCIENCE_CHANGED".into());
    }
    crate::pd_prepared::decode_prepared(&source.prepared)?.validate_dialogue_source(parent)
}

/// A transport receiver binds a durable NoVerdict response to the exact frame
/// it sent. This is data integrity only; it does not create a native receipt.
pub fn validate_response(record:&PdOperationRecord,attempt:&crate::ShadowCycleRequest,prepared:&crate::pd_prepared::PdPreparedEnvelope) -> Result<(),String> {
    record.validate()?;
    let source=validate_operation(&record.request)?;
    if source.attempt != crate::encode_cycle_request(attempt) || source.prepared != crate::pd_prepared::encode_prepared(prepared)? {
        return Err("PD_LATE_RESPONSE_ATTEMPT_MISMATCH".into());
    }
    Ok(())
}

/// Form the final current output plus iZ only from an owner-read durable record.
/// Storage/replay retains the target descriptor and never calls this renderer.
/// The caller must resolve the exact record from Shadow before presenting it.
pub fn render_iz(record:&PdOperationRecord) -> Result<String,String> {
    record.validate()?;
    let source=validate_operation(&record.request)?;
    let frontier=crate::iz_frontier::IzFrontier::decode(&source.frontier)?;
    let rendered=format!("{}\n{}",OUTPUT,frontier.render()?);
    if rendered.len()>crate::iz_frontier::MAX_RENDER_BYTES { return Err("PD_LATE_RENDER_LIMIT".into()); }
    Ok(rendered)
}
