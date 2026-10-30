//! NEW input/inspection utility for operational PD continuity.
//! No fixture defaults, Shadow authority, accepted knowledge or Vault writes.
use science_domains::{
    decode_candidate, decode_query, encode_candidate, encode_query, evaluate, AdapterId,
    CandidateEvidence, ModelRelation, ScienceError, ScienceQuery, ScienceTask, TimeScale,
};
use shadow_contracts::pd_memory::{
    decode_continuity_input, decode_record, encode_continuity_input, hex, ContinuityInput,
    NextAction, PdIdentity, ReportClass, MAX_REQUEST_BYTES,
};
use sha2::{Digest, Sha256};
use std::{
    ffi::OsString,
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
    process::ExitCode,
};

const HELP: &str = "\
AIGovern-Cyber operational continuity input utility

Create a request (all fourteen arguments are required; quote text with spaces):
  pd_continuity_input OUTPUT OWNER OBLIGATION CASE SCOPE LINEAGE OP USER_TEXT JD1 JD2 CLAIM_RADIANS TOLERANCE INPUT_SOURCE QUERY_ID

Inspect a persisted operational record:
  pd_continuity_input --inspect RECORD

OUTPUT is created exclusively and synchronized; an existing file is never overwritten.
Domain is astronomy. JD1 + JD2 MUST be caller-supplied UT1 Julian-date parts.
UTC/TT/TAI are not converted. CLAIM_RADIANS is in [0, 2*pi), TOLERANCE in (0, pi].
The caller supplies all identities, text, dates, claim, tolerance and source lineage.
The ERA result is a deterministic IAU model comparison, not a new observation.
No accepted knowledge, P/N polarity, Shadow verdict, capability or verification receipt
is created. Inspection checks record/candidate integrity, not sender authentication
or whether the record was actually admitted to the authoritative Shadow WAL.
Output uses stable key=value lines; text is JSON-quoted, hashes and numbers are plain.
";

fn quoted(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                // UTF-16 escapes keep even non-BMP control values well formed.
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    out.push_str(&format!("\\u{unit:04x}"));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
fn field(out: &mut String, key: &str, value: impl std::fmt::Display) {
    out.push_str(&format!("{key}={value}\n"));
}
fn text_field(out: &mut String, key: &str, value: &str) {
    field(out, key, quoted(value));
}
fn sha(bytes: &[u8]) -> [u8; 32] { Sha256::digest(bytes).into() }
fn relation(relation: ModelRelation) -> &'static str {
    match relation {
        ModelRelation::SupportsClaim => "supports_claim",
        ModelRelation::CountersClaim => "counters_claim",
        ModelRelation::Unresolved => "unresolved",
    }
}
fn text_arg<'a>(args: &'a [OsString], index: usize, label: &str) -> Result<&'a str, String> {
    args.get(index).and_then(|value| value.to_str())
        .ok_or_else(|| format!("{label} must be a supplied UTF-8 argument"))
}
fn number_arg(args: &[OsString], index: usize, label: &str) -> Result<f64, String> {
    let raw = text_arg(args, index, label)?;
    let number = raw.parse::<f64>().map_err(|_| format!("{label} is not a valid f64 number"))?;
    if !number.is_finite() { return Err(format!("{label} must be finite")); }
    Ok(number)
}
fn candidate_identity(candidate: &CandidateEvidence, identity: &PdIdentity) -> Result<(), String> {
    query_identity(&candidate.query, identity)
}
fn query_identity(q: &ScienceQuery, identity: &PdIdentity) -> Result<(), String> {
    let domain = match q.adapter {
        AdapterId::Astronomy => "astronomy", AdapterId::Physics => "physics",
        AdapterId::History => "history", AdapterId::SocialScience => "social-science",
        AdapterId::Psychology => "psychology",
    };
    if identity.domain != domain || q.session_id != hex(&identity.key()?)
        || q.case_id != identity.case_id || q.scope_id != identity.scope
        || q.obligation_id != identity.obligation {
        return Err("embedded scientific candidate does not match the PD identity".into());
    }
    Ok(())
}
fn create(args: &[OsString]) -> Result<String, String> {
    if args.len() != 14 {
        return Err("creation requires exactly 14 arguments; run --help for the contract".into());
    }
    let identity = PdIdentity {
        owner: text_arg(args, 1, "OWNER")?.into(),
        obligation: text_arg(args, 2, "OBLIGATION")?.into(),
        case_id: text_arg(args, 3, "CASE")?.into(),
        scope: text_arg(args, 4, "SCOPE")?.into(),
        domain: "astronomy".into(),
        lineage_root: text_arg(args, 5, "LINEAGE")?.into(),
    };
    identity.validate()?;
    let identity_key = identity.key()?;
    let query = ScienceQuery {
        adapter: AdapterId::Astronomy,
        session_id: hex(&identity_key),
        case_id: identity.case_id.clone(),
        scope_id: identity.scope.clone(),
        obligation_id: identity.obligation.clone(),
        query_id: text_arg(args, 13, "QUERY_ID")?.into(),
        input_source_id: text_arg(args, 12, "INPUT_SOURCE")?.into(),
        task: ScienceTask::EarthRotationAngle {
            jd_part1: number_arg(args, 8, "JD1")?,
            jd_part2: number_arg(args, 9, "JD2")?,
            time_scale: TimeScale::Ut1,
            claim_radians: number_arg(args, 10, "CLAIM_RADIANS")?,
            tolerance_radians: number_arg(args, 11, "TOLERANCE")?,
        },
    };
    let candidate = evaluate(&query).map_err(|error| error.to_string())?;
    candidate_identity(&candidate, &identity)?;
    let science_query = encode_query(&query).map_err(|error| error.to_string())?;
    let canonical_query = decode_query(&science_query).map_err(|error| error.to_string())?;
    if canonical_query != candidate.query {
        return Err("canonical query did not match the evaluated query".into());
    }
    let candidate_wire = encode_candidate(&candidate).map_err(|error| error.to_string())?;
    if decode_candidate(&candidate_wire).map_err(|error| error.to_string())? != candidate {
        return Err("candidate canonical roundtrip mismatch".into());
    }
    let input = ContinuityInput {
        identity,
        operation_id: text_arg(args, 6, "OP")?.into(),
        user_text: text_arg(args, 7, "USER_TEXT")?.into(),
        science_query,
    };
    let bytes = encode_continuity_input(&input)?;
    if decode_continuity_input(&bytes)? != input {
        return Err("continuity input canonical roundtrip mismatch".into());
    }
    let path = Path::new(&args[0]);
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)
        .map_err(|error| format!("OUTPUT was not created (no clobber): {error}"))?;
    file.write_all(&bytes).and_then(|()| file.flush()).and_then(|()| file.sync_all())
        .map_err(|error| format!(
            "OUTPUT write/sync did not complete: {error}; the exclusive file may contain a partial or complete unacknowledged request"
        ))?;
    let mut out = String::new();
    field(&mut out, "format_version", 1);
    field(&mut out, "artifact_type", "continuity_input");
    text_field(&mut out, "output", &path.to_string_lossy());
    field(&mut out, "identity_key", hex(&identity_key));
    text_field(&mut out, "operation_id", &input.operation_id);
    field(&mut out, "input_bytes", bytes.len());
    field(&mut out, "input_sha256", hex(&sha(&bytes)));
    field(&mut out, "query_sha256", hex(&candidate.query_sha256));
    field(&mut out, "computed_radians", candidate.computed_radians);
    field(&mut out, "model_relation", relation(candidate.relation));
    field(&mut out, "authority", "none");
    field(&mut out, "knowledge", "unaccepted");
    Ok(out)
}
fn bounded_read(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|error| format!("record open failed: {error}"))?;
    let length = file.metadata().map_err(|error| format!("record metadata failed: {error}"))?.len();
    if length > limit as u64 { return Err("record exceeds the bounded contract size".into()); }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)
        .map_err(|error| format!("record read failed: {error}"))?;
    if bytes.len() > limit { return Err("record grew beyond the bounded contract size".into()); }
    Ok(bytes)
}
fn inspect(path: &Path) -> Result<String, String> {
    let bytes = bounded_read(path, MAX_REQUEST_BYTES + 256)?;
    let record = decode_record(&bytes)?;
    let request = &record.request;
    let source_input = decode_continuity_input(&request.source_input)?;
    let source_query = decode_query(&source_input.science_query).map_err(|error| error.to_string())?;
    query_identity(&source_query, &request.identity)?;
    let candidate = if request.candidate.is_empty() {
        match evaluate(&source_query) {
            Err(ScienceError::Unavailable { .. }) => None,
            Err(error) => return Err(error.to_string()),
            Ok(_) => return Err("computable scientific candidate was omitted from the record".into()),
        }
    } else {
        let candidate = decode_candidate(&request.candidate).map_err(|error| error.to_string())?;
        candidate_identity(&candidate, &request.identity)?;
        if candidate.query_sha256 != request.candidate_key || candidate.query != source_query {
            return Err("candidate key or exact query does not match retained source input".into());
        }
        Some(candidate)
    };
    // Build output only after full record and candidate integrity validation.
    let mut out = String::new();
    field(&mut out, "format_version", 1);
    field(&mut out, "artifact_type", "operational_pd_record");
    field(&mut out, "revision", record.revision);
    text_field(&mut out, "operation_id", &request.operation_id);
    field(&mut out, "identity_key", hex(&request.identity.key()?));
    field(&mut out, "state_digest", hex(&record.state_digest));
    field(&mut out, "request_digest", hex(&record.request_digest));
    field(&mut out, "source_input_sha256", hex(&sha(&request.source_input)));
    field(&mut out, "record_id", hex(&record.record_id));
    field(&mut out, "previous_record", hex(&record.previous_record));
    field(&mut out, "operational_next_i0", record.operational_next_i0());
    field(&mut out, "checkpoint_sha256", hex(&sha(&request.checkpoint)));
    field(&mut out, "candidate_key", hex(&request.candidate_key));
    field(&mut out, "steps_count", request.lgc_steps.len());
    for (index, step) in request.lgc_steps.iter().enumerate() {
        field(&mut out, &format!("step.{index}.sequence"), step.sequence);
        text_field(&mut out, &format!("step.{index}.site"), &step.site);
        text_field(&mut out, &format!("step.{index}.action"), &step.action);
        field(&mut out, &format!("step.{index}.predecessor"), hex(&step.predecessor));
        field(&mut out, &format!("step.{index}.payload_digest"), hex(&step.payload_digest));
        field(&mut out, &format!("step.{index}.step_digest"), hex(&step.step_digest));
    }
    text_field(&mut out, "witness_condition_code", &request.witness.condition_code);
    text_field(&mut out, "witness_predecessor", &request.witness.predecessor);
    text_field(&mut out, "witness_failed_action", &request.witness.failed_action);
    text_field(&mut out, "witness_reason", &request.witness.reason);
    field(&mut out, "witness_report_class", match request.witness.report_class {
        ReportClass::MissingConditions => "missing_conditions",
        ReportClass::AuthorityRefused => "authority_refused_report",
        ReportClass::TransportUncertain => "transport_uncertain",
    });
    field(&mut out, "witness_permitted_next", match request.witness.permitted_next {
        NextAction::AcquireScopedEvidence => "acquire_scoped_evidence",
        NextAction::ExplainConditions => "explain_conditions",
        NextAction::ResolveCommitStatus => "resolve_commit_status",
    });
    match candidate {
        Some(candidate) => {
            field(&mut out, "candidate_status", "model_integrity_checked");
            text_field(&mut out, "candidate_query_id", &candidate.query.query_id);
            text_field(&mut out, "candidate_input_source", &candidate.query.input_source_id);
            field(&mut out, "candidate_computed_radians", candidate.computed_radians);
            field(&mut out, "candidate_signed_residual_radians", candidate.signed_residual_radians);
            field(&mut out, "candidate_circular_residual_radians", candidate.circular_residual_radians);
            field(&mut out, "candidate_relation", relation(candidate.relation));
            field(&mut out, "candidate_query_sha256", hex(&candidate.query_sha256));
            field(&mut out, "candidate_result_sha256", hex(&candidate.result_sha256));
            text_field(&mut out, "candidate_provider", &candidate.provenance.provider);
            text_field(&mut out, "candidate_provider_version", &candidate.provenance.version);
        }
        None => field(&mut out, "candidate_status", "absent"),
    }
    field(&mut out, "authority", "none");
    field(&mut out, "knowledge", "unaccepted");
    field(&mut out, "inspection", "structural_and_model_integrity_only");
    Ok(out)
}
fn run(args: &[OsString]) -> Result<String, String> {
    match args.first().and_then(|value| value.to_str()) {
        Some("--help") | Some("-h") if args.len() == 1 => Ok(HELP.into()),
        Some("--inspect") if args.len() == 2 => inspect(Path::new(&args[1])),
        Some("--inspect") => Err("--inspect requires exactly one RECORD path".into()),
        Some(flag) if flag.starts_with("--") => Err("unknown flag; run --help".into()),
        _ => create(args),
    }
}
fn main() -> ExitCode {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    match run(&args) {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error={}", quoted(&error));
            ExitCode::from(2)
        }
    }
}
