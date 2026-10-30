//! Exact historical native02 bytes test storage/reference behavior.
//! These tests do not execute fresh GCL/ASHT or claim an admitted new user turn.
use super::*;
use shadow_contracts::pd_memory::{self as pd, iz};
use shadow_contracts::pd_prepared::{self, PdContinuationLink};
use shadow_contracts::pd_recovery::{self, RecoveryBundle};
use std::path::PathBuf;

const BASE: &[u8] = include_bytes!("test_fixtures/native_inquiry_r8/base.wal");
const BUNDLE: &[u8] = include_bytes!("test_fixtures/native_inquiry_r8/support.bundle");

struct Fixture { path: PathBuf }
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir();
        for suffix in 0..100 {
            let dir = base.join(format!("essmai-native-inquiry-r8-{}-{}-{suffix}",
                std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
            match std::fs::create_dir(&dir) {
                Ok(()) => { let path=dir.join("vault.wal"); std::fs::write(&path,BASE).unwrap(); return Self{path}; },
                Err(e) if e.kind()==std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("isolated fixture directory: {e}"),
            }
        }
        panic!("isolated fixture allocation exhausted")
    }
    fn open(&self)->KnowledgeVault { KnowledgeVault::open_disk(&self.path).unwrap() }
    fn unchanged(&self){ assert_eq!(std::fs::read(&self.path).unwrap(),BASE); }
}
impl Drop for Fixture {
    fn drop(&mut self){
        let _=std::fs::remove_file(&self.path);
        if let Some(parent)=self.path.parent(){ let _=std::fs::remove_dir(parent); }
    }
}
fn context(vault:&KnowledgeVault)->(iz::IzContext,RecoveryBundle,pd::PdOperationRecord){
    let bundle=pd_recovery::decode_bundle(BUNDLE).unwrap();
    let prepared=pd_prepared::decode_prepared(&bundle.prepared).unwrap();
    let identity=&prepared.source_dialogue.as_ref().unwrap().context.identity;
    let head=vault.pd_head(identity).unwrap().unwrap();
    let mut context=iz::context_from_record(&head).unwrap();
    context.version=iz::IZ_NATIVE_CONTINUITY_VERSION;
    context.operation_id="actual-next-inquiry-map-regression".into();
    context.user_text="Nuk kam matjen e pavarur ende.".into();
    context.answer=iz::IzAnswer::default();
    context.parent=Some(iz::IzParent::from_record(&head).unwrap());
    context.map=None;
    context.native_anchor=Some(PdContinuationLink::from_bundle(&bundle).unwrap());
    context.native_map=None;
    context.validate().unwrap();
    (context,bundle,head)
}
fn native_head(vault:&KnowledgeVault,context:&iz::IzContext)->crate::native_cycle::NativePdHead{
    let anchor=context.native_anchor.as_ref().unwrap();
    let scope=crate::native_cycle::cognitive_scope_key(&anchor.cognitive_session_id,&anchor.territory);
    *vault.native_pd_heads.read().unwrap().get(&scope).unwrap()
}

#[test]
fn native_map_selects_native_checkpoint_without_replacing_operational_or_native_heads(){
    let fixture=Fixture::new();
    {
        let vault=fixture.open();
        let(mut c,bundle,head)=context(&vault);
        let prior_native=native_head(&vault,&c);
        let map=vault.pd_iz_native_map(&c).unwrap();
        assert_eq!(map.operational_head,Some(iz::IzParent::from_record(&head).unwrap()));
        assert_eq!(map.checkpoint_base,iz::IzCheckpointBase::NativeT2);
        assert_eq!(map.native_anchor,c.native_anchor.clone().unwrap());
        let encoded=iz::encode_native_map_snapshot(&map).unwrap();
        assert_eq!(iz::decode_native_map_snapshot(&encoded).unwrap(),map);
        c.native_map=Some(map);
        assert_eq!(vault.pd_iz_native_read(&c).unwrap(),bundle);
        assert_eq!(vault.pd_iz_native_operational_read(&c).unwrap(),Some(head.clone()));
        // Real historical data demonstrates why revision is not cognitive turns.
        let native=pd_prepared::decode_prepared(&bundle.prepared).unwrap();
        let session=pd::hex(&c.identity.key().unwrap());
        let native_meta=shadow_contracts::pd_checkpoint_data::checkpoint_metadata(&native.checkpoint,&session).unwrap();
        let operative_meta=shadow_contracts::pd_checkpoint_data::checkpoint_metadata(iz::checkpoint_raw(&head).unwrap(),&session).unwrap();
        assert_eq!(native_meta.total_turns,operative_meta.total_turns+1);
        assert_eq!(vault.pd_head(&c.identity).unwrap(),Some(head));
        assert_eq!(native_head(&vault,&c),prior_native);
    }
    fixture.unchanged();
}

#[test]
fn native_inquiry_reads_never_finalize_missing_t2(){
    let fixture=Fixture::new();
    {
        let vault=fixture.open();
        let(mut c,_,head)=context(&vault);
        c.native_map=Some(vault.pd_iz_native_map(&c).unwrap());
        let anchor=c.native_anchor.as_ref().unwrap();
        // Explicit missing-index state fixture; no fabricated receipt or T2.
        let removed=vault.continuation_refs.write().unwrap().remove(&anchor.predecessor.identity).unwrap();
        for err in [vault.pd_iz_native_map(&c).unwrap_err(),vault.pd_iz_native_read(&c).unwrap_err(),
            vault.pd_iz_native_operational_read(&c).unwrap_err()] {
            assert!(err.to_string().contains("PD_NATIVE_PREDECESSOR_T2_NOT_DURABLE"));
        }
        assert!(!vault.continuation_refs.read().unwrap().contains_key(&anchor.predecessor.identity));
        assert_eq!(vault.pd_head(&c.identity).unwrap(),Some(head));
        vault.continuation_refs.write().unwrap().insert(anchor.predecessor.identity,removed);
    }
    fixture.unchanged();
}

#[test]
fn native_inquiry_rejects_foreign_or_modified_anchor_and_stale_operational_map(){
    let fixture=Fixture::new();
    {
        let vault=fixture.open();
        let(mut c,_,head)=context(&vault);
        let before=native_head(&vault,&c);
        c.native_map=Some(vault.pd_iz_native_map(&c).unwrap());
        for fault in 0..8 {
            let mut bad=c.clone();
            let anchor=bad.native_anchor.as_mut().unwrap();
            match fault {
                0=>anchor.predecessor.identity[0]^=1,
                1=>anchor.predecessor.request_sha256[0]^=1,
                2=>anchor.predecessor_transaction_id[0]^=1,
                3=>anchor.predecessor_continuation_sha256[0]^=1,
                4=>anchor.predecessor_next_i0.push('x'),
                5=>bad.parent.as_mut().unwrap().record_id[0]^=1,
                6=>{bad.native_map.as_mut().unwrap().checkpoint_base=iz::IzCheckpointBase::Operational(bad.parent.clone().unwrap());},
                _=>bad.native_map.as_mut().unwrap().native_anchor.predecessor_continuation_sha256[0]^=1,
            }
            assert!(vault.pd_iz_native_read(&bad).is_err(),"fault {fault}");
            assert!(vault.pd_iz_native_operational_read(&bad).is_err(),"fault {fault}");
        }
        let mut missing=c.clone();missing.parent=None;missing.native_map=None;
        assert!(vault.pd_iz_native_map(&missing).is_err());
        assert_eq!(native_head(&vault,&c),before);
        assert_eq!(vault.pd_head(&c.identity).unwrap(),Some(head));
    }
    fixture.unchanged();
}

#[test]
fn native_inquiry_map_does_not_treat_a_historical_native_anchor_as_current(){
    let fixture=Fixture::new();
    {
        let vault=fixture.open();
        let(c,_,_)=context(&vault);
        let anchor=c.native_anchor.as_ref().unwrap();
        let scope=crate::native_cycle::cognitive_scope_key(&anchor.cognitive_session_id,&anchor.territory);
        let old=native_head(&vault,&c);
        // Only the reference is advanced in this rejection fixture. No claim of
        // a real admitted successor; the old historical bytes still resolve.
        let mut newer=old;newer.identity[0]^=1;
        vault.native_pd_heads.write().unwrap().insert(scope,newer);
        let error=vault.pd_iz_native_map(&c).unwrap_err();
        assert!(error.to_string().contains("PD_NATIVE_PREDECESSOR_NOT_CURRENT_HEAD"));
        assert_eq!(vault.native_pd_heads.read().unwrap().get(&scope),Some(&newer));
        vault.native_pd_heads.write().unwrap().insert(scope,old);
    }
    fixture.unchanged();
}

#[test]
fn old_operational_exact_retry_after_real_native_promotion_remains_no_append(){
    let fixture=Fixture::new();
    {
        let vault=fixture.open();
        let(c,_,head)=context(&vault);
        let before=native_head(&vault,&c);
        assert_eq!(vault.commit_pd_operation(head.request.clone()).unwrap(),head);
        assert!(vault.require_operational_pd_scope_unpromoted(&c.identity).unwrap_err()
            .to_string().contains("PD_NATIVE_CONTINUATION_REQUIRED_AFTER_PROMOTION"));
        assert_eq!(native_head(&vault,&c),before);
    }
    fixture.unchanged();
    let vault=fixture.open();
    let(c,_,head)=context(&vault);
    assert_eq!(vault.pd_head(&c.identity).unwrap(),Some(head));
}
