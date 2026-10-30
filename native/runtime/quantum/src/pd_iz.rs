//! Canonical operational inquiry orchestration in Quantum.
//! POC-NEW glue for 0-NOI, d-bab and the same Shadow PD map.
//! Operational inquiry grants no verdict; observed ASHT success links existing
//! history to the original native PD/Spine/Shadow path without replacing its authority.
use crate::progressive_debatic::{
    core::ProgressiveDebatic,
    types::{DebateMode, PD_SEAL},
};
use ess_mai_zero_noi::trigger;
use sha2::{Digest, Sha256};
use shadow_contracts::pd_memory::{
    self as pd, iz, Hash, PdLookup, PdOperationRecord, PdOperationRequest,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn sha(bytes: &[u8]) -> Hash {
    Sha256::digest(bytes).into()
}
pub struct Prepared {
    context: iz::IzContext,
    parent: Option<PdOperationRecord>,
    prior_frontier: Vec<iz::IzFieldRef>,
    // Vault-owned native checkpoint selected by the reference-only V3 map.
    native_checkpoint: Option<Vec<u8>>,
}
pub enum Preparation {
    Replay(PdOperationRecord),
    Ready(Prepared),
}

/// ICX2 binds the actual new user input to both Light/ASHT and PD. ICX1
/// retains its historical original-proposition semantics for replay. A system
/// question is never substituted for an unknown future user reply.
pub fn check_native_input(context: &iz::IzContext, text: &str, scope: &str) -> Result<(), String> {
    context.validate()?;
    if context.current_i0() != text
        || context.identity.scope != scope
        || context.identity.domain != scope
    {
        return Err("IZ_NATIVE_INPUT_OR_SCOPE".into());
    }
    Ok(())
}
fn check_record(
    record: &PdOperationRecord,
    context: &iz::IzContext,
) -> Result<iz::IzSource, String> {
    record.validate()?;
    if record.request.identity != context.identity {
        return Err("IZ_RETURNED_IDENTITY".into());
    }
    iz::validate_operation(&record.request)
}

/// Exact retry is resolved before clock, a new authority attempt or PD ingest.
/// A caller-supplied contradictory map is rejected, never silently replaced.
pub fn prepare_with(
    context: &iz::IzContext,
    transport: &mut impl FnMut(&str, &[u8]) -> Result<Vec<u8>, String>,
) -> Result<Preparation, String> {
    context.validate()?;
    let exact = PdLookup {
        identity: context.identity.clone(),
        operation_id: Some(context.operation_id.clone()),
    };
    let lookup = pd::encode_lookup(&exact)?;
    let pulse = trigger(lookup.as_slice());
    if let Some(record) =
        pd::decode_optional_record(&transport("--pd-head-once", pulse.request())?)?
    {
        let stored = check_record(&record, context)?;
        if record.request.operation_id != context.operation_id
            || !iz::same_intent(context, &stored.context)
        {
            return Err("IZ_OPERATION_CONFLICT".into());
        }
        return Ok(Preparation::Replay(record));
    }
    // A new reply must arrive through the mapped Light/BESA protocol. Quantum
    // independently compares that metadata with Vault but never supplies an
    // omitted upstream step. Public metadata is not an authenticated signature.
    if context.parent.is_some() && context.map.is_none()
        && context.version != iz::IZ_NATIVE_CONTINUITY_VERSION {
        return Err("IZ_BESA_MAP_REQUIRED".into());
    }
    let lookup = pd::encode_lookup(&PdLookup {
        identity: context.identity.clone(),
        operation_id: None,
    })?;
    let pulse = trigger(lookup.as_slice());
    let parent = pd::decode_optional_record(&transport("--pd-head-once", pulse.request())?)?;
    let native_checkpoint = if context.version == iz::IZ_NATIVE_CONTINUITY_VERSION {
        let bytes = iz::encode_context(context)?;
        let pulse = trigger(bytes.as_slice());
        let bundle = shadow_contracts::pd_recovery::decode_bundle(
            &transport("--pd-iz-native-read-once", pulse.request())?)?;
        iz::validate_native_binding(context, &bundle, parent.as_ref())?;
        let (envelope, _, _) = bundle.validated()?;
        Some(envelope.checkpoint)
    } else { None };
    let prior_frontier = if context.version == iz::IZ_NATIVE_CONTINUITY_VERSION {
        match parent.as_ref() {
            Some(record) => iz::validate_context_parent(context, record)?,
            None => Vec::new(),
        }
    } else { match (&context.parent, &parent) {
        (None, None) => Vec::new(),
        (Some(expected), Some(record)) => {
            let stored_context = if pd::late_refusal::source_is_late(&record.request.source_input) {
                let prior = iz::context_from_record(record)?;
                if record.request.identity != context.identity {
                    return Err("IZ_RETURNED_IDENTITY".into());
                }
                prior
            } else {
                check_record(record, context)?.context
            };
            if iz::IzParent::from_record(record)? != *expected
                || stored_context.origin_i0 != context.origin_i0
            {
                return Err("IZ_STALE_OR_FOREIGN_PARENT".into());
            }
            let actual = iz::IzMapSnapshot {
                identity_key: context.identity.key()?,
                head: expected.clone(),
                exact_record: expected.clone(),
                status: iz::IzMaterialStatus::OperationalNoVerdict,
            };
            if context.map.as_ref() != Some(&actual) {
                return Err("IZ_SUPPLIED_MAP_CONFLICT".into());
            }
            iz::validate_context_parent(context, record)?
        }
        (None, Some(_)) => return Err("IZ_INITIAL_SCOPE_ALREADY_EXISTS".into()),
        (Some(_), None) => return Err("IZ_PARENT_MISSING".into()),
    }};
    Ok(Preparation::Ready(Prepared {
        context: context.clone(),
        parent,
        prior_frontier,
        native_checkpoint,
    }))
}

/// These witness/activation objects are created at the caller BEFORE its
/// actual source ASHT request. They do not infer a result.
pub fn before_action(
    prepared: &Prepared,
    request: Vec<u8>,
) -> Result<iz::IzActivatedBefore, String> {
    let before = iz::IzBefore::new(&prepared.context, request)?;
    let conditions = before.condition_measurement()?;
    eprintln!(
        "[D-BAB/CONDITIONS] phase=before required={} available=unknown missing=unknown negative_condition={} authority=0",
        conditions.required_mask(), conditions.is_negative_condition_knowledge()
    );
    eprintln!(
        "[IZ/LGC] event=precondition attempt={} digest={}",
        before.pre_witness.attempt_id,
        pd::hex(&before.pre_witness.event_digest)
    );
    let active = before.activate()?;
    eprintln!(
        "[IZ/LGC] event=action_activation digest={}",
        pd::hex(&active.activation.event_digest)
    );
    Ok(active)
}

/// Quantum owns production assembly; the shared implementation is a pure
/// data contract also used by Shadow to validate the result, not a second PD.
pub fn compile_exact_iz(
    context: iz::IzContext,
    attempt: iz::IzAttempt,
    prior: Vec<iz::IzFieldRef>,
    activation: iz::IzEvent,
) -> Result<iz::IzSource, String> {
    iz::compile_exact_iz(context, attempt, prior, activation)
}

/// R4 data link to actual prior LGC/PD history. The source remains operational;
/// availability at ASHT is not final acceptance by Shadow/GCL.
pub struct PromotedOrigin {
    pub link: shadow_contracts::pd_prepared::PdIzOriginLink,
    parent: Option<PdOperationRecord>,
    native_checkpoint: Option<Vec<u8>>,
}
pub fn prepare_promotion(
    prepared: Prepared,
    active: iz::IzActivatedBefore,
    response: Vec<u8>,
    actual_attestation_sha256: &str,
) -> Result<PromotedOrigin, String> {
    let parent=prepared.parent;
    let lgc_tip=match parent.as_ref() {
        Some(parent)=>{
            let prior=iz::context_from_record(parent)?;
            if prepared.context.parent.as_ref()!=Some(&iz::IzParent::from_record(parent)?)
                || prepared.prior_frontier!=iz::frontier_for_context(&prepared.context,parent)? {
                return Err("IZ_PROMOTION_EXACT_PARENT".into());
            }
            if prior.identity!=prepared.context.identity {
                return Err("IZ_PROMOTION_COGNITIVE_IDENTITY".into());
            }
            parent.request.lgc_steps.last().ok_or("IZ_PROMOTION_PRIOR_LGC_MISSING")?.step_digest
        },
        None=>{
            if prepared.context.parent.is_some() || !prepared.prior_frontier.is_empty() {
                return Err("IZ_PROMOTION_MISSING_PARENT".into());
            }
            [0;32]
        }
    };
    let passed=active.complete_available(response,pd::parse_hash(actual_attestation_sha256)?)?;
    let link=shadow_contracts::pd_prepared::PdIzOriginLink::new(prepared.context,lgc_tip,passed)?;
    Ok(PromotedOrigin{link,parent,native_checkpoint:prepared.native_checkpoint})
}
/// Restores only actual earlier user history. The current turn is ingested by
/// the original native ingest_for_spine_with_history_sealed call, exactly once.
pub fn restore_for_promotion(
    origin:&PromotedOrigin, territory:&str, now:u64,
)->Result<ProgressiveDebatic,String>{
    origin.link.validate()?;
    let context=&origin.link.context;
    if territory!=context.identity.scope || territory!=context.identity.domain || now==0 {
        return Err("IZ_PROMOTION_SCOPE_OR_CLOCK".into());
    }
    let mut engine=ProgressiveDebatic::new();
    match checkpoint_base(context, origin.parent.as_ref(), origin.native_checkpoint.as_deref())? {
        Some(raw)=>{
            engine.import_checkpoint(&origin.link.cognitive_session_id,raw)?;
        },
        None=>{
            // Preserve original main.rs native territory policy for a genuinely new session.
            // Imported operational history above retains its already recorded mode.
            let mode=match territory.trim().to_ascii_lowercase().as_str() {
                "philosophy" | "philosophical"=>DebateMode::Philosophy,
                "science" | "scientific" | "research"=>DebateMode::Scientific,
                _=>DebateMode::Intellect,
            };
            engine.open_session_sealed(&origin.link.cognitive_session_id,territory,
                mode,now,PD_SEAL).map_err(|e|format!("IZ_PROMOTION_PD_OPEN:{e:?}"))?;
        }
    }
    let state=engine.session(&origin.link.cognitive_session_id).ok_or("IZ_PROMOTION_PD_SESSION")?;
    if !engine.active||!state.active||state.territory!=territory
        ||now<state.last_activity_at||now<state.started_at||state.total_turns>=4096 {
        return Err("IZ_PROMOTION_PD_STATE".into());
    }
    Ok(engine)
}
/// Select the exact historical checkpoint; operation revision is not a turn counter.
fn checkpoint_base<'a>(context: &iz::IzContext, parent: Option<&'a PdOperationRecord>, native: Option<&'a [u8]>) -> Result<Option<&'a [u8]>, String> {
    if context.version == iz::IZ_NATIVE_CONTINUITY_VERSION {
        let map = context.native_map.as_ref().ok_or("IZ_NATIVE_MAP_REQUIRED")?;
        if matches!(map.checkpoint_base, iz::IzCheckpointBase::NativeT2) {
            return native.map(Some).ok_or("IZ_NATIVE_CHECKPOINT_MISSING".into());
        }
    }
    parent.map(iz::checkpoint_raw).transpose()
}
pub fn finish_with(
    prepared: Prepared,
    active: iz::IzActivatedBefore,
    response: Vec<u8>,
    refusal: String,
    transport: &mut impl FnMut(&str, &[u8]) -> Result<Vec<u8>, String>,
    clock: impl FnOnce() -> Result<u64, String>,
) -> Result<PdOperationRecord, String> {
    let attempt = active.complete(response, refusal)?;
    let conditions = attempt.condition_measurement()?;
    eprintln!(
        "[D-BAB/CONDITIONS] phase=after status={:?} required={} available={:?} missing={:?} gap_count={:?} condition_mass_fixed={:?} negative_condition={} semantic_counterproof=false authority=0 lgc={}",
        conditions.status(), conditions.required_mask(), conditions.available_mask(),
        conditions.missing_mask(), conditions.gap_count(), conditions.condition_mass_fixed(),
        conditions.is_negative_condition_knowledge(), pd::hex(&attempt.outcome.event_digest)
    );
    // Rebinding to the actual prepared scope is checked before PD mutation.
    let activation =
        iz::assembly_activation(&prepared.context, &attempt, &prepared.prior_frontier)?;
    eprintln!(
        "[IZ/LGC] event=outcome digest={}",
        pd::hex(&attempt.outcome.event_digest)
    );
    let session = pd::hex(&prepared.context.identity.key()?);
    let mut engine = ProgressiveDebatic::new();
    let now = clock()?;
    if now == 0 {
        return Err("IZ_CLOCK_ZERO".into());
    }
    match checkpoint_base(&prepared.context, prepared.parent.as_ref(), prepared.native_checkpoint.as_deref())? {
        Some(raw) => {
            let pulse = trigger(raw);
            engine.import_checkpoint(&session, pulse.request())?;
        }
        None => {
            engine
                .open_session_sealed(
                    &session,
                    &prepared.context.identity.scope,
                    DebateMode::Scientific,
                    now,
                    PD_SEAL,
                )
                .map_err(|e| format!("IZ_PD_OPEN:{e:?}"))?;
        }
    }
    let state = engine.session(&session).ok_or("IZ_PD_SESSION")?;
    if state.territory != prepared.context.identity.scope || !engine.active || !state.active {
        return Err("IZ_PD_SCOPE_OR_ACTIVITY".into());
    }
    if now < state.last_activity_at {
        return Err("IZ_CLOCK_REGRESSION".into());
    }
    if state.total_turns >= 4096 {
        return Err("IZ_PD_TURN_LIMIT".into());
    }
    let before_turns = state.total_turns;
    let user = trigger(prepared.context.user_text.as_str());
    engine
        .ingest_sealed(&session, user.request(), now, PD_SEAL)
        .map_err(|e| format!("IZ_PD_INGEST:{e:?}"))?;
    if engine.session(&session).ok_or("IZ_PD_SESSION")?.total_turns
        != before_turns.checked_add(1).ok_or("IZ_PD_TURN_OVERFLOW")?
    {
        return Err("IZ_PD_ONE_USER_TURN".into());
    }
    let raw = engine.export_checkpoint(&session)?;
    let prefix = prepared.context.checkpoint_prefix();
    let capacity = prefix
        .len()
        .checked_add(raw.len())
        .ok_or("IZ_CHECKPOINT_SIZE")?;
    if capacity > pd::MAX_CHECKPOINT_BYTES {
        return Err("IZ_CHECKPOINT_LIMIT".into());
    }
    let mut checkpoint = Vec::with_capacity(capacity);
    checkpoint.extend_from_slice(prefix);
    checkpoint.extend_from_slice(&raw);
    eprintln!(
        "[IZ/LGC] event=pd_checkpoint turns={} sha256={}",
        before_turns + 1,
        pd::hex(&sha(&checkpoint))
    );
    eprintln!(
        "[IZ/LGC] event=assembly_activation digest={}",
        pd::hex(&activation.event_digest)
    );
    let arguments = (
        prepared.context,
        attempt,
        prepared.prior_frontier,
        activation,
    );
    // The pulse transports the caller-selected assembly, without deciding it.
    let pulse = trigger(&arguments);
    let (context, attempt, prior, activation) = pulse.request();
    let source = compile_exact_iz(
        context.clone(),
        attempt.clone(),
        prior.clone(),
        activation.clone(),
    )?;
    iz::validate_parent(&source, prepared.parent.as_ref())?;
    let parent = source.context.parent.as_ref();
    let request = PdOperationRequest {
        identity: source.context.identity.clone(),
        operation_id: source.context.operation_id.clone(),
        source_input: iz::encode_source(&source)?,
        expected_revision: parent.map(|p| p.revision).unwrap_or(0),
        previous_state: parent.map(|p| p.state_digest).unwrap_or([0; 32]),
        witness: iz::build_witness(&source)?,
        checkpoint,
        scoped_question: source.question_spec.question.clone(),
        candidate: Vec::new(),
        candidate_key: [0; 32],
        lgc_steps: iz::build_lgc_steps(&source)?,
    };
    for entry in &request.lgc_steps {
        eprintln!(
            "[IZ/LGC] sequence={} site={} digest={}",
            entry.sequence,
            entry.site,
            pd::hex(&entry.step_digest)
        );
    }
    iz::validate_operation(&request)?;
    let encoded = pd::encode_request(&request)?;
    let pulse = trigger(encoded.as_slice());
    let returned = transport("--pd-commit-once", pulse.request()).map_err(|e| {
        format!("IZ_COMMIT_STATUS_UNCERTAIN: resolve exact operation before retry: {e}")
    })?;
    let committed = pd::decode_record(&returned)?;
    check_record(&committed, &source.context)?;
    if committed.request != request {
        return Err("IZ_COMMIT_RETURNED_DIFFERENT_REQUEST".into());
    }
    let exact = pd::encode_lookup(&PdLookup {
        identity: request.identity.clone(),
        operation_id: Some(request.operation_id.clone()),
    })?;
    let pulse = trigger(exact.as_slice());
    let readback = pd::decode_optional_record(
        &transport("--pd-head-once", pulse.request())
            .map_err(|e| format!("IZ_COMMIT_READBACK_UNCERTAIN:{e}"))?,
    )?
    .ok_or("IZ_COMMIT_READBACK_MISSING")?;
    if readback != committed {
        return Err("IZ_COMMIT_READBACK_MISMATCH".into());
    }
    Ok(readback)
}
fn native_transport(start: Instant) -> impl FnMut(&str, &[u8]) -> Result<Vec<u8>, String> {
    move |mode, bytes| {
        let remaining = Duration::from_secs(90)
            .checked_sub(start.elapsed())
            .ok_or("IZ_OPERATION_DEADLINE")?;
        crate::pd_operational::native_transport(mode, bytes, remaining)
    }
}
pub fn prepare_native(context: &iz::IzContext) -> Result<Preparation, String> {
    let prepared = prepare_with(context, &mut native_transport(Instant::now()))?;
    if matches!(&prepared, Preparation::Ready(_)) && !matches!(context.version, 2 | 3) {
        return Err("IZ_LEGACY_NEW_WRITE_DISABLED_USE_ICX2".into());
    }
    Ok(prepared)
}
pub fn finish_native(
    prepared: Prepared,
    active: iz::IzActivatedBefore,
    response: Vec<u8>,
    refusal: String,
) -> Result<PdOperationRecord, String> {
    finish_with(
        prepared,
        active,
        response,
        refusal,
        &mut native_transport(Instant::now()),
        || {
            let millis = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| "IZ_CLOCK_BEFORE_EPOCH")?
                .as_millis();
            u64::try_from(millis).map_err(|_| "IZ_CLOCK_OVERFLOW".into())
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use shadow_contracts::pd_memory::{
        iz::{IzAnswer, IzContext, IzField, IzParent},
        PdIdentity,
    };
    use shadow_contracts::{
        KnowledgeSelectionRequestWire, KnowledgeSelectionResponseWire, SelectionPurpose,
        SelectionRequester, PROTOCOL_VERSION, SELECTION_MAX_CANDIDATES,
    };

    fn context() -> IzContext {
        let origin = "Explain why the industrial motor differs from its sensor.".to_string();
        IzContext {
            version: 1,
            identity: PdIdentity {
                owner: "test-owner".into(),
                obligation: "exact pair".into(),
                case_id: "stable-case".into(),
                scope: "industrial".into(),
                domain: "industrial".into(),
                lineage_root: pd::hex(&sha(origin.as_bytes())),
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
    // Synthetic data-only request/hold. These tests do NOT authenticate source
    // ASHT capabilities or provide native execution/accepted-knowledge evidence.
    fn active(ready: &Prepared, attempt: &str) -> (iz::IzActivatedBefore, Vec<u8>) {
        let q = KnowledgeSelectionRequestWire {
            version: PROTOCOL_VERSION,
            requester: SelectionRequester::AshtQuantum,
            purpose: SelectionPurpose::EvidenceCandidates,
            session_id: attempt.into(),
            trace_id: attempt.into(),
            parent_i0: pd::hex(&sha(ready.context.current_i0().as_bytes())),
            primitive_anchor: 7,
            split_sha256: "11".repeat(32),
            xi: vec!["motor".into()],
            yi: vec!["sensor".into()],
            upstream_hold_sha256: Some("22".repeat(32)),
            evidence_sha256: "33".repeat(32),
            max_positive: SELECTION_MAX_CANDIDATES,
            max_negative: SELECTION_MAX_CANDIDATES,
        };
        let request = shadow_contracts::encode_selection_request(&q);
        let response = KnowledgeSelectionResponseWire {
            version: PROTOCOL_VERSION,
            requester: q.requester,
            session_id: q.session_id.clone(),
            parent_i0: q.parent_i0.clone(),
            primitive_anchor: q.primitive_anchor,
            split_sha256: q.split_sha256.clone(),
            upstream_hold_sha256: q.upstream_hold_sha256.clone(),
            request_sha256: shadow_contracts::sha256_hex(&[
                b"ESSMAI/SELECTION/REQUEST/V178",
                &request,
            ]),
            selection_sha256: shadow_contracts::sha256_hex(&[
                b"ESSMAI/SELECTION/CANDIDATES/V178",
                &[],
            ]),
            hold_id: format!("synthetic-{attempt}"),
            hold_sha256: "44".repeat(32),
            positive: vec![],
            negative: vec![],
            law_seal: 1,
            system_laws_seal: 2,
            expires_at_ns: 3,
        };
        (
            before_action(ready, request).unwrap(),
            shadow_contracts::encode_selection_response(&response),
        )
    }
    #[derive(Default)]
    struct Memory {
        records: Vec<PdOperationRecord>,
    }
    impl Memory {
        fn call(&mut self, mode: &str, bytes: &[u8]) -> Result<Vec<u8>, String> {
            match mode {
                "--pd-head-once" => {
                    let q = pd::decode_lookup(bytes)?;
                    let found = self.records.iter().rev().find(|r| {
                        r.request.identity == q.identity
                            && q.operation_id
                                .as_ref()
                                .map(|op| op == &r.request.operation_id)
                                .unwrap_or(true)
                    });
                    pd::encode_optional_record(found)
                }
                "--pd-commit-once" => {
                    let q = pd::decode_request(bytes)?;
                    iz::validate_operation(&q)?;
                    let parent = self.records.last();
                    iz::validate_parent(&iz::decode_source(&q.source_input)?, parent)?;
                    let record = PdOperationRecord::construct(
                        q,
                        parent.map(|p| p.record_id).unwrap_or([0; 32]),
                    )?;
                    self.records.push(record.clone());
                    pd::encode_record(&record)
                }
                _ => Err("unexpected synthetic port".into()),
            }
        }
        fn prepare(&mut self, input: &IzContext) -> Result<Preparation, String> {
            prepare_with(input, &mut |mode, bytes| self.call(mode, bytes))
        }
        fn run(
            &mut self,
            input: &IzContext,
            attempt: &str,
            now: u64,
        ) -> Result<PdOperationRecord, String> {
            match self.prepare(input)? {
                Preparation::Replay(record) => Ok(record),
                Preparation::Ready(ready) => {
                    let (before, response) = active(&ready, attempt);
                    finish_with(
                        ready,
                        before,
                        response,
                        pd::failure::MISSING_POSITIVE.into(),
                        &mut |mode, bytes| self.call(mode, bytes),
                        || Ok(now),
                    )
                }
            }
        }
    }
    fn reply(
        context: &IzContext,
        parent: &PdOperationRecord,
        op: &str,
        text: &str,
        answer: IzAnswer,
    ) -> IzContext {
        IzContext {
            version: context.version,
            identity: context.identity.clone(),
            operation_id: op.into(),
            origin_i0: context.origin_i0.clone(),
            user_text: text.into(),
            parent: Some(IzParent::from_record(parent).unwrap()),
            answer,
            // Explicit fixture for the Light/BESA map read; not reconstructed
            // by the Quantum production path and not native authority evidence.
            map: Some(iz::IzMapSnapshot {
                identity_key: context.identity.key().unwrap(),
                head: IzParent::from_record(parent).unwrap(),
                exact_record: IzParent::from_record(parent).unwrap(),
                status: iz::IzMaterialStatus::OperationalNoVerdict,
            }),
            native_anchor: None,
            native_map: None,
        }
    }
    #[test]
    fn real_pd_restore_preserves_three_actual_turns_and_field_frontier_without_admission() {
        let mut storage = Memory::default();
        let input = context();
        let first = storage.run(&input, "physical-attempt-1", 1000).unwrap();
        let c2 = reply(
            &input,
            &first,
            "reply-2",
            "The motor emits measured torque.",
            IzAnswer {
                positive: "reported torque".into(),
                ..Default::default()
            },
        );
        let second = storage.run(&c2, "physical-attempt-2", 2000).unwrap();
        let c3 = reply(
            &input,
            &second,
            "reply-3",
            "The local laboratory notebook contains the report.",
            IzAnswer {
                source_reference: "local notebook, page 5".into(),
                ..Default::default()
            },
        );
        let third = storage.run(&c3, "physical-attempt-3", 3000).unwrap();
        let source = iz::validate_operation(&third.request).unwrap();
        assert_eq!(third.revision, 3);
        assert_eq!(third.previous_record, second.record_id);
        assert!(source.question_spec.hold);
        assert_eq!(
            source.question_spec.status,
            iz::IzMaterialStatus::OperationalNoVerdict
        );
        assert!(!source
            .question_spec
            .requested_fields
            .contains(&IzField::Positive));
        assert!(!source
            .question_spec
            .requested_fields
            .contains(&IzField::SourceReference));
        assert!(source
            .question_spec
            .requested_fields
            .contains(&IzField::Negative));
        assert!(third.request.candidate.is_empty());
        let mut pd_engine = ProgressiveDebatic::new();
        let session = pd::hex(&input.identity.key().unwrap());
        pd_engine
            .import_checkpoint(
                &session,
                third
                    .request
                    .checkpoint
                    .strip_prefix(iz::IZ_CHECKPOINT_PREFIX)
                    .unwrap(),
            )
            .unwrap();
        let state = pd_engine.session(&session).unwrap();
        assert_eq!(state.total_turns, 3);
        assert_eq!(state.started_at, 1000);
        let actual: Vec<_> = state
            .trace
            .signals
            .iter()
            .map(|s| s.raw_content.as_str())
            .collect();
        assert_eq!(
            actual,
            vec![
                input.user_text.as_str(),
                c2.user_text.as_str(),
                c3.user_text.as_str()
            ]
        );
        assert!(source.question_spec.frontier.iter().any(|f|f.field==IzField::Positive
            && matches!(&f.origin,iz::IzFieldOrigin::Prior{record_id,revision,..} if *record_id==second.record_id&&*revision==2)));
        // Historical exact retry survives a newer head, without any new PD turn.
        let replay = storage.run(&c2, "must-not-be-used", 0).unwrap();
        assert_eq!(replay, second);
        assert_eq!(storage.records.len(), 3);
    }
    #[test]
    fn conflicting_operation_stale_parent_and_regressing_clock_never_append() {
        let mut storage = Memory::default();
        let input = context();
        let first = storage.run(&input, "physical-1", 1000).unwrap();
        let mut conflict = input.clone();
        conflict.answer.positive = "changed initial payload".into();
        assert!(matches!(storage.prepare(&conflict),Err(e) if e=="IZ_OPERATION_CONFLICT"));
        let c2 = reply(
            &input,
            &first,
            "reply-2",
            "Reported comparison.",
            IzAnswer {
                distinction: "reported difference".into(),
                ..Default::default()
            },
        );
        assert!(
            matches!(storage.run(&c2,"clock-regression",999),Err(e) if e=="IZ_CLOCK_REGRESSION")
        );
        assert_eq!(storage.records.len(), 1);
        storage.run(&c2, "physical-2", 2000).unwrap();
        let mut stale = c2.clone();
        stale.operation_id = "stale-new-operation".into();
        assert!(matches!(storage.prepare(&stale),Err(e) if e=="IZ_STALE_OR_FOREIGN_PARENT"));
        assert_eq!(storage.records.len(), 2);
    }
    #[test]
    fn outcome_cannot_be_invented_from_transport_error_or_transplanted_context() {
        let mut storage = Memory::default();
        let input = context();
        let ready = match storage.prepare(&input).unwrap() {
            Preparation::Ready(v) => v,
            _ => panic!("unexpected replay"),
        };
        let (before, response) = active(&ready, "physical-1");
        assert!(finish_with(
            ready,
            before,
            response,
            "transport timeout".into(),
            &mut |mode, bytes| storage.call(mode, bytes),
            || Ok(1000)
        )
        .is_err());
        assert!(storage.records.is_empty());
        assert!(
            check_native_input(&input, "different proposition", &input.identity.scope).is_err()
        );
        assert!(check_native_input(&input, &input.origin_i0, "foreign-domain").is_err());
    }
    #[test]
    fn new_reply_requires_supplied_map_but_exact_replay_does_not_repeat_upstream_reads() {
        let mut storage = Memory::default();
        let input = context();
        let first = storage.run(&input, "physical-initial", 1000).unwrap();
        let mapped = reply(
            &input,
            &first,
            "mapped-reply",
            "The source is a local notebook.",
            IzAnswer {
                source_reference: "local notebook".into(),
                ..Default::default()
            },
        );
        let mut unmapped = mapped.clone();
        unmapped.map = None;
        let mut calls = Vec::new();
        let result = prepare_with(&unmapped, &mut |mode, bytes| {
            calls.push(mode.to_string());
            storage.call(mode, bytes)
        });
        assert!(matches!(result, Err(error) if error == "IZ_BESA_MAP_REQUIRED"));
        assert_eq!(calls, vec!["--pd-head-once"]);
        assert_eq!(storage.records.len(), 1);
        let committed = storage.run(&mapped, "physical-reply", 2000).unwrap();
        let replay = match storage.prepare(&unmapped).unwrap() {
            Preparation::Replay(record) => record,
            Preparation::Ready(_) => panic!("exact replay must precede new-reply map requirement"),
        };
        assert_eq!(replay, committed);
        assert_eq!(storage.records.len(), 2);
    }
    #[test]
    fn v2_restores_real_input_history_without_ingesting_question_or_original_again() {
        // Actual PD engine; synthetic empty ASHT selection is a contract fixture.
        let mut storage = Memory::default();
        let mut initial = context();
        initial.version = 2;
        let first = storage.run(&initial, "v2-attempt-1", 1000).unwrap();
        let c2 = reply(
            &initial,
            &first,
            "v2-reply",
            "The motor emits measured torque.",
            IzAnswer {
                positive: "measured torque".into(),
                ..Default::default()
            },
        );
        check_native_input(&c2, &c2.user_text, &c2.identity.scope).unwrap();
        assert!(check_native_input(&c2, &c2.origin_i0, &c2.identity.scope).is_err());
        let second = storage.run(&c2, "v2-attempt-2", 2000).unwrap();
        let c3 = reply(
            &initial,
            &second,
            "v2-unknown",
            "I do not know the motor result.",
            IzAnswer::default(),
        );
        let third = storage.run(&c3, "v2-attempt-3", 3000).unwrap();
        let source = iz::validate_operation(&third.request).unwrap();
        assert_eq!(source.context.version, 2);
        assert_eq!(source.question_spec.frontier.len(), 1);
        assert!(!source
            .question_spec
            .requested_fields
            .contains(&IzField::Positive));
        let (request, _) = pd::failure::decode_selection_pair(
            &source.attempt.before.request,
            &source.attempt.response,
        )
        .unwrap();
        assert_eq!(request.parent_i0, pd::hex(&sha(c3.user_text.as_bytes())));
        assert_ne!(request.parent_i0, initial.identity.lineage_root);
        let mut pd_engine = ProgressiveDebatic::new();
        let session = pd::hex(&initial.identity.key().unwrap());
        pd_engine
            .import_checkpoint(
                &session,
                third
                    .request
                    .checkpoint
                    .strip_prefix(source.context.checkpoint_prefix())
                    .unwrap(),
            )
            .unwrap();
        let state = pd_engine.session(&session).unwrap();
        let texts: Vec<_> = state
            .trace
            .signals
            .iter()
            .map(|s| s.raw_content.as_str())
            .collect();
        assert_eq!(
            texts,
            vec![
                initial.user_text.as_str(),
                c2.user_text.as_str(),
                c3.user_text.as_str()
            ]
        );
        assert_eq!(state.total_turns, 3);
        assert!(!texts
            .iter()
            .any(|t| *t == first.request.scoped_question || *t == second.request.scoped_question));
        assert_eq!(
            storage.run(&c2, "unused-replay-attempt", 0).unwrap(),
            second
        );
        assert_eq!(storage.records.len(), 3);
    }
}
