//! real Shadow selection protocol harness, no knowledge seeding.
//! This constructs source-compatible BESA/ASHT wire requests and uses actual
//! Shadow selection/hold code. It does NOT claim private BESA/ASHT capability
//! constructors, Light tracing or the full constitutional cycle were executed.
//! Exact pure helper origins (namespace/context adapters only):
//! light/src/besa_nlight.rs SHA256 19a19bcaaa4634891a7d5f027f17415d6292f7bb71db1f98c66caec1e2ffe984
//! quantum/src/asht_quantum.rs SHA256 175b9ae1b96cee028db71889143e9518eb2598c8923fa971c7038686c27555df
//! light/src/lgc_algorithm.rs SHA256 08187577523a881822d55ec7fa6e3c29496086c1ad51951e6a68c72f595545d4

use sha2::{Digest, Sha256};
use shadow_contracts::pd_memory::{self as pd, failure};
use shadow_contracts::{
    self as wire, CandidateStatisticWire, KnowledgeSelectionRequestWire,
    KnowledgeSelectionResponseWire, SelectionPurpose, SelectionRequester,
};
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
const HELP:&str="pd_failure_input create OUTPUT OWNER CASE SCOPE USER_TEXT XI YI PRIMITIVE_ANCHOR\nAll values are explicit. Runtime env: ESSMAI_HANDOFF_DIR and ESSMAI_SHADOW_BIN. Use an isolated empty Vault. No seeded P/N, private capability execution or completed GCL verdict is claimed.";
fn create_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("CREATE {}:{e}", path.display()))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("WRITE_OR_SYNC_UNCERTAIN {}:{e}", path.display()))
}
fn sidecar(output: &Path, suffix: &str) -> PathBuf {
    let mut value = output.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}
fn real_selection(
    request: &KnowledgeSelectionRequestWire,
    started: Instant,
) -> Result<(Vec<u8>, KnowledgeSelectionResponseWire), String> {
    if !request.shape_is_valid() {
        return Err("SOURCE_REQUEST_SHAPE".into());
    }
    let encoded = wire::encode_selection_request(request);
    let remaining = Duration::from_secs(90)
        .checked_sub(started.elapsed())
        .ok_or("SELECTION_DEADLINE")?;
    let raw =
        quantum_lim::pd_operational::native_transport("--selection-once", &encoded, remaining)?;
    let response =
        wire::decode_selection_response(&raw).map_err(|e| format!("SELECTION_RESPONSE:{e}"))?;
    if wire::encode_selection_response(&response) != raw
        || !response.shape_is_valid_for(request)
        || response.request_sha256
            != wire::sha256_hex(&[b"ESSMAI/SELECTION/REQUEST/V178", &encoded])
        || response.selection_sha256 != failure::relevance::selection_digest(&response)
        || response.hold_sha256 != selection_hold_digest(&response)
    {
        return Err("ACTUAL_SELECTION_RESPONSE_BINDING".into());
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    if u128::from(response.expires_at_ns) <= now {
        return Err("ACTUAL_SELECTION_HOLD_EXPIRED".into());
    }
    if response.law_seal != quantum_lim::lab_contracts::gjata_collapse_law::law_seal()
        || response.system_laws_seal != quantum_lim::lab_contracts::system_laws_seal()
    {
        return Err("ACTUAL_SELECTION_LAW_SEAL".into());
    }
    if !response.positive.is_empty() || !response.negative.is_empty() {
        return Err(
            "EMPTY_VAULT_CASE_REQUIRED: actual candidates exist; nothing seeded or erased".into(),
        );
    }
    Ok((raw, response))
}

fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => out.push_str(&format!("\\u{:04x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
fn inspect(path: &Path) -> Result<(), String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take((pd::MAX_REQUEST_BYTES + 257) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > pd::MAX_REQUEST_BYTES + 256 {
        return Err("INSPECT_RECORD_LIMIT".into());
    }
    let record = pd::decode_record(&bytes)?;
    let input = failure::validate_failure_operation(&record.request)?;
    let (_, selection) =
        failure::decode_selection_pair(&input.selection_request, &input.selection_response)?;
    let key = pd::hex(&input.identity.key()?);
    let mut engine = quantum_lim::progressive_debatic::core::ProgressiveDebatic::new();
    engine.import_checkpoint(
        &key,
        record
            .request
            .checkpoint
            .strip_prefix(failure::FAILURE_CHECKPOINT_PREFIX)
            .ok_or("INSPECT_CHECKPOINT_POLICY")?,
    )?;
    let session = engine.session(&key).ok_or("INSPECT_SESSION_MISSING")?;
    let q = json_string;
    let linked = if record.request.source_input.starts_with(b"PDS1") {
        Some(pd::linked_science::decode_linked_input(&record.request.source_input)?)
    } else { None };
    let candidate_json = if linked.is_some() {
        let c = science_domains::decode_candidate(&record.request.candidate).map_err(|e|e.to_string())?;
        format!("{{\"adapter\":{},\"authority\":{},\"relation\":{},\"computed_radians\":{},\"circular_residual_radians\":{},\"query_sha256\":{},\"result_sha256\":{}}}",
            q(&format!("{:?}",c.query.adapter)),q(&format!("{:?}",c.authority)),
            q(&format!("{:?}",c.relation)),c.computed_radians,c.circular_residual_radians,
            q(&pd::hex(&c.query_sha256)),q(&pd::hex(&c.result_sha256)))
    } else { "null".into() };
    let mut fields = vec![
        ("source_kind", q(if linked.is_some() {"PDS1"} else {"PDF1"})),
        ("science_candidate", candidate_json),
        ("linked_parent_question_sha256", linked.as_ref().map(|v|q(&pd::hex(&v.parent_question_sha256))).unwrap_or_else(||"null".into())),
        ("relevance_statement", linked.as_ref().map(|v|q(&v.relevance_statement)).unwrap_or_else(||"null".into())),
        ("revision", record.revision.to_string()),
        (
            "expected_revision",
            record.request.expected_revision.to_string(),
        ),
        ("record_id", q(&pd::hex(&record.record_id))),
        ("previous_record", q(&pd::hex(&record.previous_record))),
        ("state_digest", q(&pd::hex(&record.state_digest))),
        ("identity_key", q(&key)),
        ("operation_id", q(&input.operation_id)),
        ("in_reply_to", q(&pd::hex(&input.in_reply_to))),
        (
            "source_input_sha256",
            q(&format!(
                "{:x}",
                Sha256::digest(&record.request.source_input)
            )),
        ),
        ("user_text", q(&input.user_text)),
        ("positive", q(&input.positive)),
        ("negative", q(&input.negative)),
        ("distinction", q(&input.distinction)),
        ("source_reference", q(&input.source_reference)),
        ("refusal", q(&input.refusal)),
        (
            "candidate_empty",
            record.request.candidate.is_empty().to_string(),
        ),
        ("candidate_key", q(&pd::hex(&record.request.candidate_key))),
        (
            "witness_condition",
            q(&record.request.witness.condition_code),
        ),
        (
            "report_class",
            q(&format!("{:?}", record.request.witness.report_class)),
        ),
        ("session_turn_count", session.total_turns.to_string()),
        (
            "trace_signals_count",
            session.trace.signals.len().to_string(),
        ),
        ("session_started_at", session.started_at.to_string()),
        (
            "session_last_activity_at",
            session.last_activity_at.to_string(),
        ),
        ("question", q(&record.request.scoped_question)),
        (
            "checkpoint_sha256",
            q(&format!("{:x}", Sha256::digest(&record.request.checkpoint))),
        ),
        ("knowledge_verdict", "null".into()),
        (
            "provenance",
            q(if input.in_reply_to == [0; 32] {
                "NO_USER_ANSWER"
            } else {
                "USER_REPORTED"
            }),
        ),
        ("owner", q(&input.identity.owner)),
        ("obligation", q(&input.identity.obligation)),
        ("case_id", q(&input.identity.case_id)),
        ("scope", q(&input.identity.scope)),
        ("domain", q(&input.identity.domain)),
        ("lineage_root", q(&input.identity.lineage_root)),
        (
            "selection_positive_count",
            selection.positive.len().to_string(),
        ),
        (
            "selection_negative_count",
            selection.negative.len().to_string(),
        ),
    ];
    fields.push((
        "trace_user_texts",
        format!(
            "[{}]",
            session
                .trace
                .signals
                .iter()
                .map(|s| q(&s.raw_content))
                .collect::<Vec<_>>()
                .join(",")
        ),
    ));
    println!(
        "{{{}}}",
        fields
            .into_iter()
            .map(|(key, value)| format!("{}:{}", q(key), value))
            .collect::<Vec<_>>()
            .join(",")
    );
    Ok(())
}
fn run(args: &[String]) -> Result<(), String> {
    if args.len() == 3 && args[1] == "inspect" {
        return inspect(Path::new(&args[2]));
    }
    if args.len() != 10 || args[1] != "create" {
        return Err(HELP.into());
    }
    let output = Path::new(&args[2]);
    let suffixes = [
        ".besa-request.bin",
        ".besa-response.bin",
        ".asht-request.bin",
        ".asht-response.bin",
    ];
    if output.exists() || suffixes.iter().any(|s| sidecar(output, s).exists()) {
        return Err("OUTPUT_OR_EVIDENCE_EXISTS".into());
    }
    for value in &args[3..9] {
        if value.trim().is_empty() || value.len() > 8192 || value.contains('\0') {
            return Err("INPUT_LIMIT_OR_EMPTY".into());
        }
    }
    let anchor = args[9].parse::<u64>().map_err(|_| "PRIMITIVE_ANCHOR_U64")?;
    if anchor == 0 {
        return Err("PRIMITIVE_ANCHOR_ZERO".into());
    }
    let text = &args[6];
    let parent = format!("{:x}", Sha256::digest(text.as_bytes()));
    let split = quantum_lim::lab_contracts::PrimitiveSplit {
        xi: vec![args[7].clone()],
        yi: vec![args[8].clone()],
    };
    if split.xi == split.yi {
        return Err("XI_YI_MUST_BE_DISTINCT".into());
    }
    let split_sha = lgc_split_sha256(&split);
    let indices = negative_axis_index_witness(&split.xi, &split.yi);
    let law = quantum_lim::lab_contracts::gjata_collapse_law::law_seal();
    let system = quantum_lim::lab_contracts::system_laws_seal();
    // Exact field/digest layout: light/src/lgc_algorithm.rs BESA request.
    let besa = KnowledgeSelectionRequestWire {
        version: wire::PROTOCOL_VERSION,
        requester: SelectionRequester::BesaNlight,
        purpose: SelectionPurpose::XiYiStatistics,
        session_id: args[4].clone(),
        parent_i0: parent.clone(),
        primitive_anchor: anchor,
        trace_id: args[4].clone(),
        split_sha256: split_sha.clone(),
        xi: split.xi.clone(),
        yi: split.yi.clone(),
        upstream_hold_sha256: None,
        evidence_sha256: wire::sha256_hex(&[
            b"ESSMAI/BESA/REQUEST/V178",
            parent.as_bytes(),
            split_sha.as_bytes(),
            indices.xi_sha256.as_bytes(),
            indices.yi_sha256.as_bytes(),
            indices.combined_sha256.as_bytes(),
            &law.to_le_bytes(),
            &system.to_le_bytes(),
        ]),
        max_positive: wire::SELECTION_MAX_CANDIDATES,
        max_negative: wire::SELECTION_MAX_CANDIDATES,
    };
    let started = Instant::now();
    let (besa_raw, selection) = real_selection(&besa, started)?;
    let mut material = Vec::new();
    for c in selection.positive.iter().chain(selection.negative.iter()) {
        append_candidate_material(&mut material, c);
    }
    // Exact BesaNlight::weave_context byte layout, derived from actual returned selection.
    let enrichment = wire::sha256_hex(&[
        b"ESSMAI/BESA/ENRICHMENT/V178",
        besa.trace_id.as_bytes(),
        besa.split_sha256.as_bytes(),
        besa.evidence_sha256.as_bytes(),
        selection.request_sha256.as_bytes(),
        selection.hold_sha256.as_bytes(),
        selection.selection_sha256.as_bytes(),
        indices.xi_sha256.as_bytes(),
        indices.yi_sha256.as_bytes(),
        indices.combined_sha256.as_bytes(),
        &(selection.positive.len() as u64).to_le_bytes(),
        &(selection.negative.len() as u64).to_le_bytes(),
        &selection.law_seal.to_le_bytes(),
        &selection.system_laws_seal.to_le_bytes(),
        &selection.expires_at_ns.to_le_bytes(),
        &material,
    ]);
    // Exact BesaCompanion::attestation_sha256 byte layout. This is a digest,
    // not construction of an active capability or an authority-bearing attestation.
    let attestation = wire::sha256_hex(&[
        b"ESSMAI/BESA/ATTESTATION/V178",
        besa.session_id.as_bytes(),
        besa.parent_i0.as_bytes(),
        &anchor.to_le_bytes(),
        besa.trace_id.as_bytes(),
        besa.split_sha256.as_bytes(),
        selection.hold_id.as_bytes(),
        selection.hold_sha256.as_bytes(),
        selection.selection_sha256.as_bytes(),
        enrichment.as_bytes(),
    ]);
    let mut asht = besa.clone();
    asht.requester = SelectionRequester::AshtQuantum;
    asht.purpose = SelectionPurpose::EvidenceCandidates;
    asht.upstream_hold_sha256 = Some(selection.hold_sha256.clone());
    asht.evidence_sha256 = wire::sha256_hex(&[
        b"ESSMAI/ASHT/REQUEST/V178",
        attestation.as_bytes(),
        parent.as_bytes(),
    ]);
    let (asht_raw, _) = real_selection(&asht, started)?;
    let asht_encoded = wire::encode_selection_request(&asht);
    let missing =
        failure::validate_missing_selection(&asht_encoded, &asht_raw, failure::MISSING_POSITIVE)?;
    let input = failure::FailureDialogueInput {
        identity: pd::PdIdentity {
            owner: args[3].clone(),
            obligation: "ASHT exact positive-negative distinction".into(),
            case_id: args[4].clone(),
            scope: args[5].clone(),
            domain: args[5].clone(),
            lineage_root: parent,
        },
        operation_id: "initial-asht-refusal".into(),
        user_text: text.clone(),
        selection_request: asht_encoded.clone(),
        selection_response: asht_raw.clone(),
        refusal: missing.reason().into(),
        in_reply_to: [0; 32],
        positive: String::new(),
        negative: String::new(),
        distinction: String::new(),
        source_reference: String::new(),
    };
    let output_bytes = failure::encode_failure_input(&input)?;
    for (suffix, bytes) in suffixes.iter().zip([
        wire::encode_selection_request(&besa),
        besa_raw,
        asht_encoded,
        asht_raw,
    ]) {
        create_new(&sidecar(output, suffix), &bytes)?;
    }
    create_new(output, &output_bytes)?;
    println!("real_shadow_selection=true empty_candidate_counts=true private_capabilities_executed=false seeded_knowledge=false verdict=NoVerdict pdf1={}",output.display());
    Ok(())
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if let Err(error) = run(&args) {
        eprintln!("{error}");
        std::process::exit(70);
    }
}

const NEGATIVE_TERM_DOMAIN: &[u8] = b"ESSMAI/SELECTION/TERM/NEGATIVE";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NegativeAxisIndexWitness {
    pub xi_sha256: String,
    pub yi_sha256: String,
    pub combined_sha256: String,
}

/// Light nuk klasifikon dhe nuk rendit kandidatët negativë. Ai zgjon vetëm
/// adresat deterministe që Quantum përdor tashmë për të kërkuar −Xi/−Yi më
/// thellë. Renditja e termave ruhet byte-për-byte dhe hyn në provën GCL.
pub fn negative_axis_index_witness(xi: &[String], yi: &[String]) -> NegativeAxisIndexWitness {
    fn axis_digest(axis: &[u8], terms: &[String]) -> String {
        let mut material = Vec::new();
        for term in terms {
            let address = shadow_contracts::sha256_hex(&[NEGATIVE_TERM_DOMAIN, term.as_bytes()]);
            material.extend_from_slice(&(term.len() as u64).to_le_bytes());
            material.extend_from_slice(term.as_bytes());
            material.extend_from_slice(address.as_bytes());
        }
        shadow_contracts::sha256_hex(&[
            b"ESSMAI/GCL/NEGATIVE-AXIS-INDEX",
            axis,
            material.as_slice(),
        ])
    }

    let xi_sha256 = axis_digest(b"-Xi", xi);
    let yi_sha256 = axis_digest(b"-Yi", yi);
    let combined_sha256 = shadow_contracts::sha256_hex(&[
        b"ESSMAI/GCL/NEGATIVE-AXIS-PAIR",
        xi_sha256.as_bytes(),
        yi_sha256.as_bytes(),
    ]);
    NegativeAxisIndexWitness {
        xi_sha256,
        yi_sha256,
        combined_sha256,
    }
}

fn append_candidate_material(material: &mut Vec<u8>, candidate: &CandidateStatisticWire) {
    material.extend_from_slice(&candidate.candidate_id.to_le_bytes());
    material.push(candidate.polarity);
    material.extend_from_slice(&(candidate.domain.len() as u64).to_le_bytes());
    material.extend_from_slice(candidate.domain.as_bytes());
    material.extend_from_slice(candidate.term_sha256.as_bytes());
    material.extend_from_slice(candidate.source_sha256.as_bytes());
    material.push(candidate.security_y);
    material.push(candidate.trust_x);
    material.extend_from_slice(&candidate.confidence_fixed.to_le_bytes());
    material.extend_from_slice(&candidate.frequency.to_le_bytes());
    material.extend_from_slice(&candidate.domain_count.to_le_bytes());
    material.extend_from_slice(&candidate.legacy_score_fixed.to_le_bytes());
}

fn selection_hold_digest(response: &KnowledgeSelectionResponseWire) -> String {
    let requester = [response.requester as u8];
    let anchor = response.primitive_anchor.to_le_bytes();
    let law_seal = response.law_seal.to_le_bytes();
    let system_laws_seal = response.system_laws_seal.to_le_bytes();
    let expires_at_ns = response.expires_at_ns.to_le_bytes();
    shadow_contracts::sha256_hex(&[
        b"ESSMAI/SELECTION/HOLD/V178",
        &requester,
        response.session_id.as_bytes(),
        response.parent_i0.as_bytes(),
        &anchor,
        response.split_sha256.as_bytes(),
        response
            .upstream_hold_sha256
            .as_deref()
            .unwrap_or("")
            .as_bytes(),
        response.request_sha256.as_bytes(),
        response.selection_sha256.as_bytes(),
        response.hold_id.as_bytes(),
        &law_seal,
        &system_laws_seal,
        &expires_at_ns,
    ])
}

fn lgc_split_sha256(split: &quantum_lim::lab_contracts::PrimitiveSplit) -> String {
    let xi_sha = digest_terms("ESSMAI/LGC/XI/V2", &split.xi);
    let yi_sha = digest_terms("ESSMAI/LGC/YI/V2", &split.yi);
    domain_sha(
        "ESSMAI/LGC/SPLIT/V2",
        &[
            xi_sha.as_bytes(),
            yi_sha.as_bytes(),
            split.to_wire().as_bytes(),
        ],
    )
}

fn digest_terms(domain: &str, terms: &[String]) -> String {
    let mut canonical = Vec::new();
    for term in terms {
        canonical.extend_from_slice(&(term.len() as u64).to_le_bytes());
        canonical.extend_from_slice(term.as_bytes());
    }
    domain_sha(domain, &[canonical.as_slice()])
}

fn domain_sha(domain: &str, parts: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"ESSMAI-DOMAIN-SHA256-V1\0");
    hasher.update((domain.len() as u64).to_le_bytes());
    hasher.update(domain.as_bytes());
    for part in parts {
        hasher.update((part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    format!("{:x}", hasher.finalize())
}
