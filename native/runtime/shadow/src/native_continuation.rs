//! T2 is derived exclusively from an already durable T1.
//! No caller supplies a verdict, receipt, authority token, output tuple or question.
use crate::vault_transaction::{DurableCommit,VaultError};
use shadow_contracts::pd_prepared as pd;
use sha2::{Digest,Sha256};
pub(crate) const MAX_RECORD:usize=256*1024;
#[derive(Debug,Clone,PartialEq,Eq)]
pub struct NativeContinuationRecord {
    pub(crate) identity:[u8;32],
    pub(crate) request_sha256:[u8;32],
    pub(crate) transaction_id:[u8;32],
    pub(crate) material:Vec<u8>,
}
impl NativeContinuationRecord {
    pub(crate) fn derive(identity:[u8;32],request_sha256:[u8;32],commit:&DurableCommit)->Result<Self,String>{
        let snapshot=commit.native_cycle.as_ref().ok_or("PD_T1_SNAPSHOT_MISSING")?;
        if snapshot.request_sha256!=request_sha256 {return Err("PD_T1_REQUEST_CONFLICT".into());}
        let envelope=pd::decode_prepared(&snapshot.prepared)?;
        let response=shadow_contracts::decode_cycle_response(&snapshot.response).map_err(|e|e.to_string())?;
        if crate::native_cycle::identity_digest(&response.session_id,&response.verification_receipt.parent_i0,response.verification_receipt.primitive_anchor)!=identity {return Err("PD_T1_IDENTITY_CONFLICT".into());}
        let continuation=pd::derive_continuation(&envelope,&response)?;
        let material=pd::encode_continuation(&continuation)?;
        let record=Self {identity,request_sha256,transaction_id:commit.transaction_id,material};
        record.encode()?;
        Ok(record)
    }
    pub(crate) fn encode(&self)->Result<Vec<u8>,String>{
        if self.identity==[0;32] || self.request_sha256==[0;32] || self.transaction_id==[0;32] || self.material.len()>MAX_RECORD-136 {return Err("PD_T2_SHAPE".into());}
        pd::decode_continuation(&self.material)?;
        let mut out=b"PDT2".to_vec();
        out.extend_from_slice(&self.identity);
        out.extend_from_slice(&self.request_sha256);
        out.extend_from_slice(&self.transaction_id);
        out.extend_from_slice(&(self.material.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.material);
        let hash=Sha256::digest(&out);out.extend_from_slice(&hash);Ok(out)
    }
    pub(crate) fn decode(bytes:&[u8])->Result<Self,String>{
        if bytes.len()<136 || bytes.len()>MAX_RECORD || &bytes[..4]!=b"PDT2" {return Err("PD_T2_SHAPE".into());}
        let n=u32::from_le_bytes(bytes[100..104].try_into().map_err(|_|"PD_T2_LENGTH")?) as usize;
        if n!=bytes.len()-136 || Sha256::digest(&bytes[..bytes.len()-32]).as_slice()!=&bytes[bytes.len()-32..] {return Err("PD_T2_INTEGRITY".into());}
        let record=Self {identity:bytes[4..36].try_into().map_err(|_|"PD_T2_IDENTITY")?,
            request_sha256:bytes[36..68].try_into().map_err(|_|"PD_T2_REQUEST")?,
            transaction_id:bytes[68..100].try_into().map_err(|_|"PD_T2_TRANSACTION")?,
            material:bytes[104..104+n].to_vec()};
        record.encode()?;
        Ok(record)
    }
    pub(crate) fn validate_against(&self,commit:&DurableCommit)->Result<(),VaultError>{
        let expected=Self::derive(self.identity,self.request_sha256,commit).map_err(VaultError::CorruptTransaction)?;
        if self!=&expected {return Err(VaultError::CorruptTransaction("PD_T2_DERIVATION_MISMATCH".into()));}
        Ok(())
    }
}

#[cfg(test)]
#[path = "native_continuation_tests.rs"]
mod tests;
