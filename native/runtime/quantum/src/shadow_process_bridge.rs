//! v1.6.4 — Quantum client for the mandatory Shadow executable boundary.
//!
//! Quantum links only `shadow_contracts`. It cannot construct `Shadow`, call
//! `ingest_bridged`, open the vault, issue tokens, or seal receipts. Every real
//! verification and negative-knowledge write is mediated by `shadow_platform`
//! main.rs through a checksummed file contract.

use shadow_contracts::{KnowledgeSelectionRequestWire, KnowledgeSelectionResponseWire};
use shadow_contracts::{ShadowCycleRequest, ShadowCycleResponse};
#[cfg(feature = "dev_harness")]
use shadow_contracts::{ShadowNegativeRequest, ShadowNegativeResponse};
use std::path::{Path, PathBuf};

pub fn execute_selection(
    request: &KnowledgeSelectionRequestWire,
) -> Result<KnowledgeSelectionResponseWire, String> {
    match (request.requester, request.shape_is_valid()) {
        (shadow_contracts::SelectionRequester::AshtQuantum, true) => {}
        (shadow_contracts::SelectionRequester::BesaNlight, _) => {
            return Err("Quantum cannot issue a Besa selection request".to_string())
        }
        (_, false) => return Err("Quantum selection request shape invalid".to_string()),
    }
    let identity = format!("asht-{}", request.session_id);
    let bytes = shadow_contracts::encode_selection_request(request);
    let response = invoke("--selection-once", &identity, &bytes)?;
    let decoded = shadow_contracts::decode_selection_response(&response)
        .map_err(|error| format!("Shadow selection response rejected: {error}"))?;
    match decoded.shape_is_valid_for(request) {
        true => Ok(decoded),
        false => Err("Shadow selection response was not bound to Asht".to_string()),
    }
}

#[cfg(feature = "dev_harness")]
pub fn persist_negative(request: ShadowNegativeRequest) -> Result<ShadowNegativeResponse, String> {
    let identity = format!("negative-{:016x}", request.negative.input_id);
    let bytes = shadow_contracts::encode_negative_request(&request);
    let response = invoke("--negative-once", &identity, &bytes)?;
    shadow_contracts::decode_negative_response(&response)
        .map_err(|e| format!("Shadow negative response u refuzua: {e}"))
}

pub fn now_ns() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

fn invoke(mode: &str, _identity: &str, request_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let _shadow=resolve_shadow_binary()?;
    let _handoff=handoff_dir()?;
    quantum_lim::pd_operational::native_transport(mode,request_bytes,std::time::Duration::from_secs(90))
}

fn handoff_dir() -> Result<PathBuf, String> {
    let raw =
        std::env::var("ESSMAI_HANDOFF_DIR").map_err(|_| "ESSMAI_HANDOFF_DIR mungon".to_string())?;
    std::fs::create_dir_all(&raw).map_err(|e| format!("ESSMAI_HANDOFF_DIR s'u krijua: {e}"))?;
    std::fs::canonicalize(&raw).map_err(|e| format!("ESSMAI_HANDOFF_DIR s'u kanonikalizua: {e}"))
}

fn resolve_shadow_binary() -> Result<PathBuf, String> {
    match std::env::var("ESSMAI_SHADOW_BIN") {
        Ok(raw) => validate_binary(PathBuf::from(raw), "ESSMAI_SHADOW_BIN"),
        Err(_) => {
            let current =
                std::env::current_exe().map_err(|e| format!("current_exe dështoi: {e}"))?;
            let parent = current
                .parent()
                .ok_or_else(|| "quantum executable pa parent".to_string())?;
            validate_binary(parent.join(shadow_executable_name()), "sibling target")
        }
    }
}

fn validate_binary(path: PathBuf, source: &str) -> Result<PathBuf, String> {
    match (path.is_absolute(), path.is_file()) {
        (true, true) => Ok(path),
        (false, _) => Err(format!(
            "{source} duhet të japë rrugë absolute për Shadow main.rs"
        )),
        (_, false) => Err(format!(
            "Shadow main.rs mungon te '{}'. Ekzekuto fillimisht: cargo build --workspace",
            path.display()
        )),
    }
}

fn shadow_executable_name() -> &'static Path {
    #[cfg(windows)]
    {
        Path::new("shadow_platform.exe")
    }
    #[cfg(not(windows))]
    {
        Path::new("shadow_platform")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sibling_name_is_platform_specific_and_nonempty() {
        assert!(!shadow_executable_name().as_os_str().is_empty());
    }
}

/// Native T1 uses the unchanged constitutional request plus data-only prepared PD.
pub enum PreparedCycleOutcome {
    Native(ShadowCycleResponse),
    Operational(shadow_contracts::pd_memory::PdOperationRecord),
}
pub fn execute_prepared_cycle(request:&ShadowCycleRequest,prepared:&shadow_contracts::pd_prepared::PdPreparedEnvelope)->Result<PreparedCycleOutcome,String>{
    let reference=quantum_lim::pd_native_bridge::retain_intent(request,prepared)?;
    eprintln!("[PD/RECOVERY_REFERENCE] {}",reference.display());
    let bytes=shadow_contracts::encode_cycle_request(request);
    let envelope=shadow_contracts::pd_cycle::encode_cycle(&bytes,prepared)?;
    let response=invoke("--bridge-pd-once",&request.quantum.session_id,&envelope)?;
    if response.starts_with(b"PDR1") {
        use shadow_contracts::pd_memory as pd;
        let record=pd::decode_record(&response)?;
        pd::late_refusal::validate_response(&record,request,prepared)?;
        let lookup=pd::PdLookup{identity:record.request.identity.clone(),operation_id:Some(record.request.operation_id.clone())};
        let actual=invoke("--pd-head-once",&request.quantum.session_id,&pd::encode_lookup(&lookup)?)?;
        let stored=pd::decode_optional_record(&actual)?.ok_or("PD_LATE_DURABLE_RESULT_MISSING")?;
        if pd::encode_record(&stored)?!=response {return Err("PD_LATE_DURABLE_RESULT_MISMATCH".into());}
        return Ok(PreparedCycleOutcome::Operational(record));
    }
    let decoded=shadow_contracts::decode_cycle_response(&response).map_err(|e|e.to_string())?;
    if decoded.session_id!=request.quantum.session_id {return Err("PD_T1_RESPONSE_SESSION".into());}
    Ok(PreparedCycleOutcome::Native(decoded))
}
/// Shadow, not Quantum, selects the durable T1 receipt for T2.
pub fn finalize_prepared_cycle(request:&ShadowCycleRequest,prepared:&shadow_contracts::pd_prepared::PdPreparedEnvelope,response:&ShadowCycleResponse)->Result<shadow_contracts::pd_prepared::ContinuationData,String>{
    let key=shadow_contracts::pd_cycle::finalize_lookup(shadow_contracts::pd_cycle::identity(request)?,prepared.source_request_sha256);
    let bytes=invoke("--pd-finalize-once",&request.quantum.session_id,&key)?;
    let data=shadow_contracts::pd_prepared::decode_continuation(&bytes)?;
    data.validate_for(prepared,response)?;
    Ok(data)
}

/// KiE is a separate evidence origin; no legacy candidate statistics are forged.
pub fn execute_kie_selection(request:&shadow_contracts::kie::KieSelectionRequest)->Result<shadow_contracts::kie::KieSelectionResponse,String>{
    let encoded=shadow_contracts::kie::encode_request(request)?;
    let bytes=crate::noi_caller::invoke("QUANTUM_KIE_SELECTION",&request.selection.session_id,|| {
        let pulse=ess_mai_zero_noi::trigger(encoded.as_slice());
        quantum_lim::pd_operational::native_transport("--kie-select-once",pulse.request(),std::time::Duration::from_secs(90))
    }).map_err(|e|e.to_string())?;
    let response=shadow_contracts::kie::decode_response(&bytes)?;
    response.validate_for(request)?;
    Ok(response)
}
