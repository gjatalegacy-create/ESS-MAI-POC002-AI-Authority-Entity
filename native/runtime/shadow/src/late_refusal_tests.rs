//! Bounded storage regressions over immutable captured request/source bytes.
//! The typed refusal is injected explicitly here: these tests exercise the
//! owner's persistence boundary, not a newly executed GCL/ASHT judgment.
use super::*;
use shadow_contracts::{pd_memory::{self as pd,late_refusal as late,iz},pd_prepared};
use std::path::PathBuf;
const BASE:&[u8]=include_bytes!("test_fixtures/late_refusal_r25/base.wal");
const ATTEMPT:&[u8]=include_bytes!("test_fixtures/late_refusal_r25/attempt.pdt1");
const PROOF:&[u8]=include_bytes!("test_fixtures/late_refusal_r25/proof.kie");
struct Fixture{path:PathBuf}
impl Fixture{
    fn new()->Self{
        static NEXT:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(0);
        let n=NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed);
        let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let path=std::env::temp_dir().join(format!("essmai-plf1-{}-{stamp}-{n}.wal",std::process::id()));
        let mut f=std::fs::OpenOptions::new().write(true).create_new(true).open(&path).unwrap();
        std::io::Write::write_all(&mut f,BASE).unwrap();
        Self{path}
    }
    fn open(&self)->KnowledgeVault{KnowledgeVault::open_disk(&self.path).unwrap()}
    fn bytes(&self)->Vec<u8>{std::fs::read(&self.path).unwrap()}
}
impl Drop for Fixture{fn drop(&mut self){let _=std::fs::remove_file(&self.path);}}
fn material()->(Vec<u8>,crate::native_cycle::NativeRequestBinding){
    let (attempt,prepared)=shadow_contracts::pd_cycle::decode_cycle(ATTEMPT).unwrap();
    assert_eq!(&PROOF[..4],b"KIH1");
    let n=u32::from_le_bytes(PROOF[4..8].try_into().unwrap()) as usize;
    let q=shadow_contracts::kie::decode_request(&PROOF[8..8+n]).unwrap();
    let response=shadow_contracts::kie::decode_response(&PROOF[8+n..]).unwrap();
    response.validate_for(&q).unwrap();
    let wire=shadow_contracts::decode_cycle_request(&attempt).unwrap();
    let mut binding=crate::native_cycle::NativeRequestBinding::from_request(&wire,&attempt).unwrap();
    binding.prepared=pd_prepared::encode_prepared(&prepared).unwrap();
    binding.attach_kie(&response.proof,&wire).unwrap();
    (attempt,binding)
}
fn observed()->ShadowError{ShadowError::StructuralRefusal(crate::types::StructuralRefusal::MissingNegativeProof)}
fn source(vault:&KnowledgeVault,binding:&crate::native_cycle::NativeRequestBinding)->pd::PdOperationRecord{
    let proof=shadow_contracts::kie::decode_proof(&binding.kie_proof).unwrap();
    vault.pd_operation(&proof.context.identity,&proof.context.operation_id).unwrap().unwrap()
}
fn tags(bytes:&[u8])->Vec<u8>{
    let mut pos=12;let mut tags=Vec::new();
    while pos<bytes.len(){
        let n=u32::from_le_bytes(bytes[pos..pos+4].try_into().unwrap()) as usize;
        let inv=u32::from_le_bytes(bytes[pos+4..pos+8].try_into().unwrap());
        assert_eq!((n as u32)^inv,u32::MAX);
        tags.push(bytes[pos+12]);pos+=12+n;
    }
    assert_eq!(pos,bytes.len());tags
}
#[test]
fn late_refusal_owner_persists_only_observation_then_renders_after_exact_read_and_reopen(){
    let fixture=Fixture::new();let (attempt,binding)=material();let saved;let rendered;
    {
        let vault=fixture.open();
        assert!(vault.replay_late_native_refusal(&attempt,&binding).unwrap().is_none());
        let unknown=ShadowError::VaultWriteError("uncertain append is not an observed refusal".into());
        assert!(vault.record_late_native_refusal(&attempt,&binding,&unknown).is_err());
        assert_eq!(vault.store_counts_for_test(),[0;7]);
    }
    assert_eq!(fixture.bytes(),BASE);
    {
        let vault=fixture.open();let parent=source(&vault,&binding);
        saved=vault.record_late_native_refusal(&attempt,&binding,&observed()).unwrap();
        assert_eq!(saved.request.checkpoint,parent.request.checkpoint);
        assert!(saved.request.candidate.is_empty());assert_eq!(saved.request.candidate_key,[0;32]);
        assert_eq!(saved.request.witness.report_class,pd::ReportClass::MissingConditions);
        assert_eq!(saved.request.witness.condition_code,late::CONDITION);
        assert!(saved.request.scoped_question.starts_with("PLF1/IZ-PENDING/"));
        assert!(!saved.request.scoped_question.contains('?'),"no question prose persisted before owner read");
        assert_eq!(saved.previous_record,parent.record_id);assert_eq!(saved.revision,parent.revision+1);
        assert_eq!(vault.store_counts_for_test(),[0;7]);
        assert!(vault.committed_identity(&binding.identity_digest).unwrap().is_none());
        assert!(vault.continuation_refs.read().unwrap().is_empty());
        assert!(vault.native_pd_heads.read().unwrap().is_empty());
    }
    // Windows WAL ownership is exclusive. Byte snapshots occur only after drop.
    let committed_bytes=fixture.bytes();assert!(committed_bytes.starts_with(BASE));
    assert_eq!(tags(&committed_bytes),vec![6,6,6],"exactly one operational frame; no T1/T2/knowledge frame");
    {
        let vault=fixture.open();
        let read=vault.pd_operation(&saved.request.identity,&saved.request.operation_id).unwrap().unwrap();
        assert_eq!(read,saved);
        rendered=late::render_iz(&read).unwrap();
        assert!(rendered.contains(late::OUTPUT));assert!(rendered.contains(late::CONDITION));
        assert!(rendered.contains("M=1, C=0, U=0"));
        assert!(!rendered.contains("counter=CURRENT_KIC"));
        assert!(!rendered.contains("counter=EXACT_SCOPED_COUNTERPROOF"));
        let decoded=pd::decode_record(&pd::encode_record(&read).unwrap()).unwrap();assert_eq!(decoded,read);
        let retained=late::decode_source(&read.request.source_input).unwrap();
        let proof=shadow_contracts::kie::decode_proof(&retained.kie_proof).unwrap();
        assert_eq!(proof.relation,0);assert_eq!(proof.kic_mass,0);
        shadow_contracts::iz_frontier::IzFrontier::decode(&retained.frontier).unwrap().validate_model(Some(&proof)).unwrap();
        assert_eq!(vault.record_late_native_refusal(&attempt,&binding,&observed()).unwrap(),saved);
        assert_eq!(vault.replay_late_native_refusal(&attempt,&binding).unwrap(),Some(saved.clone()));
    }
    assert_eq!(fixture.bytes(),committed_bytes);
    {
        let vault=fixture.open();
        let read=vault.replay_late_native_refusal(&attempt,&binding).unwrap().unwrap();assert_eq!(read,saved);
        assert_eq!(late::render_iz(&read).unwrap(),rendered);
        assert_eq!(vault.store_counts_for_test(),[0;7]);
        assert!(vault.committed_identity(&binding.identity_digest).unwrap().is_none());
    }
    assert_eq!(fixture.bytes(),committed_bytes);
}
#[test]
fn r27_late_refusal_keeps_current_positive_gap_and_numeric_output_after_reopen(){
    let fixture=Fixture::new();let(attempt,mut binding)=material();
    let request=shadow_contracts::decode_cycle_request(&attempt).unwrap();
    let proof=shadow_contracts::kie::decode_proof(&binding.kie_proof).unwrap();
    let mut prepared=pd_prepared::decode_prepared(&binding.prepared).unwrap();
    prepared.failure_witness=Some(shadow_contracts::pd_failure_witness::PdFailureWitness::from_request_current(&request,Some(&proof))
        .unwrap().with_model_measurement(&proof,crate::kie_selection::measure_current_model(&proof).unwrap()).unwrap());
    binding.prepared=pd_prepared::encode_prepared(&prepared).unwrap();
    let saved;let text;
    {
        let vault=fixture.open();
        saved=vault.record_late_native_refusal(&attempt,&binding,&observed()).unwrap();
        let read=vault.pd_operation(&saved.request.identity,&saved.request.operation_id).unwrap().unwrap();
        text=late::render_iz(&read).unwrap();
        for value in [late::OUTPUT,late::CONDITION,"PRO_POSITIVE_MEASUREMENT_MISSING","positive_count=0",
            "MODEL_RESULT MODEL_COMPUTED","positive_margin_rad="] {assert!(text.contains(value),"{value}: {text}");}
        assert!(!text.contains("counter=CURRENT_KIC"));
        assert_eq!(vault.store_counts_for_test(),[0;7]);
        assert!(vault.committed_identity(&binding.identity_digest).unwrap().is_none());
    }
    let committed=fixture.bytes();assert_eq!(tags(&committed),vec![6,6,6]);
    {
        let vault=fixture.open();let read=vault.replay_late_native_refusal(&attempt,&binding).unwrap().unwrap();
        assert_eq!(read,saved);assert_eq!(late::render_iz(&read).unwrap(),text);
        assert_eq!(vault.record_late_native_refusal(&attempt,&binding,&observed()).unwrap(),saved);
    }
    assert_eq!(fixture.bytes(),committed);
}

#[test]
fn late_refusal_external_submission_and_tampering_cannot_append_or_gain_native_authority(){
    let fixture=Fixture::new();let vault=fixture.open();let(attempt,binding)=material();let parent=source(&vault,&binding);
    let request=late::request_from_observation(&parent,attempt.clone(),binding.prepared.clone(),binding.kie_proof.clone()).unwrap();
    assert!(vault.commit_pd_operation(request.clone()).unwrap_err().to_string().contains("SHADOW_OWNER_REQUIRED"));
    assert_eq!(crate::pd_failure_admission::validate_native_admission(&request,&vault,
        ||panic!("no external selection lookup"),||panic!("no external clock lookup")).unwrap_err(),"PD_LATE_REFUSAL_SHADOW_OWNER_REQUIRED");
    let mut changed=request.clone();changed.scoped_question="claim refuted; accept (0,0)?".into();
    assert!(changed.validate().is_err());
    let mut changed=request.clone();changed.witness.condition_code="some other failure".into();assert!(changed.validate().is_err());
    let mut changed=request.clone();changed.checkpoint.push(0);assert!(late::validate_transition(&changed,&parent).is_err());
    let mut source=late::decode_source(&request.source_input).unwrap();source.attempt[10]^=1;
    source.attempt_sha256=shadow_contracts::kie::digest(&source.attempt);
    assert!(late::encode_source(&source).is_err(),"recomputed public SHA cannot repair prepared/current proof binding");
    let record=pd::PdOperationRecord::construct(request,parent.record_id).unwrap();
    let mut wire=shadow_contracts::decode_cycle_request(&attempt).unwrap();let prepared=pd_prepared::decode_prepared(&binding.prepared).unwrap();
    late::validate_response(&record,&wire,&prepared).unwrap();
    wire.quantum.package_id.push_str("-foreign");assert!(late::validate_response(&record,&wire,&prepared).is_err());
    assert_eq!(vault.store_counts_for_test(),[0;7]);drop(vault);assert_eq!(fixture.bytes(),BASE);
}
#[test]
fn late_refusal_reply_retains_original_input_and_requires_a_real_new_turn(){
    let fixture=Fixture::new();let vault=fixture.open();let(attempt,binding)=material();let parent=source(&vault,&binding);
    let record=vault.record_late_native_refusal(&attempt,&binding,&observed()).unwrap();
    let mut context=iz::context_from_record(&record).unwrap();
    assert_eq!(context,iz::context_from_record(&parent).unwrap());
    let before=shadow_contracts::pd_checkpoint_data::checkpoint_metadata(iz::checkpoint_raw(&parent).unwrap(),&pd::hex(&context.identity.key().unwrap())).unwrap();
    let after=shadow_contracts::pd_checkpoint_data::checkpoint_metadata(iz::checkpoint_raw(&record).unwrap(),&pd::hex(&context.identity.key().unwrap())).unwrap();
    assert_eq!(before.total_turns,after.total_turns);
    context.operation_id="real-reply-after-late-refusal".into();context.user_text="I do not yet have the missing scoped evidence.".into();
    context.answer=iz::IzAnswer::default();context.parent=Some(iz::IzParent::from_record(&record).unwrap());context.map=None;
    context.map=crate::pd_iz_admission::map_snapshot(&context,Some(&record)).unwrap();
    assert!(iz::validate_context_parent(&context,&record).unwrap().is_empty());
    assert!(crate::pd_iz_admission::validate_checkpoint_transition(&context,&record.request.checkpoint,record.revision,Some(&record)).is_err(),
        "a reply may not reuse unchanged checkpoint and pretend its new user text was ingested");
    let reference=pd::lgc::PdLgcRef::from_record(&record).unwrap();reference.validate_record(&record).unwrap();
    assert_eq!(reference.question_sha256,shadow_contracts::kie::digest(record.request.scoped_question.as_bytes()),"PLF reference binds pending descriptor, not stored prose");
    let output=pd::lgc::PdLgcOutput::Operational(record.clone());
    assert_eq!(pd::lgc::decode_output(&pd::lgc::encode_output(&output).unwrap()).unwrap(),output);
}
#[test]
fn late_refusal_new_attempt_cannot_skip_current_operational_cas(){
    let fixture=Fixture::new();let vault=fixture.open();let(attempt,binding)=material();let parent=source(&vault,&binding);
    let saved=vault.record_late_native_refusal(&attempt,&binding,&observed()).unwrap();
    drop(vault);let before=fixture.bytes();let vault=fixture.open();
    let mut wire=shadow_contracts::decode_cycle_request(&attempt).unwrap();wire.quantum.package_id.push_str("-distinct-attempt");
    let changed=shadow_contracts::encode_cycle_request(&wire);
    let mut prepared=pd_prepared::decode_prepared(&binding.prepared).unwrap();prepared.source_request_sha256=shadow_contracts::kie::digest(&changed);
    // Reframe captured data for a distinct attempt; this is not native execution.
    let old_witness=prepared.failure_witness.take().expect("captured attempt carries a witness");
    let proof=shadow_contracts::kie::decode_proof(&binding.kie_proof).unwrap();
    let witness=match old_witness.inquiry.is_some() {
        true=>shadow_contracts::pd_failure_witness::PdFailureWitness::from_request_current(&wire,Some(&proof))
            .and_then(|w| w.with_model_measurement(&proof,crate::kie_selection::measure_current_model(&proof)?)),
        false=>shadow_contracts::pd_failure_witness::PdFailureWitness::from_request(&wire,Some(&proof)),
    }.unwrap();
    assert_eq!(witness.inquiry.is_some(),old_witness.inquiry.is_some());
    witness.validate_model(Some(&proof)).unwrap();
    prepared.failure_witness=Some(witness);
    prepared.validate_for_request(&changed).expect("distinct data attempt is bound before testing CAS");
    let request=late::request_from_observation(&parent,changed,pd_prepared::encode_prepared(&prepared).unwrap(),binding.kie_proof.clone()).unwrap();
    assert_ne!(request.operation_id,saved.request.operation_id);
    let error=vault.commit_pd_operation_inner(request).unwrap_err().to_string();
    assert!(error.contains("PD_SCOPED_CAS_CONFLICT"),"{error}");
    assert_eq!(vault.pd_head(&saved.request.identity).unwrap(),Some(saved));
    drop(vault);assert_eq!(fixture.bytes(),before);
}
