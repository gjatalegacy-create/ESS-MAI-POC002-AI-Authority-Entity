//! v1.6.4 — complete mediation + scientific project continuum + Living Trust.
//!
//! Only the Shadow executable calls this module. It decodes public wire
//! contracts, opens the sovereign persistent vault, feeds the Light PA gate,
//! invokes the internal Shadow core, and writes a sealed response. No other
//! crate can link the core because the package has `autolib = false`.

use sha2::{Digest, Sha256};
use shadow_contracts::{
    ActionEvidenceWire, LightInboundWire, NegativeKnowledgeWire, ProjectContextWitnessWire,
    ProjectRegistrationRequestWire, ProjectRegistrationResponseWire, ProActionEvidence, QuantumInboundWire,
};
use std::fs::OpenOptions;
use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const REQUIRED_ACTION_ENTRY_COUNT: usize = shadow_contracts::REQUIRED_ACTION_ORDER.len();
const MAX_ACTION_WORDS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequiredActionStage {
    Hpro,
    Pro,
    Npro,
    Srk,
    Apro,
    Mpro,
    Pim,
    Npim,
    Hcp,
}

impl RequiredActionStage {
    fn from_wire(stage: u8) -> Result<Self, String> {
        match stage {
            shadow_contracts::ACTION_STAGE_HPRO => Ok(Self::Hpro),
            shadow_contracts::ACTION_STAGE_PRO => Ok(Self::Pro),
            shadow_contracts::ACTION_STAGE_NPRO => Ok(Self::Npro),
            shadow_contracts::ACTION_STAGE_SRK => Ok(Self::Srk),
            shadow_contracts::ACTION_STAGE_APRO => Ok(Self::Apro),
            shadow_contracts::ACTION_STAGE_MPRO => Ok(Self::Mpro),
            shadow_contracts::ACTION_STAGE_PIM => Ok(Self::Pim),
            shadow_contracts::ACTION_STAGE_NPIM => Ok(Self::Npim),
            shadow_contracts::ACTION_STAGE_HCP => Ok(Self::Hcp),
            unknown => Err(format!(
                "stage mandatory i panjohur në action evidence: {unknown}"
            )),
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Hpro => 0,
            Self::Pro => 1,
            Self::Npro => 2,
            Self::Srk => 3,
            Self::Apro => 4,
            Self::Mpro => 5,
            Self::Pim => 6,
            Self::Npim => 7,
            Self::Hcp => 8,
        }
    }

    const fn expected_words(self, mpro_measurements: usize) -> usize {
        match self {
            Self::Hpro => 5,
            Self::Pro => ProActionEvidence::WORD_COUNT,
            Self::Npro => 4,
            Self::Srk => 6,
            Self::Apro => 4,
            Self::Mpro => mpro_measurements + 5,
            Self::Pim => 6,
            Self::Npim => 5,
            Self::Hcp => 7,
        }
    }
}

#[derive(Debug)]
struct ActionEvidenceView<'a> {
    hpro: &'a [u64],
    pro: ProActionEvidence,
    npro: &'a [u64],
    srk: &'a [u64],
    apro: &'a [u64],
    mpro: &'a [u64],
    pim: &'a [u64],
    npim: &'a [u64],
    hcp: &'a [u64],
}

impl<'a> ActionEvidenceView<'a> {
    fn parse(entries: &'a [ActionEvidenceWire], mpro_measurements: usize) -> Result<Self, String> {
        match entries.len() == REQUIRED_ACTION_ENTRY_COUNT {
            true => {}
            false => {
                return Err(format!(
                    "action evidence kërkon exactly {REQUIRED_ACTION_ENTRY_COUNT} stages; mori {}",
                    entries.len(),
                ))
            }
        }

        let mut slots: [Option<&'a [u64]>; REQUIRED_ACTION_ENTRY_COUNT] =
            [None; REQUIRED_ACTION_ENTRY_COUNT];
        for entry in entries {
            let stage = RequiredActionStage::from_wire(entry.stage)?;
            let expected = stage.expected_words(mpro_measurements);
            let shape_valid = if stage == RequiredActionStage::Hcp {
                shadow_contracts::hcp_family_admission_mask(&entry.evidence_words).is_ok()
            } else if stage == RequiredActionStage::Mpro
                && shadow_contracts::candidate_measurement::CandidateLedger::is_current_words(&entry.evidence_words)
            {
                shadow_contracts::candidate_measurement::CandidateLedger::from_words(&entry.evidence_words).is_ok()
            } else {
                entry.evidence_words.len() == expected
            };
            let word_limit = if stage == RequiredActionStage::Mpro {
                shadow_contracts::candidate_measurement::MAX_CANDIDATE_WORDS
            } else { MAX_ACTION_WORDS };
            match (
                shape_valid,
                entry.evidence_words.len() <= word_limit,
            ) {
                (true, true) => {}
                (false, _) => {
                    return Err(format!(
                        "stage {} ka arity {}; kërkohet exactly {expected}",
                        entry.stage,
                        entry.evidence_words.len(),
                    ))
                }
                (_, false) => {
                    return Err(format!(
                        "stage {} tejkalon kufirin {word_limit} words",
                        entry.stage,
                    ))
                }
            }
            match slots[stage.index()].replace(entry.evidence_words.as_slice()) {
                None => {}
                Some(_) => return Err(format!("duplicate action stage {}", entry.stage)),
            }
        }

        match slots {
            [Some(hpro), Some(pro), Some(npro), Some(srk), Some(apro), Some(mpro), Some(pim), Some(npim), Some(hcp)] => {
                Ok(Self {
                    hpro,
                    pro: ProActionEvidence::try_from_words(pro)
                        .map_err(|error| format!("invalid PRO action evidence: {error}"))?,
                    npro,
                    srk,
                    apro,
                    mpro,
                    pim,
                    npim,
                    hcp,
                })
            }
            _ => Err("mungon të paktën një action stage mandatory".to_string()),
        }
    }
}

/// The resident transport remains inside Shadow main. It grants no admission.
pub fn dispatch_from_args() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();
    match args.as_slice() {
        [_, flag, ready] if flag == "--bridge-worker" =>
            Some(exit_code(crate::resident_worker::serve(Path::new(ready)))),
        [_, mode, request, response] if crate::resident_worker::mode_limits(mode).is_some() => {
            let result = match std::env::var_os("ESSMAI_SHADOW_WORKER") {
                Some(ready) => crate::resident_worker::forward(Path::new(&ready), mode,
                    Path::new(request), Path::new(response)),
                None => dispatch_request(mode, Path::new(request), Path::new(response), None),
            };
            Some(exit_code(result))
        },
        #[cfg(feature = "dev_harness")]
        [_, flag, request, response] if flag == "--negative-once" =>
            Some(exit_code(run_negative(Path::new(request), Path::new(response)))),
        [_, flag, ..] if ["--pd-", "--kie-", "--bridge", "--negative", "--project", "--selection"]
            .iter().any(|prefix| flag.starts_with(prefix)) => Some(64),
        _ => None,
    }
}

/// One process owns both stores. Each wrapper has fresh verification and PA
/// state; the authenticated Vault projection is reused. Existing process globals
/// (kernel statistics and sovereign gate state) keep their process lifetime.
pub(crate) struct ResidentOwner {
    vault: std::sync::Arc<crate::knowledge_vault::KnowledgeVault>,
    _apupk: ApupkProcessLock,
}
impl ResidentOwner {
    pub(crate) fn open() -> Result<Self, String> {
        let apupk = ApupkProcessLock::acquire()?;
        init_apupk_for_process()?;
        let shadow = crate::Shadow::with_disk(super::wisdom_vault_path()).map_err(|e| e.to_string())?;
        Ok(Self { vault: shadow.resident_vault(), _apupk: apupk })
    }
}
fn request_vault(owner: Option<&ResidentOwner>) -> Result<std::sync::Arc<crate::knowledge_vault::KnowledgeVault>, String> {
    match owner {
        Some(owner) => Ok(owner.vault.clone()),
        None => crate::knowledge_vault::KnowledgeVault::open_disk(Path::new(&super::wisdom_vault_path()))
            .map(std::sync::Arc::new).map_err(|e| e.to_string()),
    }
}
fn request_shadow(owner: Option<&ResidentOwner>) -> Result<crate::Shadow, String> {
    match owner {
        Some(owner) => Ok(crate::Shadow::with_resident_vault(owner.vault.clone())),
        None => crate::Shadow::with_disk(super::wisdom_vault_path()).map_err(|e| e.to_string()),
    }
}
fn request_apupk_lock(owner: Option<&ResidentOwner>) -> Result<Option<ApupkProcessLock>, String> {
    match owner { Some(_) => Ok(None), None => ApupkProcessLock::acquire().map(Some) }
}
fn request_apupk_init(owner: Option<&ResidentOwner>) -> Result<(), String> {
    match owner { Some(_) => Ok(()), None => init_apupk_for_process() }
}
pub(crate) fn dispatch_request(mode: &str, request: &Path, response: &Path,
    owner: Option<&ResidentOwner>) -> Result<(), String> {
    match mode {
        "--selection-once" => run_selection(request, response, owner),
        "--kie-select-once" => run_kie_selection(request, response, owner),
        "--project-register-once" => run_project_registration(request, response, owner),
        "--bridge-once" => match owner {
            None => run_cycle(request, response),
            Some(_) => run_cycle_internal(request, response, false, owner),
        },
        "--bridge-pd-once" => run_cycle_internal(request, response, true, owner),
        "--pd-finalize-once" => run_native_finalize(request, response, owner),
        "--pd-recover-once" => run_native_recover(request, response, owner),
        "--pd-lgc-read-once" | "--pd-iz-native-map-once" | "--pd-iz-native-read-once"
        | "--pd-iz-native-lookup-once" | "--pd-iz-map-once" | "--pd-iz-read-once"
        | "--pd-history-once" | "--pd-head-once" | "--pd-candidate-once" | "--pd-commit-once" => {
            let kind = mode.strip_prefix("--pd-").and_then(|v| v.strip_suffix("-once"))
                .ok_or("WORKER_MODE")?;
            run_pd_operational(kind, request, response, owner)
        },
        _ => Err("WORKER_MODE".into()),
    }
}

pub(crate) fn publish_forwarded(mode: &str, response: &Path, bytes: &[u8]) -> Result<(), String> {
    if mode.starts_with("--pd-") || mode == "--kie-select-once" {
        publish_pd_response(response, bytes)
    } else { atomic_write(response, bytes) }
}

fn exit_code(result: Result<(), String>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("[SHADOW MAIN] FAIL-CLOSED: {e}");
            70
        }
    }
}

fn selection_now_ns() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos().min(u128::from(u64::MAX)) as u64)
        .map_err(|error| format!("selection clock refused: {error}"))
}

fn selection_digest(
    positive: &[shadow_contracts::CandidateStatisticWire],
    negative: &[shadow_contracts::CandidateStatisticWire],
) -> String {
    let mut material = Vec::new();
    for candidate in positive.iter().chain(negative.iter()) {
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
    shadow_contracts::sha256_hex(&[b"ESSMAI/SELECTION/CANDIDATES/V178", material.as_slice()])
}

fn selection_handoff_root() -> Result<PathBuf, String> {
    std::env::var("ESSMAI_HANDOFF_DIR")
        .map(PathBuf::from)
        .map_err(|error| format!("ESSMAI_HANDOFF_DIR missing for selection: {error}"))
}

fn run_kie_selection(request_path:&Path,response_path:&Path, owner: Option<&ResidentOwner>)->Result<(),String>{
    require_main_mediation("kie-select-once")?;
    let bytes=read_bounded_request_with_limit(request_path,"KiE selection",shadow_contracts::kie::MAX_WIRE_BYTES)?;
    let request=shadow_contracts::kie::decode_request(&bytes)?;
    let vault=request_vault(owner)?;
    let response=crate::kie_selection::select(&vault,&request,&selection_handoff_root()?,selection_now_ns()?)?;
    publish_pd_response(response_path,&shadow_contracts::kie::encode_response(&response)?)
}
fn run_selection(request_path: &Path, response_path: &Path, owner: Option<&ResidentOwner>) -> Result<(), String> {
    require_main_mediation("selection-once")?;
    let request_bytes = read_bounded_request(request_path, "selection request")?;
    let request = shadow_contracts::decode_selection_request(&request_bytes)
        .map_err(|error| format!("selection request rejected: {error}"))?;
    match request.shape_is_valid() {
        true => {}
        false => return Err("selection request shape rejected".to_string()),
    }

    let shadow = request_shadow(owner)?;
    let (positive, negative) = shadow
        .selection_candidates(&request)
        .map_err(|error| format!("selection projection rejected: {error}"))?;
    let request_sha256 =
        shadow_contracts::sha256_hex(&[b"ESSMAI/SELECTION/REQUEST/V178", request_bytes.as_slice()]);
    let selection_sha256 = selection_digest(&positive, &negative);
    let hold_material = shadow_contracts::sha256_hex(&[
        b"ESSMAI/SELECTION/HOLD-ID/V178",
        request_sha256.as_bytes(),
        selection_sha256.as_bytes(),
    ]);
    let hold_prefix = hold_material
        .get(..32)
        .ok_or_else(|| "selection hold id derivation failed".to_string())?;
    let expires_at_ns = selection_now_ns()?
        .checked_add(15 * 60 * 1_000_000_000)
        .ok_or_else(|| "selection expiry overflow".to_string())?;
    let mut response = shadow_contracts::KnowledgeSelectionResponseWire {
        version: shadow_contracts::PROTOCOL_VERSION,
        requester: request.requester,
        session_id: request.session_id.clone(),
        parent_i0: request.parent_i0.clone(),
        primitive_anchor: request.primitive_anchor,
        split_sha256: request.split_sha256.clone(),
        upstream_hold_sha256: request.upstream_hold_sha256.clone(),
        request_sha256,
        selection_sha256,
        hold_id: format!("h-{hold_prefix}"),
        hold_sha256: String::new(),
        positive,
        negative,
        law_seal: crate::lab_contracts::gjata_collapse_law::law_seal(),
        system_laws_seal: crate::lab_contracts::gjata_collapse_law::system_laws_seal(),
        expires_at_ns,
    };
    response.hold_sha256 =
        crate::selection_hold::SelectionHoldStore::recompute_hold_sha256(&response);
    match response.shape_is_valid_for(&request) {
        true => {}
        false => return Err("selection response failed internal validation".to_string()),
    }
    let store = crate::selection_hold::SelectionHoldStore::under_handoff(selection_handoff_root()?);
    let receipt = store.stage(&response)?;
    match (
        receipt.hold_id == response.hold_id,
        receipt.hold_sha256 == response.hold_sha256,
        receipt.record_path.exists(),
    ) {
        (true, true, true) => atomic_write(
            response_path,
            &shadow_contracts::encode_selection_response(&response),
        ),
        (false, _, _) => Err("selection hold receipt id mismatch".to_string()),
        (_, false, _) => Err("selection hold receipt SHA mismatch".to_string()),
        (_, _, false) => Err("selection hold receipt path missing".to_string()),
    }
}

/// Dry ndër-proces fail-closed për APUPK WAL. Shadow main ekzekutohet si
/// proces one-shot; mutex-i i SovereignLog mbron vetëm thread-et e një procesi.
/// `create_new` garanton një shkrues/lexues sovran në një kohë. Një proces i
/// rrëzuar mund të lërë lock-un; kjo kërkon ndërhyrje operatori dhe nuk
/// rikuperohet automatikisht, sepse rikuperimi pa attestation do të ishte
/// hamendësim mbi pronësinë e magazinës.
struct ApupkProcessLock {
    path: PathBuf,
}

impl ApupkProcessLock {
    fn acquire() -> Result<Self, String> {
        let path = PathBuf::from(super::handoff_path("shadow_apupk_v163.lock"));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| {
                format!(
                    "APUPK WAL është i zënë ose ka lock të pambyllur '{}': {e}",
                    path.display(),
                )
            })?;
        writeln!(
            file,
            "pid={}\nmode=shadow-main-mediation",
            std::process::id()
        )
        .map_err(|e| format!("APUPK lock s'u identifikua: {e}"))?;
        file.flush()
            .map_err(|e| format!("APUPK lock s'u flush-ua: {e}"))?;
        file.sync_all()
            .map_err(|e| format!("APUPK lock s'u fsync-ua: {e}"))?;
        Ok(Self { path })
    }
}

impl Drop for ApupkProcessLock {
    fn drop(&mut self) {
        match std::fs::remove_file(&self.path) {
            Ok(()) => {}
            Err(e) => eprintln!(
                "[SHADOW MAIN/APUPK] ALARM: lock '{}' s'u hoq: {e}",
                self.path.display(),
            ),
        }
    }
}

fn init_apupk_for_process() -> Result<(), String> {
    let path = PathBuf::from(super::handoff_path("shadow_apupk_v163.wal"));
    match crate::shadow_apupk::init_apupk_disk(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(format!("APUPK WAL s'u hap: {e}")),
    }
}

fn read_bounded_request(path: &Path, label: &str) -> Result<Vec<u8>, String> {
    read_bounded_request_with_limit(path, label, shadow_contracts::MAX_FRAME_BYTES)
}

fn read_bounded_request_with_limit(path: &Path, label: &str, limit: usize) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{label} s'u hap: {e}"))?;
    let declared = file
        .metadata()
        .map_err(|e| format!("{label} metadata s'u lexua: {e}"))?
        .len();
    match declared <= limit as u64 {
        true => {}
        false => {
            return Err(format!(
                "{label} tejkalon kufirin wire {} bytes",
                limit,
            ))
        }
    }
    let mut bytes = Vec::with_capacity(declared as usize);
    let mut bounded = file.take(limit as u64 + 1);
    bounded
        .read_to_end(&mut bytes)
        .map_err(|e| format!("{label} s'u lexua: {e}"))?;
    match bytes.len() <= limit {
        true => Ok(bytes),
        false => Err(format!(
            "{label} ndryshoi gjatë leximit dhe tejkaloi kufirin wire {} bytes",
            limit,
        )),
    }
}

fn run_project_registration(request_path: &Path, response_path: &Path, owner: Option<&ResidentOwner>) -> Result<(), String> {
    require_main_mediation("project-register-once")?;
    let request_bytes = read_bounded_request(request_path, "kërkesa APUPK")?;
    let request = shadow_contracts::decode_project_registration_request(&request_bytes)
        .map_err(|e| format!("kërkesa APUPK u refuzua: {e}"))?;

    let response = match validate_project_registration(&request) {
        Ok(()) => {
            let _apupk_lock = request_apupk_lock(owner)?;
            request_apupk_init(owner)?;
            let shadow = request_shadow(owner)?;
            let inbound = crate::shadow_apupk::ApupkInbound {
                trace: crate::shadow_apupk::ApupkTrace {
                    trace_id: request.trace_id,
                    project_id: request.project_id,
                    user_id: request.user_id,
                    initial_trace: request.initial_trace.clone(),
                    timestamp_ns: request.last_update_ns,
                },
                initial_progress: crate::shadow_apupk::ProjectProgress {
                    project_id: request.project_id,
                    progress_percentage: request.progress_percentage,
                    last_update_ns: request.last_update_ns,
                    notes: request.notes.clone(),
                },
                project_title: request.project_title.clone(),
                light_sovereign_flags: request.light_sovereign_flags,
                project_content: request.project_content.clone(),
            };
            shadow.receive_apupk_durable(inbound).map_err(|e| {
                format!("Shadow APUPK refuzoi ruajtjen durable të projektit: {e:?}")
            })?;
            let entry = crate::shadow_apupk::apupk_memory()
                .get(request.project_id)
                .ok_or_else(|| "APUPK store nuk e riktheu projektin e saporuajtur".to_string())?;
            let content_sha256 = format!("{:x}", Sha256::digest(entry.content.as_bytes()));
            let context_sha256 = crate::gcl_project_contract::context_sha256(
                &crate::gcl_project_contract::ProjectContextMaterial {
                    project_id: entry.project_id,
                    user_id: entry.user_id,
                    trace_id: entry.trace_id,
                    revision: entry.revisions,
                    project_title: &entry.project_title,
                    content_sha256: &content_sha256,
                    light_sovereign_flags: entry.light_sovereign_flags,
                },
            );
            ProjectRegistrationResponseWire {
                accepted: true,
                reason_code: 0,
                witness: Some(ProjectContextWitnessWire {
                    project_id: entry.project_id,
                    user_id: entry.user_id,
                    trace_id: entry.trace_id,
                    revision: entry.revisions,
                    content_sha256,
                    light_sovereign_flags: entry.light_sovereign_flags,
                    context_sha256,
                }),
            }
        }
        Err(reason_code) => ProjectRegistrationResponseWire {
            accepted: false,
            reason_code,
            witness: None,
        },
    };
    atomic_write(
        response_path,
        &shadow_contracts::encode_project_registration_response(&response),
    )
}

fn validate_project_registration(request: &ProjectRegistrationRequestWire) -> Result<(), u8> {
    match (
        request.project_id != 0,
        request.user_id != 0,
        request.trace_id != 0,
        !request.project_title.trim().is_empty(),
        request.trace_id
            == crate::gcl_project_contract::project_trace_id(
                request.project_id,
                request.user_id,
                &request.project_title,
            ),
        !request.initial_trace.is_empty(),
        !request.project_content.is_empty(),
        request.progress_percentage.is_finite(),
        crate::gcl_project_contract::seal_is_500(request.light_sovereign_flags),
    ) {
        (true, true, true, true, true, true, true, true, true) => Ok(()),
        (false, _, _, _, _, _, _, _, _) => Err(1),
        (_, false, _, _, _, _, _, _, _) => Err(2),
        (_, _, false, _, _, _, _, _, _) => Err(3),
        (_, _, _, false, _, _, _, _, _) => Err(4),
        (_, _, _, _, false, _, _, _, _) => Err(5),
        (_, _, _, _, _, false, _, _, _) => Err(6),
        (_, _, _, _, _, _, false, _, _) => Err(7),
        (_, _, _, _, _, _, _, false, _) => Err(8),
        (_, _, _, _, _, _, _, _, false) => Err(9),
    }
}

fn run_cycle(request_path: &Path, response_path: &Path) -> Result<(), String> {
    run_cycle_internal(request_path,response_path,false,None)
}
fn run_cycle_internal(request_path: &Path, response_path: &Path, with_pd:bool, owner: Option<&ResidentOwner>) -> Result<(), String> {
    require_main_mediation("bridge-once")?;
    // PDT1 contains the original bounded frame plus its separately bounded PD envelope.
    let request_limit = if with_pd {
        shadow_contracts::pd_cycle::MAX_CYCLE_BYTES
    } else {
        shadow_contracts::MAX_FRAME_BYTES
    };
    let raw = read_bounded_request_with_limit(request_path, "kërkesa wire", request_limit)?;
    let (request_bytes,prepared_bytes)=if with_pd {
        let (request,prepared)=shadow_contracts::pd_cycle::decode_cycle(&raw)?;
        (request,shadow_contracts::pd_prepared::encode_prepared(&prepared)?)
    } else {(raw,Vec::new())};
    let request = shadow_contracts::decode_cycle_request(&request_bytes)
        .map_err(|e| format!("kërkesa wire u refuzua: {e}"))?;

    validate_cycle_identity(&request.quantum, &request.light)?;
    validate_transport_limits(&request.quantum, &request.light)?;
    let mut native_binding=crate::native_cycle::NativeRequestBinding::from_request(&request,&request_bytes)?;
    native_binding.prepared=prepared_bytes;
    // Preserve the source lock order APUPK -> WAL even for exact replay.
    let _apupk_lock=match request.quantum.final_evidence.scientific_project.is_some() {
        true=>request_apupk_lock(owner)?,
        false=>None,
    };
    let vault_path = super::wisdom_vault_path();
    let shadow = request_shadow(owner)?;
    if let Some(bytes)=shadow.replay_native_cycle(&native_binding).map_err(|e|e.to_string())? {
        // Recover post-commit publication as well; no rejudgment, seal, or write.
        publish_negative_export(&shadow)?;
        return atomic_write(response_path,&bytes);
    }
    if let Some(record)=shadow.replay_late_native_refusal(&request_bytes,&native_binding)
        .map_err(|e|e.to_string())? {
        return atomic_write(response_path,&shadow_contracts::pd_memory::encode_record(&record)?);
    }

    // An exact durable retry returned above. New iZ promotion must bind the
    // successful pair observation to the actual Shadow-staged, unexpired hold.
    // Public event hashes alone are not proof that a selection occurred.
    if !native_binding.prepared.is_empty() {
        let prepared=shadow_contracts::pd_prepared::decode_prepared(&native_binding.prepared)?;
        if let Some(link)=&prepared.source_iz {
            let store=crate::selection_hold::SelectionHoldStore::under_handoff(selection_handoff_root()?);
            crate::pd_failure_admission::verify_initial_hold(&store,&link.passed.before.request,
                &link.passed.response,selection_now_ns()?).map_err(|e|format!("PD_IZ_PROMOTION_ACTUAL_HOLD:{e}"))?;
        }
    }
    if let Some(proof)=shadow.verify_native_kie(&request,&selection_handoff_root()?)? {
        native_binding.attach_kie(&proof,&request)?;
    }
    shadow.validate_native_pd_admission(&native_binding).map_err(|e|format!("PD_NATIVE_PREDECESSOR:{e}"))?;
    validate_negative_shape(&request.negative)?;
    let validated_negative_asset = validate_final_evidence(&request.quantum, request.negative)?;
    let measurement = request.quantum.final_evidence.candidate_measurement()?;
    let current_measurement = measurement.is_some();
    let current_matrix = measurement.as_ref().is_some_and(|(ledger, _)| ledger.uses_current_matrix());
    if request.quantum.final_evidence.scientific_project.is_some() {
        request_apupk_init(owner)?;
        validate_scientific_project_against_apupk(&request.quantum)?;
    }


    let pa_path = super::pa_export_path();
    let fed = shadow
        .feed_primitive_anchors(&pa_path, &request.quantum.session_id)
        .map_err(|e| format!("PA gate refuzoi sesionin: {e}"))?;
    println!(
        "  [SHADOW MAIN/MEDIATION] session={} PA_fed={} vault={}",
        request.quantum.session_id, fed, vault_path
    );

    println!(
        "  [SHADOW/GCL NK] token={} targets={} experimental_failure={} project={} TRL{} staged_for_gateway=true",
        validated_negative_asset.token_hex,
        validated_negative_asset.target_count,
        validated_negative_asset.experimental_failure,
        validated_negative_asset.project_id,
        validated_negative_asset.trl_level,
    );
    let response = if current_matrix {
        shadow.ingest_bridged_scoped_recorded(
            into_quantum(request.quantum, Some(validated_negative_asset)),
            into_light(request.light),
            Some(&native_binding),
        )
    } else if current_measurement {
        shadow.ingest_bridged_current_recorded(
            into_quantum(request.quantum, Some(validated_negative_asset)),
            into_light(request.light),
            Some(&native_binding),
        )
    } else {
        shadow.ingest_bridged_recorded(
            into_quantum(request.quantum, Some(validated_negative_asset)),
            into_light(request.light),
            Some(&native_binding),
        )
    };
    let response=match response {
        Ok(response)=>response,
        Err(error @ crate::types::ShadowError::StructuralRefusal(crate::types::StructuralRefusal::MissingNegativeProof))
            if !native_binding.prepared.is_empty() && !native_binding.kie_proof.is_empty()=>{
                let record=shadow.record_late_native_refusal(&request_bytes,&native_binding,&error)
                    .map_err(|e|format!("PD_LATE_REFUSAL_PERSISTENCE:{e}"))?;
                // Operational PDR1, no native verdict, receipt, T1/T2 or export.
                return atomic_write(response_path,&shadow_contracts::pd_memory::encode_record(&record)?);
            }
        Err(error)=>return Err(format!("ingest_bridged dështoi: {error}")),
    };

    // The verified vault is republished after every mediated cycle. This closes
    // the existing Shadow→Quantum feedback line without changing the verdict:
    // the next cycle imports the sealed Negative Knowledge as a PRO double-gate.
    publish_negative_export(&shadow)?;

    let wire = crate::native_cycle::response_wire(response);
    atomic_write(
        response_path,
        &shadow_contracts::encode_cycle_response(&wire),
    )
}

#[cfg(feature = "dev_harness")]
fn run_negative(request_path: &Path, response_path: &Path) -> Result<(), String> {
    require_main_mediation("negative-once")?;
    let request_bytes = read_bounded_request(request_path, "kërkesa negative")?;
    let request = shadow_contracts::decode_negative_request(&request_bytes)
        .map_err(|e| format!("kërkesa negative u refuzua: {e}"))?;
    validate_negative_shape(&request.negative)?;

    let shadow = crate::Shadow::with_disk(super::wisdom_vault_path())
        .map_err(|e| format!("vault-i sovran s'u hap: {e}"))?;
    let persisted = persist_negative(&shadow, &request.negative).is_ok();
    let negative_export = match (persisted, request.export_negative_knowledge) {
        (true, true) => shadow.export_negative_knowledge(),
        _ => Vec::new(),
    };
    let response = shadow_contracts::ShadowNegativeResponse {
        persisted,
        negative_export,
    };
    atomic_write(
        response_path,
        &shadow_contracts::encode_negative_response(&response),
    )
}

fn require_main_mediation(site: &str) -> Result<(), String> {
    match crate::phase9_integration::Phase9Integration::verify_no_bypass(site) {
        true => Ok(()),
        false => Err(format!("Phase9 refuzoi bypass-in te {site}")),
    }
}

fn validate_negative_shape(n: &NegativeKnowledgeWire) -> Result<(), String> {
    match (
        n.mass.is_finite(),
        n.suggestion_code <= 2,
        !n.process.is_empty(),
        !n.bytes.is_empty(),
    ) {
        (true, true, true, true) => Ok(()),
        (false, _, _, _) => Err("Negative Knowledge ka masë NaN/Infinity".to_string()),
        (_, false, _, _) => Err("Negative Knowledge ka suggestion_code jashtë {0,1,2}".to_string()),
        (_, _, false, _) => Err("Negative Knowledge ka process bosh".to_string()),
        (_, _, _, false) => Err("Negative Knowledge ka evidence blob bosh".to_string()),
    }
}

fn validate_cycle_identity(q: &QuantumInboundWire, l: &LightInboundWire) -> Result<(), String> {
    let identity = match (
        q.session_id.is_empty(),
        q.session_id == l.session_id,
        q.territory == l.territory,
        q.primitive_flags == l.primitive_flags,
    ) {
        (false, true, true, true) => Ok(()),
        (true, _, _, _) => Err("session_id bosh".to_string()),
        (_, false, _, _) => Err("origjinat Quantum/Light kanë session të ndryshëm".to_string()),
        (_, _, false, _) => Err("origjinat Quantum/Light kanë territory të ndryshëm".to_string()),
        (_, _, _, false) => {
            Err("origjinat Quantum/Light kanë primitive_flags të ndryshëm".to_string())
        }
    };
    identity?;
    // Public data binding, not another authority: real held BESA/ASHT evidence
    // remains subject to the existing NBES/GCL admission after this check.
    q.final_evidence.validate_source_lineage(
        &q.session_id, &q.territory, q.pd_parent_i0.as_deref(),
        q.primitive_flags, &l.proof_chain,
    )
}

fn validate_transport_limits(q: &QuantumInboundWire, l: &LightInboundWire) -> Result<(), String> {
    let evidence = &q.final_evidence;
    let proof_text_bounded = evidence
        .pim_proof_chain
        .iter()
        .chain(evidence.npim_arguments.iter())
        .all(|item| item.len() <= shadow_contracts::MAX_FIELD_BYTES);
    match (
        q.candidate_scores.len() <= shadow_contracts::MAX_VECTOR_ITEMS,
        l.proof_chain.len() <= shadow_contracts::MAX_VECTOR_ITEMS,
        evidence.pim_proof_chain.len() <= shadow_contracts::MAX_VECTOR_ITEMS,
        evidence.npim_arguments.len() <= shadow_contracts::MAX_VECTOR_ITEMS,
        evidence.quantum_action_evidence.len() <= REQUIRED_ACTION_ENTRY_COUNT,
        proof_text_bounded,
    ) {
        (true, true, true, true, true, true) => Ok(()),
        (false, _, _, _, _, _) => Err("candidate_scores tejkalon kufirin e kontratës".to_string()),
        (_, false, _, _, _, _) => Err("Light proof_chain tejkalon kufirin e kontratës".to_string()),
        (_, _, false, _, _, _) => Err("PIM proof_chain tejkalon kufirin e kontratës".to_string()),
        (_, _, _, false, _, _) => Err("NPIM arguments tejkalojnë kufirin e kontratës".to_string()),
        (_, _, _, _, false, _) => {
            Err("action evidence tejkalon numrin mandatory të stages".to_string())
        }
        (_, _, _, _, _, false) => Err("proof text tejkalon kufirin e kontratës".to_string()),
    }
}

/// Independently bind only the candidate-derived prefix. The last three PRO
/// words remain digest-bound producer observations; Shadow has no OS/resource
/// attestation or original fragment-prefix material here.
fn pro_matches_candidates(pro: &ProActionEvidence, candidate_scores: &[f32]) -> bool {
    let scores_valid = candidate_scores
        .iter()
        .all(|score| score.is_finite() && (0.0..=1.0).contains(score))
        && candidate_scores.windows(2).all(|pair| pair[0] >= pair[1]);
    let first = match candidate_scores.first() {
        Some(score) => shadow_contracts::fixed_mass(*score).map(u64::from),
        None => Some(0),
    };
    let last = match candidate_scores.last() {
        Some(score) => shadow_contracts::fixed_mass(*score).map(u64::from),
        None => Some(0),
    };
    pro.validate().is_ok()
        && scores_valid
        && usize::try_from(pro.candidate_count).ok() == Some(candidate_scores.len())
        && first == Some(pro.first_score_fixed)
        && last == Some(pro.last_score_fixed)
}

/// Arithmetic and exact inventory binding for R23. This checks current input
/// and producer material, not historical voting or model truth. Fragment refs
/// are producer provenance, not independent proof of an operator execution.
fn candidate_ledger_matches_pro(
    ledger: &shadow_contracts::candidate_measurement::CandidateLedger,
    pro: &ProActionEvidence,
    scores: &[f32],
    operator_diagnostics: &[u8],
) -> bool {
    use shadow_contracts::candidate_measurement::CandidatePolarity;
    let positives: Vec<_> = ledger.observations().iter()
        .filter(|entry| entry.polarity == CandidatePolarity::Positive).collect();
    let mut presence = [0u8; 4];
    for entry in &positives {
        presence[usize::from(entry.operator - 1)] = 1;
    }
    pro_matches_candidates(pro, scores)
        && positives.len() == scores.len()
        && positives.iter().zip(scores).all(|(entry, value)| {
            shadow_contracts::fixed_mass(*value) == Some(entry.value)
                && entry.fragment_refs.iter().all(|index| u64::from(*index) < pro.fragments_admitted)
        })
        && operator_diagnostics == presence
}

/// Preserve the exact historical 16 observations + five scalar words binding.
/// An absent new-format marker is not enough: the whole legacy action must
/// still equal the material independently supplied by FinalEvidence.
fn legacy_mpro_matches(measurements: &[u8], scalars: [u32; 5], action: &[u64]) -> bool {
    action.len() == measurements.len() + scalars.len()
        && action[..measurements.len()].iter().copied()
            .eq(measurements.iter().map(|value| u64::from(*value)))
        && action[measurements.len()..].iter().copied()
            .eq(scalars.into_iter().map(u64::from))
}

fn validate_final_evidence(
    q: &QuantumInboundWire,
    negative: NegativeKnowledgeWire,
) -> Result<crate::negative_asset::ValidatedNegativeAsset, String> {
    let finite_scalars = [
        q.epistemic_mass,
        q.evidence_density,
        q.logical_coherence,
        q.causal_integrity,
        q.convergence_strength,
        q.reproducibility,
        q.potentiality_score,
        q.ibe_score,
        q.chain_coherence,
        negative.mass,
    ]
    .iter()
    .all(|value| value.is_finite())
        && q.candidate_scores.iter().all(|value| value.is_finite());
    match finite_scalars {
        true => {}
        false => return Err("NaN/Infinity u refuzua para Shadow core".to_string()),
    }

    let evidence = &q.final_evidence;
    let action_evidence = ActionEvidenceView::parse(
        &evidence.quantum_action_evidence,
        evidence.mpro_measurements.len(),
    )?;
    match evidence.verifies_internal() {
        true => {}
        false => return Err("paketa finale PIM/NPIM/MPRO nuk rillogaritet".to_string()),
    }
    let input_sha256 = format!("{:x}", Sha256::digest(&evidence.light_input_bytes));
    match input_sha256 == evidence.light_input_sha256.to_ascii_lowercase() {
        true => {}
        false => return Err("SHA-256 i inputit Light nuk rillogaritet në Shadow".to_string()),
    }
    match evidence.scientific_project.as_ref() {
        Some(project) => validate_scientific_project_material(project, evidence)?,
        None => {}
    }
    match shadow_contracts::fnv1a64(&negative.bytes) == evidence.npim_arguments_blob_digest {
        true => {}
        false => {
            return Err(
                "blob-i NPIM/Negative Knowledge nuk përputhet me paketën finale".to_string(),
            )
        }
    }

    let pd = &evidence.pd_spine;
    let continuum_identity = pd.created_at == pd.continuum.stimulus_created_at
        && pd.session_digest == pd.continuum.session_digest
        && pd.parent_i0_digest == pd.continuum.i0_digest
        && pd.activation.response_kind_digest == pd.continuum.response_kind_digest
        && pd.continuum.user_input_digest
            == shadow_contracts::digest_parts(&[evidence.light_input_bytes.as_slice()])
        && pd.continuum.initial_i_digest
            == shadow_contracts::digest_parts(&[q.session_id.as_bytes(), b"GCL_INITIAL_I"]);
    let spine_material_identity = match q.hcp_token_gen {
        Some(token_gen) => {
            let mut layer1_material = Vec::new();
            layer1_material.extend_from_slice(&token_gen.to_le_bytes());
            layer1_material.push(pd.layer1.completed as u8);
            let mut layer3_material = Vec::new();
            layer3_material.extend_from_slice(&pd.mpro_mass.to_le_bytes());
            layer3_material.push(pd.layer3.completed as u8);
            let expected_completion_evidence = shadow_contracts::digest_parts(&[
                &pd.layer2.result_material_digest.to_le_bytes(),
                &token_gen.to_le_bytes(),
                &pd.mpro_mass.to_le_bytes(),
                &pd.activation_contract_digest.to_le_bytes(),
            ]);
            pd.layer1.result_material == layer1_material
                && pd.layer3.result_material == layer3_material
                && pd.evidence_digest == expected_completion_evidence
        }
        None => false,
    };
    match (
        pd.gcl_law_seal == crate::lab_contracts::gjata_collapse_law::law_seal(),
        pd.gcl_system_laws_seal == crate::lab_contracts::gjata_collapse_law::system_laws_seal(),
        pd.session_digest == shadow_contracts::digest_parts(&[q.session_id.as_bytes()]),
        match q.pd_parent_i0.as_deref() {
            Some(parent) => {
                pd.parent_i0_digest == shadow_contracts::digest_parts(&[parent.as_bytes()])
            }
            None => false,
        },
        q.pd_continuum_activation_digest == Some(pd.continuum_activation_digest),
        continuum_identity,
        spine_material_identity,
    ) {
        (true, true, true, true, true, true, true) => {}
        (false, _, _, _, _, _, _) | (_, false, _, _, _, _, _) => {
            return Err("paketa finale nuk i përket ligjit GCL të Shadow".to_string())
        }
        (_, _, false, _, _, _, _) => {
            return Err("session-i Quantum nuk përputhet me procesin PD/Spine9".to_string())
        }
        (_, _, _, false, _, _, _) => {
            return Err("parent i₀ nuk përputhet me procesin PD/Spine9".to_string())
        }
        (_, _, _, _, false, _, _) => {
            return Err("continuum activation nuk përputhet me procesin PD/Spine9".to_string())
        }
        (_, _, _, _, _, false, _) => {
            return Err("i + U → i₀ → 1Q nuk përputhet me identitetin Light/Quantum".to_string())
        }
        (_, _, _, _, _, _, false) => {
            return Err(
                "materiali i Layer 1/2/3 ose completion evidence nuk rillogaritet".to_string(),
            )
        }
    }

    let fixed = |value: f32| -> Result<u32, String> {
        shadow_contracts::fixed_mass(value)
            .ok_or_else(|| "masë jo-finite në paketën finale".to_string())
    };
    let hpro_bound = match action_evidence.hpro {
        [activation, generation, nonce, depth, sealed] => {
            *activation != 0
                && u64::from((*generation >= 1) as u8) == u64::from(evidence.mpro_measurements[8])
                && *nonce == u64::from(evidence.mpro_measurements[9])
                && u64::from((*sealed > 0) as u8) == u64::from(evidence.mpro_measurements[10])
                && *depth == u64::from(evidence.mpro_measurements[11])
        }
        _ => false,
    };
    let pro_bound = pro_matches_candidates(&action_evidence.pro, &q.candidate_scores);
    let npro_bound = match action_evidence.npro {
        [argument_count, weakness, _, negativity] => {
            *argument_count == evidence.npim_arguments.len() as u64
                && *weakness <= u64::from(shadow_contracts::MASS_SCALE)
                && *negativity <= u64::from(shadow_contracts::MASS_SCALE)
        }
        _ => false,
    };
    let srk_bound = match action_evidence.srk {
        [candidate_count, proof_count, total, admitted, bayes, ibe] => {
            *candidate_count <= q.candidate_scores.len() as u64
                && *proof_count == evidence.pim_proof_chain.len() as u64
                && *admitted <= *total
                && *bayes <= u64::from(shadow_contracts::MASS_SCALE)
                && *ibe <= u64::from(shadow_contracts::MASS_SCALE)
        }
        _ => false,
    };
    let apro_expected = evidence.mpro_measurements[12..16]
        .iter()
        .map(|value| u64::from(*value))
        .collect::<Vec<_>>();
    let apro_bound = action_evidence.apro == apro_expected.as_slice();
    let mpro_bound = match evidence.candidate_measurement()? {
        Some((ledger, _)) => candidate_ledger_matches_pro(
            &ledger, &action_evidence.pro, &q.candidate_scores,
            &evidence.mpro_measurements[..4],
        ),
        None => legacy_mpro_matches(&evidence.mpro_measurements, [
            evidence.mpro_positives,
            evidence.mpro_total,
            evidence.mpro_vector_mass,
            evidence.mpro_evidence_mass,
            evidence.mpro_factic_mass,
        ], action_evidence.mpro),
    };
    let pim_expected = [
        u64::from(evidence.pim_evidence_density),
        u64::from(evidence.pim_logical_coherence),
        u64::from(evidence.pim_causal_integrity),
        u64::from(evidence.pim_convergence_strength),
        u64::from(evidence.pim_reproducibility),
        evidence.pim_proof_chain.len() as u64,
    ];
    let pim_action_bound = action_evidence.pim == pim_expected.as_slice();
    let npim_expected = [
        u64::from(evidence.npim_failure_density),
        u64::from(evidence.npim_contradiction_strength),
        u64::from(evidence.npim_elimination_certainty),
        u64::from(evidence.npim_negativity_score),
        evidence.npim_arguments.len() as u64,
    ];
    let npim_action_bound = action_evidence.npim == npim_expected.as_slice();
    let hcp_bound = match (
        Some(action_evidence.hcp),
        q.hcp_input_id,
        q.hcp_token_gen,
        q.hcp_token_nonce,
        q.hcp_directive_bit,
    ) {
        (Some(words), Some(id), Some(generation), Some(nonce), Some(bit)) => {
            let expected_prefix = [id, generation, nonce, u64::from(bit), 1];
            let prefix_bound =
                words.get(..expected_prefix.len()) == Some(expected_prefix.as_slice());
            let lease_bound = words.get(5).copied().unwrap_or(0) != 0;
            let active_family_bound = words.get(6).copied().unwrap_or(u64::MAX) <= 4;
            shadow_contracts::hcp_family_admission_mask(words).is_ok()
                && bit <= 1 && prefix_bound && lease_bound && active_family_bound
        }
        _ => false,
    };
    match (
        hpro_bound,
        pro_bound,
        npro_bound,
        srk_bound,
        apro_bound,
        mpro_bound,
        pim_action_bound,
        npim_action_bound,
        hcp_bound,
    ) {
        (true, true, true, true, true, true, true, true, true) => {}
        _ => return Err(
            "ledger-i Untrust nuk lidhet fushë-për-fushë me HPRO/PRO/NPRO/SRK/APRO/MPRO/PIM/NPIM/HCP"
                .to_string(),
        ),
    }

    let pim_matches = fixed(q.evidence_density)? == evidence.pim_evidence_density
        && fixed(q.logical_coherence)? == evidence.pim_logical_coherence
        && fixed(q.causal_integrity)? == evidence.pim_causal_integrity
        && fixed(q.convergence_strength)? == evidence.pim_convergence_strength
        && fixed(q.reproducibility)? == evidence.pim_reproducibility
        && fixed(q.potentiality_score)? == evidence.pim_potentiality_score
        && q.suggested_verdict == evidence.pim_suggestion;
    let npim_matches = fixed(negative.mass)? == evidence.npim_negativity_score
        && negative.frequency as usize == evidence.npim_arguments.len()
        && negative.suggestion_code == evidence.npim_suggestion;
    let mpro_matches = fixed(q.epistemic_mass)? == evidence.mpro_evidence_mass
        && match (&q.xy_mass, &q.collapse_refused) {
            (Some(mass), None) => *mass == evidence.mpro_factic_mass,
            (None, Some(_)) => true,
            _ => false,
        };
    match (pim_matches, npim_matches, mpro_matches) {
        (true, true, true) => Ok(()),
        (false, _, _) => Err("PIM wire nuk përputhet me paketën finale".to_string()),
        (_, false, _) => Err("NPIM wire nuk përputhet me argumentet negative".to_string()),
        (_, _, false) => Err("MPRO/XY nuk përputhet me 16 matjet e paketuara".to_string()),
    }?;

    crate::negative_asset::validate(q, negative)
}

fn validate_scientific_project_material(
    project: &shadow_contracts::ScientificProjectWire,
    evidence: &shadow_contracts::FinalEvidenceWire,
) -> Result<(), String> {
    let context = &project.context;
    let expected_context = crate::gcl_project_contract::context_sha256(
        &crate::gcl_project_contract::ProjectContextMaterial {
            project_id: context.project_id,
            user_id: context.user_id,
            trace_id: context.trace_id,
            revision: context.revision,
            project_title: &project.project_title,
            content_sha256: &context.content_sha256,
            light_sovereign_flags: context.light_sovereign_flags,
        },
    );
    let files = project
        .files
        .iter()
        .map(|f| (f.kind, f.bytes.clone()))
        .collect::<Vec<_>>();
    let expected_evidence = crate::gcl_project_contract::evidence_sha256(
        &crate::gcl_project_contract::ProjectEvidenceMaterial {
            context_sha256: &context.context_sha256,
            project_title: &project.project_title,
            domain: &project.domain,
            hypothesis: &project.hypothesis,
            assumptions: &project.assumptions,
            gcl_process_digest: project.gcl_process_digest,
            trl_level: project.trl_level,
            trl_passed: project.trl_passed,
            trl_confidence: project.trl_confidence,
            trl_reproducibility: project.trl_reproducibility,
            lab_test_id: &project.lab_test_id,
            findings: &project.findings,
            description: &project.description,
            files: &files,
        },
    );
    match (
        project.shape_is_valid(),
        context.trace_id
            == crate::gcl_project_contract::project_trace_id(
                context.project_id,
                context.user_id,
                &project.project_title,
            ),
        context.content_sha256 == evidence.light_input_sha256.to_ascii_lowercase(),
        crate::gcl_project_contract::seal_is_500(context.light_sovereign_flags),
        expected_context == context.context_sha256.to_ascii_lowercase(),
        project.gcl_process_digest == evidence.pd_spine.gcl_process_digest,
        expected_evidence == project.evidence_sha256.to_ascii_lowercase(),
    ) {
        (true, true, true, true, true, true, true) => Ok(()),
        (false, _, _, _, _, _, _) => {
            Err("projekti shkencor ka formë wire të pavlefshme".to_string())
        }
        (_, false, _, _, _, _, _) => {
            Err("titulli i projektit nuk prodhon APUPK trace_id e Light".to_string())
        }
        (_, _, false, _, _, _, _) => {
            Err("projekti nuk lidhet me SHA-256 e inputit Light".to_string())
        }
        (_, _, _, false, _, _, _) => {
            Err("projekti nuk mban Vulën Sovrane 500 të Light".to_string())
        }
        (_, _, _, _, false, _, _) => Err("ProjectContextWitness nuk rillogaritet".to_string()),
        (_, _, _, _, _, false, _) => {
            Err("projekti nuk procedoi nën të njëjtin GCL process të Spine 9".to_string())
        }
        (_, _, _, _, _, _, false) => {
            Err("evidenca shkencore e projektit nuk rillogaritet".to_string())
        }
    }
}

fn validate_scientific_project_against_apupk(q: &QuantumInboundWire) -> Result<(), String> {
    let project = match q.final_evidence.scientific_project.as_ref() {
        Some(project) => project,
        None => return Ok(()),
    };
    let entry = crate::shadow_apupk::apupk_memory()
        .get(project.context.project_id)
        .ok_or_else(|| "projekti shkencor nuk ekziston në Shadow APUPK".to_string())?;
    let stored_sha256 = format!("{:x}", Sha256::digest(entry.content.as_bytes()));
    match (
        entry.project_id == project.context.project_id,
        entry.user_id == project.context.user_id,
        entry.trace_id == project.context.trace_id,
        entry.revisions == project.context.revision,
        entry.project_title == project.project_title,
        entry.light_sovereign_flags == project.context.light_sovereign_flags,
        stored_sha256 == project.context.content_sha256.to_ascii_lowercase(),
    ) {
        (true, true, true, true, true, true, true) => Ok(()),
        (false, _, _, _, _, _, _) => Err("APUPK project_id mismatch".to_string()),
        (_, false, _, _, _, _, _) => Err("APUPK user_id mismatch".to_string()),
        (_, _, false, _, _, _, _) => Err("APUPK trace_id mismatch".to_string()),
        (_, _, _, false, _, _, _) => Err("APUPK revision mismatch/replay".to_string()),
        (_, _, _, _, false, _, _) => Err("APUPK project_title mismatch".to_string()),
        (_, _, _, _, _, false, _) => Err("APUPK Vula 500 origin mismatch".to_string()),
        (_, _, _, _, _, _, false) => Err("APUPK content SHA-256 mismatch".to_string()),
    }
}

fn into_quantum(
    q: QuantumInboundWire,
    validated_negative_asset: Option<crate::negative_asset::ValidatedNegativeAsset>,
) -> crate::bridge::QuantumInbound {
    crate::bridge::QuantumInbound {
        package_id: q.package_id,
        session_id: q.session_id,
        territory: q.territory,
        epistemic_mass: q.epistemic_mass,
        evidence_density: q.evidence_density,
        logical_coherence: q.logical_coherence,
        causal_integrity: q.causal_integrity,
        convergence_strength: q.convergence_strength,
        reproducibility: q.reproducibility,
        potentiality_score: q.potentiality_score,
        ibe_score: q.ibe_score,
        chain_coherence: q.chain_coherence,
        conservation_ok: q.conservation_ok,
        candidate_scores: q.candidate_scores,
        suggested_verdict: q.suggested_verdict,
        primitive_flags: q.primitive_flags,
        raw_bytes: q.raw_bytes,
        xy_x: q.xy_x,
        xy_y: q.xy_y,
        xy_mass: q.xy_mass,
        pd_binding_digest: q.pd_binding_digest,
        pd_continuum_activation_digest: q.pd_continuum_activation_digest,
        pd_parent_i0: q.pd_parent_i0,
        collapse_refused: q.collapse_refused,
        negative_paths: q.negative_paths,
        hcp_input_id: q.hcp_input_id,
        hcp_token_gen: q.hcp_token_gen,
        hcp_token_nonce: q.hcp_token_nonce,
        hcp_directive_bit: q.hcp_directive_bit,
        final_evidence_digest: q.final_evidence.package_digest,
        quantum_action_state: q.final_evidence.quantum_action_state,
        quantum_action_mask: q.final_evidence.quantum_action_mask,
        quantum_required_action_mask: q.final_evidence.quantum_required_action_mask,
        pd_gcl_process_digest: q.final_evidence.pd_spine.gcl_process_digest,
        spine_completion_digest: q.final_evidence.pd_spine.completion_digest,
        muscle_evidence: Some(q.final_evidence.muscle_evidence),
        scientific_project: q
            .final_evidence
            .scientific_project
            .map(into_scientific_project),
        validated_negative_asset,
    }
}

fn into_scientific_project(
    p: shadow_contracts::ScientificProjectWire,
) -> crate::types::ScientificProjectContext {
    crate::types::ScientificProjectContext {
        project_id: p.context.project_id,
        user_id: p.context.user_id,
        trace_id: p.context.trace_id,
        revision: p.context.revision,
        content_sha256: p.context.content_sha256,
        light_sovereign_flags: p.context.light_sovereign_flags,
        context_sha256: p.context.context_sha256,
        project_title: p.project_title,
        domain: p.domain,
        hypothesis: p.hypothesis,
        assumptions: p.assumptions,
        gcl_process_digest: p.gcl_process_digest,
        trl_level: p.trl_level,
        trl_passed: p.trl_passed,
        trl_confidence: p.trl_confidence,
        trl_reproducibility: p.trl_reproducibility,
        lab_test_id: p.lab_test_id,
        findings: p.findings,
        description: p.description,
        files: p.files.into_iter().map(|f| (f.kind, f.bytes)).collect(),
        evidence_sha256: p.evidence_sha256,
    }
}

fn into_light(l: LightInboundWire) -> crate::bridge::LightInbound {
    crate::bridge::LightInbound {
        session_id: l.session_id,
        territory: l.territory,
        primitive_flags: l.primitive_flags,
        epistemic_mass: l.epistemic_mass,
        proof_chain: l.proof_chain,
        payload: l.payload,
    }
}

#[cfg(feature = "dev_harness")]
fn persist_negative(shadow: &crate::Shadow, n: &NegativeKnowledgeWire) -> Result<(), String> {
    shadow
        .on_negative(
            n.input_id,
            n.reason,
            &n.process,
            n.mass,
            n.frequency,
            &n.bytes,
            n.ts_ns,
        )
        .map_err(|e| format!("on_negative dështoi: {e}"))
}

fn publish_negative_export(shadow: &crate::Shadow) -> Result<(), String> {
    let body = shadow.export_negative_knowledge();
    let mut sealed = Vec::with_capacity(20usize.saturating_add(body.len()));
    sealed.extend_from_slice(b"NKL1");
    sealed.extend_from_slice(&(body.len() as u64).to_le_bytes());
    sealed.extend_from_slice(&shadow_contracts::fnv1a64(&body).to_le_bytes());
    sealed.extend_from_slice(&body);

    let export_path = PathBuf::from(super::handoff_path("shadow_nk_export.bin"));
    let marker_path = PathBuf::from(super::handoff_path("shadow_nk_export.sealed"));
    atomic_write(&export_path, &sealed)?;
    atomic_write(&marker_path, b"NKL1\n")?;
    println!(
        "  [SHADOW/GCL NK] living export published: entries_blob={} sealed_blob={}",
        body.len(),
        sealed.len(),
    );
    Ok(())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "response path pa parent".to_string())?;
    std::fs::create_dir_all(parent).map_err(|e| format!("response parent s'u krijua: {e}"))?;
    let tmp = temporary_path(path);
    std::fs::write(&tmp, bytes).map_err(|e| format!("response temp s'u shkrua: {e}"))?;
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(format!("response nuk u publikua atomikisht: {e}"))
        }
    }
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(format!(".{}.tmp", std::process::id()));
    PathBuf::from(tmp)
}

#[cfg(test)]
mod action_evidence_tests {
    use super::*;

    fn entry(stage: u8, len: usize) -> ActionEvidenceWire {
        ActionEvidenceWire {
            stage,
            evidence_words: vec![1; len],
        }
    }

    fn valid_entries() -> Vec<ActionEvidenceWire> {
        vec![
            entry(shadow_contracts::ACTION_STAGE_HPRO, 5),
            ActionEvidenceWire {
                stage: shadow_contracts::ACTION_STAGE_PRO,
                evidence_words: vec![2, 9000, 1000, 2, 1, 40],
            },
            entry(shadow_contracts::ACTION_STAGE_NPRO, 4),
            entry(shadow_contracts::ACTION_STAGE_NPIM, 5),
            entry(shadow_contracts::ACTION_STAGE_SRK, 6),
            entry(shadow_contracts::ACTION_STAGE_PIM, 6),
            entry(shadow_contracts::ACTION_STAGE_APRO, 4),
            entry(shadow_contracts::ACTION_STAGE_MPRO, 21),
            entry(shadow_contracts::ACTION_STAGE_HCP, 7),
        ]
    }

    fn replace_stage(entries: &mut [ActionEvidenceWire], stage: u8, len: usize) {
        match entries.iter_mut().find(|entry| entry.stage == stage) {
            Some(entry) => entry.evidence_words = vec![1; len],
            None => panic!("fixture stage mungon"),
        }
    }

    #[test]
    fn candidate_ledger_binds_each_positive_not_only_the_mean() {
        use shadow_contracts::candidate_measurement::{CandidateLedger, CandidateObservation, CandidatePolarity};
        let observations = vec![
            CandidateObservation { polarity: CandidatePolarity::Positive, operator: 1,
                fragment_refs: vec![0], value: 9000 },
            CandidateObservation { polarity: CandidatePolarity::Positive, operator: 2,
                fragment_refs: vec![1], value: 0 },
        ];
        let ledger = CandidateLedger::new([1;32], observations.clone()).unwrap();
        let pro = ProActionEvidence::try_from_words(&[2,9000,0,2,2,80]).unwrap();
        assert!(candidate_ledger_matches_pro(&ledger, &pro, &[0.9,0.0], &[1,1,0,0]));
        let mut changed = observations.clone();
        changed[0].value = 8000;
        changed[1].value = 1000; // same sum/mean, different evidence
        assert!(!candidate_ledger_matches_pro(&CandidateLedger::new([1;32], changed).unwrap(),
            &pro, &[0.9,0.0], &[1,1,0,0]));
        let mut changed = observations;
        changed[1].fragment_refs = vec![2]; // outside declared admitted prefix
        assert!(!candidate_ledger_matches_pro(&CandidateLedger::new([1;32], changed).unwrap(),
            &pro, &[0.9,0.0], &[1,1,0,0]));
        assert!(!candidate_ledger_matches_pro(&ledger, &pro, &[0.9], &[1,1,0,0]));
        assert!(!candidate_ledger_matches_pro(&ledger, &pro, &[0.9,0.0], &[1,0,0,0]));
    }

    #[test]
    fn candidate_action_has_explicit_bounds_without_changing_other_stage_limits() {
        use shadow_contracts::candidate_measurement::{CandidateLedger, CandidateObservation, CandidatePolarity};
        let ledger = CandidateLedger::new([1;32], vec![
            CandidateObservation { polarity: CandidatePolarity::Positive, operator: 1,
                fragment_refs: (0..80).collect(), value: 7000 },
        ]).unwrap();
        assert!(ledger.to_words().len() > 64);
        let mut entries = valid_entries();
        let index = entries.iter().position(|a| a.stage == shadow_contracts::ACTION_STAGE_MPRO).unwrap();
        entries[index].evidence_words = ledger.to_words();
        assert!(ActionEvidenceView::parse(&entries, 16).is_ok());
        entries[index].evidence_words.truncate(21);
        assert!(ActionEvidenceView::parse(&entries, 16).is_err());
        entries[index].evidence_words = ledger.to_words();
        entries[0].evidence_words = ledger.to_words();
        assert!(ActionEvidenceView::parse(&entries, 16).is_err());
    }

    #[test]
    fn removed_candidate_marker_at_legacy_length_still_fails_actual_word_binding() {
        use shadow_contracts::candidate_measurement::{CandidateLedger, CandidateObservation, CandidatePolarity};
        // A legitimate legacy action remains accepted by the exact same helper
        // called at the live Shadow boundary. The local data is harmless.
        let raw = [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        let scalars = [1, 16, 625, 10_000, 625];
        let mut legacy: Vec<u64> = raw.iter().map(|value| u64::from(*value)).collect();
        legacy.extend(scalars.into_iter().map(u64::from));
        assert!(legacy_mpro_matches(&raw, scalars, &legacy));

        let ledger = CandidateLedger::new([0x5a; 32], vec![
            CandidateObservation { polarity: CandidatePolarity::Positive, operator: 1,
                fragment_refs: vec![0], value: 0 },
            CandidateObservation { polarity: CandidatePolarity::Positive, operator: 2,
                fragment_refs: vec![1], value: 625 },
            CandidateObservation { polarity: CandidatePolarity::NegativeCondition, operator: 1,
                fragment_refs: vec![], value: 0 },
        ]).unwrap();
        let mut malformed = ledger.to_words();
        assert_eq!(malformed.len(), 21);
        malformed[0] = 0; // marker is absent, so legacy shape alone can match
        assert!(!CandidateLedger::is_current_words(&malformed));
        // Even plausible legacy scalar words cannot substitute for raw evidence.
        malformed[16..].copy_from_slice(&scalars.map(u64::from));
        let mut entries = valid_entries();
        let entry = entries.iter_mut().find(|entry| entry.stage == shadow_contracts::ACTION_STAGE_MPRO).unwrap();
        entry.evidence_words = malformed.clone();
        let parsed = ActionEvidenceView::parse(&entries, 16).unwrap();
        assert!(!legacy_mpro_matches(&raw, scalars, parsed.mpro));
        // Each scalar and each raw observation remains covered independently
        // of any separately recomputed package/action digest.
        for position in 0..legacy.len() {
            let mut changed = legacy.clone(); changed[position] += 1;
            assert!(!legacy_mpro_matches(&raw, scalars, &changed));
        }
        assert!(!legacy_mpro_matches(&raw, scalars, &legacy[..20]));
        let mut trailing = legacy; trailing.push(0);
        assert!(!legacy_mpro_matches(&raw, scalars, &trailing));
    }


    #[test]
    fn pro_requires_six_words_and_shared_semantic_bounds() {
        for words in [
            vec![],
            vec![2, 9000, 1000],
            vec![6, 9000, 1000, 6, 1, 40],
            vec![2, 9000, 1000, 1, 1, 40],
            vec![2, 10001, 1000, 2, 1, 40],
            vec![1, 8000, 7000, 1, 1, 40],
            vec![0, 1, 0, 0, 1, 40],
            vec![2, 1000, 9000, 2, 1, 40],
        ] {
            let mut entries = valid_entries();
            entries[1].evidence_words = words;
            assert!(ActionEvidenceView::parse(&entries, 16).is_err());
        }
        let entries = valid_entries();
        let parsed = ActionEvidenceView::parse(&entries, 16).expect("valid six-word PRO");
        assert_eq!(parsed.pro.to_words(), [2, 9000, 1000, 2, 1, 40]);
        assert!(pro_matches_candidates(&parsed.pro, &[0.9, 0.1]));
    }

    #[test]
    fn pro_binds_all_three_candidate_fields_independently() {
        for words in [
            [3, 9000, 1000, 3, 1, 40],
            [2, 8000, 1000, 2, 1, 40],
            [2, 9000, 2000, 2, 1, 40],
        ] {
            let pro = ProActionEvidence::try_from_words(&words).expect("valid shape, different candidate prefix");
            assert!(!pro_matches_candidates(&pro, &[0.9, 0.1]));
        }
        let empty = ProActionEvidence::try_from_words(&[0, 0, 0, 0, 1, 40]).expect("early-refusal fixture");
        assert!(pro_matches_candidates(&empty, &[]));
        let single = ProActionEvidence::try_from_words(&[1, 8000, 8000, 1, 1, 40]).expect("single fixture");
        assert!(pro_matches_candidates(&single, &[0.8]));
    }

    #[test]
    fn pro_candidate_binding_does_not_attest_resource_observations() {
        for words in [
            [2, 9000, 1000, 3, 1, 40],
            [2, 9000, 1000, 2, 0, 40],
            [2, 9000, 1000, 2, 1, 0],
        ] {
            let pro = ProActionEvidence::try_from_words(&words).expect("admissible resource declaration");
            assert!(pro_matches_candidates(&pro, &[0.9, 0.1]));
        }
    }

    #[test]
    fn pro_candidate_binding_rejects_nonfinite_clamped_or_unsorted_scores() {
        let pro = ProActionEvidence::try_from_words(&[3, 9000, 1000, 3, 1, 40]).expect("three-candidate fixture");
        assert!(pro_matches_candidates(&pro, &[0.9, 0.5, 0.1]));
        for scores in [
            [0.9, f32::NAN, 0.1],
            [0.9, f32::INFINITY, 0.1],
            [0.9, -0.1, 0.1],
            [0.9, 1.1, 0.1],
            [0.9, 0.95, 0.1],
            [0.9, 0.05, 0.1],
        ] {
            assert!(!pro_matches_candidates(&pro, &scores));
        }
        let clamped = ProActionEvidence::try_from_words(&[2, 10000, 0, 2, 1, 40]).expect("bounded wire fixture");
        assert!(!pro_matches_candidates(&clamped, &[1.1, -0.1]));
    }

    #[test]
    fn hcp_accepts_legacy_or_explicit_family_mask_without_hidden_extra_words() {
        for words in [
            vec![1,0,3,0,1,5,0], vec![1,0,3,0,1,5,0,0], vec![1,0,3,0,1,5,2,6],
        ] {
            let mut entries = valid_entries();
            entries.last_mut().unwrap().evidence_words = words;
            assert!(ActionEvidenceView::parse(&entries, 16).is_ok());
        }
        for words in [
            vec![1,0,3,0,1,5], vec![1,0,3,0,1,5,2,0],
            vec![1,0,3,0,1,5,1,16], vec![1,0,3,0,1,5,0,0,0],
        ] {
            let mut entries = valid_entries();
            entries.last_mut().unwrap().evidence_words = words;
            assert!(ActionEvidenceView::parse(&entries, 16).is_err());
        }
    }

    #[test]
    fn hpro_requires_exact_five_words_without_panic() {
        for len in [0, 1, 2, 3, 4, 6] {
            let mut entries = valid_entries();
            replace_stage(&mut entries, shadow_contracts::ACTION_STAGE_HPRO, len);
            assert!(ActionEvidenceView::parse(&entries, 16).is_err());
        }
        assert!(ActionEvidenceView::parse(&valid_entries(), 16).is_ok());
    }

    #[test]
    fn npro_requires_exact_four_words_without_panic() {
        for len in [0, 1, 2, 3, 5, 6] {
            let mut entries = valid_entries();
            replace_stage(&mut entries, shadow_contracts::ACTION_STAGE_NPRO, len);
            assert!(ActionEvidenceView::parse(&entries, 16).is_err());
        }
    }

    #[test]
    fn srk_requires_exact_six_words_without_panic() {
        for len in [0, 1, 2, 3, 4, 5, 7, 8] {
            let mut entries = valid_entries();
            replace_stage(&mut entries, shadow_contracts::ACTION_STAGE_SRK, len);
            assert!(ActionEvidenceView::parse(&entries, 16).is_err());
        }
    }

    #[test]
    fn duplicate_and_unknown_stages_are_rejected() {
        let mut duplicate = valid_entries();
        duplicate[8] = entry(shadow_contracts::ACTION_STAGE_HPRO, 5);
        assert!(ActionEvidenceView::parse(&duplicate, 16).is_err());

        let mut unknown = valid_entries();
        unknown[8] = entry(u8::MAX, 7);
        assert!(ActionEvidenceView::parse(&unknown, 16).is_err());
    }

    #[test]
    fn entry_order_does_not_change_stage_resolution() {
        let mut entries = valid_entries();
        entries.swap(0, 8);
        let parsed = ActionEvidenceView::parse(&entries, 16);
        match parsed {
            Ok(view) => {
                assert_eq!(view.hpro.len(), 5);
                assert_eq!(view.hcp.len(), 7);
            }
            Err(error) => panic!("reordered exact evidence u refuzua: {error}"),
        }
    }

    #[test]
    fn malformed_wire_returns_error_instead_of_panicking() {
        let base = std::env::temp_dir().join(format!(
            "ess_mai_shadow_malformed_wire_{}",
            std::process::id(),
        ));
        let request = base.with_extension("request");
        let response = base.with_extension("response");
        let write_result = std::fs::write(&request, b"malformed-wire");
        assert!(write_result.is_ok());
        let result = std::panic::catch_unwind(|| run_cycle(&request, &response));
        match result {
            Ok(Err(_)) => {}
            Ok(Ok(())) => panic!("malformed wire u pranua"),
            Err(_) => panic!("malformed wire shkaktoi panic"),
        }
        let _ = std::fs::remove_file(request);
        let _ = std::fs::remove_file(response);
    }
}

#[cfg(test)]
mod a1_production_call_graph_tests {
    #[test]
    fn production_run_cycle_has_zero_post_commit_negative_writer() {
        let source = include_str!("process_bridge.rs").replace("\r\n", "\n");
        let run_cycle = source
            .split("fn run_cycle_internal(")
            .nth(1)
            .expect("run_cycle source")
            .split("#[cfg(feature = \"dev_harness\")]\nfn run_negative")
            .next()
            .expect("run_cycle boundary");
        assert!(!run_cycle.contains("persist_negative("));
        assert!(run_cycle.contains("crate::native_cycle::response_wire(response)"));
    }

    #[test]
    fn compatibility_negative_writer_is_not_default_production_surface() {
        let source = include_str!("process_bridge.rs").replace("\r\n", "\n");
        let marker = "#[cfg(feature = \"dev_harness\")]\nfn persist_negative";
        assert!(source.contains(marker));
    }
}

// derivative: operational memory only. No verification receipt is issued.
fn run_pd_operational(kind: &str, request_path: &Path, response_path: &Path, owner: Option<&ResidentOwner>) -> Result<(), String> {
    use shadow_contracts::pd_memory as pd;
    // Historical helper is retained but is NOT treated as caller authentication.
    require_main_mediation("pd-operational")?;
    if response_path.exists() { return Err("PD_RESPONSE_EXISTS".into()); }
    let mut file=std::fs::File::open(request_path).map_err(|e|format!("PD_REQUEST_OPEN:{e}"))?;
    let mut bytes=Vec::new();
    (&mut file).take((pd::MAX_REQUEST_BYTES+257) as u64).read_to_end(&mut bytes)
        .map_err(|e|format!("PD_REQUEST_READ:{e}"))?;
    if bytes.len()>pd::MAX_REQUEST_BYTES+256{return Err("PD_REQUEST_LIMIT".into());}
    // This opens the SAME owner WAL, not a second PD store or direct core link.
    // The new FILE3 profile is explicit; legacy WALs are rejected without migration.
    let payload=match kind {
        "lgc-read"=>{
            let reference=pd::lgc::decode_ref(&bytes)?;
            let vault=request_vault(owner)?;
            let record=vault.pd_lgc_record(&reference).map_err(|e|format!("{e:?}"))?;
            pd::encode_optional_record(record.as_ref())?
        },
        "iz-native-map"=>{
            let context=pd::iz::decode_context(&bytes)?;
            let vault=request_vault(owner)?;
            let snapshot=vault.pd_iz_native_map(&context).map_err(|e|format!("{e:?}"))?;
            pd::iz::encode_native_map_snapshot(&snapshot)?
        },
        "iz-native-read"=>{
            let context=pd::iz::decode_context(&bytes)?;
            let vault=request_vault(owner)?;
            let bundle=vault.pd_iz_native_read(&context).map_err(|e|format!("{e:?}"))?;
            shadow_contracts::pd_recovery::encode_bundle(&bundle)?
        },
        "iz-native-lookup"=>{
            let context=pd::iz::decode_context(&bytes)?;
            let vault=request_vault(owner)?;
            let lookup=vault.pd_iz_native_lookup(&context).map_err(|e|format!("{e:?}"))?;
            pd::lgc::encode_optional_native_lookup(lookup.as_ref())?
        },
        "iz-map" | "iz-read" => {
            let context=pd::iz::decode_context(&bytes)?;
            let vault=request_vault(owner)?;
            if context.version==pd::iz::IZ_NATIVE_CONTINUITY_VERSION {
                if kind!="iz-read" {return Err("IZ_NATIVE_REQUIRES_VERSIONED_MAP_ENDPOINT".into());}
                let head=vault.pd_iz_native_operational_read(&context).map_err(|e|format!("{e:?}"))?;
                pd::encode_optional_record(head.as_ref())?
            } else {
                let head=vault.pd_head(&context.identity).map_err(|e|format!("{e:?}"))?;
                let map=crate::pd_iz_admission::map_snapshot(&context,head.as_ref())?;
                if kind=="iz-map" {
                    pd::iz::encode_map_snapshot(map.as_ref())?
                } else {
                    if context.map != map {return Err("IZ_MAP_REFERENCE_MISMATCH".into());}
                    pd::encode_optional_record(head.as_ref())?
                }
            }
        },
        "history"=>{
            let query=pd::history::decode_history_lookup(&bytes)?;
            let vault=request_vault(owner)?;
            let page=vault.pd_history(&query).map_err(|e|format!("{e:?}"))?;
            pd::history::encode_history_page(&page)?
        },
        "head"=>{
            let query=pd::decode_lookup(&bytes)?;
            let vault=request_vault(owner)?;
            let record=match query.operation_id {
                None=>vault.pd_head(&query.identity),
                Some(op)=>vault.pd_operation(&query.identity,&op),
            }.map_err(|e|format!("{e:?}"))?;
            pd::encode_optional_record(record.as_ref())?
        },
        "candidate"=>{
            let query=pd::decode_candidate_query(&bytes)?;
            let vault=request_vault(owner)?;
            let record=vault.pd_candidate(&query.identity,&query.query_digest).map_err(|e|format!("{e:?}"))?;
            pd::encode_optional_record(record.as_ref())?
        },
        "commit"=>{
            let request=pd::decode_request(&bytes)?;
            validate_pd_candidate(&request)?;
            let vault=request_vault(owner)?;
            crate::pd_failure_admission::validate_native_admission(&request, &vault, selection_handoff_root, selection_now_ns)?;
            let record=vault.commit_pd_operation(request).map_err(|e|format!("{e:?}"))?;
            pd::encode_record(&record)?
        },
        _=>return Err("PD_OPERATION_UNKNOWN".into()),
    };
    // Sync precedes publication. Response failure never rolls back a durable WAL.
    publish_pd_response(response_path,&payload)?;
    eprintln!("[SHADOW/PD] kind={kind} result=OperationalOnly verdict=None knowledge_write=0");
    Ok(())
}
fn validate_pd_candidate(request:&shadow_contracts::pd_memory::PdOperationRequest)->Result<(),String>{
    if shadow_contracts::pd_memory::late_refusal::source_is_late(&request.source_input) {
        return Err("PD_LATE_REFUSAL_SHADOW_OWNER_REQUIRED".into());
    }
    if shadow_contracts::pd_memory::iz_science::source_is_science(&request.source_input) {
        request.validate()?;
        shadow_contracts::pd_memory::iz_science::validate_operation(request)?;
        validate_failure_checkpoint_at_admission(request)?;
        return validate_linked_scientific_candidate(request);
    }
    if shadow_contracts::pd_memory::iz::source_is_iz(&request.source_input) {
        request.validate()?;
        let source=shadow_contracts::pd_memory::iz::decode_source(&request.source_input)?;
        let raw=crate::pd_iz_admission::checkpoint_raw(&source.context,&request.checkpoint)?;
        return shadow_contracts::pd_checkpoint_data::validate_checkpoint(raw,
            &shadow_contracts::pd_memory::hex(&request.identity.key()?));
    }
    if request.source_input.starts_with(b"PDF1") || request.source_input.starts_with(b"PDS1") {
        shadow_contracts::pd_memory::failure::validate_failure_operation(request)?;
        validate_failure_checkpoint_at_admission(request)?;
        if request.source_input.starts_with(b"PDS1") {
            validate_linked_scientific_candidate(request)?;
        }
        return Ok(());
    }

    use shadow_contracts::pd_memory as pd;
    request.validate()?;
    let input=pd::decode_continuity_input(&request.source_input)?;
    let query=science_domains::decode_query(&input.science_query).map_err(|e|format!("PD_INPUT_SCIENCE:{e}"))?;
    let domain=match query.adapter {
        science_domains::AdapterId::Astronomy=>"astronomy",science_domains::AdapterId::Physics=>"physics",
        science_domains::AdapterId::History=>"history",science_domains::AdapterId::SocialScience=>"social-science",
        science_domains::AdapterId::Psychology=>"psychology",
    };
    if query.session_id!=pd::hex(&request.identity.key()?)||query.case_id!=request.identity.case_id
        ||query.scope_id!=request.identity.scope||query.obligation_id!=request.identity.obligation
        ||domain!=request.identity.domain{return Err("PD_SOURCE_QUERY_SCOPE_MISMATCH".into());}
    if request.candidate.is_empty(){
        return match science_domains::evaluate(&query) {
            Err(science_domains::ScienceError::Unavailable{..})=>Ok(()),
            Err(e)=>Err(format!("PD_INPUT_SCIENCE:{e}")),
            Ok(_)=>Err("PD_COMPUTABLE_CANDIDATE_OMITTED".into()),
        };
    }
    let candidate=science_domains::decode_candidate(&request.candidate).map_err(|e|format!("PD_CANDIDATE:{e}"))?;
    let q=&candidate.query;
    if candidate.query!=query || candidate.query_sha256!=request.candidate_key
        || q.session_id!=pd::hex(&request.identity.key()?)
        || q.case_id!=request.identity.case_id || q.scope_id!=request.identity.scope
        || q.obligation_id!=request.identity.obligation
        || request.identity.domain!="astronomy" {
        return Err("PD_CANDIDATE_SCOPE_MISMATCH".into());
    }
    // decode_candidate re-evaluates exact pinned model. This checks integrity,
    // never independent observation, caller identity, or epistemic acceptance.
    Ok(())
}
fn validate_linked_scientific_candidate(request:&shadow_contracts::pd_memory::PdOperationRequest)->Result<(),String>{
    use shadow_contracts::pd_memory as pd;
    let query_bytes=pd::linked_science::science_query_from_source(&request.source_input)?;
    let query=science_domains::decode_query(&query_bytes).map_err(|e|format!("PD_LINKED_QUERY:{e}"))?;
    if query.session_id!=pd::hex(&request.identity.key()?) || query.case_id!=request.identity.case_id
        || query.scope_id!=request.identity.scope || query.obligation_id!=request.identity.obligation {
        return Err("PD_LINKED_QUERY_SCOPE".into());
    }
    let actual=science_domains::evaluate(&query).map_err(|e|format!("PD_LINKED_PROVIDER:{e}"))?;
    let encoded=science_domains::encode_candidate(&actual).map_err(|e|format!("PD_LINKED_CANDIDATE:{e}"))?;
    if encoded!=request.candidate || actual.query_sha256!=request.candidate_key {
        return Err("PD_LINKED_PROVIDER_RESULT_MISMATCH".into());
    }
    // This is MODEL_COMPUTED material, not observed physical evidence or verdict.
    Ok(())
}
// NEW native admission guard: the policy marker is not a valid cognitive snapshot.
// This verifies historical data and the exact scoped session; it grants no verdict,
// scientific candidate, caller authentication or active PD capability.
fn validate_failure_checkpoint_at_admission(
    request: &shadow_contracts::pd_memory::PdOperationRequest,
) -> Result<(), String> {
    use shadow_contracts::pd_memory as pd;
    let prefix = if pd::iz_science::source_is_science(&request.source_input) {
        pd::iz_science::decode_input(&request.source_input)?.checkpoint_prefix()
    } else { pd::failure::FAILURE_CHECKPOINT_PREFIX };
    let raw = request.checkpoint.strip_prefix(prefix)
        .ok_or("PD_FAILURE_CHECKPOINT_POLICY_REVIEW_REQUIRED")?;
    let session = pd::hex(&request.identity.key()?);
    shadow_contracts::pd_checkpoint_data::validate_checkpoint(raw, &session)
        .map_err(|error| format!("PD_FAILURE_CHECKPOINT_INVALID:{error}"))
}

fn publish_pd_response(path:&Path,bytes:&[u8])->Result<(),String>{
    let parent=path.parent().ok_or("PD_RESPONSE_PARENT")?;
    if !parent.is_dir(){return Err("PD_RESPONSE_PARENT_MISSING".into());}
    let tmp=temporary_path(path);
    let mut f=OpenOptions::new().write(true).create_new(true).open(&tmp).map_err(|e|format!("PD_RESPONSE_TEMP:{e}"))?;
    f.write_all(bytes).and_then(|_|f.sync_all()).map_err(|e|format!("PD_RESPONSE_SYNC:{e}"))?;
    drop(f);
    // Hard-link publication is no-clobber even if another writer races this path.
    std::fs::hard_link(&tmp,path).map_err(|e|format!("PD_RESPONSE_PUBLISH:{e}"))?;
    std::fs::remove_file(&tmp).map_err(|e|format!("PD_RESPONSE_TEMP_CLEANUP:{e}"))?;
    Ok(())
}

#[cfg(test)]
mod pd_operational_admission_tests {
    use super::*;
    use shadow_contracts::pd_memory as pd;
    use science_domains::{ScienceQuery,ScienceTask,TimeScale,AdapterId};
    fn bind(request:&mut pd::PdOperationRequest,query:&ScienceQuery){
        request.source_input=pd::encode_continuity_input(&pd::ContinuityInput{
            identity:request.identity.clone(),operation_id:request.operation_id.clone(),
            user_text:"Explicit scoped-admission regression fixture".into(),
            science_query:science_domains::encode_query(query).expect("fixture query"),
        }).expect("fixture input");
        request.witness.input_sha256=Sha256::digest(&request.source_input).into();
        request.lgc_steps=vec![pd::LgcStep::new(0,request.witness.input_sha256,
            "admission fixture","bounded scoped test",request.witness.input_sha256).expect("fixture step")];
        request.witness.lgc_parent=request.lgc_steps[0].step_digest;
    }
    fn fixture()->(pd::PdOperationRequest,ScienceQuery){
        let mut request=crate::pd_memory_store::tests::request("admission-fixture");
        let query=ScienceQuery{
            adapter:AdapterId::Astronomy,session_id:pd::hex(&request.identity.key().expect("key")),
            case_id:request.identity.case_id.clone(),scope_id:request.identity.scope.clone(),
            obligation_id:request.identity.obligation.clone(),query_id:"conformance-case".into(),
            input_source_id:"IAU conformance fixture; not an observation".into(),
            task:ScienceTask::EarthRotationAngle{jd_part1:2400000.5,jd_part2:54388.0,time_scale:TimeScale::Ut1,
                claim_radians:0.4022837240028158,tolerance_radians:1e-12},
        };
        bind(&mut request,&query);
        replace_candidate(&mut request,&query);
        (request,query)
    }
    fn replace_candidate(request:&mut pd::PdOperationRequest,query:&ScienceQuery){
        let candidate=science_domains::evaluate(query).expect("actual model computation");
        request.candidate_key=candidate.query_sha256;
        request.candidate=science_domains::encode_candidate(&candidate).expect("candidate codec");
    }
    #[test]fn exact_source_candidate_is_operational_only(){
        let (request,_)=fixture();
        assert!(validate_pd_candidate(&request).is_ok());
        assert_eq!(science_domains::decode_candidate(&request.candidate).unwrap().authority,science_domains::CandidateAuthority::None);
    }
    #[test]fn same_scope_different_claim_cannot_replace_retained_input(){
        let (mut request,mut other)=fixture();
        let ScienceTask::EarthRotationAngle{ref mut claim_radians,..}=other.task;
        *claim_radians+=0.5;
        replace_candidate(&mut request,&other);
        assert!(request.validate().is_ok()); // shape alone is insufficient
        assert_eq!(validate_pd_candidate(&request).unwrap_err(),"PD_CANDIDATE_SCOPE_MISMATCH");
    }
    #[test]fn foreign_scope_and_missing_computable_candidate_are_refused(){
        let (mut request,mut foreign)=fixture();
        foreign.scope_id="unrelated-topic".into();
        replace_candidate(&mut request,&foreign);
        assert!(validate_pd_candidate(&request).is_err());
        request.candidate.clear();request.candidate_key=[0;32];
        assert_eq!(validate_pd_candidate(&request).unwrap_err(),"PD_COMPUTABLE_CANDIDATE_OMITTED");
    }
    #[test]fn unavailable_provider_retains_no_candidate_and_no_knowledge(){
        let (mut request,mut query)=fixture();
        request.identity.domain="history".into();query.adapter=AdapterId::History;
        query.session_id=pd::hex(&request.identity.key().unwrap());
        request.candidate.clear();request.candidate_key=[0;32];
        bind(&mut request,&query);
        assert!(validate_pd_candidate(&request).is_ok());
    }
}

// T2 accepts only an exact lookup. Its receipt, pending state and output come from this Vault.
fn run_native_finalize(request_path:&Path,response_path:&Path, owner: Option<&ResidentOwner>)->Result<(),String>{
    require_main_mediation("pd-finalize-once")?;
    let bytes=read_bounded_request(request_path,"T2 lookup")?;
    let (identity,request_sha256)=shadow_contracts::pd_cycle::decode_finalize_lookup(&bytes)?;
    let vault=request_vault(owner)?;
    let record=vault.finalize_native_continuation(identity,request_sha256).map_err(|e|e.to_string())?;
    // Same publication discipline as the operational projection: sync and no clobber.
    publish_pd_response(response_path,&record.material)
}


// Exercises the reader with a small caller-defined bound; production passes the
// actual wire or PDT1 bounds. Fixtures do not bypass constitutional admission.
#[cfg(test)]
mod bounded_request_tests {
    use super::*;
    #[test]
    fn reader_enforces_the_selected_limit_including_exact_boundary() {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "essmai_request_limit_{}_{}.bin", std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        ));
        std::fs::write(&path, [0x51; 33]).expect("isolated reader fixture");
        assert!(read_bounded_request_with_limit(&path, "fixture", 32).is_err());
        assert_eq!(read_bounded_request_with_limit(&path, "fixture", 33).unwrap(), vec![0x51; 33]);
        assert_eq!(read_bounded_request_with_limit(&path, "fixture", 44).unwrap(), vec![0x51; 33]);
        assert!(shadow_contracts::pd_cycle::MAX_CYCLE_BYTES > shadow_contracts::MAX_FRAME_BYTES);
        std::fs::remove_file(path).expect("remove isolated reader fixture");
    }
}


// Recovery reads an explicit original identity and source request hash. It never
// reruns scientific admission or borrows the most recently used conversation.
fn run_native_recover(request_path:&Path,response_path:&Path, owner: Option<&ResidentOwner>)->Result<(),String>{
    require_main_mediation("pd-recover-once")?;
    let bytes=read_bounded_request_with_limit(request_path,"recovery lookup",68)?;
    let lookup=shadow_contracts::pd_recovery::decode_lookup(&bytes)?;
    let vault=request_vault(owner)?;
    let bundle=vault.recover_native(&lookup).map_err(|e|e.to_string())?;
    publish_pd_response(response_path,&shadow_contracts::pd_recovery::encode_bundle(&bundle)?)
}

#[cfg(test)]
mod pd_failure_checkpoint_admission_tests {
    use super::*;
    use shadow_contracts::pd_memory::{failure::*, *};
    use shadow_contracts::*;
    // Explicit data-only selection fixture, not a real constitutional outcome.
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

    fn request_for(input: &FailureDialogueInput) -> PdOperationRequest {
        let mut request = crate::pd_memory_store::tests::request(&input.operation_id);
        request.identity = input.identity.clone();
        request.source_input = encode_failure_input(input).unwrap();
        request.witness.input_sha256 = Sha256::digest(&request.source_input).into();
        request.witness.reason = input.refusal.clone();
        request.witness.condition_code = "MISSING_EXACT_POSITIVE".into();
        request.witness.attempt_trace = input.operation_id.clone();
        let step = LgcStep::new(
            0,
            request.witness.input_sha256,
            "D-BAB",
            "fixture observed missing pair",
            [9; 32],
        )
        .unwrap();
        request.witness.lgc_parent = step.step_digest;
        request.lgc_steps = vec![step];
        request.checkpoint = [FAILURE_CHECKPOINT_PREFIX, b"fixture opaque checkpoint"].concat();
        validate_failure_operation(&request).unwrap();
        request
    }

    #[test]
    fn prefix_does_not_admit_empty_or_malformed_cognitive_payload() {
        let mut request = request_for(&fixture());
        for raw in [b"".as_slice(), b"not a cognitive checkpoint".as_slice(), &[0; 40]] {
            request.checkpoint = [FAILURE_CHECKPOINT_PREFIX, raw].concat();
            assert!(validate_failure_checkpoint_at_admission(&request)
                .unwrap_err().starts_with("PD_FAILURE_CHECKPOINT_INVALID:"));
        }
        request.checkpoint = b"PD_USER_ONLY_V2\0legacy profile is not converted".to_vec();
        assert_eq!(validate_failure_checkpoint_at_admission(&request).unwrap_err(),
            "PD_FAILURE_CHECKPOINT_POLICY_REVIEW_REQUIRED");
    }

    #[test]
    fn native_failure_admission_keeps_missing_condition_without_scientific_claim() {
        let mut request = request_for(&fixture());
        validate_failure_operation(&request).unwrap();
        assert_eq!(request.witness.report_class, ReportClass::MissingConditions);
        assert_eq!(request.witness.permitted_next, NextAction::AcquireScopedEvidence);
        assert!(request.candidate.is_empty());
        assert_eq!(request.candidate_key, [0; 32]);
        // The old structural contract permits an opaque fixture, but new native
        // admission rejects it. No storage-only or historical fixture is migrated.
        assert!(validate_pd_candidate(&request).unwrap_err()
            .starts_with("PD_FAILURE_CHECKPOINT_INVALID:"));
        request.candidate = b"claimed scientific result".to_vec();
        request.candidate_key = [7; 32];
        assert_eq!(validate_pd_candidate(&request).unwrap_err(), "PD_FAILURE_STORAGE_POLICY");
    }
}

#[cfg(test)]
mod resident_owner_tests {
    use super::*;
    #[test]
    fn r37_resident_handler_reuses_exclusive_owner_and_matches_reopened_projection() {
        use shadow_contracts::pd_memory as pd;
        let root=std::env::temp_dir().join(format!("r37-owner-{}-{}",std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&root).unwrap();let path=root.join("vault.svlt");let lock=root.join("owner.lock");
        std::fs::write(&lock,b"isolated owner fixture").unwrap();
        let vault=crate::knowledge_vault::KnowledgeVault::open_disk(&path).unwrap();
        // Existing operational-storage fixture; this test does not assert a new
        // constitutional native outcome or bypass the production commit handler.
        let request=crate::pd_memory_store::tests::request("r37-owner");
        let record=vault.commit_pd_operation(request).unwrap();
        let identity=record.request.identity.clone();
        let expected=pd::encode_optional_record(Some(&record)).unwrap();
        let owner=ResidentOwner {vault:std::sync::Arc::new(vault),_apupk:ApupkProcessLock {path:lock}};
        assert!(crate::knowledge_vault::KnowledgeVault::open_disk(&path).is_err());
        let source=root.join("request.bin");
        std::fs::write(&source,pd::encode_lookup(&pd::PdLookup {identity:identity.clone(),operation_id:None}).unwrap()).unwrap();
        let before=std::fs::metadata(&path).unwrap().len();
        for n in 0..2 {
            let response=root.join(format!("response-{n}.bin"));
            dispatch_request("--pd-head-once",&source,&response,Some(&owner)).unwrap();
            assert_eq!(std::fs::read(&response).unwrap(),expected);
            assert_eq!(std::fs::metadata(&path).unwrap().len(),before);
            std::fs::remove_file(response).unwrap();
        }
        drop(owner);let before=std::fs::read(&path).unwrap();
        let reopened=crate::knowledge_vault::KnowledgeVault::open_disk(&path).unwrap();
        assert_eq!(pd::encode_optional_record(reopened.pd_head(&identity).unwrap().as_ref()).unwrap(),expected);
        drop(reopened);assert_eq!(std::fs::read(&path).unwrap(),before);
        std::fs::remove_file(source).unwrap();std::fs::remove_file(path).unwrap();std::fs::remove_dir(root).unwrap();
    }
}
