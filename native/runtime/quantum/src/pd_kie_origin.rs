//! R3: restore the existing Quantum PD from a real Shadow PDS1 record.
//! This is a transport/restore adapter, never a second PD or an evidence store.
use crate::progressive_debatic::ProgressiveDebatic;
use shadow_contracts::{kie::KieContext,pd_memory::{self}};
use std::time::Duration;

pub fn restore(context:&KieContext,territory:&str,now:u64)->Result<ProgressiveDebatic,String>{
    context.validate()?;
    if territory!=context.identity.scope || territory!=context.identity.domain {
        return Err("KIE_PD_SOURCE_TERRITORY".into());
    }
    let request=pd_memory::encode_lookup(&context.lookup())?;
    // Predecessor-directed trigger carries the borrowed exact lookup; no authority.
    let pulse=ess_mai_zero_noi::trigger(request.as_slice());
    let raw=crate::pd_operational::native_transport("--pd-head-once",pulse.request(),Duration::from_secs(90))?;
    let record=pd_memory::decode_optional_record(&raw)?.ok_or("KIE_PD_SOURCE_MISSING")?;
    record.validate()?;
    if record.record_id!=context.record_id || record.request.identity!=context.identity || record.request.operation_id!=context.operation_id {
        return Err("KIE_PD_SOURCE_IDENTITY".into());
    }
    let checkpoint=pd_memory::linked_science::science_checkpoint_raw(&record)?;
    let session_id=pd_memory::hex(&context.identity.key()?);
    let mut pd=ProgressiveDebatic::new();
    pd.import_checkpoint(&session_id,checkpoint)?;
    let state=pd.session(&session_id).ok_or("KIE_PD_SOURCE_SESSION")?;
    if !pd.active || !state.active || state.territory!=territory || now<state.last_activity_at || now<state.started_at {
        return Err("KIE_PD_SOURCE_STATE_OR_CLOCK".into());
    }
    Ok(pd)
}

/// PDS3 carries the exact historical native predecessor; no implicit latest-topic lookup.
pub fn native_anchor(context:&KieContext)->Result<Option<shadow_contracts::pd_prepared::PdContinuationLink>,String>{
    context.validate()?;
    let request=pd_memory::encode_lookup(&context.lookup())?;
    let pulse=ess_mai_zero_noi::trigger(request.as_slice());
    let raw=crate::pd_operational::native_transport("--pd-head-once",pulse.request(),Duration::from_secs(90))?;
    let record=pd_memory::decode_optional_record(&raw)?.ok_or("KIE_PD_SOURCE_MISSING")?;
    record.validate()?;
    if record.record_id!=context.record_id || record.request.identity!=context.identity || record.request.operation_id!=context.operation_id {
        return Err("KIE_PD_SOURCE_IDENTITY".into());
    }
    pd_memory::linked_science::validate_science_record(&record)?;
    if pd_memory::iz_science::source_is_native_science(&record.request.source_input) {
        Ok(pd_memory::iz_science::decode_input(&record.request.source_input)?.context.native_anchor)
    } else { Ok(None) }
}
