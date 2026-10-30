//! Explicit PIZ2 -> PDS2 acquisition, reusing the single native PD.
//! No fresh ASHT outcome is invented; the real original deficit remains in Vault.
use crate::progressive_debatic::{types::PD_SEAL, ProgressiveDebatic};
use sha2::{Digest, Sha256};
use shadow_contracts::pd_memory::{self as pd, iz, iz_science as bridge, *};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
fn sha(b: &[u8]) -> Hash {
    Sha256::digest(b).into()
}
fn pulse<T>(
    site: &str,
    bytes: &[u8],
    effect: impl FnOnce(&[u8]) -> Result<T, String>,
) -> Result<T, String> {
    eprintln!("[PDS2/0-NOI] {site}");
    let triggered = ess_mai_zero_noi::trigger(bytes);
    effect(triggered.request())
}
fn output_question(input: &bridge::IzScienceInput, actual: &science_domains::CandidateEvidence) -> Result<String, String> {
    if input.question_policy == bridge::QuestionPolicy::ComputedV1 {
        let science_domains::ScienceTask::EarthRotationAngle { claim_radians, tolerance_radians, .. } = actual.query.task;
        pd::iz_science_math::computed_question(input, &pd::iz_science_math::EraComparison {
            query_sha256: actual.query_sha256, result_sha256: actual.result_sha256,
            model_sha256: actual.provenance.compiled_model_sha256,
            claim_radians, computed_radians: actual.computed_radians,
            residual_radians: actual.circular_residual_radians, tolerance_radians,
        })
    } else { bridge::question(input) }
}
/// Injectable I/O is for regression verification, not a substitute authority.
pub fn continue_with(
    input: &bridge::IzScienceInput,
    transport: &mut impl FnMut(&str, &[u8]) -> Result<Vec<u8>, String>,
    clock: impl FnOnce() -> Result<u64, String>,
) -> Result<PdOperationRecord, String> {
    let source = bridge::encode_input(input)?;
    let exact = PdLookup {
        identity: input.context.identity.clone(),
        operation_id: Some(input.context.operation_id.clone()),
    };
    let old = pulse(
        "exact operation before clock or ingest",
        &pd::encode_lookup(&exact)?,
        |b| pd::decode_optional_record(&transport("--pd-head-once", b)?),
    )?;
    if let Some(record) = old {
        record.validate()?;
        if record.request.identity != input.context.identity
            || record.request.operation_id != input.context.operation_id
            || record.request.source_input != source
        {
            return Err("PDS2_OPERATION_CONFLICT".into());
        }
        bridge::validate_operation(&record.request)?;
        return Ok(record);
    }
    let lookup = PdLookup {
        identity: input.context.identity.clone(),
        operation_id: None,
    };
    let parent = pulse(
        "resolve current same-Vault scoped question",
        &pd::encode_lookup(&lookup)?,
        |b| pd::decode_optional_record(&transport("--pd-head-once", b)?),
    )?
    .ok_or("PDS2_PARENT_MISSING")?;
    bridge::validate_parent(input, &parent)?;
    let prior = iz::validate_operation(&parent.request)?;
    pulse(
        "D-BAB retain exact actual missing ASHT observation",
        &parent.request.source_input,
        |_| Ok(()),
    )?;
    let raw = parent
        .request
        .checkpoint
        .strip_prefix(prior.context.checkpoint_prefix())
        .ok_or("PDS2_PARENT_CHECKPOINT_POLICY")?;
    let session = pd::hex(&input.context.identity.key()?);
    let mut engine = ProgressiveDebatic::new();
    pulse("restore actual scoped PD", raw, |b| {
        engine.import_checkpoint(&session, b)
    })?;
    let before = engine.session(&session).ok_or("PDS2_SESSION_MISSING")?;
    if !engine.active
        || !before.active
        || before.territory != input.context.identity.scope
        || before.total_turns >= 4096
    {
        return Err("PDS2_SESSION_SCOPE_ACTIVITY_LIMIT".into());
    }
    let now = clock()?;
    if now < before.started_at || now < before.last_activity_at {
        return Err("PDS2_CLOCK_REGRESSION".into());
    }
    let query = science_domains::decode_query(&input.science_query)
        .map_err(|e| format!("PDS2_QUERY:{e}"))?;
    if query.session_id != session
        || query.case_id != input.context.identity.case_id
        || query.scope_id != input.context.identity.scope
        || query.obligation_id != input.context.identity.obligation
    {
        return Err("PDS2_QUERY_SCOPE".into());
    }
    let candidate = pulse(
        "evaluate explicit offline scoped query",
        &input.science_query,
        |_| science_domains::evaluate(&query).map_err(|e| format!("PDS2_PROVIDER:{e}")),
    )?;
    let candidate_bytes =
        science_domains::encode_candidate(&candidate).map_err(|e| e.to_string())?;
    let scoped_question = pulse("assemble actual result and evidence question", &candidate_bytes,
        |_| output_question(input, &candidate))?;
    pulse(
        "ingest actual user i0 once",
        input.context.user_text.as_bytes(),
        |_| {
            engine
                .ingest_sealed(&session, &input.context.user_text, now, PD_SEAL)
                .map(|_| ())
                .map_err(|e| format!("PDS2_INGEST:{e:?}"))
        },
    )?;
    let inner = pulse("export same PD checkpoint", session.as_bytes(), |_| {
        engine.export_checkpoint(&session)
    })?;
    let mut checkpoint = input.checkpoint_prefix().to_vec();
    checkpoint.extend_from_slice(&inner);
    if checkpoint.len() > pd::MAX_CHECKPOINT_BYTES {
        return Err("PDS2_CHECKPOINT_LIMIT".into());
    }
    let steps = bridge::build_steps_with_question(input, &checkpoint, &candidate_bytes, &scoped_question)?;
    let request = PdOperationRequest {
        identity: input.context.identity.clone(),
        operation_id: input.context.operation_id.clone(),
        source_input: source,
        expected_revision: parent.revision,
        previous_state: parent.state_digest,
        witness: bridge::build_witness(
            input,
            &steps,
            sha(include_bytes!(
                "../../shadow-contracts/src/pd_memory/iz_science.rs"
            )),
        )?,
        checkpoint,
        scoped_question,
        candidate: candidate_bytes,
        candidate_key: candidate.query_sha256,
        lgc_steps: steps,
    };
    bridge::validate_transition(&request, &parent)?;
    let encoded = pd::encode_request(&request)?;
    let response = pulse("request same-Vault operational append", &encoded, |b| {
        transport("--pd-commit-once", b)
    })
    .map_err(|e| format!("PDS2_COMMIT_STATUS_UNCERTAIN:{e}"))?;
    let record = pd::decode_record(&response)?;
    if record.request != request || record.previous_record != parent.record_id {
        return Err("PDS2_COMMIT_RETURNED_DIFFERENT_RECORD".into());
    }
    let readback = pulse(
        "read exact durable operation",
        &pd::encode_lookup(&exact)?,
        |b| pd::decode_optional_record(&transport("--pd-head-once", b)?),
    )?
    .ok_or("PDS2_READBACK_MISSING")?;
    if readback != record {
        return Err("PDS2_READBACK_MISMATCH".into());
    }
    eprintln!(
        "[PDS2] record={} revision={} OperationalNoVerdict knowledge_write=0",
        pd::hex(&record.record_id),
        record.revision
    );
    Ok(record)
}
pub fn continue_native(input: &bridge::IzScienceInput) -> Result<PdOperationRecord, String> {
    let start = Instant::now();
    continue_with(
        input,
        &mut |mode, bytes| {
            let remaining = Duration::from_secs(90)
                .checked_sub(start.elapsed())
                .ok_or("PDS2_DEADLINE")?;
            crate::pd_operational::native_transport(mode, bytes, remaining)
        },
        || {
            let ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_millis();
            u64::try_from(ms).map_err(|_| "PDS2_CLOCK_OVERFLOW".into())
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use shadow_contracts::{
        KnowledgeSelectionRequestWire, KnowledgeSelectionResponseWire, SelectionPurpose,
        SelectionRequester, PROTOCOL_VERSION, SELECTION_MAX_CANDIDATES,
    };
    #[derive(Default)]
    struct Memory {
        records: Vec<PdOperationRecord>,
        lose_reply: bool,
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
                    let parent = self.records.last();
                    if bridge::source_is_science(&q.source_input) {
                        bridge::validate_transition(&q, parent.ok_or("test parent missing")?)?;
                    } else {
                        iz::validate_parent(&iz::decode_source(&q.source_input)?, parent)?;
                    }
                    let saved = PdOperationRecord::construct(
                        q,
                        parent.map(|r| r.record_id).unwrap_or([0; 32]),
                    )?;
                    self.records.push(saved.clone());
                    if self.lose_reply {
                        return Err("synthetic lost transport reply".into());
                    }
                    pd::encode_record(&saved)
                }
                _ => Err("unexpected synthetic port".into()),
            }
        }
    }
    // The selection bytes below are SYNTHETIC contract data, never authentic
    // native holds or accepted evidence. The PD/checkpoint computation is real.
    fn first() -> (Memory, iz::IzContext, PdOperationRecord) {
        let origin = "Explain alpha and beta in the mathematical model.".to_string();
        let context = iz::IzContext {
            version: 2,
            identity: PdIdentity {
                owner: "test-owner".into(),
                obligation: "model distinction".into(),
                case_id: "logical-case".into(),
                scope: "WORK→NURA".into(),
                domain: "WORK→NURA".into(),
                lineage_root: pd::hex(&sha(origin.as_bytes())),
            },
            operation_id: "iz-first".into(),
            origin_i0: origin.clone(),
            user_text: origin,
            parent: None,
            answer: iz::IzAnswer::default(),
            map: None,
            native_anchor: None,
            native_map: None,
        };
        let mut memory = Memory::default();
        let ready =
            match crate::pd_iz::prepare_with(&context, &mut |m, b| memory.call(m, b)).unwrap() {
                crate::pd_iz::Preparation::Ready(v) => v,
                _ => panic!("fresh fixture"),
            };
        let q = KnowledgeSelectionRequestWire {
            version: PROTOCOL_VERSION,
            requester: SelectionRequester::AshtQuantum,
            purpose: SelectionPurpose::EvidenceCandidates,
            session_id: "physical-fixture".into(),
            trace_id: "physical-fixture".into(),
            parent_i0: pd::hex(&sha(context.user_text.as_bytes())),
            primitive_anchor: 7,
            split_sha256: "11".repeat(32),
            xi: vec!["alpha".into()],
            yi: vec!["beta".into()],
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
            primitive_anchor: 7,
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
            hold_id: "synthetic-only".into(),
            hold_sha256: "44".repeat(32),
            positive: vec![],
            negative: vec![],
            law_seal: 1,
            system_laws_seal: 2,
            expires_at_ns: 3,
        };
        let active = crate::pd_iz::before_action(&ready, request).unwrap();
        let parent = crate::pd_iz::finish_with(
            ready,
            active,
            shadow_contracts::encode_selection_response(&response),
            pd::failure::MISSING_POSITIVE.into(),
            &mut |m, b| memory.call(m, b),
            || Ok(1000),
        )
        .unwrap();
        (memory, context, parent)
    }
    fn input(context: &iz::IzContext, parent: &PdOperationRecord) -> bridge::IzScienceInput {
        let mut c = context.clone();
        c.operation_id = "science-answer".into();
        c.user_text =
            "I explicitly request the Earth rotation model for the supplied UT1 date.".into();
        c.parent = Some(iz::IzParent::from_record(parent).unwrap());
        // Published IAU conformance input is only a regression vector here.
        let q = science_domains::ScienceQuery {
            adapter: science_domains::AdapterId::Astronomy,
            session_id: pd::hex(&c.identity.key().unwrap()),
            case_id: c.identity.case_id.clone(),
            scope_id: c.identity.scope.clone(),
            obligation_id: c.identity.obligation.clone(),
            query_id: "test-query".into(),
            input_source_id: "IAU regression vector, not observation".into(),
            task: science_domains::ScienceTask::EarthRotationAngle {
                jd_part1: 2400000.5,
                jd_part2: 54388.0,
                time_scale: science_domains::TimeScale::Ut1,
                claim_radians: 0.4022837240028158,
                tolerance_radians: 1e-12,
            },
        };
        bridge::from_parent(
            c,
            parent,
            science_domains::encode_query(&q).unwrap(),
            "User-proposed conditional model subquestion; relevance unadjudicated".into(),
        )
        .unwrap()
    }
    #[test]
    fn r19_science_iz_contains_actual_result_and_discriminating_question() {
        let (mut memory, context, parent) = first();
        let mut value = input(&context, &parent);
        value.question_policy = bridge::QuestionPolicy::ComputedV1;
        let record = continue_with(&value, &mut |m, b| memory.call(m, b), || Ok(1001)).unwrap();
        assert!(record.request.scoped_question.contains("MODEL_RESULT"),
            "iZ lacks the actual computed result: {}", record.request.scoped_question);
        assert!(record.request.scoped_question.contains("KiS=1 KiC=0"));
        assert!(record.request.scoped_question.contains("residual_rad="));
        assert!(record.request.source_input.starts_with(b"PDS4"));
        assert_eq!(record.request.lgc_steps[5].payload_digest,
            sha(record.request.scoped_question.as_bytes()));
        let mut restored = ProgressiveDebatic::new();
        let session = pd::hex(&context.identity.key().unwrap());
        restored.import_checkpoint(&session, bridge::science_checkpoint_raw(&record.request).unwrap()).unwrap();
        let texts: Vec<_> = restored.session(&session).unwrap().trace.signals.iter()
            .map(|s| s.raw_content.as_str()).collect();
        assert_eq!(texts, vec![context.user_text.as_str(), value.context.user_text.as_str()]);
        assert!(!texts.contains(&record.request.scoped_question.as_str()));
        let replay = continue_with(&value, &mut |m,b| memory.call(m,b), || panic!("retry must not read clock")).unwrap();
        assert_eq!(pd::encode_record(&replay).unwrap(), pd::encode_record(&record).unwrap());
        assert_eq!(memory.records.len(), 2);
    }
    #[test]
    fn one_way_bridge_preserves_actual_history_then_exposes_science_to_existing_kie() {
        let (mut memory, context, parent) = first();
        let input = input(&context, &parent);
        let bytes = bridge::encode_input(&input).unwrap();
        assert_eq!(bridge::decode_input(&bytes).unwrap(), input);
        let record = continue_with(&input, &mut |m, b| memory.call(m, b), || Ok(1001)).unwrap();
        assert_eq!(record.previous_record, parent.record_id);
        assert_eq!(record.revision, parent.revision + 1);
        assert_eq!(record.request.identity, parent.request.identity);
        bridge::validate_transition(&record.request, &parent).unwrap();
        let raw = pd::linked_science::science_checkpoint_raw(&record).unwrap();
        let session = pd::hex(&context.identity.key().unwrap());
        let mut engine = ProgressiveDebatic::new();
        engine.import_checkpoint(&session, raw).unwrap();
        let state = engine.session(&session).unwrap();
        assert_eq!(state.total_turns, 2);
        assert_eq!(state.started_at, 1000);
        let actual: Vec<_> = state
            .trace
            .signals
            .iter()
            .map(|s| s.raw_content.as_str())
            .collect();
        assert_eq!(
            actual,
            vec![context.user_text.as_str(), input.context.user_text.as_str()]
        );
        assert!(!actual.contains(&parent.request.scoped_question.as_str()));
        assert_eq!(
            pd::linked_science::science_query_from_source(&record.request.source_input).unwrap(),
            input.science_query
        );
        let model =
            science_domains::kie::evaluate_kie(&input.science_query, &record.request.candidate)
                .unwrap();
        assert!(model.canonical_proposition.contains("MODEL_CONDITIONAL"));
        let candidate = science_domains::decode_candidate(&record.request.candidate).unwrap();
        assert_eq!(
            candidate.authority,
            science_domains::CandidateAuthority::None
        );
        assert_eq!(
            record.request.witness.report_class,
            ReportClass::MissingConditions
        );
        assert_eq!(
            pd::history::PdMapEntry::from_validated_record(&record)
                .unwrap()
                .source_kind,
            pd::history::PdSourceKind::LinkedScientificDialogue
        );
        assert!(bridge::from_parent(
            input.context.clone(),
            &record,
            input.science_query.clone(),
            "no silent repeated bridge".into()
        )
        .is_err());
    }
    #[test]
    fn exact_retry_precedes_clock_while_conflict_and_stale_parent_never_append() {
        let (mut memory, c, parent) = first();
        let value = input(&c, &parent);
        let saved = continue_with(&value, &mut |m, b| memory.call(m, b), || Ok(1001)).unwrap();
        let replay = continue_with(&value, &mut |m, b| memory.call(m, b), || {
            panic!("retry clock")
        })
        .unwrap();
        assert_eq!(replay, saved);
        let mut changed = value.clone();
        changed.context.user_text.push_str(" changed");
        assert!(
            continue_with(&changed, &mut |m, b| memory.call(m, b), || panic!(
                "conflict clock"
            ))
            .unwrap_err()
            .contains("OPERATION_CONFLICT")
        );
        let mut stale = value;
        stale.context.operation_id = "stale".into();
        assert!(
            continue_with(&stale, &mut |m, b| memory.call(m, b), || panic!(
                "stale clock"
            ))
            .is_err()
        );
        assert_eq!(memory.records.len(), 2);
    }
    #[test]
    fn foreign_scope_parent_question_lgc_and_detached_user_annotation_are_rejected() {
        let (mut memory, c, parent) = first();
        let value = input(&c, &parent);
        for which in 0..4 {
            let mut wrong = value.clone();
            match which {
                0 => wrong.parent_source_sha256[0] ^= 1,
                1 => wrong.prior_lgc_head[0] ^= 1,
                2 => {
                    wrong.context.parent.as_mut().unwrap().question_sha256[0] ^= 1;
                }
                _ => wrong.context.identity.case_id.push('x'),
            }
            assert!(
                continue_with(&wrong, &mut |m, b| memory.call(m, b), || panic!(
                    "bad parent before clock"
                ))
                .is_err()
            );
        }
        let mut wrong = value.clone();
        wrong.context.answer.positive = "not supplied".into();
        assert!(bridge::encode_input(&wrong).is_err());
        let mut wrong = value;
        let mut q = science_domains::decode_query(&wrong.science_query).unwrap();
        q.scope_id.push('x');
        wrong.science_query = science_domains::encode_query(&q).unwrap();
        assert!(
            continue_with(&wrong, &mut |m, b| memory.call(m, b), || Ok(1001))
                .unwrap_err()
                .contains("QUERY_SCOPE")
        );
        assert_eq!(memory.records.len(), 1);
    }
    #[test]
    fn checkpoint_substitution_or_restarting_cognition_cannot_be_relabelled_as_continuation() {
        let (mut memory, c, parent) = first();
        let value = input(&c, &parent);
        let saved = continue_with(&value, &mut |m, b| memory.call(m, b), || Ok(1001)).unwrap();
        let mut wrong = saved.request.clone();
        // A shape-valid old checkpoint has no new actual turn.
        wrong.checkpoint = [
            pd::failure::FAILURE_CHECKPOINT_PREFIX,
            parent
                .request
                .checkpoint
                .strip_prefix(iz::IZ_CHECKPOINT_PREFIX_V2)
                .unwrap(),
        ]
        .concat();
        wrong.lgc_steps = bridge::build_steps(&value, &wrong.checkpoint, &wrong.candidate).unwrap();
        wrong.witness =
            bridge::build_witness(&value, &wrong.lgc_steps, wrong.witness.source_sha256).unwrap();
        wrong.validate().unwrap();
        assert!(bridge::validate_transition(&wrong, &parent).is_err());
        let mut wrong = saved.request;
        wrong.checkpoint[0] ^= 1;
        assert!(bridge::validate_transition(&wrong, &parent).is_err());
    }
    #[test]
    fn lost_commit_reply_resolves_exact_operation_without_second_ingest() {
        let (mut memory, c, parent) = first();
        let value = input(&c, &parent);
        memory.lose_reply = true;
        assert!(
            continue_with(&value, &mut |m, b| memory.call(m, b), || Ok(1001))
                .unwrap_err()
                .contains("COMMIT_STATUS_UNCERTAIN")
        );
        memory.lose_reply = false;
        let saved = continue_with(&value, &mut |m, b| memory.call(m, b), || {
            panic!("resolve before clock")
        })
        .unwrap();
        assert_eq!(saved.revision, 2);
        assert_eq!(memory.records.len(), 2);
    }
}
