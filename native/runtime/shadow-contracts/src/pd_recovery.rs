//! NEW: bounded historical recovery data under stored Shadow authority.
//! This codec does not authorize a request or mint a receipt. A production caller
//! must obtain the bundle from Shadow's exact durable T1/T2 lookup.
//! Source session/parent are preserved; stable topic selection is explicit and
//! must never be guessed from the last conversation or a new physical anchor.
use crate::pd_prepared::{self, ContinuationData, Hash, PdPreparedEnvelope};
use crate::{Reader, Writer};
use sha2::{Digest, Sha256};

pub const RECOVERY_VERSION: u16 = 1;
pub const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
pub const MAX_BUNDLE_BYTES: usize = pd_prepared::MAX_PREPARED_BYTES
    + MAX_RESPONSE_BYTES
    + pd_prepared::MAX_CONTINUATION_BYTES
    + 256;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryLookup {
    pub identity: Hash,
    pub request_sha256: Hash,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryBundle {
    pub version: u16,
    pub identity: Hash,
    pub request_sha256: Hash,
    /// Actual T1 transaction identity selected by Shadow; not caller authorization.
    pub transaction_id: Hash,
    pub prepared: Vec<u8>,
    pub response: Vec<u8>,
    /// Exact durable T2 material. None means the cycle is not releasable yet.
    pub continuation: Option<Vec<u8>>,
}
fn require(ok: bool, reason: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(format!("PD_RECOVERY_{reason}"))
    }
}
fn sha(bytes: &[u8]) -> Hash {
    Sha256::digest(bytes).into()
}
fn hash(r: &mut Reader<'_>) -> Result<Hash, crate::WireError> {
    let mut value = [0; 32];
    value.copy_from_slice(r.take(32)?);
    Ok(value)
}
impl RecoveryLookup {
    pub fn validate(&self) -> Result<(), String> {
        require(
            self.identity != [0; 32] && self.request_sha256 != [0; 32],
            "LOOKUP_EMPTY",
        )
    }
}
/// Exact source Shadow identity formula; no topic/attempt alias is introduced.
pub fn native_identity(session: &str, parent: &str, anchor: u64) -> Result<Hash, String> {
    require(
        !session.is_empty()
            && session.len() <= 256
            && anchor != 0
            && parent == format!("i0-{anchor:016x}"),
        "NATIVE_IDENTITY",
    )?;
    let mut h = Sha256::new();
    for p in [
        b"ESSMAI_SHADOW_IDENTITY_V1".as_slice(),
        session.as_bytes(),
        parent.as_bytes(),
        &anchor.to_le_bytes(),
    ] {
        h.update(p);
    }
    Ok(h.finalize().into())
}
impl RecoveryBundle {
    /// Decode and bind historical data only. Authentic storage selection remains
    /// exclusively the Shadow endpoint's obligation.
    pub fn validated(
        &self,
    ) -> Result<
        (
            PdPreparedEnvelope,
            crate::ShadowCycleResponse,
            Option<ContinuationData>,
        ),
        String,
    > {
        require(self.version == RECOVERY_VERSION, "VERSION")?;
        require(
            self.identity != [0; 32]
                && self.request_sha256 != [0; 32]
                && self.transaction_id != [0; 32],
            "EMPTY_BINDING",
        )?;
        require(
            self.prepared.len() <= pd_prepared::MAX_PREPARED_BYTES
                && self.response.len() <= MAX_RESPONSE_BYTES,
            "FIELD_LIMIT",
        )?;
        let prepared = pd_prepared::decode_prepared(&self.prepared)?;
        let response = crate::decode_cycle_response(&self.response)
            .map_err(|e| format!("PD_RECOVERY_RESPONSE: {e}"))?;
        require(
            crate::encode_cycle_response(&response) == self.response,
            "RESPONSE_NON_CANONICAL",
        )?;
        let r = &response.verification_receipt;
        require(
            prepared.source_request_sha256 == self.request_sha256
                && native_identity(&response.session_id, &r.parent_i0, r.primitive_anchor)?
                    == self.identity,
            "SOURCE_BINDING",
        )?;
        // All future finalization preconditions must hold even if durable T2 is absent.
        let expected = pd_prepared::derive_continuation(&prepared, &response)?;
        let continuation = match &self.continuation {
            Some(bytes) => {
                require(
                    bytes.len() <= pd_prepared::MAX_CONTINUATION_BYTES,
                    "CONTINUATION_LIMIT",
                )?;
                let parsed = pd_prepared::decode_continuation(bytes)?;
                require(parsed == expected, "CONTINUATION_MISMATCH")?;
                Some(parsed)
            }
            None => None,
        };
        Ok((prepared, response, continuation))
    }
    pub fn validate_for(&self, lookup: &RecoveryLookup) -> Result<(), String> {
        lookup.validate()?;
        require(
            self.identity == lookup.identity && self.request_sha256 == lookup.request_sha256,
            "LOOKUP_MISMATCH",
        )?;
        self.validated().map(|_| ())
    }
}
pub fn encode_lookup(lookup: &RecoveryLookup) -> Result<Vec<u8>, String> {
    lookup.validate()?;
    let mut out = b"PDRL".to_vec();
    out.extend_from_slice(&lookup.identity);
    out.extend_from_slice(&lookup.request_sha256);
    Ok(out)
}
pub fn decode_lookup(bytes: &[u8]) -> Result<RecoveryLookup, String> {
    require(
        bytes.len() == 68 && bytes.get(..4) == Some(b"PDRL"),
        "LOOKUP_FRAME",
    )?;
    let lookup = RecoveryLookup {
        identity: bytes[4..36]
            .try_into()
            .map_err(|_| "PD_RECOVERY_IDENTITY")?,
        request_sha256: bytes[36..68]
            .try_into()
            .map_err(|_| "PD_RECOVERY_REQUEST")?,
    };
    lookup.validate()?;
    Ok(lookup)
}
pub fn encode_bundle(bundle: &RecoveryBundle) -> Result<Vec<u8>, String> {
    bundle.validated()?;
    let mut w = Writer::new();
    w.bytes.extend_from_slice(b"ESSPDR01");
    w.u16(bundle.version);
    w.bytes.extend_from_slice(&bundle.identity);
    w.bytes.extend_from_slice(&bundle.request_sha256);
    w.bytes.extend_from_slice(&bundle.transaction_id);
    w.bytes(&bundle.prepared);
    w.bytes(&bundle.response);
    match &bundle.continuation {
        None => w.u8(0),
        Some(bytes) => {
            w.u8(1);
            w.bytes(bytes);
        }
    }
    require(w.bytes.len() <= MAX_BUNDLE_BYTES - 32, "ENCODE_LIMIT")?;
    let checksum = sha(&w.bytes);
    w.bytes.extend_from_slice(&checksum);
    Ok(w.bytes)
}
pub fn decode_bundle(bytes: &[u8]) -> Result<RecoveryBundle, String> {
    require(
        bytes.len() >= 8 + 2 + 96 + 8 + 1 + 32 && bytes.len() <= MAX_BUNDLE_BYTES,
        "DECODE_LIMIT",
    )?;
    let (body, checksum) = bytes.split_at(bytes.len() - 32);
    require(sha(body).as_slice() == checksum, "CHECKSUM")?;
    let bundle = (|| -> Result<RecoveryBundle, crate::WireError> {
        let mut r = Reader::new(body);
        if r.take(8)? != b"ESSPDR01" {
            return Err(crate::WireError::InvalidMagic);
        }
        let version = r.u16()?;
        let identity = hash(&mut r)?;
        let request_sha256 = hash(&mut r)?;
        let transaction_id = hash(&mut r)?;
        let prepared = r.bytes()?;
        let response = r.bytes()?;
        let continuation = match r.u8()? {
            0 => None,
            1 => Some(r.bytes()?),
            _ => return Err(crate::WireError::InvalidMagic),
        };
        r.finish()?;
        Ok(RecoveryBundle {
            version,
            identity,
            request_sha256,
            transaction_id,
            prepared,
            response,
            continuation,
        })
    })()
    .map_err(|e| format!("PD_RECOVERY_CODEC: {e:?}"))?;
    bundle.validated()?;
    require(encode_bundle(&bundle)? == bytes, "NON_CANONICAL")?;
    Ok(bundle)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_lookup_never_selects_an_implicit_context() {
        for b in [vec![], vec![0; 68], b"PDRL".to_vec()] {
            assert!(decode_lookup(&b).is_err());
        }
        let value = RecoveryLookup {
            identity: [1; 32],
            request_sha256: [2; 32],
        };
        assert_eq!(
            decode_lookup(&encode_lookup(&value).unwrap()).unwrap(),
            value
        );
        let mut bytes = encode_lookup(&value).unwrap();
        bytes.push(0);
        assert!(decode_lookup(&bytes).is_err());
    }
    #[test]
    fn invalid_recovery_frames_are_bounded() {
        for bytes in [vec![], vec![0; 147], vec![0; MAX_BUNDLE_BYTES + 1]] {
            assert!(decode_bundle(&bytes).is_err());
        }
    }
}
