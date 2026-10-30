//! POC-NEW canonical operational iZ data contract.
//! Reuses the actual source request-bound projection; no authority/capability,
//! Shadow PD engine, knowledge admission, or verified PdIzCompletion lives here.
//! Event ordering is a checked commitment; actual temporal ordering additionally
//! requires the caller sites and native execution evidence. Public hashes are not signatures.
use super::failure::{self, relevance};
use super::*;
use crate::{KnowledgeSelectionRequestWire, SelectionPurpose, SelectionRequester};
use crate::materialization::condition_measure::{ConditionMeasurement, ConditionStatus};

/// R22: the two processing requirements, not semantic truth bits or GCL Y/X.
/// Bit 0 requires an exact relevant positive; bit 1 an exact relevant negative.
pub const IZ_REQUIRED_CONDITIONS: u64 = 0b11;

/// Legacy ICX1/PIZ1: the original inquiry remains the native input.
pub const IZ_VERSION: u16 = 1;
/// ICX2/PIZ2: the actual new user turn is the native input; origin stays history.
pub const IZ_ACTUAL_INPUT_VERSION: u16 = 2;
pub const IZ_NATIVE_CONTINUITY_VERSION: u16 = 3;
pub const IZ_CHECKPOINT_PREFIX: &[u8] = b"PD_IZ_USER_ONLY_V1\0";
pub const IZ_CHECKPOINT_PREFIX_V2: &[u8] = b"PD_IZ_USER_ONLY_V2\0";
pub const IZ_CHECKPOINT_PREFIX_V3: &[u8] = b"PD_IZ_NATIVE_USER_ONLY_V3\0";

/// Dispatch only; callers must still decode and validate the complete source.
pub fn source_is_iz(bytes: &[u8]) -> bool {
    matches!(
        bytes.get(..4),
        Some(b"PIZ1") | Some(b"PIZ2") | Some(b"PIZ3")
    )
}
pub const MAX_CONTEXT_BYTES: usize = 24 * 1024;
pub const MAX_IZ_BYTES: usize = MAX_SOURCE_BYTES;
const ZERO: Hash = [0; 32];

fn sha(bytes: &[u8]) -> Hash {
    Sha256::digest(bytes).into()
}
fn nonzero(h: &Hash) -> Result<(), String> {
    if *h == ZERO {
        Err("IZ_UNBOUND_HASH".into())
    } else {
        Ok(())
    }
}
fn optional_text(s: &str) -> Result<(), String> {
    if s.is_empty() {
        Ok(())
    } else {
        text_valid(s, MAX_TEXT_BYTES)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum IzField {
    Positive = 1,
    Negative = 2,
    Distinction = 3,
    SourceReference = 4,
    Verification = 5,
}
const FIELDS: [IzField; 5] = [
    IzField::Positive,
    IzField::Negative,
    IzField::Distinction,
    IzField::SourceReference,
    IzField::Verification,
];
impl IzField {
    fn decode(n: u8) -> Result<Self, String> {
        FIELDS
            .into_iter()
            .find(|f| *f as u8 == n)
            .ok_or("IZ_FIELD".into())
    }
    fn label(self) -> &'static str {
        match self {
            Self::Positive => "materialin pozitiv mbështetës",
            Self::Negative => "materialin negativ kundërshtues",
            Self::Distinction => "dallimin konkret midis Xi dhe Yi",
            Self::SourceReference => "burimin e kontrollueshëm",
            Self::Verification => "kushtin ose provën që do ta rrëzonte dallimin",
        }
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IzAnswer {
    pub positive: String,
    pub negative: String,
    pub distinction: String,
    pub source_reference: String,
    pub verification: String,
}
impl IzAnswer {
    pub fn get(&self, field: IzField) -> &str {
        match field {
            IzField::Positive => &self.positive,
            IzField::Negative => &self.negative,
            IzField::Distinction => &self.distinction,
            IzField::SourceReference => &self.source_reference,
            IzField::Verification => &self.verification,
        }
    }
    pub fn is_empty(&self) -> bool {
        FIELDS.iter().all(|f| self.get(*f).is_empty())
    }
    fn validate(&self) -> Result<(), String> {
        for f in FIELDS {
            optional_text(self.get(f))?;
        }
        Ok(())
    }
}
/// Exact UTF-8 byte span in the actual user turn, not semantic verification.
/// Different labels may reference the same words; they are not independent evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IzAnswerSpan {
    pub field: IzField,
    pub start: u32,
    pub end: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IzParent {
    pub record_id: Hash,
    pub revision: u64,
    pub state_digest: Hash,
    pub question_sha256: Hash,
}
impl IzParent {
    pub fn from_record(record: &PdOperationRecord) -> Result<Self, String> {
        record.validate()?;
        let value = Self {
            record_id: record.record_id,
            revision: record.revision,
            state_digest: record.state_digest,
            question_sha256: sha(record.request.scoped_question.as_bytes()),
        };
        value.validate()?;
        Ok(value)
    }
    fn validate(&self) -> Result<(), String> {
        nonzero(&self.record_id)?;
        nonzero(&self.state_digest)?;
        nonzero(&self.question_sha256)?;
        if self.revision == 0 {
            return Err("IZ_PARENT_REVISION".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IzMaterialStatus {
    OperationalNoVerdict,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IzMapSnapshot {
    pub identity_key: Hash,
    pub head: IzParent,
    pub exact_record: IzParent,
    pub status: IzMaterialStatus,
}
impl IzMapSnapshot {
    pub fn validate(&self) -> Result<(), String> {
        nonzero(&self.identity_key)?;
        self.head.validate()?;
        self.exact_record.validate()?;
        if self.head != self.exact_record {
            return Err("IZ_MAP_NOT_EXACT_HEAD".into());
        }
        Ok(())
    }
}
/// The operational CAS head and the checkpoint base are distinct. A new native
/// cycle supersedes earlier operational checkpoints without erasing their records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IzCheckpointBase {
    NativeT2,
    Operational(IzParent),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IzNativeMapSnapshot {
    pub identity_key: Hash,
    pub native_anchor: crate::pd_prepared::PdContinuationLink,
    pub operational_head: Option<IzParent>,
    pub checkpoint_base: IzCheckpointBase,
}
impl IzNativeMapSnapshot {
    pub fn validate(&self) -> Result<(), String> {
        nonzero(&self.identity_key)?;
        self.native_anchor.validate()?;
        if !self.native_anchor.clock_millis
            || self.native_anchor.cognitive_session_id != hex(&self.identity_key)
        {
            return Err("IZ_NATIVE_MAP_IDENTITY_OR_CLOCK".into());
        }
        if let Some(head) = &self.operational_head {
            head.validate()?;
        }
        if let IzCheckpointBase::Operational(base) = &self.checkpoint_base {
            base.validate()?;
            if self.operational_head.as_ref() != Some(base) {
                return Err("IZ_NATIVE_MAP_BASE_NOT_HEAD".into());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IzContext {
    pub version: u16,
    pub identity: PdIdentity,
    pub operation_id: String,
    /// Exact original inquiry text, not the fresh physical attempt/anchor.
    pub origin_i0: String,
    pub user_text: String,
    pub parent: Option<IzParent>,
    pub answer: IzAnswer,
    pub map: Option<IzMapSnapshot>,
    pub native_anchor: Option<crate::pd_prepared::PdContinuationLink>,
    pub native_map: Option<IzNativeMapSnapshot>,
}
impl IzContext {
    /// i0 is actual user input. V1 deliberately retains its historical origin-input policy.
    pub fn current_i0(&self) -> &str {
        if self.version >= IZ_ACTUAL_INPUT_VERSION {
            &self.user_text
        } else {
            &self.origin_i0
        }
    }
    pub fn checkpoint_prefix(&self) -> &'static [u8] {
        match self.version {
            IZ_NATIVE_CONTINUITY_VERSION => IZ_CHECKPOINT_PREFIX_V3,
            IZ_ACTUAL_INPUT_VERSION => IZ_CHECKPOINT_PREFIX_V2,
            _ => IZ_CHECKPOINT_PREFIX,
        }
    }
    /// V2 annotations bind presence only. No inferred truth, relevance or independent observation.
    /// All occurrences, including overlapping occurrences, count toward uniqueness.
    pub fn answer_spans(&self) -> Result<Vec<IzAnswerSpan>, String> {
        if !matches!(
            self.version,
            IZ_ACTUAL_INPUT_VERSION | IZ_NATIVE_CONTINUITY_VERSION
        ) {
            return Err("IZ_LEGACY_ANSWER_SPANS_UNVERIFIED".into());
        }
        // This public helper is bounded even when called before full context validation.
        text_valid(&self.user_text, MAX_TEXT_BYTES)?;
        self.answer.validate()?;
        let mut spans = Vec::new();
        for field in FIELDS {
            let value = self.answer.get(field);
            if value.is_empty() {
                continue;
            }
            let mut positions = self
                .user_text
                .char_indices()
                .filter_map(|(at, _)| self.user_text[at..].starts_with(value).then_some(at));
            let start = positions.next().ok_or("IZ_ANSWER_NOT_IN_USER_TEXT")?;
            if positions.next().is_some() {
                return Err("IZ_ANSWER_SPAN_AMBIGUOUS".into());
            }
            let end = start
                .checked_add(value.len())
                .ok_or("IZ_ANSWER_SPAN_OVERFLOW")?;
            if !self.user_text.is_char_boundary(start) || !self.user_text.is_char_boundary(end) {
                return Err("IZ_ANSWER_SPAN_UTF8".into());
            }
            spans.push(IzAnswerSpan {
                field,
                start: u32::try_from(start).map_err(|_| "IZ_ANSWER_SPAN_OVERFLOW")?,
                end: u32::try_from(end).map_err(|_| "IZ_ANSWER_SPAN_OVERFLOW")?,
            });
        }
        Ok(spans)
    }
    pub fn validate(&self) -> Result<(), String> {
        if !matches!(
            self.version,
            IZ_VERSION | IZ_ACTUAL_INPUT_VERSION | IZ_NATIVE_CONTINUITY_VERSION
        ) {
            return Err("IZ_CONTEXT_VERSION".into());
        }
        self.identity.validate()?;
        text_valid(&self.operation_id, 1024)?;
        text_valid(&self.origin_i0, MAX_TEXT_BYTES)?;
        text_valid(&self.user_text, MAX_TEXT_BYTES)?;
        self.answer.validate()?;
        if self.version >= IZ_ACTUAL_INPUT_VERSION {
            self.answer_spans()?;
        }
        if self.identity.lineage_root != hex(&sha(self.origin_i0.as_bytes())) {
            return Err("IZ_ORIGIN_LINEAGE".into());
        }
        match &self.parent {
            Some(parent) => {
                parent.validate()?;
                if self.version == IZ_VERSION && self.answer.is_empty() {
                    return Err("IZ_REPLY_HAS_NO_SUPPLIED_FIELD".into());
                }
            }
            None => {
                if self.version != IZ_NATIVE_CONTINUITY_VERSION && self.user_text != self.origin_i0
                {
                    return Err("IZ_INITIAL_ORIGIN_INPUT".into());
                }
                if self.map.is_some() {
                    return Err("IZ_INITIAL_HAS_MAP".into());
                }
            }
        }
        if self.version == IZ_NATIVE_CONTINUITY_VERSION {
            let anchor = self
                .native_anchor
                .as_ref()
                .ok_or("IZ_NATIVE_ANCHOR_REQUIRED")?;
            anchor.validate()?;
            if !anchor.clock_millis
                || anchor.cognitive_session_id != hex(&self.identity.key()?)
                || anchor.territory != self.identity.scope
                || self.map.is_some()
            {
                return Err("IZ_NATIVE_IDENTITY_SCOPE_CLOCK".into());
            }
            if let Some(mapped) = &self.native_map {
                mapped.validate()?;
                if matches!(mapped.checkpoint_base, IzCheckpointBase::NativeT2)
                    && !self.answer.is_empty()
                {
                    return Err("IZ_NATIVE_BASE_HAS_NO_FIELD_FRONTIER".into());
                }
                if mapped.identity_key != self.identity.key()?
                    || mapped.native_anchor != *anchor
                    || mapped.operational_head != self.parent
                {
                    return Err("IZ_NATIVE_MAP_BINDING".into());
                }
            }
        } else if self.native_anchor.is_some() || self.native_map.is_some() {
            return Err("IZ_LEGACY_NATIVE_FIELDS".into());
        }
        if let Some(map) = &self.map {
            map.validate()?;
            if map.identity_key != self.identity.key()? || self.parent.as_ref() != Some(&map.head) {
                return Err("IZ_MAP_CONTEXT_BINDING".into());
            }
        }
        Ok(())
    }
    pub fn require_mapped(&self) -> Result<(), String> {
        self.validate()?;
        if self.version == IZ_NATIVE_CONTINUITY_VERSION {
            return self
                .native_map
                .as_ref()
                .map(|_| ())
                .ok_or("IZ_NATIVE_MAP_REQUIRED".into());
        }
        if self.parent.is_some() && self.map.is_none() {
            return Err("IZ_REPLY_MAP_MISSING".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IzFieldOrigin {
    CurrentOperation {
        operation_id: String,
        value_sha256: Hash,
    },
    Prior {
        record_id: Hash,
        revision: u64,
        operation_id: String,
        value_sha256: Hash,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IzFieldRef {
    pub field: IzField,
    pub origin: IzFieldOrigin,
}
fn validate_frontier(refs: &[IzFieldRef], prior_only: bool) -> Result<(), String> {
    if refs.len() > 5 {
        return Err("IZ_FRONTIER_LIMIT".into());
    }
    let mut last = 0;
    for item in refs {
        if item.field as u8 <= last {
            return Err("IZ_FRONTIER_ORDER_OR_DUPLICATE".into());
        }
        last = item.field as u8;
        match &item.origin {
            IzFieldOrigin::CurrentOperation {
                operation_id,
                value_sha256,
            } => {
                if prior_only {
                    return Err("IZ_PRIOR_FRONTIER_HAS_CURRENT".into());
                }
                text_valid(operation_id, 1024)?;
                nonzero(value_sha256)?;
            }
            IzFieldOrigin::Prior {
                record_id,
                revision,
                operation_id,
                value_sha256,
            } => {
                nonzero(record_id)?;
                nonzero(value_sha256)?;
                text_valid(operation_id, 1024)?;
                if *revision == 0 {
                    return Err("IZ_FIELD_REVISION".into());
                }
            }
        }
    }
    Ok(())
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum IzMissing {
    Positive = 1,
    Negative = 2,
    Both = 3,
}
impl IzMissing {
    pub fn condition_code(self) -> &'static str {
        match self {
            Self::Positive => "MISSING_EXACT_POSITIVE",
            Self::Negative => "MISSING_EXACT_NEGATIVE",
            Self::Both => "MISSING_EXACT_BOTH",
        }
    }
    fn label(self) -> &'static str {
        match self {
            Self::Positive => "kandidati pozitiv",
            Self::Negative => "kandidati negativ",
            Self::Both => "të dy kandidatët pozitiv dhe negativ",
        }
    }
    fn applicable(self, f: IzField) -> bool {
        match f {
            IzField::Positive => self != Self::Negative,
            IzField::Negative => self != Self::Positive,
            _ => true,
        }
    }
    fn decode(n: u8) -> Result<Self, String> {
        match n {
            1 => Ok(Self::Positive),
            2 => Ok(Self::Negative),
            3 => Ok(Self::Both),
            _ => Err("IZ_MISSING_KIND".into()),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum IzEventKind {
    Origin = 1,
    Precondition = 2,
    ActionActivation = 3,
    Outcome = 4,
    AssemblyActivation = 5,
    Assembly = 6,
}
impl IzEventKind {
    fn decode(n: u8) -> Result<Self, String> {
        match n {
            1 => Ok(Self::Origin),
            2 => Ok(Self::Precondition),
            3 => Ok(Self::ActionActivation),
            4 => Ok(Self::Outcome),
            5 => Ok(Self::AssemblyActivation),
            6 => Ok(Self::Assembly),
            _ => Err("IZ_EVENT_KIND".into()),
        }
    }
    fn names(self) -> (&'static str, &'static str) {
        match self {
            Self::Origin => ("origin", "bind actual input and explicit subject"),
            Self::Precondition => (
                "D-BAB/pre",
                "observe required exact pair before ASHT action",
            ),
            Self::ActionActivation => (
                "0-NOI/action",
                "trigger caller-selected ASHT action; authority=0",
            ),
            Self::Outcome => (
                "D-BAB/outcome",
                "bind actual ASHT response and witnessed deficit",
            ),
            Self::AssemblyActivation => (
                "0-NOI/assembly",
                "trigger exact operational question assembly; authority=0",
            ),
            Self::Assembly => (
                "PD/Operational_iZ",
                "compile deficit and supplied-field frontier; NoVerdict",
            ),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IzEvent {
    pub kind: IzEventKind,
    pub sequence: u8,
    pub identity_key: Hash,
    pub operation_id: String,
    pub attempt_id: String,
    pub authority: u8,
    pub predecessor: Hash,
    pub payload_digest: Hash,
    pub event_digest: Hash,
}
fn event(
    context: &IzContext,
    attempt: &str,
    kind: IzEventKind,
    predecessor: Hash,
    payload: Hash,
) -> Result<IzEvent, String> {
    let mut e = IzEvent {
        kind,
        sequence: kind as u8 - 1,
        identity_key: context.identity.key()?,
        operation_id: context.operation_id.clone(),
        attempt_id: attempt.into(),
        authority: 0,
        predecessor,
        payload_digest: payload,
        event_digest: ZERO,
    };
    let mut b = Vec::new();
    event_out(&mut b, &e, false)?;
    e.event_digest = hash(&[b"ESSMAI/IZ/EVENT/1", &b]);
    Ok(e)
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IzBefore {
    pub context_sha256: Hash,
    pub request: Vec<u8>,
    pub origin: IzEvent,
    pub pre_witness: IzEvent,
}
impl IzBefore {
    /// Data derivation from the already bound pre-event. No response exists at
    /// this point, so available conditions, deficits and mass remain unknown.
    pub fn condition_measurement(&self) -> Result<ConditionMeasurement, String> {
        ConditionMeasurement::awaiting(IZ_REQUIRED_CONDITIONS)
    }
    /// Must be called at the original caller before the actual ASHT attempt.
    pub fn new(context: &IzContext, request: Vec<u8>) -> Result<Self, String> {
        context.require_mapped()?;
        let parsed = request_for_context(context, &request)?;
        let context_sha256 = sha(&encode_context(context)?);
        let origin = event(
            context,
            &parsed.session_id,
            IzEventKind::Origin,
            context_sha256,
            context_sha256,
        )?;
        let payload = hash(&[b"ASHT_EXACT_POSITIVE_AND_NEGATIVE_REQUIRED", &request]);
        let pre_witness = event(
            context,
            &parsed.session_id,
            IzEventKind::Precondition,
            origin.event_digest,
            payload,
        )?;
        Ok(Self {
            context_sha256,
            request,
            origin,
            pre_witness,
        })
    }
    /// Caller invokes its real neutral trigger immediately after this activation.
    pub fn activate(self) -> Result<IzActivatedBefore, String> {
        let mut activation = self.pre_witness.clone();
        activation.kind = IzEventKind::ActionActivation;
        activation.sequence = 2;
        activation.predecessor = self.pre_witness.event_digest;
        activation.payload_digest = hash(&[b"CALLER_SELECTED_ASHT_ACTION", &self.request]);
        activation.event_digest = ZERO;
        let mut b = Vec::new();
        event_out(&mut b, &activation, false)?;
        activation.event_digest = hash(&[b"ESSMAI/IZ/EVENT/1", &b]);
        Ok(IzActivatedBefore {
            before: self,
            activation,
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IzActivatedBefore {
    pub before: IzBefore,
    pub activation: IzEvent,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IzAttempt {
    pub before: IzBefore,
    pub activation: IzEvent,
    pub response: Vec<u8>,
    pub refusal: String,
    pub missing: IzMissing,
    pub outcome: IzEvent,
}
impl IzActivatedBefore {
    /// Only after an actual reply/refusal. Generic transport errors are not missing evidence.
    pub fn complete(self, response: Vec<u8>, refusal: String) -> Result<IzAttempt, String> {
        self.before.condition_measurement()?;
        let missing = observed_missing(&self.before.request, &response, &refusal)?;
        let mut outcome = self.activation.clone();
        outcome.kind = IzEventKind::Outcome;
        outcome.sequence = 3;
        outcome.predecessor = self.activation.event_digest;
        outcome.payload_digest = hash(&[
            b"ACTUAL_ASHT_MISSING_RESULT",
            &response,
            refusal.as_bytes(),
            &[missing as u8],
        ]);
        outcome.event_digest = ZERO;
        let mut b = Vec::new();
        event_out(&mut b, &outcome, false)?;
        outcome.event_digest = hash(&[b"ESSMAI/IZ/EVENT/1", &b]);
        Ok(IzAttempt {
            before: self.before,
            activation: self.activation,
            response,
            refusal,
            missing,
            outcome,
        })
    }
}
impl IzAttempt {
    /// Existing causal/source validation authenticates the carrier; this method
    /// recomputes only its response-bound condition measurement. A later HOLD
    /// on continuation does not erase an already observed processing deficit.
    pub fn condition_measurement(&self) -> Result<ConditionMeasurement, String> {
        let missing = observed_missing(&self.before.request, &self.response, &self.refusal)?;
        match missing == self.missing {
            true => observed_condition_measurement(&self.before.request, &self.response),
            false => Err("IZ_CONDITION_MISSING_BINDING".into()),
        }
    }
}
/// Observed availability of the exact requested pair, not a GCL verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IzPairAvailable {
    pub event: IzEvent,
}
/// The successful ASHT observation is separate from the deficit-only IzAttempt.
/// Fields are historical data. Authentic authority still comes from native ASHT/NBES.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IzPassedAttempt {
    pub before: IzBefore,
    pub activation: IzEvent,
    pub response: Vec<u8>,
    pub asht_attestation_sha256: Hash,
    pub outcome: IzPairAvailable,
}
impl IzActivatedBefore {
    /// Invoke only after the original native ASHT attest returned Ok.
    /// This pure constructor proves data binding, never that a caller actually executed ASHT.
    pub fn complete_available(
        self,
        response: Vec<u8>,
        asht_attestation_sha256: Hash,
    ) -> Result<IzPassedAttempt, String> {
        nonzero(&asht_attestation_sha256)?;
        let (q, r) = failure::decode_selection_pair(&self.before.request, &response)?;
        let projection = relevance::project_request_bound(&q, &r)?;
        self.before.condition_measurement()?;
        match measure_projection(&projection)?.status() {
            ConditionStatus::Complete => {},
            _ => return Err("IZ_AVAILABLE_CONDITION_MEASUREMENT".into()),
        }
        let mut event = self.activation.clone();
        event.kind = IzEventKind::Outcome;
        event.sequence = 3;
        event.predecessor = self.activation.event_digest;
        event.payload_digest = hash(&[
            b"ACTUAL_ASHT_PAIR_AVAILABLE",
            &response,
            projection.relevance_sha256.as_bytes(),
            &asht_attestation_sha256,
        ]);
        event.event_digest = ZERO;
        let mut encoded = Vec::new();
        event_out(&mut encoded, &event, false)?;
        event.event_digest = hash(&[b"ESSMAI/IZ/EVENT/1", &encoded]);
        Ok(IzPassedAttempt {
            before: self.before,
            activation: self.activation,
            response,
            asht_attestation_sha256,
            outcome: IzPairAvailable { event },
        })
    }
}
impl IzPassedAttempt {
    /// Complete condition availability is an ASHT observation, never positive
    /// semantic acceptance or authority to skip the remaining native filters.
    pub fn condition_measurement(&self) -> Result<ConditionMeasurement, String> {
        let (q, r) = failure::decode_selection_pair(&self.before.request, &self.response)?;
        let projection = relevance::project_request_bound(&q, &r)?;
        measure_projection(&projection)
    }
    pub fn validate(&self, context: &IzContext) -> Result<(), String> {
        context.validate()?;
        if !matches!(
            context.version,
            IZ_ACTUAL_INPUT_VERSION | IZ_NATIVE_CONTINUITY_VERSION
        ) {
            return Err("IZ_PASSED_REQUIRES_ACTUAL_INPUT".into());
        }
        let expected = IzBefore::new(context, self.before.request.clone())?
            .activate()?
            .complete_available(self.response.clone(), self.asht_attestation_sha256)?;
        if *self != expected {
            return Err("IZ_PASSED_CAUSAL_BINDING".into());
        }
        Ok(())
    }
    /// Exact data link to the existing native request. Does not replace live hold checks.
    pub fn validate_for_native_request(
        &self,
        context: &IzContext,
        request: &crate::ShadowCycleRequest,
    ) -> Result<(), String> {
        self.validate(context)?;
        let (q, r) = failure::decode_selection_pair(&self.before.request, &self.response)?;
        let native = &request.quantum;
        let evidence = &native.final_evidence;
        let muscle = &evidence.muscle_evidence;
        let input_sha = hex(&sha(context.user_text.as_bytes()));
        let anchor = format!("i0-{:016x}", q.primitive_anchor);
        let expected_request_sha = crate::sha256_hex(&[
            b"ESSMAI/ASHT/REQUEST/V178",
            muscle.besa_attestation_sha256.as_bytes(),
            input_sha.as_bytes(),
        ]);
        if native.session_id != q.session_id
            || request.light.session_id != q.session_id
            || native.pd_parent_i0.as_deref() != Some(anchor.as_str())
            || native.territory != context.identity.scope
            || request.light.territory != context.identity.scope
            || context.identity.domain != context.identity.scope
            || evidence.light_input_bytes != context.user_text.as_bytes()
            || evidence.light_input_sha256 != input_sha
            || q.parent_i0 != input_sha
            || q.evidence_sha256 != expected_request_sha
            || q.split_sha256 != muscle.split_sha256
            || q.upstream_hold_sha256.as_deref() != Some(muscle.besa_hold_sha256.as_str())
            || r.hold_id != muscle.asht_hold_id
            || r.hold_sha256 != muscle.asht_hold_sha256
            || r.selection_sha256 != muscle.asht_selection_sha256
            || hex(&self.asht_attestation_sha256) != muscle.asht_attestation_sha256
            || !muscle.verifies_internal()
        {
            return Err("IZ_PASSED_NATIVE_REQUEST_BINDING".into());
        }
        Ok(())
    }
}
/// Standalone bounded data codec. Full contextual validation is performed by the origin link.
pub fn encode_passed(passed: &IzPassedAttempt) -> Result<Vec<u8>, String> {
    // Revalidate all response-dependent data without inventing a missing condition.
    let expected = IzActivatedBefore {
        before: passed.before.clone(),
        activation: passed.activation.clone(),
    }
    .complete_available(passed.response.clone(), passed.asht_attestation_sha256)?;
    if *passed != expected {
        return Err("IZ_PASSED_OUTCOME_BINDING".into());
    }
    let mut b = start(b"IPA1");
    before_out(&mut b, &passed.before)?;
    event_out(&mut b, &passed.activation, true)?;
    put_bytes(
        &mut b,
        &passed.response,
        failure::MAX_SELECTION_RESPONSE_BYTES,
    )?;
    b.extend_from_slice(&passed.asht_attestation_sha256);
    event_out(&mut b, &passed.outcome.event, true)?;
    finish(b, MAX_IZ_BYTES)
}
pub fn decode_passed(bytes: &[u8]) -> Result<IzPassedAttempt, String> {
    let mut r = frame(bytes, b"IPA1", MAX_IZ_BYTES)?;
    let passed = IzPassedAttempt {
        before: before_in(&mut r)?,
        activation: event_in(&mut r)?,
        response: r.bytes(failure::MAX_SELECTION_RESPONSE_BYTES)?,
        asht_attestation_sha256: r.hash()?,
        outcome: IzPairAvailable {
            event: event_in(&mut r)?,
        },
    };
    r.end()?;
    if encode_passed(&passed)? != bytes {
        return Err("IZ_PASSED_NONCANONICAL".into());
    }
    Ok(passed)
}
fn request_for_context(
    context: &IzContext,
    bytes: &[u8],
) -> Result<KnowledgeSelectionRequestWire, String> {
    if bytes.len() > failure::MAX_SELECTION_REQUEST_BYTES {
        return Err("IZ_REQUEST_LIMIT".into());
    }
    let q = crate::decode_selection_request(bytes).map_err(|e| e.to_string())?;
    if crate::encode_selection_request(&q) != bytes
        || !q.shape_is_valid()
        || q.requester != SelectionRequester::AshtQuantum
        || q.purpose != SelectionPurpose::EvidenceCandidates
        || q.parent_i0 != hex(&sha(context.current_i0().as_bytes()))
        || q.session_id != q.trace_id
    {
        return Err("IZ_ACTUAL_REQUEST_BINDING".into());
    }
    Ok(q)
}
fn observed_missing(request: &[u8], response: &[u8], refusal: &str) -> Result<IzMissing, String> {
    // Preserves the original first-refusal priority and exact text.
    failure::validate_missing_selection(request, response, refusal)?;
    // Same observed classification and exact refusal priority as before R22;
    // the integer measure records the condition deficit without becoming a
    // candidate mass, negative thesis, or permission to continue a failed court.
    match observed_condition_measurement(request, response)?.missing_mask() {
        Some(3) => Ok(IzMissing::Both),
        Some(1) => Ok(IzMissing::Positive),
        Some(2) => Ok(IzMissing::Negative),
        _ => Err("IZ_NO_MISSING_CONDITION".into()),
    }
}
fn measure_projection(projection: &relevance::RequestBoundProjection<'_>) -> Result<ConditionMeasurement, String> {
    let available = u64::from(!projection.positive.is_empty())
        | (u64::from(!projection.negative.is_empty()) << 1);
    ConditionMeasurement::observed(IZ_REQUIRED_CONDITIONS, available)
}
fn observed_condition_measurement(request: &[u8], response: &[u8]) -> Result<ConditionMeasurement, String> {
    let (q, r) = failure::decode_selection_pair(request, response)?;
    let projection = relevance::project_available_bound(&q, &r)?;
    measure_projection(&projection)
}
fn validate_attempt(context: &IzContext, attempt: &IzAttempt) -> Result<(), String> {
    let expected = IzBefore::new(context, attempt.before.request.clone())?
        .activate()?
        .complete(attempt.response.clone(), attempt.refusal.clone())?;
    if *attempt != expected {
        return Err("IZ_ATTEMPT_CAUSAL_BINDING".into());
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IzQuestionSpec {
    pub context_sha256: Hash,
    pub attempt_id: String,
    pub hold_id: String,
    pub missing: IzMissing,
    pub frontier: Vec<IzFieldRef>,
    pub requested_fields: Vec<IzField>,
    pub outcome_digest: Hash,
    pub hold: bool,
    pub status: IzMaterialStatus,
    pub question: String,
    pub question_sha256: Hash,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IzSource {
    pub context: IzContext,
    pub attempt: IzAttempt,
    pub prior_frontier: Vec<IzFieldRef>,
    pub question_spec: IzQuestionSpec,
    pub assembly_activation: IzEvent,
    pub assembly: IzEvent,
}
fn check_prior(context: &IzContext, prior: &[IzFieldRef]) -> Result<(), String> {
    validate_frontier(prior, true)?;
    match &context.parent {
        None if !prior.is_empty() => return Err("IZ_INITIAL_PRIOR_FRONTIER".into()),
        Some(parent) => {
            for item in prior {
                if let IzFieldOrigin::Prior {
                    revision,
                    operation_id,
                    ..
                } = &item.origin
                {
                    if *revision > parent.revision || *operation_id == context.operation_id {
                        return Err("IZ_PRIOR_FRONTIER_CAUSALITY".into());
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}
fn current_frontier(context: &IzContext, prior: &[IzFieldRef]) -> Vec<IzFieldRef> {
    FIELDS
        .iter()
        .filter_map(|field| {
            let value = context.answer.get(*field);
            if !value.is_empty() {
                Some(IzFieldRef {
                    field: *field,
                    origin: IzFieldOrigin::CurrentOperation {
                        operation_id: context.operation_id.clone(),
                        value_sha256: sha(value.as_bytes()),
                    },
                })
            } else {
                prior.iter().find(|item| item.field == *field).cloned()
            }
        })
        .collect()
}
fn spec(
    context: &IzContext,
    attempt: &IzAttempt,
    prior: &[IzFieldRef],
) -> Result<IzQuestionSpec, String> {
    let (_, response) = failure::decode_selection_pair(&attempt.before.request, &attempt.response)?;
    let frontier = current_frontier(context, prior);
    let requested_fields: Vec<_> = FIELDS
        .into_iter()
        .filter(|field| {
            attempt.missing.applicable(*field) && !frontier.iter().any(|r| r.field == *field)
        })
        .collect();
    let ask = if requested_fields.is_empty() {
        "Nuk kërkohet përsëri material i dhënë. Rivlerësimi i tanishëm ende nuk gjeti kandidatët historikë të pranuar; deklaratat e ruajtura nuk e plotësojnë vetë këtë kusht."
            .to_string()
    } else {
        format!(
            "Jep {}.",
            requested_fields
                .iter()
                .map(|f| f.label())
                .collect::<Vec<_>>()
                .join("; ")
        )
    };
    let question = if context.version == IZ_VERSION {
        // Byte-for-byte legacy wording is part of historical PIZ1 validation.
        format!(
            "Operational_iZ — Për pyetjen fillestare «{}», rastin {} dhe detyrimin {}, mungon {} i pranuar saktësisht për Xi/Yi në tentativën {}. {} Materiali i dhënë ruhet si USER_REPORTED, jo si kandidat i pranuar. HOLD vijon; kjo pyetje ka NoVerdict.",
            context.origin_i0,context.identity.case_id,context.identity.obligation,
            attempt.missing.label(),attempt.before.origin.attempt_id,ask)
    } else {
        let ask = if requested_fields.is_empty() {
            "Fushat e etiketuara janë mbledhur për këtë detyrim; ato nuk provojnë kushtin aktual. Kandidatët historikë mbeten të paplotë dhe nuk shpiket material i ri.".to_string()
        } else {
            format!("Për të ngushtuar detyrimin e provës, jep {}. Nëse nuk e ke këtë material, thuaje; mungesa nuk plotësohet automatikisht.",
                requested_fields.iter().map(|f|f.label()).collect::<Vec<_>>().join("; "))
        };
        format!(
            "Operational_iZ — Rasti {} ruan pyetjen fillestare «{}» dhe detyrimin {}. Në hyrjen reale të tanishme «{}», tentativa {} nuk gjeti {} të pranuar saktësisht për Xi/Yi e saj. {} Referencat e mbledhura ruajnë vetëm material USER_REPORTED për të njëjtin detyrim; prania e tekstit nuk provon relevancë ose vërtetësi dhe nuk plotëson vetë Xi/Yi. Kjo pyetje kërkon provën që mungon; nuk parashikon përgjigjen e ardhshme dhe nuk është i₀ i përdoruesit. HOLD vijon; NoVerdict.",
            context.identity.case_id,context.origin_i0,context.identity.obligation,
            context.current_i0(),attempt.before.origin.attempt_id,attempt.missing.label(),ask)
    };
    text_valid(&question, MAX_TEXT_BYTES)?;
    Ok(IzQuestionSpec {
        context_sha256: sha(&encode_context(context)?),
        attempt_id: attempt.before.origin.attempt_id.clone(),
        hold_id: response.hold_id,
        missing: attempt.missing,
        frontier,
        requested_fields,
        outcome_digest: attempt.outcome.event_digest,
        hold: true,
        status: IzMaterialStatus::OperationalNoVerdict,
        question_sha256: sha(question.as_bytes()),
        question,
    })
}
/// Construct immediately before the caller's actual neutral assembly trigger.
pub fn assembly_activation(
    context: &IzContext,
    attempt: &IzAttempt,
    prior: &[IzFieldRef],
) -> Result<IzEvent, String> {
    context.require_mapped()?;
    validate_attempt(context, attempt)?;
    check_prior(context, prior)?;
    let mut b = Vec::new();
    frontier_out(&mut b, prior)?;
    event(
        context,
        &attempt.before.origin.attempt_id,
        IzEventKind::AssemblyActivation,
        attempt.outcome.event_digest,
        hash(&[
            b"COMPILE_EXACT_OPERATIONAL_IZ",
            &encode_context(context)?,
            &b,
        ]),
    )
}
/// Pure compilation invoked by Quantum in production. Shadow only checks the
/// resulting public data against this contract; it does not run native PD.
pub fn compile_exact_iz(
    context: IzContext,
    attempt: IzAttempt,
    prior_frontier: Vec<IzFieldRef>,
    activation: IzEvent,
) -> Result<IzSource, String> {
    let expected = assembly_activation(&context, &attempt, &prior_frontier)?;
    if activation != expected {
        return Err("IZ_ASSEMBLY_ACTIVATION".into());
    }
    let question_spec = spec(&context, &attempt, &prior_frontier)?;
    let mut b = Vec::new();
    spec_out(&mut b, &question_spec)?;
    let assembly = event(
        &context,
        &attempt.before.origin.attempt_id,
        IzEventKind::Assembly,
        activation.event_digest,
        sha(&b),
    )?;
    Ok(IzSource {
        context,
        attempt,
        prior_frontier,
        question_spec,
        assembly_activation: activation,
        assembly,
    })
}
impl IzSource {
    pub fn validate(&self) -> Result<(), String> {
        let activation = assembly_activation(&self.context, &self.attempt, &self.prior_frontier)?;
        let expected = spec(&self.context, &self.attempt, &self.prior_frontier)?;
        let mut b = Vec::new();
        spec_out(&mut b, &expected)?;
        let assembly = event(
            &self.context,
            &self.attempt.before.origin.attempt_id,
            IzEventKind::Assembly,
            activation.event_digest,
            sha(&b),
        )?;
        if self.question_spec != expected
            || self.assembly_activation != activation
            || self.assembly != assembly
        {
            return Err("IZ_SPEC_OR_CAUSAL_ASSEMBLY".into());
        }
        Ok(())
    }
    pub fn events(&self) -> [&IzEvent; 6] {
        [
            &self.attempt.before.origin,
            &self.attempt.before.pre_witness,
            &self.attempt.activation,
            &self.attempt.outcome,
            &self.assembly_activation,
            &self.assembly,
        ]
    }
}
/// Incoming ICX1 may precede map retrieval; a supplied conflicting map is not ignored.
pub fn same_intent(incoming: &IzContext, stored: &IzContext) -> bool {
    if (incoming.map.is_some() && incoming.map != stored.map)
        || (incoming.native_map.is_some() && incoming.native_map != stored.native_map)
    {
        return false;
    }
    let mut a = incoming.clone();
    let mut b = stored.clone();
    a.map = None;
    b.map = None;
    a.native_map = None;
    b.native_map = None;
    a == b
}
/// Upgrade only current-operation field references using the actual durable
/// parent identity. Older field references remain metadata, not copied answers.
pub fn frontier_from_parent(parent: &PdOperationRecord) -> Result<Vec<IzFieldRef>, String> {
    parent.validate()?;
    let source = decode_source(&parent.request.source_input)?;
    validate_operation(&parent.request)?;
    let mut refs = source.question_spec.frontier;
    for item in &mut refs {
        if let IzFieldOrigin::CurrentOperation {
            operation_id,
            value_sha256,
        } = &item.origin
        {
            item.origin = IzFieldOrigin::Prior {
                record_id: parent.record_id,
                revision: parent.revision,
                operation_id: operation_id.clone(),
                value_sha256: *value_sha256,
            };
        }
    }
    validate_frontier(&refs, true)?;
    Ok(refs)
}
/// Decode one stored record's own versioned context. No ancestry lookup or recursive
/// parent validation; the Vault owner remains responsible for selecting the record.
pub fn context_from_record(record: &PdOperationRecord) -> Result<IzContext, String> {
    record.validate()?;
    context_from_request(&record.request)
}
fn context_from_request(request: &PdOperationRequest) -> Result<IzContext, String> {
    if super::late_refusal::source_is_late(&request.source_input) {
        let source=super::late_refusal::validate_operation(request)?;
        return super::late_refusal::original_context(&source);
    }
    if source_is_iz(&request.source_input) {
        return Ok(validate_operation(request)?.context);
    }
    if super::iz_science::source_is_science(&request.source_input) {
        return Ok(super::iz_science::validate_operation(request)?.context);
    }
    Err("IZ_RECORD_SOURCE_VERSION".into())
}
pub fn checkpoint_raw(record: &PdOperationRecord) -> Result<&[u8], String> {
    record.validate()?;
    checkpoint_request_raw(&record.request)
}
fn checkpoint_request_raw(request: &PdOperationRequest) -> Result<&[u8], String> {
    if super::late_refusal::source_is_late(&request.source_input) {
        return super::late_refusal::checkpoint_raw(request);
    }
    if source_is_iz(&request.source_input) {
        let context = validate_operation(request)?.context;
        return request
            .checkpoint
            .strip_prefix(context.checkpoint_prefix())
            .ok_or("IZ_CHECKPOINT_POLICY".into());
    }
    if super::iz_science::source_is_science(&request.source_input) {
        super::iz_science::validate_operation(request)?;
        return super::iz_science::science_checkpoint_raw(request);
    }
    Err("IZ_RECORD_SOURCE_VERSION".into())
}
/// Data projection only. The caller must supply the actual latest native anchor
/// and actual operational head under its storage consistency boundary.
pub fn native_map_for(
    identity: &PdIdentity,
    anchor: &crate::pd_prepared::PdContinuationLink,
    head: Option<&PdOperationRecord>,
) -> Result<IzNativeMapSnapshot, String> {
    identity.validate()?;
    anchor.validate()?;
    if anchor.territory != identity.scope {
        return Err("IZ_NATIVE_SCOPE".into());
    }
    let mut base = IzCheckpointBase::NativeT2;
    let operational_head = if let Some(record) = head {
        record.validate()?;
        if record.request.identity != *identity {
            return Err("IZ_NATIVE_FOREIGN_HEAD".into());
        }
        let reference = IzParent::from_record(record)?;
        if matches!(
            record.request.source_input.get(..4),
            Some(b"PIZ3") | Some(b"PDS3") | Some(b"PDS5") | Some(b"PLF1")
        ) {
            let previous = context_from_record(record)?;
            if previous.native_anchor.as_ref() == Some(anchor) {
                base = IzCheckpointBase::Operational(reference.clone());
            }
        }
        Some(reference)
    } else {
        None
    };
    let result = IzNativeMapSnapshot {
        identity_key: identity.key()?,
        native_anchor: anchor.clone(),
        operational_head,
        checkpoint_base: base,
    };
    result.validate()?;
    Ok(result)
}
pub fn validate_native_binding(
    context: &IzContext,
    bundle: &crate::pd_recovery::RecoveryBundle,
    head: Option<&PdOperationRecord>,
) -> Result<(), String> {
    context.require_mapped()?;
    if context.version != IZ_NATIVE_CONTINUITY_VERSION {
        return Err("IZ_NATIVE_VERSION_REQUIRED".into());
    }
    let actual = crate::pd_prepared::PdContinuationLink::from_bundle(bundle)?;
    if context.native_anchor.as_ref() != Some(&actual)
        || context.native_map.as_ref() != Some(&native_map_for(&context.identity, &actual, head)?)
    {
        return Err("IZ_NATIVE_ANCHOR_OR_HEAD_MISMATCH".into());
    }
    if let Some(record) = head {
        validate_context_parent(context, record)?;
    }
    Ok(())
}
/// Both native and operational heads must be resolved by Shadow. Total turns are
/// read from the selected real checkpoint, never equated with WAL revision.
pub fn validate_native_transition(
    request: &PdOperationRequest,
    bundle: &crate::pd_recovery::RecoveryBundle,
    head: Option<&PdOperationRecord>,
) -> Result<(), String> {
    request.validate()?;
    let context = context_from_request(request)?;
    validate_native_binding(&context, bundle, head)?;
    if source_is_iz(&request.source_input) {
        validate_parent(&validate_operation(request)?, head)?;
    } else {
        super::iz_science::validate_parent(
            &super::iz_science::validate_operation(request)?,
            head.ok_or("PDS3_PARENT_REQUIRED")?,
        )?;
    }
    let (native, _, _) = bundle.validated()?;
    let previous = match &context
        .native_map
        .as_ref()
        .ok_or("IZ_NATIVE_MAP_REQUIRED")?
        .checkpoint_base
    {
        IzCheckpointBase::NativeT2 => native.checkpoint.as_slice(),
        IzCheckpointBase::Operational(_) => {
            checkpoint_raw(head.ok_or("IZ_OPERATIONAL_BASE_REQUIRED")?)?
        }
    };
    let session = hex(&context.identity.key()?);
    let before = crate::pd_checkpoint_data::checkpoint_metadata(previous, &session)?;
    if before.territory != context.identity.scope {
        return Err("IZ_NATIVE_SCOPE".into());
    }
    crate::pd_checkpoint_data::validate_user_checkpoint_transition(
        checkpoint_request_raw(request)?,
        &session,
        &context.user_text,
        u64::from(before.total_turns),
        Some(previous),
    )
}

/// Returns only provenance eligible for the target context version.
/// V1 material is retained in historical records but cannot acquire V2 span provenance.
pub fn frontier_for_context(
    context: &IzContext,
    parent: &PdOperationRecord,
) -> Result<Vec<IzFieldRef>, String> {
    context.validate()?;
    let previous = context_from_record(parent)?;
    if parent.request.identity != context.identity
        || context.parent.as_ref() != Some(&IzParent::from_record(parent)?)
        || previous.origin_i0 != context.origin_i0
        || previous.operation_id == context.operation_id
    {
        return Err("IZ_EXACT_PARENT_OR_FRONTIER".into());
    }
    if super::late_refusal::source_is_late(&parent.request.source_input) {
        // A late structural question is not an ASHT missing-pair field spec.
        // The next actual user text is processed normally; no old field value
        // or fabricated selection outcome is carried into that attempt.
        if previous.version > context.version { return Err("IZ_VERSION_DOWNGRADE".into()); }
        if context.version == IZ_NATIVE_CONTINUITY_VERSION {
            let actual=native_map_for(&context.identity,
                context.native_anchor.as_ref().ok_or("IZ_NATIVE_ANCHOR_REQUIRED")?,Some(parent))?;
            if context.native_map.as_ref()!=Some(&actual) { return Err("IZ_NATIVE_MAP_BINDING".into()); }
        }
        return Ok(Vec::new());
    }
    if context.version == IZ_NATIVE_CONTINUITY_VERSION {
        let map = context
            .native_map
            .as_ref()
            .ok_or("IZ_NATIVE_MAP_REQUIRED")?;
        let actual = native_map_for(
            &context.identity,
            context
                .native_anchor
                .as_ref()
                .ok_or("IZ_NATIVE_ANCHOR_REQUIRED")?,
            Some(parent),
        )?;
        if *map != actual {
            return Err("IZ_NATIVE_MAP_BINDING".into());
        }
        return match &map.checkpoint_base {
            IzCheckpointBase::Operational(_)
                if source_is_iz(&parent.request.source_input)
                    && previous.version == IZ_NATIVE_CONTINUITY_VERSION
                    && previous.native_anchor == context.native_anchor =>
            {
                frontier_from_parent(parent)
            }
            _ => Ok(Vec::new()),
        };
    }
    match (previous.version, context.version) {
        (IZ_VERSION, IZ_ACTUAL_INPUT_VERSION) => Ok(Vec::new()),
        (a, b) if a > b => Err("IZ_VERSION_DOWNGRADE".into()),
        (a, b) if a == b => frontier_from_parent(parent),
        _ => Err("IZ_CONTEXT_VERSION".into()),
    }
}
/// Shared reply validation for both missing-condition and native-promotion paths.
/// Returns only eligible references; it does not authorize an ASHT or GCL outcome.
pub fn validate_context_parent(
    context: &IzContext,
    parent: &PdOperationRecord,
) -> Result<Vec<IzFieldRef>, String> {
    let frontier = frontier_for_context(context, parent)?;
    if super::late_refusal::source_is_late(&parent.request.source_input) {
        if !context.answer.is_empty() { return Err("IZ_LATE_REFUSAL_HAS_NO_ASHT_FIELD_OBLIGATION".into()); }
        return Ok(frontier);
    }
    let eligible_question = if context.version == IZ_NATIVE_CONTINUITY_VERSION {
        matches!(
            context.native_map.as_ref().map(|m| &m.checkpoint_base),
            Some(IzCheckpointBase::Operational(_))
        ) && source_is_iz(&parent.request.source_input)
    } else {
        true
    };
    if !eligible_question {
        if !context.answer.is_empty() {
            return Err("IZ_NO_CURRENT_FIELD_OBLIGATION".into());
        }
    } else {
        let previous = decode_source(&parent.request.source_input)?;
        if context.version >= IZ_ACTUAL_INPUT_VERSION {
            for field in FIELDS {
                if !context.answer.get(field).is_empty()
                    && !previous.question_spec.requested_fields.contains(&field)
                    && !previous
                        .question_spec
                        .frontier
                        .iter()
                        .any(|v| v.field == field)
                {
                    return Err("IZ_ANSWER_OUTSIDE_PARENT_OBLIGATION".into());
                }
            }
        }
    }
    Ok(frontier)
}
/// Resolve against the caller-supplied exact Vault parent. No global-head
/// lookup or legacy checkpoint migration is performed by this data contract.
pub fn validate_parent(
    source: &IzSource,
    parent: Option<&PdOperationRecord>,
) -> Result<(), String> {
    source.validate()?;
    match (&source.context.parent, parent) {
        (None, None) => {
            if !source.prior_frontier.is_empty() {
                return Err("IZ_INITIAL_FRONTIER".into());
            }
        }
        (Some(expected), Some(record)) => {
            record.validate()?;
            let prior = context_from_record(record)?;
            if record.request.identity != source.context.identity
                || IzParent::from_record(record)? != *expected
                || prior.origin_i0 != source.context.origin_i0
                || record.request.operation_id == source.context.operation_id
                || validate_context_parent(&source.context, record)? != source.prior_frontier
            {
                return Err("IZ_EXACT_PARENT_OR_FRONTIER".into());
            }
        }
        _ => return Err("IZ_PARENT_REQUIRED_OR_UNEXPECTED".into()),
    }
    Ok(())
}
pub fn build_lgc_steps(source: &IzSource) -> Result<Vec<LgcStep>, String> {
    source.validate()?;
    let mut predecessor = sha(&encode_source(source)?);
    let mut steps = Vec::new();
    for (index, event) in source.events().into_iter().enumerate() {
        let mut bytes = Vec::new();
        event_out(&mut bytes, event, true)?;
        let (site, action) = event.kind.names();
        let step = LgcStep::new(index as u32, predecessor, site, action, sha(&bytes))?;
        predecessor = step.step_digest;
        steps.push(step);
    }
    Ok(steps)
}
pub fn build_witness(source: &IzSource) -> Result<ConditionWitness, String> {
    let steps = build_lgc_steps(source)?;
    Ok(ConditionWitness {
        predecessor: "ASHT exact request-bound relevance".into(),
        parent_authority: "Existing GCL/ASHT; witnessed operational deficit, no new authority"
            .into(),
        failed_action: "find both exact Xi/Yi-bound positive and negative candidates".into(),
        condition_code: source.attempt.missing.condition_code().into(),
        reason: source.attempt.refusal.clone(),
        source_component: "shadow-contracts/src/pd_memory/iz.rs::observed_missing".into(),
        source_sha256: sha(include_bytes!("iz.rs")),
        attempt_trace: source.attempt.before.origin.attempt_id.clone(),
        input_sha256: sha(&encode_source(source)?),
        lgc_parent: steps.last().ok_or("IZ_LGC_EMPTY")?.step_digest,
        report_class: ReportClass::MissingConditions,
        permitted_next: NextAction::AcquireScopedEvidence,
    })
}
/// Deliberately does not call PdOperationRequest::validate: the parent generic
/// validator may invoke this function. No recursive self-validation occurs.
pub fn validate_operation(request: &PdOperationRequest) -> Result<IzSource, String> {
    let source = decode_source(&request.source_input)?;
    let expected_parent = source.context.parent.as_ref();
    let expected_revision = expected_parent.map(|p| p.revision).unwrap_or(0);
    let expected_state = expected_parent.map(|p| p.state_digest).unwrap_or(ZERO);
    // The producer-code SHA is retained evidence, not an authenticated signature.
    // Replaying a v1 record must not compare its historical producer to a later
    // build's source file hash. The full witness remains record-digest bound.
    let mut expected_witness = build_witness(&source)?;
    nonzero(&request.witness.source_sha256)?;
    expected_witness.source_sha256 = request.witness.source_sha256;
    if request.identity != source.context.identity
        || request.operation_id != source.context.operation_id
        || request.expected_revision != expected_revision
        || request.previous_state != expected_state
        || request.scoped_question != source.question_spec.question
        || request.witness != expected_witness
        || request.lgc_steps != build_lgc_steps(&source)?
        || !request.candidate.is_empty()
        || request.candidate_key != ZERO
        || request.checkpoint.len() > MAX_CHECKPOINT_BYTES
        || request.checkpoint.len() <= source.context.checkpoint_prefix().len()
        || !request
            .checkpoint
            .starts_with(source.context.checkpoint_prefix())
    {
        return Err("IZ_OPERATION_BINDING".into());
    }
    Ok(source)
}

fn finish(mut bytes: Vec<u8>, max: usize) -> Result<Vec<u8>, String> {
    if bytes.len().checked_add(32).ok_or("IZ_ENCODE_OVERFLOW")? > max {
        return Err("IZ_FRAME_LIMIT".into());
    }
    bytes.extend_from_slice(&sha(&bytes));
    Ok(bytes)
}
fn frame<'a>(bytes: &'a [u8], magic: &[u8; 4], max: usize) -> Result<Reader<'a>, String> {
    frame_version(bytes, magic, IZ_VERSION, max)
}
fn frame_version<'a>(
    bytes: &'a [u8],
    magic: &[u8; 4],
    version: u16,
    max: usize,
) -> Result<Reader<'a>, String> {
    if bytes.len() > max || bytes.len() < 38 {
        return Err("IZ_FRAME_LIMIT".into());
    }
    let (body, checksum) = bytes.split_at(bytes.len() - 32);
    if sha(body).as_slice() != checksum {
        return Err("IZ_CHECKSUM".into());
    }
    let mut r = Reader {
        bytes: body,
        pos: 0,
    };
    if r.take(4)? != magic || r.u16()? != version {
        return Err("IZ_VERSION_OR_MAGIC".into());
    }
    Ok(r)
}
fn start(magic: &[u8; 4]) -> Vec<u8> {
    start_version(magic, IZ_VERSION)
}
fn start_version(magic: &[u8; 4], version: u16) -> Vec<u8> {
    let mut b = magic.to_vec();
    b.extend_from_slice(&version.to_le_bytes());
    b
}
fn parent_out(b: &mut Vec<u8>, p: &IzParent) {
    b.extend_from_slice(&p.record_id);
    b.extend_from_slice(&p.revision.to_le_bytes());
    b.extend_from_slice(&p.state_digest);
    b.extend_from_slice(&p.question_sha256);
}
fn parent_in(r: &mut Reader<'_>) -> Result<IzParent, String> {
    Ok(IzParent {
        record_id: r.hash()?,
        revision: r.u64()?,
        state_digest: r.hash()?,
        question_sha256: r.hash()?,
    })
}
fn status_out(b: &mut Vec<u8>, _: IzMaterialStatus) {
    b.push(1);
}
fn status_in(r: &mut Reader<'_>) -> Result<IzMaterialStatus, String> {
    match r.u8()? {
        1 => Ok(IzMaterialStatus::OperationalNoVerdict),
        _ => Err("IZ_STATUS_NOT_OPERATIONAL".into()),
    }
}
fn map_out(b: &mut Vec<u8>, m: &IzMapSnapshot) {
    b.extend_from_slice(&m.identity_key);
    parent_out(b, &m.head);
    parent_out(b, &m.exact_record);
    status_out(b, m.status);
}
fn map_in(r: &mut Reader<'_>) -> Result<IzMapSnapshot, String> {
    Ok(IzMapSnapshot {
        identity_key: r.hash()?,
        head: parent_in(r)?,
        exact_record: parent_in(r)?,
        status: status_in(r)?,
    })
}
pub fn encode_map_snapshot(map: Option<&IzMapSnapshot>) -> Result<Vec<u8>, String> {
    let mut b = start(b"IMS1");
    match map {
        None => b.push(0),
        Some(m) => {
            m.validate()?;
            b.push(1);
            map_out(&mut b, m);
        }
    }
    finish(b, 1024)
}
pub fn decode_map_snapshot(bytes: &[u8]) -> Result<Option<IzMapSnapshot>, String> {
    let mut r = frame(bytes, b"IMS1", 1024)?;
    let m = match r.u8()? {
        0 => None,
        1 => Some(map_in(&mut r)?),
        _ => return Err("IZ_OPTION_TAG".into()),
    };
    r.end()?;
    if let Some(v) = &m {
        v.validate()?;
    }
    Ok(m)
}
fn continuation_out(
    b: &mut Vec<u8>,
    a: &crate::pd_prepared::PdContinuationLink,
) -> Result<(), String> {
    a.validate()?;
    put_text(b, &a.cognitive_session_id)?;
    put_text(b, &a.territory)?;
    b.extend_from_slice(&a.predecessor.identity);
    b.extend_from_slice(&a.predecessor.request_sha256);
    b.extend_from_slice(&a.predecessor_transaction_id);
    b.extend_from_slice(&a.predecessor_continuation_sha256);
    put_text(b, &a.predecessor_next_i0)?;
    b.push(u8::from(a.clock_millis));
    Ok(())
}
fn continuation_in(r: &mut Reader<'_>) -> Result<crate::pd_prepared::PdContinuationLink, String> {
    let cognitive_session_id = r.text()?;
    let territory = r.text()?;
    let predecessor = crate::pd_recovery::RecoveryLookup {
        identity: r.hash()?,
        request_sha256: r.hash()?,
    };
    let predecessor_transaction_id = r.hash()?;
    let predecessor_continuation_sha256 = r.hash()?;
    let predecessor_next_i0 = r.text()?;
    let clock_millis = match r.u8()? {
        0 => false,
        1 => true,
        _ => return Err("IZ_BOOL".into()),
    };
    let value = crate::pd_prepared::PdContinuationLink {
        cognitive_session_id,
        territory,
        predecessor,
        predecessor_transaction_id,
        predecessor_continuation_sha256,
        predecessor_next_i0,
        clock_millis,
    };
    value.validate()?;
    Ok(value)
}
fn native_map_out(b: &mut Vec<u8>, map: &IzNativeMapSnapshot) -> Result<(), String> {
    map.validate()?;
    b.extend_from_slice(&map.identity_key);
    continuation_out(b, &map.native_anchor)?;
    match &map.operational_head {
        None => b.push(0),
        Some(p) => {
            b.push(1);
            parent_out(b, p);
        }
    }
    match &map.checkpoint_base {
        IzCheckpointBase::NativeT2 => b.push(0),
        IzCheckpointBase::Operational(p) => {
            b.push(1);
            parent_out(b, p);
        }
    }
    Ok(())
}
fn native_map_in(r: &mut Reader<'_>) -> Result<IzNativeMapSnapshot, String> {
    let identity_key = r.hash()?;
    let native_anchor = continuation_in(r)?;
    let operational_head = match r.u8()? {
        0 => None,
        1 => Some(parent_in(r)?),
        _ => return Err("IZ_OPTION_TAG".into()),
    };
    let checkpoint_base = match r.u8()? {
        0 => IzCheckpointBase::NativeT2,
        1 => IzCheckpointBase::Operational(parent_in(r)?),
        _ => return Err("IZ_BASE_TAG".into()),
    };
    let value = IzNativeMapSnapshot {
        identity_key,
        native_anchor,
        operational_head,
        checkpoint_base,
    };
    value.validate()?;
    Ok(value)
}
pub fn encode_native_map_snapshot(map: &IzNativeMapSnapshot) -> Result<Vec<u8>, String> {
    let mut bytes = start_version(b"INM3", IZ_NATIVE_CONTINUITY_VERSION);
    native_map_out(&mut bytes, map)?;
    finish(bytes, MAX_CONTEXT_BYTES)
}
pub fn decode_native_map_snapshot(bytes: &[u8]) -> Result<IzNativeMapSnapshot, String> {
    let mut r = frame_version(
        bytes,
        b"INM3",
        IZ_NATIVE_CONTINUITY_VERSION,
        MAX_CONTEXT_BYTES,
    )?;
    let map = native_map_in(&mut r)?;
    r.end()?;
    Ok(map)
}

pub fn encode_context(context: &IzContext) -> Result<Vec<u8>, String> {
    context.validate()?;
    let magic = match context.version {
        IZ_VERSION => b"ICX1",
        IZ_ACTUAL_INPUT_VERSION => b"ICX2",
        _ => b"ICX3",
    };
    let mut b = start_version(magic, context.version);
    identity_out(&mut b, &context.identity)?;
    put_text(&mut b, &context.operation_id)?;
    put_text(&mut b, &context.origin_i0)?;
    put_text(&mut b, &context.user_text)?;
    match &context.parent {
        None => b.push(0),
        Some(p) => {
            b.push(1);
            parent_out(&mut b, p);
        }
    }
    for field in FIELDS {
        put_text(&mut b, context.answer.get(field))?;
    }
    match &context.map {
        None => b.push(0),
        Some(m) => {
            b.push(1);
            map_out(&mut b, m);
        }
    }
    if context.version == IZ_NATIVE_CONTINUITY_VERSION {
        continuation_out(
            &mut b,
            context
                .native_anchor
                .as_ref()
                .ok_or("IZ_NATIVE_ANCHOR_REQUIRED")?,
        )?;
        match &context.native_map {
            None => b.push(0),
            Some(map) => {
                b.push(1);
                native_map_out(&mut b, map)?;
            }
        }
    }
    finish(b, MAX_CONTEXT_BYTES)
}
pub fn decode_context(bytes: &[u8]) -> Result<IzContext, String> {
    let (magic, version) = match bytes.get(..4) {
        Some(b"ICX1") => (b"ICX1", IZ_VERSION),
        Some(b"ICX2") => (b"ICX2", IZ_ACTUAL_INPUT_VERSION),
        Some(b"ICX3") => (b"ICX3", IZ_NATIVE_CONTINUITY_VERSION),
        _ => return Err("IZ_VERSION_OR_MAGIC".into()),
    };
    let mut r = frame_version(bytes, magic, version, MAX_CONTEXT_BYTES)?;
    let identity = identity_in(&mut r)?;
    let operation_id = r.text()?;
    let origin_i0 = r.text()?;
    let user_text = r.text()?;
    let parent = match r.u8()? {
        0 => None,
        1 => Some(parent_in(&mut r)?),
        _ => return Err("IZ_OPTION_TAG".into()),
    };
    let answer = IzAnswer {
        positive: r.text()?,
        negative: r.text()?,
        distinction: r.text()?,
        source_reference: r.text()?,
        verification: r.text()?,
    };
    let map = match r.u8()? {
        0 => None,
        1 => Some(map_in(&mut r)?),
        _ => return Err("IZ_OPTION_TAG".into()),
    };
    let (native_anchor, native_map) = if version == IZ_NATIVE_CONTINUITY_VERSION {
        let anchor = continuation_in(&mut r)?;
        let map = match r.u8()? {
            0 => None,
            1 => Some(native_map_in(&mut r)?),
            _ => return Err("IZ_OPTION_TAG".into()),
        };
        (Some(anchor), map)
    } else {
        (None, None)
    };
    r.end()?;
    let c = IzContext {
        version,
        identity,
        operation_id,
        origin_i0,
        user_text,
        parent,
        answer,
        map,
        native_anchor,
        native_map,
    };
    c.validate()?;
    Ok(c)
}
fn frontier_out(b: &mut Vec<u8>, refs: &[IzFieldRef]) -> Result<(), String> {
    validate_frontier(refs, false)?;
    b.push(refs.len() as u8);
    for item in refs {
        b.push(item.field as u8);
        match &item.origin {
            IzFieldOrigin::CurrentOperation {
                operation_id,
                value_sha256,
            } => {
                b.push(1);
                put_text(b, operation_id)?;
                b.extend_from_slice(value_sha256);
            }
            IzFieldOrigin::Prior {
                record_id,
                revision,
                operation_id,
                value_sha256,
            } => {
                b.push(2);
                b.extend_from_slice(record_id);
                b.extend_from_slice(&revision.to_le_bytes());
                put_text(b, operation_id)?;
                b.extend_from_slice(value_sha256);
            }
        }
    }
    Ok(())
}
fn frontier_in(r: &mut Reader<'_>) -> Result<Vec<IzFieldRef>, String> {
    let n = r.u8()? as usize;
    if n > 5 {
        return Err("IZ_FRONTIER_LIMIT".into());
    }
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        let field = IzField::decode(r.u8()?)?;
        let origin = match r.u8()? {
            1 => IzFieldOrigin::CurrentOperation {
                operation_id: r.text()?,
                value_sha256: r.hash()?,
            },
            2 => IzFieldOrigin::Prior {
                record_id: r.hash()?,
                revision: r.u64()?,
                operation_id: r.text()?,
                value_sha256: r.hash()?,
            },
            _ => return Err("IZ_FIELD_ORIGIN".into()),
        };
        out.push(IzFieldRef { field, origin });
    }
    validate_frontier(&out, false)?;
    Ok(out)
}
fn event_out(b: &mut Vec<u8>, e: &IzEvent, with_digest: bool) -> Result<(), String> {
    b.push(e.kind as u8);
    b.push(e.sequence);
    b.extend_from_slice(&e.identity_key);
    put_text(b, &e.operation_id)?;
    put_text(b, &e.attempt_id)?;
    b.push(e.authority);
    b.extend_from_slice(&e.predecessor);
    b.extend_from_slice(&e.payload_digest);
    if with_digest {
        b.extend_from_slice(&e.event_digest);
    }
    Ok(())
}
fn event_in(r: &mut Reader<'_>) -> Result<IzEvent, String> {
    Ok(IzEvent {
        kind: IzEventKind::decode(r.u8()?)?,
        sequence: r.u8()?,
        identity_key: r.hash()?,
        operation_id: r.text()?,
        attempt_id: r.text()?,
        authority: r.u8()?,
        predecessor: r.hash()?,
        payload_digest: r.hash()?,
        event_digest: r.hash()?,
    })
}
fn before_out(b: &mut Vec<u8>, v: &IzBefore) -> Result<(), String> {
    b.extend_from_slice(&v.context_sha256);
    put_bytes(b, &v.request, failure::MAX_SELECTION_REQUEST_BYTES)?;
    event_out(b, &v.origin, true)?;
    event_out(b, &v.pre_witness, true)
}
fn before_in(r: &mut Reader<'_>) -> Result<IzBefore, String> {
    Ok(IzBefore {
        context_sha256: r.hash()?,
        request: r.bytes(failure::MAX_SELECTION_REQUEST_BYTES)?,
        origin: event_in(r)?,
        pre_witness: event_in(r)?,
    })
}
fn attempt_out(b: &mut Vec<u8>, a: &IzAttempt) -> Result<(), String> {
    before_out(b, &a.before)?;
    event_out(b, &a.activation, true)?;
    put_bytes(b, &a.response, failure::MAX_SELECTION_RESPONSE_BYTES)?;
    put_text(b, &a.refusal)?;
    b.push(a.missing as u8);
    event_out(b, &a.outcome, true)
}
fn attempt_in(r: &mut Reader<'_>) -> Result<IzAttempt, String> {
    Ok(IzAttempt {
        before: before_in(r)?,
        activation: event_in(r)?,
        response: r.bytes(failure::MAX_SELECTION_RESPONSE_BYTES)?,
        refusal: r.text()?,
        missing: IzMissing::decode(r.u8()?)?,
        outcome: event_in(r)?,
    })
}
fn spec_out(b: &mut Vec<u8>, s: &IzQuestionSpec) -> Result<(), String> {
    b.extend_from_slice(&s.context_sha256);
    put_text(b, &s.attempt_id)?;
    put_text(b, &s.hold_id)?;
    b.push(s.missing as u8);
    frontier_out(b, &s.frontier)?;
    if s.requested_fields.len() > 5 {
        return Err("IZ_REQUESTED_FIELDS_LIMIT".into());
    }
    b.push(s.requested_fields.len() as u8);
    for f in &s.requested_fields {
        b.push(*f as u8);
    }
    b.extend_from_slice(&s.outcome_digest);
    b.push(u8::from(s.hold));
    status_out(b, s.status);
    put_text(b, &s.question)?;
    b.extend_from_slice(&s.question_sha256);
    Ok(())
}
fn spec_in(r: &mut Reader<'_>) -> Result<IzQuestionSpec, String> {
    let context_sha256 = r.hash()?;
    let attempt_id = r.text()?;
    let hold_id = r.text()?;
    let missing = IzMissing::decode(r.u8()?)?;
    let frontier = frontier_in(r)?;
    let n = r.u8()? as usize;
    if n > 5 {
        return Err("IZ_REQUESTED_FIELDS_LIMIT".into());
    }
    let mut requested_fields = Vec::with_capacity(n);
    for _ in 0..n {
        requested_fields.push(IzField::decode(r.u8()?)?);
    }
    let outcome_digest = r.hash()?;
    let hold = match r.u8()? {
        0 => false,
        1 => true,
        _ => return Err("IZ_BOOL".into()),
    };
    let status = status_in(r)?;
    let question = r.text()?;
    let question_sha256 = r.hash()?;
    Ok(IzQuestionSpec {
        context_sha256,
        attempt_id,
        hold_id,
        missing,
        frontier,
        requested_fields,
        outcome_digest,
        hold,
        status,
        question,
        question_sha256,
    })
}
pub fn encode_source(source: &IzSource) -> Result<Vec<u8>, String> {
    source.validate()?;
    let magic = match source.context.version {
        IZ_VERSION => b"PIZ1",
        IZ_ACTUAL_INPUT_VERSION => b"PIZ2",
        _ => b"PIZ3",
    };
    let mut b = start_version(magic, source.context.version);
    put_bytes(&mut b, &encode_context(&source.context)?, MAX_CONTEXT_BYTES)?;
    attempt_out(&mut b, &source.attempt)?;
    frontier_out(&mut b, &source.prior_frontier)?;
    spec_out(&mut b, &source.question_spec)?;
    event_out(&mut b, &source.assembly_activation, true)?;
    event_out(&mut b, &source.assembly, true)?;
    finish(b, MAX_IZ_BYTES)
}
pub fn decode_source(bytes: &[u8]) -> Result<IzSource, String> {
    let (magic, version) = match bytes.get(..4) {
        Some(b"PIZ1") => (b"PIZ1", IZ_VERSION),
        Some(b"PIZ2") => (b"PIZ2", IZ_ACTUAL_INPUT_VERSION),
        Some(b"PIZ3") => (b"PIZ3", IZ_NATIVE_CONTINUITY_VERSION),
        _ => return Err("IZ_VERSION_OR_MAGIC".into()),
    };
    let mut r = frame_version(bytes, magic, version, MAX_IZ_BYTES)?;
    let source = IzSource {
        context: decode_context(&r.bytes(MAX_CONTEXT_BYTES)?)?,
        attempt: attempt_in(&mut r)?,
        prior_frontier: frontier_in(&mut r)?,
        question_spec: spec_in(&mut r)?,
        assembly_activation: event_in(&mut r)?,
        assembly: event_in(&mut r)?,
    };
    r.end()?;
    if source.context.version != version {
        return Err("IZ_SOURCE_CONTEXT_VERSION".into());
    }
    source.validate()?;
    Ok(source)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CandidateStatisticWire, KnowledgeSelectionResponseWire, PROTOCOL_VERSION,
        SELECTION_MAX_CANDIDATES,
    };

    // SYNTHETIC CONTRACT DATA ONLY: these shapes are not an executed GCL
    // command, authentic Shadow hold, valid PD checkpoint, or knowledge receipt.
    fn context() -> IzContext {
        let origin = "Si dallohet alpha nga beta?".to_string();
        IzContext {
            version: IZ_VERSION,
            identity: PdIdentity {
                owner: "owner".into(),
                obligation: "exact distinction".into(),
                case_id: "stable-topic".into(),
                scope: "scope".into(),
                domain: "physics".into(),
                lineage_root: hex(&sha(origin.as_bytes())),
            },
            operation_id: "initial".into(),
            origin_i0: origin.clone(),
            user_text: origin,
            parent: None,
            answer: IzAnswer::default(),
            map: None,
            native_anchor: None,
            native_map: None,
        }
    }
    fn pair(c: &IzContext, positive: bool, negative: bool) -> (Vec<u8>, Vec<u8>, String) {
        let q = KnowledgeSelectionRequestWire {
            version: PROTOCOL_VERSION,
            requester: SelectionRequester::AshtQuantum,
            purpose: SelectionPurpose::EvidenceCandidates,
            session_id: format!("attempt-{}", c.operation_id),
            parent_i0: hex(&sha(c.current_i0().as_bytes())),
            primitive_anchor: 7,
            trace_id: format!("attempt-{}", c.operation_id),
            split_sha256: "11".repeat(32),
            xi: vec!["alpha".into()],
            yi: vec!["beta".into()],
            upstream_hold_sha256: Some("22".repeat(32)),
            evidence_sha256: "33".repeat(32),
            max_positive: SELECTION_MAX_CANDIDATES,
            max_negative: SELECTION_MAX_CANDIDATES,
        };
        let bytes = crate::encode_selection_request(&q);
        let candidate = |pos: bool| CandidateStatisticWire {
            candidate_id: if pos { 1 } else { 2 },
            polarity: u8::from(pos),
            domain: if pos { "primitive" } else { "negative-process" }.into(),
            term_sha256: crate::sha256_hex(&[
                if pos {
                    relevance::TERM_PRIMITIVE_DOMAIN
                } else {
                    relevance::TERM_NEGATIVE_DOMAIN
                },
                if pos { b"alpha" } else { b"beta" },
            ]),
            source_sha256: "55".repeat(32),
            security_y: u8::from(pos),
            trust_x: u8::from(pos),
            confidence_fixed: 7500,
            frequency: 2,
            domain_count: 1,
            legacy_score_fixed: 5000,
        };
        let mut response = KnowledgeSelectionResponseWire {
            version: PROTOCOL_VERSION,
            requester: q.requester,
            session_id: q.session_id.clone(),
            parent_i0: q.parent_i0.clone(),
            primitive_anchor: q.primitive_anchor,
            split_sha256: q.split_sha256.clone(),
            upstream_hold_sha256: q.upstream_hold_sha256.clone(),
            request_sha256: crate::sha256_hex(&[b"ESSMAI/SELECTION/REQUEST/V178", &bytes]),
            selection_sha256: String::new(),
            hold_id: "synthetic-hold".into(),
            hold_sha256: "44".repeat(32),
            positive: if positive {
                vec![candidate(true)]
            } else {
                Vec::new()
            },
            negative: if negative {
                vec![candidate(false)]
            } else {
                Vec::new()
            },
            law_seal: 1,
            system_laws_seal: 2,
            expires_at_ns: 3,
        };
        response.selection_sha256 = relevance::selection_digest(&response);
        let refusal = if positive {
            failure::MISSING_NEGATIVE
        } else {
            failure::MISSING_POSITIVE
        }
        .to_string();
        (bytes, crate::encode_selection_response(&response), refusal)
    }
    fn source(c: IzContext, positive: bool, negative: bool, prior: Vec<IzFieldRef>) -> IzSource {
        let (q, r, reason) = pair(&c, positive, negative);
        let before = IzBefore::new(&c, q).unwrap();
        let activated = before.activate().unwrap();
        let attempted = activated.complete(r, reason).unwrap();
        let activation = assembly_activation(&c, &attempted, &prior).unwrap();
        compile_exact_iz(c, attempted, prior, activation).unwrap()
    }
    fn request(source: &IzSource) -> PdOperationRequest {
        let mut checkpoint = source.context.checkpoint_prefix().to_vec();
        checkpoint.extend_from_slice(b"synthetic-data-not-a-native-checkpoint");
        PdOperationRequest {
            identity: source.context.identity.clone(),
            operation_id: source.context.operation_id.clone(),
            source_input: encode_source(source).unwrap(),
            expected_revision: source
                .context
                .parent
                .as_ref()
                .map(|p| p.revision)
                .unwrap_or(0),
            previous_state: source
                .context
                .parent
                .as_ref()
                .map(|p| p.state_digest)
                .unwrap_or(ZERO),
            witness: build_witness(source).unwrap(),
            checkpoint,
            scoped_question: source.question_spec.question.clone(),
            candidate: Vec::new(),
            candidate_key: ZERO,
            lgc_steps: build_lgc_steps(source).unwrap(),
        }
    }
    fn record(source: &IzSource) -> PdOperationRecord {
        PdOperationRecord::construct(
            request(source),
            source
                .context
                .parent
                .as_ref()
                .map(|p| p.record_id)
                .unwrap_or(ZERO),
        )
        .unwrap()
    }
    fn reply(parent: &PdOperationRecord, operation: &str, answer: IzAnswer) -> IzContext {
        let mut c = decode_source(&parent.request.source_input).unwrap().context;
        c.operation_id = operation.into();
        c.user_text = FIELDS
            .iter()
            .map(|f| answer.get(*f))
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        c.answer = answer;
        c.parent = Some(IzParent::from_record(parent).unwrap());
        c.map = Some(IzMapSnapshot {
            identity_key: c.identity.key().unwrap(),
            head: c.parent.clone().unwrap(),
            exact_record: c.parent.clone().unwrap(),
            status: IzMaterialStatus::OperationalNoVerdict,
        });
        c
    }

    // R22 new-property tests. These extend the synthetic contract fixture;
    // they do not claim a previously failing native authority or new admission.
    #[test]
    fn r22_piz_before_is_unknown_and_actual_reply_measures_the_exact_pair_deficit() {
        for (positive, negative, available, missing, count, mass) in [
            (false, false, 0, 3, 2, 10_000),
            (false, true, 2, 1, 1, 5_000),
            (true, false, 1, 2, 1, 5_000),
        ] {
            let s = source(context(), positive, negative, vec![]);
            let before = s.attempt.before.condition_measurement().unwrap();
            assert_eq!(before.status(), ConditionStatus::Awaiting);
            assert_eq!(before.available_mask(), None);
            assert_eq!(before.missing_mask(), None);
            assert_eq!(before.condition_mass_fixed(), None);
            assert!(!before.is_negative_condition_knowledge());
            let observed = s.attempt.condition_measurement().unwrap();
            assert_eq!(observed.status(), ConditionStatus::FailedMissing);
            assert_eq!(observed.required_mask(), 3);
            assert_eq!(observed.available_mask(), Some(available));
            assert_eq!(observed.missing_mask(), Some(missing));
            assert_eq!(observed.gap_count(), Some(count));
            assert_eq!(observed.condition_mass_fixed(), Some(mass));
            assert!(observed.is_negative_condition_knowledge());
            assert!(s.question_spec.hold); // a known deficit can still hold continuation
            assert!(request(&s).candidate.is_empty());
            assert!(s.events().iter().all(|event| event.authority == 0));
        }
    }

    #[test]
    fn r22_complete_pair_reports_zero_deficit_without_granting_semantic_authority() {
        let mut c = context();
        c.version = IZ_ACTUAL_INPUT_VERSION;
        let (q, r, _) = pair(&c, true, true);
        let passed = IzBefore::new(&c, q).unwrap().activate().unwrap()
            .complete_available(r, [0x42; 32]).unwrap();
        passed.validate(&c).unwrap();
        let observed = passed.condition_measurement().unwrap();
        assert_eq!(observed.status(), ConditionStatus::Complete);
        assert_eq!(observed.available_mask(), Some(3));
        assert_eq!(observed.missing_mask(), Some(0));
        assert_eq!(observed.gap_count(), Some(0));
        assert_eq!(observed.condition_mass_fixed(), Some(0));
        assert!(!observed.is_negative_condition_knowledge());
        assert_eq!(passed.outcome.event.authority, 0);
        let saved = encode_passed(&passed).unwrap();
        assert_eq!(encode_passed(&decode_passed(&saved).unwrap()).unwrap(), saved);
    }

    #[test]
    fn r22_deficit_requires_actual_complete_relevance_not_raw_counts_or_transport_error() {
        let c = context();
        let (q, r, _) = pair(&c, true, true);
        let mut unrelated = crate::decode_selection_response(&r).unwrap();
        unrelated.positive[0].term_sha256 = crate::sha256_hex(&[relevance::TERM_PRIMITIVE_DOMAIN, b"unrelated"]);
        unrelated.selection_sha256 = relevance::selection_digest(&unrelated);
        let active = IzBefore::new(&c, q.clone()).unwrap().activate().unwrap();
        let observed = active.complete(crate::encode_selection_response(&unrelated), failure::MISSING_POSITIVE.into()).unwrap()
            .condition_measurement().unwrap();
        assert_eq!(unrelated.positive.len(), 1); // present raw data is not exact relevant evidence
        assert_eq!(observed.available_mask(), Some(2));
        assert_eq!(observed.missing_mask(), Some(1));
        let active = IzBefore::new(&c, q.clone()).unwrap().activate().unwrap();
        assert!(active.complete(Vec::new(), "transport timed out".into()).is_err());
        unrelated.session_id = "other-attempt".into();
        unrelated.selection_sha256 = relevance::selection_digest(&unrelated);
        let active = IzBefore::new(&c, q).unwrap().activate().unwrap();
        assert!(active.complete(crate::encode_selection_response(&unrelated), failure::MISSING_POSITIVE.into()).is_err());
    }

    #[test]
    fn r22_derived_measurement_preserves_legacy_piz_bytes_and_cannot_relabel_a_deficit() {
        for version in [IZ_VERSION, IZ_ACTUAL_INPUT_VERSION] {
            let mut c = context();
            c.version = version;
            let s = source(c, false, true, vec![]);
            let bytes = encode_source(&s).unwrap();
            let record = request(&s);
            let digest = record.request_digest().unwrap();
            let decoded = decode_source(&bytes).unwrap();
            decoded.attempt.before.condition_measurement().unwrap();
            decoded.attempt.condition_measurement().unwrap();
            assert_eq!(encode_source(&decoded).unwrap(), bytes);
            assert_eq!(request(&decoded).request_digest().unwrap(), digest);
            let mut changed = decoded.attempt;
            changed.missing = IzMissing::Negative;
            assert_eq!(changed.condition_measurement().unwrap_err(), "IZ_CONDITION_MISSING_BINDING");
        }
    }

    #[test]
    fn a_same_origin_actual_deficits_compile_distinct_questions_without_rewriting_asht_reason() {
        let both = source(context(), false, false, vec![]);
        let positive = source(context(), false, true, vec![]);
        let negative = source(context(), true, false, vec![]);
        assert_eq!(both.attempt.missing, IzMissing::Both);
        assert_eq!(positive.attempt.missing, IzMissing::Positive);
        assert_eq!(negative.attempt.missing, IzMissing::Negative);
        assert_eq!(both.attempt.refusal, failure::MISSING_POSITIVE);
        assert_ne!(both.question_spec.question, positive.question_spec.question);
        assert_ne!(
            positive.question_spec.question,
            negative.question_spec.question
        );
        assert!(positive
            .question_spec
            .requested_fields
            .contains(&IzField::Positive));
        assert!(!positive
            .question_spec
            .requested_fields
            .contains(&IzField::Negative));
        assert!(!negative
            .question_spec
            .requested_fields
            .contains(&IzField::Positive));
        assert!(negative
            .question_spec
            .requested_fields
            .contains(&IzField::Negative));
        let (q, r, _) = pair(&context(), true, true);
        assert!(IzBefore::new(&context(), q)
            .unwrap()
            .activate()
            .unwrap()
            .complete(r, "accepted".into())
            .is_err());
        let (q, r, _) = pair(&context(), false, false);
        assert!(IzBefore::new(&context(), q)
            .unwrap()
            .activate()
            .unwrap()
            .complete(r, "transport timeout".into())
            .is_err());
    }
    #[test]
    fn b_partial_answer_changes_frontier_but_never_admits_or_releases_hold() {
        let first = record(&source(context(), false, false, vec![]));
        let c = reply(
            &first,
            "reply-one",
            IzAnswer {
                positive: "user reports alpha support".into(),
                ..IzAnswer::default()
            },
        );
        let next = source(c, false, false, frontier_from_parent(&first).unwrap());
        validate_parent(&next, Some(&first)).unwrap();
        assert!(!next
            .question_spec
            .requested_fields
            .contains(&IzField::Positive));
        assert!(next
            .question_spec
            .requested_fields
            .contains(&IzField::Negative));
        assert_eq!(next.attempt.missing, IzMissing::Both);
        assert!(next.question_spec.hold);
        assert_eq!(
            next.question_spec.status,
            IzMaterialStatus::OperationalNoVerdict
        );
        assert!(request(&next).candidate.is_empty());
        let saved = record(&next);
        let refs = frontier_from_parent(&saved).unwrap();
        assert!(
            matches!(&refs[0].origin,IzFieldOrigin::Prior{record_id,revision,operation_id,..}
            if *record_id==saved.record_id && *revision==saved.revision && operation_id=="reply-one")
        );
        let third = source(
            reply(
                &saved,
                "reply-two",
                IzAnswer {
                    verification: "a falsifying test is reported".into(),
                    ..IzAnswer::default()
                },
            ),
            false,
            false,
            refs,
        );
        validate_parent(&third, Some(&saved)).unwrap();
        let erased = source(third.context.clone(), false, false, vec![]);
        assert!(validate_parent(&erased, Some(&saved)).is_err());
        assert!(!third
            .question_spec
            .requested_fields
            .contains(&IzField::Positive));
        assert!(!third
            .question_spec
            .requested_fields
            .contains(&IzField::Verification));
        assert!(third
            .question_spec
            .requested_fields
            .contains(&IzField::SourceReference));
        assert!(third.question_spec.hold);
    }
    #[test]
    fn c_d_missing_pre_witness_wrong_activation_and_reordered_causality_refuse() {
        let s = source(context(), false, false, vec![]);
        for kind in 0..5 {
            let mut altered = s.clone();
            match kind {
                0 => altered.attempt.before.pre_witness.payload_digest = ZERO,
                1 => altered.attempt.activation.authority = 1,
                2 => {
                    altered.attempt.activation.predecessor =
                        altered.attempt.before.origin.event_digest
                }
                3 => altered.assembly_activation = altered.attempt.activation.clone(),
                _ => altered.attempt.outcome.attempt_id = "foreign".into(),
            }
            assert!(altered.validate().is_err());
        }
        for (i, e) in s.events().iter().enumerate() {
            assert_eq!(e.sequence, i as u8);
            assert_eq!(e.authority, 0);
        }
        let before = IzBefore::new(&context(), pair(&context(), false, false).0).unwrap();
        // Its request-only witness has no result field; construction is separate from activation.
        assert_eq!(before.pre_witness.kind, IzEventKind::Precondition);
    }
    #[test]
    fn k_question_outside_deficit_or_obligation_and_payload_relabel_refuse() {
        let s = source(context(), false, true, vec![]);
        let mut wrong = s.clone();
        wrong.question_spec.question = "Unrelated weather question?".into();
        wrong.question_spec.question_sha256 = sha(wrong.question_spec.question.as_bytes());
        assert!(wrong.validate().is_err());
        let mut wrong = s.clone();
        wrong.question_spec.requested_fields = vec![IzField::Negative];
        assert!(wrong.validate().is_err());
        let mut wrong = s.clone();
        wrong.context.identity.obligation = "foreign obligation".into();
        assert!(wrong.validate().is_err());
        let mut req = request(&s);
        assert!(validate_operation(&req).is_ok());
        req.witness.condition_code = "MISSING_EXACT_BOTH".into();
        assert!(validate_operation(&req).is_err());
        let mut req = request(&s);
        req.lgc_steps.remove(1);
        assert!(validate_operation(&req).is_err());
        let mut req = request(&s);
        req.candidate = vec![1];
        req.candidate_key = [2; 32];
        assert!(validate_operation(&req).is_err());
    }
    #[test]
    fn parent_map_scope_revision_question_and_legacy_upgrade_are_explicit() {
        let first = record(&source(context(), false, false, vec![]));
        let c = reply(
            &first,
            "reply",
            IzAnswer {
                negative: "reported counterexample".into(),
                ..IzAnswer::default()
            },
        );
        let s = source(
            c.clone(),
            false,
            false,
            frontier_from_parent(&first).unwrap(),
        );
        assert!(validate_parent(&s, Some(&first)).is_ok());
        assert!(validate_parent(&s, None).is_err());
        let mut wrong = c.clone();
        wrong.map.as_mut().unwrap().identity_key = [8; 32];
        assert!(wrong.validate().is_err());
        let mut wrong = c.clone();
        wrong.parent.as_mut().unwrap().question_sha256 = [8; 32];
        let fake = wrong.parent.clone().unwrap();
        wrong.map.as_mut().unwrap().head = fake.clone();
        wrong.map.as_mut().unwrap().exact_record = fake;
        let wrong = source(wrong, false, false, frontier_from_parent(&first).unwrap());
        assert!(validate_parent(&wrong, Some(&first)).is_err());
        let mut incoming = c.clone();
        incoming.map = None;
        assert!(same_intent(&incoming, &c));
        let mut changed = c.clone();
        changed.map.as_mut().unwrap().identity_key = [8; 32];
        assert!(!same_intent(&changed, &c));
        let mut legacy = first.clone();
        legacy.request.source_input[..4].copy_from_slice(b"PDF1");
        assert!(frontier_from_parent(&legacy).is_err());
    }
    #[test]
    fn bounded_codecs_roundtrip_corruption_schema_and_size_reject() {
        let s = source(context(), false, false, vec![]);
        let bytes = encode_source(&s).unwrap();
        assert_eq!(decode_source(&bytes).unwrap(), s);
        let c = encode_context(&s.context).unwrap();
        assert_eq!(decode_context(&c).unwrap(), s.context);
        assert_eq!(
            decode_map_snapshot(&encode_map_snapshot(None).unwrap()).unwrap(),
            None
        );
        for cut in [0, 4, 6, bytes.len() / 2, bytes.len() - 1] {
            assert!(decode_source(&bytes[..cut]).is_err());
        }
        for index in (0..bytes.len()).step_by(37) {
            let mut b = bytes.clone();
            b[index] ^= 1;
            assert!(decode_source(&b).is_err());
        }
        let mut bad = bytes[..bytes.len() - 32].to_vec();
        bad[4] = 2;
        let checksum = sha(&bad);
        bad.extend_from_slice(&checksum);
        assert!(decode_source(&bad).is_err());
        let mut bad = bytes[..bytes.len() - 32].to_vec();
        bad.push(0);
        let checksum = sha(&bad);
        bad.extend_from_slice(&checksum);
        assert!(decode_source(&bad).is_err());
        assert!(decode_context(&vec![0; MAX_CONTEXT_BYTES + 1]).is_err());
        let mut big = context();
        big.answer = IzAnswer {
            positive: "p".repeat(8192),
            negative: "n".repeat(8192),
            distinction: "d".repeat(8192),
            ..IzAnswer::default()
        };
        assert!(encode_context(&big).is_err());
        let first = record(&s);
        let c = reply(
            &first,
            "reply",
            IzAnswer {
                positive: "reported".into(),
                ..IzAnswer::default()
            },
        );
        assert_eq!(
            decode_map_snapshot(&encode_map_snapshot(c.map.as_ref()).unwrap()).unwrap(),
            c.map
        );
    }

    #[test]
    fn v1_sealed_native_context_bytes_and_legacy_question_remain_compatible() {
        // Pinned bytes from the sealed native_canonical_iz_01/cycle-1.icx1.
        // This codec regression is not a new native execution or authority receipt.
        let pinned_hex = "494358310100140000006e61746976652d6c696768742d72756e74696d65280000004153485420657861637420706f7369746976652d6e656761746976652064697374696e6374696f6e1300000063616e6f6e6963616c2d697a2d636173652d610b000000574f524be286924e5552410b000000574f524be286924e5552414000000033633330376136336633386266303436653433323662666435303233353834623066363262376434306239373161336437633333346166313337613566376161070000006379636c652d3135000000416e616c697a6f206d6f746f72696e20696e647573747269616c207465206b7571206468652073656e736f72696e207465726d696b35000000416e616c697a6f206d6f746f72696e20696e647573747269616c207465206b7571206468652073656e736f72696e207465726d696b00000000000000000000000000000000000000000000b4d3da48bc8e80166527239ebe063cb876c6bf51e450d8e8f5c04e0ccefccc34";
        let bytes = pinned_hex
            .as_bytes()
            .chunks_exact(2)
            .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            hex(&sha(&bytes)),
            "9794ef1a3cd3c70594f4bec5bd362b5b25f0fb827ebf7036cf97cf0d8f9f0b14"
        );
        let c = decode_context(&bytes).unwrap();
        assert_eq!(c.version, IZ_VERSION);
        assert_eq!(encode_context(&c).unwrap(), bytes);
        assert_eq!(c.current_i0(), c.origin_i0);
        assert_eq!(c.checkpoint_prefix(), IZ_CHECKPOINT_PREFIX);
        let s = source(context(), false, false, vec![]);
        assert_eq!(s.question_spec.question,
            "Operational_iZ — Për pyetjen fillestare «Si dallohet alpha nga beta?», rastin stable-topic dhe detyrimin exact distinction, mungon të dy kandidatët pozitiv dhe negativ i pranuar saktësisht për Xi/Yi në tentativën attempt-initial. Jep materialin pozitiv mbështetës; materialin negativ kundërshtues; dallimin konkret midis Xi dhe Yi; burimin e kontrollueshëm; kushtin ose provën që do ta rrëzonte dallimin. Materiali i dhënë ruhet si USER_REPORTED, jo si kandidat i pranuar. HOLD vijon; kjo pyetje ka NoVerdict.");
        let wire = encode_source(&s).unwrap();
        assert_eq!(&wire[..6], b"PIZ1\x01\x00");
        assert_eq!(encode_source(&decode_source(&wire).unwrap()).unwrap(), wire);
    }

    #[test]
    fn v2_current_input_request_and_exact_unicode_annotation_are_bound() {
        let parent = record(&source(context(), false, false, vec![]));
        let mut c = reply(&parent, "v2-one", IzAnswer::default());
        c.version = IZ_ACTUAL_INPUT_VERSION;
        c.user_text = "Unë raportoj provë për alpha.".into();
        c.answer.positive = "provë".into();
        c.validate().unwrap();
        let spans = c.answer_spans().unwrap();
        assert_eq!(spans.len(), 1);
        let span = &spans[0];
        assert_eq!(span.field, IzField::Positive);
        assert_eq!(
            &c.user_text[span.start as usize..span.end as usize],
            "provë"
        );
        assert_ne!(c.current_i0(), c.origin_i0);
        assert_eq!(c.current_i0(), c.user_text);
        let prior = frontier_for_context(&c, &parent).unwrap();
        let s = source(c.clone(), false, false, prior);
        validate_parent(&s, Some(&parent)).unwrap();
        let request_bytes = pair(&c, false, false).0;
        let mut stale = crate::decode_selection_request(&request_bytes).unwrap();
        stale.parent_i0 = hex(&sha(c.origin_i0.as_bytes()));
        assert!(IzBefore::new(&c, crate::encode_selection_request(&stale)).is_err());
        let context_wire = encode_context(&c).unwrap();
        assert_eq!(&context_wire[..6], b"ICX2\x02\x00");
        assert_eq!(decode_context(&context_wire).unwrap(), c);
        let source_wire = encode_source(&s).unwrap();
        assert_eq!(&source_wire[..6], b"PIZ2\x02\x00");
        assert_eq!(decode_source(&source_wire).unwrap(), s);
        let mut req = request(&s);
        req.validate().unwrap();
        assert!(req.checkpoint.starts_with(IZ_CHECKPOINT_PREFIX_V2));
        req.checkpoint = IZ_CHECKPOINT_PREFIX.iter().copied().chain([1]).collect();
        assert!(validate_operation(&req).is_err());
        let mut mixed = source_wire[..source_wire.len() - 32].to_vec();
        mixed[..4].copy_from_slice(b"PIZ1");
        mixed[4..6].copy_from_slice(&IZ_VERSION.to_le_bytes());
        let checksum = sha(&mixed);
        mixed.extend_from_slice(&checksum);
        assert!(decode_source(&mixed).is_err());
    }

    #[test]
    fn v2_detached_or_ambiguous_field_text_cannot_suppress_evidence_request() {
        let mut c = context();
        c.version = IZ_ACTUAL_INPUT_VERSION;
        c.answer.positive = "material absent from the actual reply".into();
        assert_eq!(c.validate().unwrap_err(), "IZ_ANSWER_NOT_IN_USER_TEXT");
        c.answer.positive = "Alpha".into();
        assert!(c.validate().is_err()); // exact bytes; no case rewriting
        c.user_text = "ababa".into();
        c.origin_i0 = c.user_text.clone();
        c.identity.lineage_root = hex(&sha(c.origin_i0.as_bytes()));
        c.answer.positive = "aba".into();
        assert_eq!(c.validate().unwrap_err(), "IZ_ANSWER_SPAN_AMBIGUOUS"); // overlapping
        c.user_text = "alpha alpha".into();
        c.origin_i0 = c.user_text.clone();
        c.identity.lineage_root = hex(&sha(c.origin_i0.as_bytes()));
        c.answer.positive = "alpha".into();
        assert_eq!(c.validate().unwrap_err(), "IZ_ANSWER_SPAN_AMBIGUOUS");
    }

    #[test]
    fn v2_unknown_reply_keeps_frontier_and_question_never_becomes_user_input() {
        let mut first = context();
        first.version = IZ_ACTUAL_INPUT_VERSION;
        first.answer.positive = "alpha".into(); // literal reported text, no truth assertion
        let parent = record(&source(first, false, false, vec![]));
        let mut c = reply(&parent, "unknown", IzAnswer::default());
        c.user_text = "Nuk e di; nuk kam provë shtesë.".into();
        c.validate().unwrap();
        assert!(c.answer_spans().unwrap().is_empty());
        let prior = frontier_for_context(&c, &parent).unwrap();
        let s = source(c.clone(), false, false, prior.clone());
        validate_parent(&s, Some(&parent)).unwrap();
        assert_eq!(s.context.user_text, c.user_text);
        assert_eq!(s.context.current_i0(), c.user_text);
        assert_ne!(s.context.current_i0(), s.question_spec.question);
        assert_eq!(s.question_spec.frontier, prior);
        assert_eq!(s.attempt.missing, IzMissing::Both);
        assert!(s.question_spec.hold);
        assert!(s
            .question_spec
            .requested_fields
            .contains(&IzField::Negative));
        assert!(!s
            .question_spec
            .requested_fields
            .contains(&IzField::Positive));
        assert!(request(&s).candidate.is_empty());
        let r = record(&s);
        assert_eq!(r.operational_question_binding(), r.operational_next_i0());
        assert_ne!(r.operational_question_binding(), c.user_text);
        // A purported mutation of the user turn invalidates the already compiled context.
        // This does not claim we can tell whether a human later quotes a question.
        let mut forged = s.clone();
        forged.context.user_text = s.question_spec.question.clone();
        assert!(forged.validate().is_err());
    }

    #[test]
    fn v2_upgrade_preserves_parent_but_never_promotes_unbound_v1_fields() {
        let mut old = context();
        old.answer.positive = "a legacy detached annotation".into();
        let parent = record(&source(old, false, false, vec![]));
        let legacy = frontier_from_parent(&parent).unwrap();
        assert_eq!(legacy.len(), 1);
        let mut c = reply(
            &parent,
            "upgraded",
            IzAnswer {
                negative: "beta response".into(),
                ..IzAnswer::default()
            },
        );
        c.version = IZ_ACTUAL_INPUT_VERSION;
        let eligible = frontier_for_context(&c, &parent).unwrap();
        assert!(eligible.is_empty());
        let s = source(c.clone(), false, false, eligible);
        validate_parent(&s, Some(&parent)).unwrap();
        assert!(s
            .question_spec
            .requested_fields
            .contains(&IzField::Positive));
        assert_eq!(s.question_spec.frontier.len(), 1);
        assert_eq!(s.question_spec.frontier[0].field, IzField::Negative);
        let forged = source(c, false, false, legacy);
        assert!(validate_parent(&forged, Some(&parent)).is_err());
        let upgraded = record(&s);
        let mut downgrade = reply(
            &upgraded,
            "downgraded",
            IzAnswer {
                positive: "alpha".into(),
                ..IzAnswer::default()
            },
        );
        downgrade.version = IZ_VERSION;
        assert_eq!(
            frontier_for_context(&downgrade, &upgraded).unwrap_err(),
            "IZ_VERSION_DOWNGRADE"
        );
    }

    #[test]
    fn v2_annotation_answers_parent_question_or_refines_existing_report_only() {
        let mut c = context();
        c.version = IZ_ACTUAL_INPUT_VERSION;
        let parent = record(&source(c, false, true, vec![])); // only positive missing
        let reply = reply(
            &parent,
            "irrelevant-label",
            IzAnswer {
                negative: "beta report".into(),
                ..IzAnswer::default()
            },
        );
        let prior = frontier_for_context(&reply, &parent).unwrap();
        let s = source(reply, false, false, prior);
        assert_eq!(
            validate_parent(&s, Some(&parent)).unwrap_err(),
            "IZ_ANSWER_OUTSIDE_PARENT_OBLIGATION"
        );
    }

    fn current_context_for_pass() -> IzContext {
        let mut c = context();
        c.version = IZ_ACTUAL_INPUT_VERSION;
        c.identity.domain = c.identity.scope.clone();
        c
    }
    fn observed_pass(c: &IzContext) -> IzPassedAttempt {
        // Synthetic pair/attestation for pure binding tests only. No live hold or authority.
        let (q, r, _) = pair(c, true, true);
        IzBefore::new(c, q)
            .unwrap()
            .activate()
            .unwrap()
            .complete_available(r, [9; 32])
            .unwrap()
    }

    #[test]
    fn passed_observation_is_separate_from_missing_and_has_no_gcl_verdict() {
        let c = current_context_for_pass();
        let p = observed_pass(&c);
        p.validate(&c).unwrap();
        assert_eq!(p.outcome.event.kind, IzEventKind::Outcome);
        assert_eq!(p.outcome.event.authority, 0);
        assert_eq!(p.outcome.event.sequence, 3);
        assert_eq!(p.outcome.event.predecessor, p.activation.event_digest);
        for (positive, negative) in [(false, false), (true, false), (false, true)] {
            let (q, r, _) = pair(&c, positive, negative);
            assert!(IzBefore::new(&c, q)
                .unwrap()
                .activate()
                .unwrap()
                .complete_available(r, [9; 32])
                .is_err());
        }
        let (q, r, _) = pair(&c, true, true);
        assert!(IzBefore::new(&c, q.clone())
            .unwrap()
            .activate()
            .unwrap()
            .complete(r.clone(), "accepted".into())
            .is_err());
        assert!(IzBefore::new(&c, q)
            .unwrap()
            .activate()
            .unwrap()
            .complete_available(r, ZERO)
            .is_err());
    }

    #[test]
    fn passed_codec_and_causal_commitments_refuse_tamper_or_mixed_context() {
        let c = current_context_for_pass();
        let p = observed_pass(&c);
        let bytes = encode_passed(&p).unwrap();
        assert_eq!(&bytes[..6], b"IPA1\x01\x00");
        assert_eq!(decode_passed(&bytes).unwrap(), p);
        for cut in [0, 6, bytes.len() / 2, bytes.len() - 1] {
            assert!(decode_passed(&bytes[..cut]).is_err());
        }
        for i in 0..5 {
            let mut changed = p.clone();
            match i {
                0 => changed.before.pre_witness.predecessor = [6; 32],
                1 => changed.activation.authority = 1,
                2 => changed.outcome.event.predecessor = changed.before.origin.event_digest,
                3 => changed.asht_attestation_sha256 = [7; 32],
                _ => changed.outcome.event.attempt_id = "another-attempt".into(),
            }
            assert!(changed.validate(&c).is_err());
        }
        let mut other = c.clone();
        other.operation_id = "another-operation".into();
        assert!(p.validate(&other).is_err());
        let mut legacy = c;
        legacy.version = IZ_VERSION;
        assert!(p.validate(&legacy).is_err());
    }

    #[test]
    fn v5_origin_link_initial_and_mapped_parent_policies_are_explicit() {
        use crate::pd_prepared::PdIzOriginLink;
        let c = current_context_for_pass();
        let p = observed_pass(&c);
        let initial = PdIzOriginLink::new(c.clone(), ZERO, p.clone()).unwrap();
        assert_eq!(initial.current_input_sha256, sha(c.user_text.as_bytes()));
        assert_eq!(
            initial.cognitive_session_id,
            hex(&c.identity.key().unwrap())
        );
        assert!(PdIzOriginLink::new(c.clone(), [1; 32], p).is_err());
        let parent = record(&source(c, false, false, vec![]));
        let mut reply = reply(&parent, "actual-reply", IzAnswer::default());
        reply.user_text = "Nuk e di; po kërkoj provën.".into();
        validate_context_parent(&reply, &parent).unwrap();
        let p = observed_pass(&reply);
        let tip = parent.request.lgc_steps.last().unwrap().step_digest;
        let link = PdIzOriginLink::new(reply.clone(), tip, p.clone()).unwrap();
        assert!(PdIzOriginLink::new(reply.clone(), ZERO, p.clone()).is_err());
        reply.map = None;
        assert!(PdIzOriginLink::new(reply, tip, p).is_err());
        let mut changed = link.clone();
        changed.current_input_sha256 = [7; 32];
        assert!(changed.validate().is_err());
        let mut changed = link;
        changed.cognitive_session_id = "new-cognitive-id".into();
        assert!(changed.validate().is_err());
    }

    #[test]
    fn successful_route_uses_same_parent_annotation_obligation_as_missing_route() {
        let parent = record(&source(current_context_for_pass(), false, true, vec![]));
        let c = reply(
            &parent,
            "wrong-field",
            IzAnswer {
                negative: "beta report".into(),
                ..IzAnswer::default()
            },
        );
        let passed = observed_pass(&c);
        passed.validate(&c).unwrap(); // pair shape alone cannot satisfy the parent's obligation
        assert_eq!(
            validate_context_parent(&c, &parent).unwrap_err(),
            "IZ_ANSWER_OUTSIDE_PARENT_OBLIGATION"
        );
    }
    // Synthetic contract shapes only. These do not authenticate a native bundle,
    // execute ASHT, or establish any accepted positive/negative knowledge.
    fn native_context(head: Option<&PdOperationRecord>) -> IzContext {
        let mut c = head
            .map(|r| context_from_record(r).unwrap())
            .unwrap_or_else(context);
        c.version = IZ_NATIVE_CONTINUITY_VERSION;
        c.operation_id = "native-followup".into();
        c.user_text = "This is the actual next user input".into();
        c.answer = IzAnswer::default();
        c.map = None;
        c.parent = head.map(|r| IzParent::from_record(r).unwrap());
        let anchor = crate::pd_prepared::PdContinuationLink {
            cognitive_session_id: hex(&c.identity.key().unwrap()),
            territory: c.identity.scope.clone(),
            predecessor: crate::pd_recovery::RecoveryLookup {
                identity: [7; 32],
                request_sha256: [8; 32],
            },
            predecessor_transaction_id: [9; 32],
            predecessor_continuation_sha256: [10; 32],
            predecessor_next_i0: "stored-question-binding-not-predicted-user-input".into(),
            clock_millis: true,
        };
        c.native_map = Some(native_map_for(&c.identity, &anchor, head).unwrap());
        c.native_anchor = Some(anchor);
        c
    }
    #[test]
    fn native_v3_preserves_actual_input_and_has_separate_versioned_map_codec() {
        let c = native_context(None);
        assert_ne!(c.user_text, c.origin_i0);
        assert_eq!(c.current_i0(), c.user_text);
        let bytes = encode_context(&c).unwrap();
        assert!(bytes.starts_with(b"ICX3"));
        assert_eq!(decode_context(&bytes).unwrap(), c);
        let map = c.native_map.as_ref().unwrap();
        assert_eq!(
            decode_native_map_snapshot(&encode_native_map_snapshot(map).unwrap()).unwrap(),
            *map
        );
        assert!(decode_map_snapshot(&encode_native_map_snapshot(map).unwrap()).is_err());
        let s = source(c.clone(), false, false, vec![]);
        assert!(encode_source(&s).unwrap().starts_with(b"PIZ3"));
        assert_eq!(decode_source(&encode_source(&s).unwrap()).unwrap(), s);
        let r = record(&s);
        assert_eq!(context_from_record(&r).unwrap(), c);
        assert_eq!(
            checkpoint_raw(&r).unwrap(),
            b"synthetic-data-not-a-native-checkpoint"
        );
        assert!(crate::pd_checkpoint_data::validate_checkpoint(
            checkpoint_raw(&r).unwrap(),
            &hex(&c.identity.key().unwrap())
        )
        .is_err());
        for version in [IZ_VERSION, IZ_ACTUAL_INPUT_VERSION] {
            let mut wrong = c.clone();
            wrong.version = version;
            assert!(wrong.validate().is_err());
        }
    }
    #[test]
    fn native_v3_rejects_foreign_scope_clock_map_and_supplied_stale_map() {
        let c = native_context(None);
        let mut wrong = c.clone();
        wrong.native_anchor.as_mut().unwrap().clock_millis = false;
        assert!(wrong.validate().is_err());
        let mut wrong = c.clone();
        wrong.native_anchor.as_mut().unwrap().territory = "other".into();
        assert!(wrong.validate().is_err());
        let mut wrong = c.clone();
        wrong.native_map.as_mut().unwrap().identity_key = [1; 32];
        assert!(wrong.validate().is_err());
        let mut wrong = c.clone();
        wrong.native_map = None;
        assert!(wrong.validate().is_ok()); // before the authoritative map read
        assert!(wrong.require_mapped().is_err());
        assert!(same_intent(&wrong, &c));
        wrong.native_map = c.native_map.clone();
        wrong
            .native_map
            .as_mut()
            .unwrap()
            .native_anchor
            .predecessor_transaction_id = [33; 32];
        assert!(!same_intent(&wrong, &c));
        let mut bytes = encode_context(&c).unwrap();
        bytes[12] ^= 1;
        assert!(decode_context(&bytes).is_err());
    }
    #[test]
    fn native_v3_frontier_uses_same_anchor_operational_head_and_resets_after_new_native() {
        let first = record(&source(native_context(None), false, false, vec![]));
        let mut second = native_context(Some(&first));
        second.operation_id = "second".into();
        second.user_text = "user report alpha supported".into();
        second.answer.positive = second.user_text.clone();
        assert!(matches!(
            second.native_map.as_ref().unwrap().checkpoint_base,
            IzCheckpointBase::Operational(_)
        ));
        let prior = validate_context_parent(&second, &first).unwrap();
        let second_record = record(&source(second, false, false, prior));
        let mut third = native_context(Some(&second_record));
        third.operation_id = "third".into();
        assert_eq!(
            validate_context_parent(&third, &second_record)
                .unwrap()
                .len(),
            1
        );
        third
            .native_anchor
            .as_mut()
            .unwrap()
            .predecessor_transaction_id = [44; 32];
        third.native_map = Some(
            native_map_for(
                &third.identity,
                third.native_anchor.as_ref().unwrap(),
                Some(&second_record),
            )
            .unwrap(),
        );
        assert!(matches!(
            third.native_map.as_ref().unwrap().checkpoint_base,
            IzCheckpointBase::NativeT2
        ));
        assert!(validate_context_parent(&third, &second_record)
            .unwrap()
            .is_empty());
        third.answer.positive = third.user_text.clone();
        assert!(third.validate().is_err());
    }
    fn science_record_for(parent: &PdOperationRecord, c: IzContext) -> PdOperationRecord {
        let input = super::super::iz_science::from_parent(
            c,
            parent,
            vec![1],
            "unverified explicit relevance".into(),
        )
        .unwrap();
        let checkpoint = [input.checkpoint_prefix(), b"invalid-test-checkpoint"].concat();
        let candidate = vec![2];
        let steps = super::super::iz_science::build_steps(&input, &checkpoint, &candidate).unwrap();
        let request = PdOperationRequest {
            identity: input.context.identity.clone(),
            operation_id: input.context.operation_id.clone(),
            source_input: super::super::iz_science::encode_input(&input).unwrap(),
            expected_revision: parent.revision,
            previous_state: parent.state_digest,
            scoped_question: super::super::iz_science::question(&input).unwrap(),
            witness: super::super::iz_science::build_witness(&input, &steps, [3; 32]).unwrap(),
            checkpoint,
            candidate,
            candidate_key: [4; 32],
            lgc_steps: steps,
        };
        PdOperationRecord::construct(request, parent.record_id).unwrap()
    }
    #[test]
    fn native_v3_can_cas_after_legacy_science_but_does_not_use_its_checkpoint_or_fields() {
        let mut first_context = context();
        first_context.version = IZ_ACTUAL_INPUT_VERSION;
        let first = record(&source(first_context, false, false, vec![]));
        let mut c = context_from_record(&first).unwrap();
        c.operation_id = "legacy-science".into();
        c.user_text = "supplied scientific input".into();
        c.parent = Some(IzParent::from_record(&first).unwrap());
        let science = science_record_for(&first, c);
        assert!(science.request.source_input.starts_with(b"PDS2"));
        let next = native_context(Some(&science));
        assert!(matches!(
            next.native_map.as_ref().unwrap().checkpoint_base,
            IzCheckpointBase::NativeT2
        ));
        assert!(validate_context_parent(&next, &science).unwrap().is_empty());
        let output = source(next, false, false, vec![]);
        validate_parent(&output, Some(&science)).unwrap();
    }
    #[test]
    fn computed_science_retains_native_anchor_and_refuses_invalid_checkpoint() {
        use super::super::iz_science as science;
        // Shape-only anchor fixture; neither this payload nor checkpoint is admitted.
        let first = record(&source(native_context(None), false, false, vec![]));
        let mut c = native_context(Some(&first));
        c.operation_id = "computed-anchored-science".into();
        let legacy = science_record_for(&first, c);
        let mut input = science::decode_input(&legacy.request.source_input).unwrap();
        input.question_policy = science::QuestionPolicy::ComputedV1;
        let encoded = science::encode_input(&input).unwrap();
        assert!(encoded.starts_with(b"PDS5"));
        assert_eq!(&encoded[4..], &legacy.request.source_input[4..]);
        assert_eq!(science::decode_input(&encoded).unwrap(), input);
        science::validate_parent(&input, &first).unwrap();
        let mut request = legacy.request;
        request.source_input = encoded;
        request.scoped_question = "Synthetic display only; no computed evidence".into();
        request.lgc_steps = science::build_steps_with_question(&input, &request.checkpoint,
            &request.candidate, &request.scoped_question).unwrap();
        request.witness = science::build_witness(&input, &request.lgc_steps, [3;32]).unwrap();
        let record = PdOperationRecord::construct(request, first.record_id).unwrap();
        assert!(science::validate_transition(&record.request, &first).is_err());
        let next = native_context(Some(&record));
        assert!(matches!(next.native_map.unwrap().checkpoint_base, IzCheckpointBase::Operational(_)));
        input.context.native_anchor.as_mut().unwrap().predecessor_transaction_id = [55;32];
        assert!(science::validate_parent(&input, &first).is_err());
    }
    #[test]
    fn anchored_science_has_own_version_and_preserves_same_anchor_without_carrying_frontier() {
        let first = record(&source(native_context(None), false, false, vec![]));
        let mut c = native_context(Some(&first));
        c.operation_id = "anchored-science".into();
        let science = science_record_for(&first, c);
        assert!(science.request.source_input.starts_with(b"PDS3"));
        assert!(science
            .request
            .checkpoint
            .starts_with(super::super::iz_science::SCIENCE_CHECKPOINT_PREFIX_V3));
        let decoded =
            super::super::iz_science::decode_input(&science.request.source_input).unwrap();
        super::super::iz_science::validate_parent(&decoded, &first).unwrap();
        assert!(super::super::iz_science::validate_transition(&science.request, &first).is_err()); // invalid checkpoint is never admitted
        let mut next = native_context(Some(&science));
        next.operation_id = "after-science".into();
        assert!(matches!(
            next.native_map.as_ref().unwrap().checkpoint_base,
            IzCheckpointBase::Operational(_)
        ));
        assert!(validate_context_parent(&next, &science).unwrap().is_empty());
        let mut wrong = decoded.clone();
        wrong
            .context
            .native_anchor
            .as_mut()
            .unwrap()
            .predecessor_transaction_id = [55; 32];
        assert!(super::super::iz_science::validate_parent(&wrong, &first).is_err());
        let mut mislabeled = science.request.source_input.clone();
        mislabeled[..4].copy_from_slice(b"PDS2");
        assert!(super::super::iz_science::decode_input(&mislabeled).is_err());
    }
}
