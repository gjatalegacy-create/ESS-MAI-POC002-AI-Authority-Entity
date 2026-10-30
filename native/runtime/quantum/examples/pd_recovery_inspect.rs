//! Read-only inspection of an actual Shadow recovery bundle.
//! Data checks do not authenticate a caller-supplied file or issue authority.
use sha2::{Digest,Sha256};
use shadow_contracts::{pd_memory::hex,pd_recovery};
use std::{io::Read,path::Path};
fn json_string(value:&str)->String {
    let mut out=String::from("\"");
    for c in value.chars() {match c {
        '"'=>out.push_str("\\\""),'\\'=>out.push_str("\\\\"),
        '\n'=>out.push_str("\\n"),'\r'=>out.push_str("\\r"),'\t'=>out.push_str("\\t"),
        c if c<' '=>out.push_str(&format!("\\u{:04x}",u32::from(c))),c=>out.push(c),
    }}out.push('"');out
}
fn inspect(path:&Path)->Result<String,String> {
    let mut bytes=Vec::new();
    std::fs::File::open(path).map_err(|e|e.to_string())?
        .take((pd_recovery::MAX_BUNDLE_BYTES+1) as u64).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
    if bytes.len()>pd_recovery::MAX_BUNDLE_BYTES {return Err("RECOVERY_INSPECT_LIMIT".into());}
    let bundle=pd_recovery::decode_bundle(&bytes)?;
    let (prepared,response,continuation)=bundle.validated()?;
    let cognitive=prepared.cognitive_session_id();
    let mut pd=quantum_lim::progressive_debatic::core::ProgressiveDebatic::new();
    pd.import_checkpoint(cognitive,&prepared.checkpoint)?;
    let session=pd.session(cognitive).ok_or("RECOVERY_INSPECT_SESSION")?;
    let q=json_string;
    let optional_hash=|value:Option<&[u8;32]>|value.map(|v|q(&hex(v))).unwrap_or_else(||"null".into());
    let fields=vec![
        ("kind",q("NativePdRecoveryInspection")),
        ("prepared_version",prepared.version.to_string()),
        ("cognitive_session_id",q(cognitive)),
        ("attempt_session_id",q(&prepared.pending.session_id)),
        ("parent_i0",q(&prepared.pending.parent_i0)),
        ("clock_millis",prepared.clock_is_millis().to_string()),
        ("session_turn_count",session.total_turns.to_string()),
        ("trace_user_texts",format!("[{}]",session.trace.signals.iter().map(|s|q(&s.raw_content)).collect::<Vec<_>>().join(","))),
        ("session_started_at",session.started_at.to_string()),
        ("session_last_activity_at",session.last_activity_at.to_string()),
        ("territory",q(&session.territory)),
        ("source_dialogue_record_id",optional_hash(prepared.source_dialogue.as_ref().map(|v|&v.context.record_id))),
        ("source_dialogue_operation_id",prepared.source_dialogue.as_ref().map(|v|q(&v.context.operation_id)).unwrap_or_else(||"null".into())),
        ("predecessor_identity",optional_hash(prepared.continuity.as_ref().map(|v|&v.predecessor.identity))),
        ("predecessor_request_sha256",optional_hash(prepared.continuity.as_ref().map(|v|&v.predecessor.request_sha256))),
        ("bundle_identity",q(&hex(&bundle.identity))),
        ("request_sha256",q(&hex(&bundle.request_sha256))),
        ("transaction_id",q(&hex(&bundle.transaction_id))),
        ("prepared_sha256",q(&format!("{:x}",Sha256::digest(&bundle.prepared)))),
        ("checkpoint_sha256",q(&format!("{:x}",Sha256::digest(&prepared.checkpoint)))),
        ("shadow_response_sha256",q(&format!("{:x}",Sha256::digest(&bundle.response)))),
        ("continuation_sha256",bundle.continuation.as_ref().map(|v|q(&format!("{:x}",Sha256::digest(v)))).unwrap_or_else(||"null".into())),
        ("continuation_next_i0",continuation.as_ref().map(|v|q(&v.next_i0_id)).unwrap_or_else(||"null".into())),
        ("y_verdict",response.verdict.verified.to_string()),
        ("x_verdict",response.verdict.primitive.to_string()),
        ("accepted",((response.verdict.verified,response.verdict.primitive)==(1,1)).to_string()),
        ("shadow_receipt_id",q(&response.verification_receipt.receipt_id)),
        ("failure_witness_lgc",optional_hash(prepared.failure_witness.as_ref().map(|w|&w.lgc_tip))),
        ("failure_witness_version",match prepared.failure_witness.as_ref() {
            Some(w)=>{let b=w.encode()?;u16::from_le_bytes([b[8],b[9]]).to_string()},None=>"null".into()}),
        ("inquiry_wire_hex",match prepared.failure_witness.as_ref().and_then(|w|w.inquiry.as_ref()) {
            Some(frontier)=>q(&frontier.encode()?.iter().map(|b|format!("{b:02x}")).collect::<String>()),None=>"null".into()}),
        ("failure_witness_request",optional_hash(prepared.failure_witness.as_ref().map(|w|&w.request_sha256))),
        ("failure_witness_cause",prepared.failure_witness.as_ref().and_then(|w|w.refusal.as_ref()).map(|v|q(v)).unwrap_or_else(||"null".into())),
        ("failure_witness_model_relation",prepared.failure_witness.as_ref().and_then(|w|w.model.as_ref()).map(|v|v.relation.to_string()).unwrap_or_else(||"null".into())),
        ("failure_witness_history_assessment",optional_hash(prepared.failure_witness.as_ref().and_then(|w|w.model.as_ref()).and_then(|m|m.history_assessment_sha256.as_ref()))),
        ("failure_witness_history_unresolved",prepared.failure_witness.as_ref().and_then(|w|w.model.as_ref()).map(|m|m.unresolved_history_count.to_string()).unwrap_or_else(||"null".into())),
        ("failure_witness_historical_veto",prepared.failure_witness.as_ref().and_then(|w|w.model.as_ref()).map(|m|m.historical_veto.to_string()).unwrap_or_else(||"null".into())),
        ("continuation_question",continuation.as_ref().map(|v|q(&v.question)).unwrap_or_else(||"null".into())),
        ("pending_question",q(&prepared.pending.prepared_question)),
        ("pending_turn_id",q(&prepared.pending.turn_id)),
        ("current_input",session.trace.signals.last().map(|v|q(&v.raw_content)).unwrap_or_else(||"null".into())),
        ("inspection_authority",q("Stored response data; this file inspector does not authenticate storage origin or issue a verdict")),
    ];
    Ok(format!("{{{}}}",fields.into_iter().map(|(k,v)|format!("{}:{}",q(k),v)).collect::<Vec<_>>().join(",")))
}
fn main(){
    let args=std::env::args().collect::<Vec<_>>();
    let result=if args.len()==2 {inspect(Path::new(&args[1]))} else {Err("usage: pd_recovery_inspect BUNDLE".into())};
    match result {Ok(value)=>println!("{value}"),Err(reason)=>{eprintln!("{reason}");std::process::exit(2);}}
}
