//! R25: a data-only mathematical frontier for the next evidence question.
//!
//! M = R \ O for a required condition explicitly reported missing by the
//! current request; C = current KiC union owner-classified exact counterproofs;
//! U = unresolved provenance, equality and undeclared/unknown causes.
//! Q = M union C union U. These are typed sets, not competing scalar scores:
//! a counterproof must never erase an independently observed process deficit.
//!
//! Source anchors: Quantum's collapse_reasoning reports primitive, one-sided,
//! co-presence and operator-evidence deficits. KiE relation 0/1/2 denotes the
//! provider's strict support/counter/equality predicates. KIP3 history classes
//! distinguish ProcessFailure, ExactModelCounterproof and LegacyUnscoped.
//! The set union and this codec are NEW derivative policy, not a v189 formula.
//! D-BAB reports these observations; it does not judge or create a veto.
//! The owner must still recompute the actual model and resolve actual Vault
//! provenance. A hash or a successful data decoder supplies no authority.

use crate::{kie, Reader, Writer};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

type Hash = [u8; 32];
const MAGIC: &[u8; 8] = b"ESSIZF01";
pub const MAX_FRONTIER_BYTES: usize = 48 * 1024;
pub const MAX_RENDER_BYTES: usize = 64 * 1024;
const MAX_COUNTERS: usize = 128; // complete enumeration: at most 127 lexical N + one scoped N

fn sha(bytes: &[u8]) -> Hash { Sha256::digest(bytes).into() }
fn hex(hash: &Hash) -> String { crate::pd_memory::hex(hash) }

// Codes name observations. In particular, none of these codes is a verdict.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequestKind {
    NoDeclaredCause = 0,
    PrimitiveMaterial = 1,
    MissingSide = 2,
    CoPresence = 3,
    OperatorCausalEvidence = 4,
    HistoricalCounterRequired = 5,
    ModelCounterReported = 6,
    ScopedCounterReported = 7,
    ProvenanceUnresolved = 8,
    EqualityUnresolved = 9,
    UnknownCause = 10,
    ShadowMissingNegativeProof = 11,
}

impl RequestKind {
    fn decode(value: u8) -> Result<Self, String> {
        match value {
            0 => Ok(Self::NoDeclaredCause), 1 => Ok(Self::PrimitiveMaterial),
            2 => Ok(Self::MissingSide), 3 => Ok(Self::CoPresence),
            4 => Ok(Self::OperatorCausalEvidence), 5 => Ok(Self::HistoricalCounterRequired),
            6 => Ok(Self::ModelCounterReported), 7 => Ok(Self::ScopedCounterReported),
            8 => Ok(Self::ProvenanceUnresolved), 9 => Ok(Self::EqualityUnresolved),
            10 => Ok(Self::UnknownCause), 11 => Ok(Self::ShadowMissingNegativeProof),
            _ => Err("IZ_FRONTIER_REQUEST_KIND".into()),
        }
    }
    fn missing(self) -> bool {
        matches!(self, Self::PrimitiveMaterial | Self::MissingSide | Self::CoPresence
            | Self::OperatorCausalEvidence | Self::HistoricalCounterRequired
            | Self::ShadowMissingNegativeProof)
    }
    fn label(self) -> &'static str {
        match self {
            Self::NoDeclaredCause => "NO_DECLARED_CAUSE",
            Self::PrimitiveMaterial => "PRIMITIVE_MATERIAL_MISSING",
            Self::MissingSide => "PRIMITIVE_SIDE_MISSING",
            Self::CoPresence => "XI_YI_COPRESENCE_MISSING",
            Self::OperatorCausalEvidence => "OPERATOR_EVIDENCE_UNRESOLVED",
            Self::HistoricalCounterRequired => "HISTORICAL_COUNTER_REQUIRED",
            Self::ModelCounterReported => "KIE_MODEL_COUNTERPROOF",
            Self::ScopedCounterReported => "KIE_SCOPED_HISTORY_VETO",
            Self::ProvenanceUnresolved => "KIE_HISTORY_ORIGIN_UNRESOLVED",
            Self::EqualityUnresolved => "KIE_EQUALITY_UNRESOLVED",
            Self::UnknownCause => "UNSTRUCTURED_CAUSE",
            Self::ShadowMissingNegativeProof => "SHADOW_MISSING_NEGATIVE_PROOF",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RequestObservation {
    kind: RequestKind,
    refusal_sha256: Option<Hash>,
}

impl RequestObservation {
    fn derive(refusal: Option<&str>) -> Result<Self, String> {
        let Some(reason) = refusal else {
            return Ok(Self { kind: RequestKind::NoDeclaredCause, refusal_sha256: None });
        };
        if reason.trim().is_empty() || reason.len() > 8192 || reason.contains('\0') {
            return Err("IZ_FRONTIER_REFUSAL_LIMIT".into());
        }
        let kind = if reason == "SHADOW_MISSING_NEGATIVE_PROOF" {
            RequestKind::ShadowMissingNegativeProof
        } else if reason.starts_with("OPERATOR_EVIDENCE_UNRESOLVED:") {
            RequestKind::OperatorCausalEvidence
        } else if reason.starts_with("hapësirë e njëanshme:") || reason.starts_with("vetem nje ane primitive:") {
            RequestKind::MissingSide
        } else if reason.starts_with("XiYi=∅: asnjë bashkëprezencë") {
            RequestKind::CoPresence
        } else if reason.starts_with("pa material primitiv:") {
            RequestKind::PrimitiveMaterial
        } else if reason.starts_with("negative parity gate R2: refuting evidence absent;") {
            RequestKind::HistoricalCounterRequired
        } else if reason.starts_with("KIE_MODEL_COUNTERPROOF:") {
            RequestKind::ModelCounterReported
        } else if reason.starts_with("KIE_SCOPED_HISTORY_VETO:") {
            RequestKind::ScopedCounterReported
        } else if reason.starts_with("KIE_HISTORY_ORIGIN_UNRESOLVED:") {
            RequestKind::ProvenanceUnresolved
        } else if reason.starts_with("KiE unresolved: equality is not refuting evidence;") {
            RequestKind::EqualityUnresolved
        } else {
            RequestKind::UnknownCause
        };
        Ok(Self { kind, refusal_sha256: Some(sha(reason.as_bytes())) })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CounterReference {
    candidate_id: u64,
    source_sha256: Hash,
    transaction_id: Hash,
    proof_sha256: Hash,
    model_family_sha256: Hash,
}
impl CounterReference {
    fn key(&self) -> (Hash, u64, Hash, Hash, Hash) {
        (self.source_sha256, self.candidate_id, self.transaction_id,
            self.proof_sha256, self.model_family_sha256)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ModelObservation {
    scope_sha256: Hash,
    proof_sha256: Hash,
    query_sha256: Hash,
    candidate_sha256: Hash,
    proposition: String,
    relation: u8,
    history_assessment_sha256: Option<Hash>,
    historical_positive_count: u32,
    historical_negative_count: u32,
    unresolved_history_count: u32,
    counters: Vec<CounterReference>,
}

fn parse_hash(value: &str) -> Result<Hash, String> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return Err("IZ_FRONTIER_SOURCE_HASH".into());
    }
    let mut out = [0; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[i * 2..i * 2 + 2], 16).map_err(|_| "IZ_FRONTIER_SOURCE_HASH")?;
    }
    if out == [0; 32] { return Err("IZ_FRONTIER_SOURCE_HASH_ZERO".into()); }
    Ok(out)
}

fn collect_counters(proof: &kie::KieProof) -> Result<Vec<CounterReference>, String> {
    // Exact repeated references are deduplicated, while distinct provenance
    // for the same source is retained. The question set is deduplicated by
    // source hash separately: aliases never multiply scientific evidence.
    let mut unique = BTreeSet::new();
    let mut include = |candidate_id, source: &str, p: &kie::KieHistoryProvenance| -> Result<(), String> {
        let reference = CounterReference { candidate_id, source_sha256: parse_hash(source)?,
            transaction_id: p.transaction_id, proof_sha256: p.proof_sha256,
            model_family_sha256: p.model_family_sha256 };
        unique.insert(reference.key());
        Ok(())
    };
    if let Some(history) = &proof.history_assessment {
        for entry in &history.entries {
            if entry.class == kie::KieHistoryClass::ExactModelCounterproof {
                include(entry.candidate_id, &entry.source_sha256,
                    entry.provenance.as_ref().ok_or("IZ_FRONTIER_COUNTER_PROVENANCE")?)?;
            }
        }
        for entry in &history.scoped_evidence {
            if entry.class == kie::KieHistoryClass::ExactModelCounterproof {
                include(entry.candidate.candidate_id, &entry.candidate.source_sha256, &entry.provenance)?;
            }
        }
    }
    Ok(unique.into_iter().map(|(source_sha256,candidate_id,transaction_id,proof_sha256,model_family_sha256)|
        CounterReference { source_sha256,candidate_id,transaction_id,proof_sha256,model_family_sha256 }).collect())
}

impl ModelObservation {
    fn derive(proof: &kie::KieProof) -> Result<Self, String> {
        proof.validate()?;
        let combined = kie::combined_evaluation(proof)?;
        let count = |n| u32::try_from(n).map_err(|_| "IZ_FRONTIER_COUNT".to_string());
        Ok(Self {
            scope_sha256: proof.context.identity.key()?, proof_sha256: proof.proof_sha256,
            query_sha256: sha(&proof.query_bytes), candidate_sha256: sha(&proof.candidate_bytes),
            proposition: proof.canonical_proposition.clone(), relation: proof.relation,
            history_assessment_sha256: proof.history_assessment_digest()?,
            historical_positive_count: count(combined.historical_positive_count)?,
            historical_negative_count: count(combined.historical_negative_count)?,
            unresolved_history_count: count(combined.unresolved_history_count)?,
            counters: collect_counters(proof)?,
        })
    }
}

/// A projection of the measured CURRENT request, never an epistemic verdict.
/// n+ = 0 is absence of a measured candidate; sum+ = 0 with n+ > 0 is not.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ProcessingObservation {
    request_sha256: Hash,
    input_sha256: Hash,
    ledger_sha256: Hash,
    positive_count: u32,
    positive_sum: u32,
    negative_count: u32,
    negative_sum: u32,
    family_mask: u8,
    original: RequestObservation,
    // Only explicit frontier V3 carries this current numerical method. The
    // original lexical observations above are retained without unit mixing.
    model_operators: Option<crate::model_operator_measurement::ModelOperatorMeasurements>,
    model_hcp_words: Option<[u64; 11]>,
}
impl ProcessingObservation {
    fn derive(request: &crate::ShadowCycleRequest) -> Result<Option<Self>, String> {
        let evidence=&request.quantum.final_evidence;
        let Some(action)=evidence.quantum_action_evidence.iter().find(|a|a.stage==crate::ACTION_STAGE_MPRO)
            else { return Ok(None); };
        if !crate::candidate_measurement::CandidateLedger::is_current_words(&action.evidence_words) {
            return Ok(None);
        }
        let (ledger, summary)=evidence.candidate_measurement()?.ok_or("IZ_PROCESS_LEDGER_MISSING")?;
        let hcp=evidence.quantum_action_evidence.iter().find(|a|a.stage==crate::ACTION_STAGE_HCP)
            .ok_or("IZ_PROCESS_HCP_MISSING")?;
        let family_mask=crate::hcp_family_admission_mask(&hcp.evidence_words)?
            .ok_or("IZ_PROCESS_FAMILY_MASK_MISSING")?;
        let model_operators=ledger.model_measurements().cloned();
        let model_hcp_words=match &model_operators {
            None=>None,
            Some(_)=>Some(hcp.evidence_words.as_slice().try_into()
                .map_err(|_|"IZ_PROCESS_MODEL_HCP_FORMAT")?),
        };
        let mut material=b"OPL2S/IZ/CURRENT-CANDIDATE-LEDGER/V1".to_vec();
        for word in ledger.to_words() { material.extend_from_slice(&word.to_le_bytes()); }
        let value=Self {
            request_sha256:sha(&crate::encode_cycle_request(request)),input_sha256:ledger.input_sha256(),
            ledger_sha256:sha(&material),positive_count:summary.positive_observed.count,
            positive_sum:summary.positive_observed.sum,negative_count:summary.negative_observed.count,
            negative_sum:summary.negative_observed.sum,family_mask,
            original:RequestObservation::derive(request.quantum.collapse_refused.as_deref())?,
            model_operators,model_hcp_words,
        };
        value.validate()?;
        Ok(Some(value))
    }
    fn validate(&self) -> Result<(), String> {
        if [self.request_sha256,self.input_sha256,self.ledger_sha256].contains(&[0;32])
            || self.family_mask & !15 != 0
            || self.positive_count.saturating_add(self.negative_count)>128
            || self.positive_sum>self.positive_count.saturating_mul(10_000)
            || self.negative_sum>self.negative_count.saturating_mul(10_000)
            || (self.original.kind==RequestKind::NoDeclaredCause)!=self.original.refusal_sha256.is_none()
            || self.original.refusal_sha256==Some([0;32]) {
            return Err("IZ_PROCESS_OBSERVATION_SHAPE".into());
        }
        match (&self.model_operators,&self.model_hcp_words) {
            (None,None)=>{},
            (Some(model),Some(words))=>{
                model.summarize()?;
                if crate::hcp_family_admission_mask(words)?!=Some(self.family_mask) {
                    return Err("IZ_PROCESS_MODEL_HCP_FAMILY".into());
                }
            }
            _=>return Err("IZ_PROCESS_MODEL_POLICY_SHAPE".into()),
        }
        Ok(())
    }
}


/// A selection observation, not an authority token or a thesis verdict.
/// Reuse the original request-bound ASHT method; semantic KiE masses cannot
/// replace the actual selection result. Corrupt/foreign data is an error.
#[derive(Debug, Clone, PartialEq, Eq)]
struct HistoricalBranchObservation {
    request_sha256: Hash,
    response_sha256: Hash,
    // 0: pair present; 1/2: the exact first failed selection obligation.
    missing: u8,
}
impl HistoricalBranchObservation {
    fn derive(proof: &kie::KieProof) -> Result<Self, String> {
        use crate::pd_memory::failure::{self, MissingSelection};
        proof.validate()?;
        let (request, response) = failure::decode_selection_pair(
            &proof.historical_request, &proof.historical_response)?;
        let missing = match failure::relevance::project_request_bound(&request, &response) {
            Ok(_) => 0,
            Err(reason) => match failure::validate_missing_selection(
                &proof.historical_request, &proof.historical_response, &reason)? {
                MissingSelection::Positive => 1,
                MissingSelection::Negative => 2,
            },
        };
        Ok(Self { request_sha256: sha(&proof.historical_request),
            response_sha256: sha(&proof.historical_response), missing })
    }
    fn validate(&self) -> Result<(), String> {
        if self.missing > 2 || [self.request_sha256, self.response_sha256].contains(&[0;32]) {
            return Err("IZ_HISTORY_BRANCH_SHAPE".into());
        }
        Ok(())
    }
    fn label(&self) -> &'static str {
        match self.missing { 1 => "MISSING_EXACT_POSITIVE", 2 => "MISSING_EXACT_NEGATIVE",
            _ => "PAIR_PRESENT" }
    }
}

/// Canonical data projection. Constructing this value grants no authority;
/// request and proof validation must both run against their actual owners.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IzFrontier {
    request: RequestObservation,
    model: Option<ModelObservation>,
    // None selects immutable frontier v1. Current processing selects v2;
    // a typed numerical operator projection explicitly selects v3.
    processing: Option<ProcessingObservation>,
    model_measurement: Option<crate::kie_measurement::KieMeasurement>,
    // Explicit V4 only; older records keep their original bytes and questions.
    historical_branch: Option<HistoricalBranchObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Target {
    Missing(u8),
    MissingProMeasurement,
    MissingProAdmission,
    MissingHistoricalPositive,
    MissingHistoricalNegative,
    MissingModelProPermission,
    MissingModelNproPermission,
    CurrentCounter,
    HistoricalCounter(Hash),
    Uncertainty(u8),
}

impl IzFrontier {
    pub fn from_observation(refusal: Option<&str>, proof: Option<&kie::KieProof>) -> Result<Self, String> {
        let value = Self { request: RequestObservation::derive(refusal)?,
            model: proof.map(ModelObservation::derive).transpose()?, processing:None,model_measurement:None,historical_branch:None };
        value.validate()?;
        Ok(value)
    }

    /// Current observation data is bound to the exact request. Neither a count,
    /// digest nor a missing-measurement question authorizes a positive verdict.
    pub fn from_current_request(refusal: Option<&str>, request:&crate::ShadowCycleRequest,
        proof:Option<&kie::KieProof>) -> Result<Self,String> {
        let mut value=Self::from_observation(refusal,proof)?;
        value.processing=ProcessingObservation::derive(request)?;
        value.validate()?;
        Ok(value)
    }
    pub fn with_historical_branch(mut self, proof: &kie::KieProof) -> Result<Self,String> {
        if self.processing.is_none() { return Err("IZ_HISTORY_REQUIRES_CURRENT_REQUEST".into()); }
        self.historical_branch = Some(HistoricalBranchObservation::derive(proof)?);
        self.validate_model(Some(proof))?;
        Ok(self)
    }
    pub fn uses_historical_branch(&self) -> bool { self.historical_branch.is_some() }
    pub fn uses_current_processing(&self) -> bool { self.processing.is_some() }
    pub fn uses_model_policy(&self) -> bool { self.model_operators().is_some() }
    pub fn model_operators(&self)->Option<&crate::model_operator_measurement::ModelOperatorMeasurements> {
        self.processing.as_ref().and_then(|p|p.model_operators.as_ref())
    }
    pub fn model_hcp_words(&self)->Option<&[u64;11]> {
        self.processing.as_ref().and_then(|p|p.model_hcp_words.as_ref())
    }
    pub fn model_measurement(&self)->Option<&crate::kie_measurement::KieMeasurement> {
        self.model_measurement.as_ref()
    }
    pub fn with_model_measurement(mut self,proof:&kie::KieProof,
        value:crate::kie_measurement::KieMeasurement)->Result<Self,String> {
        if self.processing.is_none() { return Err("IZ_MEASUREMENT_REQUIRES_CURRENT_REQUEST".into()); }
        value.validate_model(proof)?;
        self.model_measurement=Some(value);
        self.validate_model(Some(proof))?;
        Ok(self)
    }
    pub fn validate_request_hash(&self,request_sha256:&Hash)->Result<(),String> {
        self.validate()?;
        if self.processing.as_ref().is_some_and(|p|&p.request_sha256!=request_sha256) {
            return Err("IZ_PROCESS_REQUEST_HASH".into());
        }
        Ok(())
    }
    pub fn validate_current_request(&self,request:&crate::ShadowCycleRequest)->Result<(),String> {
        self.validate()?;
        if self.processing.is_some() && self.processing!=ProcessingObservation::derive(request)? {
            return Err("IZ_PROCESS_ACTUAL_REQUEST".into());
        }
        Ok(())
    }

    pub fn validate_refusal(&self, refusal: Option<&str>) -> Result<(), String> {
        self.validate()?;
        if self.request != RequestObservation::derive(refusal)? {
            return Err("IZ_FRONTIER_ACTUAL_REFUSAL".into());
        }
        Ok(())
    }

    pub fn validate_model(&self, proof: Option<&kie::KieProof>) -> Result<(), String> {
        self.validate()?;
        if self.model != proof.map(ModelObservation::derive).transpose()? {
            return Err("IZ_FRONTIER_ACTUAL_MODEL".into());
        }
        if let Some(branch)=&self.historical_branch {
            if branch != &HistoricalBranchObservation::derive(proof.ok_or("IZ_HISTORY_MODEL_MISSING")?)? {
                return Err("IZ_HISTORY_ACTUAL_SELECTION".into());
            }
        }
        if let Some(value)=&self.model_measurement {
            value.validate_model(proof.ok_or("IZ_MEASUREMENT_MODEL_MISSING")?)?;
        }
        if let Some(value)=self.model_operators() {
            let proof=proof.ok_or("IZ_MODEL_OPERATOR_PROOF_MISSING")?;
            value.validate_model(proof)?;
            let permitted=crate::model_operator_measurement::resource_permitted(
                proof,self.model_hcp_words().ok_or("IZ_PROCESS_MODEL_HCP_FORMAT")?)?;
            if value.pro_permitted()!=permitted || value.npro_permitted()!=permitted {
                return Err("IZ_MODEL_OPERATOR_RESOURCE_MISMATCH".into());
            }
            if self.processing.as_ref().map(|p|p.input_sha256)!=Some(sha(proof.canonical_proposition.as_bytes())) {
                return Err("IZ_MODEL_OPERATOR_INPUT_MISMATCH".into());
            }
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), String> {
        if (self.request.kind == RequestKind::NoDeclaredCause) != self.request.refusal_sha256.is_none()
            || self.request.refusal_sha256 == Some([0; 32]) {
            return Err("IZ_FRONTIER_REQUEST_SHAPE".into());
        }
        if let Some(branch)=&self.historical_branch {
            branch.validate()?;
            if self.processing.is_none() || self.model.is_none() {
                return Err("IZ_HISTORY_CONTEXT_MISSING".into());
            }
        }
        if let Some(processing)=&self.processing { processing.validate()?; }
        if let Some(operators)=self.model_operators() {
            if self.model.is_none() {return Err("IZ_MODEL_OPERATOR_CONTEXT_MISSING".into());}
            if self.model_measurement.as_ref().is_some_and(|m|m!=operators.measurement()) {
                return Err("IZ_MODEL_OPERATOR_MEASUREMENT_MISMATCH".into());
            }
        }
        if let Some(value)=&self.model_measurement {
            if self.processing.is_none() || self.model.is_none() {return Err("IZ_MEASUREMENT_CONTEXT_MISSING".into());}
            value.validate()?;
        }
        if let Some(model) = &self.model {
            if model.proposition.is_empty() || model.proposition.len() > kie::MAX_TEXT_BYTES
                || model.relation > 2 || [model.scope_sha256, model.proof_sha256,
                    model.query_sha256, model.candidate_sha256].contains(&[0; 32])
                || model.history_assessment_sha256 == Some([0; 32])
                || model.counters.len() > MAX_COUNTERS
                || model.counters.windows(2).any(|p| p[0].key() >= p[1].key())
                || model.historical_positive_count > 128 || model.historical_negative_count > 128
                || model.unresolved_history_count > 128
                || (model.history_assessment_sha256.is_none() && !model.counters.is_empty()) {
                return Err("IZ_FRONTIER_MODEL_SHAPE".into());
            }
            for counter in &model.counters {
                if counter.candidate_id == 0 || [counter.source_sha256, counter.transaction_id, counter.proof_sha256,
                    counter.model_family_sha256].contains(&[0; 32]) {
                    return Err("IZ_FRONTIER_COUNTER_SHAPE".into());
                }
            }
        }
        Ok(())
    }

    fn targets(&self) -> BTreeSet<Target> {
        let mut targets = BTreeSet::new();
        if self.request.kind.missing() { targets.insert(Target::Missing(self.request.kind as u8)); }
        match self.request.kind {
            RequestKind::NoDeclaredCause => { targets.insert(Target::Uncertainty(1)); }
            RequestKind::UnknownCause => { targets.insert(Target::Uncertainty(2)); }
            RequestKind::ProvenanceUnresolved => { targets.insert(Target::Uncertainty(3)); }
            RequestKind::EqualityUnresolved => { targets.insert(Target::Uncertainty(4)); }
            _ => {},
        }
        if let Some(branch)=&self.historical_branch {
            match branch.missing {
                1 => { targets.insert(Target::MissingHistoricalPositive); },
                2 => { targets.insert(Target::MissingHistoricalNegative); },
                _ => {},
            }
        }
        if let Some(process)=&self.processing {
            if process.positive_count==0 { targets.insert(Target::MissingProMeasurement); }
            else if process.family_mask & 1==0 { targets.insert(Target::MissingProAdmission); }
            // A late owner observation must not erase an earlier explicit
            // condition from this same attempt. An absent earlier cause is
            // not invented as a second failure.
            if process.original.kind.missing() {
                targets.insert(Target::Missing(process.original.kind as u8));
            }
            let unknown=match process.original.kind {
                RequestKind::UnknownCause=>Some(2),RequestKind::ProvenanceUnresolved=>Some(3),
                RequestKind::EqualityUnresolved=>Some(4),_=>None,
            };
            if let Some(kind)=unknown { targets.insert(Target::Uncertainty(kind)); }
            if let Some(model)=&process.model_operators {
                if !model.pro_permitted() {targets.insert(Target::MissingModelProPermission);}
                if !model.npro_permitted() {targets.insert(Target::MissingModelNproPermission);}
            }
        }
        if let Some(model) = &self.model {
            if model.relation == 1 { targets.insert(Target::CurrentCounter); }
            if model.relation == 2 { targets.insert(Target::Uncertainty(4)); }
            if model.unresolved_history_count > 0 { targets.insert(Target::Uncertainty(3)); }
            if model.history_assessment_sha256.is_none() && model.historical_negative_count > 0 {
                targets.insert(Target::Uncertainty(5));
            }
            for counter in &model.counters { targets.insert(Target::HistoricalCounter(counter.source_sha256)); }
        }
        if (self.request.kind == RequestKind::ModelCounterReported
                && !self.model.as_ref().is_some_and(|m| m.relation == 1))
            || (self.request.kind == RequestKind::ScopedCounterReported
                && !self.model.as_ref().is_some_and(|m| !m.counters.is_empty())) {
            targets.insert(Target::Uncertainty(6));
        }
        targets
    }

    pub fn encode(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut w = Writer::new();
        w.bytes.extend_from_slice(MAGIC); w.u16(if self.uses_historical_branch() {4} else if self.uses_model_policy() {3} else if self.processing.is_some() {2} else {1}); w.u8(self.request.kind as u8);
        write_optional_hash(&mut w, self.request.refusal_sha256);
        match &self.model {
            None => w.u8(0), Some(m) => {
                w.u8(1);
                for hash in [m.scope_sha256, m.proof_sha256, m.query_sha256, m.candidate_sha256] {
                    w.bytes.extend_from_slice(&hash);
                }
                w.string(&m.proposition); w.u8(m.relation);
                write_optional_hash(&mut w, m.history_assessment_sha256);
                w.u32(m.historical_positive_count); w.u32(m.historical_negative_count);
                w.u32(m.unresolved_history_count); w.u16(m.counters.len() as u16);
                for c in &m.counters {
                    w.u64(c.candidate_id);
                    for hash in [c.source_sha256, c.transaction_id, c.proof_sha256, c.model_family_sha256] {
                        w.bytes.extend_from_slice(&hash);
                    }
                }
            }
        }
        if let Some(p)=&self.processing {
            for hash in [p.request_sha256,p.input_sha256,p.ledger_sha256] { w.bytes.extend_from_slice(&hash); }
            w.u32(p.positive_count);w.u32(p.positive_sum);w.u32(p.negative_count);w.u32(p.negative_sum);
            w.u8(p.family_mask);w.u8(p.original.kind as u8);write_optional_hash(&mut w,p.original.refusal_sha256);
            match &self.model_measurement {
                None=>w.u8(0),Some(value)=>{w.u8(1);w.bytes.extend_from_slice(&value.encode()?);}
            }
            if self.uses_historical_branch() { w.u8(u8::from(p.model_operators.is_some())); }
            if let Some(operators)=&p.model_operators {
                w.bytes.extend_from_slice(&operators.encode()?);
                for word in p.model_hcp_words.as_ref().ok_or("IZ_PROCESS_MODEL_HCP_FORMAT")? {w.u64(*word);}
            }
        }
        if let Some(branch)=&self.historical_branch {
            w.bytes.extend_from_slice(&branch.request_sha256);
            w.bytes.extend_from_slice(&branch.response_sha256);
            w.u8(branch.missing);
        }
        if w.bytes.len() > MAX_FRONTIER_BYTES { return Err("IZ_FRONTIER_LIMIT".into()); }
        Ok(w.bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_FRONTIER_BYTES { return Err("IZ_FRONTIER_LIMIT".into()); }
        let parsed = (|| -> Result<Self, String> {
            let mut r = Reader::new(bytes);
            let error = |e| format!("IZ_FRONTIER_CODEC:{e:?}");
            if r.take(8).map_err(error)? != MAGIC { return Err("IZ_FRONTIER_VERSION".into()); }
            let version=r.u16().map_err(error)?;
            if !matches!(version,1|2|3|4) { return Err("IZ_FRONTIER_VERSION".into()); }
            let request = RequestObservation { kind: RequestKind::decode(r.u8().map_err(error)?)?,
                refusal_sha256: read_optional_hash(&mut r)? };
            let model = match read_bool(&mut r)? {
                false => None, true => {
                    let scope_sha256 = read_hash(&mut r)?; let proof_sha256 = read_hash(&mut r)?;
                    let query_sha256 = read_hash(&mut r)?; let candidate_sha256 = read_hash(&mut r)?;
                    let proposition = r.string().map_err(error)?; let relation = r.u8().map_err(error)?;
                    let history_assessment_sha256 = read_optional_hash(&mut r)?;
                    let historical_positive_count = r.u32().map_err(error)?;
                    let historical_negative_count = r.u32().map_err(error)?;
                    let unresolved_history_count = r.u32().map_err(error)?;
                    let count = usize::from(r.u16().map_err(error)?);
                    if count > MAX_COUNTERS { return Err("IZ_FRONTIER_COUNTER_LIMIT".into()); }
                    let mut counters = Vec::with_capacity(count);
                    for _ in 0..count {
                        counters.push(CounterReference { candidate_id: r.u64().map_err(error)?,
                            source_sha256: read_hash(&mut r)?, transaction_id: read_hash(&mut r)?,
                            proof_sha256: read_hash(&mut r)?, model_family_sha256: read_hash(&mut r)? });
                    }
                    Some(ModelObservation { scope_sha256, proof_sha256, query_sha256, candidate_sha256,
                        proposition, relation, history_assessment_sha256, historical_positive_count,
                        historical_negative_count, unresolved_history_count, counters })
                }
            };
            let mut processing=if version>=2 {Some(ProcessingObservation {
                request_sha256:read_hash(&mut r)?,input_sha256:read_hash(&mut r)?,ledger_sha256:read_hash(&mut r)?,
                positive_count:r.u32().map_err(error)?,positive_sum:r.u32().map_err(error)?,
                negative_count:r.u32().map_err(error)?,negative_sum:r.u32().map_err(error)?,
                family_mask:r.u8().map_err(error)?,original:RequestObservation {
                    kind:RequestKind::decode(r.u8().map_err(error)?)?,refusal_sha256:read_optional_hash(&mut r)?,
                },model_operators:None,model_hcp_words:None,
            })} else {None};
            let model_measurement=if version>=2 && read_bool(&mut r)? {
                Some(crate::kie_measurement::KieMeasurement::decode(
                    r.take(crate::kie_measurement::KieMeasurement::WIRE_BYTES).map_err(error)?)?)
            } else {None};
            if version==3 || (version==4 && read_bool(&mut r)?) {
                let p=processing.as_mut().ok_or("IZ_PROCESS_MODEL_POLICY_SHAPE")?;
                p.model_operators=Some(crate::model_operator_measurement::ModelOperatorMeasurements::decode(
                    r.take(crate::model_operator_measurement::ModelOperatorMeasurements::WIRE_BYTES).map_err(error)?)?);
                let mut words=[0u64;11];
                for word in &mut words {*word=r.u64().map_err(error)?;}
                p.model_hcp_words=Some(words);
            }
            let historical_branch=if version==4 {Some(HistoricalBranchObservation {
                request_sha256:read_hash(&mut r)?, response_sha256:read_hash(&mut r)?,
                missing:r.u8().map_err(error)?,
            })} else {None};
            r.finish().map_err(error)?;
            Ok(Self { request, model, processing, model_measurement, historical_branch })
        })()?;
        parsed.validate()?;
        if parsed.encode()? != bytes { return Err("IZ_FRONTIER_NONCANONICAL".into()); }
        Ok(parsed)
    }

    /// Conservative arithmetic bound, without constructing provisional text.
    /// Non-historical labels/questions consume fewer than 4096 UTF-8 bytes.
    /// Each exact origin has at most a 20-digit ID, three 64-byte hex hashes
    /// and 52 punctuation/label bytes; each distinct source heading is <128.
    pub(crate) fn rendered_byte_bound(&self) -> Result<usize, String> {
        self.validate()?;
        let process_bound=usize::from(self.processing.is_some())*4096
            +usize::from(self.model_measurement.is_some())*2048
            +usize::from(self.uses_model_policy())*4096
            +usize::from(self.uses_historical_branch())*2048;
        let Some(model)=&self.model else { return Ok(4096+process_bound); };
        let proposition=model.proposition.chars().map(|ch|match ch {
            '\\' => 2, '|' => 6,
            other if other.is_control() => other.escape_unicode().count(),
            other => other.len_utf8(),
        }).sum::<usize>();
        let sources=model.counters.iter().map(|c|c.source_sha256).collect::<BTreeSet<_>>().len();
        Ok(4096 + process_bound + proposition + model.counters.len()*264 + sources*128)
    }

    pub fn render(&self) -> Result<String, String> {
        self.render_targets(self.targets())
    }
    /// Post-receipt projection: absence of a refusal is not an unknown refusal.
    /// Real missing conditions and counter references are never removed.
    pub(crate) fn render_verified_model(&self) -> Result<String,String> {
        if !self.model.as_ref().is_some_and(|m|m.relation==0) {
            return Err("IZ_MODEL_POSITIVE_RELATION".into());
        }
        let mut targets=self.targets();
        targets.remove(&Target::Uncertainty(1));
        self.render_targets(targets)
    }
    fn render_targets(&self, targets:BTreeSet<Target>) -> Result<String,String> {
        self.validate()?;
        let missing = targets.iter().filter(|t| matches!(t, Target::Missing(_) | Target::MissingProMeasurement | Target::MissingProAdmission
            | Target::MissingModelProPermission | Target::MissingModelNproPermission
            | Target::MissingHistoricalPositive | Target::MissingHistoricalNegative)).count();
        let counters = targets.iter().filter(|t| matches!(t, Target::CurrentCounter | Target::HistoricalCounter(_))).count();
        let unresolved = targets.len() - missing - counters;
        let mut out = format!("Frontiera iZ: M={missing}, C={counters}, U={unresolved}; Q=M∪C∪U. ");
        if let Some(m) = &self.model {
            let relation = match m.relation {
                0 => "KiS mbeshtet vetem pohimin kushtor nen modelin e deklaruar",
                1 => "KiC kundershton pohimin kushtor nen modelin e deklaruar",
                _ => "barazi/papercaktueshmeri: as KiS as KiC, jo kunderprove",
            };
            out.push_str(&format!("Pohimi ekzakt: [{}]. {relation}. scope_sha256={} proof_sha256={} query_sha256={} candidate_sha256={}. Historiku i klasifikuar: P={}, N={}, prejardhje_e_pazgjidhur={}; keto numerime nuk jane besueshmeri probabilistike dhe mungesa e P/N historike nuk eshte kusht i ri pranimi per KiE. ",
                display(&m.proposition), hex(&m.scope_sha256), hex(&m.proof_sha256), hex(&m.query_sha256),
                hex(&m.candidate_sha256), m.historical_positive_count, m.historical_negative_count, m.unresolved_history_count));
        }
        if let Some(value)=&self.model_measurement { out.push_str(&value.render()?); }
        if let Some(p)=&self.processing {
            if self.uses_model_policy() {
                out.push_str("ESS_MAI_ORIGINAL: matjet e meposhtme jane te kandidateve/kushteve origjinale; ato nuk bashkohen me radianet e KiE. ");
            }
            let mean=|sum:u32,count:u32|if count==0 {"undefined".to_string()} else {(sum/count).to_string()};
            out.push_str(&format!("Matja e procesit aktual: positive_count={} positive_sum={} positive_mean={} negative_condition_count={} negative_condition_sum={} negative_condition_mean={} family_mask={}; input_sha256={} ledger_sha256={} current_request_sha256={}. Matjet e kushteve negative nuk jane kunderprova semantike; mungesa e kandidatit nuk eshte eliminim. ",
                p.positive_count,p.positive_sum,mean(p.positive_sum,p.positive_count),p.negative_count,p.negative_sum,
                mean(p.negative_sum,p.negative_count),p.family_mask,hex(&p.input_sha256),hex(&p.ledger_sha256),hex(&p.request_sha256)));
            if let Some(hash)=p.original.refusal_sha256 {
                out.push_str(&format!("Kushti origjinal i tentatives: {} original_refusal_sha256={}. ",p.original.kind.label(),hex(&hash)));
            }
        }
        if let Some(operators)=self.model_operators() {
            let summary=operators.summarize()?;
            let mean=|v:Option<f64>|v.map(|x|format!("{x:.17e}")).unwrap_or_else(||"undefined".into());
            for (label,side) in [("KIS_TO_PRO",summary.pro),("KIC_TO_NPRO",summary.npro)] {
                out.push_str(&format!("KIE_MODEL_{label}: predicate_tested={} outcome={:?} candidate_count={} sum_rad={:.17e} mean_rad={}; ",
                    side.predicate_tested,side.outcome,side.candidates.count,side.candidates.sum_radians,
                    mean(side.candidates.mean_radians)));
            }
            let words=self.model_hcp_words().ok_or("IZ_PROCESS_MODEL_HCP_FORMAT")?;
            out.push_str(&format!("KIE_MODEL_UNRESOLVED={}; HCP_MODEL_RESOURCE bytes={} parallel={} active={}. Keto jane matje shkencore te ciklit aktual; testi me zero nuk shpik kandidat semantik, dhe leja e ekzekutimit nuk jep verdikt. ",
                summary.unresolved,words[8],words[9],words[10]));
        }
        if let Some(branch)=&self.historical_branch {
            out.push_str(&format!("[BRANCH:authority=ASHT route=ESS_MAI outcome={}] selection_request_sha256={} selection_response_sha256={}. Ky eshte rezultati i perzgjedhjes historike; nuk rishkruan KiS/KiC, nuk transferon autoritet dhe nuk jep verdikt. ",
                branch.label(),hex(&branch.request_sha256),hex(&branch.response_sha256)));
        }
        for target in &targets {
            match target {
                Target::MissingHistoricalPositive => out.push_str("[M:ASHT_POSITIVE_EVIDENCE_MISSING] ASHT nuk gjeti kandidatin pozitiv ekzakt per kete kerkese ne rrjedhen ESS_MAI. Cila evidence me burim dhe dallim Xi/Yi ploteson kete mungese ne kontekstin e pohimit? KiS/KiC ruajne rezultatet e tyre te pavarura; mungesa nuk eshte kunderprove dhe nuk bllokon vetvetiu rrjedhen shkencore. "),
                Target::MissingHistoricalNegative => out.push_str("[M:ASHT_NEGATIVE_EVIDENCE_MISSING] ASHT nuk gjeti kandidatin negativ ekzakt per kete kerkese ne rrjedhen ESS_MAI. Cila evidence kundershtuese me burim mund te testohet per kete pohim dhe kontekst, nese ekziston? Mos shpik kunderprove. KiC ruan kundershtimin e vet shkencor; ky deshtim perzgjedhjeje nuk e fshin dhe nuk transferon autoritet. "),
                Target::MissingModelProPermission => out.push_str("[M:KIE_PRO_EXECUTION_PERMISSION_MISSING] Testi numerik mbeshtetes nuk ka lejen e regjistruar per ekzekutim ne PRO. Cili kusht i burimeve ose autorizimit te ketij cikli duhet plotesuar qe te provohet mbeshtetja? Kjo eshte mungese ekzekutimi, jo kundershtim i tezes. "),
                Target::MissingModelNproPermission => out.push_str("[M:KIE_NPRO_EXECUTION_PERMISSION_MISSING] Testi numerik kundershtues nuk ka lejen e regjistruar per ekzekutim ne NPRO. Cili kusht i burimeve ose autorizimit te ketij cikli duhet plotesuar qe te testohet kundershtimi? Mungesa e testimit nuk eshte prove e mungeses se kundershtimit. "),
                Target::MissingProMeasurement => out.push_str("[M:PRO_POSITIVE_MEASUREMENT_MISSING] PRO nuk regjistroi kandidat pozitiv te matur ne kete cikel (n+=0, mesatarja e papercaktuar). Cila evidence me burim ose dallim i kontrollueshem mund te matet per kete pohim, dhe cili kusht i ekzekutimit duhet plotesuar per ta matur? Kjo mungese nuk eshte kunderprove dhe nuk provon gabim te perdoruesit. "),
                Target::MissingProAdmission => out.push_str("[M:PRO_POSITIVE_ADMISSION_MISSING] Kandidati pozitiv eshte matur, por familja PRO nuk eshte pranuar ne kete tentative. Cili kusht i dokumentuar i pranimit mbetet i paplotesuar dhe cila evidence e ploteson? Matja zero nuk quhet mungese kandidati dhe pyetja nuk jep leje pranimi. "),
                Target::Missing(kind) => {
                    let question = match RequestKind::decode(*kind)? {
                        RequestKind::PrimitiveMaterial => "Cili dallim konkret Xi/Yi, me burim, siguron materialin qe mungon per shqyrtim?",
                        RequestKind::MissingSide => "Ana e pranishme ruhet. Cili element Xi ose Yi qe mungon, me burim, ploteson kete rast?",
                        RequestKind::CoPresence => "Kandidatet ruhen. Cili fragment me burim dokumenton bashkeprezencen Xi/Yi ose sqaron lidhjen e propozuar?",
                        RequestKind::OperatorCausalEvidence => "Xi/Yi ruhen. Cila prove shkakore ose dalluese me burim adreson dobesine konkrete te operatorit?",
                        RequestKind::HistoricalCounterRequired => "Rruga historike raportoi mungese kunderkandidati. Cila prove kundershtuese me burim lejon testimin e saj, pa shpikur kunderprove?",
                        RequestKind::ShadowMissingNegativeProof => "Shadow raportoi se refuzimi nuk ka prove negative te lidhur qe lejon mbylljen e tij. Cila deshmi ekzakte per kushtin e deshtuar, ose kunderprove nese ekziston, e lidh kete rezultat me burimin e vet? Nuk kerkohet shpikje e negativit dhe mungesa nuk behet kunderprove.",
                        _ => return Err("IZ_FRONTIER_MISSING_KIND".into()),
                    };
                    out.push_str(&format!("[M:{kind} condition={}] {question} Mungesa e ketij kushti nuk e rrezon vete pohimin. ", RequestKind::decode(*kind)?.label()));
                }
                Target::CurrentCounter => {
                    let m = self.model.as_ref().ok_or("IZ_FRONTIER_CURRENT_MODEL_MISSING")?;
                    out.push_str(&format!("[C:KiC:{}] counter=CURRENT_KIC Cilin input, kusht zbatueshmerie ose prove te kontrollueshme sqaron per kete kundershtim te pohimit ekzakt? Mund ta pranosh kundershtimin, te rishikosh pohimin ose te japesh prove per ta testuar; nuk kerkohet mbrojtje e detyruar. ",hex(&m.proof_sha256)));
                }
                Target::HistoricalCounter(source) => {
                    let model = self.model.as_ref().ok_or("IZ_FRONTIER_HISTORICAL_SOURCE_MISSING")?;
                    out.push_str(&format!("[C:historik:{}] counter=EXACT_SCOPED_COUNTERPROOF ", hex(source)));
                    for c in model.counters.iter().filter(|c| &c.source_sha256 == source) {
                        out.push_str(&format!("candidate_id={} T1={} source_proof={} model_family={}; ",
                            c.candidate_id, hex(&c.transaction_id), hex(&c.proof_sha256), hex(&c.model_family_sha256)));
                    }
                }
                Target::Uncertainty(kind) => {
                    let question = match kind {
                        1 => "Shkaku specifik i refuzimit final nuk eshte deklaruar. Cili kusht mbeti i paprovuar? Ky regjistrim nuk ia atribuon deshtimin nje autoriteti te caktuar.",
                        2 => "Arsyeja e raportuar nuk identifikon nje kusht te strukturuar. Cila evidence dhe cili kusht konkret mungon? Nuk shpallet kunderprove nga teksti i arsyes.",
                        3 => "Cila evidence lidh prejardhjen e pazgjidhur me pohimin dhe kontekstin ekzakt? E panjohura nuk behet kunderprove ose leje pranimi.",
                        4 => "Cila matje ose evidence dalluese zgjidh barazine pa ndryshuar arbitrarisht tolerancen? Barazia nuk eshte kunderprove.",
                        5 => "Historiku i trasheguar nuk ka klasifikim te provuar te prejardhjes. Cila lidhje e kontrollueshme percakton rolin e tij ndaj pohimit?",
                        6 => "Arsyeja permend kundershtim, por ketu nuk ka referencen perkatese te provuar. Cila prove e sakte e mbeshtet ate deklarim?",
                        _ => return Err("IZ_FRONTIER_UNCERTAINTY_KIND".into()),
                    };
                    out.push_str(&format!("[U:{kind}] {question} "));
                }
            }
        }
        if targets.iter().any(|t| matches!(t, Target::HistoricalCounter(_))) {
            out.push_str("Per secilen kunderprove historike te listuar: cili sqarim, prove ose rishikim i pohimit ekzakt e adreson ne kete kontekst? Pranimi i kundershtimit eshte gjithashtu pergjigje e vlefshme. ");
        }
        if let Some(hash) = self.request.refusal_sha256 { out.push_str(&format!("refusal_sha256={}. ", hex(&hash))); }
        out.push_str("D-BAB deshmon; frontiera nuk jep veto ose verdikt. Pyetjet ngushtojne provat e kerkuara, nuk parashikojne i0.");
        if out.len() > MAX_RENDER_BYTES { return Err("IZ_FRONTIER_RENDER_LIMIT".into()); }
        Ok(out)
    }
}

fn write_optional_hash(w: &mut Writer, value: Option<Hash>) {
    match value { None => w.u8(0), Some(hash) => { w.u8(1); w.bytes.extend_from_slice(&hash); } }
}
fn read_bool(r: &mut Reader<'_>) -> Result<bool, String> {
    match r.u8().map_err(|e| format!("IZ_FRONTIER_CODEC:{e:?}"))? {
        0 => Ok(false), 1 => Ok(true), _ => Err("IZ_FRONTIER_BOOL".into()),
    }
}
fn read_hash(r: &mut Reader<'_>) -> Result<Hash, String> {
    let mut hash = [0; 32];
    hash.copy_from_slice(r.take(32).map_err(|e| format!("IZ_FRONTIER_CODEC:{e:?}"))?);
    Ok(hash)
}
fn read_optional_hash(r: &mut Reader<'_>) -> Result<Option<Hash>, String> {
    if read_bool(r)? { Ok(Some(read_hash(r)?)) } else { Ok(None) }
}
fn display(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str(r"\\"), '|' => out.push_str(r"\u{7c}"),
            other if other.is_control() => out.extend(other.escape_unicode()),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
pub(crate) fn test_proof(relation: u8) -> kie::KieProof {
    // Synthetic codec/partition fixture. Neither these query bytes nor the
    // identifiers constitute a scientific evaluation or native admission.
    let identity = crate::pd_memory::PdIdentity { owner:"o".into(), obligation:"b".into(),
        case_id:"c".into(), scope:"s".into(), domain:"d".into(), lineage_root:"r".into() };
    let (historical_request, historical_response) = kie::test_history("typed-model");
    kie::KieProof {
        context: kie::KieContext { identity:identity.clone(), operation_id:"op".into(), record_id:[1;32] },
        query_bytes:vec![1], candidate_bytes:vec![2], canonical_proposition:"typed-model".into(),
        historical_request, historical_response, kis_mass:if relation==0 {10_000} else {0},
        kic_mass:if relation==1 {10_000} else {0}, relation, proof_sha256:[0;32], prior_adjudication:None,
        history_assessment:Some(kie::KieHistoryAssessment { scope_sha256:identity.key().unwrap(),
            proposition_sha256:sha(b"typed-model"), entries:vec![], scoped_evidence:vec![] }),
    }.seal_data().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    const GAP: &str = "OPERATOR_EVIDENCE_UNRESOLVED: causal evidence missing, Xi/Yi retained";

    fn negative(id: u64, source: u8, proposition: &str) -> crate::CandidateStatisticWire {
        crate::CandidateStatisticWire {
            candidate_id:id, polarity:0, domain:"negative".into(),
            term_sha256:crate::sha256_hex(&[crate::pd_memory::failure::relevance::TERM_NEGATIVE_DOMAIN,
                proposition.as_bytes()]), source_sha256:hex(&[source;32]), security_y:1, trust_x:1,
            confidence_fixed:3500, frequency:1, domain_count:1, legacy_score_fixed:3500,
        }
    }
    fn provenance() -> kie::KieHistoryProvenance {
        kie::KieHistoryProvenance { transaction_id:[6;32],proof_sha256:[7;32],model_family_sha256:[8;32] }
    }
    fn classified_history(mut proof:kie::KieProof,
        negatives:Vec<(crate::CandidateStatisticWire,kie::KieHistoryClass)>,
        scoped:Vec<kie::KieScopedEvidence>) -> kie::KieProof {
        // Synthetic schema fixtures. Actual classification is an independent
        // Shadow source-record obligation, not established by this helper.
        let mut response=crate::decode_selection_response(&proof.historical_response).unwrap();
        response.negative=negatives.iter().map(|(c,_)|c.clone()).collect();
        response.selection_sha256=crate::pd_memory::failure::relevance::selection_digest(&response);
        proof.historical_response=crate::encode_selection_response(&response);
        let history=proof.history_assessment.as_mut().unwrap();
        history.entries=negatives.iter().map(|(c,class)|kie::KieHistoryEntry {
            selection_context:None,
            polarity:c.polarity,candidate_id:c.candidate_id,source_sha256:c.source_sha256.clone(),class:*class,
            provenance:if *class==kie::KieHistoryClass::LegacyUnscoped {None} else {Some(provenance())},
        }).collect();
        history.entries.sort_by(|a,b|a.key().cmp(&b.key()));
        history.scoped_evidence=scoped;
        proof.seal_data().unwrap()
    }

    fn current_request_fixture()->crate::ShadowCycleRequest {
        let bytes=include_bytes!("../../shadow/src/test_fixtures/late_refusal_r25/attempt.pdt1");
        let (request,_)=crate::pd_cycle::decode_cycle(bytes).unwrap();
        crate::decode_cycle_request(&request).unwrap()
    }
    #[test]
    fn r27_current_ledger_projection_preserves_legacy_and_rejects_relabelled_counts() {
        let request=current_request_fixture();
        let legacy=IzFrontier::from_observation(Some(GAP),None).unwrap();
        let legacy_bytes=legacy.encode().unwrap();let legacy_text=legacy.render().unwrap();
        assert_eq!(&legacy_bytes[8..10],&1u16.to_le_bytes());
        let restored=IzFrontier::decode(&legacy_bytes).unwrap();
        assert_eq!(restored.encode().unwrap(),legacy_bytes);assert_eq!(restored.render().unwrap(),legacy_text);
        assert!(!legacy_text.contains("PRO_POSITIVE_MEASUREMENT_MISSING"));
        let current=IzFrontier::from_current_request(Some(GAP),&request,None).unwrap();
        current.validate_current_request(&request).unwrap();
        let bytes=current.encode().unwrap();assert_eq!(&bytes[8..10],&2u16.to_le_bytes());
        assert_eq!(IzFrontier::decode(&bytes).unwrap(),current);
        assert!(current.render().unwrap().contains("positive_count=0"));
        assert!(current.render().unwrap().len()<=current.rendered_byte_bound().unwrap());
        let mut relabelled=current.clone();
        relabelled.processing.as_mut().unwrap().positive_count=1;
        relabelled.validate().unwrap();
        assert_eq!(relabelled.validate_current_request(&request).unwrap_err(),"IZ_PROCESS_ACTUAL_REQUEST");
        assert_eq!(current.validate_request_hash(&[42;32]).unwrap_err(),"IZ_PROCESS_REQUEST_HASH");
        let mut changed=request.clone();
        changed.quantum.final_evidence.quantum_action_evidence.iter_mut()
            .find(|a|a.stage==crate::ACTION_STAGE_MPRO).unwrap().evidence_words[1]=99;
        assert!(IzFrontier::from_current_request(Some(GAP),&changed,None).is_err(),"marked malformed ledger cannot fall back to v1");
    }
    #[test]
    fn r27_late_condition_keeps_original_gap_and_counter_as_independent_targets() {
        let mut request=current_request_fixture();
        request.quantum.collapse_refused=Some(GAP.into());
        let proof=test_proof(1); // synthetic model classification; no native authority
        let current=IzFrontier::from_current_request(Some("SHADOW_MISSING_NEGATIVE_PROOF"),&request,Some(&proof)).unwrap();
        assert_eq!(current.targets(),BTreeSet::from([Target::Missing(11),Target::Missing(4),
            Target::MissingProMeasurement,Target::CurrentCounter]));
        let text=current.render().unwrap();
        for label in ["SHADOW_MISSING_NEGATIVE_PROOF","OPERATOR_EVIDENCE_UNRESOLVED",
            "PRO_POSITIVE_MEASUREMENT_MISSING","counter=CURRENT_KIC"] {assert!(text.contains(label),"{label}");}
        assert_eq!(text.matches("counter=CURRENT_KIC").count(),1);
    }
    #[test]
    fn r27_numerical_output_and_question_keep_exact_current_binding() {
        let request=current_request_fixture();let proof=test_proof(0);
        let numeric=crate::kie_measurement::KieMeasurement::from_comparison(&proof,
            &crate::pd_memory::iz_science_math::EraComparison {
                query_sha256:sha(&proof.query_bytes),result_sha256:[8;32],model_sha256:[9;32],
                claim_radians:0.0,computed_radians:0.25,residual_radians:0.25,tolerance_radians:0.5,
            }).unwrap();
        assert!(IzFrontier::from_observation(Some(GAP),Some(&proof)).unwrap()
            .with_model_measurement(&proof,numeric.clone()).is_err(),"legacy cannot silently acquire new numeric output");
        let current=IzFrontier::from_current_request(Some(GAP),&request,Some(&proof)).unwrap()
            .with_model_measurement(&proof,numeric.clone()).unwrap();
        current.validate_model(Some(&proof)).unwrap();
        assert_eq!(current.model_measurement(),Some(&numeric));
        let text=current.render().unwrap();
        assert!(text.contains("MODEL_RESULT MODEL_COMPUTED") && text.contains("PRO_POSITIVE_MEASUREMENT_MISSING"));
        assert!(text.len()<=current.rendered_byte_bound().unwrap());
        let restored=IzFrontier::decode(&current.encode().unwrap()).unwrap();
        assert_eq!(restored.render().unwrap(),text);restored.validate_current_request(&request).unwrap();
        assert!(restored.validate_model(Some(&test_proof(1))).is_err());
    }

    // Synthetic codec/observation fixtures. The real provider/owner acceptance
    // is covered separately by the native process probe, not asserted here.
    fn model_request_fixture(relation:u8,pro:bool,npro:bool)
        ->(crate::ShadowCycleRequest,kie::KieProof,crate::model_operator_measurement::ModelOperatorMeasurements) {
        let proof=test_proof(relation);
        let tolerance=match relation {0=>0.5,1=>0.125,_=>0.25};
        let numeric=crate::kie_measurement::KieMeasurement::from_comparison(&proof,
            &crate::pd_memory::iz_science_math::EraComparison {
                query_sha256:sha(&proof.query_bytes),result_sha256:[8;32],model_sha256:[9;32],
                claim_radians:0.0,computed_radians:0.25,residual_radians:0.25,tolerance_radians:tolerance,
            }).unwrap();
        let operators=crate::model_operator_measurement::ModelOperatorMeasurements::new(numeric,pro,npro).unwrap();
        let mut request=current_request_fixture();
        let evidence=&mut request.quantum.final_evidence;
        let old=crate::candidate_measurement::CandidateLedger::from_words(
            &evidence.quantum_action_evidence.iter().find(|a|a.stage==crate::ACTION_STAGE_MPRO)
                .unwrap().evidence_words).unwrap();
        evidence.light_input_bytes=proof.canonical_proposition.as_bytes().to_vec();
        evidence.light_input_sha256=hex(&sha(&evidence.light_input_bytes));
        let ledger=crate::candidate_measurement::CandidateLedger::new(
            sha(&evidence.light_input_bytes),old.observations().to_vec()).unwrap()
            .with_model_measurements(operators.clone()).unwrap();
        evidence.quantum_action_evidence.iter_mut().find(|a|a.stage==crate::ACTION_STAGE_MPRO)
            .unwrap().evidence_words=ledger.to_words();
        let needed=(crate::kie_measurement::KieMeasurement::WIRE_BYTES
            +proof.query_bytes.len()+proof.candidate_bytes.len()) as u64;
        evidence.quantum_action_evidence.iter_mut().find(|a|a.stage==crate::ACTION_STAGE_HCP)
            .unwrap().evidence_words=vec![1,0,3,0,1,5,0,0,needed,1,1];
        request.quantum.collapse_refused=Some(GAP.into());
        (request,proof,operators)
    }

    #[test]
    fn r28_frontier_model_operators_preserve_original_counts_and_exact_v2() {
        let legacy_request=current_request_fixture();
        let v2=IzFrontier::from_current_request(Some(GAP),&legacy_request,None).unwrap();
        let old_bytes=v2.encode().unwrap();let old_text=v2.render().unwrap();
        for relation in 0..3 {
            let (request,proof,operators)=model_request_fixture(relation,true,true);
            let current=IzFrontier::from_current_request(Some(GAP),&request,Some(&proof)).unwrap()
                .with_model_measurement(&proof,operators.measurement().clone()).unwrap();
            current.validate_current_request(&request).unwrap();
            current.validate_model(Some(&proof)).unwrap();
            assert!(current.uses_model_policy());
            assert_eq!(current.model_operators(),Some(&operators));
            assert_eq!(current.model_hcp_words().unwrap().len(),11);
            let bytes=current.encode().unwrap();
            assert_eq!(&bytes[8..10],&3u16.to_le_bytes());
            let restored=IzFrontier::decode(&bytes).unwrap();
            assert_eq!(restored,current);
            assert_eq!(restored.render().unwrap(),current.render().unwrap());
            let text=current.render().unwrap();
            assert!(text.contains("ESS_MAI_ORIGINAL") && text.contains("positive_count=0")
                && text.contains("negative_condition_count=4"));
            assert!(text.contains("PRO_POSITIVE_MEASUREMENT_MISSING"));
            assert!(text.contains("KIE_MODEL_KIS_TO_PRO") && text.contains("KIE_MODEL_KIC_TO_NPRO"));
            assert!(text.contains("sum_rad=") && text.contains("mean_rad=undefined"));
            assert_eq!(text.contains("counter=CURRENT_KIC"),relation==1);
            assert_eq!(text.contains("KIE_MODEL_UNRESOLVED=true"),relation==2);
            assert!(text.len()<=current.rendered_byte_bound().unwrap());
        }
        let old=IzFrontier::decode(&old_bytes).unwrap();
        assert_eq!(old.encode().unwrap(),old_bytes);assert_eq!(old.render().unwrap(),old_text);
        assert!(!old.uses_model_policy() && old.model_hcp_words().is_none());
    }

    #[test]
    fn r28_frontier_rejects_model_resource_relabelling_and_malformed_version() {
        let (request,proof,operators)=model_request_fixture(0,true,true);
        let current=IzFrontier::from_current_request(Some(GAP),&request,Some(&proof)).unwrap()
            .with_model_measurement(&proof,operators.measurement().clone()).unwrap();
        let mut altered=current.clone();
        altered.processing.as_mut().unwrap().model_hcp_words.as_mut().unwrap()[8]+=1;
        altered.validate().unwrap();
        assert_eq!(altered.validate_current_request(&request).unwrap_err(),"IZ_PROCESS_ACTUAL_REQUEST");
        let mut other=request.clone();
        other.quantum.final_evidence.quantum_action_evidence.iter_mut()
            .find(|a|a.stage==crate::ACTION_STAGE_HCP).unwrap().evidence_words[9]+=1;
        assert_eq!(current.validate_current_request(&other).unwrap_err(),"IZ_PROCESS_ACTUAL_REQUEST");
        let mut missing=current.clone();missing.processing.as_mut().unwrap().model_operators=None;
        assert_eq!(missing.validate().unwrap_err(),"IZ_PROCESS_MODEL_POLICY_SHAPE");
        let mut other_proof=proof.clone();other_proof.query_bytes.push(3);
        assert!(current.validate_model(Some(&other_proof.seal_data().unwrap())).is_err());
        let bytes=current.encode().unwrap();
        for end in [0,10,bytes.len()-1,bytes.len()-88,bytes.len()-216] {
            assert!(IzFrontier::decode(&bytes[..end]).is_err());
        }
        let mut downgraded=bytes.clone();downgraded[8..10].copy_from_slice(&2u16.to_le_bytes());
        assert!(IzFrontier::decode(&downgraded).is_err());
        let mut extra=bytes.clone();extra.push(0);assert!(IzFrontier::decode(&extra).is_err());
        let mut mismatched=current.clone();
        let (_,_,counter)=model_request_fixture(1,true,true);
        mismatched.model_measurement=Some(counter.measurement().clone());
        assert_eq!(mismatched.validate().unwrap_err(),"IZ_MODEL_OPERATOR_MEASUREMENT_MISMATCH");
        let mut unearned=current.clone();
        unearned.processing.as_mut().unwrap().model_hcp_words.as_mut().unwrap()[10]=0;
        assert_eq!(unearned.validate_model(Some(&proof)).unwrap_err(),"IZ_MODEL_OPERATOR_RESOURCE_MISMATCH");
        let (request,proof,_)=model_request_fixture(0,true,false);
        let unperformed=IzFrontier::from_current_request(Some(GAP),&request,Some(&proof)).unwrap();
        assert_eq!(unperformed.validate_model(Some(&proof)).unwrap_err(),"IZ_MODEL_OPERATOR_RESOURCE_MISMATCH");
    }

    #[test]
    fn r28_held_model_endpoint_and_late_refusal_preserve_real_counter_and_original_gap() {
        let (mut request,proof,operators)=model_request_fixture(1,false,false);
        request.quantum.final_evidence.quantum_action_evidence.iter_mut()
            .find(|a|a.stage==crate::ACTION_STAGE_HCP).unwrap().evidence_words[10]=0;
        let current=IzFrontier::from_current_request(Some("SHADOW_MISSING_NEGATIVE_PROOF"),&request,Some(&proof)).unwrap()
            .with_model_measurement(&proof,operators.measurement().clone()).unwrap();
        let text=current.render().unwrap();
        for expected in ["KIE_PRO_EXECUTION_PERMISSION_MISSING","counter=CURRENT_KIC",
            "SHADOW_MISSING_NEGATIVE_PROOF","OPERATOR_EVIDENCE_UNRESOLVED",
            "PRO_POSITIVE_MEASUREMENT_MISSING","KIE_MODEL_KIS_TO_PRO: predicate_tested=false outcome=Held",
            "KIE_MODEL_KIC_TO_NPRO: predicate_tested=false outcome=Held"] {
            assert!(text.contains(expected),"{expected}: {text}");
        }
        assert!(text.contains("KIE_NPRO_EXECUTION_PERMISSION_MISSING"));
        assert_eq!(text.matches("counter=CURRENT_KIC").count(),1);
        let (held_request,held_proof,_)=model_request_fixture(0,true,false);
        let held=IzFrontier::from_current_request(Some(GAP),&held_request,Some(&held_proof)).unwrap();
        assert!(held.render().unwrap().contains("KIE_NPRO_EXECUTION_PERMISSION_MISSING"));
        assert!(!held.render().unwrap().contains("counter=CURRENT_KIC"));
    }


    fn r29_selection(proof:kie::KieProof, missing:u8)->kie::KieProof {
        // Synthetic selection fixtures test exact projection, never admission.
        let mut proof=proof;
        let request=crate::decode_selection_request(&proof.historical_request).unwrap();
        let mut response=crate::decode_selection_response(&proof.historical_response).unwrap();
        response.positive.clear();response.negative.clear();
        if missing!=1 {
            response.positive.push(crate::CandidateStatisticWire {
                candidate_id:1,polarity:1,domain:"primitive".into(),
                term_sha256:crate::sha256_hex(&[crate::pd_memory::failure::relevance::TERM_PRIMITIVE_DOMAIN,
                    request.xi[0].as_bytes()]),source_sha256:hex(&[55;32]),
                security_y:1,trust_x:1,confidence_fixed:7500,frequency:1,domain_count:1,legacy_score_fixed:7500,
            });
        }
        if missing==0 {response.negative.push(negative(2,66,&request.xi[0]));}
        response.selection_sha256=crate::pd_memory::failure::relevance::selection_digest(&response);
        proof.historical_response=crate::encode_selection_response(&response);
        proof.history_assessment=None;
        proof.seal_data().unwrap()
    }

    #[test]
    fn r29_branch_questions_preserve_independent_support_counter_and_equality() {
        for relation in 0..3 {
            let request=current_request_fixture();
            for missing in 0..3 {
                let proof=r29_selection(test_proof(relation),missing);
                let legacy=IzFrontier::from_current_request(Some(GAP),&request,Some(&proof)).unwrap();
                let old_bytes=legacy.encode().unwrap();let old_text=legacy.render().unwrap();
                let current=legacy.with_historical_branch(&proof).unwrap();
                current.validate_model(Some(&proof)).unwrap();
                current.validate_current_request(&request).unwrap();
                assert_eq!(current.historical_branch.as_ref().unwrap().missing,missing);
                let text=current.render().unwrap();
                assert_eq!(text.contains("ASHT_POSITIVE_EVIDENCE_MISSING"),missing==1);
                assert_eq!(text.contains("ASHT_NEGATIVE_EVIDENCE_MISSING"),missing==2);
                assert_eq!(text.contains("counter=CURRENT_KIC"),relation==1);
                assert!(text.contains("OPERATOR_EVIDENCE_UNRESOLVED"));
                assert!(text.len()<=current.rendered_byte_bound().unwrap());
                let bytes=current.encode().unwrap();
                assert_eq!(&bytes[8..10],&4u16.to_le_bytes());
                assert_eq!(IzFrontier::decode(&bytes).unwrap().render().unwrap(),text);
                let old=IzFrontier::decode(&old_bytes).unwrap();
                assert_eq!(old.render().unwrap(),old_text);assert_eq!(old.encode().unwrap(),old_bytes);
                assert!(!old.uses_historical_branch());
            }
        }
    }

    #[test]
    fn r29_branch_relabel_and_resealed_foreign_selection_are_rejected() {
        let request=current_request_fixture();let proof=test_proof(0);
        let current=IzFrontier::from_current_request(Some(GAP),&request,Some(&proof)).unwrap()
            .with_historical_branch(&proof).unwrap();
        for field in 0..3 {
            let mut bad=current.clone();let b=bad.historical_branch.as_mut().unwrap();
            match field {0=>b.missing=2,1=>b.request_sha256[0]^=1,_=>b.response_sha256[0]^=1};
            bad.validate().unwrap();
            assert_eq!(bad.validate_model(Some(&proof)).unwrap_err(),"IZ_HISTORY_ACTUAL_SELECTION");
        }
        let mut bad=proof.clone();
        let mut response=crate::decode_selection_response(&bad.historical_response).unwrap();
        response.session_id.push('x');
        response.selection_sha256=crate::pd_memory::failure::relevance::selection_digest(&response);
        bad.historical_response=crate::encode_selection_response(&response);
        assert!(current.clone().with_historical_branch(&bad).is_err(),
            "a new digest cannot turn foreign context into missing evidence");
        let mut bad=proof.clone();bad.historical_response[0]^=1;
        assert!(current.with_historical_branch(&bad).is_err());
    }

    #[test]
    fn r29_v4_keeps_v3_numeric_bytes_and_held_endpoint_questions() {
        for permitted in [false,true] {
            let (mut request,proof,operators)=model_request_fixture(1,permitted,permitted);
            if !permitted {
                request.quantum.final_evidence.quantum_action_evidence.iter_mut()
                    .find(|a|a.stage==crate::ACTION_STAGE_HCP).unwrap().evidence_words[10]=0;
            }
            let legacy=IzFrontier::from_current_request(Some(GAP),&request,Some(&proof)).unwrap()
                .with_model_measurement(&proof,operators.measurement().clone()).unwrap();
            let old_bytes=legacy.encode().unwrap();let old_text=legacy.render().unwrap();
            assert_eq!(&old_bytes[8..10],&3u16.to_le_bytes());
            let current=legacy.with_historical_branch(&proof).unwrap();
            current.validate_model(Some(&proof)).unwrap();
            let bytes=current.encode().unwrap();let restored=IzFrontier::decode(&bytes).unwrap();
            restored.validate_current_request(&request).unwrap();restored.validate_model(Some(&proof)).unwrap();
            assert_eq!(restored.model_operators(),Some(&operators));
            assert_eq!(restored.render().unwrap().contains("KIE_NPRO_EXECUTION_PERMISSION_MISSING"),!permitted);
            assert!(restored.render().unwrap().contains("ASHT_POSITIVE_EVIDENCE_MISSING"));
            assert!(restored.render().unwrap().contains("counter=CURRENT_KIC"));
            assert_eq!(IzFrontier::decode(&old_bytes).unwrap().render().unwrap(),old_text);
            for end in [0,10,bytes.len()-1,bytes.len()-65] {assert!(IzFrontier::decode(&bytes[..end]).is_err());}
            let mut bad=bytes.clone();bad[8..10].copy_from_slice(&3u16.to_le_bytes());
            assert!(IzFrontier::decode(&bad).is_err());
            let mut bad=bytes;bad.push(0);assert!(IzFrontier::decode(&bad).is_err());
        }
    }

    #[test]
    fn exact_counter_refs_are_retained_but_aliases_do_not_multiply_questions() {
        use kie::KieHistoryClass::{ExactModelCounterproof, LegacyUnscoped, ProcessFailure};
        let counter=negative(1,10,"typed-model");
        let mut alias=counter.clone(); alias.candidate_id=2;
        let procedural=negative(3,11,"typed-model");
        let unknown=negative(4,12,"typed-model");
        let proof=classified_history(test_proof(1),vec![(counter.clone(),ExactModelCounterproof),
            (alias,ExactModelCounterproof),(procedural,ProcessFailure),(unknown,LegacyUnscoped)],
            vec![kie::KieScopedEvidence { candidate:counter,class:ExactModelCounterproof,provenance:provenance() }]);
        let frontier=IzFrontier::from_observation(Some(GAP),Some(&proof)).unwrap();
        assert_eq!(frontier.model.as_ref().unwrap().counters.len(),2,"same ref through lexical and scoped indexes is one ref; distinct candidate ID is preserved");
        assert_eq!(frontier.targets(),BTreeSet::from([Target::Missing(4),Target::CurrentCounter,
            Target::HistoricalCounter([10;32]),Target::Uncertainty(3)]));
        let text=frontier.render().unwrap();
        assert_eq!(text.matches("counter=EXACT_SCOPED_COUNTERPROOF").count(),1);
        for exact in ["candidate_id=1","candidate_id=2",&hex(&[6;32]),&hex(&[7;32]),&hex(&[8;32])] {
            assert!(text.contains(exact));
        }
        assert!(!text.contains(&format!("[C:historik:{}]",hex(&[11;32]))));
        assert!(!text.contains(&format!("[C:historik:{}]",hex(&[12;32]))));
        let decoded=IzFrontier::decode(&frontier.encode().unwrap()).unwrap();
        decoded.validate_model(Some(&proof)).unwrap();
        let mut altered=decoded; altered.model.as_mut().unwrap().counters[0].transaction_id=[9;32];
        let encoded=altered.encode().unwrap();
        assert_eq!(IzFrontier::decode(&encoded).unwrap().validate_model(Some(&proof)).unwrap_err(),"IZ_FRONTIER_ACTUAL_MODEL");
        let mut absent_source=proof;
        absent_source.history_assessment.as_mut().unwrap().entries[0].provenance=None;
        assert!(absent_source.seal_data().is_err(),"unbacked classification cannot supply a counter target");
    }

    #[test]
    fn late_missing_negative_proof_is_an_obligation_not_a_semantic_negative() {
        const LATE:&str="SHADOW_MISSING_NEGATIVE_PROOF";
        let missing=IzFrontier::from_observation(Some(LATE),None).unwrap();
        assert_eq!(missing.targets(),BTreeSet::from([Target::Missing(11)]));
        assert!(missing.render().unwrap().contains("condition=SHADOW_MISSING_NEGATIVE_PROOF"));
        let equality=IzFrontier::from_observation(Some(LATE),Some(&test_proof(2))).unwrap();
        assert_eq!(equality.targets(),BTreeSet::from([Target::Missing(11),Target::Uncertainty(4)]));
        let counter=IzFrontier::from_observation(Some(LATE),Some(&test_proof(1))).unwrap();
        assert!(missing.targets().is_subset(&counter.targets()));
        assert!(counter.targets().contains(&Target::CurrentCounter));
        assert!(counter.validate_refusal(Some("SHADOW_MISSING_NEGATIVE_PROOF copied text")).is_err());
    }

    #[test]
    fn canonical_size_bounds_do_not_truncate_propositions_or_counter_references() {
        use kie::KieHistoryClass::ExactModelCounterproof;
        let negatives=(1..=127u8).map(|n|(negative(u64::from(n),n,"typed-model"),ExactModelCounterproof)).collect();
        let proof=classified_history(test_proof(1),negatives,vec![kie::KieScopedEvidence {
            candidate:negative(128,128,"typed-model"),class:ExactModelCounterproof,provenance:provenance() }]);
        let frontier=IzFrontier::from_observation(Some(GAP),Some(&proof)).unwrap();
        assert_eq!(frontier.model.as_ref().unwrap().counters.len(),128);
        assert_eq!(frontier.targets().iter().filter(|t|matches!(t,Target::HistoricalCounter(_))).count(),128);
        let encoded=frontier.encode().unwrap();
        assert!(encoded.len()<MAX_FRONTIER_BYTES);
        assert_eq!(IzFrontier::decode(&encoded).unwrap(),frontier);
        assert!(frontier.render().unwrap().contains("candidate_id=128"));
        let mut long=frontier.clone();
        long.model.as_mut().unwrap().proposition="x".repeat(kie::MAX_TEXT_BYTES);
        assert_eq!(IzFrontier::decode(&long.encode().unwrap()).unwrap(),long);
        assert!(long.render().unwrap().contains(&"x".repeat(kie::MAX_TEXT_BYTES)));
        long.model.as_mut().unwrap().proposition.push('x');
        assert!(long.encode().is_err());
        let mut over=frontier;
        let mut extra=over.model.as_ref().unwrap().counters.last().unwrap().clone();
        extra.source_sha256=[129;32];
        over.model.as_mut().unwrap().counters.push(extra);
        assert!(over.encode().is_err(),"an extra reference is refused, never dropped");
    }

    #[test]
    fn missing_and_counterproof_are_independent_union_targets() {
        let proof = test_proof(1);
        let missing = IzFrontier::from_observation(Some(GAP), None).unwrap();
        let both = IzFrontier::from_observation(Some(GAP), Some(&proof)).unwrap();
        assert!(missing.targets().is_subset(&both.targets()));
        assert!(both.targets().contains(&Target::Missing(4)));
        assert!(both.targets().contains(&Target::CurrentCounter));
        let text = both.render().unwrap();
        assert!(text.contains("M=1, C=1, U=0"));
        assert!(text.contains("prove shkakore ose dalluese"));
        assert!(text.contains("[C:KiC:"));
        assert!(text.contains(&proof.canonical_proposition));
        assert!(text.contains(&hex(&proof.proof_sha256)));
        assert!(text.contains("Mund ta pranosh kundershtimin"));
        assert_eq!(both.encode().unwrap(), IzFrontier::decode(&both.encode().unwrap()).unwrap().encode().unwrap());
    }

    #[test]
    fn missing_zero_equality_and_reason_strings_do_not_manufacture_counterproof() {
        for relation in [0, 2] {
            let proof = test_proof(relation);
            let frontier = IzFrontier::from_observation(Some(GAP), Some(&proof)).unwrap();
            assert!(!frontier.targets().iter().any(|t| matches!(t, Target::CurrentCounter | Target::HistoricalCounter(_))));
            assert!(frontier.targets().contains(&Target::Missing(4)));
            assert_eq!(frontier.targets().contains(&Target::Uncertainty(4)), relation==2);
            // Empty historical P/N is coverage data, not a new KiE obligation.
            assert_eq!(frontier.targets().iter().filter(|t| matches!(t, Target::Missing(_))).count(),1);
        }
        let claim = IzFrontier::from_observation(Some("KIE_MODEL_COUNTERPROOF: unbacked text"), None).unwrap();
        assert!(!claim.targets().contains(&Target::CurrentCounter));
        assert!(claim.targets().contains(&Target::Uncertainty(6)));
        let empty = IzFrontier::from_observation(None, None).unwrap();
        assert_eq!(empty.targets(), BTreeSet::from([Target::Uncertainty(1)]));
    }

    #[test]
    fn validators_rederive_observations_even_after_valid_encoding() {
        let proof = test_proof(1);
        let frontier = IzFrontier::from_observation(Some(GAP), Some(&proof)).unwrap();
        frontier.validate_refusal(Some(GAP)).unwrap(); frontier.validate_model(Some(&proof)).unwrap();
        assert_eq!(frontier.validate_refusal(Some("different reported failure")).unwrap_err(),"IZ_FRONTIER_ACTUAL_REFUSAL");
        let supported = test_proof(0);
        assert_eq!(frontier.validate_model(Some(&supported)).unwrap_err(),"IZ_FRONTIER_ACTUAL_MODEL");
        let mut changed = frontier.clone();
        changed.model.as_mut().unwrap().scope_sha256 = [9;32];
        let decoded = IzFrontier::decode(&changed.encode().unwrap()).unwrap();
        assert_eq!(decoded.validate_model(Some(&proof)).unwrap_err(),"IZ_FRONTIER_ACTUAL_MODEL");
        let mut changed = frontier.clone();
        changed.request.kind = RequestKind::CoPresence;
        let decoded = IzFrontier::decode(&changed.encode().unwrap()).unwrap();
        assert!(decoded.validate_refusal(Some(GAP)).is_err());
    }

    #[test]
    fn strict_codec_and_display_keep_exact_source_bytes() {
        let reason = "OPERATOR_EVIDENCE_UNRESOLVED: source|line\n\\n\tend";
        let frontier = IzFrontier::from_observation(Some(reason), Some(&test_proof(1))).unwrap();
        let encoded = frontier.encode().unwrap();
        for end in 0..encoded.len() { assert!(IzFrontier::decode(&encoded[..end]).is_err()); }
        let mut trailing = encoded.clone(); trailing.push(0); assert!(IzFrontier::decode(&trailing).is_err());
        for (index, value) in [(0,0), (8,2), (10,255), (11,2)] {
            let mut altered = encoded.clone(); altered[index]=value;
            assert!(IzFrontier::decode(&altered).is_err());
        }
        assert!(IzFrontier::decode(&vec![0;MAX_FRONTIER_BYTES+1]).is_err());
        assert_eq!(frontier.encode().unwrap(),encoded);
        let rendered=frontier.render().unwrap();
        assert!(!rendered.chars().any(|ch|ch=='|' || ch.is_control()));
        assert_ne!(display("|"),display(r"\u{7c}"));
        assert_ne!(display("\n"),display(r"\n"));
    }
}
