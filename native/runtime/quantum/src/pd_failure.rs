//! actual missing-history continuation under existing authority.
//! User turns only enter native PD. Supplied P/N/distinction/source text is
//! USER-REPORTED Vault material, never accepted knowledge or a new verdict.
//! pd_mp is the persistent map of scoped record/parent/question references;
//! the same authoritative Vault owns material and checkpoint record bytes.
use crate::progressive_debatic::{
    core::ProgressiveDebatic,
    types::{DebateMode, PD_SEAL},
};
use ess_mai_zero_noi::trigger;
use sha2::{Digest, Sha256};
use shadow_contracts::pd_memory::failure::*;
use shadow_contracts::pd_memory::linked_science::{self, LinkedScienceInput};
use shadow_contracts::pd_memory::*;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn sha(bytes: &[u8]) -> Hash {
    Sha256::digest(bytes).into()
}
fn source_digest() -> Hash {
    sha(include_bytes!(
        "../../shadow-contracts/src/pd_memory/failure.rs"
    ))
}

// 0-NOI is only a predecessor-supplied trigger. Decisions are made explicitly
// by the caller below; the trigger does not infer authority or choose a route.
fn step<T>(
    steps: &mut Vec<LgcStep>,
    input: Hash,
    site: &str,
    action: &str,
    bytes: &[u8],
    effect: impl FnOnce(&[u8]) -> Result<T, String>,
) -> Result<T, String> {
    let pulse = trigger(bytes);
    let value = effect(pulse.request())?;
    let parent = steps.last().map(|s| s.step_digest).unwrap_or(input);
    let entry = LgcStep::new(steps.len() as u32, parent, site, action, sha(bytes))?;
    eprintln!(
        "LGC failure sequence={} site={} action={} digest={}",
        entry.sequence,
        entry.site,
        entry.action,
        hex(&entry.step_digest)
    );
    steps.push(entry);
    Ok(value)
}
fn record_scope(record: &PdOperationRecord, input: &FailureDialogueInput) -> Result<(), String> {
    record.validate()?;
    if record.request.identity != input.identity {
        return Err("PD_FAILURE_RETURNED_SCOPE".into());
    }
    Ok(())
}
fn checkpoint(inner: Vec<u8>) -> Result<Vec<u8>, String> {
    let size = FAILURE_CHECKPOINT_PREFIX
        .len()
        .checked_add(inner.len())
        .ok_or("PD_CHECKPOINT_SIZE")?;
    if size > MAX_CHECKPOINT_BYTES {
        return Err("PD_CHECKPOINT_PAYLOAD_LIMIT".into());
    }
    let mut out = Vec::with_capacity(size);
    out.extend_from_slice(FAILURE_CHECKPOINT_PREFIX);
    out.extend_from_slice(&inner);
    Ok(out)
}
fn question(
    pd: &ProgressiveDebatic,
    session: &str,
    input: &FailureDialogueInput,
    missing: MissingSelection,
) -> Result<String, String> {
    let state = pd.session(session).ok_or("PD_FAILURE_SESSION_MISSING")?;
    let turn = state.turns.last().ok_or("PD_FAILURE_TURN_MISSING")?;
    let (selection, _) =
        decode_selection_pair(&input.selection_request, &input.selection_response)?;
    let absent = match missing {
        MissingSelection::Positive => "pozitiv",
        MissingSelection::Negative => "negativ",
    };
    let prefix = if input.in_reply_to == [0; 32] {
        "Kushti i munguar u konstatua nga përzgjedhja reale."
    } else {
        "Përgjigjja jote për pyetjen e mëparshme u ruajt si material i raportuar nga përdoruesi; ende nuk është dije e pranuar."
    };
    let result=format!(
        "{prefix} Për rastin {} dhe detyrimin {}, përzgjedhja e ruajtur nuk kishte kandidat {absent} të lidhur saktësisht me Xi/Yi. Xi={:?}; Yi={:?}. Jep materialin mbështetës dhe kundërshtues, dallimin konkret midis Xi dhe Yi, burimin e kontrollueshëm dhe çfarë prove do ta rrëzonte këtë dallim. Konteksti real PD: {}. Turne reale={}; sfida të përdorura={}. Ky vazhdim ka NoVerdict dhe nuk zëvendëson gjykimin ekzistues.",
        input.identity.case_id,input.identity.obligation,selection.xi,selection.yi,turn.pd_response_hint,
        state.total_turns,state.pressure_budget.challenges_issued,
    );
    // Preserve full Xi/Yi rather than silently truncating the conditions. An
    // oversized question is an explicit resource refusal before any commit.
    if result.len() > 8192 {
        return Err("PD_FAILURE_QUESTION_LIMIT".into());
    }
    Ok(result)
}

/// Real orchestration with an injectable native transport/clock for tests.
/// This does not simulate the authority or reconstruct an active capability.
pub fn continue_with(
    input: &FailureDialogueInput,
    transport: &mut impl FnMut(&str, &[u8]) -> Result<Vec<u8>, String>,
    clock: impl FnOnce() -> Result<u64, String>,
) -> Result<PdOperationRecord, String> {
    continue_dialogue_with(input, None, transport, clock)
}

/// PDS1 keeps the exact PDF1 identity and raw cognitive policy; the provider
/// candidate is a separate payload in the same single Vault record.
pub fn continue_linked_with(
    linked: &LinkedScienceInput,
    transport: &mut impl FnMut(&str, &[u8]) -> Result<Vec<u8>, String>,
    clock: impl FnOnce() -> Result<u64, String>,
) -> Result<PdOperationRecord, String> {
    linked.validate()?;
    continue_dialogue_with(&linked.dialogue, Some(linked), transport, clock)
}

fn continue_dialogue_with(
    input: &FailureDialogueInput,
    linked: Option<&LinkedScienceInput>,
    transport: &mut impl FnMut(&str, &[u8]) -> Result<Vec<u8>, String>,
    clock: impl FnOnce() -> Result<u64, String>,
) -> Result<PdOperationRecord, String> {
    let source = match linked {
        Some(value) => linked_science::encode_linked_input(value)?,
        None => encode_failure_input(input)?,
    };
    let input_hash = sha(&source);
    let session = hex(&input.identity.key()?);
    let mut steps = Vec::new();
    let exact = PdLookup {
        identity: input.identity.clone(),
        operation_id: Some(input.operation_id.clone()),
    };
    let historical = step(
        &mut steps,
        input_hash,
        "pd_mp",
        "lookup exact operation before clock and ingest",
        &encode_lookup(&exact)?,
        |bytes| decode_optional_record(&transport("--pd-head-once", bytes)?),
    )?;
    if let Some(record) = historical {
        record_scope(&record, input)?;
        if record.request.operation_id != input.operation_id
            || record.request.source_input != source
        {
            return Err("PD_FAILURE_OPERATION_CONFLICT".into());
        }
        return Ok(record);
    }
    let lookup = PdLookup {
        identity: input.identity.clone(),
        operation_id: None,
    };
    let head = step(
        &mut steps,
        input_hash,
        "pd_mp",
        "resolve same scope head and parent question reference",
        &encode_lookup(&lookup)?,
        |bytes| decode_optional_record(&transport("--pd-head-once", bytes)?),
    )?;
    let mut pd = ProgressiveDebatic::new();
    let (revision, previous_state) = match head.as_ref() {
        Some(record) => {
            record_scope(record, input)?;
            validate_failure_operation(&record.request)?;
            if let Some(value) = linked {
                linked_science::validate_linked_parent(value, record)?;
            }
            if input.in_reply_to != [0; 32] {
                if input.in_reply_to != record.record_id {
                    return Err("PD_FAILURE_STALE_ANSWER_PARENT".into());
                }
                let prior = linked_science::decode_dialogue_source(&record.request.source_input)?;
                if prior.selection_request != input.selection_request
                    || prior.selection_response != input.selection_response
                    || prior.refusal != input.refusal
                {
                    return Err("PD_FAILURE_ANSWER_CHANGED_ORIGINAL_SELECTION".into());
                }
                // The exact parent record already hashes the question; this
                // step additionally gives its precise byte digest in the trace.
                step(
                    &mut steps,
                    input_hash,
                    "pd_mp",
                    "bind answer to exact stored parent question",
                    record.request.scoped_question.as_bytes(),
                    |_| Ok(()),
                )?;
            }
            let raw = record
                .request
                .checkpoint
                .strip_prefix(FAILURE_CHECKPOINT_PREFIX)
                .ok_or("PD_FAILURE_CHECKPOINT_POLICY_REVIEW_REQUIRED")?;
            step(
                &mut steps,
                input_hash,
                "PD",
                "restore actual-user-only cognitive state reference",
                raw,
                |bytes| pd.import_checkpoint(&session, bytes),
            )?;
            (record.revision, record.state_digest)
        }
        None => {
            if input.in_reply_to != [0; 32] {
                return Err("PD_FAILURE_ANSWER_PARENT_MISSING".into());
            }
            (0, [0; 32])
        }
    };
    let now = clock()?;
    if head.is_none() {
        step(
            &mut steps,
            input_hash,
            "PD",
            "open absent scoped cognitive session",
            source.as_slice(),
            |_| {
                pd.open_session_sealed(
                    &session,
                    &input.identity.scope,
                    DebateMode::Scientific,
                    now,
                    PD_SEAL,
                )
                .map_err(|e| format!("PD_FAILURE_OPEN:{e:?}"))
            },
        )?;
    }
    let state = pd.session(&session).ok_or("PD_FAILURE_SESSION_MISSING")?;
    if state.territory != input.identity.scope || !pd.active || !state.active {
        return Err("PD_FAILURE_SESSION_SCOPE_OR_ACTIVITY".into());
    }
    if now < state.last_activity_at {
        return Err("PD_CLOCK_REGRESSION".into());
    }
    if state.total_turns >= 4096 {
        return Err("PD_CHECKPOINT_TURN_LIMIT".into());
    }
    step(
        &mut steps,
        input_hash,
        "D-BAB",
        "witness required exact Xi/Yi pair before continuation",
        &input.selection_request,
        |_| Ok(()),
    )?;
    let missing = step(
        &mut steps,
        input_hash,
        "ASHT/relevance",
        "reproduce retained actual missing-condition refusal",
        &input.selection_response,
        |_| {
            validate_missing_selection(
                &input.selection_request,
                &input.selection_response,
                &input.refusal,
            )
        },
    )?;
    step(
        &mut steps,
        input_hash,
        "D-BAB",
        "witness reproduced missing condition; no epistemic verdict",
        input.refusal.as_bytes(),
        |_| Ok(()),
    )?;
    let (candidate, candidate_key, science_summary) = match linked {
        None => (Vec::new(), [0; 32], None),
        Some(value) => {
            let query = science_domains::decode_query(&value.science_query)
                .map_err(|e| format!("PD_LINKED_SCIENCE_QUERY:{e}"))?;
            if query.adapter != science_domains::AdapterId::Astronomy
                || query.session_id != session
                || query.case_id != input.identity.case_id
                || query.scope_id != input.identity.scope
                || query.obligation_id != input.identity.obligation
            {
                return Err("PD_LINKED_SCIENCE_SCOPE_OR_PROVIDER".into());
            }
            // The provider subject is not a canonical CoreDomain and does not
            // relabel input.identity.domain or increment cross-domain evidence.
            let computed = step(
                &mut steps,
                input_hash,
                "science",
                "evaluate explicit offline query; model candidate has no authority",
                &value.science_query,
                |_| science_domains::evaluate(&query).map_err(|e| format!("PD_LINKED_SCIENCE:{e}")),
            )?;
            let bytes = science_domains::encode_candidate(&computed).map_err(|e| e.to_string())?;
            let summary = format!(
                "Material i modelit offline, i papranuar: provider={}; query={}; computed_radians={:.17}; circular_residual_radians={:.17}; relation={:?}; result_sha256={}. Lidhja e propozuar nga perdoruesi (relevanca ende e paverifikuar): {}. Nuk eshte vezhgim, K+/K-, TRL4 ose verdikt.",
                computed.provenance.provider,computed.query.query_id,computed.computed_radians,
                computed.circular_residual_radians,computed.relation,
                science_domains::hash_hex(&computed.result_sha256),value.relevance_statement);
            (bytes, computed.query_sha256, Some(summary))
        }
    };
    step(
        &mut steps,
        input_hash,
        "PD",
        "ingest only actual supplied user text",
        input.user_text.as_bytes(),
        |_| {
            pd.ingest_sealed(&session, &input.user_text, now, PD_SEAL)
                .map(|_| ())
                .map_err(|e| format!("PD_FAILURE_INGEST:{e:?}"))
        },
    )?;
    let checkpoint = step(
        &mut steps,
        input_hash,
        "PD",
        "export full actual-user state for Vault record",
        session.as_bytes(),
        |_| checkpoint(pd.export_checkpoint(&session)?),
    )?;
    let scoped_question = step(
        &mut steps,
        input_hash,
        "PD/iZ",
        "ask exact missing distinction with source and falsification",
        &checkpoint,
        |_| {
            let mut text = question(&pd, &session, input, missing)?;
            if let Some(summary) = science_summary.as_ref() {
                text.push('\n');
                text.push_str(summary);
            }
            if text.len() > MAX_TEXT_BYTES {
                return Err("PD_FAILURE_QUESTION_LIMIT".into());
            }
            Ok(text)
        },
    )?;
    step(
        &mut steps,
        input_hash,
        "Shadow",
        "request durable same-Vault commit; outcome not yet known",
        &checkpoint,
        |_| Ok(()),
    )?;
    let request = PdOperationRequest {
        identity: input.identity.clone(),
        operation_id: input.operation_id.clone(),
        source_input: source,
        expected_revision: revision,
        previous_state,
        witness: ConditionWitness {
            predecessor: "ASHT exact request-bound relevance".into(),
            parent_authority:
                "Existing GCL/ASHT selection; operational missing-condition report only".into(),
            failed_action: "find both exact Xi/Yi-bound positive and negative candidates".into(),
            condition_code: match missing {
                MissingSelection::Positive => "MISSING_EXACT_POSITIVE",
                MissingSelection::Negative => "MISSING_EXACT_NEGATIVE",
            }
            .into(),
            reason: input.refusal.clone(),
            source_component:
                "shadow-contracts/src/pd_memory/failure.rs::relevance::project_request_bound".into(),
            source_sha256: source_digest(),
            attempt_trace: input.operation_id.clone(),
            input_sha256: input_hash,
            lgc_parent: steps.last().ok_or("PD_LGC_MISSING")?.step_digest,
            report_class: ReportClass::MissingConditions,
            permitted_next: NextAction::AcquireScopedEvidence,
        },
        checkpoint,
        scoped_question,
        candidate,
        candidate_key,
        lgc_steps: steps,
    };
    let encoded = encode_request(&request)?;
    let pulse = trigger(encoded.as_slice());
    let bytes = transport("--pd-commit-once", pulse.request()).map_err(|e| {
        format!("PD_COMMIT_STATUS_UNCERTAIN: resolve exact operation before retry: {e}")
    })?;
    let committed = decode_record(&bytes)?;
    record_scope(&committed, input)?;
    if committed.request != request {
        return Err("PD_FAILURE_RETURNED_DIFFERENT_REQUEST".into());
    }
    let lookup_bytes = encode_lookup(&exact)?;
    let pulse = trigger(lookup_bytes.as_slice());
    let readback = decode_optional_record(
        &transport("--pd-head-once", pulse.request())
            .map_err(|e| format!("PD_COMMIT_READBACK_UNCERTAIN:{e}"))?,
    )?
    .ok_or("PD_COMMIT_READBACK_MISSING")?;
    if readback != committed {
        return Err("PD_COMMIT_READBACK_MISMATCH".into());
    }
    eprintln!(
        "LGC failure committed record={} revision={} NoVerdict; pd_mp=references Vault=material",
        hex(&committed.record_id),
        committed.revision
    );
    Ok(committed)
}

/// Uses the existing real Shadow process transport and one bounded wall budget.
pub fn continue_native(input: &FailureDialogueInput) -> Result<PdOperationRecord, String> {
    let start = Instant::now();
    let mut transport = |mode: &str, bytes: &[u8]| {
        let remaining = Duration::from_secs(90)
            .checked_sub(start.elapsed())
            .ok_or("PD_FAILURE_OPERATION_DEADLINE")?;
        crate::pd_operational::native_transport(mode, bytes, remaining)
    };
    continue_with(input, &mut transport, || {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis();
        u64::try_from(millis).map_err(|_| "PD_CLOCK_OVERFLOW".into())
    })
}

/// Native PDS1 delivery uses the existing bounded Shadow process port.
pub fn continue_linked_native(input: &LinkedScienceInput) -> Result<PdOperationRecord, String> {
    let start = Instant::now();
    let mut transport = |mode: &str, bytes: &[u8]| {
        let remaining = Duration::from_secs(90)
            .checked_sub(start.elapsed())
            .ok_or("PD_FAILURE_OPERATION_DEADLINE")?;
        crate::pd_operational::native_transport(mode, bytes, remaining)
    };
    continue_linked_with(input, &mut transport, || {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis();
        u64::try_from(millis).map_err(|_| "PD_CLOCK_OVERFLOW".into())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use shadow_contracts::{
        encode_selection_request, encode_selection_response, KnowledgeSelectionRequestWire,
        KnowledgeSelectionResponseWire, SelectionPurpose, SelectionRequester, PROTOCOL_VERSION,
        SELECTION_MAX_CANDIDATES,
    };

    // Synthetic bound selection data. This is not a GCL judgement or scientific
    // proof. The tests below exercise real PD with a contract-level transport.
    fn fixture() -> FailureDialogueInput {
        let request = KnowledgeSelectionRequestWire {
            version: PROTOCOL_VERSION,
            requester: SelectionRequester::AshtQuantum,
            purpose: SelectionPurpose::EvidenceCandidates,
            session_id: "case-A".into(),
            parent_i0: "i0-0000000000000007".into(),
            primitive_anchor: 7,
            trace_id: "trace-A".into(),
            split_sha256: "11".repeat(32),
            xi: vec!["alpha".into()],
            yi: vec!["beta".into()],
            upstream_hold_sha256: Some("22".repeat(32)),
            evidence_sha256: "33".repeat(32),
            max_positive: SELECTION_MAX_CANDIDATES,
            max_negative: SELECTION_MAX_CANDIDATES,
        };
        let request_bytes = encode_selection_request(&request);
        let response = KnowledgeSelectionResponseWire {
            version: PROTOCOL_VERSION,
            requester: request.requester,
            session_id: request.session_id.clone(),
            parent_i0: request.parent_i0.clone(),
            primitive_anchor: 7,
            split_sha256: request.split_sha256.clone(),
            upstream_hold_sha256: request.upstream_hold_sha256.clone(),
            request_sha256: shadow_contracts::sha256_hex(&[
                b"ESSMAI/SELECTION/REQUEST/V178",
                &request_bytes,
            ]),
            selection_sha256: shadow_contracts::sha256_hex(&[
                b"ESSMAI/SELECTION/CANDIDATES/V178",
                &[],
            ]),
            hold_id: "hold-A".into(),
            hold_sha256: "44".repeat(32),
            positive: vec![],
            negative: vec![],
            law_seal: 1,
            system_laws_seal: 2,
            expires_at_ns: 3,
        };
        FailureDialogueInput {
            identity: PdIdentity {
                owner: "owner".into(),
                obligation: "obligation".into(),
                case_id: request.session_id.clone(),
                scope: "scope".into(),
                domain: "physics".into(),
                lineage_root: request.parent_i0.clone(),
            },
            operation_id: "failed-initial".into(),
            user_text: "Si dallohet alpha nga beta?".into(),
            selection_request: request_bytes,
            selection_response: encode_selection_response(&response),
            refusal: MISSING_POSITIVE.into(),
            in_reply_to: [0; 32],
            positive: String::new(),
            negative: String::new(),
            distinction: String::new(),
            source_reference: String::new(),
        }
    }

    #[derive(Default)]
    struct Memory {
        records: Vec<PdOperationRecord>,
        calls: Vec<String>,
        lose_commit_reply: bool,
    }
    impl Memory {
        fn call(&mut self, mode: &str, bytes: &[u8]) -> Result<Vec<u8>, String> {
            self.calls.push(mode.into());
            match mode {
                "--pd-head-once" => {
                    let lookup = decode_lookup(bytes)?;
                    encode_optional_record(self.records.iter().rev().find(|record| {
                        record.request.identity == lookup.identity
                            && lookup
                                .operation_id
                                .as_ref()
                                .map(|op| op == &record.request.operation_id)
                                .unwrap_or(true)
                    }))
                }
                "--pd-commit-once" => {
                    let request = decode_request(bytes)?;
                    if let Some(old) = self.records.iter().find(|record| {
                        record.request.identity == request.identity
                            && record.request.operation_id == request.operation_id
                    }) {
                        return if old.request == request {
                            encode_record(old)
                        } else {
                            Err("fixture operation conflict".into())
                        };
                    }
                    let head = self
                        .records
                        .iter()
                        .rev()
                        .find(|record| record.request.identity == request.identity);
                    let (revision, state, previous) = head
                        .map(|r| (r.revision, r.state_digest, r.record_id))
                        .unwrap_or((0, [0; 32], [0; 32]));
                    if request.expected_revision != revision || request.previous_state != state {
                        return Err("fixture CAS conflict".into());
                    }
                    let record = PdOperationRecord::construct(request, previous)?;
                    let encoded = encode_record(&record)?;
                    self.records.push(record);
                    if self.lose_commit_reply {
                        return Err("fixture reply lost AFTER commit".into());
                    }
                    Ok(encoded)
                }
                _ => Err("unexpected transport mode".into()),
            }
        }
        fn run(
            &mut self,
            input: &FailureDialogueInput,
            now: u64,
        ) -> Result<PdOperationRecord, String> {
            continue_with(input, &mut |mode, bytes| self.call(mode, bytes), || Ok(now))
        }
    }
    fn answer(
        initial: &FailureDialogueInput,
        parent: Hash,
        operation: &str,
    ) -> FailureDialogueInput {
        let mut value = initial.clone();
        value.operation_id = operation.into();
        value.in_reply_to = parent;
        value.user_text = "Une propozoj nje dallim te kontrollueshem ne kete matje.".into();
        value.positive = "userpositiveunique: pohim ende i paverifikuar".into();
        value.negative = "usernegativeunique: kundershtim ende i paverifikuar".into();
        value.distinction = "userdistinctionunique: dallim i raportuar".into();
        value.source_reference = "usersourceunique: raport lokal i raportuar".into();
        value
    }
    fn restore(input: &FailureDialogueInput, record: &PdOperationRecord) -> ProgressiveDebatic {
        let mut pd = ProgressiveDebatic::new();
        pd.import_checkpoint(
            &hex(&input.identity.key().unwrap()),
            record
                .request
                .checkpoint
                .strip_prefix(FAILURE_CHECKPOINT_PREFIX)
                .unwrap(),
        )
        .unwrap();
        pd
    }
    #[test]
    fn actual_missing_pair_creates_scoped_question_and_one_real_user_turn() {
        let input = fixture();
        let mut memory = Memory::default();
        let first = memory.run(&input, 100).unwrap();
        assert_eq!(first.revision, 1);
        assert_eq!(first.previous_record, [0; 32]);
        assert!(first.request.candidate.is_empty());
        assert_eq!(first.request.candidate_key, [0; 32]);
        assert_eq!(
            first.request.witness.report_class,
            ReportClass::MissingConditions
        );
        assert_eq!(first.request.witness.reason, input.refusal);
        assert_eq!(
            first.request.source_input,
            encode_failure_input(&input).unwrap()
        );
        for text in [
            "alpha",
            "beta",
            "pozitiv",
            "burimin",
            "rrëzonte",
            "NoVerdict",
        ] {
            assert!(
                first.request.scoped_question.contains(text),
                "missing {text}"
            );
        }
        let pd = restore(&input, &first);
        let session = pd.session(&hex(&input.identity.key().unwrap())).unwrap();
        assert_eq!(
            (
                session.total_turns,
                session.started_at,
                session.last_activity_at
            ),
            (1, 100, 100)
        );
        assert_eq!(session.trace.signals.len(), 1);
        assert_eq!(session.trace.signals[0].raw_content, input.user_text);
        let before = first
            .request
            .lgc_steps
            .iter()
            .position(|s| s.site == "D-BAB" && s.action.contains("before"))
            .unwrap();
        let observed = first
            .request
            .lgc_steps
            .iter()
            .position(|s| s.site == "ASHT/relevance")
            .unwrap();
        let question = first
            .request
            .lgc_steps
            .iter()
            .position(|s| s.site == "PD/iZ")
            .unwrap();
        assert!(before < observed && observed < question);
        first.validate().unwrap();
    }
    #[test]
    fn answer_restores_budget_and_binds_parent_question_without_ingesting_raw_support_as_turns() {
        let input = fixture();
        let mut memory = Memory::default();
        let first = memory.run(&input, 100).unwrap();
        let first_pd = restore(&input, &first);
        let key = hex(&input.identity.key().unwrap());
        let prior = first_pd.session(&key).unwrap();
        let next = answer(&input, first.record_id, "answer-2");
        let second = memory.run(&next, 101).unwrap();
        assert_eq!(second.previous_record, first.record_id);
        let pd = restore(&input, &second);
        let session = pd.session(&key).unwrap();
        assert_eq!(
            (
                session.total_turns,
                session.started_at,
                session.last_activity_at
            ),
            (2, 100, 101)
        );
        assert!(
            session.pressure_budget.challenges_issued >= prior.pressure_budget.challenges_issued
        );
        assert_eq!(session.trace.signals.len(), 2);
        assert_eq!(session.trace.signals[1].raw_content, next.user_text);
        assert!(!session.trace.concept_map.contains_key("userpositiveunique"));
        assert!(!session.trace.concept_map.contains_key("usernegativeunique"));
        assert!(!session.trace.concept_map.contains_key("usersourceunique"));
        assert_eq!(
            decode_failure_input(&second.request.source_input).unwrap(),
            next
        );
        assert!(second
            .request
            .lgc_steps
            .iter()
            .any(|s| s.action.contains("exact stored parent question")
                && s.payload_digest == sha(first.request.scoped_question.as_bytes())));
        assert!(second
            .request
            .scoped_question
            .contains("ende nuk është dije e pranuar"));
        assert!(second.request.candidate.is_empty());
    }
    #[test]
    fn source_pd_checkpoints_append_one_user_turn_without_rewriting_history() {
        use shadow_contracts::pd_checkpoint_data::validate_user_checkpoint_transition as check;
        fn raw(record: &PdOperationRecord) -> &[u8] {
            record
                .request
                .checkpoint
                .strip_prefix(FAILURE_CHECKPOINT_PREFIX)
                .unwrap()
        }
        let input = fixture();
        let session = hex(&input.identity.key().unwrap());
        let mut memory = Memory::default();
        let first = memory.run(&input, 100).unwrap();
        check(raw(&first), &session, &input.user_text, 0, None).unwrap();
        let next = answer(&input, first.record_id, "answer-2");
        let second = memory.run(&next, 101).unwrap();
        check(
            raw(&second),
            &session,
            &next.user_text,
            1,
            Some(raw(&first)),
        )
        .unwrap();
        let third_input = answer(&input, second.record_id, "answer-3");
        let third = memory.run(&third_input, 102).unwrap();
        check(
            raw(&third),
            &session,
            &third_input.user_text,
            2,
            Some(raw(&second)),
        )
        .unwrap();
        assert!(check(
            raw(&second),
            &session,
            "invented text",
            1,
            Some(raw(&first))
        )
        .unwrap_err()
        .contains("actual text mismatch"));
        assert!(check(raw(&second), &session, &next.user_text, 0, None).is_err());
        assert!(check(raw(&second), &session, &next.user_text, 1, None).is_err());
        assert!(check(
            raw(&third),
            &session,
            &third_input.user_text,
            2,
            Some(raw(&first))
        )
        .is_err());
        assert!(check(raw(&first), &session, &input.user_text, u64::MAX, None).is_err());
    }

    #[test]
    fn individually_valid_alternate_pd_history_cannot_replace_durable_prefix() {
        use shadow_contracts::pd_checkpoint_data::{
            validate_checkpoint, validate_user_checkpoint_transition as check,
        };
        fn raw(record: &PdOperationRecord) -> &[u8] {
            record
                .request
                .checkpoint
                .strip_prefix(FAILURE_CHECKPOINT_PREFIX)
                .unwrap()
        }
        let input = fixture();
        let session = hex(&input.identity.key().unwrap());
        let first = Memory::default().run(&input, 100).unwrap();
        let mut changed_input = input.clone();
        changed_input.user_text = "Nje histori tjeter e raportuar nga perdoruesi.".into();
        let mut alternate = Memory::default();
        let alternate_first = alternate.run(&changed_input, 100).unwrap();
        let next = answer(&changed_input, alternate_first.record_id, "answer-2");
        let alternate_second = alternate.run(&next, 101).unwrap();
        validate_checkpoint(raw(&alternate_second), &session).unwrap();
        assert!(check(
            raw(&alternate_second),
            &session,
            &next.user_text,
            1,
            Some(raw(&first))
        )
        .unwrap_err()
        .contains("history changed"));
        let mut restarted = Memory::default();
        let restarted_first = restarted.run(&input, 101).unwrap();
        let next = answer(&input, restarted_first.record_id, "answer-2");
        let restarted_second = restarted.run(&next, 102).unwrap();
        validate_checkpoint(raw(&restarted_second), &session).unwrap();
        assert!(check(
            raw(&restarted_second),
            &session,
            &next.user_text,
            1,
            Some(raw(&first))
        )
        .unwrap_err()
        .contains("lineage changed"));
    }

    #[test]
    fn historical_retry_precedes_clock_and_ingest_even_after_head_advanced() {
        let input = fixture();
        let mut memory = Memory::default();
        let first = memory.run(&input, 100).unwrap();
        let next = answer(&input, first.record_id, "answer-2");
        memory.run(&next, 101).unwrap();
        memory.calls.clear();
        let retried = continue_with(&input, &mut |mode, bytes| memory.call(mode, bytes), || {
            panic!("historical retry must not read current clock")
        })
        .unwrap();
        assert_eq!(retried, first);
        assert_eq!(memory.calls, vec!["--pd-head-once"]);
        assert_eq!(memory.records.len(), 2);
        let mut changed = input.clone();
        changed.user_text.push_str(" changed");
        assert!(memory
            .run(&changed, 999)
            .unwrap_err()
            .contains("OPERATION_CONFLICT"));
        assert_eq!(memory.records.len(), 2);
    }
    #[test]
    fn stale_answer_changed_selection_and_clock_regression_never_commit() {
        let input = fixture();
        let mut memory = Memory::default();
        let first = memory.run(&input, 100).unwrap();
        let next = answer(&input, first.record_id, "answer-2");
        memory.run(&next, 101).unwrap();
        let stale = answer(&input, first.record_id, "stale-answer");
        assert!(
            continue_with(&stale, &mut |m, b| memory.call(m, b), || panic!(
                "stale parent must precede clock"
            ))
            .unwrap_err()
            .contains("STALE_ANSWER_PARENT")
        );
        let head = memory.records.last().unwrap().clone();
        let mut changed = answer(&input, head.record_id, "changed-answer");
        let (mut selection, mut response) =
            decode_selection_pair(&changed.selection_request, &changed.selection_response).unwrap();
        selection.xi = vec!["newalpha".into()];
        changed.selection_request = encode_selection_request(&selection);
        response.request_sha256 = shadow_contracts::sha256_hex(&[
            b"ESSMAI/SELECTION/REQUEST/V178",
            &changed.selection_request,
        ]);
        changed.selection_response = encode_selection_response(&response);
        changed.validate().unwrap();
        assert!(
            continue_with(&changed, &mut |m, b| memory.call(m, b), || panic!(
                "changed original selection must precede clock"
            ))
            .unwrap_err()
            .contains("ANSWER_CHANGED_ORIGINAL_SELECTION")
        );
        let next = answer(&input, head.record_id, "time-regression");
        assert!(memory
            .run(&next, 99)
            .unwrap_err()
            .contains("CLOCK_REGRESSION"));
        assert_eq!(memory.records.len(), 2);
    }
    #[test]
    fn generic_transport_refusal_cannot_create_operational_missing_condition() {
        let mut input = fixture();
        input.refusal = "transport timeout".into();
        assert!(continue_with(
            &input,
            &mut |_, _| panic!("invalid refusal must not reach transport"),
            || panic!("invalid refusal must not read clock")
        )
        .unwrap_err()
        .contains("REASON_MISMATCH"));
    }
    #[test]
    fn lost_commit_reply_is_uncertain_then_exact_replay_resolves_without_new_turn() {
        let input = fixture();
        let mut memory = Memory {
            lose_commit_reply: true,
            ..Memory::default()
        };
        assert!(memory
            .run(&input, 100)
            .unwrap_err()
            .contains("COMMIT_STATUS_UNCERTAIN"));
        assert_eq!(memory.records.len(), 1);
        let committed = memory.records[0].clone();
        memory.lose_commit_reply = false;
        let resolved = continue_with(&input, &mut |m, b| memory.call(m, b), || {
            panic!("recovery must not read clock")
        })
        .unwrap();
        assert_eq!(resolved, committed);
        assert_eq!(memory.records.len(), 1);
    }
    #[test]
    fn another_checkpoint_ingest_policy_never_silently_resets_history() {
        let input = fixture();
        let mut memory = Memory::default();
        let first = memory.run(&input, 100).unwrap();
        let mut legacy_request = first.request.clone();
        legacy_request.checkpoint = legacy_request
            .checkpoint
            .strip_prefix(FAILURE_CHECKPOINT_PREFIX)
            .unwrap()
            .to_vec();
        let legacy = PdOperationRecord::construct(legacy_request, [0; 32]).unwrap();
        memory.records[0] = legacy.clone();
        assert_eq!(memory.run(&input, 0).unwrap(), legacy);
        let next = answer(&input, legacy.record_id, "answer-2");
        let calls_before=memory.calls.len();
        // Shared semantic admission now detects the wrong policy prefix before
        // the later restore branch; this stronger refusal must not be bypassed.
        let error=continue_with(&next,&mut |mode,bytes|memory.call(mode,bytes),|| {
            panic!("invalid checkpoint policy must fail before clock or ingest")
        }).unwrap_err();
        assert_eq!(error,"PD_FAILURE_STORAGE_POLICY");
        assert!(memory.calls[calls_before..].iter().all(|mode|mode=="--pd-head-once"),
            "invalid policy must perform no commit");
        assert_eq!(memory.records, vec![legacy]);
    }

    // Explicit IAU conformance values are regression fixtures only, never
    // production defaults, observed facts or an admitted Xi/Yi pair.
    fn linked_fixture(
        initial: &FailureDialogueInput,
        parent: &PdOperationRecord,
        op: &str,
    ) -> LinkedScienceInput {
        let mut dialogue = answer(initial, parent.record_id, op);
        dialogue.user_text = [
            &dialogue.positive,
            &dialogue.negative,
            &dialogue.distinction,
            &dialogue.source_reference,
        ]
        .map(String::as_str)
        .join("\n");
        let query = science_domains::ScienceQuery {
            adapter: science_domains::AdapterId::Astronomy,
            session_id: hex(&dialogue.identity.key().unwrap()),
            case_id: dialogue.identity.case_id.clone(),
            scope_id: dialogue.identity.scope.clone(),
            obligation_id: dialogue.identity.obligation.clone(),
            query_id: op.into(),
            input_source_id: "IAU conformance regression; not observation".into(),
            task: science_domains::ScienceTask::EarthRotationAngle {
                jd_part1: 2400000.5,
                jd_part2: 54388.0,
                time_scale: science_domains::TimeScale::Ut1,
                claim_radians: 0.4022837240028158,
                tolerance_radians: 1e-12,
            },
        };
        LinkedScienceInput {
            dialogue,
            parent_question_sha256: sha(parent.request.scoped_question.as_bytes()),
            science_query: science_domains::encode_query(&query).unwrap(),
            relevance_statement: "User-proposed model subquestion; relevance is not adjudicated"
                .into(),
        }
    }
    #[test]
    fn linked_science_and_ordinary_answers_share_exact_user_history_and_scope() {
        let original = fixture();
        let mut memory = Memory::default();
        let first = memory.run(&original, 100).unwrap();
        let linked = linked_fixture(&original, &first, "linked-2");
        let encoded = linked_science::encode_linked_input(&linked).unwrap();
        assert_eq!(
            linked_science::decode_linked_input(&encoded).unwrap(),
            linked
        );
        for end in [0, 4, encoded.len() - 1] {
            assert!(linked_science::decode_linked_input(&encoded[..end]).is_err());
        }
        let second =
            continue_linked_with(&linked, &mut |m, b| memory.call(m, b), || Ok(101)).unwrap();
        validate_failure_operation(&second.request).unwrap();
        assert_eq!(second.request.identity, first.request.identity);
        assert_eq!(second.previous_record, first.record_id);
        assert!(!second.request.candidate.is_empty());
        let candidate = science_domains::decode_candidate(&second.request.candidate).unwrap();
        assert_eq!(
            candidate.authority,
            science_domains::CandidateAuthority::None
        );
        assert_eq!(candidate.query_sha256, second.request.candidate_key);
        assert!(second.request.scoped_question.contains("computed_radians"));
        let ordinary = answer(&original, second.record_id, "ordinary-3");
        let third = memory.run(&ordinary, 102).unwrap();
        assert!(third.request.candidate.is_empty());
        assert_eq!(third.previous_record, second.record_id);
        let fourth_input = linked_fixture(&original, &third, "linked-4");
        let fourth =
            continue_linked_with(&fourth_input, &mut |m, b| memory.call(m, b), || Ok(103)).unwrap();
        let pd = restore(&original, &fourth);
        let session = pd.session(&hex(&original.identity.key().unwrap())).unwrap();
        assert_eq!((session.total_turns, session.started_at), (4, 100));
        let texts: Vec<_> = session
            .trace
            .signals
            .iter()
            .map(|s| s.raw_content.as_str())
            .collect();
        assert_eq!(
            texts,
            vec![
                original.user_text.as_str(),
                linked.dialogue.user_text.as_str(),
                ordinary.user_text.as_str(),
                fourth_input.dialogue.user_text.as_str()
            ]
        );
        assert!(texts.iter().all(|t| !t.contains("computed_radians")));
        let retried = continue_linked_with(&linked, &mut |m, b| memory.call(m, b), || {
            panic!("exact retry must not consult clock")
        })
        .unwrap();
        assert_eq!(retried, second);
        assert_eq!(memory.records.len(), 4);
        let page_entry =
            shadow_contracts::pd_memory::history::PdMapEntry::from_validated_record(&second)
                .unwrap();
        assert_eq!(
            page_entry.source_kind,
            shadow_contracts::pd_memory::history::PdSourceKind::LinkedScientificDialogue
        );
    }
    #[test]
    fn linked_science_wrong_parent_question_scope_and_operation_do_not_commit() {
        let original = fixture();
        let mut memory = Memory::default();
        let first = memory.run(&original, 100).unwrap();
        let linked = linked_fixture(&original, &first, "linked-2");
        let mut wrong = linked.clone();
        wrong.parent_question_sha256[0] ^= 1;
        assert!(
            continue_linked_with(&wrong, &mut |m, b| memory.call(m, b), || panic!(
                "bad parent before clock"
            ))
            .is_err()
        );
        let mut wrong = linked.clone();
        let mut query = science_domains::decode_query(&wrong.science_query).unwrap();
        query.scope_id.push_str("-foreign");
        wrong.science_query = science_domains::encode_query(&query).unwrap();
        assert!(continue_linked_with(&wrong, &mut |m, b| memory.call(m, b), || Ok(101)).is_err());
        assert_eq!(memory.records.len(), 1);
        continue_linked_with(&linked, &mut |m, b| memory.call(m, b), || Ok(101)).unwrap();
        let mut changed = linked.clone();
        changed.relevance_statement.push_str("-changed");
        assert!(
            continue_linked_with(&changed, &mut |m, b| memory.call(m, b), || panic!(
                "conflict before clock"
            ))
            .is_err()
        );
        let mut stale = linked.clone();
        stale.dialogue.operation_id = "linked-stale".into();
        assert!(
            continue_linked_with(&stale, &mut |m, b| memory.call(m, b), || panic!(
                "stale before clock"
            ))
            .is_err()
        );
        assert_eq!(memory.records.len(), 2);
    }
}
