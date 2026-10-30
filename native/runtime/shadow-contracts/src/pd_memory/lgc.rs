//! Exact LGC references to existing Vault material.
//! A checksum/reference is integrity data, never caller authentication or authority.
//! No checkpoint, answer or second payload warehouse is stored in a reference.
use super::{Hash, PdOperationRecord, Reader};
use crate::pd_recovery::{self, RecoveryBundle, RecoveryLookup};
use sha2::{Digest, Sha256};

pub const REF_BYTES: usize = 270;
pub const MAX_OUTPUT_BYTES: usize = if pd_recovery::MAX_BUNDLE_BYTES > super::MAX_REQUEST_BYTES + 256 {
    pd_recovery::MAX_BUNDLE_BYTES + 64
} else { super::MAX_REQUEST_BYTES + 320 };
const VERSION: u16 = 1;
fn sha(bytes:&[u8])->Hash {Sha256::digest(bytes).into()}
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct PdLgcRef {
    pub identity_key:Hash,
    pub record_id:Hash,
    pub revision:u64,
    pub state_digest:Hash,
    pub question_sha256:Hash,
    pub lgc_tip:Hash,
    /// Actual user field, not the witness input hash over the entire PIZ frame.
    pub user_input_sha256:Hash,
    pub question_binding:Hash,
}
impl PdLgcRef {
    pub fn validate(&self)->Result<(),String> {
        if self.revision==0 || [&self.identity_key,&self.record_id,&self.state_digest,
            &self.question_sha256,&self.lgc_tip,&self.user_input_sha256,&self.question_binding]
            .iter().any(|h| **h==[0;32]) {return Err("PD_LGC_REF_UNBOUND".into());}
        Ok(())
    }
    pub fn from_record(record:&PdOperationRecord)->Result<Self,String> {
        record.validate()?;
        if super::late_refusal::source_is_late(&record.request.source_input) {
            let source=super::late_refusal::validate_operation(&record.request)?;
            let request=crate::decode_cycle_request(&source.attempt).map_err(|e|e.to_string())?;
            let raw=super::late_refusal::checkpoint_raw(&record.request)?;
            crate::pd_checkpoint_data::validate_checkpoint(raw,&super::hex(&record.request.identity.key()?))?;
            let value=Self{identity_key:record.request.identity.key()?,record_id:record.record_id,
                revision:record.revision,state_digest:record.state_digest,
                question_sha256:sha(record.request.scoped_question.as_bytes()),
                lgc_tip:record.request.witness.lgc_parent,
                // Exact input of the witnessed attempt, not an appended user turn.
                user_input_sha256:sha(&request.quantum.final_evidence.light_input_bytes),
                question_binding:super::parse_hash(&record.operational_question_binding())?};
            value.validate()?;return Ok(value);
        }
        let source=super::iz::validate_operation(&record.request)?;
        let last=record.request.lgc_steps.last().ok_or("PD_LGC_STEPS_EMPTY")?;
        if last.step_digest!=record.request.witness.lgc_parent {
            return Err("PD_LGC_TIP_MISMATCH".into());
        }
        let raw=record.request.checkpoint.strip_prefix(source.context.checkpoint_prefix())
            .ok_or("PD_LGC_CHECKPOINT_POLICY")?;
        crate::pd_checkpoint_data::validate_checkpoint(raw,&super::hex(&record.request.identity.key()?))?;
        let value=Self{identity_key:record.request.identity.key()?,record_id:record.record_id,
            revision:record.revision,state_digest:record.state_digest,
            question_sha256:sha(record.request.scoped_question.as_bytes()),
            lgc_tip:last.step_digest,user_input_sha256:sha(source.context.user_text.as_bytes()),
            question_binding:super::parse_hash(&record.operational_question_binding())?};
        value.validate()?;Ok(value)
    }
    pub fn validate_record(&self,record:&PdOperationRecord)->Result<(),String> {
        self.validate()?;
        if *self!=Self::from_record(record)? {return Err("PD_LGC_EXACT_REFERENCE_MISMATCH".into());}
        Ok(())
    }
}
pub fn encode_ref(value:&PdLgcRef)->Result<Vec<u8>,String> {
    value.validate()?;
    let mut out=b"LGR1".to_vec();out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&value.identity_key);out.extend_from_slice(&value.record_id);
    out.extend_from_slice(&value.revision.to_le_bytes());
    for h in [&value.state_digest,&value.question_sha256,&value.lgc_tip,&value.user_input_sha256,&value.question_binding] {
        out.extend_from_slice(h);
    }
    out.extend_from_slice(&sha(&out));Ok(out)
}
pub fn decode_ref(bytes:&[u8])->Result<PdLgcRef,String> {
    if bytes.len()!=REF_BYTES {return Err("PD_LGC_REF_LENGTH".into());}
    let (body,checksum)=bytes.split_at(bytes.len()-32);
    if sha(body).as_slice()!=checksum {return Err("PD_LGC_REF_CHECKSUM".into());}
    let mut r=Reader{bytes:body,pos:0};
    if r.take(4)?!=b"LGR1" || r.u16()?!=VERSION {return Err("PD_LGC_REF_VERSION".into());}
    let value=PdLgcRef{identity_key:r.hash()?,record_id:r.hash()?,revision:r.u64()?,
        state_digest:r.hash()?,question_sha256:r.hash()?,lgc_tip:r.hash()?,
        user_input_sha256:r.hash()?,question_binding:r.hash()?};
    r.end()?;value.validate()?;Ok(value)
}
/// Explicit client result transport, not a pd_mp payload index.
#[derive(Debug,Clone,PartialEq,Eq)]
pub enum PdLgcOutput {Operational(PdOperationRecord), Native(RecoveryBundle)}
fn validate_native(bundle:&RecoveryBundle)->Result<(),String> {
    let (_,_,continuation)=bundle.validated()?;
    if continuation.is_none() {return Err("PD_LGC_NATIVE_T2_REQUIRED".into());}
    Ok(())
}
pub fn encode_output(value:&PdLgcOutput)->Result<Vec<u8>,String> {
    let (kind,payload)=match value {
        PdLgcOutput::Operational(record)=>{PdLgcRef::from_record(record)?;(0,super::encode_record(record)?)},
        PdLgcOutput::Native(bundle)=>{validate_native(bundle)?;(1,pd_recovery::encode_bundle(bundle)?)},
    };
    let mut out=b"LGO1".to_vec();out.extend_from_slice(&VERSION.to_le_bytes());out.push(kind);
    super::put_bytes(&mut out,&payload,MAX_OUTPUT_BYTES-43)?;
    if out.len().checked_add(32).ok_or("PD_LGC_OUTPUT_OVERFLOW")?>MAX_OUTPUT_BYTES {
        return Err("PD_LGC_OUTPUT_LIMIT".into());
    }
    out.extend_from_slice(&sha(&out));Ok(out)
}
pub fn decode_output(bytes:&[u8])->Result<PdLgcOutput,String> {
    if bytes.len()<43 || bytes.len()>MAX_OUTPUT_BYTES {return Err("PD_LGC_OUTPUT_LIMIT".into());}
    let (body,checksum)=bytes.split_at(bytes.len()-32);
    if sha(body).as_slice()!=checksum {return Err("PD_LGC_OUTPUT_CHECKSUM".into());}
    let mut r=Reader{bytes:body,pos:0};
    if r.take(4)?!=b"LGO1" || r.u16()?!=VERSION {return Err("PD_LGC_OUTPUT_VERSION".into());}
    let tag=r.u8()?;let payload=r.bytes(MAX_OUTPUT_BYTES-43)?;r.end()?;
    let value=match tag {
        0=>{let record=super::decode_record(&payload)?;PdLgcRef::from_record(&record)?;PdLgcOutput::Operational(record)},
        1=>{let bundle=pd_recovery::decode_bundle(&payload)?;validate_native(&bundle)?;PdLgcOutput::Native(bundle)},
        _=>return Err("PD_LGC_OUTPUT_KIND".into()),
    };Ok(value)
}
/// Exactly 0, or 1 followed by the existing 68-byte PDRL lookup frame.
pub fn encode_optional_native_lookup(value:Option<&RecoveryLookup>)->Result<Vec<u8>,String> {
    match value {None=>Ok(vec![0]),Some(value)=>{let mut out=vec![1];out.extend_from_slice(&pd_recovery::encode_lookup(value)?);Ok(out)}}
}
pub fn decode_optional_native_lookup(bytes:&[u8])->Result<Option<RecoveryLookup>,String> {
    match bytes { [0]=>Ok(None), [1,rest @ ..] if rest.len()==68=>pd_recovery::decode_lookup(rest).map(Some),
        _=>Err("PD_LGC_NATIVE_LOOKUP_FRAME".into())}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reference_frame_is_exact_bounded_and_never_authorizes_a_record(){
        // Shape-only data fixture, not a claim of material existence or admission.
        let value=PdLgcRef{identity_key:[1;32],record_id:[2;32],revision:7,state_digest:[3;32],
            question_sha256:[4;32],lgc_tip:[5;32],user_input_sha256:[6;32],question_binding:[7;32]};
        let bytes=encode_ref(&value).unwrap();assert_eq!(bytes.len(),REF_BYTES);
        assert_eq!(decode_ref(&bytes).unwrap(),value);
        for at in [0,4,6,38,70,bytes.len()-1] {let mut bad=bytes.clone();bad[at]^=1;assert!(decode_ref(&bad).is_err());}
        assert!(decode_ref(&bytes[..bytes.len()-1]).is_err());
        let mut extra=bytes.clone();extra.push(0);assert!(decode_ref(&extra).is_err());
        let mut zero=value;zero.revision=0;assert!(encode_ref(&zero).is_err());
    }
    #[test]
    fn lookup_never_falls_back_to_an_implicit_native_identity(){
        let lookup=RecoveryLookup{identity:[1;32],request_sha256:[2;32]};
        let bytes=encode_optional_native_lookup(Some(&lookup)).unwrap();
        assert_eq!(bytes.len(),69);assert_eq!(decode_optional_native_lookup(&bytes).unwrap(),Some(lookup));
        assert_eq!(decode_optional_native_lookup(&[0]).unwrap(),None);
        for bytes in [vec![],vec![0,0],vec![1],vec![2],vec![1;70]] {assert!(decode_optional_native_lookup(&bytes).is_err());}
        assert!(decode_output(&vec![0;MAX_OUTPUT_BYTES+1]).is_err());
        assert!(decode_output(b"LGO1").is_err());
    }
}
