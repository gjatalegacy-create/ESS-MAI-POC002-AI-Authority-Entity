//! Light resumes an explicit Vault question reference.
//! It transmits actual user fields; only Quantum runs PD. pd_mp indexes scoped
//! Vault records, and no user statement becomes accepted knowledge here.
use crate::light_coordinator::LightResponse;
use crate::pd_operational_bridge::{self as bridge, NativePeer};
use sha2::{Digest, Sha256};
use shadow_contracts::pd_memory::linked_science::{self, LinkedScienceInput};
use shadow_contracts::pd_memory::{
    self as pd,
    failure::{self, FailureDialogueInput},
};
use std::path::Path;
const INITIAL_OPERATION: &str = "initial-asht-refusal";
const ANSWER_JSON_LIMIT: usize = 48 * 1024;

fn identity(response: &LightResponse) -> Result<pd::PdIdentity, String> {
    let lineage = response.input_sha256.to_ascii_lowercase();
    pd::parse_hash(&lineage)?;
    let value = pd::PdIdentity {
        owner: "native-light-runtime".into(),
        obligation: "ASHT exact positive-negative distinction".into(),
        case_id: response.trace_id.clone(),
        scope: response.domain.clone(),
        domain: response.domain.clone(),
        lineage_root: lineage,
    };
    value.validate()?;
    Ok(value)
}
fn no_verdict(record: &pd::PdOperationRecord) -> Result<(), String> {
    record.validate()?;
    failure::validate_failure_operation(&record.request)?;
    if record.request.source_input.starts_with(b"PDS1") {
        let linked = linked_science::decode_linked_input(&record.request.source_input)?;
        let query =
            science_domains::decode_query(&linked.science_query).map_err(|e| e.to_string())?;
        let candidate = science_domains::decode_candidate(&record.request.candidate)
            .map_err(|e| e.to_string())?;
        if query.adapter != science_domains::AdapterId::Astronomy
            || query.session_id != pd::hex(&record.request.identity.key()?)
            || query.case_id != record.request.identity.case_id
            || query.scope_id != record.request.identity.scope
            || query.obligation_id != record.request.identity.obligation
            || candidate.query != query
            || candidate.query_sha256 != record.request.candidate_key
        {
            return Err("LIGHT_PD_SCIENCE_CANDIDATE_BINDING".into());
        }
    }
    Ok(())
}
fn validate_recovered(
    expected: &pd::PdIdentity,
    text: &str,
    record: &pd::PdOperationRecord,
) -> Result<(), String> {
    no_verdict(record)?;
    let input = linked_science::decode_dialogue_source(&record.request.source_input)?;
    if &record.request.identity != expected
        || record.request.operation_id != INITIAL_OPERATION
        || input.user_text != text
        || input.in_reply_to != [0; 32]
    {
        return Err("LIGHT_PD_REFUSAL_BINDING".into());
    }
    Ok(())
}
fn recover_with(
    expected: &pd::PdIdentity,
    text: &str,
    transport: &mut impl FnMut(&str, &[u8]) -> Result<Vec<u8>, String>,
) -> Result<Option<pd::PdOperationRecord>, String> {
    expected.validate()?;
    let lookup = pd::PdLookup {
        identity: expected.clone(),
        operation_id: Some(INITIAL_OPERATION.into()),
    };
    let bytes = transport("--pd-head-once", &pd::encode_lookup(&lookup)?)?;
    let record = pd::decode_optional_record(&bytes)?;
    if let Some(record) = record.as_ref() {
        validate_recovered(expected, text, record)?;
    }
    Ok(record)
}
/// Read the exact committed initial refusal by the original Light trace.
/// A missing record is not invented evidence, and a timeout is not a refusal.
pub fn recover_native_refusal(
    response: &LightResponse,
    text: &str,
) -> Result<Option<pd::PdOperationRecord>, String> {
    let expected = identity(response)?;
    recover_with(&expected, text, &mut |mode, bytes| {
        bridge::native_exchange(NativePeer::Shadow, mode, bytes, pd::MAX_REQUEST_BYTES + 256)
    })
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Answer {
    operation_id: String,
    positive: String,
    negative: String,
    distinction: String,
    source_reference: String,
}
fn text_bound(value: &str, limit: usize) -> Result<(), String> {
    if value.trim().is_empty() || value.len() > limit || value.contains('\0') {
        return Err("LIGHT_PD_ANSWER_TEXT_LIMIT_OR_EMPTY".into());
    }
    Ok(())
}
fn parse_answer(bytes: &[u8]) -> Result<Answer, String> {
    if bytes.len() > ANSWER_JSON_LIMIT {
        return Err("LIGHT_PD_ANSWER_JSON_LIMIT".into());
    }
    let answer: Answer =
        serde_json::from_slice(bytes).map_err(|e| format!("LIGHT_PD_ANSWER_JSON:{e}"))?;
    text_bound(&answer.operation_id, 1024)?;
    for value in [
        &answer.positive,
        &answer.negative,
        &answer.distinction,
        &answer.source_reference,
    ] {
        text_bound(value, pd::MAX_TEXT_BYTES)?;
    }
    Ok(answer)
}
fn answer_input(
    parent: &pd::PdOperationRecord,
    operation_id: &str,
    positive: &str,
    negative: &str,
    distinction: &str,
    source_reference: &str,
) -> Result<FailureDialogueInput, String> {
    no_verdict(parent)?;
    text_bound(operation_id, 1024)?;
    if operation_id == parent.request.operation_id {
        return Err("LIGHT_PD_ANSWER_REUSES_PARENT_OPERATION".into());
    }
    for value in [positive, negative, distinction, source_reference] {
        text_bound(value, pd::MAX_TEXT_BYTES)?;
    }
    let mut input = linked_science::decode_dialogue_source(&parent.request.source_input)?;
    input.operation_id = operation_id.into();
    input.in_reply_to = parent.record_id;
    input.positive = positive.into();
    input.negative = negative.into();
    input.distinction = distinction.into();
    input.source_reference = source_reference.into();
    // Every byte with semantic content comes from explicit user fields. Only
    // newline delimiters are inserted; no fabricated dialogue or labels enter PD.
    input.user_text = [positive, negative, distinction, source_reference].join("\n");
    input.validate()?;
    Ok(input)
}
/// The parent is an explicit persisted question reference, never a new trace
/// guessed from the answer text. Quantum enforces current-head CAS and replay.
pub fn answer_record(
    parent: &pd::PdOperationRecord,
    operation_id: &str,
    positive: &str,
    negative: &str,
    distinction: &str,
    source_reference: &str,
) -> Result<pd::PdOperationRecord, String> {
    let input = answer_input(
        parent,
        operation_id,
        positive,
        negative,
        distinction,
        source_reference,
    )?;
    let encoded = failure::encode_failure_input(&input)?;
    send_dialogue(parent, operation_id, &encoded)
}
fn send_dialogue(
    parent: &pd::PdOperationRecord,
    operation_id: &str,
    encoded: &[u8],
) -> Result<pd::PdOperationRecord, String> {
    let bytes = bridge::native_exchange(
        NativePeer::Quantum,
        "--pd-failure-once",
        encoded,
        pd::MAX_REQUEST_BYTES + 256,
    )?;
    let record = pd::decode_record(&bytes)?;
    no_verdict(&record)?;
    if record.request.source_input.as_slice() != encoded
        || record.request.identity != parent.request.identity
        || record.request.operation_id != operation_id
        || record.previous_record != parent.record_id
        || record.request.expected_revision != parent.revision
        || record.request.previous_state != parent.state_digest
    {
        return Err("LIGHT_PD_ANSWER_RETURN_BINDING".into());
    }
    Ok(record)
}
/// Explicit provider parameters; every numeric value comes from the user.
/// Preserve the decimal token until Rust performs its correctly rounded f64
/// conversion. Default serde_json f64 parsing can move a boundary by one ULP.
/// RawValue is scoped to these input fields; historical checkpoint parsing is
/// unchanged. Source guards still reject nonfinite and out-of-domain values.
fn exact_science_f64<'de,D: serde::Deserializer<'de>>(deserializer:D)->Result<f64,D::Error> {
    let raw=<Box<serde_json::value::RawValue> as serde::Deserialize>::deserialize(deserializer)?;
    raw.get().parse::<f64>().map_err(serde::de::Error::custom)
}
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EraQuery {
    task: String,
    time_scale: String,
    angle_unit: String,
    #[serde(deserialize_with="exact_science_f64")]
    jd_part1: f64,
    #[serde(deserialize_with="exact_science_f64")]
    jd_part2: f64,
    #[serde(deserialize_with="exact_science_f64")]
    claim_radians: f64,
    #[serde(deserialize_with="exact_science_f64")]
    tolerance_radians: f64,
    input_source_id: String,
    query_id: String,
    relevance_statement: String,
}
impl EraQuery {
    pub(crate) fn bind(self, identity:&pd::PdIdentity)->Result<(Vec<u8>,String),String> {
    if self.task != "earth_rotation_angle"
        || self.time_scale != "UT1"
        || self.angle_unit != "radian"
    {
        return Err("LIGHT_PD_SCIENCE_TASK_SCALE_UNIT".into());
    }
    let query = science_domains::ScienceQuery {
        adapter: science_domains::AdapterId::Astronomy,
        session_id: pd::hex(&identity.key()?),
        case_id: identity.case_id.clone(),
        scope_id: identity.scope.clone(),
        obligation_id: identity.obligation.clone(),
        query_id: self.query_id,
        input_source_id: self.input_source_id,
        task: science_domains::ScienceTask::EarthRotationAngle {
            jd_part1: self.jd_part1,
            jd_part2: self.jd_part2,
            time_scale: science_domains::TimeScale::Ut1,
            claim_radians: self.claim_radians,
            tolerance_radians: self.tolerance_radians,
        },
    };
        Ok((science_domains::encode_query(&query).map_err(|e|e.to_string())?,
            self.relevance_statement))
    }
}
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ScienceAnswer {
    operation_id: String,
    positive: String,
    negative: String,
    distinction: String,
    source_reference: String,
    scientific_query: EraQuery,
}
fn linked_input(
    parent: &pd::PdOperationRecord,
    bytes: &[u8],
) -> Result<LinkedScienceInput, String> {
    if bytes.len() > ANSWER_JSON_LIMIT {
        return Err("LIGHT_PD_ANSWER_JSON_LIMIT".into());
    }
    let answer: ScienceAnswer =
        serde_json::from_slice(bytes).map_err(|e| format!("LIGHT_PD_SCIENCE_JSON:{e}"))?;
    let input = answer_input(
        parent,
        &answer.operation_id,
        &answer.positive,
        &answer.negative,
        &answer.distinction,
        &answer.source_reference,
    )?;
    let supplied = answer.scientific_query;
    let (science_query,relevance_statement)=supplied.bind(&parent.request.identity)?;
    let linked = LinkedScienceInput {
        dialogue: input,
        parent_question_sha256: Sha256::digest(parent.request.scoped_question.as_bytes()).into(),
        science_query,
        relevance_statement,
    };
    linked_science::validate_linked_parent(&linked, parent)?;
    Ok(linked)
}
/// Same native port and same parent identity; the new wrapper only carries data.
pub fn science_answer_json(
    parent: &pd::PdOperationRecord,
    bytes: &[u8],
) -> Result<pd::PdOperationRecord, String> {
    let linked = linked_input(parent, bytes)?;
    let encoded = linked_science::encode_linked_input(&linked)?;
    send_dialogue(parent, &linked.dialogue.operation_id, &encoded)
}

/// Usage: light-platform --pd-answer-once PARENT_RECORD ANSWER_JSON OUTPUT_RECORD
/// ANSWER_JSON has exactly operation_id,positive,negative,distinction,source_reference.
pub fn dispatch(args: &[String]) -> Option<i32> {
    let science = match args.get(1).map(String::as_str) {
        Some("--pd-answer-once") => false,
        Some("--pd-science-answer-once") => true,
        _ => return None,
    };
    if args.len() != 5 {
        return Some(64);
    }
    let result = (|| {
        let output = Path::new(&args[4]);
        if output.exists() {
            return Err("PD_RESPONSE_EXISTS".into());
        }
        let parent = pd::decode_record(&bridge::read_bounded(
            Path::new(&args[2]),
            pd::MAX_REQUEST_BYTES + 256,
        )?)?;
        let answer_bytes = bridge::read_bounded(Path::new(&args[3]), ANSWER_JSON_LIMIT)?;
        let record = if science {
            science_answer_json(&parent, &answer_bytes)?
        } else {
            let answer = parse_answer(&answer_bytes)?;
            answer_record(
                &parent,
                &answer.operation_id,
                &answer.positive,
                &answer.negative,
                &answer.distinction,
                &answer.source_reference,
            )?
        };
        bridge::publish_new(output, &pd::encode_record(&record)?)?;
        bridge::print_operational(&record)
    })();
    Some(match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("[LIGHT/PD/ANSWER] NoVerdict: {e}");
            70
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use failure::MISSING_POSITIVE;
    use pd::PdIdentity;
    use sha2::{Digest, Sha256};
    use shadow_contracts::*;
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
            operation_id: INITIAL_OPERATION.into(),
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

    // Data-only record fixture. No actual judgement or durable storage is claimed.
    fn record(input: &FailureDialogueInput) -> pd::PdOperationRecord {
        let source = failure::encode_failure_input(input).unwrap();
        let hash: pd::Hash = Sha256::digest(&source).into();
        let step =
            pd::LgcStep::new(0, hash, "D-BAB", "fixture missing condition", [2; 32]).unwrap();
        let request = pd::PdOperationRequest {
            identity: input.identity.clone(),
            operation_id: input.operation_id.clone(),
            source_input: source,
            expected_revision: 0,
            previous_state: [0; 32],
            // Envelope-only fixture: the native Quantum checkpoint is validated
            // by Shadow, not reconstructed by this transmission frontend test.
            checkpoint: [failure::FAILURE_CHECKPOINT_PREFIX, &[1]].concat(),
            scoped_question: "Pyetje e kufizuar NoVerdict".into(),
            candidate: vec![],
            candidate_key: [0; 32],
            witness: pd::ConditionWitness {
                predecessor: "fixture ASHT".into(),
                parent_authority: "fixture source authority".into(),
                failed_action: "exact missing pair".into(),
                condition_code: "MISSING_EXACT_POSITIVE".into(),
                reason: input.refusal.clone(),
                source_component: "test fixture".into(),
                source_sha256: [1; 32],
                attempt_trace: input.operation_id.clone(),
                input_sha256: hash,
                lgc_parent: step.step_digest,
                report_class: pd::ReportClass::MissingConditions,
                permitted_next: pd::NextAction::AcquireScopedEvidence,
            },
            lgc_steps: vec![step],
        };
        pd::PdOperationRecord::construct(request, [0; 32]).unwrap()
    }
    #[test]
    fn recover_requests_exact_initial_operation_and_checks_text_and_entire_scope() {
        let input = fixture();
        let saved = record(&input);
        let actual = recover_with(&input.identity, &input.user_text, &mut |mode, bytes| {
            assert_eq!(mode, "--pd-head-once");
            let lookup = pd::decode_lookup(bytes).unwrap();
            assert_eq!(lookup.identity, input.identity);
            assert_eq!(lookup.operation_id.as_deref(), Some(INITIAL_OPERATION));
            pd::encode_optional_record(Some(&saved))
        })
        .unwrap();
        assert_eq!(actual, Some(saved.clone()));
        assert!(validate_recovered(&input.identity, "other actual user text", &saved).is_err());
        for field in 0..6 {
            let mut other = input.identity.clone();
            match field {
                0 => other.owner.push('x'),
                1 => other.obligation.push('x'),
                2 => other.case_id.push('x'),
                3 => other.scope.push('x'),
                4 => other.domain.push('x'),
                _ => other.lineage_root.push('x'),
            };
            assert!(validate_recovered(&other, &input.user_text, &saved).is_err());
        }
        let missing = recover_with(&input.identity, &input.user_text, &mut |_, _| {
            pd::encode_optional_record(None)
        })
        .unwrap();
        assert_eq!(missing, None);
    }
    #[test]
    fn answer_preserves_original_selection_and_explicit_question_parent() {
        let original = fixture();
        let saved = record(&original);
        let answer = answer_input(
            &saved,
            "answer-2",
            "support supplied",
            "counter supplied",
            "difference supplied",
            "source supplied",
        )
        .unwrap();
        assert_eq!(answer.in_reply_to, saved.record_id);
        assert_eq!(answer.identity, original.identity);
        assert_eq!(answer.selection_request, original.selection_request);
        assert_eq!(answer.selection_response, original.selection_response);
        assert_eq!(answer.refusal, original.refusal);
        assert_eq!(
            answer.user_text,
            "support supplied\ncounter supplied\ndifference supplied\nsource supplied"
        );
        assert!(answer_input(&saved, INITIAL_OPERATION, "s", "c", "d", "r").is_err());
        assert!(answer_input(&saved, "answer-2", "s", "", "d", "r").is_err());
    }
    #[test]
    fn json_requires_all_explicit_fields_and_rejects_unknown_duplicate_oversize() {
        let good=br#"{"operation_id":"answer-2","positive":"p","negative":"n","distinction":"d","source_reference":"s"}"#;
        let parsed = parse_answer(good).unwrap();
        assert_eq!(parsed.distinction, "d");
        for invalid in [
            br#"{"operation_id":"x","positive":"p","negative":"n","distinction":"d"}"#.as_slice(),
            br#"{"operation_id":"x","positive":"p","negative":"n","distinction":"d","source_reference":"s","verdict":true}"#.as_slice(),
            br#"{"operation_id":"x","positive":"p","positive":"p2","negative":"n","distinction":"d","source_reference":"s"}"#.as_slice(),
            b"not json".as_slice(),
        ]{assert!(parse_answer(invalid).is_err());}
        assert!(parse_answer(&vec![b' '; ANSWER_JSON_LIMIT + 1]).is_err());
        assert_eq!(
            dispatch(&["light".into(), "--pd-answer-once".into()]),
            Some(64)
        );
        assert_eq!(dispatch(&["light".into(), "--other".into()]), None);
    }
    #[test]
    fn transport_error_is_not_fabricated_missing_condition_or_question() {
        let input = fixture();
        let error = recover_with(&input.identity, &input.user_text, &mut |_, _| {
            Err("actual process timeout".into())
        })
        .unwrap_err();
        assert_eq!(error, "actual process timeout");
        assert!(
            bridge::native_exchange(NativePeer::Shadow, "--pd-commit-once", b"", 10)
                .unwrap_err()
                .contains("ARGUMENT_LIMIT")
        );
    }

    #[test]
    fn science_json_requires_explicit_parameters_and_preserves_parent_identity() {
        // IAU conformance values: regression input only, no production defaults.
        let saved = record(&fixture());
        let object = serde_json::json!({
            "operation_id":"science-2","positive":"p reported","negative":"n reported",
            "distinction":"d reported","source_reference":"user supplied source",
            "scientific_query":{"task":"earth_rotation_angle","time_scale":"UT1","angle_unit":"radian",
                "jd_part1":2400000.5,"jd_part2":54388.0,"claim_radians":0.4022837240028158,
                "tolerance_radians":1e-12,"input_source_id":"IAU regression fixture only",
                "query_id":"era-query","relevance_statement":"User-proposed subquestion; no admitted relevance"}
        });
        let linked = linked_input(&saved, &serde_json::to_vec(&object).unwrap()).unwrap();
        assert_eq!(linked.dialogue.identity, saved.request.identity);
        assert_eq!(linked.dialogue.in_reply_to, saved.record_id);
        assert_eq!(
            linked.parent_question_sha256,
            <[u8; 32]>::from(Sha256::digest(saved.request.scoped_question.as_bytes()))
        );
        let query = science_domains::decode_query(&linked.science_query).unwrap();
        assert_eq!(query.adapter, science_domains::AdapterId::Astronomy);
        assert_eq!(
            query.session_id,
            pd::hex(&saved.request.identity.key().unwrap())
        );
        for field in [
            "jd_part1",
            "jd_part2",
            "claim_radians",
            "tolerance_radians",
            "input_source_id",
            "time_scale",
        ] {
            let mut bad = object.clone();
            bad["scientific_query"]
                .as_object_mut()
                .unwrap()
                .remove(field);
            assert!(linked_input(&saved, &serde_json::to_vec(&bad).unwrap()).is_err());
        }
        for field in ["task", "time_scale", "angle_unit"] {
            let mut bad = object.clone();
            bad["scientific_query"][field] = serde_json::json!("wrong");
            assert!(linked_input(&saved, &serde_json::to_vec(&bad).unwrap()).is_err());
        }
        let mut bad = object.clone();
        bad["scientific_query"]["tolerance_radians"] = serde_json::json!(0);
        assert!(linked_input(&saved, &serde_json::to_vec(&bad).unwrap()).is_err());
        let mut bad = object;
        bad["scientific_query"]["authority"] = serde_json::json!(true);
        assert!(linked_input(&saved, &serde_json::to_vec(&bad).unwrap()).is_err());
        assert_eq!(
            dispatch(&["light".into(), "--pd-science-answer-once".into()]),
            Some(64)
        );
    }
}
