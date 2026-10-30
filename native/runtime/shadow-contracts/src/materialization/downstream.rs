// Source: ess-mai-dbab-zero-noi/src/downstream.rs (Bledar Gjata / Gjata Legacy,
// Apache-2.0), integrated into AIGovern-Cyber.
// Provenance and adaptations: ../../../SOURCE_NOTES.md.
//! NEW integration contracts, not the inherited v189 BESA/ASHT/NBES implementation.
//! Gates preserve an immutable obligation and require every negative constraint.
//! Fulfilment evidence is prepared for independent Shadow review, never a final verdict.
use super::common::*;
use super::dbab::{BindingOutcome, BoundObligation, DbabInput, GapEvidence, Requirement};
use super::zero_noi::{LightAcknowledgement, LightTriggerConsumer, ZeroNoiTrigger};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DownstreamError {
    BindingFailed,
    ObligationInvalidated,
    Replay,
    CapacityExhausted,
    MissingRequirement(String),
    UnknownRequirement(String),
    DuplicateRequirement(String),
    InvalidStatement,
    Evidence(EvidenceError),
}
impl std::fmt::Display for DownstreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f,"{self:?}") }
}
impl std::error::Error for DownstreamError {}

type CycleKey = (String, String, u64);
fn cycle_key(case: &CaseIdentity) -> CycleKey { (case.cycle_id.clone(),case.scope.clone(),case.revision) }

/// Host-owned intake state, explicitly NOT 0-NOI persistence or an authority issuer.
/// Replay protection is bounded and process-local; durable replay protection belongs to the host.
pub struct LightBoundary {
    received: BTreeSet<CycleKey>,
    capacity: usize,
}
impl LightBoundary {
    pub fn new(capacity: usize) -> Result<Self,DownstreamError> {
        if capacity == 0 || capacity > MAX_RECORDS { return Err(DownstreamError::CapacityExhausted); }
        Ok(Self { received:BTreeSet::new(),capacity })
    }
    pub fn accepted_cycles(&self) -> usize { self.received.len() }
}
impl LightTriggerConsumer for LightBoundary {
    type Error = DownstreamError;
    fn receive_trigger(&mut self, trigger:&ZeroNoiTrigger) -> Result<LightAcknowledgement,Self::Error> {
        let key=cycle_key(trigger.case());
        if self.received.contains(&key) { return Err(DownstreamError::Replay); }
        if self.received.len() >= self.capacity { return Err(DownstreamError::CapacityExhausted); }
        self.received.insert(key);
        Ok(LightAcknowledgement { case:trigger.case().clone(),execution_provenance:trigger.execution_provenance() })
    }
}

/// A new promise gate. It cannot consume a Gap or a caller-deserialized synthetic binding.
pub struct BesaGateway { armed: BTreeSet<CycleKey>, capacity:usize }
impl BesaGateway {
    pub fn new(capacity:usize) -> Result<Self,DownstreamError> {
        if capacity == 0 || capacity > MAX_RECORDS { return Err(DownstreamError::CapacityExhausted); }
        Ok(Self { armed:BTreeSet::new(),capacity })
    }
    pub fn armed_cycles(&self)->usize { self.armed.len() }
    pub fn arm(&mut self, outcome:BindingOutcome,current:&DbabInput,
        verifier:&dyn EvidenceVerifier,now:u64) -> Result<BesaPromise,DownstreamError>
    {
        let obligation=match outcome { BindingOutcome::Bound(value)=>value, BindingOutcome::Gap(_)=>return Err(DownstreamError::BindingFailed) };
        obligation.validate_for(current,verifier,now).map_err(|_|DownstreamError::ObligationInvalidated)?;
        let key=cycle_key(obligation.case());
        if self.armed.contains(&key) { return Err(DownstreamError::Replay); }
        if self.armed.len() >= self.capacity { return Err(DownstreamError::CapacityExhausted); }
        self.armed.insert(key);
        Ok(BesaPromise { obligation })
    }
}

/// Non-Clone, non-Deserialize, no public constructor; moved to the reasoning boundary once.
pub struct BesaPromise { obligation:BoundObligation }
#[derive(Debug,Clone,Serialize)]
pub struct PromiseReceipt {
    pub case:CaseIdentity, pub binding_digest:Digest32, pub context_digest:Digest32,
    pub required_constraints:usize, pub downstream_armed:bool,
    pub final_verdict_created:bool,
}
impl BesaPromise {
    pub fn receipt(&self)->PromiseReceipt {
        PromiseReceipt { case:self.obligation.case().clone(), binding_digest:self.obligation.binding_digest(),
            context_digest:self.obligation.context_digest(), required_constraints:self.obligation.requirements().len(),
            downstream_armed:true, final_verdict_created:false }
    }
    pub fn asht_handoff(self,current:&DbabInput,verifier:&dyn EvidenceVerifier,now:u64)
        ->Result<ReasoningJob,DownstreamError>
    {
        self.obligation.validate_for(current,verifier,now).map_err(|_|DownstreamError::ObligationInvalidated)?;
        Ok(ReasoningJob { obligation:self.obligation })
    }
}

/// Immutable requirements passed TO reasoning; not a replacement reasoning algorithm.
pub struct ReasoningJob { obligation:BoundObligation }
#[derive(Debug,Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FulfilmentEvidence { pub requirement_id:String, pub statement:String, pub evidence:Vec<EvidenceRef> }

pub fn fulfilment_claim(case:&CaseIdentity,binding_digest:Digest32,claim:&FulfilmentEvidence)->Result<Digest32,EvidenceError> {
    digest("ESS-MAI/fulfilment-claim/v1",&(case,binding_digest,&claim.requirement_id,&claim.statement))
}

impl ReasoningJob {
    pub fn requirements(&self)->&[Requirement] { self.obligation.requirements() }
    pub fn binding_digest(&self)->Digest32 { self.obligation.binding_digest() }
    pub fn case(&self)->&CaseIdentity { self.obligation.case() }

    /// Checks completeness of submitted evidence, not epistemic fulfilment truth.
    /// Each critical negative has a mandatory requirement; no informational-only escape.
    pub fn prepare_shadow_review(self,current:&DbabInput,claims:Vec<FulfilmentEvidence>,
        verifier:&dyn EvidenceVerifier,now:u64)->Result<ShadowReviewRequest,DownstreamError>
    {
        self.obligation.validate_for(current,verifier,now).map_err(|_|DownstreamError::ObligationInvalidated)?;
        if claims.len() > MAX_ITEMS * 5 { return Err(DownstreamError::CapacityExhausted); }
        let required:BTreeSet<&str>=self.obligation.requirements().iter().map(|r|r.id.as_str()).collect();
        let mut provided=BTreeSet::new();
        for claim in &claims {
            if !valid_text(&claim.statement) { return Err(DownstreamError::InvalidStatement); }
            if !required.contains(claim.requirement_id.as_str()) { return Err(DownstreamError::UnknownRequirement(claim.requirement_id.clone())); }
            if !provided.insert(claim.requirement_id.as_str()) { return Err(DownstreamError::DuplicateRequirement(claim.requirement_id.clone())); }
            let d=fulfilment_claim(self.obligation.case(),self.obligation.binding_digest(),claim).map_err(DownstreamError::Evidence)?;
            verify_references(verifier,&claim.evidence,self.obligation.case(),EvidenceRole::Fulfilment,d,now).map_err(DownstreamError::Evidence)?;
        }
        for missing in required.difference(&provided) { return Err(DownstreamError::MissingRequirement((*missing).to_owned())); }
        Ok(ShadowReviewRequest { case:self.obligation.case().clone(),binding_digest:self.obligation.binding_digest(),
            context_digest:self.obligation.context_digest(),requirements:self.obligation.requirements().to_vec(),claims,
            status:ShadowReviewStatus::AwaitingIndependentVerification,final_verdict_created:false })
    }
}

#[derive(Debug,Serialize)]
#[serde(rename_all="snake_case")]
pub enum ShadowReviewStatus { AwaitingIndependentVerification }
#[derive(Debug,Serialize)]
pub struct ShadowReviewRequest {
    case:CaseIdentity,
    binding_digest:Digest32,
    context_digest:Digest32,
    requirements:Vec<Requirement>,
    claims:Vec<FulfilmentEvidence>,
    status:ShadowReviewStatus,
    final_verdict_created:bool,
}

/// Failure remains a continuation frontier, never a synthetic success or final iZ verdict.
#[derive(Debug,Serialize)]
pub struct GapContinuation {
    pub current_iz:GapEvidence,
    pub next_i0_scope:String,
    pub next_i0_exact_content_predicted:bool,
    pub final_verdict_created:bool,
}
pub fn gap_to_continuation(gap:GapEvidence)->GapContinuation {
    GapContinuation { next_i0_scope:gap.case.scope.clone(),current_iz:gap,
        next_i0_exact_content_predicted:false,final_verdict_created:false }
}
