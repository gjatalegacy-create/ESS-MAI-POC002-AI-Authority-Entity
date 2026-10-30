//! Ordinary isolated v5 disk persistence tests.
//! Actual PD checkpoint/pending bytes, SYNTHETIC selection/layer/receipt/knowledge
//! fixture data. Never native ASHT/GCL acceptance or real scientific P/N proof.
//! Embedded fixtures are immutable, bounded and provenance-hashed; no environment
//! prerequisite, ignored test, production authority endpoint or new dependency.
use super::*;
use shadow_contracts::pd_memory::{self as memory,lgc};

struct Material {
    prepared:&'static [u8],response:&'static [u8],continuation:&'static [u8],
    parent:Option<&'static [u8]>,child_prepared:&'static [u8],child_response:&'static [u8],child_continuation:&'static [u8],
}
macro_rules! material {
    ($name:literal,$parent:expr)=>{Material{
        prepared:include_bytes!(concat!("test_fixtures/pd_v5/",$name,"/prepared.bin")),
        response:include_bytes!(concat!("test_fixtures/pd_v5/",$name,"/response.wire")),
        continuation:include_bytes!(concat!("test_fixtures/pd_v5/",$name,"/continuation.bin")),
        parent:$parent,
        child_prepared:include_bytes!(concat!("test_fixtures/pd_v5/",$name,"/child.prepared.bin")),
        child_response:include_bytes!(concat!("test_fixtures/pd_v5/",$name,"/child.response.wire")),
        child_continuation:include_bytes!(concat!("test_fixtures/pd_v5/",$name,"/child.continuation.bin")),
    }};
}
fn tx_fixture(raw:&[u8],response_bytes:&[u8],expected:&[u8],input_id:u64,
    predecessor:Option<&crate::vault_transaction::DurableCommit>)->Fixture {
    let mut prepared=pd::decode_prepared(raw).expect("checked fixture prepared");
    assert_eq!(prepared.source_request_sha256,[7;32],"declared synthetic request identity");
    let response=shadow_contracts::decode_cycle_response(response_bytes).unwrap();
    assert_eq!(shadow_contracts::encode_cycle_response(&response),response_bytes);
    pd::decode_continuation(expected).unwrap().validate_for(&prepared,&response).unwrap();
    if let Some(prior)=predecessor {
        prepared.continuity.as_mut().expect("v4 child").predecessor_transaction_id=prior.transaction_id;
    }
    let expected=pd::encode_continuation(&pd::derive_continuation(&prepared,&response).unwrap()).unwrap();
    let w=&response.verification_receipt;
    let receipt=VerificationReceipt{session_id:w.session_id.clone(),parent_i0:w.parent_i0.clone(),
        primitive_anchor:w.primitive_anchor,xy_digest:w.xy_digest,pd_binding_digest:w.pd_binding_digest,
        pd_continuum_activation_digest:w.pd_continuum_activation_digest,living_trust_digest:w.living_trust_digest,
        living_trust_sha256:hex32(&w.living_trust_sha256),y_verdict:w.y_verdict,x_verdict:w.x_verdict,
        seal:w.seal,generation:w.generation,receipt_id:w.receipt_id.clone()};
    let identity=identity_digest(&receipt.session_id,&receipt.parent_i0,receipt.primitive_anchor);
    let binding=NativeRequestBinding{model_case:None,identity_digest:identity,request_sha256:prepared.source_request_sha256,
        prepared:pd::encode_prepared(&prepared).unwrap(),kie_proof:vec![]};
    let negative=matches!((response.verdict.verified,response.verdict.primitive),(0,0));
    assert_eq!(response.negative_persisted,negative);
    let verdict=LawConfirmedVerdict::confirm(supreme(&response.verdict),negative).unwrap();
    let mut base=crate::vault_transaction::test_support::sample_transaction();base.lineage.input_id=input_id;
    let writes=if negative {vec![KnowledgeWrite::Negative{input_id,raw_bytes:NOTICE.as_bytes().to_vec(),
        context:NegativeContext{failure_reason_code:1,process:"SYNTHETIC_V5_STORAGE_TEST".into(),mass_at_fail:0.0,frequency:1},ts_ns:1001}]}
    else {for write in &mut base.knowledge_writes {match write {
        KnowledgeWrite::Primitive{input_id:id,raw_bytes,domain,..}=>{*id=input_id;*raw_bytes=NOTICE.as_bytes().to_vec();*domain="synthetic-v5-storage-fixture".into();},
        _=>panic!("unexpected fixture write"),}}
        base.knowledge_writes};
    let cycle=crate::vault_transaction::sha256_parts(&[b"SYNTHETIC_V5_STORAGE_ONLY",&identity,&input_id.to_le_bytes()]);
    let transaction=VaultTransaction::build(identity,cycle,base.anchor_generation,base.negative_batch_digest,
        base.runtime_delta_digest,base.vds_close_digest,base.law0_digest,&verdict,writes,base.lineage,None,receipt)
        .with_native_cycle(NativeCycleSnapshot::new(&binding,response).unwrap()).unwrap();
    Fixture{transaction,identity,request_sha256:binding.request_sha256,expected}
}
#[test]
fn checked_v5_fixtures_commit_reopen_t2_and_rebuild_exact_historical_operation_index(){
    let cases=[
        ("initial-negative",material!("initial-negative",None)),
        ("parent-negative",material!("parent-negative",Some(include_bytes!("test_fixtures/pd_v5/parent-negative/parent.record").as_slice()))),
        ("initial-positive",material!("initial-positive",None)),
        ("parent-positive",material!("parent-positive",Some(include_bytes!("test_fixtures/pd_v5/parent-positive/parent.record").as_slice()))),
    ];
    for (index,(name,data)) in cases.into_iter().enumerate(){
        let wal=TempWal::new(name);let env=pd::decode_prepared(data.prepared).unwrap();
        assert_eq!(env.version,pd::PREPARED_IZ_ORIGIN_VERSION);
        let context=env.source_iz.as_ref().unwrap().context.clone();
        let f=tx_fixture(data.prepared,data.response,data.continuation,0xC000+index as u64*4,None);
        let parent=data.parent.map(|bytes|memory::decode_record(bytes).unwrap());
        let commit;
        {
            let vault=KnowledgeVault::open_disk(&wal.path).unwrap();
            if let Some(parent)=&parent {
                let err=vault.commit_test_transaction(f.transaction.clone()).unwrap_err();
                assert!(err.to_string().contains("PD_NATIVE_IZ_NOT_CURRENT_HEAD"));
                assert!(vault.pd_iz_native_lookup(&context).unwrap().is_none());
                assert_eq!(vault.commit_pd_operation(parent.request.clone()).unwrap(),*parent);
                let reference=lgc::PdLgcRef::from_record(parent).unwrap();
                assert_eq!(vault.pd_lgc_record(&reference).unwrap(),Some(parent.clone()));
            }
            assert_eq!(counts(&vault),(0,0),"operational fixture creates no knowledge");
            commit=vault.commit_test_transaction(f.transaction.clone()).unwrap();
            assert_eq!(commit.status,CommitStatus::Committed);
            assert_eq!(commit.native_cycle.as_ref().unwrap().prepared,data.prepared);
        }
        let t1_bytes=std::fs::read(&wal.path).unwrap();
        let child=tx_fixture(data.child_prepared,data.child_response,data.child_continuation,
            0xC001+index as u64*4,Some(&commit));
        let lookup=shadow_contracts::pd_recovery::RecoveryLookup{identity:f.identity,request_sha256:f.request_sha256};
        {
            let vault=KnowledgeVault::open_disk(&wal.path).unwrap();
            assert_eq!(vault.pd_iz_native_lookup(&context).unwrap(),Some(lookup.clone()));
            let restored=vault.committed_identity(&f.identity).unwrap().unwrap();
            assert_eq!(restored.native_cycle,commit.native_cycle);
            assert_eq!(restored.transaction_id,commit.transaction_id);
            // Actual current-head T1 is insufficient: child needs actual durable T2.
            assert!(vault.commit_test_transaction(child.transaction.clone()).unwrap_err().to_string()
                .contains("PD_NATIVE_PREDECESSOR_T2_NOT_DURABLE"));
        }
        assert_eq!(std::fs::read(&wal.path).unwrap(),t1_bytes,"lookup/refusal never implicitly finalizes T2");
        let root_t2;
        {
            let vault=KnowledgeVault::open_disk(&wal.path).unwrap();let before=counts(&vault);
            root_t2=vault.finalize_native_continuation(f.identity,f.request_sha256).unwrap();
            assert_eq!(root_t2.material,f.expected);assert_eq!(root_t2.transaction_id,commit.transaction_id);
            assert_eq!(counts(&vault),before,"T2 is not a knowledge write");
        }
        let t2_bytes=std::fs::read(&wal.path).unwrap();assert!(t2_bytes.len()>t1_bytes.len());
        {
            let vault=KnowledgeVault::open_disk(&wal.path).unwrap();
            assert_eq!(vault.finalize_native_continuation(f.identity,f.request_sha256).unwrap(),root_t2);
        }
        assert_eq!(std::fs::read(&wal.path).unwrap(),t2_bytes);
        {
            let vault=KnowledgeVault::open_disk(&wal.path).unwrap();
            let committed_child=vault.commit_test_transaction(child.transaction.clone()).unwrap();
            assert_eq!(committed_child.status,CommitStatus::Committed);
            let child_t2=vault.finalize_native_continuation(child.identity,child.request_sha256).unwrap();
            assert_eq!(child_t2.material,child.expected);
        }
        let advanced_bytes=std::fs::read(&wal.path).unwrap();
        {
            let vault=KnowledgeVault::open_disk(&wal.path).unwrap();
            // Head has advanced to the v4 child. Lookup must still identify the original v5 T1.
            assert_eq!(vault.pd_iz_native_lookup(&context).unwrap(),Some(lookup));
            assert_eq!(vault.commit_test_transaction(f.transaction.clone()).unwrap().status,CommitStatus::AlreadyCommitted);
            assert_eq!(vault.finalize_native_continuation(f.identity,f.request_sha256).unwrap(),root_t2);
            let mut altered=context.clone();altered.answer.positive="alpha".into();altered.validate().unwrap();
            assert!(vault.pd_iz_native_lookup(&altered).unwrap_err().to_string().contains("PD_IZ_NATIVE_OPERATION_CONFLICT"));
            let mut unknown=context.clone();unknown.operation_id.push_str("-not-stored");
            assert!(vault.pd_iz_native_lookup(&unknown).unwrap().is_none());
            let actual=vault.recover_native(&shadow_contracts::pd_recovery::RecoveryLookup{
                identity:f.identity,request_sha256:f.request_sha256}).unwrap();
            let (_,response,continuation)=actual.validated().unwrap();assert!(continuation.is_some());
            let expected_pair=if name.ends_with("negative"){(0,0)}else{(1,1)};
            assert_eq!((response.verdict.verified,response.verdict.primitive),expected_pair);
            // Source dedup: equal reason/process/mass negatives increment
            // frequency; identical positive raw bytes add a trace, not an entry.
            assert_eq!(counts(&vault),if expected_pair==(0,0){(0,1)}else{(1,0)});
            if expected_pair==(0,0){assert_eq!(vault.export_negative()[0].frequency,2);}
            let child_commit=vault.committed_identity(&child.identity).unwrap().unwrap();
            let child_response=shadow_contracts::decode_cycle_response(&child_commit.native_cycle.unwrap().response).unwrap();
            assert_eq!((child_response.verdict.verified,child_response.verdict.primitive),expected_pair);
        }
        assert_eq!(std::fs::read(&wal.path).unwrap(),advanced_bytes,"historical retry/read/refusal is byte-identical");
    }
}


#[test]
fn one_logical_iz_operation_cannot_be_reused_across_operational_and_native_routes(){
    use shadow_contracts::pd_memory::iz;
    // Existing explicitly SYNTHETIC admission fixtures, real checkpoint and disk
    // semantics. No fresh ASHT/GCL result or scientific positive is asserted.
    let data=material!("parent-positive",Some(include_bytes!("test_fixtures/pd_v5/parent-positive/parent.record").as_slice()));
    let parent=memory::decode_record(data.parent.unwrap()).unwrap();
    let mut envelope=pd::decode_prepared(data.prepared).unwrap();
    let link=envelope.source_iz.as_mut().unwrap();
    assert_ne!(link.context.operation_id,parent.request.operation_id);
    link.context.operation_id=parent.request.operation_id.clone();
    // Rebind public causal hashes, so this is a semantically conflicting request
    // with valid framing/checksums, not a trivial malformed-byte rejection.
    link.passed=iz::IzBefore::new(&link.context,link.passed.before.request.clone()).unwrap()
        .activate().unwrap().complete_available(link.passed.response.clone(),link.passed.asht_attestation_sha256).unwrap();
    let response=shadow_contracts::decode_cycle_response(data.response).unwrap();
    let raw=pd::encode_prepared(&envelope).unwrap();
    let expected=pd::encode_continuation(&pd::derive_continuation(&envelope,&response).unwrap()).unwrap();
    let conflict=tx_fixture(&raw,data.response,&expected,0xD801,None);
    conflict.transaction.validate().unwrap();
    let binding=NativeRequestBinding{model_case:None,identity_digest:conflict.identity,request_sha256:conflict.request_sha256,
        prepared:raw,kie_proof:vec![]};
    let wal=TempWal::new("iz-cross-route-namespace");
    {
        let vault=KnowledgeVault::open_disk(&wal.path).unwrap();
        assert_eq!(vault.commit_pd_operation(parent.request.clone()).unwrap(),parent);
    }
    // The production WAL holds an OS-exclusive lock. Inspect bytes only after
    // dropping its owner, and reopen for each independently verified phase.
    let before=std::fs::read(&wal.path).unwrap();
    {
        let vault=KnowledgeVault::open_disk(&wal.path).unwrap();
        for error in [vault.validate_native_pd_admission(&binding).unwrap_err(),
            vault.commit_test_transaction(conflict.transaction.clone()).unwrap_err()] {
            assert!(error.to_string().contains("PD_IZ_OPERATION_ALREADY_OPERATIONAL"));
        }
        assert_eq!(counts(&vault),(0,0));
        assert_eq!(vault.pd_head(&parent.request.identity).unwrap(),Some(parent.clone()));
    }
    assert_eq!(std::fs::read(&wal.path).unwrap(),before);
    // A distinct native operation still admits the fixture and exact retry
    // still returns its existing receipt before any new-operation guards.
    let legitimate=tx_fixture(data.prepared,data.response,data.continuation,0xD802,None);
    let commit={
        let vault=KnowledgeVault::open_disk(&wal.path).unwrap();
        vault.commit_test_transaction(legitimate.transaction.clone()).unwrap()
    };
    let committed_bytes=std::fs::read(&wal.path).unwrap();
    {
        let vault=KnowledgeVault::open_disk(&wal.path).unwrap();
        let retried=vault.commit_test_transaction(legitimate.transaction).unwrap();
        assert_eq!(retried.status,CommitStatus::AlreadyCommitted);
        assert_eq!(retried.transaction_id,commit.transaction_id);
        assert_eq!(retried.receipt,commit.receipt);
    }
    assert_eq!(std::fs::read(&wal.path).unwrap(),committed_bytes);
    // A low-level writer deliberately constructs the same checksum-valid attack.
    // Authoritative replay must reject it before any torn-tail repair/publication.
    let corrupt=TempWal::new("iz-cross-route-replay");
    {
        let vault=KnowledgeVault::open_disk(&corrupt.path).unwrap();
        vault.commit_pd_operation(parent.request).unwrap();
    }
    {
        let(backend,_)=DiskBackend::open(&corrupt.path).unwrap();
        backend.commit_test_transaction(&conflict.transaction).unwrap();
    }
    for torn in [false,true] {
        if torn {append_torn_header(&corrupt.path);}
        let before=std::fs::read(&corrupt.path).unwrap();
        let error=KnowledgeVault::open_disk(&corrupt.path).err().expect("semantic cross-route collision rejected");
        assert!(format!("{error:?}").contains("PD_IZ_OPERATION_ALREADY_OPERATIONAL"));
        assert_eq!(std::fs::read(&corrupt.path).unwrap(),before);
    }
}


#[test]
fn native_iz_operation_cannot_be_reused_by_an_anchored_operational_inquiry(){
    use shadow_contracts::pd_memory::{failure,iz};
    use sha2::{Digest,Sha256};
    // SYNTHETIC selection/receipt fixture, source-produced native PD checkpoints.
    // The valid native+1 child checkpoint is reused as data only: this tests the
    // storage boundary, not a newly admitted ASHT request or real P/N science.
    let data=material!("initial-positive",None);
    let native=tx_fixture(data.prepared,data.response,data.continuation,0xD803,None);
    let prepared=pd::decode_prepared(data.prepared).unwrap();
    let origin=prepared.source_iz.as_ref().unwrap();
    let child=pd::decode_prepared(data.child_prepared).unwrap();
    let child_meta=shadow_contracts::pd_checkpoint_data::checkpoint_metadata(
        &child.checkpoint,prepared.cognitive_session_id()).unwrap();
    let lookup=shadow_contracts::pd_recovery::RecoveryLookup{
        identity:native.identity,request_sha256:native.request_sha256};
    let wal=TempWal::new("native-to-operational-namespace");
    let bundle={
        let vault=KnowledgeVault::open_disk(&wal.path).unwrap();
        vault.commit_test_transaction(native.transaction.clone()).unwrap();
        vault.recover_native(&lookup).unwrap()
    };
    let native_bytes=std::fs::read(&wal.path).unwrap();
    let make_request=|operation_id:&str|{
        let mut context=origin.context.clone();
        context.version=iz::IZ_NATIVE_CONTINUITY_VERSION;
        context.operation_id=operation_id.into();
        context.user_text=child_meta.last_user_text.clone();
        context.answer=iz::IzAnswer::default();
        context.parent=None;context.map=None;
        let anchor=pd::PdContinuationLink::from_bundle(&bundle).unwrap();
        context.native_map=Some(iz::native_map_for(&context.identity,&anchor,None).unwrap());
        context.native_anchor=Some(anchor);
        context.validate().unwrap();
        // Rebuild every public causal hash around a synthetically empty response.
        // A malformed frame or stale history must not explain the rejection.
        let mut selection=shadow_contracts::decode_selection_request(&origin.passed.before.request).unwrap();
        selection.session_id=format!("synthetic-namespace-{operation_id}");
        selection.trace_id=selection.session_id.clone();
        selection.parent_i0=format!("{:x}",Sha256::digest(context.current_i0().as_bytes()));
        let selected=shadow_contracts::encode_selection_request(&selection);
        let mut response=shadow_contracts::decode_selection_response(&origin.passed.response).unwrap();
        response.session_id=selection.session_id.clone();
        response.parent_i0=selection.parent_i0.clone();
        response.request_sha256=shadow_contracts::sha256_hex(&[b"ESSMAI/SELECTION/REQUEST/V178",&selected]);
        response.positive.clear();response.negative.clear();
        response.selection_sha256=failure::relevance::selection_digest(&response);
        let attempt=iz::IzBefore::new(&context,selected).unwrap().activate().unwrap()
            .complete(shadow_contracts::encode_selection_response(&response),failure::MISSING_POSITIVE.into()).unwrap();
        let activation=iz::assembly_activation(&context,&attempt,&[]).unwrap();
        let source=iz::compile_exact_iz(context,attempt,vec![],activation).unwrap();
        let mut checkpoint=source.context.checkpoint_prefix().to_vec();
        checkpoint.extend_from_slice(&child.checkpoint);
        let request=memory::PdOperationRequest{
            identity:source.context.identity.clone(),operation_id:source.context.operation_id.clone(),
            source_input:iz::encode_source(&source).unwrap(),expected_revision:0,previous_state:[0;32],
            witness:iz::build_witness(&source).unwrap(),checkpoint,
            scoped_question:source.question_spec.question.clone(),candidate:vec![],candidate_key:[0;32],
            lgc_steps:iz::build_lgc_steps(&source).unwrap(),
        };
        request.validate().unwrap();
        iz::validate_native_transition(&request,&bundle,None).unwrap();
        request
    };
    let conflict=make_request(&origin.context.operation_id);
    {
        let vault=KnowledgeVault::open_disk(&wal.path).unwrap();
        assert_eq!(vault.pd_iz_native_lookup(&origin.context).unwrap(),Some(lookup.clone()));
        assert!(vault.pd_operation(&conflict.identity,&conflict.operation_id).unwrap().is_none());
        for error in [vault.validate_pd_native_inquiry(&conflict).unwrap_err(),
            vault.commit_pd_operation(conflict.clone()).unwrap_err()] {
            assert!(error.to_string().contains("PD_IZ_OPERATION_ALREADY_NATIVE"));
        }
        assert!(vault.pd_head(&conflict.identity).unwrap().is_none());
        assert_eq!(counts(&vault),(1,0));
        assert_eq!(vault.pd_iz_native_lookup(&origin.context).unwrap(),Some(lookup.clone()));
    }
    assert_eq!(std::fs::read(&wal.path).unwrap(),native_bytes);
    // Positive control differs only in logical operation ownership. It admits a
    // real native+1 checkpoint; its exact operational retry must remain no-append.
    let allowed=make_request("distinct-operational-inquiry");
    let committed={
        let vault=KnowledgeVault::open_disk(&wal.path).unwrap();
        vault.validate_pd_native_inquiry(&allowed).unwrap();
        vault.commit_pd_operation(allowed.clone()).unwrap()
    };
    let allowed_bytes=std::fs::read(&wal.path).unwrap();
    {
        let vault=KnowledgeVault::open_disk(&wal.path).unwrap();
        assert_eq!(vault.commit_pd_operation(allowed).unwrap(),committed);
        assert_eq!(vault.pd_iz_native_lookup(&origin.context).unwrap(),Some(lookup));
        assert_eq!(counts(&vault),(1,0));
    }
    assert_eq!(std::fs::read(&wal.path).unwrap(),allowed_bytes);
    // A checksum-valid record bypassing live admission must fail authoritative
    // replay before publication or tail repair. All file reads occur after Drop.
    let corrupt=TempWal::new("native-to-operational-namespace-replay");
    std::fs::write(&corrupt.path,&native_bytes).unwrap();
    let conflicting_record=memory::PdOperationRecord::construct(conflict,[0;32]).unwrap();
    {
        let(backend,_)=DiskBackend::open(&corrupt.path).unwrap();
        backend.commit_pd_operation(&conflicting_record).unwrap();
    }
    for torn in [false,true] {
        if torn {append_torn_header(&corrupt.path);}
        let before=std::fs::read(&corrupt.path).unwrap();
        let error=KnowledgeVault::open_disk(&corrupt.path).err().expect("native-owned operation cannot replay as operational");
        assert!(format!("{error:?}").contains("PD_IZ_OPERATION_ALREADY_NATIVE"));
        assert_eq!(std::fs::read(&corrupt.path).unwrap(),before);
    }
}
