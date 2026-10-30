//! NEW PDF1 native admission, using existing Shadow hold evidence.
//! A stored ASHT selection establishes the origin of a missing-pair report.
//! This is neither NBES attestation nor knowledge/verdict admission. Exact
//! durable retries and parent-bound answers do not reacquire ephemeral holds.
use crate::knowledge_vault::KnowledgeVault;
use crate::selection_hold::SelectionHoldStore;
use shadow_contracts::pd_memory::{failure, PdOperationRequest};
use shadow_contracts::{encode_selection_response, SelectionRequester};
use std::path::PathBuf;

pub(crate) fn validate_native_admission(
    request: &PdOperationRequest,
    vault: &KnowledgeVault,
    handoff_root: impl FnOnce() -> Result<PathBuf, String>,
    now_ns: impl FnOnce() -> Result<u64, String>,
) -> Result<(), String> {
    match request.source_input.get(..4) {
        Some(b"PLF1") => return Err("PD_LATE_REFUSAL_SHADOW_OWNER_REQUIRED".into()),
        Some(b"PIZ1") | Some(b"PIZ2") | Some(b"PIZ3") => return crate::pd_iz_admission::validate_native_admission(request, vault, handoff_root, now_ns),
        Some(b"PDC1") => return validate_operational_admission(request, vault),
        Some(b"PDS2") | Some(b"PDS3") | Some(b"PDS4") | Some(b"PDS5") => return validate_iz_science_admission(request, vault),
        Some(b"PDF1") | Some(b"PDS1") => {}
        _ => return Err("PD_NATIVE_SOURCE_INPUT_VERSION".into()),
    }
    let input = failure::validate_failure_operation(request)?;
    if let Some(prior) = vault
        .pd_operation(&request.identity, &request.operation_id)
        .map_err(|e| format!("PD_FAILURE_EXISTING_LOOKUP:{e:?}"))?
    {
        return if prior.request == *request {
            Ok(())
        } else {
            Err("PD_FAILURE_EXISTING_OPERATION_CONFLICT".into())
        };
    }
    // Exact durable retries returned above. New operations must retain the
    // current Vault-owned cognitive history; no ephemeral hold can waive this.
    let previous = vault
        .pd_head(&request.identity)
        .map_err(|e| format!("PD_FAILURE_PREVIOUS_LOOKUP:{e:?}"))?;
    let (previous_revision, previous_state) = previous
        .as_ref()
        .map(|record| (record.revision, record.state_digest))
        .unwrap_or((0, [0; 32]));
    if request.expected_revision != previous_revision || request.previous_state != previous_state {
        return Err("PD_FAILURE_CHECKPOINT_STALE_HEAD".into());
    }
    let previous_raw = previous
        .as_ref()
        .map(|record| {
            record
                .request
                .checkpoint
                .strip_prefix(failure::FAILURE_CHECKPOINT_PREFIX)
                .ok_or("PD_FAILURE_PREVIOUS_CHECKPOINT_POLICY_REVIEW_REQUIRED")
        })
        .transpose()?;
    let current_raw = request
        .checkpoint
        .strip_prefix(failure::FAILURE_CHECKPOINT_PREFIX)
        .ok_or("PD_FAILURE_CHECKPOINT_POLICY_REVIEW_REQUIRED")?;
    shadow_contracts::pd_checkpoint_data::validate_user_checkpoint_transition(
        current_raw,
        &shadow_contracts::pd_memory::hex(&request.identity.key()?),
        &input.user_text,
        request.expected_revision,
        previous_raw,
    )
    .map_err(|error| format!("PD_FAILURE_CHECKPOINT_TRANSITION:{error}"))?;
    if input.in_reply_to != [0; 32] {
        // KnowledgeVault::commit_pd_operation requires the exact current
        // durable PDF1 parent and preserves its original refusal provenance.
        return if request.expected_revision > 0 {
            Ok(())
        } else {
            Err("PD_FAILURE_REPLY_REVISION".into())
        };
    }
    if request.expected_revision != 0 {
        return Err("PD_FAILURE_INITIAL_REVISION".into());
    }
    let store = SelectionHoldStore::under_handoff(handoff_root()?);
    verify_initial_hold(
        &store,
        &input.selection_request,
        &input.selection_response,
        now_ns()?,
    )
}

// PDC1 uses the same data-only native checkpoint admission as PDF1. The
// provider result remains separate unadmitted material; it cannot replace a
// real user turn or supply a new PD history. Exact historical retries retain
// their original policy bytes; only advancement requires USER_ONLY_V2.
const OPERATIONAL_CHECKPOINT_PREFIX: &[u8] = b"PD_USER_ONLY_V2\0";
fn validate_operational_admission(
    request: &PdOperationRequest,
    vault: &KnowledgeVault,
) -> Result<(), String> {
    request.validate()?; // Exact source identity, operation and raw input SHA.
    let input = shadow_contracts::pd_memory::decode_continuity_input(&request.source_input)?;
    if let Some(prior) = vault
        .pd_operation(&request.identity, &request.operation_id)
        .map_err(|e| format!("PD_OPERATIONAL_EXISTING_LOOKUP:{e:?}"))?
    {
        return if prior.request == *request {
            Ok(())
        } else {
            Err("PD_OPERATIONAL_EXISTING_OPERATION_CONFLICT".into())
        };
    }
    let previous = vault
        .pd_head(&request.identity)
        .map_err(|e| format!("PD_OPERATIONAL_PREVIOUS_LOOKUP:{e:?}"))?;
    let (revision, state) = previous
        .as_ref()
        .map(|record| (record.revision, record.state_digest))
        .unwrap_or((0, [0; 32]));
    if request.expected_revision != revision || request.previous_state != state {
        return Err("PD_OPERATIONAL_CHECKPOINT_STALE_HEAD".into());
    }
    let previous_raw = previous
        .as_ref()
        .map(|record| {
            record
                .request
                .checkpoint
                .strip_prefix(OPERATIONAL_CHECKPOINT_PREFIX)
                .ok_or("PD_OPERATIONAL_PREVIOUS_CHECKPOINT_POLICY_REVIEW_REQUIRED")
        })
        .transpose()?;
    let current_raw = request
        .checkpoint
        .strip_prefix(OPERATIONAL_CHECKPOINT_PREFIX)
        .ok_or("PD_OPERATIONAL_CHECKPOINT_POLICY_REVIEW_REQUIRED")?;
    shadow_contracts::pd_checkpoint_data::validate_user_checkpoint_transition(
        current_raw,
        &shadow_contracts::pd_memory::hex(&request.identity.key()?),
        &input.user_text,
        request.expected_revision,
        previous_raw,
    )
    .map_err(|error| format!("PD_OPERATIONAL_CHECKPOINT_TRANSITION:{error}"))
}

/// A PDS2 record is parent-directed acquisition, not a fresh ASHT selection.
/// Reusing an actual durable parent's hold observation never renews its authority.
fn validate_iz_science_admission(request:&PdOperationRequest,vault:&KnowledgeVault)->Result<(),String>{
    use shadow_contracts::pd_memory::iz_science;
    request.validate()?;
    if let Some(prior)=vault.pd_operation(&request.identity,&request.operation_id)
        .map_err(|e|format!("PDS2_EXISTING_LOOKUP:{e:?}"))? {
        return if prior.request==*request{Ok(())}else{Err("PDS2_OPERATION_CONFLICT".into())}
    }
    let parent=vault.pd_head(&request.identity).map_err(|e|format!("PDS2_PARENT_LOOKUP:{e:?}"))?
        .ok_or("PDS2_PARENT_MISSING")?;
    iz_science::validate_transition(request,&parent)?;
    if iz_science::source_is_native_science(&request.source_input) {
        vault.validate_pd_native_inquiry(request).map_err(|e| format!("PDS3_NATIVE_HISTORY:{e:?}"))?;
    }
    validate_pds2_candidate(request)
}
pub(crate) fn validate_pds2_candidate(request:&PdOperationRequest)->Result<(),String>{
    use shadow_contracts::pd_memory as pd;
    let input=pd::iz_science::validate_operation(request)?;
    let query=science_domains::decode_query(&input.science_query).map_err(|e|format!("PDS2_QUERY:{e}"))?;
    if query.session_id!=pd::hex(&request.identity.key()?) || query.case_id!=request.identity.case_id
        || query.scope_id!=request.identity.scope || query.obligation_id!=request.identity.obligation {
        return Err("PDS2_QUERY_SCOPE".into())
    }
    let actual=science_domains::evaluate(&query).map_err(|e|format!("PDS2_PROVIDER:{e}"))?;
    let bytes=science_domains::encode_candidate(&actual).map_err(|e|format!("PDS2_CANDIDATE:{e}"))?;
    if bytes!=request.candidate || actual.query_sha256!=request.candidate_key {
        return Err("PDS2_CANDIDATE_BINDING".into())
    }
    if input.question_policy == pd::iz_science::QuestionPolicy::ComputedV1 {
        let expected = {
        let science_domains::ScienceTask::EarthRotationAngle { claim_radians, tolerance_radians, .. } = actual.query.task;
        pd::iz_science_math::computed_question(&input, &pd::iz_science_math::EraComparison {
            query_sha256: actual.query_sha256, result_sha256: actual.result_sha256,
            model_sha256: actual.provenance.compiled_model_sha256,
            claim_radians, computed_radians: actual.computed_radians,
            residual_radians: actual.circular_residual_radians, tolerance_radians,
        })
        }?;
        if request.scoped_question != expected {
            return Err("IZ_SCIENCE_COMPUTED_OUTPUT_MISMATCH".into());
        }
    }
    Ok(())
}

pub(crate) fn verify_initial_hold(
    store: &SelectionHoldStore,
    request_bytes: &[u8],
    response_bytes: &[u8],
    now_ns: u64,
) -> Result<(), String> {
    let (request, response) = failure::decode_selection_pair(request_bytes, response_bytes)?;
    if response.requester != SelectionRequester::AshtQuantum
        || !response.shape_is_valid_for(&request)
    {
        return Err("PD_FAILURE_HOLD_SCOPE".into());
    }
    let stored = store
        .load_bound(
            &response.hold_id,
            &response.session_id,
            response.primitive_anchor,
            &response.split_sha256,
            &response.hold_sha256,
        )
        .map_err(|e| format!("PD_FAILURE_HOLD_UNAVAILABLE:{e}"))?;
    // Hash self-consistency alone is insufficient: bind every candidate and
    // response field to the actual Shadow-staged bytes.
    if encode_selection_response(&stored) != response_bytes {
        return Err("PD_FAILURE_HOLD_RESPONSE_MISMATCH".into());
    }
    if stored.law_seal != crate::lab_contracts::gjata_collapse_law::law_seal()
        || stored.system_laws_seal != crate::lab_contracts::gjata_collapse_law::system_laws_seal()
    {
        return Err("PD_FAILURE_HOLD_LAW_SEALS".into());
    }
    // Same inclusive boundary as SelectionHoldStore::verify_muscle_evidence.
    if stored.expires_at_ns < now_ns {
        return Err("PD_FAILURE_HOLD_EXPIRED".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use shadow_contracts::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    struct Fixture {
        root: PathBuf,
        store: SelectionHoldStore,
        request: Vec<u8>,
        response: KnowledgeSelectionResponseWire,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "essmai-pdf1-admission-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed),
            ));
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
            let mut response = KnowledgeSelectionResponseWire {
                version: PROTOCOL_VERSION,
                requester: request.requester,
                session_id: request.session_id.clone(),
                parent_i0: request.parent_i0.clone(),
                primitive_anchor: 7,
                split_sha256: request.split_sha256.clone(),
                upstream_hold_sha256: request.upstream_hold_sha256.clone(),
                request_sha256: sha256_hex(&[b"ESSMAI/SELECTION/REQUEST/V178", &request_bytes]),
                selection_sha256: sha256_hex(&[b"ESSMAI/SELECTION/CANDIDATES/V178", &[]]),
                hold_id: "hold-ASHT".into(),
                hold_sha256: String::new(),
                positive: vec![],
                negative: vec![],
                law_seal: crate::lab_contracts::gjata_collapse_law::law_seal(),
                system_laws_seal: crate::lab_contracts::gjata_collapse_law::system_laws_seal(),
                expires_at_ns: 100,
            };
            response.hold_sha256 = SelectionHoldStore::recompute_hold_sha256(&response);
            Self {
                store: SelectionHoldStore::under_handoff(&root),
                root,
                request: request_bytes,
                response,
            }
        }
        fn check(&self, response: &KnowledgeSelectionResponseWire, now: u64) -> Result<(), String> {
            verify_initial_hold(
                &self.store,
                &self.request,
                &encode_selection_response(response),
                now,
            )
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            if self.root.exists() {
                std::fs::remove_dir_all(&self.root).unwrap();
            }
        }
    }

    #[test]
    fn actual_staged_asht_hold_accepts_exact_response_inclusive_expiry() {
        let fixture = Fixture::new();
        fixture.store.stage(&fixture.response).unwrap();
        assert!(fixture.check(&fixture.response, 99).is_ok());
        assert!(fixture.check(&fixture.response, 100).is_ok());
    }
    #[test]
    fn self_consistent_missing_hold_and_altered_response_are_rejected() {
        let fixture = Fixture::new();
        assert!(fixture
            .check(&fixture.response, 99)
            .unwrap_err()
            .contains("HOLD_UNAVAILABLE"));
        fixture.store.stage(&fixture.response).unwrap();
        let mut altered = fixture.response.clone();
        altered.selection_sha256 = "77".repeat(32);
        altered.hold_sha256 = SelectionHoldStore::recompute_hold_sha256(&altered);
        assert!(fixture.check(&altered, 99).is_err());
        // Candidate fields are not directly included in hold_sha256. Full
        // canonical response equality must still reject altered payload bytes.
        let mut altered = fixture.response.clone();
        altered.positive.push(CandidateStatisticWire {
            candidate_id: 1,
            polarity: 1,
            domain: "primitive".into(),
            term_sha256: "88".repeat(32),
            source_sha256: "99".repeat(32),
            security_y: 1,
            trust_x: 1,
            confidence_fixed: 7500,
            frequency: 2,
            domain_count: 1,
            legacy_score_fixed: 5000,
        });
        assert!(fixture.check(&altered, 99).is_err());
    }
    #[test]
    fn expired_first_commit_and_wrong_source_law_seals_are_rejected() {
        let fixture = Fixture::new();
        fixture.store.stage(&fixture.response).unwrap();
        assert_eq!(
            fixture.check(&fixture.response, 101).unwrap_err(),
            "PD_FAILURE_HOLD_EXPIRED"
        );
        let mut fixture = Fixture::new();
        fixture.response.law_seal ^= 1;
        fixture.response.hold_sha256 = SelectionHoldStore::recompute_hold_sha256(&fixture.response);
        fixture.store.stage(&fixture.response).unwrap();
        assert_eq!(
            fixture.check(&fixture.response, 99).unwrap_err(),
            "PD_FAILURE_HOLD_LAW_SEALS"
        );
    }
}

#[cfg(test)]
mod operational_tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use shadow_contracts::pd_memory as pd;

    fn admission(request: &PdOperationRequest, vault: &KnowledgeVault) -> Result<(), String> {
        validate_native_admission(
            request,
            vault,
            || panic!("PDC1 cannot request an ASHT hold directory"),
            || panic!("PDC1 cannot require a live hold clock"),
        )
    }
    fn rebind(request: &mut PdOperationRequest, operation: &str) {
        let mut input = pd::decode_continuity_input(&request.source_input).unwrap();
        input.operation_id = operation.into();
        request.operation_id = operation.into();
        request.source_input = pd::encode_continuity_input(&input).unwrap();
        request.witness.input_sha256 = Sha256::digest(&request.source_input).into();
        let step = pd::LgcStep::new(
            0,
            request.witness.input_sha256,
            "test",
            "explicit historical retry policy",
            [7; 32],
        )
        .unwrap();
        request.witness.lgc_parent = step.step_digest;
        request.lgc_steps = vec![step];
    }
    #[test]
    fn missing_marker_marker_only_and_corrupt_checkpoint_never_admit() {
        let vault = KnowledgeVault::new();
        let mut request = crate::pd_memory_store::tests::request("native-pdc1");
        request.validate().unwrap();
        assert_eq!(
            admission(&request, &vault).unwrap_err(),
            "PD_OPERATIONAL_CHECKPOINT_POLICY_REVIEW_REQUIRED"
        );
        for suffix in [&[][..], &[1, 2, 3][..], &[0; 64][..]] {
            request.checkpoint = OPERATIONAL_CHECKPOINT_PREFIX.to_vec();
            request.checkpoint.extend_from_slice(suffix);
            assert!(admission(&request, &vault)
                .unwrap_err()
                .starts_with("PD_OPERATIONAL_CHECKPOINT_TRANSITION:"));
        }
        assert!(vault.pd_head(&request.identity).unwrap().is_none());
    }
    #[test]
    fn source_identity_operation_or_hash_cannot_be_relabelled() {
        let vault = KnowledgeVault::new();
        let request = crate::pd_memory_store::tests::request("native-pdc1");
        let mut altered = request.clone();
        altered.operation_id.push('x');
        assert!(admission(&altered, &vault).is_err());
        let mut altered = request.clone();
        altered.identity.scope.push('x');
        assert!(admission(&altered, &vault).is_err());
        let mut altered = request.clone();
        altered.witness.input_sha256[0] ^= 1;
        assert!(admission(&altered, &vault).is_err());
        let mut altered = request;
        altered.source_input[0] = b'X';
        assert_eq!(
            admission(&altered, &vault).unwrap_err(),
            "PD_NATIVE_SOURCE_INPUT_VERSION"
        );
    }
    #[test]
    fn legacy_exact_retry_remains_historical_but_advance_and_conflict_refuse() {
        let path = std::env::temp_dir().join(format!(
            "essmai_pdc1_admission_{}_{}.wal",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        {
            let vault = KnowledgeVault::open_disk(&path).unwrap();
            // Deliberate internal storage fixture: simulates a pre-admission
            // historical record, never an allowed new native operation.
            let request = crate::pd_memory_store::tests::request("legacy");
            let saved = vault.commit_pd_operation(request.clone()).unwrap();
            assert!(admission(&request, &vault).is_ok());
            assert_eq!(vault.commit_pd_operation(request.clone()).unwrap(), saved);
            let mut conflict = request.clone();
            conflict.scoped_question.push('x');
            assert_eq!(
                admission(&conflict, &vault).unwrap_err(),
                "PD_OPERATIONAL_EXISTING_OPERATION_CONFLICT"
            );
            let mut advance = request;
            rebind(&mut advance, "next");
            advance.expected_revision = saved.revision;
            advance.previous_state = saved.state_digest;
            advance.checkpoint = OPERATIONAL_CHECKPOINT_PREFIX.to_vec();
            assert_eq!(
                admission(&advance, &vault).unwrap_err(),
                "PD_OPERATIONAL_PREVIOUS_CHECKPOINT_POLICY_REVIEW_REQUIRED"
            );
            assert_eq!(
                vault.pd_head(&saved.request.identity).unwrap().unwrap(),
                saved
            );
        }
        std::fs::remove_file(&path).unwrap();
    }
}
