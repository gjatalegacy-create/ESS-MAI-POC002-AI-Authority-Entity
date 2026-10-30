//! data-only envelope for a native cycle and its prepared PD state.
use crate::pd_prepared::{self,PdPreparedEnvelope};
pub const MAX_CYCLE_BYTES:usize=crate::MAX_FRAME_BYTES+pd_prepared::MAX_PREPARED_BYTES+12;
pub fn encode_cycle(request:&[u8],prepared:&PdPreparedEnvelope)->Result<Vec<u8>,String>{
    prepared.validate_for_request(request)?;
    let pending=pd_prepared::encode_prepared(prepared)?;
    if request.len()>crate::MAX_FRAME_BYTES {return Err("PD_NATIVE_FRAME_LIMIT".into());}
    let mut out=b"PDT1".to_vec();
    out.extend_from_slice(&(request.len() as u32).to_le_bytes());
    out.extend_from_slice(request);
    out.extend_from_slice(&(pending.len() as u32).to_le_bytes());
    out.extend_from_slice(&pending);
    Ok(out)
}
pub fn decode_cycle(bytes:&[u8])->Result<(Vec<u8>,PdPreparedEnvelope),String>{
    if bytes.len()>MAX_CYCLE_BYTES || bytes.get(..4)!=Some(b"PDT1") {return Err("PD_NATIVE_FRAME".into());}
    let mut pos=4usize;
    fn field(bytes:&[u8],pos:&mut usize,max:usize)->Result<Vec<u8>,String>{
        let h=bytes.get(*pos..pos.checked_add(4).ok_or("PD_NATIVE_OVERFLOW")?).ok_or("PD_NATIVE_TRUNCATED")?;
        let n=u32::from_le_bytes(h.try_into().map_err(|_|"PD_NATIVE_TRUNCATED")?) as usize;
        *pos+=4;
        if n>max {return Err("PD_NATIVE_FIELD_LIMIT".into());}
        let end=pos.checked_add(n).ok_or("PD_NATIVE_OVERFLOW")?;
        let data=bytes.get(*pos..end).ok_or("PD_NATIVE_TRUNCATED")?.to_vec();*pos=end;Ok(data)
    }
    let request=field(bytes,&mut pos,crate::MAX_FRAME_BYTES)?;
    let pending=field(bytes,&mut pos,pd_prepared::MAX_PREPARED_BYTES)?;
    if pos!=bytes.len(){return Err("PD_NATIVE_TRAILING".into());}
    let envelope=pd_prepared::decode_prepared(&pending)?;
    envelope.validate_for_request(&request)?;
    Ok((request,envelope))
}

#[cfg(test)]
mod tests {
    #[test]
    fn malformed_frames_are_rejected_without_panic() {
        for bytes in [vec![],b"PDT1".to_vec(),b"PDT1\xff\xff\xff\xff".to_vec(),vec![0;20]] {
            assert!(super::decode_cycle(&bytes).is_err());
        }
    }
}

pub fn identity(request:&crate::ShadowCycleRequest)->Result<[u8;32],String>{
    use sha2::{Digest,Sha256};
    let parent=request.quantum.pd_parent_i0.as_deref().ok_or("PD_NATIVE_PARENT")?;
    let anchor=parent.strip_prefix("i0-").and_then(|v|u64::from_str_radix(v,16).ok()).filter(|v|*v!=0).ok_or("PD_NATIVE_ANCHOR")?;
    if parent!=format!("i0-{anchor:016x}") || request.quantum.session_id!=request.light.session_id {return Err("PD_NATIVE_IDENTITY".into());}
    let mut h=Sha256::new();
    for bytes in [b"ESSMAI_SHADOW_IDENTITY_V1".as_slice(),request.quantum.session_id.as_bytes(),parent.as_bytes(),&anchor.to_le_bytes()] { h.update(bytes); }
    Ok(h.finalize().into())
}
pub fn finalize_lookup(identity:[u8;32],request_sha256:[u8;32])->Vec<u8>{
    let mut bytes=b"PDL2".to_vec();bytes.extend_from_slice(&identity);bytes.extend_from_slice(&request_sha256);bytes
}
pub fn decode_finalize_lookup(bytes:&[u8])->Result<([u8;32],[u8;32]),String>{
    if bytes.len()!=68 || &bytes[..4]!=b"PDL2" {return Err("PD_FINALIZE_LOOKUP".into());}
    Ok((bytes[4..36].try_into().map_err(|_|"PD_FINALIZE_LOOKUP")?,bytes[36..68].try_into().map_err(|_|"PD_FINALIZE_LOOKUP")?))
}
