//! R37: bounded local transport inside the existing Shadow executable.
//! Generation/digests bind transport data, never grant epistemic authority.
//! Only the existing handlers may interpret wire or commit to the resident Vault.
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, atomic::{AtomicBool, AtomicU64, Ordering}};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const READY_LIMIT: usize = 16 * 1024;
const ERROR_LIMIT: usize = 8192;
const SLOTS: usize = 8;
const REQUEST_HEADER: usize = 109;
const RESPONSE_HEADER: usize = 173;
const WAIT_LIMIT: Duration = Duration::from_secs(85);
const POLL: Duration = Duration::from_millis(5);
const MODES: [&str; 17] = [
    "--selection-once", "--kie-select-once", "--project-register-once",
    "--bridge-once", "--bridge-pd-once", "--pd-finalize-once", "--pd-recover-once",
    "--pd-lgc-read-once", "--pd-iz-native-map-once", "--pd-iz-native-read-once",
    "--pd-iz-native-lookup-once", "--pd-iz-map-once", "--pd-iz-read-once",
    "--pd-history-once", "--pd-head-once", "--pd-candidate-once", "--pd-commit-once",
];

pub(crate) fn mode_limits(mode: &str) -> Option<(usize, usize)> {
    if !MODES.contains(&mode) { return None; }
    use shadow_contracts::{pd_memory as pd, pd_recovery};
    let request = match mode {
        "--kie-select-once" => shadow_contracts::kie::MAX_WIRE_BYTES,
        "--bridge-pd-once" => shadow_contracts::pd_cycle::MAX_CYCLE_BYTES,
        "--pd-recover-once" | "--pd-finalize-once" => 68,
        // Retain each existing handler's bound; typed decoders impose their own tighter limits.
        v if v.starts_with("--pd-") && v != "--pd-finalize-once" => pd::MAX_REQUEST_BYTES + 256,
        _ => shadow_contracts::MAX_FRAME_BYTES,
    };
    let response = match mode {
        "--kie-select-once" => shadow_contracts::kie::MAX_WIRE_BYTES,
        "--pd-recover-once" | "--pd-iz-native-read-once" => pd_recovery::MAX_BUNDLE_BYTES,
        "--pd-lgc-read-once" | "--pd-iz-native-lookup-once" => pd::lgc::MAX_OUTPUT_BYTES,
        "--pd-history-once" => pd::history::MAX_HISTORY_PAGE_BYTES,
        v if v.starts_with("--pd-") => pd::MAX_REQUEST_BYTES + 512,
        _ => shadow_contracts::MAX_FRAME_BYTES,
    };
    Some((request, response))
}
fn hash(bytes: &[u8]) -> [u8; 32] { Sha256::digest(bytes).into() }
fn hex(bytes: &[u8; 32]) -> String { shadow_contracts::pd_memory::hex(bytes) }
fn parse_hash(raw: &str) -> Result<[u8; 32], String> {
    if raw.len() != 64 || !raw.bytes().all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v)) {
        return Err("WORKER_HEX".into());
    }
    let mut out = [0; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&raw[i * 2..i * 2 + 2], 16).map_err(|_| "WORKER_HEX")?;
    }
    Ok(out)
}
fn unique_hash(material: &[u8]) -> [u8; 32] {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let mut h = Sha256::new();
    h.update(b"ESSMAI/SHADOW/TRANSPORT-NONCE/R37");
    h.update(std::process::id().to_le_bytes());
    h.update(NEXT.fetch_add(1, Ordering::Relaxed).to_le_bytes());
    h.update(SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |v| v.as_nanos()).to_le_bytes());
    h.update(material);
    h.finalize().into()
}
fn plain(path: &Path, directory: bool) -> Result<(), String> {
    let m = fs::symlink_metadata(path).map_err(|e| format!("WORKER_PATH:{e}"))?;
    if m.file_type().is_symlink() || (directory && !m.is_dir()) || (!directory && !m.is_file()) {
        return Err("WORKER_PATH_KIND".into());
    }
    Ok(())
}
fn read_bound(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    plain(path, false)?;
    let file = fs::File::open(path).map_err(|e| format!("WORKER_READ:{e}"))?;
    if file.metadata().map_err(|e| e.to_string())?.len() > limit as u64 {
        return Err("WORKER_SIZE".into());
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes).map_err(|e| format!("WORKER_READ:{e}"))?;
    if bytes.len() > limit { return Err("WORKER_SIZE".into()); }
    Ok(bytes)
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut f = OpenOptions::new().write(true).create_new(true).open(path)
        .map_err(|e| format!("WORKER_WRITE:{e}"))?;
    f.write_all(bytes).and_then(|_| f.sync_all()).map_err(|e| format!("WORKER_SYNC:{e}"))
}
fn publish(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension(format!("{}.tmp", hex(&unique_hash(bytes))));
    write_new(&temp, bytes)?;
    let result = fs::hard_link(&temp, path).map_err(|e| format!("WORKER_PUBLISH:{e}"));
    let _ = fs::remove_file(temp);
    result
}
fn configuration() -> Result<(PathBuf, [u8; 32]), String> {
    let root = std::env::var_os("ESSMAI_HANDOFF_DIR").ok_or("WORKER_HANDOFF_MISSING")?;
    let root = PathBuf::from(root);
    if !root.is_absolute() { return Err("WORKER_HANDOFF_RELATIVE".into()); }
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
    let vault = super::wisdom_vault_path();
    let mut h = Sha256::new();
    h.update(b"ESSMAI/SHADOW/WORKER-CONFIG/R37");
    for part in [root.to_str().ok_or("WORKER_PATH_UTF8")?, vault.as_str()] {
        h.update((part.len() as u64).to_le_bytes()); h.update(part.as_bytes());
    }
    Ok((root, h.finalize().into()))
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Ready { generation: [u8; 32], mailbox: PathBuf, configuration: [u8; 32] }
impl Ready {
    fn encode(&self) -> Result<Vec<u8>, String> {
        let path = self.mailbox.to_str().ok_or("WORKER_PATH_UTF8")?;
        if path.contains(['\n', '\r']) { return Err("WORKER_PATH_NEWLINE".into()); }
        let bytes = format!("SWR1\n{}\n{}\n{}\n", hex(&self.generation), path, hex(&self.configuration)).into_bytes();
        if bytes.len() > READY_LIMIT { return Err("WORKER_READY_SIZE".into()); }
        Ok(bytes)
    }
    fn decode(bytes: &[u8], root: &Path, configuration: [u8; 32]) -> Result<Self, String> {
        if bytes.len() > READY_LIMIT { return Err("WORKER_READY_SIZE".into()); }
        let raw = std::str::from_utf8(bytes).map_err(|_| "WORKER_READY_UTF8")?;
        let parts = raw.split('\n').collect::<Vec<_>>();
        if parts.len() != 5 || parts[0] != "SWR1" || parts[4] != "" { return Err("WORKER_READY_SHAPE".into()); }
        let generation = parse_hash(parts[1])?;
        let actual = parse_hash(parts[3])?;
        let mailbox = PathBuf::from(parts[2]);
        if actual != configuration || mailbox != root.join(format!(".shadow-worker-{}", hex(&generation))) {
            return Err("WORKER_CONFIGURATION_MISMATCH".into());
        }
        plain(&mailbox, true)?;
        Ok(Self { generation, mailbox, configuration })
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Request { generation: [u8; 32], id: [u8; 32], mode: u8, length: u64, digest: [u8; 32] }
impl Request {
    fn encode(&self) -> Vec<u8> {
        let mut out = b"SWQ1".to_vec();
        out.extend(self.generation); out.extend(self.id); out.push(self.mode);
        out.extend(self.length.to_le_bytes()); out.extend(self.digest); out
    }
    fn decode(bytes: &[u8], generation: [u8; 32]) -> Result<Self, String> {
        if bytes.len() != REQUEST_HEADER || &bytes[..4] != b"SWQ1" { return Err("WORKER_REQUEST_SHAPE".into()); }
        let mut request = Self { generation: [0;32], id:[0;32], mode:bytes[68], length:0, digest:[0;32] };
        request.generation.copy_from_slice(&bytes[4..36]); request.id.copy_from_slice(&bytes[36..68]);
        let mut length=[0;8]; length.copy_from_slice(&bytes[69..77]); request.length=u64::from_le_bytes(length);
        request.digest.copy_from_slice(&bytes[77..109]);
        let mode = MODES.get(usize::from(request.mode)).ok_or("WORKER_MODE")?;
        let (limit, _) = mode_limits(mode).ok_or("WORKER_MODE")?;
        if request.generation != generation { return Err("WORKER_STALE_GENERATION".into()); }
        if request.length > limit as u64 { return Err("WORKER_REQUEST_LIMIT".into()); }
        Ok(request)
    }
    fn verify_body(&self, bytes: &[u8]) -> Result<(), String> {
        if self.length != bytes.len() as u64 || self.digest != hash(bytes) { return Err("WORKER_REQUEST_BINDING".into()); }
        Ok(())
    }
}
fn response_header(request: &Request, failed: bool, bytes: &[u8]) -> Vec<u8> {
    let mut out = b"SWA1".to_vec(); out.extend(request.generation); out.extend(request.id);
    out.extend(hash(&request.encode())); out.push(u8::from(failed));
    out.extend((bytes.len() as u64).to_le_bytes()); out.extend(hash(bytes));
    out.extend(hash(&out)); out
}
fn verify_response(request: &Request, header: &[u8], bytes: &[u8], limit: usize) -> Result<bool, String> {
    if header.len() != RESPONSE_HEADER || &header[..4] != b"SWA1" || header[100] > 1 {
        return Err("WORKER_RESPONSE_SHAPE".into());
    }
    let failed=header[100]==1;
    if bytes.len() > if failed {ERROR_LIMIT} else {limit} { return Err("WORKER_RESPONSE_LIMIT".into()); }
    if response_header(request, failed, bytes) != header { return Err("WORKER_RESPONSE_BINDING".into()); }
    Ok(failed)
}
const SLOT_FILES: [&str; 8] = ["accepted.bin", "request.bin", "ready", "claimed", "response.bin", "result", "abandoned", "rejected"];
fn clean_slot(slot: &Path) {
    // Exact transport files only; never recurse and never follow a directory link.
    if plain(slot, true).is_err() { return; }
    for name in SLOT_FILES { let _ = fs::remove_file(slot.join(name)); }
    let _ = fs::remove_dir(slot);
}
struct Pending { slot: PathBuf, id: [u8; 32], published: bool, completed: bool }
impl Drop for Pending {
    fn drop(&mut self) {
        if !self.published || self.completed { clean_slot(&self.slot); }
        else {
            // Never delete a request a worker may already be applying. Its terminal
            // result or not-yet-accepted abandonment is cleaned by that owner.
            let _ = write_new(&self.slot.join("abandoned"), &self.id);
        }
    }
}
fn reserve_slot(mailbox: &Path, id: [u8; 32]) -> Result<Pending, String> {
    plain(mailbox, true)?;
    for i in 0..SLOTS {
        let slot=mailbox.join(format!("slot-{i:02}"));
        match fs::create_dir(&slot) {
            Ok(())=>return Ok(Pending {slot,id,published:false,completed:false}),
            Err(e) if e.kind()==std::io::ErrorKind::AlreadyExists=>{},
            Err(e)=>return Err(format!("WORKER_SLOT:{e}")),
        }
    }
    Err("WORKER_QUEUE_FULL".into())
}
fn preflight_response(mode: &str, destination: &Path) -> Result<(), String> {
    // Match run_pd_operational's refusal BEFORE any operational commit. Finalize
    // and recovery retain their original post-operation publication semantics.
    if mode.starts_with("--pd-") && !matches!(mode,"--pd-finalize-once"|"--pd-recover-once")
        && destination.exists() { return Err("PD_RESPONSE_EXISTS".into()); }
    Ok(())
}
pub(crate) fn forward(ready_path: &Path, mode: &str, source: &Path, destination: &Path) -> Result<(), String> {
    if !ready_path.is_absolute() { return Err("WORKER_READY_RELATIVE".into()); }
    let (request_limit,response_limit)=mode_limits(mode).ok_or("WORKER_MODE")?;
    preflight_response(mode,destination)?;
    let (root, config)=configuration()?;
    let ready_bytes=read_bound(ready_path,READY_LIMIT)?;
    let ready=Ready::decode(&ready_bytes,&root,config)?;
    let body=read_bound(source,request_limit)?;
    let request=Request {generation:ready.generation,id:unique_hash(&body),
        mode:u8::try_from(MODES.iter().position(|v|*v==mode).ok_or("WORKER_MODE")?).map_err(|_|"WORKER_MODE")?,
        length:body.len() as u64,digest:hash(&body)};
    let mut pending=reserve_slot(&ready.mailbox,request.id)?;
    write_new(&pending.slot.join("request.bin"),&body)?;
    publish(&pending.slot.join("ready"),&request.encode())?;
    pending.published=true;
    let started=Instant::now();
    loop {
        if pending.slot.join("result").is_file() {
            let header=read_bound(&pending.slot.join("result"),RESPONSE_HEADER)?;
            let bytes=read_bound(&pending.slot.join("response.bin"),response_limit.max(ERROR_LIMIT))?;
            let failed=verify_response(&request,&header,&bytes,response_limit)?;
            pending.completed=true;
            if failed { return Err(format!("WORKER_HANDLER:{}",String::from_utf8_lossy(&bytes))); }
            return crate::process_bridge::publish_forwarded(mode,destination,&bytes);
        }
        if pending.slot.join("rejected").exists() { return Err("WORKER_TRANSPORT_REJECTED_STATUS_UNCERTAIN".into()); }
        if started.elapsed()>=WAIT_LIMIT { return Err("WORKER_TIMEOUT_STATUS_UNCERTAIN".into()); }
        if read_bound(ready_path,READY_LIMIT).as_deref()!=Ok(ready_bytes.as_slice()) {
            return Err("WORKER_STALE_GENERATION_STATUS_UNCERTAIN".into());
        }
        std::thread::sleep(POLL);
    }
}
fn abandoned(slot: &Path, id: &[u8;32]) -> bool {
    read_bound(&slot.join("abandoned"),32).is_ok_and(|bytes| bytes.as_slice()==id)
}
// An idle/completed slot may disappear between the poll and this inspection.
fn slot_is_present(slot: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(slot) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("WORKER_PATH:{error}")),
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() =>
            Err("WORKER_PATH_KIND".into()),
        Ok(_) => Ok(true),
    }
}
fn process_slot(slot: &Path, ready: &Ready, owner: &crate::process_bridge::ResidentOwner) -> Result<bool,String> {
    if !slot_is_present(slot)? { return Ok(false); }
    if !slot.join("ready").exists() { return Ok(false); }
    let header=read_bound(&slot.join("ready"),REQUEST_HEADER)?;
    let request=Request::decode(&header,ready.generation)?;
    if abandoned(slot,&request.id) { clean_slot(slot); return Ok(false); }
    fs::rename(slot.join("ready"),slot.join("claimed")).map_err(|e|format!("WORKER_CLAIM:{e}"))?;
    let mode=MODES[usize::from(request.mode)];
    let (request_limit,response_limit)=mode_limits(mode).ok_or("WORKER_MODE")?;
    let body=read_bound(&slot.join("request.bin"),request_limit)?;
    request.verify_body(&body)?;
    // Snapshot the verified bytes into a separate owner-created file; later
    // caller deletion/change of its transport source cannot alter handler input.
    write_new(&slot.join("accepted.bin"), &body)?;
    drop(body);
    // No caller-supplied filesystem paths cross this boundary. Handlers receive
    // only these fixed names within the owner's generation and claimed slot.
    if slot.join("response.bin").exists() || slot.join("result").exists() { return Err("WORKER_SLOT_NOT_EMPTY".into()); }
    let outcome=crate::process_bridge::dispatch_request(mode,&slot.join("accepted.bin"),&slot.join("response.bin"),Some(owner));
    let (failed,bytes)=match outcome {
        Ok(())=>(false,read_bound(&slot.join("response.bin"),response_limit)?),
        Err(error)=>{
            // A handler may have committed before publication failed. Preserve
            // its failure and let exact replay/recovery determine durable status.
            let _=fs::remove_file(slot.join("response.bin"));
            let mut bytes=error.into_bytes(); bytes.truncate(ERROR_LIMIT);
            write_new(&slot.join("response.bin"),&bytes)?;
            (true,bytes)
        },
    };
    publish(&slot.join("result"),&response_header(&request,failed,&bytes))?;
    if abandoned(slot,&request.id) { clean_slot(slot); }
    Ok(true)
}
struct ReadyPublication { path: PathBuf, bytes: Vec<u8> }
impl Drop for ReadyPublication {
    fn drop(&mut self) {
        if read_bound(&self.path,READY_LIMIT).as_deref()==Ok(self.bytes.as_slice()) { let _=fs::remove_file(&self.path); }
    }
}
pub(crate) fn serve(ready_path: &Path) -> Result<(),String> {
    if !ready_path.is_absolute() || ready_path.exists() { return Err("WORKER_READY_PATH".into()); }
    if !crate::phase9_integration::Phase9Integration::verify_no_bypass("bridge-worker") { return Err("WORKER_MEDIATION".into()); }
    let (root,config)=configuration()?;
    let generation=unique_hash(root.as_os_str().to_string_lossy().as_bytes());
    let mailbox=root.join(format!(".shadow-worker-{}",hex(&generation)));
    fs::create_dir(&mailbox).map_err(|e|format!("WORKER_MAILBOX:{e}"))?;
    // Existing lock order and complete authentication precede readiness.
    let owner=crate::process_bridge::ResidentOwner::open()?;
    let ready=Ready {generation,mailbox,configuration:config};
    let bytes=ready.encode()?;
    publish(ready_path,&bytes)?;
    let publication=ReadyPublication {path:ready_path.to_path_buf(),bytes};
    let stopped=Arc::new(AtomicBool::new(false));
    let signal=stopped.clone();
    let input=std::thread::Builder::new().name("shadow-owner-lifetime".into()).spawn(move || {
        let mut stdin=std::io::stdin().lock(); let mut chunk=[0u8;256];
        loop {match stdin.read(&mut chunk) {Ok(0)|Err(_)=>break,Ok(_)=>{}}}
        signal.store(true,Ordering::Release);
    }).map_err(|e|format!("WORKER_LIFETIME:{e}"))?;
    let mut served=0u64; let mut rejected=0u64;
    while !stopped.load(Ordering::Acquire) {
        for i in 0..SLOTS {
            if stopped.load(Ordering::Acquire) { break; }
            let slot=ready.mailbox.join(format!("slot-{i:02}"));
            if !slot.exists() { continue; }
            if slot.join("result").is_file() {
                if let Ok(raw)=read_bound(&slot.join("claimed"),REQUEST_HEADER) {
                    if let Ok(request)=Request::decode(&raw,ready.generation) {
                        if abandoned(&slot,&request.id) {clean_slot(&slot);}
                    }
                }
                continue;
            }
            if slot.join("rejected").exists() {
                for name in ["claimed", "ready"] {
                    if let Ok(raw)=read_bound(&slot.join(name),REQUEST_HEADER) {
                        if raw.len()==REQUEST_HEADER {
                            let mut id=[0;32];id.copy_from_slice(&raw[36..68]);
                            if abandoned(&slot,&id) {clean_slot(&slot);break;}
                        }
                    }
                }
                continue;
            }
            match process_slot(&slot,&ready,&owner) {
                Ok(true)=>served=served.saturating_add(1),Ok(false)=>{},
                Err(error)=>{
                    rejected=rejected.saturating_add(1);
                    eprintln!("[SHADOW WORKER] transport rejected: {error}");
                    if plain(&slot,true).is_ok() {let _=write_new(&slot.join("rejected"),b"WORKER_TRANSPORT_REJECTED\n");}
                },
            }
        }
        std::thread::sleep(POLL);
    }
    input.join().map_err(|_|"WORKER_LIFETIME_PANIC")?;
    drop(publication);
    eprintln!("[SHADOW WORKER] generation={} requests_served={} transport_rejected={} vault_opens=1 shutdown=stdin_eof",hex(&generation),served,rejected);
    // GLOBAL_VAULT also retains this same Arc until executable termination.
    // APUPK's existing crash-left lock policy is unchanged; normal drop releases it.
    drop(owner);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> Request { Request {generation:[1;32],id:[2;32],mode:0,length:3,digest:hash(b"abc")} }
    #[test]
    fn r37_transport_request_rejects_stale_mode_length_hash_and_trailing_bytes() {
        let q=request(); let bytes=q.encode();
        assert_eq!(Request::decode(&bytes,q.generation).unwrap(),q);
        assert!(Request::decode(&bytes,[9;32]).is_err());
        for mutation in [0,68,76] {let mut bad=bytes.clone();bad[mutation]=255;assert!(Request::decode(&bad,q.generation).is_err());}
        let mut trailing=bytes;trailing.push(0);assert!(Request::decode(&trailing,q.generation).is_err());
        assert!(q.verify_body(b"abd").is_err());assert!(q.verify_body(b"ab").is_err());
        assert!(mode_limits("--negative-once").is_none());assert!(mode_limits("--bridge-worker").is_none());
    }
    #[test]
    fn r37_transport_response_binds_generation_request_mode_status_length_and_body() {
        let q=request();let bytes=b"response";let header=response_header(&q,false,bytes);
        assert!(!verify_response(&q,&header,bytes,bytes.len()).unwrap());
        assert!(verify_response(&q,&header,bytes,bytes.len()-1).is_err());
        for offset in [4,36,68,100,101,109] {let mut changed=header.clone();changed[offset]^=1;assert!(verify_response(&q,&changed,bytes,100).is_err());}
        let mut different=q.clone();different.mode=1;assert!(verify_response(&different,&header,bytes,100).is_err());
        assert!(verify_response(&q,&header,b"different",100).is_err());
    }
    #[test]
    fn r37_transport_ready_rejects_foreign_store_and_mailbox() {
        let root=std::env::temp_dir().join(format!("r37-ready-{}",hex(&unique_hash(b"test"))));fs::create_dir(&root).unwrap();
        let generation=[3;32];let mailbox=root.join(format!(".shadow-worker-{}",hex(&generation)));fs::create_dir(&mailbox).unwrap();
        let ready=Ready {generation,mailbox:mailbox.clone(),configuration:[4;32]};let bytes=ready.encode().unwrap();
        assert_eq!(Ready::decode(&bytes,&root,[4;32]).unwrap(),ready);
        assert!(Ready::decode(&bytes,&root,[5;32]).is_err());
        let mut wrong=ready;wrong.mailbox=root.clone();assert!(Ready::decode(&wrong.encode().unwrap(),&root,[4;32]).is_err());
        fs::remove_dir(mailbox).unwrap();fs::remove_dir(root).unwrap();
    }
    #[test]
    fn r37_operational_response_preflight_preserves_refusal_before_commit() {
        let path=std::env::temp_dir().join(format!("r37-existing-{}",hex(&unique_hash(b"test"))));
        write_new(&path,b"existing exact response").unwrap();
        for mode in ["--pd-commit-once","--pd-head-once","--pd-history-once"] {
            assert_eq!(preflight_response(mode,&path).unwrap_err(),"PD_RESPONSE_EXISTS");
        }
        for mode in ["--pd-finalize-once","--pd-recover-once","--selection-once"] {
            preflight_response(mode,&path).unwrap();
        }
        assert_eq!(fs::read(&path).unwrap(),b"existing exact response");fs::remove_file(path).unwrap();
    }
    #[test]
    fn r37_transport_queue_is_bounded_and_timeout_does_not_delete_request() {
        let root=std::env::temp_dir().join(format!("r37-slots-{}",hex(&unique_hash(b"test"))));fs::create_dir(&root).unwrap();
        let mut held=Vec::new();for _ in 0..SLOTS {held.push(reserve_slot(&root,[7;32]).unwrap());}
        assert_eq!(reserve_slot(&root,[8;32]).err().unwrap(),"WORKER_QUEUE_FULL");
        let mut one=held.pop().unwrap();let slot=one.slot.clone();write_new(&slot.join("request.bin"),b"still owned").unwrap();one.published=true;drop(one);
        assert_eq!(fs::read(slot.join("request.bin")).unwrap(),b"still owned");assert!(abandoned(&slot,&[7;32]));clean_slot(&slot);
        drop(held);fs::remove_dir(root).unwrap();
    }

    #[test]
    fn r37_idle_cleanup_is_absent_but_malformed_or_claimed_input_remains_error() {
        let root=std::env::temp_dir().join(format!("r37-cleanup-{}",hex(&unique_hash(b"test"))));
        fs::create_dir(&root).unwrap();
        let mut completed=reserve_slot(&root,[2;32]).unwrap();
        let slot=completed.slot.clone();
        let q=request();
        write_new(&slot.join("claimed"),&q.encode()).unwrap();
        write_new(&slot.join("result"),&response_header(&q,false,b"done")).unwrap();
        // Deterministic interleaving: the poll saw a completed slot; its client
        // then consumed the result and removed the slot before worker inspection.
        assert!(slot.exists());
        completed.published=true;completed.completed=true;drop(completed);
        assert!(!slot_is_present(&slot).unwrap());

        fs::create_dir(&slot).unwrap();
        assert!(slot_is_present(&slot).unwrap());
        assert!(!slot.join("ready").exists()); // idle reservation, no work yet
        write_new(&slot.join("ready"),b"malformed").unwrap();
        let bad=read_bound(&slot.join("ready"),REQUEST_HEADER).unwrap();
        assert!(Request::decode(&bad,q.generation).is_err());
        fs::remove_file(slot.join("ready")).unwrap();
        write_new(&slot.join("ready"),&q.encode()).unwrap();
        fs::rename(slot.join("ready"),slot.join("claimed")).unwrap();
        // Once claimed, missing request input is still a transport error. The
        // benign absence rule applies only to the initial directory inspection.
        assert!(read_bound(&slot.join("request.bin"),3).unwrap_err().starts_with("WORKER_PATH:"));
        let malformed_slot=root.join("slot-file");
        write_new(&malformed_slot,b"not a directory").unwrap();
        assert_eq!(slot_is_present(&malformed_slot).unwrap_err(),"WORKER_PATH_KIND");
        clean_slot(&slot);fs::remove_file(malformed_slot).unwrap();fs::remove_dir(root).unwrap();
    }
}
