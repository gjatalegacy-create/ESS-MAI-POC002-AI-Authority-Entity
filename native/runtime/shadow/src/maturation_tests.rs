//! Synthetic storage fixtures exercise durability, not native scientific admission.
use super::*;
use crate::vault_transaction::{test_support, GclReinforcement};

fn path(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("r36-{label}-{}-{}.svlt", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()))
}
fn transaction(n: u8, bytes: &[u8], domain: &str, reinforce: bool) -> VaultTransaction {
    let base = test_support::sample_transaction_with([n;32], [n+64;32], bytes);
    let mut writes = base.knowledge_writes;
    for write in &mut writes {
        if let KnowledgeWrite::Primitive {domain: d, ts_ns, ..} = write {
            *d = domain.into(); *ts_ns = u64::from(n);
        }
    }
    let verdict = crate::types::LawConfirmedVerdict::confirm(test_support::sample_supreme(1,1),false).unwrap();
    VaultTransaction::build(base.identity_digest,base.cycle_digest,base.anchor_generation,
        base.negative_batch_digest,base.runtime_delta_digest,base.vds_close_digest,
        base.law0_digest,&verdict,writes,base.lineage,
        reinforce.then_some(GclReinforcement {primitive_anchor:41,hits:1}),base.receipt)
}
fn seed_ready(v: &KnowledgeVault) { seed_ready_with(v, b"mature-object"); }
fn seed_ready_with(v: &KnowledgeVault, bytes: &[u8]) {
    for (i,domain) in ["science","industrial","medical","security","research"].iter().enumerate() {
        v.commit_test_transaction(transaction(i as u8+1,bytes,domain,i<3)).unwrap();
    }
    assert_eq!((v.primitive_count(),v.legacy_count()),(1,0));
    assert!(v.primitive.read().unwrap()[0].ready_for_legacy());
    // A later, unready object shares the package id, but not the content.
    v.commit_test_transaction(transaction(6,b"later-object","science",false)).unwrap();
}
fn request() -> shadow_contracts::KnowledgeSelectionRequestWire {
    use shadow_contracts::*;
    KnowledgeSelectionRequestWire {version:PROTOCOL_VERSION,requester:SelectionRequester::AshtQuantum,
        purpose:SelectionPurpose::EvidenceCandidates,session_id:"maturity-storage".into(),
        parent_i0:"11".repeat(32),primitive_anchor:41,trace_id:"maturity-storage".into(),
        split_sha256:"22".repeat(32),xi:vec!["mature-object".into(),"later-object".into()],
        yi:vec!["mature-object".into()],upstream_hold_sha256:Some("33".repeat(32)),
        evidence_sha256:"44".repeat(32),max_positive:SELECTION_MAX_CANDIDATES,
        max_negative:SELECTION_MAX_CANDIDATES}
}
#[test]
fn r36_manual_maturation_survives_disk_reopen_with_exact_selection() {
    let path=path("manual-reopen");
    let expected;
    let expected_reason;
    let expected_legacy;
    {
        let v=KnowledgeVault::open_disk(&path).unwrap();
        seed_ready(&v);
        // Warm the derived addresses before removal shifts the surviving row.
        let before=v.select_bounded_candidates(&request()).unwrap();
        assert_eq!(before.0.len(),2);
        assert!(before.0.iter().all(|row|row.domain=="primitive"));
        // These soft consultations are deliberately not journaled. Eligible
        // durable history already saturates the unchanged observer access term.
        let durable_reason=v.primitive.read().unwrap()[0].legacy_reason();
        for _ in 0..17 { v.note_primitive_access(41); }
        assert_eq!(v.primitive.read().unwrap()[0].legacy_reason(),durable_reason);
        assert_eq!(v.run_maturation().unwrap(),1);
        expected_reason=v.observer.reason_for(41).unwrap();
        assert_eq!(expected_reason,durable_reason);
        let row=v.legacy.read().unwrap()[0].clone();
        expected_legacy=(row.raw_bytes,row.crystallized_at,row.legacy_score.to_bits(),row.immutable_seal);
        let length=std::fs::metadata(&path).unwrap().len();
        assert_eq!(v.run_maturation().unwrap(),0);
        assert_eq!(std::fs::metadata(&path).unwrap().len(),length,"empty retry must not append");
        assert_eq!((v.primitive_count(),v.legacy_count(),v.observer.birth_count()),(1,1,1));
        expected=v.select_bounded_candidates(&request()).unwrap();
        assert_eq!(expected.0.len(),2);
        assert_eq!(expected.0.iter().map(|row|row.domain.as_str()).collect::<Vec<_>>(),["primitive","legacy"]);
        assert_eq!(v.primitive.read().unwrap()[0].raw_bytes,b"later-object");
    }
    let committed_bytes=std::fs::read(&path).unwrap(); // ownership released on Windows
    {
        let v=KnowledgeVault::open_disk(&path).unwrap();
        assert_eq!((v.primitive_count(),v.legacy_count(),v.observer.birth_count()),(1,1,1),
            "successful manual projection must survive reopen without new evidence");
        assert_eq!(v.select_bounded_candidates(&request()).unwrap(),expected);
        assert_eq!(v.observer.reason_for(41).unwrap(),expected_reason);
        let row=v.legacy.read().unwrap()[0].clone();
        assert_eq!((row.raw_bytes,row.crystallized_at,row.legacy_score.to_bits(),row.immutable_seal),expected_legacy);
        assert_eq!(v.run_maturation().unwrap(),0);
        assert_eq!(std::fs::metadata(&path).unwrap().len(),committed_bytes.len() as u64);
    }
    assert_eq!(std::fs::read(&path).unwrap(),committed_bytes,"reopen/empty retry must preserve exact WAL bytes");
    std::fs::remove_file(path).unwrap();
}
#[derive(Debug)]
struct RejectMaturation { _owner: Box<dyn VaultBackend> }
impl VaultBackend for RejectMaturation {
    fn append_maturation(&self, permit: MaturationAppend<'_>) -> Result<(), VaultError> {
        permit.record_for(self)?;
        Err(VaultError::Io("injected maturation append failure".into()))
    }
}
#[test]
fn r36_maturation_append_failure_keeps_rows_births_and_wal_unchanged() {
    let path=path("append-failure");
    let expected;
    {
        let v=KnowledgeVault::open_disk(&path).unwrap();
        seed_ready(&v);
        expected=v.select_bounded_candidates(&request()).unwrap();
    }
    let before=std::fs::read(&path).unwrap();
    {
        let mut v=KnowledgeVault::open_disk(&path).unwrap();
        let owner=std::mem::replace(&mut v.backend,Box::new(InMemoryBackend));
        v.backend=Box::new(RejectMaturation {_owner:owner});
        assert!(v.run_maturation().unwrap_err().to_string().contains("injected maturation append failure"));
        assert_eq!((v.primitive_count(),v.legacy_count(),v.observer.birth_count()),(2,0,0));
        assert_eq!(v.select_bounded_candidates(&request()).unwrap(),expected);
        assert_eq!(std::fs::metadata(&path).unwrap().len(),before.len() as u64);
    }
    assert_eq!(std::fs::read(&path).unwrap(),before,"rejected append must preserve exact WAL bytes");
    let v=KnowledgeVault::open_disk(&path).unwrap();
    assert_eq!((v.primitive_count(),v.legacy_count()),(2,0));
    drop(v);std::fs::remove_file(path).unwrap();
}
#[derive(Debug)]
struct RedirectMaturation { other: InMemoryBackend }
impl VaultBackend for RedirectMaturation {
    fn append_maturation(&self, permit: MaturationAppend<'_>) -> Result<(), VaultError> {
        self.other.append_maturation(permit)
    }
}
#[test]
fn r36_maturation_permission_cannot_move_to_another_backend() {
    let v=KnowledgeVault::with_backend(Box::new(RedirectMaturation {other:InMemoryBackend}));
    seed_ready(&v);
    assert!(v.run_maturation().unwrap_err().to_string().contains("MATURATION_APPEND_WRONG_BACKEND"));
    assert_eq!((v.primitive_count(),v.legacy_count(),v.observer.birth_count()),(2,0,0));
}
fn fixture_wal(raw:&[u8], mature:bool) -> Vec<u8> {
    let path=path("fault-source");
    {
        let v=KnowledgeVault::open_disk(&path).unwrap();
        seed_ready_with(&v,raw);
        if mature {assert_eq!(v.run_maturation().unwrap(),1);}
    }
    let bytes=std::fs::read(&path).unwrap();
    std::fs::remove_file(path).unwrap();
    bytes
}
fn last_frame(bytes:&[u8])->usize {
    let mut offset=12;
    loop {
        let len=u32::from_le_bytes(bytes[offset..offset+4].try_into().unwrap()) as usize;
        if offset+12+len==bytes.len() {return offset;}
        offset+=12+len;
        assert!(offset+12<bytes.len());
    }
}
// Independent test CRC reconstruction lets altered semantic records pass framing.
fn fixture_crc(bytes:&[u8])->u32 {
    let mut crc=!0u32;
    for &byte in bytes {
        crc^=u32::from(byte);
        for _ in 0..8 {crc=(crc>>1)^(0xEDB88320u32.wrapping_mul(crc&1));}
    }
    !crc
}
fn reframe(bytes:&mut [u8],offset:usize) {
    let len=u32::from_le_bytes(bytes[offset..offset+4].try_into().unwrap()) as usize;
    let mut checked=bytes[offset..offset+4].to_vec();
    checked.extend_from_slice(&bytes[offset+12..offset+12+len]);
    bytes[offset+8..offset+12].copy_from_slice(&fixture_crc(&checked).to_le_bytes());
}
fn reject_with_torn_tail(mut bytes:Vec<u8>,label:&str,semantic:bool) {
    // Consistent LEN/!LEN; missing CRC and payload would be repairable in isolation.
    bytes.extend_from_slice(&46u32.to_le_bytes());
    bytes.extend_from_slice(&(!46u32).to_le_bytes());
    let path=path(label);
    std::fs::write(&path,&bytes).unwrap();
    let error=KnowledgeVault::open_disk(&path).unwrap_err().to_string();
    if semantic {assert!(error.contains("MATURATION_PLAN_MISMATCH"),"{label}: {error}");}
    assert_eq!(std::fs::read(&path).unwrap(),bytes,"{label}: failure must precede tail repair");
    std::fs::remove_file(path).unwrap();
}
#[test]
fn r36_maturation_corrupt_or_semantically_changed_action_prevents_tail_repair() {
    let original=fixture_wal(b"mature-object",true);
    let offset=last_frame(&original);
    assert_eq!(original[offset+12],8);
    let mut crc_bad=original.clone();crc_bad[offset+18]^=1;
    reject_with_torn_tail(crc_bad,"crc",false);
    let mut digest_bad=original.clone();digest_bad[offset+18]^=1;reframe(&mut digest_bad,offset);
    reject_with_torn_tail(digest_bad,"plan-digest",true);
    let mut count_bad=original.clone();
    count_bad[offset+14..offset+18].copy_from_slice(&2u32.to_le_bytes());
    reframe(&mut count_bad,offset);
    reject_with_torn_tail(count_bad,"plan-count",true);
    let mut version_bad=original.clone();version_bad[offset+13]=2;reframe(&mut version_bad,offset);
    reject_with_torn_tail(version_bad,"unknown-policy",false);
    let mut repeated=original.clone();repeated.extend_from_slice(&original[offset..]);
    reject_with_torn_tail(repeated,"repeated-action",true);
}
#[test]
fn r36_maturation_foreign_same_count_and_anchor_action_is_rejected() {
    let original=fixture_wal(b"mature-object",true);
    let offset=last_frame(&original);
    let mut foreign=fixture_wal(b"foreign-object",false);
    // Same row position, count, anchor, traces and metrics; only content differs.
    foreign.extend_from_slice(&original[offset..]);
    reject_with_torn_tail(foreign,"foreign-content",true);
}

#[derive(Debug)]
struct RedirectKnowledge { other: InMemoryBackend }
impl VaultBackend for RedirectKnowledge {
    fn append_verified(&self, permit: KnowledgeAppend<'_>) -> Result<(), VaultError> {
        self.other.append_verified(permit)
    }
}
#[test]
fn r36_knowledge_permission_rejects_same_address_different_backend_type() {
    let v=KnowledgeVault::with_backend(Box::new(RedirectKnowledge {other:InMemoryBackend}));
    let error=v.commit_test_transaction(transaction(1,b"object","science",false)).unwrap_err();
    assert!(error.to_string().contains("LIVE_APPEND_WRONG_BACKEND"));
    assert_eq!(v.store_counts_for_test(),[0;7]);
}
#[test]
fn r36_torn_maturation_action_recovers_only_its_complete_transaction_prefix() {
    let original=fixture_wal(b"mature-object",true);
    let offset=last_frame(&original);
    let prefix=&original[..offset];
    let path=path("torn-action");
    // Complete, consistent header; only eight bytes of its 46-byte payload.
    std::fs::write(&path,&original[..offset+12+8]).unwrap();
    {
        let v=KnowledgeVault::open_disk(&path).unwrap();
        assert_eq!((v.primitive_count(),v.legacy_count(),v.observer.birth_count()),(2,0,0));
        let selected=v.select_bounded_candidates(&request()).unwrap();
        assert_eq!(selected.0.len(),2);
        assert!(selected.0.iter().all(|row|row.domain=="primitive"));
        assert_eq!(std::fs::metadata(&path).unwrap().len(),prefix.len() as u64);
    }
    assert_eq!(std::fs::read(&path).unwrap(),prefix,"repair must retain every complete prior transaction");
    std::fs::remove_file(path).unwrap();
}
