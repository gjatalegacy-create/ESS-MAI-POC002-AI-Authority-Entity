//! Exact LGC-addressed continuation, with no topic/head guessing.
//! pd_mp supplies references; the authoritative material remains in Shadow Vault.
use crate::pd_operational_bridge::{self as bridge, NativePeer};
use sha2::{Digest, Sha256};
use shadow_contracts::pd_memory::{self as pd, iz, lgc};
use std::path::PathBuf;

pub struct LgcArgs {
    operation: String,
    reference: Option<PathBuf>,
    native_bundle: Option<PathBuf>,
    native_lookup: Option<PathBuf>,
    input: PathBuf,
    output: PathBuf,
    answer_fields: Option<PathBuf>,
}
pub struct LgcInput {
    pub context: iz::IzContext,
    pub output: PathBuf,
}
pub fn parse_args(args: &[String]) -> Result<Option<LgcArgs>, String> {
    let (reply, native, lookup) = match args.get(1).map(String::as_str) {
        Some("--iz-lgc-new-once") => (false, false, false),
        Some("--iz-lgc-reply-once") => (true, false, false),
        Some("--iz-lgc-native-reply-once") => (true, true, false),
        Some("--iz-native-reply-once") => (false, false, true),
        _ => return Ok(None),
    };
    let count = match (reply, native, lookup) { (_, true, _) => 7, (true, _, _) | (_, _, true) => 6, _ => 5 };
    let annotated=args.len()==count+2 && args.get(count).map(String::as_str)==Some("--answer-fields");
    if (args.len()!=count && !annotated) || args[2..].iter().any(String::is_empty) {
        return Err("LIGHT_LGC_USAGE: --iz-lgc-new-once OP INPUT OUTPUT | --iz-lgc-reply-once OP REFERENCE INPUT OUTPUT | --iz-lgc-native-reply-once OP REFERENCE NATIVE_BUNDLE INPUT OUTPUT | --iz-native-reply-once OP NATIVE_LOOKUP INPUT OUTPUT [--answer-fields JSON]".into());
    }
    Ok(Some(LgcArgs {
        operation:args[2].clone(),
        reference:reply.then(||PathBuf::from(&args[3])),
        native_bundle:native.then(||PathBuf::from(&args[4])),
        native_lookup:lookup.then(||PathBuf::from(&args[3])),
        input:PathBuf::from(&args[count-2]),output:PathBuf::from(&args[count-1]),
        answer_fields:annotated.then(||PathBuf::from(&args[count+1])),
    }))
}
fn hash(bytes:&[u8])->pd::Hash {Sha256::digest(bytes).into()}
fn derive_reply(operation:String, user:String, reference:&lgc::PdLgcRef,
                record:&pd::PdOperationRecord, answer:iz::IzAnswer)->Result<iz::IzContext,String> {
    reference.validate_record(record)?;
    let prior=iz::context_from_record(record)?;
    let context=iz::IzContext {
        version:prior.version.max(iz::IZ_ACTUAL_INPUT_VERSION),identity:prior.identity,operation_id:operation,
        origin_i0:prior.origin_i0,user_text:user,
        parent:Some(iz::IzParent::from_record(record)?),
        answer,map:None,
        native_anchor:prior.native_anchor,native_map:None,
    };
    context.validate()?;
    Ok(context)
}
/// Carry a validated historical reference, never authorize it. The existing
/// BESA native-map/read path resolves this anchor against Shadow's actual latest
/// durable T1/T2 before Quantum's witnessed ASHT attempt. Identity, original i0,
/// current i0 and operational CAS parent are preserved, not inferred or renamed.
fn bind_native_anchor(mut context: iz::IzContext,
    anchor: shadow_contracts::pd_prepared::PdContinuationLink) -> Result<iz::IzContext,String> {
    anchor.validate()?;
    if !anchor.clock_millis { return Err("LIGHT_LGC_NATIVE_CLOCK_MIGRATION_REQUIRED".into()); }
    if anchor.cognitive_session_id != pd::hex(&context.identity.key()?) {
        return Err("LIGHT_LGC_NATIVE_COGNITIVE_IDENTITY_MISMATCH".into());
    }
    if anchor.territory != context.identity.scope || anchor.territory != context.identity.domain {
        return Err("LIGHT_LGC_NATIVE_TERRITORY_MISMATCH".into());
    }
    context.version = iz::IZ_NATIVE_CONTINUITY_VERSION;
    context.native_anchor = Some(anchor);
    context.map = None;
    context.native_map = None;
    context.validate()?;
    Ok(context)
}
/// Derive a new actual user operation under an already scoped native case.
/// This is transport data only: BESA still obtains the current native map from
/// Shadow, and Shadow still validates the exact T2/head/checkpoint transition.
fn native_reply_context(
    origin: &iz::IzContext,
    anchor: shadow_contracts::pd_prepared::PdContinuationLink,
    operation: String,
    user: String,
    head: Option<&pd::PdOperationRecord>,
    answer: iz::IzAnswer,
) -> Result<iz::IzContext, String> {
    origin.validate()?;
    if let Some(head) = head {
        let prior = iz::context_from_record(head)?;
        if prior.identity != origin.identity || prior.origin_i0 != origin.origin_i0 {
            return Err("LIGHT_NATIVE_INQUIRY_FOREIGN_OPERATIONAL_HEAD".into());
        }
    }
    let context = iz::IzContext {
        version: iz::IZ_NATIVE_CONTINUITY_VERSION,
        identity: origin.identity.clone(), operation_id: operation,
        origin_i0: origin.origin_i0.clone(), user_text: user,
        parent: head.map(iz::IzParent::from_record).transpose()?,
        answer, map: None,
        native_anchor: Some(anchor.clone()), native_map: None,
    };
    bind_native_anchor(context, anchor)
}
fn validate_native_retry(
    stored: &iz::IzContext, origin: &iz::IzContext,
    anchor: &shadow_contracts::pd_prepared::PdContinuationLink,
    operation: &str, user: &str, answer: &iz::IzAnswer,
) -> Result<(), String> {
    stored.validate()?;
    origin.validate()?;
    if stored.version != iz::IZ_NATIVE_CONTINUITY_VERSION
        || stored.identity != origin.identity || stored.origin_i0 != origin.origin_i0
        || stored.operation_id != operation || stored.current_i0() != user
        || stored.native_anchor.as_ref() != Some(anchor) || stored.answer != *answer {
        return Err("LIGHT_NATIVE_INQUIRY_OPERATION_CONFLICT".into());
    }
    Ok(())
}
/// The lookup and scoped origin must be owner-resolved; a file's self-consistent
/// hashes alone are not admission. No operation is appended by this function.
fn native_reply_with(
    operation: String, user: String,
    reference: &shadow_contracts::pd_recovery::RecoveryLookup,
    answer: iz::IzAnswer,
    exchange: &mut impl FnMut(&str, &[u8], usize) -> Result<Vec<u8>, String>,
) -> Result<iz::IzContext, String> {
    use shadow_contracts::pd_recovery as recovery;
    let bytes = exchange("--pd-recover-once", &recovery::encode_lookup(reference)?, recovery::MAX_BUNDLE_BYTES)?;
    let bundle = recovery::decode_bundle(&bytes)?;
    bundle.validate_for(reference)?;
    let anchor = shadow_contracts::pd_prepared::PdContinuationLink::from_bundle(&bundle)?;
    let (prepared, _, _) = bundle.validated()?;
    let origin = match (&prepared.source_iz, &prepared.source_dialogue) {
        (Some(link), None) => link.context.clone(),
        (None, Some(link)) => {
            let bytes = exchange("--pd-head-once", &pd::encode_lookup(&link.context.lookup())?, pd::MAX_REQUEST_BYTES + 256)?;
            let record = pd::decode_optional_record(&bytes)?.ok_or("LIGHT_NATIVE_INQUIRY_SOURCE_MISSING")?;
            if record.record_id != link.context.record_id
                || record.request.identity != link.context.identity
                || record.request.operation_id != link.context.operation_id {
                return Err("LIGHT_NATIVE_INQUIRY_SOURCE_MISMATCH".into());
            }
            iz::context_from_record(&record)?
        },
        _ => return Err("LIGHT_NATIVE_INQUIRY_SCOPED_ORIGIN_REQUIRED".into()),
    };
    // Resolve this exact logical operation before asking for today's head. A
    // retry must retain its original parent and must never ingest i0 twice.
    let exact = pd::PdLookup { identity: origin.identity.clone(), operation_id: Some(operation.clone()) };
    let bytes = exchange("--pd-head-once", &pd::encode_lookup(&exact)?, pd::MAX_REQUEST_BYTES + 256)?;
    if let Some(record) = pd::decode_optional_record(&bytes)? {
        let context = iz::context_from_record(&record)?;
        validate_native_retry(&context, &origin, &anchor, &operation, &user, &answer)?;
        return Ok(context);
    }
    let lookup = pd::PdLookup { identity: origin.identity.clone(), operation_id: None };
    let bytes = exchange("--pd-head-once", &pd::encode_lookup(&lookup)?, pd::MAX_REQUEST_BYTES + 256)?;
    let head = pd::decode_optional_record(&bytes)?;
    native_reply_context(&origin, anchor, operation, user, head.as_ref(), answer)
}
pub fn native_reply(
    operation: String, user: String,
    reference: &shadow_contracts::pd_recovery::RecoveryLookup,
    answer: iz::IzAnswer,
) -> Result<iz::IzContext, String> {
    native_reply_with(operation, user, reference, answer, &mut |mode, input, limit| {
        let mut h = Sha256::new();
        h.update(b"OPL2S/LIGHT/NATIVE-INQUIRY-READ/V1");
        h.update((mode.len() as u64).to_le_bytes()); h.update(mode.as_bytes());
        h.update((input.len() as u64).to_le_bytes()); h.update(input);
        let scope = format!("{:x}", h.finalize());
        crate::noi_caller::invoke("LIGHT_NATIVE_INQUIRY_READ", &scope, || {
            bridge::native_exchange(NativePeer::Shadow, mode, input, limit)
        })
    })
}
/// A new interactive i0 is a new operation even when its text repeats. Stable
/// caller-selected OP identifiers remain available through the one-shot CLI
/// for exact retry after a transport interruption.
pub fn interactive_native_reply(
    user: &str, reference: &shadow_contracts::pd_recovery::RecoveryLookup,
) -> Result<iz::IzContext, String> {
    native_reply(next_operation()?, user.to_owned(), reference, Default::default())
}
fn next_operation() -> Result<String, String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let sequence = NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
        .map_err(|_| "LIGHT_NATIVE_INQUIRY_SEQUENCE_OVERFLOW")?;
    let clock = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| "LIGHT_NATIVE_INQUIRY_CLOCK")?.as_nanos();
    Ok(format!("native-i0-{}-{clock}-{sequence}", std::process::id()))
}

/// The envelope names the displayed question. Its checksum never grants authority;
/// the owning Shadow process must resolve the reference before any continuation.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InteractiveEnvelope<T> {
    operation_id: String,
    reference: Vec<u8>,
    payload: T,
}
pub(crate) fn interactive_envelope<T: serde::de::DeserializeOwned>(
    current: &lgc::PdLgcRef, bytes: &[u8],
) -> Result<(String, lgc::PdLgcRef, T), String> {
    if bytes.len() > 48 * 1024 { return Err("LIGHT_IZ_INTERACTIVE_LIMIT".into()); }
    if bytes.iter().copied().find(|b| !b.is_ascii_whitespace()) != Some(b'{') {
        return Err("LIGHT_IZ_INTERACTIVE_OBJECT_REQUIRED".into());
    }
    let value: InteractiveEnvelope<T> = serde_json::from_slice(bytes)
        .map_err(|e| format!("LIGHT_IZ_INTERACTIVE_JSON:{e}"))?;
    current.validate()?;
    let reference = lgc::decode_ref(&value.reference)?;
    if reference.identity_key != current.identity_key {
        return Err("LIGHT_IZ_INTERACTIVE_FOREIGN_CASE".into());
    }
    Ok((value.operation_id, reference, value.payload))
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct InteractiveAnswer {
    user_text: String,
    answer: crate::pd_iz_frontend::LiteralAnswer,
}
pub(crate) fn read_reference(reference: &lgc::PdLgcRef) -> Result<pd::PdOperationRecord, String> {
    let pulse = ess_mai_zero_noi::trigger(reference);
    let bytes = bridge::native_exchange(NativePeer::Shadow, "--pd-lgc-read-once",
        &lgc::encode_ref(pulse.request())?, pd::MAX_REQUEST_BYTES + 256)?;
    let record = pd::decode_optional_record(&bytes)?.ok_or("LIGHT_LGC_REFERENCE_NOT_FOUND")?;
    reference.validate_record(&record)?;
    Ok(record)
}
fn referenced_reply(reference: &lgc::PdLgcRef, operation: String, user: String,
    answer: iz::IzAnswer) -> Result<iz::IzContext, String> {
    let parsed = crate::light_coordinator::LightRequest::try_new(&user)
        .map_err(|e| format!("LIGHT_LGC_INPUT:{e}"))?;
    if parsed.text() != user { return Err("LIGHT_LGC_INPUT_NOT_EXACT".into()); }
    let context = derive_reply(operation, user, reference, &read_reference(reference)?, answer)?;
    crate::pd_iz_frontend::require_current_input_mode(&context)?;
    crate::pd_iz_frontend::validate_territory(&context,
        crate::nura_core::Domain::detect(context.current_i0()).label())?;
    Ok(context)
}
pub fn interactive_answer(current: &lgc::PdLgcRef, bytes: &[u8]) -> Result<iz::IzContext, String> {
    let (operation, reference, answer) = interactive_envelope::<InteractiveAnswer>(current, bytes)?;
    referenced_reply(&reference, operation, answer.user_text, answer.answer.into())
}
pub fn interactive_reply(reference: &lgc::PdLgcRef, user: &str) -> Result<iz::IzContext, String> {
    referenced_reply(reference, next_operation()?, user.to_owned(), Default::default())
}
pub fn display_reference(record: &pd::PdOperationRecord, out: &mut impl std::io::Write)
    -> Result<lgc::PdLgcRef, String> {
    // Only PIZ has the field frontier admitted by this route; PLF/PDS do not.
    iz::validate_operation(&record.request)?;
    let reference = lgc::PdLgcRef::from_record(record)?;
    let bytes = serde_json::to_string(&lgc::encode_ref(&reference)?).map_err(|e| e.to_string())?;
    writeln!(out, "[PD_IZ_REFERENCE] {bytes}").map_err(|e| e.to_string())?;
    out.flush().map_err(|e| e.to_string())?;
    Ok(reference)
}

pub fn prepare(args:LgcArgs)->Result<LgcInput,String> {
    if args.output.exists() {return Err("LIGHT_LGC_OUTPUT_EXISTS".into());}
    let user=String::from_utf8(bridge::read_bounded(&args.input,2048)?)
        .map_err(|_|"LIGHT_LGC_INPUT_UTF8")?;
    let parsed=crate::light_coordinator::LightRequest::try_new(user.as_str())
        .map_err(|e|format!("LIGHT_LGC_INPUT:{e}"))?;
    if parsed.text()!=user {return Err("LIGHT_LGC_INPUT_NOT_EXACT".into());}
    let answer=match args.answer_fields {
        Some(path)=>serde_json::from_slice::<crate::pd_iz_frontend::LiteralAnswer>(
            &bridge::read_bounded(&path,48*1024)?)
            .map_err(|e|format!("LIGHT_LGC_ANSWER_JSON:{e}"))?.into(),
        None=>iz::IzAnswer::default(),
    };
    let context=if let Some(path)=args.native_lookup {
        let reference=shadow_contracts::pd_recovery::decode_lookup(&bridge::read_bounded(&path,68)?)?;
        native_reply(args.operation,user,&reference,answer)?
    } else if let Some(path)=args.reference {
        let reference=lgc::decode_ref(&bridge::read_bounded(&path,lgc::REF_BYTES)?)?;
        let pulse=ess_mai_zero_noi::trigger(&reference);
        let input=lgc::encode_ref(pulse.request())?;
        let bytes=bridge::native_exchange(NativePeer::Shadow,"--pd-lgc-read-once",
            &input,pd::MAX_REQUEST_BYTES+256)?;
        let record=pd::decode_optional_record(&bytes)?.ok_or("LIGHT_LGC_REFERENCE_NOT_FOUND")?;
        let context=derive_reply(args.operation,user,&reference,&record,answer)?;
        match args.native_bundle {
            Some(path)=>{
                // This file is untrusted historical data. No recovery, T2
                // finalization, native receipt or knowledge write occurs here.
                let bytes=bridge::read_bounded(&path,shadow_contracts::pd_recovery::MAX_BUNDLE_BYTES)?;
                let bundle=shadow_contracts::pd_recovery::decode_bundle(&bytes)?;
                let anchor=shadow_contracts::pd_prepared::PdContinuationLink::from_bundle(&bundle)?;
                bind_native_anchor(context,anchor)?
            },
            None=>context,
        }
    } else {
        let territory=crate::nura_core::Domain::detect(&user).label().to_string();
        // Explicit new case token, scoped together with the actual origin; never a global latest case.
        iz::IzContext {
            version:2,identity:pd::PdIdentity {
                owner:"native-light-runtime".into(),obligation:"ASHT exact positive-negative distinction".into(),
                case_id:format!("lgc-{}",pd::hex(&hash(args.operation.as_bytes()))),
                scope:territory.clone(),domain:territory,lineage_root:pd::hex(&hash(user.as_bytes())),
            },
            operation_id:args.operation,origin_i0:user.clone(),user_text:user,
            parent:None,answer,map:None,native_anchor:None,native_map:None,
        }
    };
    crate::pd_iz_frontend::require_current_input_mode(&context)?;
    crate::pd_iz_frontend::validate_territory(&context,
        crate::nura_core::Domain::detect(context.current_i0()).label())?;
    Ok(LgcInput{context,output:args.output})
}
fn bind_native(context:&iz::IzContext, bundle:shadow_contracts::pd_recovery::RecoveryBundle)
    ->Result<lgc::PdLgcOutput,String> {
    let (prepared,_,continuation)=bundle.validated()?;
    let link=prepared.source_iz.as_ref().ok_or("LIGHT_LGC_NATIVE_IZ_ORIGIN_MISSING")?;
    if continuation.is_none() || !iz::same_intent(context,&link.context) {
        return Err("LIGHT_LGC_NATIVE_INTENT_MISMATCH".into());
    }
    Ok(lgc::PdLgcOutput::Native(bundle))
}
pub fn completed(context:&iz::IzContext, lookup:&shadow_contracts::pd_recovery::RecoveryLookup)
    ->Result<lgc::PdLgcOutput,String> {
    // Deliberate recovery may finish an already durable T1; never called a read-only operation.
    eprintln!("[LGC] explicit_native_recovery begin");
    bind_native(context,crate::pd_native_followup::fetch(lookup)?)
}
pub fn exact(context:&iz::IzContext)->Result<Option<lgc::PdLgcOutput>,String> {
    if let Some(record)=crate::pd_iz_frontend::exact_operation(context)? {
        return Ok(Some(lgc::PdLgcOutput::Operational(record)));
    }
    let pulse=ess_mai_zero_noi::trigger(context);
    let input=iz::encode_context(pulse.request())?;
    let bytes=bridge::native_exchange(NativePeer::Shadow,"--pd-iz-native-lookup-once",&input,69)?;
    match lgc::decode_optional_native_lookup(&bytes)? {
        None=>Ok(None),
        Some(lookup)=>completed(context,&lookup).map(Some),
    }
}
pub fn publish(input:&LgcInput,value:&lgc::PdLgcOutput)->Result<(),String> {
    let delivery_scope = shadow_contracts::pd_memory::hex(&input.context.identity.key()?);
    crate::noi_caller::invoke("LIGHT_IZ_DELIVERY", &delivery_scope, || {
    let encoded=lgc::encode_output(value)?;
    bridge::publish_new(&input.output,&encoded)?;
    match value {
        lgc::PdLgcOutput::Operational(record)=>{
            println!("{}",record.request.scoped_question);
            bridge::print_operational(record)
        }
        lgc::PdLgcOutput::Native(bundle)=>{
            let (_,response,_)=bundle.validated()?;
            println!("[LGC] status=NativeTerminal durable_T1=true durable_T2=true Y={} X={} accepted_knowledge={}",
                response.verdict.verified,response.verdict.primitive,
                (response.verdict.verified,response.verdict.primitive)==(1,1));
            Ok(())
        }
    }

    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_native_inquiry_cli_keeps_both_exact_predecessor_references() {
        let args = ["light", "--iz-lgc-native-reply-once", "reply-2", "operational.ref",
            "native.bundle", "actual-i0.txt", "result.lgc"].map(String::from);
        let parsed = parse_args(&args).unwrap();
        assert!(parsed.is_some(), "the explicit native inquiry must enter the existing LGC/BESA path");
        let parsed=parsed.unwrap();
        assert_eq!(parsed.reference,Some(PathBuf::from("operational.ref")));
        assert_eq!(parsed.native_bundle,Some(PathBuf::from("native.bundle")));
        assert_eq!(parsed.input,PathBuf::from("actual-i0.txt"));
        assert_eq!(parsed.output,PathBuf::from("result.lgc"));
        assert_eq!(parsed.operation,"reply-2");
        for count in 2..7 { assert!(parse_args(&args[..count]).is_err()); }
        let mut extra=args.to_vec();extra.push("extra".into());assert!(parse_args(&extra).is_err());
    }
    fn native_reply_parts()->(iz::IzContext,shadow_contracts::pd_prepared::PdContinuationLink) {
        let origin="motor era00".to_string();
        let identity=pd::PdIdentity { owner:"native-light-runtime".into(),
            obligation:"ASHT exact positive-negative distinction".into(),case_id:"case-1".into(),
            scope:"WORK→NURA".into(),domain:"WORK→NURA".into(),lineage_root:pd::hex(&hash(origin.as_bytes())) };
        let anchor=shadow_contracts::pd_prepared::PdContinuationLink {
            cognitive_session_id:pd::hex(&identity.key().unwrap()),territory:identity.scope.clone(),
            predecessor:shadow_contracts::pd_recovery::RecoveryLookup {identity:[4;32],request_sha256:[5;32]},
            predecessor_transaction_id:[6;32],predecessor_continuation_sha256:[7;32],
            predecessor_next_i0:"next-native".into(),clock_millis:true };
        let context=iz::IzContext {version:iz::IZ_ACTUAL_INPUT_VERSION,identity,operation_id:"reply-2".into(),
            origin_i0:origin,user_text:"motor era00 with an exact distinction".into(),
            parent:Some(iz::IzParent {record_id:[1;32],revision:2,state_digest:[2;32],question_sha256:[3;32]}),
            answer:Default::default(),map:None,native_anchor:None,native_map:None};
        (context,anchor)
    }
    #[test]
    fn native_inquiry_adapter_preserves_identity_input_parent_and_requires_shadow_map() {
        let (before,anchor)=native_reply_parts();
        let after=bind_native_anchor(before.clone(),anchor.clone()).unwrap();
        assert_eq!(after.version,iz::IZ_NATIVE_CONTINUITY_VERSION);
        assert_eq!(after.identity,before.identity);
        assert_eq!(after.origin_i0,before.origin_i0);
        assert_eq!(after.user_text,before.user_text);
        assert_eq!(after.parent,before.parent);
        assert_eq!(after.native_anchor,Some(anchor.clone()));
        assert!(after.native_map.is_none());
        assert_eq!(after.require_mapped().unwrap_err(),"IZ_NATIVE_MAP_REQUIRED");
        assert_eq!(crate::pd_iz_frontend::native_predecessor(&after).unwrap(),Some(anchor.predecessor));
    }
    #[test]
    fn native_inquiry_adapter_refuses_clock_identity_and_scope_relabeling() {
        let (context,anchor)=native_reply_parts();
        let mut seconds=anchor.clone();seconds.clock_millis=false;
        assert_eq!(bind_native_anchor(context.clone(),seconds).unwrap_err(),"LIGHT_LGC_NATIVE_CLOCK_MIGRATION_REQUIRED");
        let mut foreign=anchor.clone();foreign.cognitive_session_id="another-session".into();
        assert_eq!(bind_native_anchor(context.clone(),foreign).unwrap_err(),"LIGHT_LGC_NATIVE_COGNITIVE_IDENTITY_MISMATCH");
        let mut foreign=anchor;foreign.territory="HOME→ALISAH".into();
        assert_eq!(bind_native_anchor(context,foreign).unwrap_err(),"LIGHT_LGC_NATIVE_TERRITORY_MISMATCH");
    }
    #[test]
    fn cli_requires_explicit_new_or_exact_reference_mode() {
        assert!(parse_args(&["light".into()]).unwrap().is_none());
        for mode in ["--iz-lgc-new-once","--iz-lgc-reply-once"] {
            assert!(parse_args(&["light".into(),mode.into()]).is_err());
        }
        let args=["light","--iz-lgc-reply-once","op","ref","input","output"].map(String::from);
        let parsed=parse_args(&args).unwrap().unwrap();
        assert_eq!(parsed.reference,Some(PathBuf::from("ref")));
        assert_eq!(parsed.operation,"op");
        let mut bad=args.to_vec();bad.push("unexpected".into());assert!(parse_args(&bad).is_err());
    }
}

#[cfg(test)]
mod r34_native_entry_regression {
    use super::*;
    use shadow_contracts::pd_recovery as recovery;
    const BUNDLE: &[u8] = include_bytes!("../test_fixtures/r33_native_origin/support-02.bundle");
    const SOURCE: &[u8] = include_bytes!("../test_fixtures/r33_native_origin/support-02.pds");
    fn parts() -> (recovery::RecoveryBundle, pd::PdOperationRecord, recovery::RecoveryLookup) {
        let bundle = recovery::decode_bundle(BUNDLE).unwrap();
        let source = pd::decode_record(SOURCE).unwrap();
        let reference = recovery::RecoveryLookup { identity: bundle.identity, request_sha256: bundle.request_sha256 };
        (bundle, source, reference)
    }
    #[test]
    fn r34_native_lookup_reply_reaches_existing_inquiry_entry() {
        let args = ["light", "--iz-native-reply-once", "reply-3", "native.lookup", "actual-i0.txt", "result.lgc"].map(String::from);
        let parsed = super::parse_args(&args).unwrap();
        assert!(parsed.is_some(), "an exact native T2 lookup and actual user reply must enter the existing mapped inquiry path");
        let parsed = parsed.unwrap();
        assert_eq!(parsed.native_lookup, Some(PathBuf::from("native.lookup")));
        assert!(parsed.reference.is_none() && parsed.native_bundle.is_none());
        assert_eq!(parsed.operation, "reply-3");
    }
    #[test]
    fn r34_native_origin_owner_reads_preserve_checkpoint_and_actual_i0() {
        let (bundle, source, reference) = parts();
        let checkpoint = bundle.validated().unwrap().0.checkpoint;
        let mut calls = Vec::new();
        let current = "Evaluate the same Earth rotation angle with the missing distinction.";
        let context = native_reply_with("reply-3".into(), current.into(), &reference, Default::default(), &mut |mode, bytes, _| {
            calls.push(mode.to_string());
            match mode {
                "--pd-recover-once" => {
                    assert_eq!(recovery::decode_lookup(bytes).unwrap(), reference);
                    Ok(BUNDLE.to_vec())
                },
                "--pd-head-once" => {
                    let lookup = pd::decode_lookup(bytes)?;
                    assert_eq!(lookup.identity, source.request.identity);
                    match lookup.operation_id.as_deref() {
                        Some("reply-3") => pd::encode_optional_record(None),
                        Some(id) if id == source.request.operation_id => pd::encode_optional_record(Some(&source)),
                        None => pd::encode_optional_record(Some(&source)),
                        _ => panic!("unscoped read"),
                    }
                },
                _ => panic!("inquiry preparation cannot commit or issue a verdict"),
            }
        }).unwrap();
        let origin = iz::context_from_record(&source).unwrap();
        assert_eq!(context.identity, source.request.identity);
        assert_eq!(context.origin_i0, origin.origin_i0);
        assert_eq!(context.current_i0(), current);
        assert_eq!(context.parent, Some(iz::IzParent::from_record(&source).unwrap()));
        assert_eq!(context.native_anchor.as_ref().unwrap().predecessor, reference);
        assert!(context.native_map.is_none(), "BESA must still obtain the owner map");
        assert!(context.answer.is_empty(), "no answer or scientific evidence is invented");
        assert_eq!(context.require_mapped().unwrap_err(), "IZ_NATIVE_MAP_REQUIRED");
        assert_eq!(bundle.validated().unwrap().0.checkpoint, checkpoint);
        assert_eq!(calls, ["--pd-recover-once", "--pd-head-once", "--pd-head-once", "--pd-head-once"]);
    }
    #[test]
    fn r34_native_inquiry_rejects_foreign_lookup_and_incomplete_t2_before_other_reads() {
        let (bundle, _, mut reference) = parts();
        reference.request_sha256[0] ^= 1;
        let mut calls=0;
        assert!(native_reply_with("reply".into(), "actual user input".into(), &reference, Default::default(), &mut |mode, _, _| {
            calls+=1; assert_eq!(mode,"--pd-recover-once"); Ok(BUNDLE.to_vec())
        }).is_err());
        assert_eq!(calls,1);
        let reference=recovery::RecoveryLookup { identity:bundle.identity, request_sha256:bundle.request_sha256 };
        let mut incomplete=bundle;incomplete.continuation=None;
        let incomplete=recovery::encode_bundle(&incomplete).unwrap();
        let mut calls=0;
        assert!(native_reply_with("reply".into(), "actual user input".into(), &reference, Default::default(), &mut |mode, _, _| {
            calls+=1;assert_eq!(mode,"--pd-recover-once");Ok(incomplete.clone())
        }).is_err());
        assert_eq!(calls,1);
    }
    #[test]
    fn r34_native_retry_binds_exact_operation_user_origin_and_predecessor() {
        let (bundle, source, _) = parts();
        let origin=iz::context_from_record(&source).unwrap();
        let anchor=shadow_contracts::pd_prepared::PdContinuationLink::from_bundle(&bundle).unwrap();
        let current="What exact distinction is still missing?";
        let context=native_reply_context(&origin,anchor.clone(),"reply-3".into(),current.into(),Some(&source),Default::default()).unwrap();
        let before=iz::encode_context(&context).unwrap();
        validate_native_retry(&context,&origin,&anchor,"reply-3",current,&Default::default()).unwrap();
        for (operation,user) in [("reply-4",current),("reply-3","Different user input")] {
            assert!(validate_native_retry(&context,&origin,&anchor,operation,user,&Default::default()).is_err());
        }
        let mut foreign=anchor.clone();foreign.predecessor.request_sha256[0]^=1;
        assert!(validate_native_retry(&context,&origin,&foreign,"reply-3",current,&Default::default()).is_err());
        let mut foreign_origin=origin.clone();foreign_origin.identity.case_id.push('x');
        assert!(validate_native_retry(&context,&foreign_origin,&anchor,"reply-3",current,&Default::default()).is_err());
        assert_eq!(iz::encode_context(&context).unwrap(),before,"retry is data validation, no new turn or replacement parent");
    }
    #[test]
    fn r34_native_reply_context_does_not_copy_previous_answers_or_maps() {
        let (bundle, source, _) = parts();
        let origin=iz::context_from_record(&source).unwrap();
        let before=iz::encode_context(&origin).unwrap();
        let anchor=shadow_contracts::pd_prepared::PdContinuationLink::from_bundle(&bundle).unwrap();
        let context=native_reply_context(&origin,anchor,"next-i0".into(),"New exact reply".into(),Some(&source),Default::default()).unwrap();
        assert!(context.answer.is_empty());assert!(context.map.is_none()&&context.native_map.is_none());
        assert_eq!(context.origin_i0,origin.origin_i0);assert_eq!(context.current_i0(),"New exact reply");
        assert_eq!(iz::encode_context(&origin).unwrap(),before);
    }

    #[test]
    fn literal_native_retry_cannot_add_change_or_omit_answer_fields() {
        let (bundle,source,_)=parts();
        let origin=iz::context_from_record(&source).unwrap();
        let anchor=shadow_contracts::pd_prepared::PdContinuationLink::from_bundle(&bundle).unwrap();
        let user="motor evidence: alpha, beta.";
        let answer=iz::IzAnswer{positive:"alpha,".into(),..Default::default()};
        let context=native_reply_context(&origin,anchor.clone(),"literal-retry".into(),user.into(),
            Some(&source),answer.clone()).unwrap();
        validate_native_retry(&context,&origin,&anchor,"literal-retry",user,&answer).unwrap();
        for changed in [iz::IzAnswer::default(),
            iz::IzAnswer{positive:"beta.".into(),..Default::default()},
            iz::IzAnswer{positive:"alpha,".into(),negative:"beta.".into(),..Default::default()}] {
            assert_eq!(validate_native_retry(&context,&origin,&anchor,"literal-retry",user,&changed).unwrap_err(),
                "LIGHT_NATIVE_INQUIRY_OPERATION_CONFLICT");
        }
        let empty=native_reply_context(&origin,anchor.clone(),"old-empty".into(),user.into(),
            Some(&source),Default::default()).unwrap();
        assert_eq!(validate_native_retry(&empty,&origin,&anchor,"old-empty",user,&answer).unwrap_err(),
            "LIGHT_NATIVE_INQUIRY_OPERATION_CONFLICT");
        let mut mapped=context;
        mapped.native_map=Some(iz::native_map_for(&mapped.identity,&anchor,Some(&source)).unwrap());
        assert_eq!(iz::validate_context_parent(&mapped,&source).unwrap_err(),"IZ_NATIVE_BASE_HAS_NO_FIELD_FRONTIER",
            "transported annotations cannot open an obligation on a scientific parent");
    }
}

#[cfg(test)]
mod literal_answer_entry_regression {
    use super::*;
    #[test]
    fn literal_answer_option_is_explicit_and_preserves_positional_paths() {
        let args=["light","--iz-lgc-reply-once","op","parent.ref","actual.txt","result.lgc",
            "--answer-fields","fields.json"].map(String::from);
        let parsed=parse_args(&args).expect("optional exact field input must be accepted").unwrap();
        assert_eq!(parsed.input,PathBuf::from("actual.txt"));
        assert_eq!(parsed.output,PathBuf::from("result.lgc"));
        assert_eq!(parsed.reference,Some(PathBuf::from("parent.ref")));
        let mut unknown=args.clone();unknown[6]="--invent-fields".into();
        assert!(parse_args(&unknown).is_err());
    }
}

#[cfg(test)]
mod literal_file_input_regression {
    use super::*;
    #[test]
    fn literal_file_fields_preserve_i0_bytes_and_leave_absence_unchanged() {
        let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root=std::env::temp_dir().join(format!("opl2s-literal-{}-{stamp}",std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let input=root.join("i0.txt");let fields=root.join("fields.json");let output=root.join("out.lgc");
        let text="motor Provë, alpha.";
        std::fs::write(&input,text.as_bytes()).unwrap();
        std::fs::write(&fields,br#"{}"#).unwrap();
        let plain=vec!["light".into(),"--iz-lgc-new-once".into(),"literal-new".into(),
            input.to_str().unwrap().into(),output.to_str().unwrap().into()];
        let mut with_fields=plain.clone();with_fields.extend(["--answer-fields".into(),fields.to_str().unwrap().into()]);
        let original=prepare(parse_args(&plain).unwrap().unwrap()).unwrap().context;
        let empty=prepare(parse_args(&with_fields).unwrap().unwrap()).unwrap().context;
        assert_eq!(iz::encode_context(&original).unwrap(),iz::encode_context(&empty).unwrap());
        std::fs::write(&fields,r#"{"positive":"Provë,"}"#.as_bytes()).unwrap();
        let filled=prepare(parse_args(&with_fields).unwrap().unwrap()).unwrap().context;
        assert_eq!(filled.current_i0(),text);assert_eq!(filled.answer.positive,"Provë,");
        assert_eq!(filled.identity,original.identity);
        let span=filled.answer_spans().unwrap().remove(0);
        assert_eq!(&text.as_bytes()[span.start as usize..span.end as usize],"Provë,".as_bytes());
        assert_ne!(iz::encode_context(&filled).unwrap(),iz::encode_context(&original).unwrap());
        std::fs::write(&fields,br#"{"positive":"absent"}"#).unwrap();
        assert_eq!(prepare(parse_args(&with_fields).unwrap().unwrap()).err().unwrap(),"IZ_ANSWER_NOT_IN_USER_TEXT");
        assert!(!output.exists(),"preparation cannot publish or write knowledge");
        std::fs::remove_file(input).unwrap();std::fs::remove_file(fields).unwrap();std::fs::remove_dir(root).unwrap();
    }
}

#[cfg(test)]
mod interactive_reference_tests {
    use super::*;
    fn parent() -> pd::PdOperationRecord {
        match lgc::decode_output(include_bytes!("../test_fixtures/literal_answer/parent.lgc")).unwrap() {
            lgc::PdLgcOutput::Operational(record)=>record,
            _=>panic!("PIZ parent required"),
        }
    }
    fn envelope(reference:&lgc::PdLgcRef,text:&str,field:&str)->Vec<u8>{
        serde_json::to_vec(&serde_json::json!({
            "operation_id":"interactive-reply",
            "reference":lgc::encode_ref(reference).unwrap(),
            "payload":{"user_text":text,"answer":{"positive":field}}
        })).unwrap()
    }
    #[test]
    fn explicit_reference_keeps_parent_and_exact_literal_bytes_without_a_map_or_authority() {
        let parent=parent();
        let reference=lgc::PdLgcRef::from_record(&parent).unwrap();
        let bytes=envelope(&reference,"motor provë, evidence.","provë,");
        let (operation,selected,answer)=interactive_envelope::<InteractiveAnswer>(&reference,&bytes).unwrap();
        let context=derive_reply(operation,answer.user_text,&selected,&parent,answer.answer.into()).unwrap();
        assert_eq!(context.parent,Some(iz::IzParent::from_record(&parent).unwrap()));
        assert_eq!(context.current_i0(),"motor provë, evidence.");
        assert_eq!(context.answer.positive,"provë,");
        assert!(context.map.is_none() && context.native_map.is_none());
        assert!(context.require_mapped().is_err(),"only the owner may supply the current map");
        let spans=context.answer_spans().unwrap();
        assert_eq!(spans.len(),1);
        assert_eq!(&context.current_i0().as_bytes()[spans[0].start as usize..spans[0].end as usize],"provë,".as_bytes());
    }
    #[test]
    fn envelope_rejects_other_cases_tampered_references_unknown_keys_and_excess_bytes() {
        let reference=lgc::PdLgcRef::from_record(&parent()).unwrap();
        let mut foreign=reference.clone();foreign.identity_key[0]^=1;
        let bytes=envelope(&foreign,"motor evidence","evidence");
        assert_eq!(interactive_envelope::<InteractiveAnswer>(&reference,&bytes).err().unwrap(),"LIGHT_IZ_INTERACTIVE_FOREIGN_CASE");
        let mut value:serde_json::Value=serde_json::from_slice(&envelope(&reference,"motor evidence","evidence")).unwrap();
        value["reference"][100]=serde_json::json!(value["reference"][100].as_u64().unwrap()^1);
        assert_eq!(interactive_envelope::<InteractiveAnswer>(&reference,&serde_json::to_vec(&value).unwrap()).err().unwrap(),"PD_LGC_REF_CHECKSUM");
        let mut value:serde_json::Value=serde_json::from_slice(&envelope(&reference,"motor evidence","evidence")).unwrap();
        value["authority"]=serde_json::json!("granted");
        assert!(interactive_envelope::<InteractiveAnswer>(&reference,&serde_json::to_vec(&value).unwrap()).is_err());
        assert!(interactive_envelope::<InteractiveAnswer>(&reference,b"[]").is_err());
        assert_eq!(interactive_envelope::<InteractiveAnswer>(&reference,&vec![b' ';48*1024+1]).err().unwrap(),"LIGHT_IZ_INTERACTIVE_LIMIT");
    }
    #[test]
    fn interactive_annotation_cannot_invent_a_span_or_disambiguate_repeated_text() {
        let parent=parent();
        let reference=lgc::PdLgcRef::from_record(&parent).unwrap();
        for (text,field,reason) in [
            ("motor evidence","absent","IZ_ANSWER_NOT_IN_USER_TEXT"),
            ("motor same same","same","IZ_ANSWER_SPAN_AMBIGUOUS"),
        ] {
            let (operation,selected,answer)=interactive_envelope::<InteractiveAnswer>(
                &reference,&envelope(&reference,text,field)).unwrap();
            assert_eq!(derive_reply(operation,answer.user_text,&selected,&parent,answer.answer.into()).err().unwrap(),reason);
        }
    }
    #[test]
    fn displayed_reference_is_existing_lgr1_and_never_the_legacy_dialogue_source() {
        let parent=parent();let mut out=Vec::new();
        let reference=display_reference(&parent,&mut out).unwrap();
        let line=std::str::from_utf8(&out).unwrap().trim_end();
        let bytes:Vec<u8>=serde_json::from_str(line.strip_prefix("[PD_IZ_REFERENCE] ").unwrap()).unwrap();
        assert_eq!(lgc::decode_ref(&bytes).unwrap(),reference);
        reference.validate_record(&parent).unwrap();
        assert!(!parent.request.source_input.starts_with(b"PDF1"));
        assert!(!parent.request.source_input.starts_with(b"PDS1"));
    }
}
