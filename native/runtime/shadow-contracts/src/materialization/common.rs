// Source: ess-mai-dbab-zero-noi/src/common.rs (Bledar Gjata / Gjata Legacy,
// Apache-2.0), integrated into AIGovern-Cyber.
// Provenance and adaptations: ../../../SOURCE_NOTES.md.
//! Typed identity, deterministic hashing, and an explicit external trust boundary.
//! SHA-256 proves content identity, NOT signer identity, authority, or truth.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_ITEMS: usize = 64;
pub const MAX_TEXT_BYTES: usize = 4096;
pub const MAX_REFERENCES: usize = 8;
pub const MAX_RECORDS: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Digest32(pub [u8; 32]);

impl Digest32 {
    pub fn of_bytes(bytes: &[u8]) -> Self { Self(Sha256::digest(bytes).into()) }
    pub fn is_zero(self) -> bool { self.0 == [0; 32] }
    pub fn hex(self) -> String { self.0.iter().map(|b| format!("{b:02x}")).collect() }
}

/// Domain-separated deterministic JSON over typed structs/BTreeMaps only.
/// Vector order is meaningful; this is not a general-purpose JSON canonicalizer.
pub fn digest<T: Serialize + ?Sized>(domain: &str, value: &T) -> Result<Digest32, EvidenceError> {
    let bytes = serde_json::to_vec(value).map_err(|_| EvidenceError::Encoding)?;
    let mut h = Sha256::new();
    h.update((domain.len() as u64).to_le_bytes()); h.update(domain.as_bytes());
    h.update((bytes.len() as u64).to_le_bytes()); h.update(bytes);
    Ok(Digest32(h.finalize().into()))
}

pub fn valid_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128
        && value.bytes().all(|b| b.is_ascii_alphanumeric() || b"_-.:/".contains(&b))
}
pub fn valid_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_TEXT_BYTES
        && !value.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaseIdentity {
    pub cycle_id: String,
    pub i0_digest: Digest32,
    pub scope: String,
    pub revision: u64,
    pub law_digest: Digest32,
}
impl CaseIdentity {
    pub fn validate(&self) -> Result<(), EvidenceError> {
        if !valid_id(&self.cycle_id) || !valid_id(&self.scope) || self.i0_digest.is_zero() || self.law_digest.is_zero() {
            return Err(EvidenceError::InvalidIdentity);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceRole { ConfrontationRequirement, Component, Condition, Circumstance, Consequence, ConfrontationManifest, Fulfilment }

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRef { pub record_id: String, pub record_digest: Digest32 }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRecord {
    pub record_id: String,
    pub case: CaseIdentity,
    pub role: EvidenceRole,
    pub issuer: String,
    pub claim_digest: Digest32,
    pub issued_at: u64,
    pub expires_at: u64,
}
impl EvidenceRecord {
    pub fn reference(&self) -> Result<EvidenceRef, EvidenceError> {
        Ok(EvidenceRef { record_id: self.record_id.clone(), record_digest: digest("ESS-MAI/evidence-record/v1", self)? })
    }
    pub fn validate(&self) -> Result<(), EvidenceError> {
        self.case.validate()?;
        if !valid_id(&self.record_id) || !valid_id(&self.issuer) || self.claim_digest.is_zero()
            || self.expires_at <= self.issued_at {
            return Err(EvidenceError::InvalidRecord);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceError {
    InvalidIdentity, InvalidRecord, Encoding, ResourceLimit, DuplicateRecord, MissingRecord,
    UnknownIssuer, DigestMismatch, CaseMismatch, RoleMismatch, ClaimMismatch, NotYetValid, Expired,
    InvalidInput, UpstreamUnavailable,
}
impl std::fmt::Display for EvidenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { write!(f,"{self:?}") }
}
impl std::error::Error for EvidenceError {}

/// Trusted host adapter: it MUST authenticate/authorize its source before returning Ok.
/// An implementation that blindly returns Ok invalidates the security claim of the host.
/// This read-only interface supplies no persistence, selection, or verdict capability.
pub trait EvidenceVerifier {
    fn verify(&self, reference: &EvidenceRef, case: &CaseIdentity, role: EvidenceRole,
        claim: Digest32, now: u64) -> Result<(), EvidenceError>;
}

/// In-memory view of records ALREADY approved by the host. This is not a signature verifier.
/// CLI fixtures explicitly use this policy boundary; production must authenticate its registry.
#[derive(Debug)]
pub struct EvidenceCatalog {
    records: BTreeMap<String, EvidenceRecord>,
    trusted_issuers: BTreeSet<String>,
}
impl EvidenceCatalog {
    pub fn new(records: Vec<EvidenceRecord>, trusted_issuers: BTreeSet<String>) -> Result<Self, EvidenceError> {
        if records.len() > MAX_RECORDS || trusted_issuers.is_empty() || trusted_issuers.len() > MAX_ITEMS {
            return Err(EvidenceError::ResourceLimit);
        }
        if trusted_issuers.iter().any(|id| !valid_id(id)) { return Err(EvidenceError::InvalidIdentity); }
        let mut map = BTreeMap::new();
        for record in records {
            record.validate()?;
            if !trusted_issuers.contains(&record.issuer) { return Err(EvidenceError::UnknownIssuer); }
            if map.insert(record.record_id.clone(),record).is_some() { return Err(EvidenceError::DuplicateRecord); }
        }
        Ok(Self { records: map, trusted_issuers })
    }
}
impl EvidenceVerifier for EvidenceCatalog {
    fn verify(&self, reference: &EvidenceRef, case: &CaseIdentity, role: EvidenceRole,
        claim: Digest32, now: u64) -> Result<(), EvidenceError>
    {
        case.validate()?;
        if !valid_id(&reference.record_id) || reference.record_digest.is_zero() { return Err(EvidenceError::InvalidRecord); }
        let r = self.records.get(&reference.record_id).ok_or(EvidenceError::MissingRecord)?;
        if !self.trusted_issuers.contains(&r.issuer) { return Err(EvidenceError::UnknownIssuer); }
        if r.reference()? != *reference { return Err(EvidenceError::DigestMismatch); }
        if r.case != *case { return Err(EvidenceError::CaseMismatch); }
        if r.role != role { return Err(EvidenceError::RoleMismatch); }
        if r.claim_digest != claim { return Err(EvidenceError::ClaimMismatch); }
        if now < r.issued_at { return Err(EvidenceError::NotYetValid); }
        if now >= r.expires_at { return Err(EvidenceError::Expired); }
        Ok(())
    }
}

pub fn verify_references(verifier: &dyn EvidenceVerifier, refs: &[EvidenceRef], case: &CaseIdentity,
    role: EvidenceRole, claim: Digest32, now: u64) -> Result<(), EvidenceError>
{
    if refs.is_empty() { return Err(EvidenceError::MissingRecord); }
    if refs.len() > MAX_REFERENCES { return Err(EvidenceError::ResourceLimit); }
    let mut seen = BTreeSet::new();
    for r in refs {
        if !seen.insert(&r.record_id) { return Err(EvidenceError::DuplicateRecord); }
        verifier.verify(r,case,role,claim,now)?;
    }
    Ok(())
}
