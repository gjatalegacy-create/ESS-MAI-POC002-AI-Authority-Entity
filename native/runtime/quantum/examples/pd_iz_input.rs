//! Explicit test/user input codec and read-only inspection.
//! This helper does not seed candidates, holds, receipts or knowledge.
use sha2::{Digest, Sha256};
use shadow_contracts::pd_memory::{self as pd, iz, lgc};
use std::{
    fs::OpenOptions,
    io::{Read, Write},
};

fn read(path: &str, max: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(max as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|e| e.to_string())?;
    if out.len() > max {
        return Err("INPUT_LIMIT".into());
    }
    Ok(out)
}
fn text(path: &str) -> Result<String, String> {
    if path == "-" {
        return Ok(String::new());
    }
    String::from_utf8(read(path, pd::MAX_TEXT_BYTES)?).map_err(|e| e.to_string())
}
fn write(path: &str, bytes: &[u8]) -> Result<(), String> {
    let mut f = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    f.write_all(bytes)
        .and_then(|_| f.sync_all())
        .map_err(|e| e.to_string())
}
fn q(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => out.push_str(&format!("\\u{:04x}", c as u32)),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}
fn record(path: &str) -> Result<pd::PdOperationRecord, String> {
    pd::decode_record(&read(path, pd::MAX_REQUEST_BYTES + 512)?)
}
fn inspect_record(r: &pd::PdOperationRecord) -> Result<String, String> {
    r.validate()?;
    if pd::late_refusal::source_is_late(&r.request.source_input) {
        let source=pd::late_refusal::validate_operation(&r.request)?;
        let context=pd::late_refusal::original_context(&source)?;
        let key=pd::hex(&r.request.identity.key()?);
        let mut engine=quantum_lim::progressive_debatic::core::ProgressiveDebatic::new();
        engine.import_checkpoint(&key,pd::late_refusal::checkpoint_raw(&r.request)?)?;
        let state=engine.session(&key).ok_or("SESSION_MISSING")?;
        return Ok(object(vec![
            ("source_kind",q("PLF1")),("status",q("OperationalNoVerdict")),
            ("revision",r.revision.to_string()),("record_id",q(&pd::hex(&r.record_id))),
            ("previous_record",q(&pd::hex(&r.previous_record))),
            ("identity_key",q(&key)),("operation_id",q(&r.request.operation_id)),
            ("case_id",q(&r.request.identity.case_id)),("scope",q(&r.request.identity.scope)),
            ("actual_light_i0",q(context.current_i0())),
            ("condition",q(pd::late_refusal::CONDITION)),
            ("question",q(&pd::late_refusal::render_iz(r)?)),
            ("stored_descriptor",q(&r.request.scoped_question)),
            ("descriptor_sha256",q(&format!("{:x}",Sha256::digest(r.request.scoped_question.as_bytes())))),
            ("inquiry_wire_hex",q(&source.frontier.iter().map(|b|format!("{b:02x}")).collect::<String>())),
            ("session_turn_count",state.total_turns.to_string()),
            ("authority_transferred","false".into()),("knowledge_verdict","null".into()),("knowledge_write","0".into()),
            ("inspection_authority",q("Stored data inspection only; no owner authentication or verdict")),
        ]));
    }
    let s = iz::validate_operation(&r.request)?;
    let before_conditions = s.attempt.before.condition_measurement()?;
    let conditions = s.attempt.condition_measurement()?;
    let key = pd::hex(&r.request.identity.key()?);
    let mut engine = quantum_lim::progressive_debatic::core::ProgressiveDebatic::new();
    engine.import_checkpoint(
        &key,
        r.request
            .checkpoint
            .strip_prefix(s.context.checkpoint_prefix())
            .ok_or("IZ_CHECKPOINT_POLICY")?,
    )?;
    let state = engine.session(&key).ok_or("SESSION_MISSING")?;
    let events = [
        &s.attempt.before.origin,
        &s.attempt.before.pre_witness,
        &s.attempt.activation,
        &s.attempt.outcome,
        &s.assembly_activation,
        &s.assembly,
    ];
    let mut fields=vec![
 ("source_kind",q(match s.context.version {3=>"PIZ3",2=>"PIZ2",_=>"PIZ1"})),("context_version",s.context.version.to_string()),("revision",r.revision.to_string()),("record_id",q(&pd::hex(&r.record_id))),
 ("previous_record",q(&pd::hex(&r.previous_record))),("state_digest",q(&pd::hex(&r.state_digest))),
 ("identity_key",q(&key)),("operation_id",q(&s.context.operation_id)),("case_id",q(&s.context.identity.case_id)),
 ("scope",q(&s.context.identity.scope)),("obligation",q(&s.context.identity.obligation)),
 ("origin_i0",q(&s.context.origin_i0)),("actual_user_text",q(&s.context.user_text)),
  ("actual_light_i0",q(s.context.current_i0())),
  ("parent_question_sha256",s.context.parent.as_ref().map(|p|q(&pd::hex(&p.question_sha256))).unwrap_or("null".into())),
  ("answer_spans", if s.context.version>=2 {format!("[{}]",s.context.answer_spans()?.iter().map(|v|format!("{{\"field\":{},\"start_byte\":{},\"end_byte\":{},\"value_sha256\":{}}}",
    q(&format!("{:?}",v.field)),v.start,v.end,q(&format!("{:x}",Sha256::digest(s.context.answer.get(v.field).as_bytes()))))).collect::<Vec<_>>().join(","))} else {"null".into()}),
 ("question",q(&r.request.scoped_question)),("question_sha256",q(&pd::hex(&s.question_spec.question_sha256))),
 ("missing",q(&format!("{:?}",s.attempt.missing))),("hold",s.question_spec.hold.to_string()),
 ("status",q(&format!("{:?}",s.question_spec.status))),("authority_transferred","false".into()),
 ("knowledge_verdict","null".into()),("knowledge_write","0".into()),
 ("session_turn_count",state.total_turns.to_string()),("session_started_at",state.started_at.to_string()),
 ("session_last_activity_at",state.last_activity_at.to_string()),
 ("mapped_parent",(s.context.map.is_some() || s.context.native_map.is_some()).to_string()),
 ("native_anchor",s.context.native_anchor.as_ref().map(|a|q(&pd::hex(&a.predecessor.identity))).unwrap_or("null".into())),
 ("checkpoint_base",s.context.native_map.as_ref().map(|m|q(match &m.checkpoint_base {iz::IzCheckpointBase::NativeT2=>"NativeT2",iz::IzCheckpointBase::Operational(_)=>"Operational"})).unwrap_or("null".into())),
 ("native_attempt",q(&s.question_spec.attempt_id)),("hold_id",q(&s.question_spec.hold_id)),
 ("requested_fields",format!("[{}]",s.question_spec.requested_fields.iter().map(|v|q(&format!("{v:?}"))).collect::<Vec<_>>().join(","))),
 ("supplied_fields",format!("[{}]",s.question_spec.frontier.iter().map(|v|q(&format!("{:?}",v.field))).collect::<Vec<_>>().join(","))),
 ("frontier",format!("[{}]",s.question_spec.frontier.iter().map(|v|q(&format!("{v:?}"))).collect::<Vec<_>>().join(","))),
 ("events",format!("[{}]",events.iter().map(|e|format!("{{\"kind\":{},\"sequence\":{},\"authority\":{},\"attempt\":{},\"digest\":{}}}",
   q(&format!("{:?}",e.kind)),e.sequence,e.authority,q(&e.attempt_id),q(&pd::hex(&e.event_digest)))).collect::<Vec<_>>().join(","))),
 ("trace_user_texts",format!("[{}]",state.trace.signals.iter().map(|v|q(&v.raw_content)).collect::<Vec<_>>().join(","))),
 ];
    let (req, res) =
        pd::failure::decode_selection_pair(&s.attempt.before.request, &s.attempt.response)?;
    fields.push(("selection_positive_count", res.positive.len().to_string()));
    fields.push(("selection_negative_count", res.negative.len().to_string()));
    fields.push(("selection_negative_debug", q(&format!("{:?}",res.negative))));
    fields.push(("selection_xi",format!("[{}]",req.xi.iter().map(|v|q(v)).collect::<Vec<_>>().join(","))));
    fields.push(("selection_yi",format!("[{}]",req.yi.iter().map(|v|q(v)).collect::<Vec<_>>().join(","))));
    fields.push(("selection_parent_i0", q(&req.parent_i0)));
    // Derived from the exact persisted request/response and validated LGC events.
    // This measures condition coverage, not thesis truth or authority.
    fields.push(("dbab_before_status", q(&format!("{:?}", before_conditions.status()))));
    fields.push(("dbab_before_negative_condition", before_conditions.is_negative_condition_knowledge().to_string()));
    fields.push(("dbab_before_available_mask", before_conditions.available_mask().map(|v|v.to_string()).unwrap_or("null".into())));
    fields.push(("dbab_status", q(&format!("{:?}", conditions.status()))));
    fields.push(("dbab_required_mask", conditions.required_mask().to_string()));
    fields.push(("dbab_available_mask", conditions.available_mask().map(|v|v.to_string()).unwrap_or("null".into())));
    fields.push(("dbab_missing_mask", conditions.missing_mask().map(|v|v.to_string()).unwrap_or("null".into())));
    fields.push(("dbab_gap_count", conditions.gap_count().map(|v|v.to_string()).unwrap_or("null".into())));
    fields.push(("dbab_condition_mass_fixed", conditions.condition_mass_fixed().map(|v|v.to_string()).unwrap_or("null".into())));
    fields.push(("dbab_negative_condition_knowledge", conditions.is_negative_condition_knowledge().to_string()));
    fields.push(("dbab_semantic_counterproof", "false".into()));
    let tip = r
        .request
        .lgc_steps
        .last()
        .ok_or("LGC_INSPECT_STEPS_EMPTY")?;
    fields.push(("lgc_tip", q(&pd::hex(&tip.step_digest))));
    fields.push(("lgc_step_count", r.request.lgc_steps.len().to_string()));
    Ok(object(fields))
}
fn object(fields: Vec<(&str, String)>) -> String {
    format!(
        "{{{}}}",
        fields
            .into_iter()
            .map(|(k, v)| format!("{}:{}", q(k), v))
            .collect::<Vec<_>>()
            .join(",")
    )
}
fn inspect_science(path:&str)->Result<(),String>{
    let r=record(path)?;pd::linked_science::validate_science_record(&r)?;
    let input=pd::iz_science::validate_operation(&r.request)?;
    let candidate=science_domains::decode_candidate(&r.request.candidate).map_err(|e|e.to_string())?;
    let query=pd::linked_science::science_query_from_source(&r.request.source_input)?;
    let model=science_domains::kie::evaluate_kie(&query,&r.request.candidate).map_err(|e|e.to_string())?;
    let key=pd::hex(&r.request.identity.key()?);
    let mut engine=quantum_lim::progressive_debatic::core::ProgressiveDebatic::new();
    engine.import_checkpoint(&key,pd::linked_science::science_checkpoint_raw(&r)?)?;
    let state=engine.session(&key).ok_or("SCIENCE_INSPECT_SESSION")?;
    let parent=input.context.parent.as_ref().ok_or("SCIENCE_INSPECT_PARENT")?;
    println!("{}",object(vec![
        ("source_kind",q(std::str::from_utf8(&r.request.source_input[..4]).map_err(|e| e.to_string())?)),("status",q("OperationalNoVerdict")),
        ("record_id",q(&pd::hex(&r.record_id))),("state_digest",q(&pd::hex(&r.state_digest))),
        ("revision",r.revision.to_string()),("identity_key",q(&key)),
        ("previous_record",q(&pd::hex(&r.previous_record))),
        ("parent_question_sha256",q(&pd::hex(&parent.question_sha256))),
        ("prior_lgc_head",q(&pd::hex(&input.prior_lgc_head))),
        ("actual_user_text",q(&input.context.user_text)),
        ("session_turn_count",state.total_turns.to_string()),
        ("trace_user_texts",format!("[{}]",state.trace.signals.iter().map(|v|q(&v.raw_content)).collect::<Vec<_>>().join(","))),
        ("query_sha256",q(&pd::hex(&candidate.query_sha256))),
        ("canonical_proposition",q(&model.canonical_proposition)),
        ("kis_mass",model.kis_mass.to_string()),("kic_mass",model.kic_mass.to_string()),
        ("relation",model.relation.to_string()),("knowledge_write","0".into()),
        ("knowledge_verdict","null".into()),("authority_transferred","false".into()),
        ("scoped_question",q(&r.request.scoped_question)),
    ]));
    Ok(())
}
fn inspect(path: &str) -> Result<(), String> {
    println!("{}", inspect_record(&record(path)?)?);
    Ok(())
}

/// Public metadata only; validated references do not authorize an operation.
fn reference_json(value: &lgc::PdLgcRef) -> Result<String, String> {
    value.validate()?;
    Ok(object(vec![
        ("identity_key", q(&pd::hex(&value.identity_key))),
        ("record_id", q(&pd::hex(&value.record_id))),
        ("revision", value.revision.to_string()),
        ("state_digest", q(&pd::hex(&value.state_digest))),
        ("question_sha256", q(&pd::hex(&value.question_sha256))),
        ("lgc_tip", q(&pd::hex(&value.lgc_tip))),
        ("input_sha256", q(&pd::hex(&value.user_input_sha256))),
        ("question_binding", q(&pd::hex(&value.question_binding))),
    ]))
}

/// Data inspection mirrors pd_recovery_inspect.rs; this never issues authority.
fn native_json(bundle: &shadow_contracts::pd_recovery::RecoveryBundle) -> Result<String, String> {
    let (prepared, response, continuation) = bundle.validated()?;
    let continuation = continuation.ok_or("LGC_INSPECT_NATIVE_T2_REQUIRED")?;
    let cognitive = prepared.cognitive_session_id();
    let mut engine = quantum_lim::progressive_debatic::core::ProgressiveDebatic::new();
    engine.import_checkpoint(cognitive, &prepared.checkpoint)?;
    let session = engine
        .session(cognitive)
        .ok_or("LGC_INSPECT_NATIVE_SESSION")?;
    Ok(object(vec![
        ("kind",q("NativePdRecoveryInspection")),
        ("prepared_version",prepared.version.to_string()),
        ("cognitive_session_id",q(cognitive)),
        ("attempt_session_id",q(&prepared.pending.session_id)),
        ("parent_i0",q(&prepared.pending.parent_i0)),
        ("clock_millis",prepared.clock_is_millis().to_string()),
        ("session_turn_count",session.total_turns.to_string()),
        ("trace_user_texts",format!("[{}]",session.trace.signals.iter().map(|v|q(&v.raw_content)).collect::<Vec<_>>().join(","))),
        ("session_started_at",session.started_at.to_string()),
        ("session_last_activity_at",session.last_activity_at.to_string()),
        ("territory",q(&session.territory)),
        ("bundle_identity",q(&pd::hex(&bundle.identity))),
        ("request_sha256",q(&pd::hex(&bundle.request_sha256))),
        ("transaction_id",q(&pd::hex(&bundle.transaction_id))),
        ("prepared_sha256",q(&format!("{:x}",Sha256::digest(&bundle.prepared)))),
        ("checkpoint_sha256",q(&format!("{:x}",Sha256::digest(&prepared.checkpoint)))),
        ("shadow_response_sha256",q(&format!("{:x}",Sha256::digest(&bundle.response)))),
        ("continuation_sha256",q(&format!("{:x}",Sha256::digest(bundle.continuation.as_ref().ok_or("LGC_INSPECT_NATIVE_T2_REQUIRED")?)))),
        ("continuation_next_i0",q(&continuation.next_i0_id)),
        ("y_verdict",response.verdict.verified.to_string()),
        ("x_verdict",response.verdict.primitive.to_string()),
        ("accepted_knowledge",((response.verdict.verified,response.verdict.primitive)==(1,1)).to_string()),
        ("shadow_receipt_id",q(&response.verification_receipt.receipt_id)),
        ("pending_question",q(&prepared.pending.prepared_question)),
        ("pending_turn_id",q(&prepared.pending.turn_id)),
        ("current_input",session.trace.signals.last().map(|v|q(&v.raw_content)).unwrap_or_else(||"null".into())),
        ("inspection_authority",q("Validated stored-response data only; this file inspector does not authenticate storage origin or issue a verdict")),
    ]))
}
fn inspect_lgc_result(path: &str) -> Result<(), String> {
    let output = lgc::decode_output(&read(path, lgc::MAX_OUTPUT_BYTES)?)?;
    let fields = match output {
        lgc::PdLgcOutput::Operational(record) => {
            let reference = lgc::PdLgcRef::from_record(&record)?;
            vec![
                ("status", q("OperationalNoVerdict")),
                ("record", inspect_record(&record)?),
                ("reference", reference_json(&reference)?),
                ("native", "null".into()),
            ]
        }
        lgc::PdLgcOutput::Native(bundle) => vec![
            ("status", q("NativeTerminal")),
            ("record", "null".into()),
            ("reference", "null".into()),
            ("native", native_json(&bundle)?),
        ],
    };
    println!("{}", object(fields));
    Ok(())
}
fn export_lgc(path: &str, target: &str) -> Result<(), String> {
    let reference = lgc::PdLgcRef::from_record(&record(path)?)?;
    write(target, &lgc::encode_ref(&reference)?)
}
fn export_lgc_result(path: &str, target: &str) -> Result<(), String> {
    match lgc::decode_output(&read(path, lgc::MAX_OUTPUT_BYTES)?)? {
        lgc::PdLgcOutput::Operational(record) => write(
            target,
            &lgc::encode_ref(&lgc::PdLgcRef::from_record(&record)?)?,
        ),
        lgc::PdLgcOutput::Native(_) => {
            Err("LGC_EXPORT_NATIVE_REQUIRES_RECOVERY_LOOKUP_NOT_OPERATIONAL_REF".into())
        }
    }
}
/// Negative-test input only: preserve framing/checksum while changing metadata.
/// The resulting frame does not prove that a matching record exists.
fn changed_reference(
    original: &lgc::PdLgcRef,
    field: &str,
    value: &str,
) -> Result<lgc::PdLgcRef, String> {
    let mut changed = original.clone();
    match field {
        "record" => changed.record_id = pd::parse_hash(value)?,
        "state" => changed.state_digest = pd::parse_hash(value)?,
        "question" => changed.question_sha256 = pd::parse_hash(value)?,
        "tip" => changed.lgc_tip = pd::parse_hash(value)?,
        "input" => changed.user_input_sha256 = pd::parse_hash(value)?,
        "identity" => changed.identity_key = pd::parse_hash(value)?,
        "binding" => changed.question_binding = pd::parse_hash(value)?,
        "revision" => {
            changed.revision = value.parse::<u64>().map_err(|_| "LGC_MUTATION_REVISION")?
        }
        _ => return Err("LGC_MUTATION_KIND".into()),
    }
    changed.validate()?;
    if changed == *original {
        return Err("LGC_MUTATION_NO_CHANGE".into());
    }
    Ok(changed)
}
fn mutate_lgc_reference(input: &str, output: &str, field: &str, value: &str) -> Result<(), String> {
    let original = lgc::decode_ref(&read(input, lgc::REF_BYTES)?)?;
    let changed = changed_reference(&original, field, value)?;
    write(output, &lgc::encode_ref(&changed)?)
}
fn run(a: &[String]) -> Result<(), String> {
    match a.get(1).map(String::as_str){
  Some("initial" | "initial-v2" | "new-input-v2") if a.len()==7=>{
   let origin=text(&a[6])?;
   let c=iz::IzContext{
    version:if a[1]=="initial" {1} else {2},
    identity:pd::PdIdentity{owner:"native-light-runtime".into(),obligation:"ASHT exact positive-negative distinction".into(),
      case_id:a[4].clone(),scope:a[5].clone(),domain:a[5].clone(),lineage_root:format!("{:x}",Sha256::digest(origin.as_bytes()))},
    operation_id:a[3].clone(),origin_i0:origin.clone(),user_text:origin,parent:None,answer:iz::IzAnswer::default(),map:None,native_anchor:None,native_map:None};
   write(&a[2],&iz::encode_context(&c)?)
  }
  Some("answer" | "answer-v2") if a.len()==11=>{
   let parent=record(&a[4])?;let prior=iz::decode_source(&parent.request.source_input)?;
   let c=iz::IzContext{version:if a[1]=="answer-v2" {2} else {1},identity:prior.context.identity,operation_id:a[3].clone(),origin_i0:prior.context.origin_i0,
    user_text:text(&a[5])?,parent:Some(iz::IzParent::from_record(&parent)?),
    answer:iz::IzAnswer{positive:text(&a[6])?,negative:text(&a[7])?,distinction:text(&a[8])?,
      source_reference:text(&a[9])?,verification:text(&a[10])?},map:None,native_anchor:None,native_map:None};
   write(&a[2],&iz::encode_context(&c)?)
  }
  Some("native-inquiry") if a.len()==7=>{
   // Only construct intent from explicitly supplied historical data. Production
   // Light/BESA/Shadow independently resolve BOTH references from actual Vault.
   let parent=record(&a[4])?;
   let prior=iz::context_from_record(&parent)?;
   let bundle=shadow_contracts::pd_recovery::decode_bundle(&read(&a[5],shadow_contracts::pd_recovery::MAX_BUNDLE_BYTES)?)?;
   let anchor=shadow_contracts::pd_prepared::PdContinuationLink::from_bundle(&bundle)?;
   let context=iz::IzContext{version:iz::IZ_NATIVE_CONTINUITY_VERSION,
    identity:prior.identity,operation_id:a[3].clone(),origin_i0:prior.origin_i0,
    user_text:text(&a[6])?,parent:Some(iz::IzParent::from_record(&parent)?),
    answer:iz::IzAnswer::default(),map:None,native_anchor:Some(anchor),native_map:None};
   write(&a[2],&iz::encode_context(&context)?)
  }
  Some("extract-operational") if a.len()==4=>{
   let output=lgc::decode_output(&read(&a[2],lgc::MAX_OUTPUT_BYTES)?)?;
   let lgc::PdLgcOutput::Operational(value)=output else {return Err("OPERATIONAL_REQUIRED".into())};
   write(&a[3],&pd::encode_record(&value)?)
  }
  Some("inspect") if a.len()==3=>inspect(&a[2]),
  Some("inspect-science") if a.len()==3=>inspect_science(&a[2]),
  Some("export-lgc") if a.len()==4=>export_lgc(&a[2],&a[3]),
  Some("export-lgc-result") if a.len()==4=>export_lgc_result(&a[2],&a[3]),
  Some("export-native-lookup") if a.len()==4=>{
   let output=lgc::decode_output(&read(&a[2],lgc::MAX_OUTPUT_BYTES)?)?;
   let lgc::PdLgcOutput::Native(bundle)=output else {
    return Err("LGC_EXPORT_OPERATIONAL_HAS_NO_NATIVE_AUTHORITY".into());
   };
   let lookup=shadow_contracts::pd_recovery::RecoveryLookup {
    identity:bundle.identity,request_sha256:bundle.request_sha256
   };
   bundle.validate_for(&lookup)?;
   write(&a[3],&shadow_contracts::pd_recovery::encode_lookup(&lookup)?)
  }

  Some("inspect-lgc-result") if a.len()==3=>inspect_lgc_result(&a[2]),
  Some("mutate-lgc-reference") if a.len()==6=>mutate_lgc_reference(&a[2],&a[3],&a[4],&a[5]),
  Some("mutate-context") if a.len()==6=>{
   let mut c=iz::decode_context(&read(&a[2],iz::MAX_CONTEXT_BYTES)?)?;
   match a[4].as_str(){
    "case"=>c.identity.case_id=a[5].clone(),
    "user"=>c.user_text=text(&a[5])?,
    "native-t2"=>{c.native_anchor.as_mut().ok_or("NATIVE_ANCHOR_REQUIRED")?.predecessor_continuation_sha256=[17;32];},
    "scope"=>{c.identity.scope=a[5].clone();c.identity.domain=a[5].clone();},
    "operation"=>c.operation_id=a[5].clone(),
    "question"=>{c.parent.as_mut().ok_or("PARENT_REQUIRED")?.question_sha256=[17;32];},
    _=>return Err("MUTATION_KIND".into())}
   write(&a[3],&iz::encode_context(&c)?)
  }
  Some("export-request") if a.len()==4=>write(&a[3],&pd::encode_request(&record(&a[2])?.request)?),
  _=>Err("native-inquiry OUTPUT OP PARENT_RECORD NATIVE_BUNDLE ACTUAL_INPUT_FILE | extract-operational LGC_RESULT OUTPUT | initial|initial-v2|new-input-v2 OUTPUT OP CASE SCOPE ACTUAL_INPUT_FILE | answer|answer-v2 OUTPUT OP PARENT ACTUAL_INPUT POS NEG DIST SOURCE VERIFY (use - for absent fields; v2 annotations must be exact unique input spans) | inspect RECORD | mutate-context INPUT OUTPUT case|scope|operation|question VALUE | export-request RECORD OUTPUT | export-lgc RECORD REF | export-lgc-result RESULT REF (Operational only) | export-native-lookup RESULT LOOKUP (Native only, existing --pd-resume-native route) | inspect-lgc-result RESULT | mutate-lgc-reference REF OUTPUT record|state|question|tip|input|identity|binding|revision VALUE".into())
 }
}
fn main() {
    if let Err(e) = run(&std::env::args().collect::<Vec<_>>()) {
        eprintln!("{e}");
        std::process::exit(70);
    }
}

#[cfg(test)]
mod lgc_helper_tests {
    use super::*;
    fn reference() -> lgc::PdLgcRef {
        // Shape-only fixture. No actual record, authority or admitted knowledge.
        lgc::PdLgcRef {
            identity_key: [1; 32],
            record_id: [2; 32],
            revision: 3,
            state_digest: [4; 32],
            question_sha256: [5; 32],
            lgc_tip: [6; 32],
            user_input_sha256: [7; 32],
            question_binding: [8; 32],
        }
    }
    #[test]
    fn mutation_recomputes_valid_codec_without_claiming_record_existence() {
        let original = reference();
        for field in [
            "record", "state", "question", "tip", "input", "identity", "binding",
        ] {
            let changed = changed_reference(&original, field, &"11".repeat(32)).unwrap();
            assert_ne!(changed, original);
            assert_eq!(
                lgc::decode_ref(&lgc::encode_ref(&changed).unwrap()).unwrap(),
                changed
            );
        }
        let changed = changed_reference(&original, "revision", "4").unwrap();
        assert_eq!(changed.revision, 4);
        assert_eq!(original, reference());
    }
    #[test]
    fn invalid_or_noop_mutations_cannot_produce_a_reference() {
        let value = reference();
        for (field, text) in [
            ("input", "no-hash"),
            ("record", &"00".repeat(32)),
            ("revision", "0"),
            ("revision", "-1"),
            ("unknown", "value"),
            ("revision", "3"),
        ] {
            assert!(changed_reference(&value, field, text).is_err());
        }
        assert!(changed_reference(&value, "input", &pd::hex(&value.user_input_sha256)).is_err());
    }
}
