//! Explicit historical reference; no authority or implicit latest topic.
use crate::pd_recovery::{self, RecoveryLookup};
pub fn encode_reference(lookup: &RecoveryLookup)->Result<String,String> {
 Ok(pd_recovery::encode_lookup(lookup)?.iter().map(|b|format!("{b:02x}")).collect())
}
pub fn decode_reference(value:&str)->Result<RecoveryLookup,String> {
 if value.len()!=136 || !value.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b)) {
  return Err("PD_FOLLOWUP_REFERENCE_ENCODING".into());
 }
 let mut bytes=Vec::with_capacity(68);
 for i in (0..136).step_by(2) {bytes.push(u8::from_str_radix(&value[i..i+2],16).map_err(|_|"PD_FOLLOWUP_REFERENCE_ENCODING")?);}
 pd_recovery::decode_lookup(&bytes)
}
#[cfg(test)] mod tests {
 use super::*;
 #[test] fn exact_reference_and_noncanonical_refusal(){
 let q=RecoveryLookup{identity:[1;32],request_sha256:[2;32]};
 let s=encode_reference(&q).unwrap();assert_eq!(decode_reference(&s).unwrap(),q);
 assert!(decode_reference(&(s.clone()+"0")).is_err());
 assert!(decode_reference(&s.to_ascii_uppercase()).is_err());
 }
}
