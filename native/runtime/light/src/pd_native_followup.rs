//! Light only carries an explicit native-history reference.
use shadow_contracts::pd_recovery::{self as recovery,RecoveryLookup,RecoveryBundle};
use crate::pd_operational_bridge::{self as bridge,NativePeer};
pub fn initial(args:&[String])->Result<Option<RecoveryLookup>,String>{
 if args.get(1).map(String::as_str)!=Some("--pd-resume-native"){return Ok(None);}
 if args.len()!=3 {return Err("LIGHT_PD_RESUME_USAGE: --pd-resume-native LOOKUP_FILE".into());}
 let bytes=bridge::read_bounded(std::path::Path::new(&args[2]),68)?;
 let lookup=recovery::decode_lookup(&bytes)?;
 fetch(&lookup)?; // exact Shadow ownership before starting a new attempt
 Ok(Some(lookup))
}
pub fn fetch(lookup:&RecoveryLookup)->Result<RecoveryBundle,String>{
    let delivery_scope = shadow_contracts::pd_memory::hex(&lookup.identity);
    crate::noi_caller::invoke("LIGHT_NATIVE_RECOVERY", &delivery_scope, || {
 let bytes=bridge::native_exchange(NativePeer::Shadow,"--pd-recover-once",
    &recovery::encode_lookup(lookup)?,recovery::MAX_BUNDLE_BYTES)?;
 let bundle=recovery::decode_bundle(&bytes)?;bundle.validate_for(lookup)?;
 if bundle.continuation.is_none(){return Err("LIGHT_PD_RESUME_T2_MISSING".into());}
 Ok(bundle)

    })
}
pub fn bind_completed(result:&crate::quantum_bridge::QuantumResult,trace:&str,territory:&str)->Result<RecoveryLookup,String>{
 let lookup=result.pd_continuity.clone().ok_or("LIGHT_PD_NATIVE_REFERENCE_MISSING")?;
 let bundle=fetch(&lookup)?;
 let (prepared,response,_)=bundle.validated()?;
 if response.verification_receipt.session_id!=trace || prepared.pending.session_id!=trace {
  return Err("LIGHT_PD_NATIVE_REFERENCE_ATTEMPT".into());
 }
 // Read exact source checkpoint territory through validated historical data.
 let metadata=shadow_contracts::pd_checkpoint_data::checkpoint_metadata(&prepared.checkpoint,prepared.cognitive_session_id())?;
 if metadata.territory!=territory {return Err("LIGHT_PD_NATIVE_REFERENCE_TERRITORY".into());}
 Ok(lookup)
}
