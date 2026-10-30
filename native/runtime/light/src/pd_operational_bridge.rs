//! new explicit operational continuity route. Light transports and renders;
//! it never creates PD state, knowledge, receipts, or a scientific verdict.
use sha2::{Digest, Sha256};
use shadow_contracts::pd_memory as pd;
use std::{
    fs::OpenOptions,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub fn dispatch(args: &[String]) -> Option<i32> {
    if args.get(1).map(String::as_str)==Some("--pd-history-once") {
        if args.len()!=4 {return Some(64);}
        return Some(match run_history(Path::new(&args[2]),Path::new(&args[3])) {Ok(())=>0,Err(e)=>{eprintln!("[LIGHT/PD/MAP] {e}");70}});
    }
    if args.get(1).map(String::as_str)==Some("--pd-native-recover-once") {
        if args.len()!=4 { return Some(64); }
        return Some(match recover_native(Path::new(&args[2]),Path::new(&args[3])) {
            Ok(())=>0,Err(e)=>{eprintln!("[LIGHT/PD/RECOVERY] {e}");70}
        });
    }

    if matches!(args.get(1).map(String::as_str),Some("--pd-answer-once"|"--pd-science-answer-once")) {
        return crate::pd_failure_frontend::dispatch(args);
    }
    match args {
        [_, flag, request, response]
            if flag == "--pd-continuity-once" || flag == "--pd-failure-once" =>
        {
            Some(match run(Path::new(request), Path::new(response), flag) {
                Ok(()) => 0,
                Err(e) => {
                    eprintln!("[LIGHT/PD] NoVerdict fail-closed: {e}");
                    70
                }
            })
        }
        // The interactive route has its own argument and owner/T2 validation.
        [_, flag, ..] if flag == "--pd-resume-native" => None,
        [_, flag, ..] if flag.starts_with("--pd-") => Some(64),
        _ => None,
    }
}
pub(crate) fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let f = std::fs::File::open(path).map_err(|e| format!("PD_OPEN:{e}"))?;
    let mut b = Vec::new();
    f.take((limit + 1) as u64)
        .read_to_end(&mut b)
        .map_err(|e| format!("PD_READ:{e}"))?;
    if b.len() > limit {
        return Err("PD_READ_LIMIT".into());
    }
    Ok(b)
}

pub(crate) fn publish_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let delivery_scope = shadow_contracts::sha256_hex(&[bytes]);
    crate::noi_caller::invoke("LIGHT_EXACT_BYTES_PUBLISH", &delivery_scope, || {
    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(format!(".{}.tmp", std::process::id()));
    let tmp = PathBuf::from(tmp);
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|e| format!("PD_PUBLISH_OPEN:{e}"))?;
    let result = (|| {
        f.write_all(bytes)
            .and_then(|_| f.sync_all())
            .map_err(|e| format!("PD_PUBLISH_SYNC:{e}"))?;
        drop(f);
        std::fs::hard_link(&tmp, path).map_err(|e| format!("PD_PUBLISH_LINK:{e}"))
    })();
    let _ = std::fs::remove_file(&tmp);
    result

    })
}
struct Exchange(PathBuf);
impl Drop for Exchange {
    fn drop(&mut self) {
        // Only exact files owned by this exchange, never a recursive cleanup.
        for name in ["input.pdc", "response.pdr"] {
            let _ = std::fs::remove_file(self.0.join(name));
        }
        let _ = std::fs::remove_dir(&self.0);
    }
}
fn exchange(root: &Path) -> Result<Exchange, String> {
    for index in 0..64 {
        let path = root.join(format!("pd-light-{}-{index}", std::process::id()));
        match std::fs::create_dir(&path) {
            Ok(()) => return Ok(Exchange(path)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("PD_HANDOFF_CREATE:{e}")),
        }
    }
    Err("PD_HANDOFF_COLLISION_LIMIT".into())
}
#[derive(Clone, Copy)]
pub(crate) enum NativePeer {
    Quantum,
    Shadow,
}
fn native_mode_allowed(peer: NativePeer, mode: &str) -> bool {
    matches!(
        (peer, mode),
        (
            NativePeer::Quantum,
            "--pd-continuity-once" | "--pd-failure-once" | "--pd-native-recover-once"
        ) | (NativePeer::Shadow, "--pd-head-once" | "--pd-history-once" | "--pd-recover-once" | "--pd-iz-map-once" | "--pd-iz-read-once" | "--pd-iz-native-map-once" | "--pd-iz-native-read-once" | "--pd-lgc-read-once" | "--pd-iz-native-lookup-once")
    )
}
// The native read returns the existing recovery envelope, not an operational
// record. Keep the envelope bound exclusive to its three typed read routes.
fn validate_transport_bounds(
    peer: NativePeer, mode: &str, input_len: usize, response_limit: usize,
) -> Result<(), String> {
    let max_response = if matches!(mode,
        "--pd-native-recover-once" | "--pd-recover-once" | "--pd-iz-native-read-once") {
        shadow_contracts::pd_recovery::MAX_BUNDLE_BYTES
    } else {
        pd::MAX_REQUEST_BYTES + 256
    };
    if !native_mode_allowed(peer, mode)
        || input_len > pd::MAX_SOURCE_BYTES || response_limit > max_response {
        return Err("PD_TRANSPORT_ARGUMENT_LIMIT".into());
    }
    Ok(())
}
/// Bounded native transport only. This does not mint a verdict or select a PD route.
pub(crate) fn native_exchange(
    peer: NativePeer,
    mode: &str,
    input: &[u8],
    response_limit: usize,
) -> Result<Vec<u8>, String> {
    use crate::lab_contracts::{CollapsePhase, GjataCollapseLaw};
    // Preserve the source Coordination phase discipline; this is not authentication.
    let directive = GjataCollapseLaw::issue(CollapsePhase::Coordination);
    if !GjataCollapseLaw::verify(&directive, CollapsePhase::Coordination) {
        return Err("PD_COORDINATION_PHASE".into());
    }
    validate_transport_bounds(peer, mode, input.len(), response_limit)?;
    let root = std::env::var_os("ESSMAI_HANDOFF_DIR")
        .map(PathBuf::from)
        .ok_or("PD_HANDOFF_MISSING")?;
    if !root.is_absolute() || !root.is_dir() {
        return Err("PD_HANDOFF_NOT_ABSOLUTE_DIRECTORY".into());
    }
    let work = exchange(&root)?;
    let staged = work.0.join("input.pdc");
    publish_new(&staged, input)?;
    let receipt = work.0.join("response.pdr");
    let current = std::env::current_exe().map_err(|e| format!("PD_EXE:{e}"))?;
    let parent = current.parent().ok_or("PD_EXE_PARENT")?;
    let executable = match peer {
        NativePeer::Quantum => parent.join(if cfg!(windows) {
            "quantum-platform.exe"
        } else {
            "quantum-platform"
        }),
        NativePeer::Shadow => std::env::var_os("ESSMAI_SHADOW_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                parent.join(if cfg!(windows) {
                    "shadow_platform.exe"
                } else {
                    "shadow_platform"
                })
            }),
    };
    if !executable.is_absolute() || !executable.is_file() {
        return Err("PD_NATIVE_EXECUTABLE_MISSING_OR_RELATIVE".into());
    }
    let mut command = Command::new(executable);
    command
        .arg(mode)
        .arg(&staged)
        .arg(&receipt)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .spawn()
        .map_err(|e| format!("PD_NATIVE_SPAWN:{e}"))?;
    let start = Instant::now();
    loop {
        let status = match child.try_wait() {
            Ok(status) => status,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("PD_TRANSPORT_STATUS_UNCERTAIN:{error}"));
            }
        };
        match status {
            Some(status) if status.success() => break,
            Some(status) => return Err(format!("PD_NATIVE_EXIT:{status}")),
            None if start.elapsed() >= Duration::from_secs(120) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("PD_TRANSPORT_UNCERTAIN: resolve exact operation before retry".into());
            }
            None => std::thread::sleep(Duration::from_millis(20)),
        }
    }
    read_bounded(&receipt, response_limit)
}

pub(crate) fn print_operational(record: &pd::PdOperationRecord) -> Result<(), String> {
    let delivery_scope = pd::hex(&record.record_id);
    crate::noi_caller::invoke("LIGHT_OPERATIONAL_OUTPUT", &delivery_scope, || {
    record.validate()?;
    println!(
        "{}",
        serde_json::json!({
            "kind":"OperationalNoVerdict","light_role":"transmitter",
            "operation_id":record.request.operation_id,"revision":record.revision,
            "identity_key":pd::hex(&record.request.identity.key()?),
            "state_digest":pd::hex(&record.state_digest),"record_id":pd::hex(&record.record_id),
            "question":record.request.scoped_question,"question_binding":record.operational_question_binding(),
            "next_user_i0":null,
            "knowledge_verdict":null,"spine9_verified":false,"authority_transferred":false,
            "candidate_status":if record.request.source_input.starts_with(b"PDF1") || pd::iz::source_is_iz(&record.request.source_input) {"user_reported_material_pending_admission"}
                else if record.request.candidate.is_empty(){"unavailable"}else{"unadmitted_model_material"}
        })
    );
    Ok(())

    })
}

fn run(request_path: &Path, response_path: &Path, command_name: &str) -> Result<(), String> {
    use crate::lab_contracts::{CollapsePhase, GjataCollapseLaw};
    // This static phase check preserves the source phase discipline. It is not
    // advertised as caller authentication or completion of the normal GCL cycle.
    let directive = GjataCollapseLaw::issue(CollapsePhase::Coordination);
    if !GjataCollapseLaw::verify(&directive, CollapsePhase::Coordination) {
        return Err("PD_COORDINATION_PHASE".into());
    }
    if response_path.exists() {
        return Err("PD_RESPONSE_EXISTS".into());
    }
    let input = read_bounded(request_path, pd::MAX_SOURCE_BYTES)?;
    let (identity, operation_id) = if command_name == "--pd-failure-once" {
        let request = pd::failure::decode_failure_input(&input)?;
        (request.identity, request.operation_id)
    } else {
        let request = pd::decode_continuity_input(&input)?;
        (request.identity, request.operation_id)
    };
    let input_sha: [u8; 32] = Sha256::digest(&input).into();
    let bytes = native_exchange(
        NativePeer::Quantum,
        command_name,
        &input,
        pd::MAX_REQUEST_BYTES + 256,
    )?;
    let record = pd::decode_record(&bytes)?;
    if record.request.identity != identity
        || record.request.operation_id != operation_id
        || record.request.witness.input_sha256 != input_sha
        || record.request.source_input != input
    {
        return Err("PD_QUANTUM_RESPONSE_BINDING".into());
    }
    publish_new(response_path, &bytes)?;
    print_operational(&record)?;
    Ok(())
}


/// Light transmits an explicit recovery reference. Quantum validates the native
/// finalizer and Shadow returns the original durable history; Light never judges.
fn recover_native(request:&Path,response:&Path)->Result<(),String>{
    use shadow_contracts::pd_recovery as recovery;
    if response.exists(){return Err("PD_RESPONSE_EXISTS".into());}
    let bytes=read_bounded(request,68)?;
    let lookup=recovery::decode_lookup(&bytes)?;
    let returned=native_exchange(NativePeer::Quantum,"--pd-native-recover-once",&bytes,recovery::MAX_BUNDLE_BYTES)?;
    let bundle=recovery::decode_bundle(&returned)?;
    bundle.validate_for(&lookup)?;
    let (_,_,continuation)=bundle.validated()?;
    let continuation=continuation.ok_or("LIGHT_PD_RECOVERY_WITHOUT_DURABLE_T2")?;
    publish_new(response,&returned)?;
    println!("{}",serde_json::json!({
        "route":"native_pd_recovery","status":"historical_continuation",
        "new_verdict":false,"rejudged":false,"question":continuation.question,
        "next_i0_id":continuation.next_i0_id,
        "transaction_id":pd::hex(&bundle.transaction_id),
        "identity":pd::hex(&bundle.identity),
    }));
    Ok(())
}

fn bind_history_response(query:&pd::history::PdHistoryLookup, bytes:&[u8])->Result<pd::history::PdHistoryPage,String>{
    let page=pd::history::decode_history_page(bytes)?;
    if &page.query!=query{return Err("LIGHT_PD_HISTORY_QUERY_BINDING".into());}
    Ok(page)
}
fn run_history(request_path:&Path,response_path:&Path)->Result<(),String>{
    use pd::history as h;
    if response_path.exists(){return Err("PD_RESPONSE_EXISTS".into());}
    let request=read_bounded(request_path,h::MAX_HISTORY_QUERY_BYTES)?;
    let query=h::decode_history_lookup(&request)?;
    let response=native_exchange(NativePeer::Shadow,"--pd-history-once",&request,h::MAX_HISTORY_PAGE_BYTES)?;
    let page=bind_history_response(&query,&response)?;
    publish_new(response_path,&response)?;
    let entries=page.entries.iter().map(|entry|serde_json::json!({
        "revision":entry.revision,"record_id":pd::hex(&entry.record_id),"previous_record":pd::hex(&entry.previous_record),
        "operation_id":entry.operation_id,"source_kind":format!("{:?}",entry.source_kind),"status":"OperationalNoVerdict",
        "question_reference":pd::hex(&entry.question_reference),"has_candidate":entry.has_candidate,
        "report_class":format!("{:?}",entry.report_class),"permitted_next":format!("{:?}",entry.permitted_next),"knowledge_verdict":null
    })).collect::<Vec<_>>();
    println!("{}",serde_json::json!({"kind":"PdMapHistory","light_role":"transmitter","material_store":"Vault","map":"pd_mp",
        "identity_key":pd::hex(&query.identity.key()?),"identity":{"owner":query.identity.owner,"obligation":query.identity.obligation,"case_id":query.identity.case_id,"scope":query.identity.scope,"domain":query.identity.domain,"lineage_root":query.identity.lineage_root},
        "from_revision":query.from_revision,"limit":query.limit,"head_revision":page.head_revision,"next_revision":page.next_revision,"entries":entries,"knowledge_verdict":null}));
    Ok(())
}
#[cfg(test)]
mod history_bridge_tests {
    use super::*;
    #[test] fn history_binds_full_query_and_transports_no_knowledge_status(){
        use pd::history::*;
        let query=PdHistoryLookup{identity:pd::PdIdentity{owner:"owner".into(),obligation:"o".into(),case_id:"c".into(),scope:"s".into(),domain:"d".into(),lineage_root:"l".into()},from_revision:1,limit:4};
        let page=PdHistoryPage{query:query.clone(),head_revision:0,entries:vec![],next_revision:None};
        let bytes=encode_history_page(&page).unwrap();assert_eq!(bind_history_response(&query,&bytes).unwrap(),page);
        let mut different=query.clone();different.identity.case_id="other".into();assert!(bind_history_response(&different,&bytes).is_err());
        different=query.clone();different.from_revision=2;assert!(bind_history_response(&different,&bytes).is_err());
        different=query;different.limit=3;assert!(bind_history_response(&different,&bytes).is_err());
        assert_eq!(dispatch(&["light".into(),"--pd-history-once".into()]),Some(64));
    }
}

#[cfg(test)]
mod lgc_transport_tests {
    use super::*;
    #[test]
    fn native_bundle_transport_accepts_its_contract_bound_without_widening_other_routes() {
        let bundle = shadow_contracts::pd_recovery::MAX_BUNDLE_BYTES;
        let record = pd::MAX_REQUEST_BYTES + 256;
        assert!(bundle > record);
        for (peer, mode) in [
            (NativePeer::Shadow, "--pd-iz-native-read-once"),
            (NativePeer::Shadow, "--pd-recover-once"),
            (NativePeer::Quantum, "--pd-native-recover-once"),
        ] {
            assert!(validate_transport_bounds(peer, mode, pd::MAX_SOURCE_BYTES, bundle).is_ok());
            assert!(validate_transport_bounds(peer, mode, 0, bundle + 1).is_err());
            assert!(validate_transport_bounds(peer, mode, pd::MAX_SOURCE_BYTES + 1, bundle).is_err());
        }
        for mode in ["--pd-head-once", "--pd-iz-native-map-once", "--pd-iz-read-once"] {
            assert!(validate_transport_bounds(NativePeer::Shadow, mode, 0, record).is_ok());
            assert!(validate_transport_bounds(NativePeer::Shadow, mode, 0, record + 1).is_err());
            assert!(validate_transport_bounds(NativePeer::Shadow, mode, 0, bundle).is_err());
        }
        assert!(validate_transport_bounds(NativePeer::Quantum, "--pd-iz-native-read-once", 0, bundle).is_err());
        assert!(validate_transport_bounds(NativePeer::Shadow, "--arbitrary-command", 0, 1).is_err());
    }
    #[test]
    fn lgc_read_routes_are_available_only_on_shadow_and_never_arbitrary_commands() {
        for mode in ["--pd-lgc-read-once","--pd-iz-native-lookup-once","--pd-iz-native-map-once","--pd-iz-native-read-once"] {
            assert!(native_mode_allowed(NativePeer::Shadow,mode));
            assert!(!native_mode_allowed(NativePeer::Quantum,mode));
        }
        assert!(!native_mode_allowed(NativePeer::Shadow,"--pd-lgc-write-once"));
        assert!(!native_mode_allowed(NativePeer::Quantum,"--pd-lgc-write-once"));
        assert!(native_mode_allowed(NativePeer::Shadow,"--pd-recover-once"));
        assert!(native_mode_allowed(NativePeer::Quantum,"--pd-native-recover-once"));
    }
}

#[cfg(test)]
mod r31_delivery_tests {
    #[test]
    fn r31_exact_publication_is_triggered_and_failed_retry_is_not_success() {
        let dir=std::env::temp_dir().join(format!("r31-publication-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&dir).unwrap();
        let path=dir.join("output");
        let input=b"red, black: exact bytes";
        crate::noi_caller::test_events();
        super::publish_new(&path,input).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(),input);
        let events=crate::noi_caller::test_events();
        assert!(events.iter().any(|(site,phase)|site=="LIGHT_EXACT_BYTES_PUBLISH" && phase=="RETURNED_OK"),
            "publication bypassed actual 0-NOI callback: {events:?}");
        assert!(super::publish_new(&path,b"different").is_err());
        let refused=crate::noi_caller::test_events();
        assert!(refused.iter().any(|(site,phase)|site=="LIGHT_EXACT_BYTES_PUBLISH" && phase=="RETURNED_ERROR"));
        assert_eq!(std::fs::read(&path).unwrap(),input);
        std::fs::remove_file(path).unwrap();std::fs::remove_dir(dir).unwrap();
    }
}

#[cfg(test)]
mod r34_native_resume_dispatch_tests {
    #[test]
    fn native_resume_reaches_its_owner_validated_handler_without_accepting_unknown_pd_commands() {
        let resume=vec!["light".into(),"--pd-resume-native".into(),"owner.lookup".into()];
        assert_eq!(super::dispatch(&resume),None);
        let malformed=vec!["light".into(),"--pd-resume-native".into()];
        assert_eq!(super::dispatch(&malformed),None);
        assert!(crate::pd_native_followup::initial(&malformed).unwrap_err().starts_with("LIGHT_PD_RESUME_USAGE"));
        assert_eq!(super::dispatch(&["light".into(),"--pd-unknown".into(),"owner.lookup".into()]),Some(64));
        assert_eq!(super::dispatch(&["light".into(),"--pd-resume-native-extra".into(),"owner.lookup".into()]),Some(64));
    }
}
