//! Offline, scope-bound scientific candidates. This crate has no acceptance,
//! Vault-writing, Shadow-verdict, network, or P/N-knowledge authority.
//! NEW adapter; vendored sofars is exact third-party code.
pub mod exact_tests;
pub mod kie; // NEW typed model-only proposition rendering and predicates.

use sha2::{Digest, Sha256};
use std::{error::Error, f64::consts::{PI, TAU}, fmt};

pub const MAX_WIRE_BYTES: usize = 16 * 1024;
pub const MAX_IDENTITY_BYTES: usize = 256;
const MAX_EXACT_INTEGER: f64 = 9_007_199_254_740_992.0;
const QUERY_MAGIC: &[u8] = b"ESS-MAI/SCIENCE/QUERY/1\0";
const CANDIDATE_MAGIC: &[u8] = b"ESS-MAI/SCIENCE/CANDIDATE/1\0";
pub const PROVIDER_VERSION: &str = "0.6.1";
pub const PROVIDER_COMMIT: &str = "1cb4bdf1f84176ba0128bfa3aa82c205e75716e7";
pub const PROVIDER_ARCHIVE_SHA256: &str =
    "5dd1c9b1b5d34e1e949deb0dac9ad612e20746c59eaa737e52fd29703c19ec5c";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdapterId { Physics, Astronomy, History, SocialScience, Psychology }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeScale { Ut1, Utc, Tai, Tt, Tdb }
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScienceTask {
    EarthRotationAngle {
        jd_part1: f64,
        jd_part2: f64,
        time_scale: TimeScale,
        claim_radians: f64,
        tolerance_radians: f64,
    },
}
#[derive(Clone, Debug, PartialEq)]
pub struct ScienceQuery {
    pub adapter: AdapterId,
    pub session_id: String,
    pub case_id: String,
    pub scope_id: String,
    pub obligation_id: String,
    pub query_id: String,
    /// Caller-supplied input lineage. This is bound, not independently authenticated.
    pub input_source_id: String,
    pub task: ScienceTask,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelRelation { SupportsClaim, CountersClaim, Unresolved }
/// A computed candidate can never confer constitutional or knowledge authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateAuthority { None }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderProvenance {
    pub model_id: String,
    pub provider: String,
    pub version: String,
    pub commit: String,
    pub archive_sha256: String,
    pub repository: String,
    pub source_path: String,
    pub source_sha256: String,
    /// Hash of exact compiled ERA, constants, and normalization source bytes.
    pub compiled_model_sha256: [u8; 32],
    pub primary_release: String,
    pub primary_source_url: String,
    pub primary_archive_sha256: String,
    pub primary_source_path: String,
    pub primary_source_sha256: String,
    pub license: String,
    pub license_notice_path: String,
    pub input_time_scale: String,
    pub output_unit: String,
    pub limitations: Vec<String>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct CandidateEvidence {
    pub query: ScienceQuery,
    pub computed_radians: f64,
    /// Shortest computed-minus-claimed signed arc in (-pi, pi].
    pub signed_residual_radians: f64,
    pub circular_residual_radians: f64,
    pub relation: ModelRelation,
    pub query_sha256: [u8; 32],
    /// SHA256 of canonical candidate body, excluding this digest itself.
    pub result_sha256: [u8; 32],
    pub provenance: ProviderProvenance,
    pub authority: CandidateAuthority,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScienceError {
    Unavailable { adapter: AdapterId },
    InvalidIdentity { field: &'static str },
    NonFinite { field: &'static str },
    WrongTimeScale { supplied: TimeScale },
    OutOfDomain { field: &'static str },
    InvalidTolerance,
    InvalidComputation,
    Oversized,
    Malformed(&'static str),
    NonCanonical,
    CandidateMismatch,
}
impl fmt::Display for ScienceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable { adapter } => write!(f, "no installed scientific provider for {adapter:?}"),
            Self::InvalidIdentity { field } => write!(f, "invalid scoped identity: {field}"),
            Self::NonFinite { field } => write!(f, "nonfinite numeric input: {field}"),
            Self::WrongTimeScale { supplied } => write!(f, "ERA requires UT1, received {supplied:?}"),
            Self::OutOfDomain { field } => write!(f, "outside adapter numeric domain: {field}"),
            Self::InvalidTolerance => f.write_str("tolerance must be finite and in (0, pi] radians"),
            Self::InvalidComputation => f.write_str("provider returned an invalid angle"),
            Self::Oversized => f.write_str("scientific record exceeds the bounded wire size"),
            Self::Malformed(reason) => write!(f, "malformed scientific record: {reason}"),
            Self::NonCanonical => f.write_str("scientific record is not canonical"),
            Self::CandidateMismatch => f.write_str("candidate does not match its scoped computation and provenance"),
        }
    }
}
impl Error for ScienceError {}

fn zero(value: f64) -> f64 { if value == 0.0 { 0.0 } else { value } }
fn sha(bytes: &[u8]) -> [u8; 32] { Sha256::digest(bytes).into() }
pub fn hash_hex(hash: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(64);
    for byte in hash {
        value.push(HEX[usize::from(byte >> 4)] as char);
        value.push(HEX[usize::from(byte & 15)] as char);
    }
    value
}
fn identity(value: &str, field: &'static str) -> Result<(), ScienceError> {
    if value.is_empty() || value.len() > MAX_IDENTITY_BYTES || value.trim() != value
        || value.chars().any(char::is_control) {
        return Err(ScienceError::InvalidIdentity { field });
    }
    Ok(())
}
fn normalized(query: &ScienceQuery) -> Result<ScienceQuery, ScienceError> {
    for (value, field) in [
        (&query.session_id, "session_id"), (&query.case_id, "case_id"),
        (&query.scope_id, "scope_id"), (&query.obligation_id, "obligation_id"),
        (&query.query_id, "query_id"), (&query.input_source_id, "input_source_id"),
    ] { identity(value, field)?; }
    let ScienceTask::EarthRotationAngle {
        jd_part1, jd_part2, time_scale, claim_radians, tolerance_radians,
    } = query.task;
    for (value, field) in [
        (jd_part1, "jd_part1"), (jd_part2, "jd_part2"),
        (claim_radians, "claim_radians"), (tolerance_radians, "tolerance_radians"),
    ] {
        if !value.is_finite() { return Err(ScienceError::NonFinite { field }); }
    }
    if time_scale != TimeScale::Ut1 {
        return Err(ScienceError::WrongTimeScale { supplied: time_scale });
    }
    // This is a numerical representability guard, not a physical validity epoch.
    if jd_part1.abs() > MAX_EXACT_INTEGER || jd_part2.abs() > MAX_EXACT_INTEGER
        || (jd_part1 + jd_part2).abs() > MAX_EXACT_INTEGER {
        return Err(ScienceError::OutOfDomain { field: "two_part_julian_date" });
    }
    if !(0.0..TAU).contains(&claim_radians) {
        return Err(ScienceError::OutOfDomain { field: "claim_radians" });
    }
    if tolerance_radians <= 0.0 || tolerance_radians > PI {
        return Err(ScienceError::InvalidTolerance);
    }
    let mut result = query.clone();
    result.task = ScienceTask::EarthRotationAngle {
        jd_part1: zero(jd_part1), jd_part2: zero(jd_part2),
        time_scale, claim_radians: zero(claim_radians), tolerance_radians,
    };
    Ok(result)
}
fn compiled_model_hash() -> [u8; 32] {
    let sources: [(&str, &[u8]); 3] = [
        ("src/erst/era00.rs", include_bytes!("../../../vendor/sofars-0.6.1/src/erst/era00.rs")),
        ("src/consts.rs", include_bytes!("../../../vendor/sofars-0.6.1/src/consts.rs")),
        ("src/vm/anp.rs", include_bytes!("../../../vendor/sofars-0.6.1/src/vm/anp.rs")),
    ];
    let mut hasher = Sha256::new();
    hasher.update(b"ESS-MAI/SCIENCE/COMPILED-MODEL/1\0");
    for (path, source) in sources {
        hasher.update((path.len() as u32).to_le_bytes());
        hasher.update(path.as_bytes());
        hasher.update((source.len() as u64).to_le_bytes());
        hasher.update(source);
    }
    hasher.finalize().into()
}

pub fn provider_provenance() -> ProviderProvenance {
    ProviderProvenance {
        model_id: "earth-rotation-angle/iau-2000/ess-mai-adapter-v1".into(),
        provider: "sofars::erst::era00".into(),
        version: PROVIDER_VERSION.into(),
        commit: PROVIDER_COMMIT.into(),
        archive_sha256: PROVIDER_ARCHIVE_SHA256.into(),
        repository: "https://github.com/astro-xao/sofars".into(),
        source_path: "vendor/sofars-0.6.1/src/erst/era00.rs".into(),
        source_sha256: hash_hex(&sha(include_bytes!(
            "../../../vendor/sofars-0.6.1/src/erst/era00.rs"))),
        compiled_model_sha256: compiled_model_hash(),
        primary_release: "IAU SOFA 2023-10-11 C release; ERA IAU 2000 model".into(),
        primary_source_url: "https://www.iausofa.org/s/sofa_c-20231011tar.gz".into(),
        primary_archive_sha256:
            "d9c10833cae8b4d9361a0ffda31ec361fd1262362025bec4d4e51a880150ace2".into(),
        primary_source_path: "sofa/20231011/c/src/era00.c".into(),
        primary_source_sha256:
            "11513afbebee3c52eba971c2a9754b9ae1cc9ef76027866055dddb2d5328acbd".into(),
        license: "MIT plus retained SOFA-derived-work notices and terms; not official SOFA".into(),
        license_notice_path: "vendor/sofars-0.6.1/LICENSE".into(),
        input_time_scale: "UT1 (caller-supplied; no UTC/TT/TAI conversion)".into(),
        output_unit: "radian; normalized [0, 2*pi)".into(),
        limitations: vec![
            "A deterministic IAU 2000 model comparison, not an observation, universal scientific truth, or an independent P/N source.".into(),
            "The caller supplies UT1 and its source identifier; binding does not authenticate that source or establish measurement uncertainty.".into(),
            "Tolerance is the caller's model-comparison criterion, not a guaranteed physical or floating-point accuracy bound.".into(),
            "The 2^53 Julian-part bound is only a representability guard; it establishes no physical epoch-validity or numerical accuracy interval.".into(),
            "Supports when circular residual is below tolerance, counters when above; exact equality is unresolved.".into(),
            "No acceptance, knowledge, Vault-writing, Shadow-verdict, or constitutional authority; the downstream owner must validate scope and decide.".into(),
            "Only Astronomy/EarthRotationAngle is installed. Physics, History, SocialScience, and Psychology remain unavailable.".into(),
            "Re-evaluation and wire hashes check deterministic integrity, not independent scientific verification or sender authentication.".into(),
        ],
    }
}

/// Evaluate actual caller-supplied inputs. Nothing here creates persisted knowledge.
pub fn evaluate(query: &ScienceQuery) -> Result<CandidateEvidence, ScienceError> {
    let query = normalized(query)?;
    if query.adapter != AdapterId::Astronomy {
        return Err(ScienceError::Unavailable { adapter: query.adapter });
    }
    let ScienceTask::EarthRotationAngle {
        jd_part1, jd_part2, claim_radians, tolerance_radians, ..
    } = query.task;
    let computed = zero(sofars::erst::era00(jd_part1, jd_part2));
    if !computed.is_finite() || !(0.0..TAU).contains(&computed) {
        return Err(ScienceError::InvalidComputation);
    }
    let mut signed = (computed - claim_radians).rem_euclid(TAU);
    if signed > PI { signed -= TAU; }
    signed = zero(signed);
    let residual = signed.abs();
    let relation = if residual < tolerance_radians {
        ModelRelation::SupportsClaim
    } else if residual > tolerance_radians {
        ModelRelation::CountersClaim
    } else { ModelRelation::Unresolved };
    let query_sha256 = sha(&encode_query(&query)?);
    let mut candidate = CandidateEvidence {
        query, computed_radians: computed, signed_residual_radians: signed,
        circular_residual_radians: residual, relation, query_sha256,
        result_sha256: [0; 32], provenance: provider_provenance(),
        authority: CandidateAuthority::None,
    };
    candidate.result_sha256 = sha(&candidate_body(&candidate)?);
    Ok(candidate)
}
fn adapter_tag(adapter: AdapterId) -> u8 {
    match adapter { AdapterId::Physics => 0, AdapterId::Astronomy => 1,
        AdapterId::History => 2, AdapterId::SocialScience => 3, AdapterId::Psychology => 4 }
}
fn scale_tag(scale: TimeScale) -> u8 {
    match scale { TimeScale::Ut1 => 0, TimeScale::Utc => 1, TimeScale::Tai => 2,
        TimeScale::Tt => 3, TimeScale::Tdb => 4 }
}
fn relation_tag(relation: ModelRelation) -> u8 {
    match relation { ModelRelation::SupportsClaim => 0, ModelRelation::CountersClaim => 1,
        ModelRelation::Unresolved => 2 }
}
fn blob(output: &mut Vec<u8>, bytes: &[u8]) -> Result<(), ScienceError> {
    let length = u32::try_from(bytes.len()).map_err(|_| ScienceError::Oversized)?;
    let new_len = output.len().checked_add(4).and_then(|n| n.checked_add(bytes.len()))
        .ok_or(ScienceError::Oversized)?;
    if new_len > MAX_WIRE_BYTES { return Err(ScienceError::Oversized); }
    output.extend_from_slice(&length.to_le_bytes());
    output.extend_from_slice(bytes);
    Ok(())
}
fn text(output: &mut Vec<u8>, value: &str) -> Result<(), ScienceError> {
    blob(output, value.as_bytes())
}
fn number(output: &mut Vec<u8>, value: f64) {
    output.extend_from_slice(&zero(value).to_bits().to_le_bytes());
}
fn bounded(output: Vec<u8>) -> Result<Vec<u8>, ScienceError> {
    if output.len() > MAX_WIRE_BYTES { Err(ScienceError::Oversized) } else { Ok(output) }
}

/// Domain-separated, length-prefixed UTF-8 identities and little-endian f64 bits.
/// Negative zero is normalized to positive zero; NaN/infinities are rejected.
pub fn encode_query(query: &ScienceQuery) -> Result<Vec<u8>, ScienceError> {
    let query = normalized(query)?;
    let mut output = QUERY_MAGIC.to_vec();
    output.push(adapter_tag(query.adapter));
    for value in [&query.session_id, &query.case_id, &query.scope_id,
        &query.obligation_id, &query.query_id, &query.input_source_id] {
        text(&mut output, value)?;
    }
    output.push(0); // EarthRotationAngle task tag in wire version 1.
    let ScienceTask::EarthRotationAngle {
        jd_part1, jd_part2, time_scale, claim_radians, tolerance_radians,
    } = query.task;
    output.push(scale_tag(time_scale));
    number(&mut output, jd_part1);
    number(&mut output, jd_part2);
    number(&mut output, claim_radians);
    number(&mut output, tolerance_radians);
    bounded(output)
}
struct Reader<'a> { bytes: &'a [u8], position: usize }
impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Result<Self, ScienceError> {
        if bytes.len() > MAX_WIRE_BYTES { return Err(ScienceError::Oversized); }
        Ok(Self { bytes, position: 0 })
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], ScienceError> {
        let end = self.position.checked_add(count).ok_or(ScienceError::Oversized)?;
        let value = self.bytes.get(self.position..end)
            .ok_or(ScienceError::Malformed("truncated field"))?;
        self.position = end;
        Ok(value)
    }
    fn magic(&mut self, magic: &[u8]) -> Result<(), ScienceError> {
        if self.take(magic.len())? != magic { return Err(ScienceError::Malformed("unknown wire format")); }
        Ok(())
    }
    fn byte(&mut self) -> Result<u8, ScienceError> { Ok(self.take(1)?[0]) }
    fn blob(&mut self, max: usize) -> Result<&'a [u8], ScienceError> {
        let mut encoded = [0; 4];
        encoded.copy_from_slice(self.take(4)?);
        let count = usize::try_from(u32::from_le_bytes(encoded)).map_err(|_| ScienceError::Oversized)?;
        if count > max { return Err(ScienceError::Oversized); }
        self.take(count)
    }
    fn identity(&mut self) -> Result<String, ScienceError> {
        let value = self.blob(MAX_IDENTITY_BYTES)?;
        Ok(std::str::from_utf8(value).map_err(|_| ScienceError::Malformed("invalid UTF-8"))?.into())
    }
    fn number(&mut self) -> Result<f64, ScienceError> {
        let mut encoded = [0; 8];
        encoded.copy_from_slice(self.take(8)?);
        let bits = u64::from_le_bytes(encoded);
        if bits == (-0.0f64).to_bits() { return Err(ScienceError::NonCanonical); }
        Ok(f64::from_bits(bits))
    }
    fn end(&self) -> Result<(), ScienceError> {
        if self.position != self.bytes.len() { return Err(ScienceError::Malformed("trailing data")); }
        Ok(())
    }
}
pub fn decode_query(bytes: &[u8]) -> Result<ScienceQuery, ScienceError> {
    let mut input = Reader::new(bytes)?;
    input.magic(QUERY_MAGIC)?;
    let adapter = match input.byte()? {
        0 => AdapterId::Physics, 1 => AdapterId::Astronomy, 2 => AdapterId::History,
        3 => AdapterId::SocialScience, 4 => AdapterId::Psychology,
        _ => return Err(ScienceError::Malformed("unknown adapter")),
    };
    let session_id = input.identity()?;
    let case_id = input.identity()?;
    let scope_id = input.identity()?;
    let obligation_id = input.identity()?;
    let query_id = input.identity()?;
    let input_source_id = input.identity()?;
    if input.byte()? != 0 { return Err(ScienceError::Malformed("unknown task")); }
    let time_scale = match input.byte()? {
        0 => TimeScale::Ut1, 1 => TimeScale::Utc, 2 => TimeScale::Tai,
        3 => TimeScale::Tt, 4 => TimeScale::Tdb,
        _ => return Err(ScienceError::Malformed("unknown time scale")),
    };
    let task = ScienceTask::EarthRotationAngle {
        jd_part1: input.number()?, jd_part2: input.number()?,
        claim_radians: input.number()?, tolerance_radians: input.number()?, time_scale,
    };
    input.end()?;
    let query = normalized(&ScienceQuery { adapter, session_id, case_id, scope_id,
        obligation_id, query_id, input_source_id, task })?;
    if encode_query(&query)? != bytes { return Err(ScienceError::NonCanonical); }
    Ok(query)
}

fn candidate_body(candidate: &CandidateEvidence) -> Result<Vec<u8>, ScienceError> {
    let mut output = CANDIDATE_MAGIC.to_vec();
    blob(&mut output, &encode_query(&candidate.query)?)?;
    number(&mut output, candidate.computed_radians);
    number(&mut output, candidate.signed_residual_radians);
    number(&mut output, candidate.circular_residual_radians);
    output.push(relation_tag(candidate.relation));
    output.extend_from_slice(&candidate.query_sha256);
    output.push(0); // CandidateAuthority::None: cannot be promoted by transport.
    let p = &candidate.provenance;
    for value in [
        &p.model_id, &p.provider, &p.version, &p.commit, &p.archive_sha256,
        &p.repository, &p.source_path, &p.source_sha256,
    ] { text(&mut output, value)?; }
    output.extend_from_slice(&p.compiled_model_sha256);
    for value in [
        &p.primary_release, &p.primary_source_url, &p.primary_archive_sha256,
        &p.primary_source_path, &p.primary_source_sha256, &p.license,
        &p.license_notice_path, &p.input_time_scale, &p.output_unit,
    ] { text(&mut output, value)?; }
    let count = u32::try_from(p.limitations.len()).map_err(|_| ScienceError::Oversized)?;
    output.extend_from_slice(&count.to_le_bytes());
    for limitation in &p.limitations { text(&mut output, limitation)?; }
    bounded(output)
}
/// Only serialize a candidate that exactly matches re-evaluation of its query.
/// This is integrity verification by the same model, never independent evidence.
pub fn encode_candidate(candidate: &CandidateEvidence) -> Result<Vec<u8>, ScienceError> {
    let expected = evaluate(&candidate.query)?;
    if candidate != &expected { return Err(ScienceError::CandidateMismatch); }
    let mut output = candidate_body(candidate)?;
    output.extend_from_slice(&candidate.result_sha256);
    bounded(output)
}
/// Decode by recomputing the installed pinned model and comparing the entire
/// canonical record (all numeric fields, provenance, authority and both hashes).
/// Unsupported provider versions require a deliberate migration; none is silent.
pub fn decode_candidate(bytes: &[u8]) -> Result<CandidateEvidence, ScienceError> {
    let mut input = Reader::new(bytes)?;
    input.magic(CANDIDATE_MAGIC)?;
    let query = decode_query(input.blob(MAX_WIRE_BYTES)?)?;
    let expected = evaluate(&query)?;
    let canonical = encode_candidate(&expected)?;
    if canonical != bytes { return Err(ScienceError::CandidateMismatch); }
    Ok(expected)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Conformance oracle, NOT a runtime seed or independent P/N observation.
    // IAU SOFA C 2023-10-11 t_sofa_c.c:3635-3637, official primary archive.
    fn query() -> ScienceQuery {
        ScienceQuery {
            adapter: AdapterId::Astronomy, session_id: "session-1".into(),
            case_id: "case-1".into(), scope_id: "earth-rotation".into(),
            obligation_id: "obligation-1".into(), query_id: "query-1".into(),
            input_source_id: "IAU-SOFA-2023-10-11/conformance-vector-only".into(),
            task: ScienceTask::EarthRotationAngle {
                jd_part1: 2_400_000.5, jd_part2: 54_388.0, time_scale: TimeScale::Ut1,
                claim_radians: 0.4022837240028158102, tolerance_radians: 1e-12,
            },
        }
    }
    fn task_mut(query: &mut ScienceQuery) -> (&mut f64, &mut f64, &mut TimeScale, &mut f64, &mut f64) {
        let ScienceTask::EarthRotationAngle {
            jd_part1, jd_part2, time_scale, claim_radians, tolerance_radians,
        } = &mut query.task;
        (jd_part1, jd_part2, time_scale, claim_radians, tolerance_radians)
    }
    #[test]
    fn iau_primary_conformance_and_real_counter_computation() {
        let q = query();
        let evidence = evaluate(&q).unwrap();
        assert!((evidence.computed_radians - 0.4022837240028158102).abs() < 1e-12);
        assert_eq!(evidence.relation, ModelRelation::SupportsClaim);
        assert_eq!(evidence.authority, CandidateAuthority::None);
        let mut counter = q.clone();
        *task_mut(&mut counter).3 += 0.1;
        let negative = evaluate(&counter).unwrap();
        assert_eq!(negative.relation, ModelRelation::CountersClaim);
        assert_eq!(negative.computed_radians, evidence.computed_radians);
        assert!((negative.circular_residual_radians - 0.1).abs() < 1e-14);
        assert_ne!(negative.result_sha256, evidence.result_sha256);
    }
    #[test]
    fn all_uninstalled_domains_explicitly_unavailable() {
        for adapter in [AdapterId::Physics, AdapterId::History,
            AdapterId::SocialScience, AdapterId::Psychology] {
            let mut q = query();
            q.adapter = adapter;
            assert_eq!(evaluate(&q), Err(ScienceError::Unavailable { adapter }));
        }
    }
    #[test]
    fn wrong_timescale_nonfinite_and_out_of_domain_are_rejected() {
        for scale in [TimeScale::Utc, TimeScale::Tai, TimeScale::Tt, TimeScale::Tdb] {
            let mut q = query(); *task_mut(&mut q).2 = scale;
            assert_eq!(evaluate(&q), Err(ScienceError::WrongTimeScale { supplied: scale }));
        }
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for field in 0..4 {
                let mut q = query();
                let (a,b,_,c,t) = task_mut(&mut q);
                *[a,b,c,t][field] = bad;
                assert!(matches!(evaluate(&q), Err(ScienceError::NonFinite { .. })));
                assert!(encode_query(&q).is_err());
            }
        }
        for claim in [-1.0, TAU, TAU + 1.0] {
            let mut q = query(); *task_mut(&mut q).3 = claim;
            assert!(matches!(evaluate(&q), Err(ScienceError::OutOfDomain { .. })));
        }
        for tolerance in [0.0, -1.0, PI + 1.0] {
            let mut q = query(); *task_mut(&mut q).4 = tolerance;
            assert_eq!(evaluate(&q), Err(ScienceError::InvalidTolerance));
        }
        let mut q = query(); *task_mut(&mut q).0 = 1e100;
        assert!(matches!(evaluate(&q), Err(ScienceError::OutOfDomain { .. })));
    }
    #[test]
    fn all_scope_fields_and_numeric_input_bind_hashes() {
        let original = query();
        let expected = evaluate(&original).unwrap();
        for field in 0..6 {
            let mut q = original.clone();
            let fields = [&mut q.session_id, &mut q.case_id, &mut q.scope_id,
                &mut q.obligation_id, &mut q.query_id, &mut q.input_source_id];
            fields.into_iter().nth(field).unwrap().push_str("-different");
            let actual = evaluate(&q).unwrap();
            assert_ne!(actual.query_sha256, expected.query_sha256);
            assert_ne!(actual.result_sha256, expected.result_sha256);
        }
        let mut q = original.clone(); *task_mut(&mut q).1 += 1.0;
        let different_date = evaluate(&q).unwrap();
        assert_ne!(different_date.computed_radians, expected.computed_radians);
        assert_ne!(different_date.query_sha256, expected.query_sha256);
    }
    #[test]
    fn empty_control_or_unbounded_identity_is_rejected() {
        for value in ["".to_string(), " space".to_string(), "x\n".to_string(),
            "x".repeat(MAX_IDENTITY_BYTES + 1)] {
            let mut q = query(); q.obligation_id = value;
            assert_eq!(encode_query(&q),
                Err(ScienceError::InvalidIdentity { field: "obligation_id" }));
        }
    }
    #[test]
    fn canonical_codec_roundtrip_and_candidate_tamper_rejection() {
        let q = query();
        let query_wire = encode_query(&q).unwrap();
        assert_eq!(decode_query(&query_wire).unwrap(), q);
        let evidence = evaluate(&q).unwrap();
        let bytes = encode_candidate(&evidence).unwrap();
        assert!(bytes.len() < MAX_WIRE_BYTES);
        assert_eq!(decode_candidate(&bytes).unwrap(), evidence);
        // Mutation at header, query, outputs/provenance and final digest.
        for offset in [0, CANDIDATE_MAGIC.len() + 7, bytes.len()/2, bytes.len()-1] {
            let mut corrupted = bytes.clone(); corrupted[offset] ^= 1;
            assert!(decode_candidate(&corrupted).is_err());
        }
        let mut changed = evidence.clone(); changed.computed_radians += 0.1;
        assert_eq!(encode_candidate(&changed), Err(ScienceError::CandidateMismatch));
        let mut changed = evidence.clone(); changed.provenance.version = "other".into();
        assert_eq!(encode_candidate(&changed), Err(ScienceError::CandidateMismatch));
        let mut changed = evidence.clone(); changed.query.scope_id = "other".into();
        assert_eq!(encode_candidate(&changed), Err(ScienceError::CandidateMismatch));
        let mut trailing = bytes.clone(); trailing.push(0);
        assert!(decode_candidate(&trailing).is_err());
        assert!(decode_candidate(&bytes[..bytes.len()-1]).is_err());
    }
    #[test]
    fn length_prefix_overflow_truncation_and_noncanonical_numbers_rejected() {
        let q = query();
        let bytes = encode_query(&q).unwrap();
        let mut large_length = bytes.clone();
        let start = QUERY_MAGIC.len() + 1;
        large_length[start..start+4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(decode_query(&large_length), Err(ScienceError::Oversized));
        assert!(decode_query(&bytes[..bytes.len()-1]).is_err());
        let mut trailing = bytes.clone(); trailing.push(0);
        assert!(decode_query(&trailing).is_err());
        assert_eq!(decode_query(&vec![0; MAX_WIRE_BYTES+1]), Err(ScienceError::Oversized));
        let mut negative_zero = bytes.clone();
        let first_number = bytes.len()-32;
        negative_zero[first_number..first_number+8].copy_from_slice(&(-0.0f64).to_bits().to_le_bytes());
        assert_eq!(decode_query(&negative_zero), Err(ScienceError::NonCanonical));
        let mut nonfinite = bytes.clone();
        nonfinite[first_number..first_number+8].copy_from_slice(&f64::NAN.to_bits().to_le_bytes());
        assert!(matches!(decode_query(&nonfinite), Err(ScienceError::NonFinite { .. })));
    }
    #[test]
    fn negative_zero_input_normalization_and_exact_boundary() {
        let mut q = query(); *task_mut(&mut q).1 = -0.0;
        let mut plus = q.clone(); *task_mut(&mut plus).1 = 0.0;
        assert_eq!(encode_query(&q).unwrap(), encode_query(&plus).unwrap());
        let mut q = query(); *task_mut(&mut q).3 += 0.1;
        let distance = evaluate(&q).unwrap().circular_residual_radians;
        *task_mut(&mut q).4 = distance;
        assert_eq!(evaluate(&q).unwrap().relation, ModelRelation::Unresolved);
    }
    #[test]
    fn swapped_julian_parts_follow_exact_upstream_contract() {
        let q = query();
        let mut swapped = q.clone();
        let (a,b,_,_,_) = task_mut(&mut swapped);
        std::mem::swap(a,b);
        assert_eq!(evaluate(&q).unwrap().computed_radians,
            evaluate(&swapped).unwrap().computed_radians);
        assert_ne!(evaluate(&q).unwrap().query_sha256,
            evaluate(&swapped).unwrap().query_sha256);
    }
}
