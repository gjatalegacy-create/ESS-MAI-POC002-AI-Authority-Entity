//! NEW R3 KiE proof-origin transport, subordinate to the existing GCL/ASHT/NBES.
//! KiS/KiC are dependent predicates of ONE model, never historical statistics,
//! observed facts, independent P/N sources, capabilities, or an acceptance bit.
use crate::pd_memory::{self, Hash, PdIdentity, PdLookup};
use crate::{KnowledgeSelectionRequestWire, KnowledgeSelectionResponseWire, SelectionRequester};
use sha2::{Digest, Sha256};

pub mod history;
pub use history::{KieHistoryAssessment, KieHistoryClass, KieHistoryContext, KieHistoryEntry, KieHistoryProvenance, KieScopedEvidence};

pub const MAX_WIRE_BYTES: usize = 512 * 1024;
pub const MAX_PROOF_BYTES: usize = 256 * 1024;
pub const MAX_TEXT_BYTES: usize = 8192;
pub const PREDICATE_SCALE: u32 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KieContext {
    pub identity: PdIdentity,
    pub operation_id: String,
    pub record_id: Hash,
}
impl KieContext {
    pub fn lookup(&self) -> PdLookup {
        PdLookup {
            identity: self.identity.clone(),
            operation_id: Some(self.operation_id.clone()),
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        pd_memory::encode_lookup(&self.lookup())?;
        if self.record_id == [0; 32] {
            return Err("KIE_RECORD_REQUIRED".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KieSelectionRequest {
    pub selection: KnowledgeSelectionRequestWire,
    pub context: KieContext,
    pub input_text: String,
    pub historical_response: KnowledgeSelectionResponseWire,
}
impl KieSelectionRequest {
    pub fn validate(&self) -> Result<(), String> {
        self.context.validate()?;
        crate::pd_memory::failure::relevance::project_available_bound(
            &self.selection,
            &self.historical_response,
        )?;
        if self.historical_response.hold_id.starts_with("kie-") {
            return Err("KIE_HISTORY_ROUTE_REQUIRED".into());
        }
        if !self.selection.shape_is_valid()
            || self.selection.requester != SelectionRequester::AshtQuantum
            || self.selection.trace_id != self.selection.session_id
            || self.input_text.is_empty()
            || self.input_text.len() > MAX_TEXT_BYTES
            || self.selection.parent_i0 != hex(&digest(self.input_text.as_bytes()))
        {
            return Err("KIE_REQUEST_SCOPE".into());
        }
        Ok(())
    }
}

/// Reference to an actual earlier native T1 in the same Vault. This is a
/// reported whole-cycle judgment, never a copied live receipt/capability or
/// independent model support. Raw historical material remains in that T1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KieAdjudicatedRef {
    pub identity_digest: Hash,
    pub transaction_id: Hash,
    pub request_sha256: Hash,
    pub proof_sha256: Hash,
    pub evidence_family_digest: Hash,
    pub y: u8,
    pub x: u8,
    /// Current source native receipt has no general court reason code.
    pub reason_code: Option<u8>,
}
impl KieAdjudicatedRef {
    pub fn validate(&self) -> Result<(), String> {
        if [
            self.identity_digest,
            self.transaction_id,
            self.request_sha256,
            self.proof_sha256,
            self.evidence_family_digest,
        ]
        .iter()
        .any(|v| *v == [0; 32])
            || !matches!((self.y, self.x), (0, 0) | (1, 1))
            || self.reason_code.is_some()
        {
            return Err("KIE_PRIOR_REFERENCE_SHAPE_OR_UNAVAILABLE_REASON".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KieProof {
    pub context: KieContext,
    pub query_bytes: Vec<u8>,
    pub candidate_bytes: Vec<u8>,
    pub canonical_proposition: String,
    pub historical_request: Vec<u8>,
    pub historical_response: Vec<u8>,
    /// Boolean predicate mass, not confidence or historical legacy mass.
    pub kis_mass: u32,
    pub kic_mass: u32,
    /// 0 support, 1 counter, 2 equality/unresolved (strict provider predicates).
    pub relation: u8,
    pub proof_sha256: Hash,
    pub prior_adjudication: Option<KieAdjudicatedRef>,
    /// KIP3: Shadow-owned classification of actual, scoped source records.
    /// None preserves the exact KIP2 codec and legacy gate for replay.
    pub history_assessment: Option<KieHistoryAssessment>,
}
impl KieProof {
    pub fn seal_data(mut self) -> Result<Self, String> {
        self.validate_shape()?;
        self.proof_sha256 = digest(&proof_body(&self)?);
        Ok(self)
    }
    fn validate_shape(&self) -> Result<(), String> {
        self.context.validate()?;
        if let Some(prior) = &self.prior_adjudication {
            prior.validate()?;
        }
        if self.query_bytes.is_empty()
            || self.query_bytes.len() > 16 * 1024
            || self.candidate_bytes.is_empty()
            || self.candidate_bytes.len() > 16 * 1024
            || self.canonical_proposition.is_empty()
            || self.canonical_proposition.len() > MAX_TEXT_BYTES
        {
            return Err("KIE_PROOF_LIMIT".into());
        }
        if self.historical_request.len() > 64 * 1024 || self.historical_response.len() > 128 * 1024
        {
            return Err("KIE_HISTORICAL_LIMIT".into());
        }
        if let Some(assessment)=&self.history_assessment { assessment.validate_for(self)?; }
        combined_evaluation(self)?;
        let expected = match self.relation {
            0 => (PREDICATE_SCALE, 0),
            1 => (0, PREDICATE_SCALE),
            2 => (0, 0),
            _ => return Err("KIE_RELATION".into()),
        };
        if (self.kis_mass, self.kic_mass) != expected {
            return Err("KIE_PREDICATE_MASS".into());
        }
        Ok(())
    }
    /// Data integrity only. Every authority boundary must also recompute the
    /// pinned model and resolve context against the ACTUAL Shadow Vault.
    pub fn validate(&self) -> Result<(), String> {
        self.validate_shape()?;
        if self.proof_sha256 != digest(&proof_body(self)?) {
            return Err("KIE_PROOF_DIGEST".into());
        }
        Ok(())
    }
    pub fn selection_sha256(&self) -> String {
        crate::sha256_hex(&[b"ESSMAI/KIE/SELECTION/1", &self.proof_sha256])
    }
    pub fn history_assessment_digest(&self)->Result<Option<Hash>,String> {
        self.history_assessment.as_ref().map(|h| {
            h.validate_for(self)?;
            Ok(digest(&h.encode()?))
        }).transpose()
    }
    pub fn predicate_references(&self) -> Vec<String> {
        [b"KiS".as_slice(), b"KiC".as_slice()]
            .into_iter()
            .map(|role| crate::sha256_hex(&[b"ESSMAI/KIE/PREDICATE/1", role, &self.proof_sha256]))
            .collect()
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KieSelectionResponse {
    pub hold: KnowledgeSelectionResponseWire,
    pub proof: KieProof,
}
impl KieSelectionResponse {
    pub fn validate_for(&self, request: &KieSelectionRequest) -> Result<(), String> {
        request.validate()?;
        self.proof.validate()?;
        if !self.hold.shape_is_valid_for(&request.selection)
            || self.proof.context != request.context
            || self.proof.canonical_proposition != request.input_text
            || self.proof.historical_request != crate::encode_selection_request(&request.selection)
            || self.proof.historical_response
                != crate::encode_selection_response(&request.historical_response)
            || self.hold.selection_sha256 != self.proof.selection_sha256()
            || !self.hold.positive.is_empty()
            || !self.hold.negative.is_empty()
            || !self.hold.hold_id.starts_with("kie-")
            || self.hold.request_sha256
                != crate::sha256_hex(&[
                    b"ESSMAI/SELECTION/REQUEST/V178",
                    &crate::encode_selection_request(&request.selection),
                ])
        {
            return Err("KIE_SELECTION_BINDING".into());
        }
        Ok(())
    }
}
pub fn digest(b: &[u8]) -> Hash {
    Sha256::digest(b).into()
}
pub fn hex(b: &Hash) -> String {
    pd_memory::hex(b)
}
fn blob(out: &mut Vec<u8>, b: &[u8]) -> Result<(), String> {
    if b.len() > MAX_PROOF_BYTES
        || out.len().saturating_add(b.len()).saturating_add(4) > MAX_WIRE_BYTES
    {
        return Err("KIE_WIRE_LIMIT".into());
    }
    out.extend_from_slice(&(b.len() as u32).to_le_bytes());
    out.extend_from_slice(b);
    Ok(())
}
struct Reader<'a> {
    b: &'a [u8],
    p: usize,
}
impl<'a> Reader<'a> {
    fn new(b: &'a [u8], magic: &[u8]) -> Result<Self, String> {
        if b.len() > MAX_WIRE_BYTES || !b.starts_with(magic) {
            return Err("KIE_WIRE_VERSION_LIMIT".into());
        }
        Ok(Self { b, p: magic.len() })
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.p.checked_add(n).ok_or("KIE_WIRE_OVERFLOW")?;
        let v = self.b.get(self.p..end).ok_or("KIE_WIRE_TRUNCATED")?;
        self.p = end;
        Ok(v)
    }
    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| "KIE_U32")?,
        ))
    }
    fn hash(&mut self) -> Result<Hash, String> {
        self.take(32)?.try_into().map_err(|_| "KIE_HASH".into())
    }
    fn bytes(&mut self, limit: usize) -> Result<Vec<u8>, String> {
        let n = self.u32()? as usize;
        if n > limit {
            return Err("KIE_FIELD_LIMIT".into());
        }
        Ok(self.take(n)?.to_vec())
    }
    fn text(&mut self) -> Result<String, String> {
        String::from_utf8(self.bytes(MAX_TEXT_BYTES)?).map_err(|_| "KIE_UTF8".into())
    }
    fn end(self) -> Result<(), String> {
        if self.p == self.b.len() {
            Ok(())
        } else {
            Err("KIE_TRAILING".into())
        }
    }
}
pub fn encode_context(c: &KieContext) -> Result<Vec<u8>, String> {
    c.validate()?;
    let mut b = b"KIC1".to_vec();
    blob(&mut b, &pd_memory::encode_lookup(&c.lookup())?)?;
    b.extend_from_slice(&c.record_id);
    Ok(b)
}
pub fn decode_context(b: &[u8]) -> Result<KieContext, String> {
    let mut r = Reader::new(b, b"KIC1")?;
    let q = pd_memory::decode_lookup(&r.bytes(10 * 1024)?)?;
    let c = KieContext {
        identity: q.identity,
        operation_id: q.operation_id.ok_or("KIE_EXACT_OPERATION_REQUIRED")?,
        record_id: r.hash()?,
    };
    r.end()?;
    c.validate()?;
    if encode_context(&c)? != b {
        return Err("KIE_CONTEXT_CANONICAL".into());
    }
    Ok(c)
}
pub fn encode_request(q: &KieSelectionRequest) -> Result<Vec<u8>, String> {
    q.validate()?;
    let mut b = b"KIR1".to_vec();
    blob(&mut b, &crate::encode_selection_request(&q.selection))?;
    blob(&mut b, &encode_context(&q.context)?)?;
    blob(&mut b, q.input_text.as_bytes())?;
    blob(
        &mut b,
        &crate::encode_selection_response(&q.historical_response),
    )?;
    Ok(b)
}
pub fn decode_request(b: &[u8]) -> Result<KieSelectionRequest, String> {
    let mut r = Reader::new(b, b"KIR1")?;
    let selection = crate::decode_selection_request(&r.bytes(MAX_PROOF_BYTES)?)
        .map_err(|e| format!("KIE_SELECTION:{e}"))?;
    let q = KieSelectionRequest {
        selection,
        context: decode_context(&r.bytes(16 * 1024)?)?,
        input_text: r.text()?,
        historical_response: crate::decode_selection_response(&r.bytes(128 * 1024)?)
            .map_err(|e| format!("KIE_HISTORY:{e}"))?,
    };
    r.end()?;
    q.validate()?;
    if encode_request(&q)? != b {
        return Err("KIE_REQUEST_CANONICAL".into());
    }
    Ok(q)
}
fn proof_body(p: &KieProof) -> Result<Vec<u8>, String> {
    let mut b = if p.history_assessment.is_some() {b"KIP3".to_vec()} else {b"KIP2".to_vec()};
    blob(&mut b, &encode_context(&p.context)?)?;
    blob(&mut b, &p.query_bytes)?;
    blob(&mut b, &p.candidate_bytes)?;
    blob(&mut b, p.canonical_proposition.as_bytes())?;
    blob(&mut b, &p.historical_request)?;
    blob(&mut b, &p.historical_response)?;
    b.extend_from_slice(&p.kis_mass.to_le_bytes());
    b.extend_from_slice(&p.kic_mass.to_le_bytes());
    b.push(p.relation);
    match &p.prior_adjudication {
        None => b.push(0),
        Some(v) => {
            v.validate()?;
            b.push(1);
            for h in [
                v.identity_digest,
                v.transaction_id,
                v.request_sha256,
                v.proof_sha256,
                v.evidence_family_digest,
            ] {
                b.extend_from_slice(&h);
            }
            b.extend_from_slice(&[v.y, v.x, 0]); // general source reason unavailable
        }
    }
    if let Some(assessment)=&p.history_assessment { blob(&mut b,&assessment.encode()?)?; }
    if b.len() + 32 > MAX_PROOF_BYTES {
        return Err("KIE_PROOF_LIMIT".into());
    }
    Ok(b)
}
pub fn encode_proof(p: &KieProof) -> Result<Vec<u8>, String> {
    p.validate()?;
    let mut b = proof_body(p)?;
    b.extend_from_slice(&p.proof_sha256);
    Ok(b)
}
pub fn decode_proof(b: &[u8]) -> Result<KieProof, String> {
    if b.len() > MAX_PROOF_BYTES {
        return Err("KIE_PROOF_LIMIT".into());
    }
    let classified=b.starts_with(b"KIP3");
    let mut r = Reader::new(b, if classified {b"KIP3"} else {b"KIP2"})?;
    let mut p = KieProof {
        context: decode_context(&r.bytes(16 * 1024)?)?,
        query_bytes: r.bytes(16 * 1024)?,
        candidate_bytes: r.bytes(16 * 1024)?,
        canonical_proposition: r.text()?,
        historical_request: r.bytes(64 * 1024)?,
        historical_response: r.bytes(128 * 1024)?,
        kis_mass: r.u32()?,
        kic_mass: r.u32()?,
        relation: r.take(1)?[0],
        proof_sha256: [0; 32],
        prior_adjudication: None,
        history_assessment: None,
    };
    p.prior_adjudication = match r.take(1)?[0] {
        0 => None,
        1 => {
            let identity_digest = r.hash()?;
            let transaction_id = r.hash()?;
            let request_sha256 = r.hash()?;
            let proof_sha256 = r.hash()?;
            let evidence_family_digest = r.hash()?;
            let y = r.take(1)?[0];
            let x = r.take(1)?[0];
            if r.take(1)?[0] != 0 {
                return Err("KIE_PRIOR_REASON_NOT_AVAILABLE".into());
            }
            Some(KieAdjudicatedRef {
                identity_digest,
                transaction_id,
                request_sha256,
                proof_sha256,
                evidence_family_digest,
                y,
                x,
                reason_code: None,
            })
        }
        _ => return Err("KIE_PRIOR_TAG".into()),
    };
    if classified {p.history_assessment=Some(KieHistoryAssessment::decode(&r.bytes(MAX_PROOF_BYTES)?)?);}
    p.proof_sha256 = r.hash()?;
    r.end()?;
    p.validate()?;
    if encode_proof(&p)? != b {
        return Err("KIE_PROOF_CANONICAL".into());
    }
    Ok(p)
}
pub fn encode_response(s: &KieSelectionResponse) -> Result<Vec<u8>, String> {
    s.proof.validate()?;
    let mut b = b"KIS1".to_vec();
    blob(&mut b, &crate::encode_selection_response(&s.hold))?;
    blob(&mut b, &encode_proof(&s.proof)?)?;
    Ok(b)
}
pub fn decode_response(b: &[u8]) -> Result<KieSelectionResponse, String> {
    let mut r = Reader::new(b, b"KIS1")?;
    let hold = crate::decode_selection_response(&r.bytes(MAX_PROOF_BYTES)?)
        .map_err(|e| format!("KIE_HOLD:{e}"))?;
    let s = KieSelectionResponse {
        hold,
        proof: decode_proof(&r.bytes(MAX_PROOF_BYTES)?)?,
    };
    r.end()?;
    if encode_response(&s)? != b {
        return Err("KIE_RESPONSE_CANONICAL".into());
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context() -> KieContext {
        KieContext {
            identity: PdIdentity {
                owner: "owner".into(),
                obligation: "model".into(),
                case_id: "case".into(),
                scope: "scope".into(),
                domain: "science".into(),
                lineage_root: "root".into(),
            },
            operation_id: "op".into(),
            record_id: [3; 32],
        }
    }
    #[test]
    fn context_and_proof_codec_are_bounded_and_detect_tamper() {
        let c = context();
        assert_eq!(decode_context(&encode_context(&c).unwrap()).unwrap(), c);
        let p = KieProof {
            context: c,
            query_bytes: vec![1],
            candidate_bytes: vec![2],
            canonical_proposition: "typed-model".into(),
            historical_request: test_history("typed-model").0,
            historical_response: test_history("typed-model").1,
            kis_mass: PREDICATE_SCALE,
            kic_mass: 0,
            relation: 0,
            proof_sha256: [0; 32],
            prior_adjudication: None,
            history_assessment: None,
        }
        .seal_data()
        .unwrap();
        let bytes = encode_proof(&p).unwrap();
        assert_eq!(decode_proof(&bytes).unwrap(), p);
        for n in [0, 4, bytes.len() - 1] {
            assert!(decode_proof(&bytes[..n]).is_err());
        }
        let mut bad = bytes.clone();
        let end = bad.len() - 1;
        bad[end] ^= 1;
        assert!(decode_proof(&bad).is_err());
        let mut lie = p.clone();
        lie.kic_mass = PREDICATE_SCALE;
        assert!(lie.seal_data().is_err());
        assert!(decode_proof(&vec![0; MAX_PROOF_BYTES + 1]).is_err());
        assert_ne!(p.predicate_references()[0], p.predicate_references()[1]);
    }
}

/// NEW common-filter evidence summary. The historical and KiE origins retain
/// distinct scales. A critical historical route cannot be hidden by KiS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KieCombined {
    pub historical_positive_count: usize,
    pub historical_negative_count: usize,
    pub historical_positive_mass: u64,
    pub historical_negative_mass: u64,
    pub historical_veto: bool,
    /// KIP3 candidates whose origin cannot establish relevant semantic polarity.
    /// These hold a supporting model claim unresolved; they are not counterproof.
    pub unresolved_history_count: usize,
    pub historical_relevance_sha256: String,
    pub permits_positive: bool,
    /// Boolean common-filter gate masses, not averaged evidence confidence.
    pub positive_mass: u64,
    pub negative_mass: u64,
}
/// R10: computed data classification; none of these values is an authority token.
/// Shadow/Quantum must still re-evaluate the provider and verify the actual hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KieModelOutcome {
    SupportsClaim,
    CountersClaim,
    Unresolved,
}
/// Eligibility at the KiE/history gate, never the final GCL or Shadow verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KieGateOutcome {
    PositiveEligible,
    NegativeDominant,
    Undetermined,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KieComputedOutcome {
    pub model: KieModelOutcome,
    pub gate: KieGateOutcome,
    pub combined: KieCombined,
}
/// Preserve the independent meanings of model outcome and historical veto.
/// The existing historical weights, relevance projection and critical veto are
/// unchanged. In particular, an evaluated equality is not refuting evidence.
/// This validates data integrity; it does not itself execute the scientific model.
pub fn computed_outcome(proof: &KieProof) -> Result<KieComputedOutcome, String> {
    proof.validate()?;
    let model = match proof.relation {
        0 => KieModelOutcome::SupportsClaim,
        1 => KieModelOutcome::CountersClaim,
        2 => KieModelOutcome::Unresolved,
        _ => return Err("KIE_RELATION".into()),
    };
    let combined = combined_evaluation(proof)?;
    let gate = match (model, combined.historical_veto) {
        (_, true) | (KieModelOutcome::CountersClaim, false) => KieGateOutcome::NegativeDominant,
        (KieModelOutcome::SupportsClaim, false) if combined.unresolved_history_count==0 => KieGateOutcome::PositiveEligible,
        (KieModelOutcome::SupportsClaim, false) => KieGateOutcome::Undetermined,
        (KieModelOutcome::Unresolved, false) => KieGateOutcome::Undetermined,
    };
    Ok(KieComputedOutcome { model, gate, combined })
}

/// Source-exact historical mass formula from quantum/candidate_weight.rs,
/// W_CONFIDENCE=W_LEGACY=5000, canonical bracket only (0,0)|(1,1).
fn historical_mass(c: &crate::CandidateStatisticWire) -> u64 {
    let bracket = u32::from(matches!((c.trust_x, c.security_y), (0, 0) | (1, 1)));
    u64::from(
        ((c.confidence_fixed
            .saturating_mul(5000)
            .saturating_add(c.legacy_score_fixed.saturating_mul(5000)))
            / crate::MASS_SCALE)
            .min(crate::MASS_SCALE)
            .saturating_mul(bracket),
    )
}
/// R15: the one V1 historical veto formula, used by the gate below and by the
/// stored-witness validator in `pd_failure_witness`, so the two can never drift.
/// Arithmetic unchanged; stored proofs replay against exactly this rule.
pub fn historical_veto_v1(positive_count: u64, negative_count: u64, positive_mass: u64, negative_mass: u64) -> bool {
    negative_count > 0 && (positive_count == 0 || negative_mass >= positive_mass)
}

fn scoped_candidates<'a>(
    candidates:&[crate::pd_memory::failure::relevance::RelevantCandidate<'a>],
    assessment:&'a KieHistoryAssessment,class:KieHistoryClass,
)->Result<Vec<&'a crate::CandidateStatisticWire>,String> {
    let mut unique=std::collections::BTreeMap::new();
    let lexical=candidates.iter().map(|v|v.candidate).filter(|c|
        assessment.entry_for(c).is_some_and(|entry| entry.class == class));
    let scoped=assessment.scoped_evidence.iter().filter(|e|e.class==class).map(|e|&e.candidate);
    for c in lexical.chain(scoped) {
        if let Some(previous)=unique.insert(c.source_sha256.as_str(),c) {
            // Repeated references to the same bytes are not new evidence. A
            // conflicting weight is held, never resolved by caller ordering.
            if historical_mass(previous)!=historical_mass(c) {
                return Err("KIE_DUPLICATE_SOURCE_WEIGHT_CONFLICT".into());
            }
        }
    }
    Ok(unique.into_values().collect())
}

/// Borrow the exact KIP3 semantic projection used by the common gate.
/// Conditions and unresolved origins remain in the proof, not in these lists.
/// This data view grants no authority; owners must verify actual provenance.
pub fn with_scoped_semantic_candidates<T>(
    proof: &KieProof,
    visit: impl FnOnce(&[&crate::CandidateStatisticWire], &[&crate::CandidateStatisticWire]) -> T,
) -> Result<T, String> {
    proof.validate()?;
    let Some(assessment) = &proof.history_assessment else {
        return Ok(visit(&[], &[]));
    };
    let request = crate::decode_selection_request(&proof.historical_request)
        .map_err(|e| format!("KIE_HISTORY_REQUEST:{e}"))?;
    let response = crate::decode_selection_response(&proof.historical_response)
        .map_err(|e| format!("KIE_HISTORY_RESPONSE:{e}"))?;
    let projected = crate::pd_memory::failure::relevance::project_available_bound(&request, &response)?;
    let positive = scoped_candidates(&projected.positive, assessment, KieHistoryClass::ExactModelSupport)?;
    let negative = scoped_candidates(&projected.negative, assessment, KieHistoryClass::ExactModelCounterproof)?;
    Ok(visit(&positive, &negative))
}

pub fn combined_evaluation(proof: &KieProof) -> Result<KieCombined, String> {
    let request = crate::decode_selection_request(&proof.historical_request)
        .map_err(|e| format!("KIE_HISTORY_REQUEST:{e}"))?;
    let response = crate::decode_selection_response(&proof.historical_response)
        .map_err(|e| format!("KIE_HISTORY_RESPONSE:{e}"))?;
    if request.parent_i0 != hex(&digest(proof.canonical_proposition.as_bytes()))
        || response.hold_id.starts_with("kie-")
    {
        return Err("KIE_HISTORY_CURRENT_PROPOSITION".into());
    }
    let p = crate::pd_memory::failure::relevance::project_available_bound(&request, &response)?;
    if let Some(assessment)=&proof.history_assessment {assessment.validate_for(proof)?;}
    // KIP2 deliberately retains the original all-candidate policy. KIP3 only
    // weighs semantic evidence whose actual scoped origin Shadow has resolved.
    let (positive,negative)=match &proof.history_assessment {
        None=>(p.positive.iter().map(|v|v.candidate).collect::<Vec<_>>(),p.negative.iter().map(|v|v.candidate).collect::<Vec<_>>()),
        Some(assessment)=>(scoped_candidates(&p.positive,assessment,KieHistoryClass::ExactModelSupport)?,
            scoped_candidates(&p.negative,assessment,KieHistoryClass::ExactModelCounterproof)?),
    };
    let unresolved_history_count=proof.history_assessment.as_ref().map_or(0,|assessment|
        assessment.entries.iter().filter(|e|e.class==KieHistoryClass::LegacyUnscoped).count());
    let pm = positive
        .iter()
        .map(|v| historical_mass(v))
        .fold(0u64, u64::saturating_add);
    let nm = negative
        .iter()
        .map(|v| historical_mass(v))
        .fold(0u64, u64::saturating_add);
    let veto = historical_veto_v1(positive.len() as u64, negative.len() as u64, pm, nm);
    let unresolved=unresolved_history_count>0 && !veto && proof.relation!=1;
    let positive_mass = if unresolved {0} else {u64::from(proof.kis_mass)};
    let negative_mass = if unresolved {0} else {u64::from(proof.kic_mass.max(if veto { PREDICATE_SCALE } else { 0 }))};
    Ok(KieCombined {
        historical_positive_count: positive.len(),
        historical_negative_count: negative.len(),
        historical_positive_mass: pm,
        historical_negative_mass: nm,
        historical_veto: veto,
        unresolved_history_count,
        historical_relevance_sha256: p.relevance_sha256,
        permits_positive: positive_mass > negative_mass,
        positive_mass,
        negative_mass,
    })
}

/// Data-only fixture utility scoped to crate tests; no authoritative hold issued.
#[cfg(test)]
pub(crate) fn test_history(text: &str) -> (Vec<u8>, Vec<u8>) {
    let q = crate::KnowledgeSelectionRequestWire {
        version: crate::PROTOCOL_VERSION,
        requester: crate::SelectionRequester::AshtQuantum,
        purpose: crate::SelectionPurpose::EvidenceCandidates,
        session_id: "attempt".into(),
        parent_i0: hex(&digest(text.as_bytes())),
        primitive_anchor: 1,
        trace_id: "attempt".into(),
        split_sha256: "a".repeat(64),
        xi: vec![text.into()],
        yi: vec![],
        upstream_hold_sha256: Some("b".repeat(64)),
        evidence_sha256: "c".repeat(64),
        max_positive: crate::SELECTION_MAX_CANDIDATES,
        max_negative: crate::SELECTION_MAX_CANDIDATES,
    };
    let qb = crate::encode_selection_request(&q);
    let mut s = crate::KnowledgeSelectionResponseWire {
        version: crate::PROTOCOL_VERSION,
        requester: q.requester,
        session_id: q.session_id.clone(),
        parent_i0: q.parent_i0.clone(),
        primitive_anchor: q.primitive_anchor,
        split_sha256: q.split_sha256.clone(),
        upstream_hold_sha256: q.upstream_hold_sha256.clone(),
        request_sha256: crate::sha256_hex(&[b"ESSMAI/SELECTION/REQUEST/V178", &qb]),
        selection_sha256: String::new(),
        hold_id: "historical-fixture".into(),
        hold_sha256: "d".repeat(64),
        positive: vec![],
        negative: vec![],
        law_seal: 1,
        system_laws_seal: 1,
        expires_at_ns: 1,
    };
    s.selection_sha256 = crate::pd_memory::failure::relevance::selection_digest(&s);
    (qb, crate::encode_selection_response(&s))
}

#[cfg(test)]
mod combined_tests {
    use super::*;
    fn proof() -> KieProof {
        let (q, s) = test_history("typed-model");
        KieProof {
            context: KieContext {
                identity: PdIdentity {
                    owner: "o".into(),
                    obligation: "b".into(),
                    case_id: "c".into(),
                    scope: "s".into(),
                    domain: "d".into(),
                    lineage_root: "r".into(),
                },
                operation_id: "op".into(),
                record_id: [1; 32],
            },
            query_bytes: vec![1],
            candidate_bytes: vec![2],
            canonical_proposition: "typed-model".into(),
            historical_request: q,
            historical_response: s,
            kis_mass: 10000,
            kic_mass: 0,
            relation: 0,
            proof_sha256: [0; 32],
            prior_adjudication: None,
            history_assessment: None,
        }
        .seal_data()
        .unwrap()
    }
    fn candidate(positive: bool, term: &str, mass: u32) -> crate::CandidateStatisticWire {
        crate::CandidateStatisticWire {
            candidate_id: if positive { 1 } else { 2 },
            polarity: u8::from(positive),
            domain: if positive { "primitive" } else { "negative" }.into(),
            term_sha256: crate::sha256_hex(&[
                if positive {
                    crate::pd_memory::failure::relevance::TERM_PRIMITIVE_DOMAIN
                } else {
                    crate::pd_memory::failure::relevance::TERM_NEGATIVE_DOMAIN
                },
                term.as_bytes(),
            ]),
            source_sha256: "e".repeat(64),
            security_y: 1,
            trust_x: 1,
            confidence_fixed: mass,
            frequency: 1,
            domain_count: 1,
            legacy_score_fixed: mass,
        }
    }
    fn with_history(
        mut p: KieProof,
        pos: Vec<crate::CandidateStatisticWire>,
        neg: Vec<crate::CandidateStatisticWire>,
    ) -> KieProof {
        let mut s = crate::decode_selection_response(&p.historical_response).unwrap();
        s.positive = pos;
        s.negative = neg;
        s.selection_sha256 = crate::pd_memory::failure::relevance::selection_digest(&s);
        p.historical_response = crate::encode_selection_response(&s);
        p.seal_data().unwrap()
    }
    // These are data-contract fixtures, not provider execution or native admission.
    // R21 red regression: storage provenance below is explicit synthetic DATA;
    // no caller-built assessment is sufficient for actual Shadow admission.
    #[test]
    fn r21_procedural_negative_does_not_veto_scoped_model_support() {
        let negative=candidate(false,"typed-model",10000);
        let mut p=with_history(proof(),vec![],vec![negative.clone()]);
        p.history_assessment=Some(KieHistoryAssessment{
            scope_sha256:p.context.identity.key().unwrap(),
            proposition_sha256:digest(p.canonical_proposition.as_bytes()),
            scoped_evidence:vec![],
            entries:vec![KieHistoryEntry{
                selection_context:None,
                polarity:negative.polarity,candidate_id:negative.candidate_id,
                source_sha256:negative.source_sha256,class:KieHistoryClass::ProcessFailure,
                provenance:Some(KieHistoryProvenance{transaction_id:[1;32],proof_sha256:[2;32],model_family_sha256:[3;32]}),
            }],
        });
        let p=p.seal_data().unwrap();
        let encoded=encode_proof(&p).unwrap();
        assert!(encoded.starts_with(b"KIP3"));
        assert_eq!(decode_proof(&encoded).unwrap(),p);
        assert_eq!(computed_outcome(&p).unwrap().gate,KieGateOutcome::PositiveEligible,
            "a witnessed process failure is not an arithmetic counterproof");
    }

    fn classified(mut p:KieProof,negative_class:KieHistoryClass)->KieProof {
        let q=crate::decode_selection_request(&p.historical_request).unwrap();
        let s=crate::decode_selection_response(&p.historical_response).unwrap();
        let available=crate::pd_memory::failure::relevance::project_available_bound(&q,&s).unwrap();
        let mut entries=available.positive.iter().chain(available.negative.iter()).map(|v| {
            let c=v.candidate;
            let class=if c.polarity==1 {KieHistoryClass::ExactModelSupport} else {negative_class};
            KieHistoryEntry{polarity:c.polarity,candidate_id:c.candidate_id,source_sha256:c.source_sha256.clone(),selection_context:None,class,
                provenance:if class==KieHistoryClass::LegacyUnscoped {None} else {
                    Some(KieHistoryProvenance{transaction_id:[1;32],proof_sha256:[2;32],model_family_sha256:[3;32]})},}
        }).collect::<Vec<_>>();
        entries.sort_by(|a,b|a.key().cmp(&b.key()));
        p.history_assessment=Some(KieHistoryAssessment{scope_sha256:p.context.identity.key().unwrap(),
            proposition_sha256:digest(p.canonical_proposition.as_bytes()),entries,scoped_evidence:vec![]});
        p.seal_data().unwrap()
    }

    #[test]
    fn r21_classified_gate_matrix_preserves_counterproof_and_unknown() {
        let old=with_history(proof(),vec![],vec![candidate(false,"typed-model",10000)]);
        let old_bytes=encode_proof(&old).unwrap();
        assert!(old_bytes.starts_with(b"KIP2"));
        for (class,gate,count) in [
            (KieHistoryClass::ProcessFailure,KieGateOutcome::PositiveEligible,0),
            (KieHistoryClass::DifferentScope,KieGateOutcome::PositiveEligible,0),
            (KieHistoryClass::LegacyUnscoped,KieGateOutcome::Undetermined,1),
            (KieHistoryClass::ExactModelCounterproof,KieGateOutcome::NegativeDominant,0),
        ] {
            let p=classified(old.clone(),class);
            let outcome=computed_outcome(&p).unwrap();
            assert_eq!(outcome.gate,gate);
            assert_eq!(outcome.combined.unresolved_history_count,count);
            assert_eq!((p.kis_mass,p.kic_mass),(10000,0),"history never relabels the actual model predicate");
            if count>0 {assert_eq!((outcome.combined.positive_mass,outcome.combined.negative_mass),(0,0));}
            let encoded=encode_proof(&p).unwrap();
            assert_eq!(encode_proof(&decode_proof(&encoded).unwrap()).unwrap(),encoded);
        }
        assert_eq!(encode_proof(&decode_proof(&old_bytes).unwrap()).unwrap(),old_bytes);
        assert_eq!(computed_outcome(&old).unwrap().gate,KieGateOutcome::NegativeDominant,"KIP2 replay retains its historical policy");
        let empty=classified(proof(),KieHistoryClass::LegacyUnscoped);
        assert_eq!(computed_outcome(&empty).unwrap().gate,KieGateOutcome::PositiveEligible,"first scoped support does not require a previous positive");
        for relation in [1,2] {
            let p=classified(model_variant(old.clone(),relation),KieHistoryClass::LegacyUnscoped);
            let gate=computed_outcome(&p).unwrap().gate;
            assert_eq!(gate,if relation==1 {KieGateOutcome::NegativeDominant} else {KieGateOutcome::Undetermined});
        }
        let equality=classified(model_variant(old,2),KieHistoryClass::ProcessFailure);
        let outcome=computed_outcome(&equality).unwrap();
        assert_eq!((outcome.combined.positive_mass,outcome.combined.negative_mass),(0,0));
        assert_eq!(outcome.gate,KieGateOutcome::Undetermined,"equality cannot be converted to thesis counterproof");
    }

    #[test]
    fn r21_semantic_mass_is_source_weighted_and_duplicate_sources_do_not_multiply() {
        let positive=candidate(true,"typed-model",3000);
        let negative=candidate(false,"typed-model",1000);
        let mut repeated=negative.clone();repeated.candidate_id+=1;
        let p=classified(with_history(proof(),vec![positive],vec![negative,repeated]),KieHistoryClass::ExactModelCounterproof);
        let c=combined_evaluation(&p).unwrap();
        assert_eq!((c.historical_positive_count,c.historical_negative_count),(1,1));
        assert_eq!((c.historical_positive_mass,c.historical_negative_mass),(3000,1000));
        assert!(c.permits_positive);
        let mut bad=p;
        let mut response=crate::decode_selection_response(&bad.historical_response).unwrap();
        response.negative[1].confidence_fixed=1001;
        response.negative[1].legacy_score_fixed=1001;
        response.selection_sha256=crate::pd_memory::failure::relevance::selection_digest(&response);
        bad.historical_response=crate::encode_selection_response(&response);
        assert_eq!(bad.seal_data().unwrap_err(),"KIE_DUPLICATE_SOURCE_WEIGHT_CONFLICT");
    }

    #[test]
    fn r21_assessment_is_complete_exact_scoped_and_bound_to_polarity() {
        let base=classified(with_history(proof(),vec![],vec![candidate(false,"typed-model",1)]),KieHistoryClass::ProcessFailure);
        for mutation in 0..4 {
            let mut p=base.clone();let a=p.history_assessment.as_mut().unwrap();
            match mutation {0=>a.entries.clear(),1=>a.scope_sha256[0]^=1,2=>a.proposition_sha256[0]^=1,
                3=>a.entries[0].class=KieHistoryClass::ExactModelSupport,_=>unreachable!()}
            assert!(p.seal_data().is_err());
        }
        let mut positives=Vec::new();let mut negatives=Vec::new();
        for id in 1..=64u64 {
            let mut p=candidate(true,"typed-model",1);p.candidate_id=id;p.source_sha256=format!("{:064x}",id);
            let mut n=candidate(false,"typed-model",1);n.candidate_id=id;n.source_sha256=format!("{:064x}",id+64);
            positives.push(p);negatives.push(n);
        }
        let p=classified(with_history(proof(),positives,negatives),KieHistoryClass::ProcessFailure);
        assert_eq!(p.history_assessment.as_ref().unwrap().entries.len(),128);
        let mut response=crate::decode_selection_response(&p.historical_response).unwrap();
        let mut extra=candidate(true,"typed-model",1);extra.candidate_id=65;extra.source_sha256=format!("{:064x}",129);
        response.positive.push(extra);response.selection_sha256=crate::pd_memory::failure::relevance::selection_digest(&response);
        let mut too_many=p;too_many.historical_response=crate::encode_selection_response(&response);
        assert!(too_many.seal_data().unwrap_err().contains("existing Asht directive capacity"),"129 relevant entries must reject rather than truncate");
    }

    #[test]
    fn r21_exact_proposition_history_is_consumed_without_inventing_lexical_candidates() {
        let mut p=classified(proof(),KieHistoryClass::LegacyUnscoped);
        let scoped=KieScopedEvidence{candidate:candidate(true,"typed-model",3500),class:KieHistoryClass::ExactModelSupport,
            provenance:KieHistoryProvenance{transaction_id:[6;32],proof_sha256:[7;32],model_family_sha256:[8;32]}};
        p.history_assessment.as_mut().unwrap().scoped_evidence.push(scoped.clone());
        let p=p.seal_data().unwrap();
        let raw=encode_proof(&p).unwrap();
        assert_eq!(decode_proof(&raw).unwrap(),p);
        let history=crate::decode_selection_response(&p.historical_response).unwrap();
        assert!(history.positive.is_empty() && history.negative.is_empty(),"full proposition lookup does not populate fake Xi/Yi candidates");
        let c=combined_evaluation(&p).unwrap();
        assert_eq!((c.historical_positive_count,c.historical_positive_mass),(1,3500));
        assert!(c.permits_positive);
        let mut repeated=p.clone();repeated.history_assessment.as_mut().unwrap().scoped_evidence.push(scoped.clone());
        assert!(repeated.seal_data().is_err(),"one dependent model family cannot multiply into independent support");
        let mut swapped=p;swapped.history_assessment.as_mut().unwrap().scoped_evidence[0].candidate.term_sha256="f".repeat(64);
        assert_eq!(swapped.seal_data().unwrap_err(),"KIE_SCOPED_EVIDENCE_EXACT_PROPOSITION");
        let mut duplicate=classified(with_history(proof(),vec![scoped.candidate.clone()],vec![]),KieHistoryClass::LegacyUnscoped);
        duplicate.history_assessment.as_mut().unwrap().scoped_evidence.push(scoped);
        let c=combined_evaluation(&duplicate.seal_data().unwrap()).unwrap();
        assert_eq!((c.historical_positive_count,c.historical_positive_mass),(1,3500),"the same source reached by two indexes counts once");
        let mut negative=classified(proof(),KieHistoryClass::LegacyUnscoped);
        negative.history_assessment.as_mut().unwrap().scoped_evidence.push(KieScopedEvidence{
            candidate:candidate(false,"typed-model",3500),class:KieHistoryClass::ExactModelCounterproof,
            provenance:KieHistoryProvenance{transaction_id:[6;32],proof_sha256:[7;32],model_family_sha256:[8;32]}});
        assert_eq!(computed_outcome(&negative.seal_data().unwrap()).unwrap().gate,KieGateOutcome::NegativeDominant,
            "an actual scoped historical counterproof remains effective without lexical matching");
    }

    fn model_variant(mut p: KieProof, relation: u8) -> KieProof {
        p.relation = relation;
        (p.kis_mass, p.kic_mass) = match relation {
            0 => (PREDICATE_SCALE, 0),
            1 => (0, PREDICATE_SCALE),
            2 => (0, 0),
            _ => panic!("test relation"),
        };
        p.seal_data().unwrap()
    }

    #[test]
    fn r10_model_and_history_outcomes_remain_distinct() {
        for (relation, model, gate) in [
            (0, KieModelOutcome::SupportsClaim, KieGateOutcome::PositiveEligible),
            (1, KieModelOutcome::CountersClaim, KieGateOutcome::NegativeDominant),
            (2, KieModelOutcome::Unresolved, KieGateOutcome::Undetermined),
        ] {
            let p = model_variant(proof(), relation);
            let actual = computed_outcome(&p).unwrap();
            assert_eq!((actual.model, actual.gate), (model, gate));
            assert_eq!(actual.combined, combined_evaluation(&p).unwrap());
            assert_eq!(actual.combined.permits_positive, relation == 0);
            let critical = with_history(p, vec![], vec![candidate(false, "typed-model", 6000)]);
            let blocked = computed_outcome(&critical).unwrap();
            assert_eq!(blocked.model, model); // history never rewrites the model result
            assert_eq!(blocked.gate, KieGateOutcome::NegativeDominant);
            assert!(blocked.combined.historical_veto);
            assert!(!blocked.combined.permits_positive);
        }
    }

    #[test]
    fn r10_historical_support_cannot_resolve_or_overrule_model_counterproof() {
        for relation in [1, 2] {
            let p = with_history(model_variant(proof(), relation),
                vec![candidate(true, "typed-model", 10000)], vec![]);
            let actual = computed_outcome(&p).unwrap();
            assert!(!actual.combined.historical_veto);
            assert!(!actual.combined.permits_positive);
            assert_eq!(actual.gate, if relation == 1 {
                KieGateOutcome::NegativeDominant
            } else { KieGateOutcome::Undetermined });
        }
    }

    #[test]
    fn r10_unrelated_or_repeated_history_does_not_manufacture_model_mass() {
        let unrelated = with_history(model_variant(proof(), 2), vec![],
            vec![candidate(false, "unrelated", 10000)]);
        assert_eq!(computed_outcome(&unrelated).unwrap().gate, KieGateOutcome::Undetermined);
        let mut repeated = candidate(true, "typed-model", 8000);
        repeated.frequency = 100;
        let p = with_history(proof(), vec![repeated], vec![candidate(false, "typed-model", 4000)]);
        let actual = computed_outcome(&p).unwrap();
        assert_eq!(actual.combined.historical_positive_mass, 8000);
        assert_eq!(actual.combined.historical_negative_mass, 4000);
        assert_eq!(actual.combined.positive_mass, 10000);
        assert_eq!(actual.gate, KieGateOutcome::PositiveEligible);
    }

    #[test]
    fn r10_computed_outcome_rejects_model_status_or_digest_tampering() {
        let mut altered = proof();
        altered.relation = 2; // masses and digest still claim support
        assert!(computed_outcome(&altered).is_err());
        let mut altered = model_variant(proof(), 2);
        altered.proof_sha256[0] ^= 1;
        assert!(computed_outcome(&altered).is_err());
    }

    #[test]
    fn missing_history_does_not_penalize_kis_but_relevant_criticism_is_never_lost() {
        assert!(combined_evaluation(&proof()).unwrap().permits_positive);
        let critical = with_history(proof(), vec![], vec![candidate(false, "typed-model", 6000)]);
        let c = combined_evaluation(&critical).unwrap();
        assert!(c.historical_veto);
        assert!(!c.permits_positive);
        assert_eq!(c.historical_negative_count, 1);
        let unrelated = with_history(proof(), vec![], vec![candidate(false, "unrelated", 10000)]);
        assert!(combined_evaluation(&unrelated).unwrap().permits_positive);
        let history_support = with_history(
            proof(),
            vec![candidate(true, "typed-model", 8000)],
            vec![candidate(false, "typed-model", 4000)],
        );
        let s = combined_evaluation(&history_support).unwrap();
        assert!(s.permits_positive);
        assert_eq!(s.positive_mass, 10000); // no 8000 historical/model double count
        let tie = with_history(
            proof(),
            vec![candidate(true, "typed-model", 4000)],
            vec![candidate(false, "typed-model", 4000)],
        );
        assert!(!combined_evaluation(&tie).unwrap().permits_positive);
    }
}
