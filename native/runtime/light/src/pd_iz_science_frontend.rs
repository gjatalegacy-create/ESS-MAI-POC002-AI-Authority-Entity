//! R5: actual user science answer to an exact stored iZ/LGC question.
//! Light transmits explicit parameters. Only Quantum evaluates; Shadow owns storage.
use crate::pd_operational_bridge::{self as bridge,NativePeer};
use shadow_contracts::pd_memory::{self as pd,iz,iz_science,lgc};
use std::path::Path;
const LIMIT:usize=48*1024;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Answer {
    user_text:String,
    #[serde(default)]
    answer:crate::pd_iz_frontend::LiteralAnswer,
    scientific_query:crate::pd_failure_frontend::EraQuery,
}
fn input(parent:&pd::PdOperationRecord,operation:&str,bytes:&[u8])
    ->Result<iz_science::IzScienceInput,String>{
    if bytes.len()>LIMIT{return Err("LIGHT_IZ_SCIENCE_INPUT_LIMIT".into());}
    let supplied:Answer=serde_json::from_slice(bytes).map_err(|e|format!("LIGHT_IZ_SCIENCE_JSON:{e}"))?;
    input_value(parent,operation,supplied)
}
fn input_value(parent:&pd::PdOperationRecord,operation:&str,supplied:Answer)
    ->Result<iz_science::IzScienceInput,String>{
    let user=crate::light_coordinator::LightRequest::try_new(supplied.user_text.as_str())
        .map_err(|e|format!("LIGHT_IZ_SCIENCE_USER:{e}"))?;
    if user.text()!=supplied.user_text{return Err("LIGHT_IZ_SCIENCE_USER_NOT_EXACT".into());}
    let prior=iz::context_from_record(parent)?;
    let parent_ref=iz::IzParent::from_record(parent)?;
    let mapped=iz::IzMapSnapshot{identity_key:parent.request.identity.key()?,
        head:parent_ref.clone(),exact_record:parent_ref.clone(),
        status:iz::IzMaterialStatus::OperationalNoVerdict};
    let native = prior.version == iz::IZ_NATIVE_CONTINUITY_VERSION;
    let context=iz::IzContext{version:prior.version.max(iz::IZ_ACTUAL_INPUT_VERSION),
        identity:prior.identity,operation_id:operation.into(),
        origin_i0:prior.origin_i0,user_text:supplied.user_text,
        parent:Some(parent_ref),answer:supplied.answer.into(),
        map:if native {None} else {Some(mapped)},
        native_anchor:prior.native_anchor,native_map:None};
    crate::pd_iz_frontend::validate_territory(&context,
        crate::nura_core::Domain::detect(context.current_i0()).label())?;
    let (query,relevance)=supplied.scientific_query.bind(&context.identity)?;
    iz_science::from_parent_computed(context,parent,query,relevance)
}
/// Only an already durable exact operation may retain its legacy output policy.
/// New operations cannot request a downgrade; mismatched bytes remain conflicts.
fn preserve_exact_retry_policy(answer: &mut iz_science::IzScienceInput,
    stored: &pd::PdOperationRecord) -> Result<(),String> {
    stored.validate()?;
    let prior = iz_science::validate_operation(&stored.request)?;
    let mut retry = answer.clone();
    retry.question_policy = prior.question_policy;
    if stored.request.identity != answer.context.identity
        || stored.request.operation_id != answer.context.operation_id
        || iz_science::encode_input(&retry)? != stored.request.source_input {
        return Err("LIGHT_IZ_SCIENCE_RETRY_CONFLICT".into());
    }
    *answer = retry;
    Ok(())
}

fn submit(parent:&pd::PdOperationRecord,mut answer:iz_science::IzScienceInput)
    ->Result<pd::PdOperationRecord,String>{
    let lookup=pd::encode_lookup(&pd::PdLookup {
        identity:answer.context.identity.clone(), operation_id:Some(answer.context.operation_id.clone()),
    })?;
    let pulse=ess_mai_zero_noi::trigger(lookup.as_slice());
    let existing=bridge::native_exchange(NativePeer::Shadow,"--pd-head-once",
        pulse.request(),pd::MAX_REQUEST_BYTES+256)?;
    if let Some(stored)=pd::decode_optional_record(&existing)? {
        preserve_exact_retry_policy(&mut answer,&stored)?;
    }
    let encoded=iz_science::encode_input(&answer)?;
    let pulse=ess_mai_zero_noi::trigger(encoded.as_slice());
    let response=bridge::native_exchange(NativePeer::Quantum,"--pd-failure-once",
        pulse.request(),pd::MAX_REQUEST_BYTES+256)?;
    let record=pd::decode_record(&response)?;
    if record.request.source_input!=encoded||record.previous_record!=parent.record_id{
        return Err("LIGHT_IZ_SCIENCE_RETURN_BINDING".into());
    }
    iz_science::validate_transition(&record.request,parent)?;
    Ok(record)
}
pub fn interactive(current:&lgc::PdLgcRef,bytes:&[u8])->Result<pd::PdOperationRecord,String>{
    let (operation,reference,supplied)=
        crate::pd_lgc_frontend::interactive_envelope::<Answer>(current,bytes)?;
    let parent=crate::pd_lgc_frontend::read_reference(&reference)?;
    let answer=input_value(&parent,&operation,supplied)?;
    submit(&parent,answer)
}

/// Explicit new user answer; never derive a future i0 from the question.
pub fn dispatch(args:&[String])->Option<i32>{
    if args.get(1).map(String::as_str)!=Some("--iz-science-answer-once"){return None;}
    if args.len()!=6 || args[2..].iter().any(String::is_empty){return Some(64);}
    let result=(||{
        let output=Path::new(&args[5]);
        if output.exists(){return Err("LIGHT_IZ_SCIENCE_OUTPUT_EXISTS".into());}
        let reference=lgc::decode_ref(&bridge::read_bounded(Path::new(&args[3]),lgc::REF_BYTES)?)?;
        let parent=crate::pd_lgc_frontend::read_reference(&reference)?;
        let answer=input(&parent,&args[2],&bridge::read_bounded(Path::new(&args[4]),LIMIT)?)?;
        let record=submit(&parent,answer)?;
        bridge::publish_new(output,&pd::encode_record(&record)?)?;
        println!("{}",record.request.scoped_question);
        bridge::print_operational(&record)
    })();
    Some(match result{Ok(())=>0,Err(error)=>{
        eprintln!("[iZ/KiE] {error}");70
    }})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn science_json_preserves_the_declared_boundary_bits() {
        let identity=pd::PdIdentity { owner:"test".into(),obligation:"model comparison".into(),
            case_id:"case".into(),scope:"WORK→NURA".into(),domain:"WORK→NURA".into(),lineage_root:"11".repeat(32) };
        let json=br#"{"user_text":"Exact numerical boundary, supplied by user.","scientific_query":{
            "task":"earth_rotation_angle","time_scale":"UT1","angle_unit":"radian",
            "jd_part1":2400000.5,"jd_part2":54388.0,"claim_radians":0.0,
            "tolerance_radians":0.40228372400281387,"input_source_id":"model test only",
            "query_id":"boundary","relevance_statement":"unverified relevance"}}"#;
        let answer:Answer=serde_json::from_slice(json).unwrap();
        let (bytes,_)=answer.scientific_query.bind(&identity).unwrap();
        let query=science_domains::decode_query(&bytes).unwrap();
        let science_domains::ScienceTask::EarthRotationAngle{tolerance_radians,..}=query.task;
        assert_eq!(tolerance_radians.to_bits(),0x3fd9bf043b938800,
            "Light must preserve the user-declared f64 boundary before Quantum compares it");
        assert_eq!(science_domains::evaluate(&query).unwrap().relation,
            science_domains::ModelRelation::Unresolved);
        let text=std::str::from_utf8(json).unwrap();
        for token in ["\"0.4\"", "null", "true", "[]", "{}", "1e999", "0", "-0.1"] {
            let invalid=text.replace("0.40228372400281387", token);
            let result=serde_json::from_str::<Answer>(&invalid).map_err(|e|e.to_string())
                .and_then(|a|a.scientific_query.bind(&identity));
            assert!(result.is_err(), "invalid tolerance must not reach model: {token}");
        }
    }
    #[test]fn no_implicit_science_dispatch_or_missing_user_arguments(){
        assert!(dispatch(&["light".into()]).is_none());
        assert_eq!(dispatch(&["light".into(),"--iz-science-answer-once".into()]),Some(64));
        assert!(serde_json::from_slice::<Answer>(br#"{"scientific_query":{}}"#).is_err());
        assert!(serde_json::from_slice::<Answer>(br#"{"user_text":"answer","science":{}}"#).is_err());
    }
}

#[cfg(test)]
mod literal_science_entry_regression {
    use super::*;
    #[test]
    fn literal_scientific_answer_reaches_existing_context_and_codec() {
        let parent=match lgc::decode_output(include_bytes!("../test_fixtures/literal_answer/parent.lgc")).unwrap() {
            lgc::PdLgcOutput::Operational(record)=>record,
            _=>panic!("fixture must be a real operational PIZ parent"),
        };
        let source=iz::validate_operation(&parent.request).unwrap();
        let field=*source.question_spec.requested_fields.first().unwrap();
        let key=match field {
            iz::IzField::Positive=>"positive",iz::IzField::Negative=>"negative",
            iz::IzField::Distinction=>"distinction",iz::IzField::SourceReference=>"source_reference",
            iz::IzField::Verification=>"verification",
        };
        let text="motor evidence: shqiponjë, exact.";
        let mut answer=serde_json::Map::new();
        answer.insert(key.into(),serde_json::Value::String("shqiponjë,".into()));
        let bytes=serde_json::to_vec(&serde_json::json!({
            "user_text":text,"answer":answer,"scientific_query":{
                "task":"earth_rotation_angle","time_scale":"UT1","angle_unit":"radian",
                "jd_part1":2400000.5,"jd_part2":54388.0,"claim_radians":0.0,
                "tolerance_radians":1e-12,"input_source_id":"unit-test only",
                "query_id":"literal-fields","relevance_statement":"user reported model-only proposal"
            }
        })).unwrap();
        let result=input(&parent,"literal-science-reply",&bytes)
            .expect("literal fields must reach existing validation without becoming scientific proof");
        assert_eq!(result.context.current_i0(),text);
        assert_eq!(result.context.answer.get(field),"shqiponjë,");
        let wire=iz_science::encode_input(&result).unwrap();
        assert!(wire.starts_with(b"PDS4"));
        assert_eq!(iz_science::decode_input(&wire).unwrap(),result);
        let mut payload:serde_json::Value=serde_json::from_slice(&bytes).unwrap();
        payload["answer"][key]="words absent from actual input".into();
        assert_eq!(input(&parent,"missing-fragment",&serde_json::to_vec(&payload).unwrap()).unwrap_err(),
            "IZ_ANSWER_NOT_IN_USER_TEXT");
        payload["answer"][key]="shqiponjë,".into();
        payload["user_text"]="motor shqiponjë, shqiponjë,".into();
        assert_eq!(input(&parent,"ambiguous-fragment",&serde_json::to_vec(&payload).unwrap()).unwrap_err(),
            "IZ_ANSWER_SPAN_AMBIGUOUS");
        payload["user_text"]=text.into();payload.as_object_mut().unwrap().remove("answer");
        let legacy=input(&parent,"same-operation",&serde_json::to_vec(&payload).unwrap()).unwrap();
        payload["answer"]=serde_json::json!({});
        let explicit_empty=input(&parent,"same-operation",&serde_json::to_vec(&payload).unwrap()).unwrap();
        assert_eq!(iz_science::encode_input(&legacy).unwrap(),iz_science::encode_input(&explicit_empty).unwrap());
        assert_eq!(legacy.science_query,result.science_query);
        payload["answer"]=serde_json::json!({"verdict":"1:1"});
        assert!(input(&parent,"unknown-label",&serde_json::to_vec(&payload).unwrap()).unwrap_err()
            .starts_with("LIGHT_IZ_SCIENCE_JSON:"));
        let mut altered=result.clone();altered.context.answer=Default::default();
        assert_ne!(iz_science::encode_input(&altered).unwrap(),wire,"retry comparison includes annotations");
    }
}

#[cfg(test)]
mod interactive_science_tests {
    use super::*;
    #[test]
    fn interactive_science_uses_the_same_computed_contract_as_explicit_cli() {
        let parent=match lgc::decode_output(include_bytes!("../test_fixtures/literal_answer/parent.lgc")).unwrap(){
            lgc::PdLgcOutput::Operational(record)=>record,_=>panic!("PIZ parent required"),
        };
        let reference=lgc::PdLgcRef::from_record(&parent).unwrap();
        let payload=serde_json::json!({"user_text":"motor model-only measurement, exact.",
            "scientific_query":{"task":"earth_rotation_angle","time_scale":"UT1","angle_unit":"radian",
                "jd_part1":2400000.5,"jd_part2":54388.0,"claim_radians":0.40228372400281387,
                "tolerance_radians":1e-12,"input_source_id":"declared model parameters",
                "query_id":"interactive-model","relevance_statement":"limited to the model"}});
        let bytes=serde_json::to_vec(&serde_json::json!({"operation_id":"science-reply",
            "reference":lgc::encode_ref(&reference).unwrap(),"payload":payload})).unwrap();
        let (operation,selected,answer)=crate::pd_lgc_frontend::interactive_envelope::<Answer>(&reference,&bytes).unwrap();
        selected.validate_record(&parent).unwrap();
        let interactive=input_value(&parent,&operation,answer).unwrap();
        let cli=input(&parent,"science-reply",&serde_json::to_vec(&payload).unwrap()).unwrap();
        assert_eq!(iz_science::encode_input(&interactive).unwrap(),iz_science::encode_input(&cli).unwrap());
        assert_eq!(interactive.context.current_i0(),"motor model-only measurement, exact.");
        assert!(interactive.context.answer.is_empty());
    }
}
