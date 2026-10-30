//! NEW operational continuity, V189_DERIVATIVE.
//! Calls the real PD engine and a separate Shadow process. This explicitly named
//! CLI does not run or replace the constitutional P/N pipeline, mint a Spine-9
//! capability, admit knowledge, or turn a scientific model result into a verdict.
use crate::progressive_debatic::{
    core::ProgressiveDebatic,
    types::{DebateMode, PressureBudget, PD_SEAL},
};
use ess_mai_zero_noi::trigger;
use science_domains::{self as science, AdapterId, CandidateEvidence, ScienceError, ScienceQuery};
use sha2::{Digest, Sha256};
use shadow_contracts::pd_memory::*;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// Operational producer policy: the enclosed generic v1 checkpoint has only actual user turns.
// Marker binds policy within the retained Shadow record; it is not authentication.
const USER_ONLY_CHECKPOINT: &[u8] = b"PD_USER_ONLY_V2\0";

const CONTINUITY_LIMIT: usize = 40 * 1024;
const RECORD_LIMIT: usize = MAX_REQUEST_BYTES + 512;
const CHILD_LIMIT: Duration = Duration::from_secs(30);
const OPERATION_LIMIT: Duration = Duration::from_secs(90);
static HANDSHAKE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn sha(bytes: &[u8]) -> Hash {
    Sha256::digest(bytes).into()
}
fn source_digest() -> Hash {
    sha(include_bytes!("pd_operational.rs"))
}

fn user_only_checkpoint(inner: Vec<u8>) -> Result<Vec<u8>, String> {
    let size = USER_ONLY_CHECKPOINT
        .len()
        .checked_add(inner.len())
        .ok_or("PD_CHECKPOINT_SIZE")?;
    if size > MAX_CHECKPOINT_BYTES {
        return Err("PD_CHECKPOINT_PAYLOAD_LIMIT".into());
    }
    let mut bytes = Vec::with_capacity(size);
    bytes.extend_from_slice(USER_ONLY_CHECKPOINT);
    bytes.extend_from_slice(&inner);
    Ok(bytes)
}
fn restore_user_only(
    pd: &mut ProgressiveDebatic,
    session: &str,
    bytes: &[u8],
) -> Result<(), String> {
    let inner = bytes.strip_prefix(USER_ONLY_CHECKPOINT)
        .ok_or("PD_LEGACY_INGEST_POLICY_REVIEW_REQUIRED: preserve old history; no implicit migration or reset")?;
    pd.import_checkpoint(session, inner)
}

/// The scheduler chooses the exact predecessor-required action. 0-NOI only
/// borrows its supplied payload; it has no condition, selector, policy or effect.
fn step<T>(
    steps: &mut Vec<LgcStep>,
    input: Hash,
    site: &str,
    action: &str,
    payload: &[u8],
    effect: impl FnOnce(&[u8]) -> Result<T, String>,
) -> Result<T, String> {
    let pulse = trigger(payload);
    let value = effect(pulse.request())?;
    let predecessor = steps.last().map(|s| s.step_digest).unwrap_or(input);
    let entry = LgcStep::new(steps.len() as u32, predecessor, site, action, sha(payload))?;
    eprintln!(
        "LGC operational sequence={} site={} action={} digest={}",
        entry.sequence,
        entry.site,
        entry.action,
        hex(&entry.step_digest)
    );
    steps.push(entry);
    Ok(value)
}

fn bound_query(input: &ContinuityInput, session: &str) -> Result<ScienceQuery, String> {
    let query = science::decode_query(&input.science_query).map_err(|e| e.to_string())?;
    let domain = match query.adapter {
        AdapterId::Physics => "physics",
        AdapterId::Astronomy => "astronomy",
        AdapterId::History => "history",
        AdapterId::SocialScience => "social-science",
        AdapterId::Psychology => "psychology",
    };
    if query.session_id != session
        || query.case_id != input.identity.case_id
        || query.scope_id != input.identity.scope
        || query.obligation_id != input.identity.obligation
        || input.identity.domain != domain
    {
        return Err("PD_SCIENCE_SCOPE_MISMATCH".into());
    }
    Ok(query)
}

fn check_record_scope(record: &PdOperationRecord, input: &ContinuityInput) -> Result<(), String> {
    record.validate()?;
    if record.request.identity != input.identity {
        return Err("PD_RETURNED_RECORD_SCOPE_MISMATCH".into());
    }
    Ok(())
}

fn candidate_text(candidate: Option<&CandidateEvidence>, status: &str) -> String {
    match candidate {
        Some(c) => format!(
            "[UNADMITTED SCIENTIFIC MODEL CANDIDATE; authority=None; not an accepted positive/negative] \
            query={} source={} model_relation={:?}; computed_radians={:.17}; \
            circular_residual_radians={:.17}; result_sha256={}; {}",
            c.query.query_id, c.query.input_source_id, c.relation,
            c.computed_radians, c.circular_residual_radians, hex(&c.result_sha256), status),
        None => format!("[NO SCIENTIFIC CANDIDATE; authority=None; no accepted positive/negative] {status}"),
    }
}

/// Operational question uses the actual engine state, not a new scorer.
/// The whole hint and all concepts remain in the full checkpoint. Only the
/// concept-name preview is bounded here; its full count is always stated.
fn pd_question(
    pd: &ProgressiveDebatic,
    session_id: &str,
    input: &ContinuityInput,
    provider_status: &str,
) -> Result<String, String> {
    let session = pd
        .session(session_id)
        .ok_or("PD_QUESTION_SESSION_MISSING")?;
    let last = session.turns.last().ok_or("PD_QUESTION_TURN_MISSING")?;
    let user_turn = last; // Every turn under this operational policy is actual user input.
    let issued = session.pressure_budget.challenges_issued;
    let limit = PressureBudget::max_for(session.current_mode);
    let remaining = limit.saturating_sub(issued);
    let returned = &user_turn.user_signal.returns_to;
    let preview: Vec<String> = returned
        .iter()
        .take(8)
        .map(|value| {
            let mut short: String = value.chars().take(96).collect();
            if value.chars().count() > 96 {
                short.push_str(" [preview truncated]");
            }
            short
        })
        .collect();
    Ok(format!(
        "For case {} in scope {} under obligation {}: actual PD hint: {}. \
        Restored progressive context: total_turns={}; mode={}; challenges_issued={}; \
        pressure_remaining={}; pressure_limit={}; user_returns_to_count={}; \
        user_returned_concepts_preview={:?}. \
        What independently sourced material and native admission conditions remain necessary? \
        Separate scientific evidence (never a user turn): {}. Cognitive context and unadmitted model output have no constitutional verdict.",
        input.identity.case_id, input.identity.scope, input.identity.obligation,
        last.pd_response_hint, session.total_turns, session.current_mode.label(),
        issued, remaining, limit, returned.len(), preview, provider_status,
    ))
}

/// Internally testable orchestration; the production caller always supplies
/// real file IPC to the independently executing Shadow binary.
fn continue_with(
    input: &ContinuityInput,
    transport: &mut impl FnMut(&str, &[u8]) -> Result<Vec<u8>, String>,
    clock: impl FnOnce() -> Result<u64, String>,
) -> Result<PdOperationRecord, String> {
    let canonical_input = encode_continuity_input(input)?;
    let input_hash = sha(&canonical_input);
    let session_id = hex(&input.identity.key()?);
    let mut steps = Vec::new();

    // First operation is exact historical replay, BEFORE provider evaluation,
    // timestamps, head restoration or any new PD ingest.
    let exact = PdLookup {
        identity: input.identity.clone(),
        operation_id: Some(input.operation_id.clone()),
    };
    let replay = step(
        &mut steps,
        input_hash,
        "mp_pd",
        "lookup exact operation before any state change",
        &encode_lookup(&exact)?,
        |b| decode_optional_record(&transport("--pd-head-once", b)?),
    )?;
    if let Some(record) = replay {
        check_record_scope(&record, input)?;
        if record.request.operation_id != input.operation_id
            || record.request.witness.input_sha256 != input_hash
        {
            return Err("PD_OPERATION_ID_CONFLICT".into());
        }
        eprintln!(
            "LGC operational exact_replay revision={} record={} stored_policy={}",
            record.revision,
            hex(&record.record_id),
            if record.request.checkpoint.starts_with(USER_ONLY_CHECKPOINT) {
                "user-only-v2"
            } else {
                "legacy-unmarked-original-result"
            }
        );
        return Ok(record);
    }

    let now = clock()?; // Exact replay above never consults the current wall clock.
    let query = bound_query(input, &session_id)?;
    let query_bytes = science::encode_query(&query).map_err(|e| e.to_string())?;
    let query_digest = sha(&query_bytes);
    let head_lookup = PdLookup {
        identity: input.identity.clone(),
        operation_id: None,
    };
    let head = step(
        &mut steps,
        input_hash,
        "mp_pd",
        "read committed map head",
        &encode_lookup(&head_lookup)?,
        |b| decode_optional_record(&transport("--pd-head-once", b)?),
    )?;
    let mut pd = ProgressiveDebatic::new();
    let (revision, previous_state) = match head {
        Some(record) => {
            check_record_scope(&record, input)?;
            step(
                &mut steps,
                input_hash,
                "mp_pd",
                "restore exact checkpoint into fresh PD engine",
                &record.request.checkpoint,
                |b| restore_user_only(&mut pd, &session_id, b),
            )?;
            (record.revision, record.state_digest)
        }
        None => {
            step(
                &mut steps,
                input_hash,
                "mp_pd",
                "initialize previously absent scoped session",
                &canonical_input,
                |_| {
                    // Preserve the existing cognitive entry guard. PD_SEAL
                    // is a static module marker, not a Spine/verification token.
                    pd.open_session_sealed(
                        &session_id,
                        &input.identity.scope,
                        DebateMode::Scientific,
                        now,
                        PD_SEAL,
                    )
                    .map_err(|e| format!("PD_OPEN_SEAL: {e:?}"))
                },
            )?;
            (0, [0; 32])
        }
    };
    let session = pd
        .session(&session_id)
        .ok_or("PD_RESTORED_SESSION_MISSING")?;
    if session.territory != input.identity.scope || !pd.active || !session.active {
        return Err("PD_SESSION_SCOPE_OR_ACTIVITY_REFUSAL".into());
    }
    if now < session.last_activity_at {
        return Err("PD_CLOCK_REGRESSION".into());
    }
    // Only actual user input is a PD turn. Provider evidence is retained separately.
    // Refuse before mutation if the full history cannot retain this user turn.
    if session.total_turns >= 4096 {
        return Err("PD_CHECKPOINT_TURN_LIMIT".into());
    }

    let candidate_lookup = step(
        &mut steps,
        input_hash,
        "map",
        "construct exact Vault query from restored identity and scoped science",
        &query_bytes,
        |_| {
            encode_candidate_query(&PdCandidateQuery {
                identity: input.identity.clone(),
                query_digest,
            })
        },
    )?;
    // Science is always evaluated, even on a Vault hit. The worker performs
    // only local pinned computation; lookup remains a distinct real process.
    let (stored, evaluated) = std::thread::scope(|scope| {
        let worker = scope.spawn(|| {
            let pulse = trigger(&query);
            science::evaluate(pulse.request())
        });
        let stored = step(
            &mut steps,
            input_hash,
            "Vault",
            "lookup exact scoped candidate from map key",
            &candidate_lookup,
            |b| decode_optional_record(&transport("--pd-candidate-once", b)?),
        );
        (stored, worker.join())
    });
    let stored = stored?;
    let evaluated = evaluated.map_err(|_| "PD_PROVIDER_WORKER_FAILED")?;
    let (computed, provider_status) = match evaluated {
        Ok(candidate) => (
            Some(candidate),
            "installed scientific model computed; admission not requested".to_string(),
        ),
        Err(ScienceError::Unavailable { adapter }) => (
            None,
            format!("no installed provider for {adapter:?}; admission not requested"),
        ),
        Err(error) => return Err(format!("PD_PROVIDER_REJECTED: {error}")),
    };
    step(
        &mut steps,
        input_hash,
        "science-domains",
        "retain actual parallel provider outcome",
        provider_status.as_bytes(),
        |_| Ok(()),
    )?;
    let (candidate, reuse_status, candidate_bytes) = step(
        &mut steps,
        input_hash,
        "map",
        "bind candidate selection to exact query and provider",
        &query_bytes,
        |_| {
            let (candidate, reuse_status) = match (stored, computed) {
                (Some(record), Some(computed)) => {
                    check_record_scope(&record, input)?;
                    if record.request.candidate_key != query_digest {
                        return Err("PD_CANDIDATE_KEY_MISMATCH".into());
                    }
                    let decoded = science::decode_candidate(&record.request.candidate)
                        .map_err(|e| e.to_string())?;
                    if decoded != computed || decoded.query != query {
                        return Err("PD_CANDIDATE_REEVALUATION_MISMATCH".into());
                    }
                    (
                        Some(decoded),
                        "exact stored candidate reused after scoped re-evaluation",
                    )
                }
                (None, candidate) => (candidate, "no exact stored candidate"),
                (Some(_), None) => {
                    return Err("PD_STORED_CANDIDATE_WITHOUT_CURRENT_PROVIDER".into())
                }
            };
            let bytes = candidate
                .as_ref()
                .map(science::encode_candidate)
                .transpose()
                .map_err(|e| e.to_string())?
                .unwrap_or_default();
            Ok((candidate, reuse_status, bytes))
        },
    )?;

    // Source core.rs:108-145 and genius_detector.rs evaluate every ingested
    // signal as user cognition. Therefore provider bytes/status never enter
    // this API and cannot consume pressure or create a user genius signal.
    step(
        &mut steps,
        input_hash,
        "PD",
        "ingest actual supplied user text",
        input.user_text.as_bytes(),
        |_| {
            pd.ingest_sealed(&session_id, &input.user_text, now, PD_SEAL)
                .map(|_| ())
                .map_err(|e| format!("PD_INGEST: {e:?}"))
        },
    )?;
    let scientific_summary = candidate_text(
        candidate.as_ref(),
        &format!("{provider_status}; {reuse_status}"),
    );
    // Full model bytes remain request.candidate with their scoped query/provenance;
    // this summary is used only in the separate question/witness, never PD ingest.
    let checkpoint = step(
        &mut steps,
        input_hash,
        "PD",
        "export full progressive state without resetting budget",
        session_id.as_bytes(),
        |_| user_only_checkpoint(pd.export_checkpoint(&session_id)?),
    )?;
    let condition = if candidate.is_some() {
        "UNADMITTED_MODEL_CANDIDATE"
    } else {
        "SCIENTIFIC_PROVIDER_UNAVAILABLE"
    };
    let reason = format!(
        "{provider_status}; {reuse_status}. This operational CLI has not invoked \
        ASHT/GCL adjudication and reports no authority refusal or admitted positive/negative."
    );
    // D-BAB witnesses the actual local condition; it does not invent a refusal,
    // select authority, or approve admission.
    step(
        &mut steps,
        input_hash,
        "D-BAB",
        "witness reported operational condition for scoped iZ question",
        reason.as_bytes(),
        |_| Ok(()),
    )?;
    let scoped_question = step(
        &mut steps,
        input_hash,
        "iZ",
        "derive scoped question from actual restored PD history, response hint and budget",
        &checkpoint,
        |_| pd_question(&pd, &session_id, input, &scientific_summary),
    )?;
    step(
        &mut steps,
        input_hash,
        "Shadow",
        "prepare requested durable commit; outcome still pending",
        &checkpoint,
        |_| Ok(()),
    )?;
    let witness = ConditionWitness {
        predecessor: "Quantum operational candidate acquisition".into(),
        parent_authority: "Explicit operational continuity request; no GCL authority token".into(),
        failed_action:
            "Constitutional admission unavailable in this operational command; not invoked".into(),
        condition_code: condition.into(),
        reason,
        source_component: "quantum/src/pd_operational.rs".into(),
        source_sha256: source_digest(),
        attempt_trace: input.operation_id.clone(),
        input_sha256: input_hash,
        lgc_parent: steps.last().ok_or("PD_LGC_MISSING")?.step_digest,
        report_class: ReportClass::MissingConditions,
        permitted_next: if candidate.is_some() {
            NextAction::ExplainConditions
        } else {
            NextAction::AcquireScopedEvidence
        },
    };
    let request = PdOperationRequest {
        identity: input.identity.clone(),
        operation_id: input.operation_id.clone(),
        expected_revision: revision,
        previous_state,
        witness,
        checkpoint,
        scoped_question,
        candidate: candidate_bytes,
        candidate_key: candidate
            .as_ref()
            .map(|c| c.query_sha256)
            .unwrap_or([0; 32]),
        lgc_steps: steps,
        source_input: canonical_input.clone(),
    };
    let encoded = encode_request(&request)?;
    let pulse = trigger(encoded.as_slice());
    let committed_bytes = transport("--pd-commit-once", pulse.request()).map_err(|e| {
        format!("PD_COMMIT_STATUS_UNCERTAIN: resolve exact operation before retry: {e}")
    })?;
    let committed = decode_record(&committed_bytes)?;
    check_record_scope(&committed, input)?;
    if committed.request != request {
        return Err("PD_COMMIT_RETURNED_DIFFERENT_REQUEST".into());
    }
    // A response is exposed only after a second native Shadow process opens the
    // durable store and returns exactly the same historical operation.
    let readback_query = encode_lookup(&exact)?;
    let readback_pulse = trigger(readback_query.as_slice());
    let readback_bytes = transport("--pd-head-once", readback_pulse.request())
        .map_err(|e| format!("PD_COMMIT_READBACK_UNCERTAIN: {e}"))?;
    let readback = decode_optional_record(&readback_bytes)
        .map_err(|e| format!("PD_COMMIT_READBACK_UNCERTAIN: {e}"))?
        .ok_or("PD_COMMIT_READBACK_MISSING")?;
    if readback != committed {
        return Err("PD_COMMIT_READBACK_MISMATCH".into());
    }
    eprintln!(
        "LGC operational durable_readback revision={} record={} verdict=NoVerdict",
        readback.revision,
        hex(&readback.record_id)
    );
    Ok(readback)
}

fn now_millis() -> Result<u64, String> {
    let value = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "PD_CLOCK_BEFORE_EPOCH")?
        .as_millis();
    u64::try_from(value).map_err(|_| "PD_CLOCK_OVERFLOW".into())
}
fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|e| format!("PD_READ {}: {e}", path.display()))?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("PD_INPUT_NOT_FILE".into());
    }
    let mut out = Vec::new();
    file.take((limit as u64) + 1)
        .read_to_end(&mut out)
        .map_err(|e| e.to_string())?;
    if out.len() > limit {
        return Err("PD_FILE_LIMIT".into());
    }
    Ok(out)
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("PD_CREATE_NEW {}: {e}", path.display()))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("PD_WRITE_SYNC: {e}"))
}
/// Publish a complete file without clobbering a previous response. Hard-link is
/// an atomic no-replace publication on the same filesystem; not an auth seal.
pub(crate) fn publish_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let stage = unique_directory(parent)?;
    let source = stage.join("response.ready");
    let result = write_new(&source, bytes).and_then(|_| {
        fs::hard_link(&source, path).map_err(|e| format!("PD_PUBLISH_NO_CLOBBER: {e}"))
    });
    let _ = fs::remove_file(source);
    let _ = fs::remove_dir(stage);
    result
}
fn unique_directory(parent: &Path) -> Result<PathBuf, String> {
    for _ in 0..64 {
        let sequence = HANDSHAKE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        // PID + atomic sequence + exclusive directory creation is independent
        // of wall-clock availability, including an exact historical replay.
        let path = parent.join(format!(".pd-{}-{}", std::process::id(), sequence));
        match fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("PD_STAGE_CREATE: {e}")),
        }
    }
    Err("PD_STAGE_COLLISION_LIMIT".into())
}
fn shadow_binary() -> Result<PathBuf, String> {
    let path = match std::env::var_os("ESSMAI_SHADOW_BIN") {
        Some(path) => PathBuf::from(path),
        None => {
            let own = std::env::current_exe().map_err(|e| e.to_string())?;
            own.parent()
                .ok_or("PD_EXECUTABLE_PARENT")?
                .join(if cfg!(windows) {
                    "shadow_platform.exe"
                } else {
                    "shadow_platform"
                })
        }
    };
    if !path.is_absolute() || !path.is_file() {
        return Err("PD_SHADOW_BINARY_NOT_ABSOLUTE_FILE".into());
    }
    fs::canonicalize(path).map_err(|e| e.to_string())
}
pub fn native_transport(mode: &str, request: &[u8], remaining: Duration) -> Result<Vec<u8>, String> {
    if remaining.is_zero() {
        return Err("PD_OPERATION_DEADLINE".into());
    }
    let child_limit = remaining.min(CHILD_LIMIT);
    let exchange_started = Instant::now();
    if !matches!(
        mode,
        "--pd-head-once" | "--pd-candidate-once" | "--pd-commit-once" | "--pd-iz-native-read-once"
            | "--selection-once" | "--bridge-once" | "--bridge-pd-once" | "--pd-finalize-once" | "--pd-recover-once" | "--kie-select-once"
    ) {
        return Err("PD_SHADOW_MODE".into());
    }
    let request_limit = match mode {
        "--pd-iz-native-read-once" => shadow_contracts::pd_memory::iz::MAX_CONTEXT_BYTES,
        "--kie-select-once" => shadow_contracts::kie::MAX_WIRE_BYTES,
        "--bridge-pd-once" => shadow_contracts::pd_cycle::MAX_CYCLE_BYTES,
        "--selection-once" | "--bridge-once" => 64 * 1024 * 1024,
        "--pd-finalize-once" | "--pd-recover-once" => 68,
        _ => shadow_contracts::pd_memory::MAX_REQUEST_BYTES,
    };
    if request.len() > request_limit {
        return Err("PD_SHADOW_REQUEST_LIMIT".into());
    }
    let root = std::env::var_os("ESSMAI_HANDOFF_DIR")
        .map(PathBuf::from)
        .ok_or("PD_HANDOFF_DIR_MISSING")?;
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
    let stage = unique_directory(&root)?;
    let request_path = stage.join("request.bin");
    let response_path = stage.join("response.bin");
    let outcome = (|| {
        write_new(&request_path, request)?;
        let mut command = Command::new(shadow_binary()?);
        command
            .arg(mode)
            .arg(&request_path)
            .arg(&response_path)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }
        if exchange_started.elapsed() >= child_limit {
            return Err("PD_OPERATION_DEADLINE_BEFORE_SPAWN".into());
        }
        let mut child = command
            .spawn()
            .map_err(|e| format!("PD_SHADOW_SPAWN: {e}"))?;
        loop {
            let status = match child.try_wait() {
                Ok(status) => status,
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("PD_SHADOW_WAIT_STATUS_UNCERTAIN: {error}"));
                }
            };
            match status {
                Some(status) if status.success() => break,
                Some(status) => return Err(format!("PD_SHADOW_EXIT: {status}")),
                None if exchange_started.elapsed() >= child_limit => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err("PD_SHADOW_TIMEOUT_STATUS_UNCERTAIN".into());
                }
                None => std::thread::sleep(Duration::from_millis(10)),
            }
        }
        read_bounded(&response_path, match mode {
 "--pd-recover-once" | "--pd-iz-native-read-once"=>shadow_contracts::pd_recovery::MAX_BUNDLE_BYTES,
 "--kie-select-once"=>shadow_contracts::kie::MAX_WIRE_BYTES,
 _=>RECORD_LIMIT,
 })
    })();
    // Remove only the exact files and directory created for this exchange.
    let _ = fs::remove_file(&request_path);
    let _ = fs::remove_file(&response_path);
    let _ = fs::remove_dir(&stage);
    outcome
}

/// None preserves the original runtime path. Some always consumes this explicit
/// NEW command, including malformed invocation, and never falls back to it.
pub fn dispatch_from_args(args: &[String]) -> Option<i32> {
    let command = args.get(1).map(String::as_str).unwrap_or("");
    if !command.starts_with("--pd-") {
        return None;
    }
    if command != "--pd-continuity-once" || args.len() != 4 {
        eprintln!("usage: quantum-platform --pd-continuity-once REQUEST RESPONSE");
        return Some(64);
    }
    let result = (|| {
        let operation_started = Instant::now();
        let raw = read_bounded(Path::new(&args[2]), CONTINUITY_LIMIT)?;
        let input = decode_continuity_input(&raw)?;
        let mut transport = |mode: &str, bytes: &[u8]| {
            let remaining = OPERATION_LIMIT
                .checked_sub(operation_started.elapsed())
                .ok_or("PD_OPERATION_DEADLINE")?;
            native_transport(mode, bytes, remaining)
        };
        let record = continue_with(&input, &mut transport, now_millis)?;
        publish_new(Path::new(&args[3]), &encode_record(&record)?)
    })();
    Some(match result {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("PD_CONTINUITY_ERROR: {error}");
            70
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use science::{ScienceTask, TimeScale};

    fn input(op: &str) -> ContinuityInput {
        let identity = PdIdentity {
            owner: "owner".into(),
            obligation: "o1".into(),
            case_id: "case-A".into(),
            scope: "earth-rotation".into(),
            domain: "astronomy".into(),
            lineage_root: "root".into(),
        };
        let query = ScienceQuery {
            adapter: AdapterId::Astronomy,
            session_id: hex(&identity.key().unwrap()),
            case_id: identity.case_id.clone(),
            scope_id: identity.scope.clone(),
            obligation_id: identity.obligation.clone(),
            query_id: "query-1".into(),
            input_source_id: "explicit-test-vector-only".into(),
            task: ScienceTask::EarthRotationAngle {
                jd_part1: 2400000.5,
                jd_part2: 54388.0,
                time_scale: TimeScale::Ut1,
                claim_radians: 0.4022837240028158,
                tolerance_radians: 1e-12,
            },
        };
        ContinuityInput { identity, operation_id: op.into(),
            user_text: "Earth rotation measurement returns a scoped scientific claim; how can we verify it?".into(),
            science_query: science::encode_query(&query).unwrap() }
    }

    /// Contract-level transport fixture only. Native process and WAL durability
    /// must also be tested by the root's real three-binary scenario.
    #[derive(Default)]
    struct Memory {
        records: Vec<PdOperationRecord>,
        hide_committed_readback: bool,
        call_modes: Vec<String>,
    }
    impl Memory {
        fn call(&mut self, mode: &str, bytes: &[u8]) -> Result<Vec<u8>, String> {
            self.call_modes.push(mode.into());
            match mode {
                "--pd-head-once" => {
                    let lookup = decode_lookup(bytes)?;
                    let record = self.records.iter().rev().find(|r| {
                        r.request.identity == lookup.identity
                            && lookup
                                .operation_id
                                .as_ref()
                                .map(|id| &r.request.operation_id == id)
                                .unwrap_or(true)
                    });
                    if self.hide_committed_readback
                        && lookup.operation_id.is_some()
                        && record.is_some()
                    {
                        return encode_optional_record(None);
                    }
                    encode_optional_record(record)
                }
                "--pd-candidate-once" => {
                    let query = decode_candidate_query(bytes)?;
                    encode_optional_record(self.records.iter().rev().find(|r| {
                        r.request.identity == query.identity
                            && r.request.candidate_key == query.query_digest
                    }))
                }
                "--pd-commit-once" => {
                    let request = decode_request(bytes)?;
                    let head = self
                        .records
                        .iter()
                        .rev()
                        .find(|r| r.request.identity == request.identity);
                    let (revision, state, previous) = head
                        .map(|r| (r.revision, r.state_digest, r.record_id))
                        .unwrap_or((0, [0; 32], [0; 32]));
                    if request.expected_revision != revision || request.previous_state != state {
                        return Err("fixture CAS conflict".into());
                    }
                    let record = PdOperationRecord::construct(request, previous)?;
                    let bytes = encode_record(&record)?;
                    self.records.push(record);
                    Ok(bytes)
                }
                _ => Err("unexpected mode".into()),
            }
        }
        fn run(&mut self, input: &ContinuityInput, now: u64) -> Result<PdOperationRecord, String> {
            continue_with(input, &mut |mode, bytes| self.call(mode, bytes), || Ok(now))
        }
    }

    #[test]
    fn restart_uses_full_checkpoint_and_scoped_candidate_without_admission() {
        let mut memory = Memory::default();
        let first_input = input("op-1");
        let first = memory.run(&first_input, 100).unwrap();
        let session_id = hex(&first_input.identity.key().unwrap());
        let mut first_pd = ProgressiveDebatic::new();
        restore_user_only(&mut first_pd, &session_id, &first.request.checkpoint).unwrap();
        let prior = first_pd.session(&session_id).unwrap();
        assert_eq!(prior.total_turns, 1);
        assert_eq!(prior.started_at, 100);
        assert_eq!(prior.trace.signals[0].raw_content, first_input.user_text);
        let previous_budget = prior.pressure_budget.challenges_issued;
        assert!(!first.request.candidate.is_empty());
        assert!(first
            .request
            .scoped_question
            .contains("Separate scientific evidence"));
        assert!(first
            .request
            .scoped_question
            .contains("UNADMITTED SCIENTIFIC MODEL"));
        let mut second_input = first_input.clone();
        second_input.operation_id = "op-2".into();
        second_input.user_text =
            "A separate observation must challenge this earth rotation claim.".into();
        let second = memory.run(&second_input, 101).unwrap();
        let mut restored = ProgressiveDebatic::new();
        restore_user_only(&mut restored, &session_id, &second.request.checkpoint).unwrap();
        let session = restored.session(&session_id).unwrap();
        assert_eq!(
            (
                second.revision,
                session.total_turns,
                session.started_at,
                session.last_activity_at
            ),
            (2, 2, 100, 101)
        );
        assert!(session.pressure_budget.challenges_issued >= previous_budget);
        assert_eq!(session.trace.signals[1].raw_content, second_input.user_text);
        assert!(second
            .request
            .scoped_question
            .contains("exact stored candidate reused"));
        assert_eq!(
            second.request.witness.report_class,
            ReportClass::MissingConditions
        );
        assert!(second
            .request
            .witness
            .reason
            .contains("has not invoked ASHT/GCL"));
        assert_eq!(
            second.request.source_input,
            encode_continuity_input(&second_input).unwrap()
        );
        assert_eq!(second.request.candidate, first.request.candidate);
        assert_eq!(second.previous_record, first.record_id);
        assert!(second.request.lgc_steps.iter().any(|s| s.site == "D-BAB"));
        assert!(second.request.lgc_steps.iter().any(|s| s.site == "iZ"));
    }

    #[test]
    fn replay_precedes_clock_provider_and_ingest_but_conflicting_input_is_rejected() {
        let mut memory = Memory::default();
        let original = input("op-1");
        let record = memory.run(&original, 100).unwrap();
        memory.call_modes.clear();
        assert_eq!(memory.run(&original, 0).unwrap(), record);
        assert_eq!(memory.call_modes, vec!["--pd-head-once"]);
        let without_clock = continue_with(
            &original,
            &mut |mode, bytes| memory.call(mode, bytes),
            || Err("unavailable current wall clock".into()),
        )
        .unwrap();
        assert_eq!(without_clock, record);
        assert_eq!(memory.records.len(), 1);
        let mut conflict = original.clone();
        conflict.user_text.push_str(" altered input");
        assert!(memory
            .run(&conflict, 101)
            .unwrap_err()
            .contains("PD_OPERATION_ID_CONFLICT"));
        assert_eq!(memory.records.len(), 1);
    }

    #[test]
    fn scope_mismatch_and_clock_regression_do_not_commit_or_reset_state() {
        let mut memory = Memory::default();
        let original = input("op-1");
        let first = memory.run(&original, 100).unwrap();
        let mut next = original.clone();
        next.operation_id = "op-2".into();
        assert!(memory
            .run(&next, 99)
            .unwrap_err()
            .contains("PD_CLOCK_REGRESSION"));
        let mut query = science::decode_query(&next.science_query).unwrap();
        query.obligation_id = "another-obligation".into();
        next.science_query = science::encode_query(&query).unwrap();
        assert!(memory
            .run(&next, 101)
            .unwrap_err()
            .contains("PD_SCIENCE_SCOPE_MISMATCH"));
        assert_eq!(memory.records, vec![first]);
    }

    #[test]
    fn unavailable_provider_is_persisted_as_missing_condition_without_candidate() {
        let mut memory = Memory::default();
        let mut request = input("op-1");
        request.identity.domain = "physics".into();
        let mut query = science::decode_query(&request.science_query).unwrap();
        query.adapter = AdapterId::Physics;
        query.session_id = hex(&request.identity.key().unwrap());
        request.science_query = science::encode_query(&query).unwrap();
        let record = memory.run(&request, 1).unwrap();
        assert!(record.request.candidate.is_empty());
        assert_eq!(record.request.candidate_key, [0; 32]);
        assert_eq!(
            record.request.witness.condition_code,
            "SCIENTIFIC_PROVIDER_UNAVAILABLE"
        );
        assert_eq!(
            record.request.witness.permitted_next,
            NextAction::AcquireScopedEvidence
        );
        assert_eq!(
            record.request.source_input,
            encode_continuity_input(&request).unwrap()
        );
    }

    #[test]
    fn lost_commit_readback_is_resolved_by_exact_replay_without_repeating_ingest() {
        let mut memory = Memory {
            hide_committed_readback: true,
            ..Memory::default()
        };
        let request = input("op-1");
        assert!(memory
            .run(&request, 100)
            .unwrap_err()
            .contains("PD_COMMIT_READBACK_MISSING"));
        assert_eq!(memory.records.len(), 1);
        let committed = memory.records[0].clone();
        memory.hide_committed_readback = false;
        assert_eq!(memory.run(&request, 101).unwrap(), committed);
        assert_eq!(memory.records.len(), 1);
    }
    #[test]
    fn restored_history_causally_changes_question_and_preserves_real_pd_hint() {
        let mut with_history = Memory::default();
        let first = input("prior");
        with_history.run(&first, 100).unwrap();
        let mut next = first.clone();
        next.operation_id = "next".into();
        let continued = with_history.run(&next, 101).unwrap();
        let fresh = Memory::default().run(&next, 101).unwrap();
        let session_id = hex(&next.identity.key().unwrap());
        let mut restored = ProgressiveDebatic::new();
        restore_user_only(&mut restored, &session_id, &continued.request.checkpoint).unwrap();
        let current = restored.session(&session_id).unwrap();
        let last_user = current.turns.last().unwrap();
        assert!(!last_user.user_signal.returns_to.is_empty());
        assert!(continued.request.scoped_question.contains(&format!(
            "user_returns_to_count={}",
            last_user.user_signal.returns_to.len()
        )));
        assert!(fresh
            .request
            .scoped_question
            .contains("user_returns_to_count=0;"));
        assert!(continued
            .request
            .scoped_question
            .contains(&current.turns.last().unwrap().pd_response_hint));
        assert_ne!(
            continued.request.scoped_question,
            fresh.request.scoped_question
        );
        let map_step = continued
            .request
            .lgc_steps
            .iter()
            .position(|s| s.site == "map" && s.action.contains("construct"))
            .unwrap();
        let vault_step = continued
            .request
            .lgc_steps
            .iter()
            .position(|s| s.site == "Vault")
            .unwrap();
        assert!(map_step < vault_step);
    }
    #[test]
    fn malformed_pd_namespace_is_consumed_without_entering_original_runtime() {
        assert_eq!(
            dispatch_from_args(&["q".into(), "--pd-unknown".into()]),
            Some(64)
        );
        assert_eq!(
            dispatch_from_args(&["q".into(), "--pd-continuity-once".into()]),
            Some(64)
        );
        assert_eq!(dispatch_from_args(&["q".into(), "--original".into()]), None);
        assert!(native_transport("--pd-head-once", b"", Duration::ZERO)
            .unwrap_err()
            .contains("PD_OPERATION_DEADLINE"));
    }
    #[test]
    fn scientific_words_never_enter_user_trace_genius_or_pressure_budget() {
        let mut memory = Memory::default();
        let mut request = input("op-0");
        request.user_text = "aa".into();
        let mut query = science::decode_query(&request.science_query).unwrap();
        query.query_id = "zephyrcountermodel".into();
        query.input_source_id = "zirconiumastrolabewitness".into();
        request.science_query = science::encode_query(&query).unwrap();
        let session_id = hex(&request.identity.key().unwrap());
        let mut user_only = ProgressiveDebatic::new();
        user_only
            .open_session_sealed(
                &session_id,
                &request.identity.scope,
                DebateMode::Scientific,
                1,
                PD_SEAL,
            )
            .unwrap();
        for time in 1..=8 {
            request.operation_id = format!("op-{time}");
            let actual = memory.run(&request, time).unwrap();
            user_only
                .ingest_sealed(&session_id, &request.user_text, time, PD_SEAL)
                .unwrap();
            assert_eq!(
                actual.request.checkpoint,
                user_only_checkpoint(user_only.export_checkpoint(&session_id).unwrap()).unwrap()
            );
            assert!(!actual.request.candidate.is_empty());
            assert!(actual
                .request
                .scoped_question
                .contains("zephyrcountermodel"));
        }
        let session = user_only.session(&session_id).unwrap();
        assert_eq!(session.total_turns, 8);
        assert!(session
            .trace
            .signals
            .iter()
            .all(|signal| signal.raw_content == "aa"));
        assert!(!session.trace.concept_map.contains_key("zephyrcountermodel"));
        assert!(!session
            .trace
            .concept_map
            .contains_key("zirconiumastrolabewitness"));
        assert!(session.genius_signal.is_none());
    }

    #[test]
    fn unmarked_operational_history_is_replayable_but_never_silently_advanced() {
        let mut memory = Memory::default();
        let original = input("legacy-op");
        let current = memory.run(&original, 100).unwrap();
        let mut old_request = current.request.clone();
        old_request.checkpoint = old_request
            .checkpoint
            .strip_prefix(USER_ONLY_CHECKPOINT)
            .unwrap()
            .to_vec();
        let legacy = PdOperationRecord::construct(old_request, [0; 32]).unwrap();
        memory.records[0] = legacy.clone();
        assert_eq!(memory.run(&original, 0).unwrap(), legacy);
        let mut next = original.clone();
        next.operation_id = "new-op".into();
        assert!(memory
            .run(&next, 101)
            .unwrap_err()
            .contains("PD_LEGACY_INGEST_POLICY_REVIEW_REQUIRED"));
        assert_eq!(memory.records, vec![legacy.clone()]);
        // Generic v1 stays readable for legitimate producers. The operational
        // wrapper refuses a fresh interpretation rather than rewriting history.
        let mut generic = ProgressiveDebatic::new();
        generic
            .import_checkpoint(
                &hex(&original.identity.key().unwrap()),
                &legacy.request.checkpoint,
            )
            .unwrap();
    }
}
