// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  VAULT_DISK.RS — PERSISTENCA SOVRANE (Write-Ahead Log) · FAZA 2  ║
// ║  Gjata Legacy™  |  Arkitekt: Bledar Gjata                             ║
// ║                                                                          ║
// ║  Memoria sovrane duhet të MBIJETOJË restart-in. Ky modul ofron një     ║
// ║  backend disku append-only (WAL) për KnowledgeVault.                  ║
// ║                                                                          ║
// ║  PARIMI — WAL LOGJIK + RIPËRSËRITJE BESNIKE:                          ║
// ║    Regjistrojmë THIRRJET logjike (Primitive/Verified/Negative/Confirm) ║
// ║    në renditje. Në startup, ripërsëriten përmes TË NJËJTËS logjikë —   ║
// ║    dedup-i negativ dhe promovimi Primitive→Legacy riprodhohen EKZAKT.  ║
// ║    (Një primitiv fresh s'e arrin kurrë pragun 0.75 → çdo promovim      ║
// ║     vjen pas një `Confirm` të regjistruar.)                            ║
// ║                                                                          ║
// ║  FILE3: [LEN][!LEN][CRC32(LEN || PAYLOAD)][PAYLOAD], pastaj fsync.   ║
// ║  Vetëm bishti i paplotë me header të qëndrueshëm rikuperohet;        ║
// ║  korruptimi i dallueshëm refuzohet pa shkurtuar skedarin.             ║
// ║                                                                          ║
// ║  ZERO varësi të jashtme — vetëm std. Codec binar i shkruar me dorë.   ║
// ╚══════════════════════════════════════════════════════════════════════════╝

use crate::knowledge_lineage::Lineage;
use crate::knowledge_vault::{KnowledgeAppend, MaturationAppend, MaturationRecord, VaultBackend};
use crate::shadow_commit::KnowledgeWrite;
use crate::sovereign_ffi_gate::VerificationReceipt;
use crate::types::{ConstitutionalVerdict, KnowledgeBand, NegativeContext, ShadowPath};
use crate::vault_transaction::{GclReinforcement, VaultError, VaultTransaction};
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

// ── Identifikuesit e skedarit/rekordit ──────────────────────────────────────
const FILE_MAGIC: u64 = 0x474A_5356_4C30_3401; // "GJSVL04" + version 1
const HEADER_LEN: usize = 12; // 8 magic + 4 version
const FILE_VERSION: u32 = 3; // NEW isolated profile: protected framing + PD; no v2 migration
const FRAME_HEADER_LEN: usize = 12; // u32 LEN + u32 !LEN + CRC32(LEN || payload)
const WAL_READ_BUFFER_BYTES: usize = 64 * 1024;

// Etiketat e ngjarjeve
const TAG_PRIMITIVE: u8 = 1;
const TAG_VERIFIED: u8 = 2;
const TAG_NEGATIVE: u8 = 3;
const TAG_CONFIRM: u8 = 4;
const TAG_TRANSACTION: u8 = 5;
const TAG_PD_OPERATION: u8 = 6;
const TAG_NATIVE_CONTINUATION:u8=7;
const TAG_MATURATION: u8 = 8;
const MATURATION_COMMIT_MARKER: u64 = 0x4D41_5455_5245_5631; // MATUREV1
const PD_COMMIT_MARKER: u64 = 0x5044_5F43_4F4D_4D49; // PD_COMMI
const TX_FORMAT_VERSION: u8 = 1;
const TX_COMMIT_MARKER: u64 = 0x5458_5F43_4F4D_4D49; // "TX_COMMI"

// operational profile: FILE v3, unchanged knowledge TX v1, new PD tag 6.
// A frame is bounded independently of total WAL history. Previously accepted
// records exceeding these explicit resource limits now fail closed, never truncate.
const MAX_FIELD_BYTES: usize = 64 * 1024 * 1024;
const MAX_RECORD_BYTES: usize = 128 * 1024 * 1024;
const MAX_TRANSACTION_WRITES: usize = 65_536;
const MIN_KNOWLEDGE_WRITE_BYTES: usize = 37;

fn codec_error(message: &str) -> VaultError {
    VaultError::CorruptTransaction(message.to_string())
}

fn checked_record_len(parts: &[usize]) -> Result<usize, VaultError> {
    parts.iter().try_fold(0usize, |total, part| {
        total
            .checked_add(*part)
            .filter(|length| *length <= MAX_RECORD_BYTES)
            .ok_or_else(|| codec_error("WAL record exceeds bounded codec limit"))
    })
}

fn checked_field_len(length: usize) -> Result<u32, VaultError> {
    match length <= MAX_FIELD_BYTES {
        true => u32::try_from(length).map_err(|_| codec_error("WAL field length does not fit u32")),
        false => Err(codec_error("WAL field exceeds bounded codec limit")),
    }
}

fn payload_buffer(length: usize) -> Result<Vec<u8>, VaultError> {
    checked_record_len(&[length])?;
    let mut buffer = Vec::new();
    buffer
        .try_reserve_exact(length)
        .map_err(|error| codec_error(&format!("WAL allocation refused: {error}")))?;
    Ok(buffer)
}

// ════════════════════════════════════════════════════════════════════════════
// §1  CRC32 (IEEE, pa tabelë) — integritet rekordi
// ════════════════════════════════════════════════════════════════════════════

fn decode_u32_le(bytes: &[u8]) -> Option<u32> {
    <[u8; 4]>::try_from(bytes).ok().map(u32::from_le_bytes)
}

fn decode_u64_le(bytes: &[u8]) -> Option<u64> {
    <[u8; 8]>::try_from(bytes).ok().map(u64::from_le_bytes)
}

fn decode_f32_le(bytes: &[u8]) -> Option<f32> {
    <[u8; 4]>::try_from(bytes)
        .ok()
        .map(f32::from_le_bytes)
        .filter(|value| value.is_finite())
}

fn crc32_parts(parts: &[&[u8]]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &b in parts.iter().flat_map(|part| part.iter()) {
        crc ^= b as u32;
        let mut i = 0;
        while i < 8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            i += 1;
        }
    }
    !crc
}

#[cfg(test)]
fn crc32(data: &[u8]) -> u32 {
    crc32_parts(&[data])
}

fn frame_crc(length: u32, payload: &[u8]) -> u32 {
    crc32_parts(&[&length.to_le_bytes(), payload])
}

// ════════════════════════════════════════════════════════════════════════════
// §2  ENKODIM — shkrues binar mbi Vec<u8> (little-endian)
// ════════════════════════════════════════════════════════════════════════════

fn put_u8(b: &mut Vec<u8>, v: u8) {
    b.push(v);
}
fn put_u32(b: &mut Vec<u8>, v: u32) {
    b.extend_from_slice(&v.to_le_bytes());
}
fn put_u64(b: &mut Vec<u8>, v: u64) {
    b.extend_from_slice(&v.to_le_bytes());
}
fn put_f32(b: &mut Vec<u8>, v: f32) {
    b.extend_from_slice(&v.to_le_bytes());
}
fn put_bytes(b: &mut Vec<u8>, s: &[u8]) -> Result<(), VaultError> {
    let length = checked_field_len(s.len())?;
    checked_record_len(&[b.len(), 4, s.len()])?;
    put_u32(b, length);
    b.extend_from_slice(s);
    Ok(())
}
fn put_str(b: &mut Vec<u8>, s: &str) -> Result<(), VaultError> {
    put_bytes(b, s.as_bytes())
}

// ════════════════════════════════════════════════════════════════════════════
// §3  DEKODIM — lexues me kursor; kthen None nëse mungojnë bytes
// ════════════════════════════════════════════════════════════════════════════

struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        match self.remaining() < n {
            true => return None,
            false => {}
        }
        let s = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|s| s[0])
    }
    fn u32(&mut self) -> Option<u32> {
        self.take(4).and_then(decode_u32_le)
    }
    fn u64(&mut self) -> Option<u64> {
        self.take(8).and_then(decode_u64_le)
    }
    fn f32(&mut self) -> Option<f32> {
        self.take(4).and_then(decode_f32_le)
    }
    fn fixed32(&mut self) -> Option<[u8; 32]> {
        self.take(32)?.try_into().ok()
    }
    fn bytes(&mut self) -> Option<Vec<u8>> {
        self.bounded_bytes(MAX_FIELD_BYTES)
    }
    fn bounded_bytes(&mut self, limit: usize) -> Option<Vec<u8>> {
        let n = usize::try_from(self.u32()?).ok()?;
        // P8.1: WAL guard — 64MB kufizim OOM. Zero if — match.
        match n <= limit && n <= MAX_FIELD_BYTES {
            true => {
                let source = self.take(n)?;
                let mut owned = Vec::new();
                owned.try_reserve_exact(n).ok()?;
                owned.extend_from_slice(source);
                Some(owned)
            }
            false => None,
        }
    }
    fn string(&mut self) -> Option<String> {
        let raw = self.bytes()?;
        String::from_utf8(raw).ok()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// §4  REPLAY OP — ngjarja e dekoduar për ripërsëritje në startup
// ════════════════════════════════════════════════════════════════════════════

/// Ngjarjet e regjistruara (versioni i zotëruar, për ripërsëritje).
#[derive(Debug, Clone)]
pub enum ReplayOp {
    Primitive {
        input_id: u64,
        vector_points: f32,
        temporal_score: f32,
        mass: f32,
        seal: u32,
        path: u8,
        ts: u64,
        domain: String,
        bytes: Vec<u8>,
    },
    Verified {
        input_id: u64,
        band: u8,
        mass: f32,
        ts: u64,
        bytes: Vec<u8>,
    },
    Negative {
        input_id: u64,
        reason: u32,
        process: String,
        mass: f32,
        frequency: u32,
        ts: u64,
        bytes: Vec<u8>,
    },
    Confirm {
        input_id: u64,
        hits: u32,
    },
    Transaction(VaultTransaction),
    Maturation(MaturationRecord),
    PdOperation(shadow_contracts::pd_memory::PdOperationRecord),
    NativeContinuation(crate::native_continuation::NativeContinuationRecord),
}

fn encode_receipt(out: &mut Vec<u8>, receipt: &VerificationReceipt) -> Result<(), VaultError> {
    put_str(out, &receipt.session_id)?;
    put_str(out, &receipt.parent_i0)?;
    put_u64(out, receipt.primitive_anchor);
    put_u64(out, receipt.xy_digest);
    put_u64(out, receipt.pd_binding_digest);
    put_u64(out, receipt.pd_continuum_activation_digest);
    put_u64(out, receipt.living_trust_digest);
    out.extend_from_slice(&receipt.living_trust_sha256);
    put_u8(out, receipt.y_verdict);
    put_u8(out, receipt.x_verdict);
    put_u64(out, receipt.seal);
    put_u64(out, receipt.generation);
    put_str(out, &receipt.receipt_id)?;
    Ok(())
}

fn decode_receipt(c: &mut Cursor<'_>) -> Option<VerificationReceipt> {
    Some(VerificationReceipt {
        session_id: c.string()?,
        parent_i0: c.string()?,
        primitive_anchor: c.u64()?,
        xy_digest: c.u64()?,
        pd_binding_digest: c.u64()?,
        pd_continuum_activation_digest: c.u64()?,
        living_trust_digest: c.u64()?,
        living_trust_sha256: c.fixed32()?,
        y_verdict: c.u8()?,
        x_verdict: c.u8()?,
        seal: c.u64()?,
        generation: c.u64()?,
        receipt_id: c.string()?,
    })
}

fn encoded_transaction_len(tx: &VaultTransaction) -> Result<usize, VaultError> {
    match tx.knowledge_writes.len() <= MAX_TRANSACTION_WRITES {
        true => {}
        false => return Err(codec_error("WAL transaction write count exceeds limit")),
    }
    // Exact fixed bytes: header+identity/digests+lineage+flags/count, receipt
    // fixed fields and three length prefixes, receipt digest and commit trailer.
    let mut length = 474usize + usize::from(tx.exact_candidate_identity);
    match tx.gcl_reinforcement {
        Some(_) => length = checked_record_len(&[length, 12])?,
        None => {}
    }
    for value in [
        &tx.receipt.session_id,
        &tx.receipt.parent_i0,
        &tx.receipt.receipt_id,
    ] {
        checked_field_len(value.len())?;
        length = checked_record_len(&[length, value.len()])?;
    }
    for write in &tx.knowledge_writes {
        let (fixed, label, raw) = match write {
            KnowledgeWrite::Primitive {
                domain, raw_bytes, ..
            } => (42, domain, raw_bytes),
            KnowledgeWrite::Negative {
                context, raw_bytes, ..
            } => (37, &context.process, raw_bytes),
        };
        checked_field_len(label.len())?;
        checked_field_len(raw.len())?;
        length = checked_record_len(&[length, fixed, label.len(), raw.len()])?;
    }
    if let Some(snapshot) = &tx.native_cycle {
        if snapshot.model_judgment.is_some() {
            if snapshot.kie_proof.is_empty() {return Err(codec_error("model policy requires proof"));}
            length = checked_record_len(&[length,40])?;
        }
        checked_field_len(snapshot.response.len())?;
        checked_field_len(snapshot.prepared.len())?;
        length = checked_record_len(&[length, 40, snapshot.response.len(),snapshot.prepared.len()])?;
        if !snapshot.kie_proof.is_empty() {
            if snapshot.kie_proof.len()>shadow_contracts::kie::MAX_PROOF_BYTES {return Err(codec_error("KIE proof limit"));}
            length=checked_record_len(&[length,4,snapshot.kie_proof.len()])?;
        }
    }
    Ok(length)
}

fn encode_transaction(tx: &VaultTransaction) -> Result<Vec<u8>, VaultError> {
    let encoded_len = encoded_transaction_len(tx)?;
    let mut out = payload_buffer(encoded_len)?;
    put_u8(&mut out, TAG_TRANSACTION);
    let snapshot_profile = if tx.native_cycle.as_ref().is_some_and(|s|s.model_judgment.is_some()) {
        4
    } else if tx.native_cycle.as_ref().is_some_and(|s|!s.kie_proof.is_empty()) {
        3
    } else if tx.native_cycle.is_some() { 2 } else { TX_FORMAT_VERSION };
    // TX1-4 retain their bytes and projection. TX5 binds exact-content storage.
    put_u8(&mut out, if tx.exact_candidate_identity {5} else {snapshot_profile});
    if tx.exact_candidate_identity { put_u8(&mut out, snapshot_profile); }
    out.extend_from_slice(&tx.transaction_id);
    out.extend_from_slice(&tx.identity_digest);
    out.extend_from_slice(&tx.cycle_digest);
    put_u64(&mut out, tx.anchor_generation);
    out.extend_from_slice(&tx.negative_batch_digest);
    out.extend_from_slice(&tx.runtime_delta_digest);
    out.extend_from_slice(&tx.vds_close_digest);
    out.extend_from_slice(&tx.law0_digest);
    put_u8(
        &mut out,
        match tx.verdict_kind {
            ConstitutionalVerdict::Constructive => 1,
            ConstitutionalVerdict::RigorousNegative => 2,
        },
    );
    put_u8(&mut out, tx.y);
    put_u8(&mut out, tx.x);
    out.extend_from_slice(&tx.living_trust_sha256);
    put_u64(&mut out, tx.lineage.input_id);
    put_u32(&mut out, u32::from(tx.lineage.chain_mask));
    put_u8(&mut out, tx.lineage.chain_count);
    put_u32(&mut out, tx.lineage.proof_len);
    put_u64(&mut out, tx.lineage.derivation);
    put_u8(&mut out, tx.lineage.cap_sealed as u8);
    match tx.gcl_reinforcement {
        Some(reinforcement) => {
            put_u8(&mut out, 1);
            put_u64(&mut out, reinforcement.primitive_anchor);
            put_u32(&mut out, reinforcement.hits);
        }
        None => put_u8(&mut out, 0),
    }
    put_u32(
        &mut out,
        u32::try_from(tx.knowledge_writes.len())
            .map_err(|_| codec_error("WAL write count does not fit u32"))?,
    );
    for write in &tx.knowledge_writes {
        match write {
            KnowledgeWrite::Primitive {
                input_id,
                vector_points,
                temporal_score,
                mass,
                seal,
                path,
                domain,
                raw_bytes,
                ts_ns,
            } => {
                put_u8(&mut out, 1);
                put_u64(&mut out, *input_id);
                put_f32(&mut out, *vector_points);
                put_f32(&mut out, *temporal_score);
                put_f32(&mut out, *mass);
                put_u32(&mut out, *seal);
                put_u8(&mut out, path.as_u8());
                put_str(&mut out, domain)?;
                put_bytes(&mut out, raw_bytes)?;
                put_u64(&mut out, *ts_ns);
            }
            KnowledgeWrite::Negative {
                input_id,
                raw_bytes,
                context,
                ts_ns,
            } => {
                put_u8(&mut out, 2);
                put_u64(&mut out, *input_id);
                put_u32(&mut out, context.failure_reason_code);
                put_str(&mut out, &context.process)?;
                put_f32(&mut out, context.mass_at_fail);
                put_u32(&mut out, context.frequency);
                put_bytes(&mut out, raw_bytes)?;
                put_u64(&mut out, *ts_ns);
            }
        }
    }
    encode_receipt(&mut out, &tx.receipt)?;
    out.extend_from_slice(&tx.receipt_digest);
    if let Some(snapshot) = &tx.native_cycle {
        out.extend_from_slice(&snapshot.request_sha256);
        put_bytes(&mut out, &snapshot.response)?;
        put_bytes(&mut out, &snapshot.prepared)?;
        if !snapshot.kie_proof.is_empty() {put_bytes(&mut out,&snapshot.kie_proof)?;}
        if let Some((binding,evidence))=snapshot.model_judgment {
            out.extend_from_slice(&binding);put_u64(&mut out,evidence);
        }
    }
    out.extend_from_slice(&tx.transaction_id);
    put_u64(&mut out, TX_COMMIT_MARKER);
    match out.len() == encoded_len {
        true => Ok(out),
        false => Err(codec_error("WAL encoder length invariant failed")),
    }
}

fn decode_transaction(c: &mut Cursor<'_>) -> Option<VaultTransaction> {
    let format = c.u8()?;
    let exact_candidate_identity = format == 5;
    let version = if exact_candidate_identity { c.u8()? } else { format };
    match version {
        TX_FORMAT_VERSION | 2 | 3 | 4 => {}
        _ => return None,
    }
    let transaction_id = c.fixed32()?;
    let identity_digest = c.fixed32()?;
    let cycle_digest = c.fixed32()?;
    let anchor_generation = c.u64()?;
    let negative_batch_digest = c.fixed32()?;
    let runtime_delta_digest = c.fixed32()?;
    let vds_close_digest = c.fixed32()?;
    let law0_digest = c.fixed32()?;
    let verdict_kind = match c.u8()? {
        1 => ConstitutionalVerdict::Constructive,
        2 => ConstitutionalVerdict::RigorousNegative,
        0 | 3..=u8::MAX => return None,
    };
    let y = c.u8()?;
    let x = c.u8()?;
    let living_trust_sha256 = c.fixed32()?;
    let lineage = Lineage {
        input_id: c.u64()?,
        chain_mask: u16::try_from(c.u32()?).ok()?,
        chain_count: c.u8()?,
        proof_len: c.u32()?,
        derivation: c.u64()?,
        cap_sealed: match c.u8()? {
            0 => false,
            1 => true,
            _ => return None,
        },
    };
    let gcl_reinforcement = match c.u8()? {
        0 => None,
        1 => Some(GclReinforcement {
            primitive_anchor: c.u64()?,
            hits: c.u32()?,
        }),
        2..=u8::MAX => return None,
    };
    let write_count = usize::try_from(c.u32()?).ok()?;
    match write_count <= MAX_TRANSACTION_WRITES
        && write_count <= c.remaining() / MIN_KNOWLEDGE_WRITE_BYTES
    {
        true => {}
        false => return None,
    }
    let mut knowledge_writes = Vec::new();
    knowledge_writes.try_reserve_exact(write_count).ok()?;
    for _write_index in 0..write_count {
        let write = match c.u8()? {
            1 => KnowledgeWrite::Primitive {
                input_id: c.u64()?,
                vector_points: c.f32()?,
                temporal_score: c.f32()?,
                mass: c.f32()?,
                seal: c.u32()?,
                path: ShadowPath::from_u8(
                    c.u8().filter(|path| *path <= ShadowPath::Deep.as_u8())?,
                ),
                domain: c.string()?,
                raw_bytes: c.bytes()?,
                ts_ns: c.u64()?,
            },
            2 => KnowledgeWrite::Negative {
                input_id: c.u64()?,
                context: NegativeContext {
                    failure_reason_code: c.u32()?,
                    process: c.string()?,
                    mass_at_fail: c.f32()?,
                    frequency: c.u32()?,
                },
                raw_bytes: c.bytes()?,
                ts_ns: c.u64()?,
            },
            0 | 3..=u8::MAX => return None,
        };
        knowledge_writes.push(write);
    }
    let receipt = decode_receipt(c)?;
    let receipt_digest = c.fixed32()?;
    let native_cycle = if matches!(version, 2..=4) {
        Some(crate::native_cycle::NativeCycleSnapshot {
            request_sha256: c.fixed32()?,
            response: c.bytes()?,
            prepared:c.bytes()?,
            kie_proof:if version>=3 {
                let proof=c.bounded_bytes(shadow_contracts::kie::MAX_PROOF_BYTES)?;
                if proof.is_empty() {return None;}
                proof
            } else {Vec::new()},
            model_judgment: if version==4 {Some((c.fixed32()?,c.u64()?))} else {None},
        })
    } else {
        None
    };
    let commit_transaction_id = c.fixed32()?;
    let commit_marker = c.u64()?;
    let tx = VaultTransaction {
        exact_candidate_identity,
        transaction_id,
        identity_digest,
        cycle_digest,
        anchor_generation,
        negative_batch_digest,
        runtime_delta_digest,
        vds_close_digest,
        law0_digest,
        verdict_kind,
        y,
        x,
        living_trust_sha256,
        knowledge_writes,
        lineage,
        gcl_reinforcement,
        receipt_digest,
        receipt,
        native_cycle,
    };
    let transaction_valid = (c.remaining() == 0)
        & (commit_transaction_id == tx.transaction_id)
        & (commit_marker == TX_COMMIT_MARKER)
        & tx.validate().is_ok();
    match transaction_valid {
        true => Some(tx),
        false => None,
    }
}

fn encode_pd_operation(
    record: &shadow_contracts::pd_memory::PdOperationRecord,
) -> Result<Vec<u8>, VaultError> {
    let encoded = shadow_contracts::pd_memory::encode_record(record)
        .map_err(|message| codec_error(&message))?;
    let mut payload = payload_buffer(checked_record_len(&[45, encoded.len()])?)?;
    put_u8(&mut payload, TAG_PD_OPERATION);
    put_bytes(&mut payload, &encoded)?;
    payload.extend_from_slice(&record.record_id);
    put_u64(&mut payload, PD_COMMIT_MARKER);
    Ok(payload)
}

fn decode_pd_operation(
    cursor: &mut Cursor<'_>,
) -> Option<shadow_contracts::pd_memory::PdOperationRecord> {
    let encoded = cursor.bounded_bytes(shadow_contracts::pd_memory::MAX_REQUEST_BYTES + 256)?;
    let record = shadow_contracts::pd_memory::decode_record(&encoded).ok()?;
    let repeated_id = cursor.fixed32()?;
    let marker = cursor.u64()?;
    match repeated_id == record.record_id && marker == PD_COMMIT_MARKER && cursor.remaining() == 0 {
        true => Some(record),
        false => None,
    }
}

fn encode_maturation(record: &MaturationRecord) -> Result<Vec<u8>, VaultError> {
    if record.count == 0 { return Err(codec_error("empty maturation action")); }
    let mut payload = payload_buffer(46)?;
    put_u8(&mut payload, TAG_MATURATION);
    put_u8(&mut payload, 1); // projection policy; never reinterpret old frames
    put_u32(&mut payload, record.count);
    payload.extend_from_slice(&record.plan_digest);
    put_u64(&mut payload, MATURATION_COMMIT_MARKER);
    Ok(payload)
}
fn decode_maturation(c: &mut Cursor<'_>) -> Option<MaturationRecord> {
    if c.u8()? != 1 { return None; }
    let count = c.u32()?;
    let plan_digest = c.fixed32()?;
    if count == 0 || c.u64()? != MATURATION_COMMIT_MARKER || c.remaining() != 0 { return None; }
    Some(MaturationRecord { count, plan_digest })
}

/// Dekodon një payload të vetëm (pas verifikimit CRC) → ReplayOp.
fn decode_payload(payload: &[u8]) -> Option<ReplayOp> {
    let mut c = Cursor::new(payload);
    let tag = c.u8()?;
    let operation = match tag {
        TAG_PRIMITIVE => Some(ReplayOp::Primitive {
            input_id: c.u64()?,
            vector_points: c.f32()?,
            temporal_score: c.f32()?,
            mass: c.f32()?,
            seal: c.u32()?,
            path: c.u8().filter(|path| *path <= ShadowPath::Deep.as_u8())?,
            ts: c.u64()?,
            domain: c.string()?,
            bytes: c.bytes()?,
        }),
        TAG_VERIFIED => Some(ReplayOp::Verified {
            input_id: c.u64()?,
            band: c.u8().filter(|band| *band <= KnowledgeBand::Scientific as u8)?,
            mass: c.f32()?,
            ts: c.u64()?,
            bytes: c.bytes()?,
        }),
        TAG_NEGATIVE => Some(ReplayOp::Negative {
            input_id: c.u64()?,
            reason: c.u32()?,
            process: c.string()?,
            mass: c.f32()?,
            frequency: c.u32()?,
            ts: c.u64()?,
            bytes: c.bytes()?,
        }),
        TAG_CONFIRM => Some(ReplayOp::Confirm {
            input_id: c.u64()?,
            hits: c.u32()?,
        }),
        TAG_TRANSACTION => decode_transaction(&mut c).map(ReplayOp::Transaction),
        TAG_PD_OPERATION => decode_pd_operation(&mut c).map(ReplayOp::PdOperation),
        TAG_NATIVE_CONTINUATION => {
            let data=c.bytes()?;
            if c.remaining()!=0 {return None;}
            crate::native_continuation::NativeContinuationRecord::decode(&data).ok().map(ReplayOp::NativeContinuation)
        },
        TAG_MATURATION => decode_maturation(&mut c).map(ReplayOp::Maturation),
        0 | 9..=u8::MAX => None, /* etiketë e panjohur → ndal (siguri) */
    }?;
    (c.remaining() == 0).then_some(operation)
}

// ════════════════════════════════════════════════════════════════════════════
// §5  DISK BACKEND — WAL append-only me fsync
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WalHealth {
    Healthy,
    RecoveryRequired,
}

#[derive(Debug)]
struct WalFile {
    // The exclusive OS lock belongs to this file handle and is released on drop.
    // No clone/duplicate handle is exposed by DiskBackend.
    file: File,
    health: WalHealth,
}

/// Small production I/O core; tests inject write/flush/sync faults through the
/// same state machine without manufacturing a durable success.
trait DurableWriter: Write {
    fn sync_wal(&mut self) -> io::Result<()>;
}

impl DurableWriter for File {
    fn sync_wal(&mut self) -> io::Result<()> {
        self.sync_all()
    }
}

fn append_frame<W: DurableWriter>(
    writer: &mut W,
    health: &mut WalHealth,
    frame: &[u8],
) -> Result<(), VaultError> {
    match health {
        WalHealth::RecoveryRequired => {
            return Err(VaultError::Io(
                "VAULT_RECOVERY_REQUIRED: reopen under exclusive ownership".to_string(),
            ))
        }
        WalHealth::Healthy => {}
    }
    let result = writer
        .write_all(frame)
        .and_then(|()| writer.flush())
        .and_then(|()| writer.sync_wal());
    match result {
        Ok(()) => Ok(()),
        Err(error) => {
            // A write or sync error cannot prove that no bytes reached disk.
            // Never append again to an uncertain suffix with this handle.
            *health = WalHealth::RecoveryRequired;
            Err(VaultError::Io(format!(
                "VAULT_RECOVERY_REQUIRED: append outcome uncertain: {error}"
            )))
        }
    }
}

fn read_prefix<R: Read>(reader: &mut R, bytes: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0usize;
    while filled < bytes.len() {
        match reader.read(&mut bytes[filled..]) {
            Ok(0) => break,
            Ok(count) => filled += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok(filled)
}

#[derive(Debug)]
pub struct DiskBackend {
    file: Mutex<WalFile>,
    path: PathBuf,
}

impl DiskBackend {
    /// Exclusive ownership is acquired before reading, initializing or repairing
    /// the WAL. A competing opener receives io::ErrorKind::WouldBlock and does
    /// not parse or truncate the file. Dropping this backend releases ownership.
    pub(crate) fn open_replaying<F>(path: &Path, mut apply: F) -> io::Result<Self>
    where
        F: FnMut(ReplayOp) -> io::Result<()>,
    {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        file.try_lock().map_err(io::Error::from)?;
        let original_len = file.metadata()?.len();

        let good_len = match original_len == 0 {
            true => {
                let mut header = Vec::with_capacity(HEADER_LEN);
                put_u64(&mut header, FILE_MAGIC);
                put_u32(&mut header, FILE_VERSION);
                file.write_all(&header)?;
                file.sync_all()?;
                HEADER_LEN as u64
            }
            false => Self::replay_io(&mut file, &mut apply)?,
        };

        match good_len < original_len {
            true => {
                eprintln!(
                    "[VAULT_REPLAY] partial tail recovered: {} bytes",
                    original_len - good_len
                );
                file.set_len(good_len)?;
                // Repair is complete before exposing a healthy append handle.
                file.sync_all()?;
            }
            false => {}
        }
        file.seek(SeekFrom::Start(good_len))?;

        Ok(Self {
            file: Mutex::new(WalFile {
                file,
                health: WalHealth::Healthy,
            }),
            path: path.to_path_buf(),
        })
    }

    // Collectors are test helpers only. Production applies one bounded decoded
    // operation to an unpublished projection while retaining the same OS lock.
    #[cfg(test)]
    pub fn open(path: &Path) -> io::Result<(Self, Vec<ReplayOp>)> {
        let mut ops = Vec::new();
        let backend = Self::open_replaying(path, |op| {
            ops.try_reserve(1)
                .map_err(|error| io::Error::new(io::ErrorKind::OutOfMemory, error))?;
            ops.push(op);
            Ok(())
        })?;
        Ok((backend, ops))
    }

    // Bounded physical read-ahead only: every frame still passes replay_reader.
    // This buffer drops before the owner repairs or seeks to good_len for append.
    fn replay_io<R: Read, F>(reader: &mut R, apply: &mut F) -> io::Result<u64>
    where
        F: FnMut(ReplayOp) -> io::Result<()>,
    {
        let mut buffered = BufReader::with_capacity(WAL_READ_BUFFER_BYTES, reader);
        Self::replay_reader(&mut buffered, apply)
    }

    /// Decode and semantically apply one frame at a time. Any callback error
    /// aborts opening before tail repair. The callback must stage its effects;
    /// neither this function nor its caller may publish a partly rebuilt vault.
    fn replay_reader<R: Read, F>(reader: &mut R, apply: &mut F) -> io::Result<u64>
    where
        F: FnMut(ReplayOp) -> io::Result<()>,
    {
        let mut header = [0u8; HEADER_LEN];
        match read_prefix(reader, &mut header)? == HEADER_LEN {
            true => {}
            false => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "truncated WAL header",
                ))
            }
        }
        let magic = decode_u64_le(&header[..8])
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "CORRUPT_HEADER: magic"))?;
        let version = decode_u32_le(&header[8..])
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "CORRUPT_HEADER: version"))?;
        match (magic == FILE_MAGIC, version == FILE_VERSION) {
            (true, true) => {}
            (false, _) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unknown WAL magic",
                ))
            }
            (_, false) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unsupported WAL version",
                ))
            }
        }

        let mut good = HEADER_LEN as u64;
        loop {
            let mut frame_header = [0u8; FRAME_HEADER_LEN];
            let present = read_prefix(reader, &mut frame_header)?;
            // Fewer than four LEN bytes cannot describe a complete frame. Once
            // LEN exists, validate its bound and EVERY available !LEN byte even
            // on an incomplete header. Never classify a detected header fault
            // as a repairable tail. CRC needs a complete payload to validate.
            match present < 4 {
                true => break,
                false => {}
            }
            let encoded_length = decode_u32_le(&frame_header[..4])
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "CORRUPT_LEN"))?;
            let length = usize::try_from(encoded_length)
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "CORRUPT_LEN"))?;
            match length > 0 && length <= MAX_RECORD_BYTES {
                true => {}
                false => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("WAL_RECORD_LIMIT at offset {good}"),
                    ))
                }
            }
            let complement = (!encoded_length).to_le_bytes();
            let complement_present = present.saturating_sub(4).min(4);
            match frame_header[4..4 + complement_present] == complement[..complement_present] {
                true => {}
                false => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("CORRUPT_LEN_COMPLEMENT at offset {good}"),
                    ))
                }
            }
            match present == FRAME_HEADER_LEN {
                true => {}
                false => break,
            }
            let expected_crc = decode_u32_le(&frame_header[8..12])
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "CORRUPT_CRC"))?;
            let mut payload = Vec::new();
            payload.try_reserve_exact(length).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::OutOfMemory,
                    format!("WAL frame allocation: {error}"),
                )
            })?;
            payload.resize(length, 0);
            match read_prefix(reader, &mut payload)? == length {
                true => {}
                false => break,
            }
            match frame_crc(encoded_length, &payload) == expected_crc {
                true => {}
                false => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("CORRUPT_FULL_RECORD at offset {good}"),
                    ))
                }
            }
            let op = decode_payload(&payload).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("CORRUPT_TRANSACTION_OR_TAG at offset {good}"),
                )
            })?;
            apply(op)?;
            good = good
                .checked_add(FRAME_HEADER_LEN as u64)
                .and_then(|offset| offset.checked_add(length as u64))
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "WAL offset overflow"))?;
        }
        Ok(good)
    }

    #[cfg(test)]
    fn parse_reader<R: Read>(reader: &mut R) -> io::Result<(Vec<ReplayOp>, u64)> {
        let mut ops = Vec::new();
        let good = Self::replay_reader(reader, &mut |op| {
            ops.try_reserve(1)
                .map_err(|error| io::Error::new(io::ErrorKind::OutOfMemory, error))?;
            ops.push(op);
            Ok(())
        })?;
        Ok((ops, good))
    }

    #[cfg(test)]
    fn parse(buf: &[u8]) -> io::Result<(Vec<ReplayOp>, u64)> {
        Self::parse_reader(&mut std::io::Cursor::new(buf))
    }

    fn write_record(&self, payload: &[u8]) -> Result<(), VaultError> {
        checked_record_len(&[payload.len()])?;
        let length = u32::try_from(payload.len())
            .map_err(|_| codec_error("WAL record length does not fit u32"))?;
        let frame_len = payload
            .len()
            .checked_add(FRAME_HEADER_LEN)
            .ok_or_else(|| codec_error("WAL frame length overflow"))?;
        let mut frame = Vec::new();
        frame
            .try_reserve_exact(frame_len)
            .map_err(|error| codec_error(&format!("WAL frame allocation refused: {error}")))?;
        put_u32(&mut frame, length);
        put_u32(&mut frame, !length);
        put_u32(&mut frame, frame_crc(length, payload));
        frame.extend_from_slice(payload);
        let mut guard = self.file.lock().map_err(|error| {
            drop(error);
            VaultError::LockPoisoned("vault WAL file")
        })?;
        let WalFile { file, health } = &mut *guard;
        append_frame(file, health, &frame)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl VaultBackend for DiskBackend {
    fn append_maturation(&self, permit: MaturationAppend<'_>) -> Result<(), VaultError> {
        self.write_record(&encode_maturation(permit.record_for(self)?)?)
    }

    fn commit_native_continuation(&self,record:&crate::native_continuation::NativeContinuationRecord)->Result<(),VaultError>{
        let data=record.encode().map_err(|e|codec_error(&e))?;
        let mut payload=payload_buffer(5+data.len())?;
        put_u8(&mut payload,TAG_NATIVE_CONTINUATION);
        put_bytes(&mut payload,&data)?;
        self.write_record(&payload)
    }

    fn commit_pd_operation(
        &self,
        record: &shadow_contracts::pd_memory::PdOperationRecord,
    ) -> Result<(), VaultError> {
        self.write_record(&encode_pd_operation(record)?)
    }

    fn append_verified(&self, permit: KnowledgeAppend<'_>) -> Result<(), VaultError> {
        let tx = permit.transaction_for(self)?;
        self.write_record(&encode_transaction(tx)?)
    }
    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
    fn on_primitive(
        &self,
        input_id: u64,
        vector_points: f32,
        temporal_score: f32,
        mass: f32,
        seal: u32,
        path: u8,
        domain: &str,
        bytes: &[u8],
        ts: u64,
    ) -> Result<(), VaultError> {
        checked_field_len(domain.len())?;
        checked_field_len(bytes.len())?;
        let mut p = payload_buffer(checked_record_len(&[42, domain.len(), bytes.len()])?)?;
        put_u8(&mut p, TAG_PRIMITIVE);
        put_u64(&mut p, input_id);
        put_f32(&mut p, vector_points);
        put_f32(&mut p, temporal_score);
        put_f32(&mut p, mass);
        put_u32(&mut p, seal);
        put_u8(&mut p, path);
        put_u64(&mut p, ts);
        put_str(&mut p, domain)?;
        put_bytes(&mut p, bytes)?;
        self.write_record(&p)
    }

    #[cfg(test)]
    fn on_verified(
        &self,
        input_id: u64,
        band: u8,
        mass: f32,
        bytes: &[u8],
        ts: u64,
    ) -> Result<(), VaultError> {
        checked_field_len(bytes.len())?;
        let mut p = payload_buffer(checked_record_len(&[26, bytes.len()])?)?;
        put_u8(&mut p, TAG_VERIFIED);
        put_u64(&mut p, input_id);
        put_u8(&mut p, band);
        put_f32(&mut p, mass);
        put_u64(&mut p, ts);
        put_bytes(&mut p, bytes)?;
        self.write_record(&p)
    }

    #[cfg(any(test, feature = "dev_harness"))]
    fn on_negative(
        &self,
        input_id: u64,
        reason: u32,
        process: &str,
        mass: f32,
        frequency: u32,
        bytes: &[u8],
        ts: u64,
    ) -> Result<(), VaultError> {
        checked_field_len(process.len())?;
        checked_field_len(bytes.len())?;
        let mut p = payload_buffer(checked_record_len(&[37, process.len(), bytes.len()])?)?;
        put_u8(&mut p, TAG_NEGATIVE);
        put_u64(&mut p, input_id);
        put_u32(&mut p, reason);
        put_str(&mut p, process)?;
        put_f32(&mut p, mass);
        put_u32(&mut p, frequency);
        put_u64(&mut p, ts);
        put_bytes(&mut p, bytes)?;
        self.write_record(&p)
    }

    #[cfg(test)]
    fn on_confirm(&self, input_id: u64, hits: u32) -> Result<(), VaultError> {
        let mut p = payload_buffer(13)?;
        put_u8(&mut p, TAG_CONFIRM);
        put_u64(&mut p, input_id);
        put_u32(&mut p, hits);
        self.write_record(&p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audit_wal_legacy_payloads() -> Vec<Vec<u8>> {
        let mut primitive = vec![TAG_PRIMITIVE];
        put_u64(&mut primitive, 42);
        for value in [0.9, 0.8, 0.95] { put_f32(&mut primitive, value); }
        put_u32(&mut primitive, 0xA451);
        put_u8(&mut primitive, ShadowPath::Deep.as_u8());
        put_u64(&mut primitive, 1000);
        put_str(&mut primitive, "science").unwrap();
        put_bytes(&mut primitive, b"a,b").unwrap();

        let mut verified = vec![TAG_VERIFIED];
        put_u64(&mut verified, 43);
        put_u8(&mut verified, 2);
        put_f32(&mut verified, 1.0);
        put_u64(&mut verified, 1001);
        put_bytes(&mut verified, b"verified").unwrap();

        let mut negative = vec![TAG_NEGATIVE];
        put_u64(&mut negative, 44);
        put_u32(&mut negative, 7);
        put_str(&mut negative, "process").unwrap();
        put_f32(&mut negative, 0.5);
        put_u32(&mut negative, 1);
        put_u64(&mut negative, 1002);
        put_bytes(&mut negative, b"negative").unwrap();

        let mut confirm = vec![TAG_CONFIRM];
        put_u64(&mut confirm, 42);
        put_u32(&mut confirm, 2);
        vec![primitive, verified, negative, confirm]
    }

    fn audit_wal_transaction(cap_sealed: bool, path: ShadowPath) -> VaultTransaction {
        let mut sample = crate::vault_transaction::test_support::sample_transaction();
        sample.lineage.cap_sealed = cap_sealed;
        match &mut sample.knowledge_writes[0] {
            KnowledgeWrite::Primitive { path: old_path, .. } => *old_path = path,
            _ => panic!("primitive fixture required"),
        }
        let verdict = crate::types::LawConfirmedVerdict::confirm(
            crate::vault_transaction::test_support::sample_supreme(1, 1), false,
        ).unwrap();
        VaultTransaction::build(
            sample.identity_digest, sample.cycle_digest, sample.anchor_generation,
            sample.negative_batch_digest, sample.runtime_delta_digest,
            sample.vds_close_digest, sample.law0_digest, &verdict,
            sample.knowledge_writes, sample.lineage, sample.gcl_reinforcement, sample.receipt,
        )
    }

    fn audit_wal_reject_payload(payload: &[u8]) {
        // The frame CRC is recomputed: rejection must come from the payload codec.
        let frame = framed_file(payload);
        let error = DiskBackend::parse(&frame).expect_err("invalid payload must not replay");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn audit_wal_transaction_rejects_noncanonical_boolean() {
        let tx = audit_wal_transaction(false, ShadowPath::Fast);
        for candidate in [tx.clone(), tx.legacy_projection_fixture()] {
            let payload = encode_transaction(&candidate).unwrap();
            assert!(decode_payload(&payload).is_some());
            let cap_offset = 2 + usize::from(candidate.exact_candidate_identity)
                + 3 * 32 + 8 + 4 * 32 + 3 + 32 + 8 + 4 + 1 + 4 + 8;
            assert_eq!(payload[cap_offset], 0);
            for invalid in 2..=u8::MAX {
                let mut changed = payload.clone();
                changed[cap_offset] = invalid;
                audit_wal_reject_payload(&changed);
            }
        }
    }

    #[test]
    fn audit_wal_transaction_rejects_noncanonical_path() {
        let tx = audit_wal_transaction(true, ShadowPath::Deep);
        for candidate in [tx.clone(), tx.legacy_projection_fixture()] {
            let payload = encode_transaction(&candidate).unwrap();
            assert!(decode_payload(&payload).is_some());
            let path_offset = 2 + usize::from(candidate.exact_candidate_identity)
                + 3 * 32 + 8 + 4 * 32 + 3 + 32 + 8 + 4 + 1 + 4 + 8
                + 1 + 1 + 4 + 1 + 8 + 3 * 4 + 4;
            assert_eq!(payload[path_offset], ShadowPath::Deep.as_u8());
            for invalid in 3..=u8::MAX {
                let mut changed = payload.clone();
                changed[path_offset] = invalid;
                audit_wal_reject_payload(&changed);
            }
        }
    }

    #[test]
    fn audit_wal_legacy_rejects_trailing_bytes() {
        for payload in audit_wal_legacy_payloads() {
            assert!(DiskBackend::parse(&framed_file(&payload)).is_ok());
            for suffix in [&[0u8][..], &[0xFF, TAG_CONFIRM][..]] {
                let mut changed = payload.clone();
                changed.extend_from_slice(suffix);
                audit_wal_reject_payload(&changed);
            }
        }
    }

    #[test]
    fn audit_wal_legacy_rejects_nonfinite_numbers() {
        let payloads = audit_wal_legacy_payloads();
        for (index, offset) in [(0, 9), (0, 13), (0, 17), (1, 10), (2, 24)] {
            for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                let mut changed = payloads[index].clone();
                changed[offset..offset + 4].copy_from_slice(&invalid.to_le_bytes());
                audit_wal_reject_payload(&changed);
            }
        }
    }

    #[test]
    fn audit_wal_legacy_rejects_unknown_enums() {
        let payloads = audit_wal_legacy_payloads();
        for (index, offset, first_invalid) in [(0, 25, 3), (1, 9, 4)] {
            for invalid in first_invalid..=u8::MAX {
                let mut changed = payloads[index].clone();
                changed[offset] = invalid;
                audit_wal_reject_payload(&changed);
            }
        }
    }

    #[test]
    fn audit_wal_valid_legacy_domains_and_transaction_bytes_are_preserved() {
        let payloads = audit_wal_legacy_payloads();
        for (index, offset, last_valid) in [(0, 25, 2), (1, 9, 3)] {
            for valid in 0..=last_valid {
                let mut payload = payloads[index].clone();
                payload[offset] = valid;
                assert!(DiskBackend::parse(&framed_file(&payload)).is_ok());
            }
        }
        for (index, offset) in [(0, 9), (0, 13), (0, 17), (1, 10), (2, 24)] {
            for valid in [f32::MIN, -0.0, 0.0, f32::MIN_POSITIVE, 0.5, f32::MAX] {
                let mut payload = payloads[index].clone();
                payload[offset..offset + 4].copy_from_slice(&valid.to_le_bytes());
                assert!(decode_payload(&payload).is_some(), "codec must not invent a narrower numeric domain");
            }
        }
        for cap_sealed in [false, true] {
            for path in [ShadowPath::Fast, ShadowPath::Standard, ShadowPath::Deep] {
                let tx = audit_wal_transaction(cap_sealed, path);
                for candidate in [tx.clone(), tx.legacy_projection_fixture()] {
                    let payload = encode_transaction(&candidate).unwrap();
                    let Some(ReplayOp::Transaction(decoded)) = decode_payload(&payload) else {
                        panic!("valid canonical transaction");
                    };
                    assert_eq!(decoded.transaction_id, candidate.transaction_id);
                    assert_eq!(encode_transaction(&decoded).unwrap(), payload);
                }
            }
        }
    }

    #[test]
    fn crc32_known_vector() {
        // CRC32(IEEE) i "123456789" = 0xCBF43926
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn roundtrip_encode_decode_primitive() {
        let mut p = Vec::new();
        put_u8(&mut p, TAG_PRIMITIVE);
        put_u64(&mut p, 42);
        put_f32(&mut p, 0.9); // vector_points
        put_f32(&mut p, 0.8); // temporal_score
        put_f32(&mut p, 0.95); // mass
        put_u32(&mut p, 0xA451); // seal
        put_u8(&mut p, 1); // path (Standard)
        put_u64(&mut p, 1000); // ts
        put_str(&mut p, "science").expect("bounded test string"); // domain
        put_bytes(&mut p, b"abc").expect("bounded test bytes");
        match decode_payload(&p)
            .expect("roundtrip_encode_decode_primitive: rezultat i papritur në test")
        {
            ReplayOp::Primitive {
                input_id,
                vector_points,
                temporal_score,
                mass,
                seal,
                path,
                ts,
                domain,
                bytes,
            } => {
                assert_eq!(input_id, 42);
                assert!((vector_points - 0.9).abs() < 1e-6);
                assert!((temporal_score - 0.8).abs() < 1e-6);
                assert!((mass - 0.95).abs() < 1e-6);
                assert_eq!(seal, 0xA451);
                assert_eq!(path, 1);
                assert_eq!(ts, 1000);
                assert_eq!(domain, "science");
                assert_eq!(bytes, b"abc");
            }
            _ => panic!("tag i gabuar"),
        }
    }

    #[test]
    fn truncated_payload_returns_none() {
        let mut p = Vec::new();
        put_u8(&mut p, TAG_PRIMITIVE);
        put_u64(&mut p, 42);
        // mungojnë fushat e tjera
        assert!(decode_payload(&p).is_none());
    }

    #[test]
    fn r35_tx5_roundtrip_preserves_old_wire_and_rejects_policy_substitution() {
        let tx = crate::vault_transaction::test_support::sample_transaction();
        let legacy = tx.clone().legacy_projection_fixture();
        let old = encode_transaction(&legacy).unwrap();
        assert_eq!(old[1], 1);
        let current = encode_transaction(&tx).unwrap();
        assert_eq!(&current[..3], &[TAG_TRANSACTION, 5, 1]);
        assert_eq!(current.len(), old.len() + 1);
        for (payload, exact) in [(&old, false), (&current, true)] {
            let Some(ReplayOp::Transaction(decoded)) = decode_payload(payload) else { panic!("transaction") };
            assert_eq!(decoded.exact_candidate_identity, exact);
            assert!(decoded.validate().is_ok());
            assert_eq!(encode_transaction(&decoded).unwrap(), *payload);
        }
        // Keep the new tx digest but strip the policy discriminator.
        let mut downgrade = current.clone();
        downgrade.remove(1);
        assert!(decode_payload(&downgrade).is_none());
        let mut upgrade = old.clone();
        upgrade.insert(1, 5);
        assert!(decode_payload(&upgrade).is_none());
        for profile in [0, 5, 255] {
            let mut bad = current.clone(); bad[2] = profile;
            assert!(decode_payload(&bad).is_none());
        }
    }

    #[test]
    fn transaction_payload_roundtrip_requires_commit_marker() {
        let tx = crate::vault_transaction::test_support::sample_transaction();
        let payload = encode_transaction(&tx).expect("bounded transaction encoding");
        match decode_payload(&payload).expect("committed transaction payload") {
            ReplayOp::PdOperation(_) | ReplayOp::NativeContinuation(_) | ReplayOp::Maturation(_) => panic!("expected knowledge transaction"),
            ReplayOp::Transaction(decoded) => {
                assert_eq!(decoded.transaction_id, tx.transaction_id);
                assert_eq!(decoded.receipt, tx.receipt);
                assert_eq!(decoded.validate(), Ok(()));
            }
            ReplayOp::Primitive {
                input_id,
                vector_points,
                temporal_score,
                mass,
                seal,
                path,
                ts,
                domain,
                bytes,
            } => {
                let _unexpected = (
                    input_id,
                    vector_points,
                    temporal_score,
                    mass,
                    seal,
                    path,
                    ts,
                    domain,
                    bytes,
                );
                panic!("transaction tag");
            }
            ReplayOp::Verified {
                input_id,
                band,
                mass,
                ts,
                bytes,
            } => {
                let _unexpected = (input_id, band, mass, ts, bytes);
                panic!("transaction tag");
            }
            ReplayOp::Negative {
                input_id,
                reason,
                process,
                mass,
                frequency,
                ts,
                bytes,
            } => {
                let _unexpected = (input_id, reason, process, mass, frequency, ts, bytes);
                panic!("transaction tag");
            }
            ReplayOp::Confirm { input_id, hits } => {
                let _unexpected = (input_id, hits);
                panic!("transaction tag");
            }
        }
        let truncated = &payload[..payload.len() - 8];
        assert!(decode_payload(truncated).is_none());
    }

    #[test]
    fn committed_transaction_replays_exactly_once_after_restart() {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "shadow_tx_restart_{}_{}.wal",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos(),
        ));
        let tx = crate::vault_transaction::test_support::sample_transaction();
        let cycle = tx.cycle_digest;
        {
            let vault = crate::knowledge_vault::KnowledgeVault::open_disk(&path)
                .expect("open transactional WAL");
            vault.commit_test_transaction(tx).expect("durable commit");
            assert_eq!(vault.primitive_count(), 1);
        }
        {
            let vault =
                crate::knowledge_vault::KnowledgeVault::open_disk(&path).expect("first replay");
            assert_eq!(vault.primitive_count(), 1);
            assert!(vault.committed_receipt(&cycle).is_some());
        }
        {
            let vault =
                crate::knowledge_vault::KnowledgeVault::open_disk(&path).expect("second replay");
            assert_eq!(vault.primitive_count(), 1);
            assert!(vault.committed_receipt(&cycle).is_some());
        }
        drop(std::fs::remove_file(path));
    }

    #[test]
    fn partial_tail_is_recovered_but_full_crc_corruption_is_rejected() {
        let tx = crate::vault_transaction::test_support::sample_transaction();
        let payload = encode_transaction(&tx).expect("bounded transaction encoding");
        let mut frame = framed_file(&payload);
        let committed_len = frame.len() as u64;
        frame.extend_from_slice(&[0xAA, 0xBB, 0xCC]);
        let (ops, good_len) = DiskBackend::parse(&frame).expect("recover partial tail");
        assert_eq!(ops.len(), 1);
        assert_eq!(good_len, committed_len);

        let mut corrupt = frame[..committed_len as usize].to_vec();
        let payload_offset = HEADER_LEN + FRAME_HEADER_LEN;
        corrupt[payload_offset + 1] ^= 0x01;
        let error = DiskBackend::parse(&corrupt).expect_err("full CRC corruption");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn disk_write_failure_is_returned_not_reported_as_success() {
        let file = OpenOptions::new()
            .write(true)
            .open("/dev/full")
            .expect("/dev/full");
        let backend = DiskBackend {
            file: Mutex::new(WalFile {
                file,
                health: WalHealth::Healthy,
            }),
            path: PathBuf::from("/dev/full"),
        };
        let tx = crate::vault_transaction::test_support::sample_transaction();
        match backend.commit_test_transaction(&tx) {
            Err(VaultError::Io(message)) => assert!(!message.is_empty()),
            Err(VaultError::LockPoisoned(name)) => {
                panic!("pritej I/O failure, jo poisoned lock: {name}")
            }
            Err(VaultError::ConflictingReplay) => {
                panic!("pritej I/O failure, jo conflicting replay")
            }
            Err(VaultError::CorruptTransaction(message)) => {
                panic!("pritej I/O failure, jo corrupt transaction: {message}")
            }
            Ok(commit) => panic!("/dev/full nuk duhet të raportojë sukses: {commit:?}"),
        }
    }

    fn isolated_wal_path(label: &str) -> PathBuf {
        static NEXT_PATH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let occurrence = NEXT_PATH.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tick = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("test clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "essmai_wal_foundation_{label}_{}_{}_{occurrence}.wal",
            std::process::id(),
            tick
        ))
    }

    fn framed_file(payload: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        put_u64(&mut bytes, FILE_MAGIC);
        put_u32(&mut bytes, FILE_VERSION);
        let length = u32::try_from(payload.len()).expect("test payload length");
        put_u32(&mut bytes, length);
        put_u32(&mut bytes, !length);
        put_u32(&mut bytes, frame_crc(length, payload));
        bytes.extend_from_slice(payload);
        bytes
    }

    #[test]
    fn exclusive_lock_precedes_read_and_is_released_on_drop() {
        let path = isolated_wal_path("lock");
        let (owner, ops) = DiskBackend::open(&path).expect("first owner");
        assert!(ops.is_empty());
        let before = std::fs::metadata(&path).expect("metadata").len();
        let error = DiskBackend::open(&path).expect_err("competing owner must be refused");
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        assert_eq!(std::fs::metadata(&path).expect("metadata").len(), before);
        drop(owner);
        let (next, ops) = DiskBackend::open(&path).expect("lock released with owner");
        assert!(ops.is_empty());
        drop(next);
        std::fs::remove_file(path).expect("remove isolated test WAL");
    }

    #[test]
    fn exclusive_lock_child_probe() {
        let (path, expectation) = match (
            std::env::var_os("ESSMAI_WAL_FOUNDATION_LOCK_PATH"),
            std::env::var("ESSMAI_WAL_FOUNDATION_EXPECT_BLOCK"),
        ) {
            (None, Err(std::env::VarError::NotPresent)) => {
                // Normal Cargo executes the real parent/child protocol, never an empty probe.
                exclusive_lock_blocks_other_process();
                return;
            }
            (Some(path), Ok(expectation)) => (PathBuf::from(path), expectation),
            _ => panic!("lock probe requires both explicit child parameters or neither"),
        };
        let should_block = match expectation.as_str() {
            "1" => true,
            "0" => false,
            _ => panic!("explicit binary lock expectation required"),
        };
        let length = std::fs::metadata(&path).unwrap().len();
        match (DiskBackend::open(&path), should_block) {
            (Err(error), true) => assert_eq!(error.kind(), io::ErrorKind::WouldBlock),
            (Ok((backend, ops)), false) => {
                assert_eq!(ops.len(), 1, "release restores the existing frame, not a fresh WAL");
                drop(backend);
            }
            (Err(error), false) => panic!("released owner still blocks: {error}"),
            (Ok(_), true) => panic!("second process acquired owned WAL"),
        }
        assert_eq!(std::fs::metadata(&path).unwrap().len(), length);
        if should_block {
            println!("R20_MATH {{\"test\":\"exclusive_lock_child_probe\",\"checks\":[\"lock_exclusion\",\"length_unchanged\"]}}");
        } else {
            println!("R20_MATH {{\"test\":\"exclusive_lock_child_probe\",\"checks\":[\"lock_release\",\"existing_frame_preserved\",\"length_unchanged\"]}}");
        }
    }

    #[test]
    fn exclusive_lock_blocks_other_process() {
        let path = isolated_wal_path("process_lock");
        let (owner, _) = DiskBackend::open(&path).expect("first owner");
        owner.on_confirm(41, 1).expect("one durable storage fixture frame");
        let executable = std::env::current_exe().expect("current test executable");
        let listing = std::process::Command::new(&executable)
            .arg("--list")
            .output()
            .expect("list child test");
        assert!(listing.status.success());
        let listing = String::from_utf8(listing.stdout).expect("UTF-8 test listing");
        let probe = listing
            .lines()
            .find_map(|line| {
                line.strip_suffix(": test")
                    .filter(|name| name.ends_with("::exclusive_lock_child_probe"))
            })
            .expect("child probe registered")
            .to_string();
        let run_probe = |expected: &str| {
            let result = std::process::Command::new(&executable)
                .args(["--exact", probe.as_str(), "--nocapture"])
                .env("ESSMAI_WAL_FOUNDATION_LOCK_PATH", &path)
                .env("ESSMAI_WAL_FOUNDATION_EXPECT_BLOCK", expected)
                .output()
                .expect("execute lock probe");
            assert!(
                result.status.success(),
                "child probe failed: stdout={} stderr={}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            let stdout = String::from_utf8_lossy(&result.stdout);
            assert!(stdout.contains("test result: ok. 1 passed; 0 failed; 0 ignored;"), "child must execute exactly one test: {stdout}");
            assert!(stdout.contains(if expected == "1" { "lock_exclusion" } else { "lock_release" }), "child must observe the expected branch: {stdout}");
            for line in String::from_utf8_lossy(&result.stdout).lines() {
                // Propagate only semantic observations. Nested libtest summary
                // lines would falsely inflate the outer workspace test count.
                if let Some((_, observation)) = line.split_once("R20_MATH ") {
                    println!("R20_MATH {observation}");
                }
            }
        };
        // R20: sum(active owners) <= 1. Contenders cannot mutate the owned WAL;
        // release/reacquire preserves its bytes and the one existing frame.
        for _ in 0..3 { run_probe("1"); }
        drop(owner);
        let durable = std::fs::read(&path).unwrap();
        run_probe("0");
        assert_eq!(std::fs::read(&path).unwrap(), durable);
        let (next_owner, ops) = DiskBackend::open(&path).expect("parent reacquires released ownership");
        assert_eq!(ops.len(), 1);
        for _ in 0..2 { run_probe("1"); }
        drop(next_owner);
        assert_eq!(std::fs::read(&path).unwrap(), durable);
        run_probe("0");
        assert_eq!(std::fs::read(&path).unwrap(), durable);
        println!("R20_MATH {{\"test\":\"exclusive_lock_blocks_other_process\",\"checks\":[\"unique_owner\",\"reacquire\",\"bytes_unchanged\"]}}");
        std::fs::remove_file(path).expect("remove isolated test WAL");
    }

    #[test]
    fn partial_tail_repair_allows_one_clean_following_append() {
        let path = isolated_wal_path("repair");
        let (owner, _) = DiskBackend::open(&path).expect("initial owner");
        owner.on_confirm(11, 1).expect("first synced record");
        drop(owner);
        let committed = std::fs::metadata(&path).expect("committed length").len();
        {
            let mut file = OpenOptions::new()
                .append(true)
                .open(&path)
                .expect("fault writer");
            file.write_all(&[0xaa, 0xbb, 0xcc]).expect("partial header");
            file.sync_all().expect("persist partial test tail");
        }
        let (owner, ops) = DiskBackend::open(&path).expect("exclusive partial-tail repair");
        assert_eq!(ops.len(), 1);
        assert_eq!(
            std::fs::metadata(&path).expect("repaired length").len(),
            committed
        );
        owner.on_confirm(22, 2).expect("append after synced repair");
        drop(owner);
        let (owner, ops) = DiskBackend::open(&path).expect("replay clean suffix");
        assert_eq!(ops.len(), 2);
        assert!(matches!(
            ops[0],
            ReplayOp::Confirm {
                input_id: 11,
                hits: 1
            }
        ));
        assert!(matches!(
            ops[1],
            ReplayOp::Confirm {
                input_id: 22,
                hits: 2
            }
        ));
        drop(owner);
        std::fs::remove_file(path).expect("remove isolated test WAL");
    }

    #[derive(Clone, Copy)]
    enum InjectedFault {
        FirstWrite,
        PartialWrite,
        Flush,
        Sync,
        None,
    }

    struct FaultWriter {
        fault: InjectedFault,
        bytes: Vec<u8>,
        writes: usize,
        flushes: usize,
        syncs: usize,
    }

    impl Write for FaultWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.writes += 1;
            match (self.fault, self.writes) {
                (InjectedFault::FirstWrite, _) | (InjectedFault::PartialWrite, 2..) => Err(
                    io::Error::new(io::ErrorKind::Other, "injected write failure"),
                ),
                (InjectedFault::PartialWrite, 1) => {
                    let count = (bytes.len() / 2).max(1).min(bytes.len());
                    self.bytes.extend_from_slice(&bytes[..count]);
                    Ok(count)
                }
                _ => {
                    self.bytes.extend_from_slice(bytes);
                    Ok(bytes.len())
                }
            }
        }

        fn flush(&mut self) -> io::Result<()> {
            self.flushes += 1;
            match self.fault {
                InjectedFault::Flush => Err(io::Error::new(
                    io::ErrorKind::Other,
                    "injected flush failure",
                )),
                _ => Ok(()),
            }
        }
    }

    impl DurableWriter for FaultWriter {
        fn sync_wal(&mut self) -> io::Result<()> {
            self.syncs += 1;
            match self.fault {
                InjectedFault::Sync => Err(io::Error::new(
                    io::ErrorKind::Other,
                    "injected sync failure",
                )),
                _ => Ok(()),
            }
        }
    }

    #[test]
    fn every_io_failure_taints_the_same_production_append_core() {
        for fault in [
            InjectedFault::FirstWrite,
            InjectedFault::PartialWrite,
            InjectedFault::Flush,
            InjectedFault::Sync,
        ] {
            let mut writer = FaultWriter {
                fault,
                bytes: Vec::new(),
                writes: 0,
                flushes: 0,
                syncs: 0,
            };
            let mut health = WalHealth::Healthy;
            assert!(append_frame(&mut writer, &mut health, b"complete-frame").is_err());
            assert_eq!(health, WalHealth::RecoveryRequired);
            let before = (
                writer.writes,
                writer.flushes,
                writer.syncs,
                writer.bytes.clone(),
            );
            writer.fault = InjectedFault::None;
            assert!(append_frame(&mut writer, &mut health, b"must-not-append").is_err());
            assert_eq!(
                (writer.writes, writer.flushes, writer.syncs, writer.bytes),
                before,
                "tainted state must refuse before performing any further I/O"
            );
        }
    }

    #[test]
    fn malformed_count_is_rejected_before_count_sized_allocation() {
        let tx = crate::vault_transaction::test_support::sample_transaction();
        for tx in [tx.clone().legacy_projection_fixture(), tx] {
            let encoded = encode_transaction(&tx).expect("valid fixture");
            assert!(tx.gcl_reinforcement.is_none());
            let offset = 296 + usize::from(tx.exact_candidate_identity);
            assert_eq!(u32::from_le_bytes(encoded[offset..offset+4].try_into().unwrap()), 1);
            for count in [u32::MAX, MAX_TRANSACTION_WRITES as u32 + 1, 65_536] {
                let mut payload = encoded.clone();
                payload[offset..offset + 4].copy_from_slice(&count.to_le_bytes());
                let bytes = framed_file(&payload); // Valid CRC; exercise declared count bounds.
                assert_eq!(DiskBackend::parse(&bytes)
                    .expect_err("invalid declared write count").kind(), io::ErrorKind::InvalidData);
            }
        }
    }

    #[test]
    fn oversized_record_unknown_tag_and_unknown_versions_fail_closed() {
        let mut oversized = framed_file(&[]);
        oversized[HEADER_LEN..HEADER_LEN + 4]
            .copy_from_slice(&(MAX_RECORD_BYTES as u32 + 1).to_le_bytes());
        assert_eq!(
            DiskBackend::parse(&oversized)
                .expect_err("oversized LEN")
                .kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            DiskBackend::parse(&framed_file(&[0xff]))
                .expect_err("unknown tag")
                .kind(),
            io::ErrorKind::InvalidData
        );
        let mut file_version = framed_file(&[TAG_CONFIRM, 0]);
        file_version[8..12].copy_from_slice(&(FILE_VERSION + 1).to_le_bytes());
        assert_eq!(
            DiskBackend::parse(&file_version)
                .expect_err("unknown file version")
                .kind(),
            io::ErrorKind::InvalidData
        );
        let tx = crate::vault_transaction::test_support::sample_transaction();
        let mut payload = encode_transaction(&tx).expect("fixture");
        payload[1] = 255; // Unsupported outer format, not a recognized legacy profile.
        assert_eq!(
            DiskBackend::parse(&framed_file(&payload))
                .expect_err("unknown tx version")
                .kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn complete_decode_error_does_not_truncate_the_existing_file() {
        let path = isolated_wal_path("corruption");
        let bytes = framed_file(&[0xff]);
        std::fs::write(&path, &bytes).expect("isolated corrupt fixture");
        assert_eq!(
            DiskBackend::open(&path)
                .expect_err("complete unknown tag")
                .kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            std::fs::read(&path).expect("read after rejected open"),
            bytes
        );
        std::fs::remove_file(path).expect("remove isolated test WAL");
    }

    // Data-only transport fixtures: no scientific admission or authority claim.
    fn r35_confirm_stream(count: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        put_u64(&mut bytes, FILE_MAGIC);
        put_u32(&mut bytes, FILE_VERSION);
        for index in 0..count {
            let mut payload = vec![TAG_CONFIRM];
            put_u64(&mut payload, u64::from(index));
            put_u32(&mut payload, index + 1);
            bytes.extend_from_slice(&framed_file(&payload)[HEADER_LEN..]);
        }
        bytes
    }

    struct R35CountingReader<'a> {
        cursor: std::io::Cursor<&'a [u8]>,
        calls: usize,
        bytes_read: usize,
        largest_requested: usize,
        chunk_limit: usize,
        interrupt_next: bool,
    }

    impl<'a> R35CountingReader<'a> {
        fn new(bytes: &'a [u8], chunk_limit: usize, interrupt_next: bool) -> Self {
            Self {
                cursor: std::io::Cursor::new(bytes),
                calls: 0,
                bytes_read: 0,
                largest_requested: 0,
                chunk_limit,
                interrupt_next,
            }
        }
    }

    impl Read for R35CountingReader<'_> {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            self.calls += 1;
            self.largest_requested = self.largest_requested.max(bytes.len());
            if self.interrupt_next {
                self.interrupt_next = false;
                return Err(io::Error::from(io::ErrorKind::Interrupted));
            }
            let limit = bytes.len().min(self.chunk_limit);
            let count = self.cursor.read(&mut bytes[..limit])?;
            self.bytes_read += count;
            Ok(count)
        }
    }

    type R35ReplayObservation = (
        Result<u64, (io::ErrorKind, String)>,
        Vec<(u64, u32)>,
    );

    fn r35_observe(reader: &mut impl Read, production_io: bool) -> R35ReplayObservation {
        let mut operations = Vec::new();
        let mut apply = |op| {
            match op {
                ReplayOp::Confirm { input_id, hits } => operations.push((input_id, hits)),
                other => panic!("unexpected fixture operation: {other:?}"),
            }
            Ok(())
        };
        let result = match production_io {
            true => DiskBackend::replay_io(reader, &mut apply),
            false => DiskBackend::replay_reader(reader, &mut apply),
        };
        (result.map_err(|error| (error.kind(), error.to_string())), operations)
    }

    #[test]
    fn r35_wal_buffered_replay_reduces_reads_without_changing_operations() {
        let bytes = r35_confirm_stream(10_000);
        let mut direct = R35CountingReader::new(&bytes, usize::MAX, false);
        let mut production = R35CountingReader::new(&bytes, usize::MAX, false);
        let direct_result = r35_observe(&mut direct, false);
        let production_result = r35_observe(&mut production, true);
        assert_eq!(production_result, direct_result);
        assert_eq!(production_result.0, Ok(bytes.len() as u64));
        assert_eq!(production_result.1, (0..10_000).map(|i| (u64::from(i), i + 1)).collect::<Vec<_>>());
        assert_eq!(direct.calls, 20_002);
        assert_eq!(direct.bytes_read, bytes.len());
        assert_eq!(production.bytes_read, bytes.len());
        println!("R35_WAL_IO frames=10000 bytes={} direct_reads={} production_reads={} largest_request={}",
            bytes.len(), direct.calls, production.calls, production.largest_requested);
        assert!(production.calls <= 8, "bounded production replay must amortize small reads: {}", production.calls);
        assert!(production.largest_requested <= 64 * 1024, "read-ahead remains bounded independently of history");
    }

    #[test]
    fn r35_wal_buffered_replay_matches_fragmented_and_interrupted_input() {
        let bytes = r35_confirm_stream(31);
        let mut direct = R35CountingReader::new(&bytes, 3, true);
        let mut production = R35CountingReader::new(&bytes, 3, true);
        let expected = r35_observe(&mut direct, false);
        assert_eq!(r35_observe(&mut production, true), expected);
        assert_eq!(expected.0, Ok(bytes.len() as u64));
        assert_eq!(expected.1.len(), 31);
        assert_eq!(production.bytes_read, bytes.len());
    }

    #[test]
    fn r35_wal_buffered_replay_matches_torn_and_corrupt_streams() {
        let valid = r35_confirm_stream(3_000);
        let frame = &valid[HEADER_LEN..HEADER_LEN + FRAME_HEADER_LEN + 13];
        let mut cases = vec![valid.clone()];
        for cut in [1, 5, 11, 13, 24] {
            let mut torn = valid.clone();
            torn.extend_from_slice(&frame[..cut]);
            cases.push(torn);
        }
        // Corruption after the first 64 KiB also exercises a refill boundary.
        for field in [4, 8, 12] {
            let mut corrupt = valid.clone();
            corrupt[HEADER_LEN + 2_800 * 25 + field] ^= 1;
            corrupt.extend_from_slice(&frame[..3]);
            cases.push(corrupt);
        }
        let mut unknown = valid.clone();
        unknown.extend_from_slice(&framed_file(&[0xff])[HEADER_LEN..]);
        cases.push(unknown);
        let mut bad_header = valid.clone();
        bad_header[0] ^= 1;
        cases.push(bad_header);
        let mut oversized = valid.clone();
        put_u32(&mut oversized, MAX_RECORD_BYTES as u32 + 1);
        put_u32(&mut oversized, !(MAX_RECORD_BYTES as u32 + 1));
        cases.push(oversized);
        for (index, bytes) in cases.iter().enumerate() {
            let expected = r35_observe(&mut std::io::Cursor::new(bytes), false);
            let actual = r35_observe(&mut std::io::Cursor::new(bytes), true);
            assert_eq!(actual, expected, "differential stream {index}");
            if index < 6 {
                assert_eq!(actual.0, Ok(valid.len() as u64));
                assert_eq!(actual.1.len(), 3_000);
            } else {
                assert_eq!(actual.0.as_ref().unwrap_err().0, io::ErrorKind::InvalidData);
            }
        }
    }

    #[test]
    fn r35_wal_buffered_open_never_repairs_after_callback_or_integrity_failure() {
        for corrupt in [false, true] {
            let path = isolated_wal_path("r35_buffered_failure");
            let mut bytes = r35_confirm_stream(3_000);
            if corrupt {
                bytes[HEADER_LEN + 2_700 * 25 + FRAME_HEADER_LEN] ^= 1;
            }
            let torn = bytes[HEADER_LEN..HEADER_LEN + 3].to_vec();
            bytes.extend_from_slice(&torn);
            std::fs::write(&path, &bytes).expect("isolated fault fixture");
            let mut applied = 0;
            let error = DiskBackend::open_replaying(&path, |op| {
                applied += 1;
                match op {
                    ReplayOp::Confirm { input_id: 2_700, .. } if !corrupt => {
                        Err(io::Error::new(io::ErrorKind::InvalidData, "R35_INJECTED_SEMANTIC_FAILURE"))
                    }
                    ReplayOp::Confirm { .. } => Ok(()),
                    other => panic!("unexpected operation: {other:?}"),
                }
            }).expect_err("callback/integrity failure precedes any tail repair");
            assert_eq!(error.kind(), io::ErrorKind::InvalidData);
            assert_eq!(applied, if corrupt { 2_700 } else { 2_701 });
            assert!(error.to_string().contains(if corrupt { "CORRUPT_FULL_RECORD" } else { "R35_INJECTED_SEMANTIC_FAILURE" }));
            assert_eq!(std::fs::read(&path).expect("preserved evidence"), bytes);
            std::fs::remove_file(path).expect("failed open released ownership");
        }
    }

    struct FragmentedReader {
        cursor: std::io::Cursor<Vec<u8>>,
        largest_requested: usize,
    }

    impl Read for FragmentedReader {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            self.largest_requested = self.largest_requested.max(bytes.len());
            let limited = bytes.len().min(3);
            self.cursor.read(&mut bytes[..limited])
        }
    }

    #[test]
    fn replay_streams_fragmented_frames_instead_of_reading_entire_wal() {
        let mut payload = vec![TAG_CONFIRM];
        put_u64(&mut payload, 9);
        put_u32(&mut payload, 1);
        let mut wal = framed_file(&payload);
        let repeated_frame = wal[HEADER_LEN..].to_vec();
        for _ in 0..99 {
            wal.extend_from_slice(&repeated_frame);
        }
        let expected_length = wal.len() as u64;
        let mut reader = FragmentedReader {
            cursor: std::io::Cursor::new(wal),
            largest_requested: 0,
        };
        let (ops, good) =
            DiskBackend::parse_reader(&mut reader).expect("fragmented streaming replay");
        assert_eq!(ops.len(), 100);
        assert_eq!(good, expected_length);
        assert_eq!(reader.largest_requested, payload.len());
    }

    #[test]
    fn legacy_oversized_field_is_rejected_without_a_field_sized_allocation() {
        let mut payload = vec![TAG_VERIFIED];
        put_u64(&mut payload, 1);
        put_u8(&mut payload, 1);
        put_f32(&mut payload, 1.0);
        put_u64(&mut payload, 1);
        put_u32(&mut payload, MAX_FIELD_BYTES as u32 + 1);
        assert!(decode_payload(&payload).is_none());
    }

    #[test]
    fn protected_length_crc_and_unknown_tag_corruption_never_truncate() {
        let mut payload = vec![TAG_CONFIRM];
        put_u64(&mut payload, 9);
        put_u32(&mut payload, 1);
        let valid = framed_file(&payload);
        let mut corruptions = Vec::new();
        // Every single-bit LEN error is detected, including a plausible length
        // beyond EOF, before deciding to repair or allocating the payload.
        for offset in HEADER_LEN..HEADER_LEN + FRAME_HEADER_LEN {
            for bit in 0..8 {
                let mut changed = valid.clone();
                changed[offset] ^= 1 << bit;
                corruptions.push(changed);
            }
        }
        let mut changed = valid.clone();
        changed[HEADER_LEN + FRAME_HEADER_LEN + 1] ^= 1;
        corruptions.push(changed);
        // A valid framing checksum does not excuse an unsupported record tag.
        corruptions.push(framed_file(&[0xff]));
        for (case, bytes) in corruptions.into_iter().enumerate() {
            let path = isolated_wal_path(&format!("protected_corruption_{case}"));
            std::fs::write(&path, &bytes).expect("write corruption fixture");
            assert_eq!(
                DiskBackend::open(&path)
                    .expect_err("corruption must reject")
                    .kind(),
                io::ErrorKind::InvalidData,
                "corruption case {case}",
            );
            assert_eq!(std::fs::read(&path).expect("read refused WAL"), bytes);
            std::fs::remove_file(path).expect("remove fixture");
        }
    }

    #[test]
    fn every_consistent_torn_frame_prefix_recovers_before_next_append() {
        let mut payload = vec![TAG_CONFIRM];
        put_u64(&mut payload, 9);
        put_u32(&mut payload, 1);
        let committed = framed_file(&payload);
        let frame = &committed[HEADER_LEN..];
        // Includes partial LEN, partial complement, partial CRC and partial
        // payload. Only bytes from a correctly encoded frame are supplied.
        for cut in 1..frame.len() {
            let path = isolated_wal_path(&format!("torn_prefix_{cut}"));
            let mut bytes = committed.clone();
            bytes.extend_from_slice(&frame[..cut]);
            std::fs::write(&path, bytes).expect("write consistent torn frame");
            let (owner, ops) = DiskBackend::open(&path).expect("repair torn prefix");
            assert_eq!(ops.len(), 1);
            assert_eq!(
                std::fs::metadata(&path).expect("length").len(),
                committed.len() as u64
            );
            owner.on_confirm(10, 2).expect("append after repair");
            drop(owner);
            let (owner, ops) = DiskBackend::open(&path).expect("reopen following append");
            assert_eq!(ops.len(), 2);
            drop(owner);
            std::fs::remove_file(path).expect("remove fixture");
        }
    }

    #[test]
    fn inconsistent_available_length_complement_is_not_repaired_as_partial_header() {
        let mut payload = vec![TAG_CONFIRM];
        put_u64(&mut payload, 9);
        put_u32(&mut payload, 1);
        let complete = framed_file(&payload);
        // Even one available complement byte must match; lack of the remaining
        // header or payload never licenses ignoring an already detected error.
        for present in 5..FRAME_HEADER_LEN {
            let path = isolated_wal_path(&format!("bad_partial_header_{present}"));
            let mut bytes = complete[..HEADER_LEN + present].to_vec();
            bytes[HEADER_LEN + 4] ^= 1;
            std::fs::write(&path, &bytes).expect("partial inconsistent header");
            assert_eq!(
                DiskBackend::open(&path).expect_err("reject header").kind(),
                io::ErrorKind::InvalidData
            );
            assert_eq!(std::fs::read(&path).expect("read unchanged WAL"), bytes);
            std::fs::remove_file(path).expect("remove fixture");
        }
    }

    #[test]
    fn pd_wal_profile_refuses_file_v2_without_mutating_it() {
        let path = isolated_wal_path("v2_profile");
        let mut payload = vec![TAG_CONFIRM];
        put_u64(&mut payload, 9);
        put_u32(&mut payload, 1);
        let mut bytes = framed_file(&payload);
        bytes[8..12].copy_from_slice(&2u32.to_le_bytes());
        std::fs::write(&path, &bytes).expect("v2 fixture");
        assert_eq!(
            DiskBackend::open(&path)
                .expect_err("v2 requires explicit migration")
                .kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(std::fs::read(&path).expect("read refused v2"), bytes);
        std::fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn pd_record_requires_integrity_and_commit_marker() {
        let request = crate::pd_memory_store::tests::request("wal-record");
        let record = shadow_contracts::pd_memory::PdOperationRecord::construct(request, [0; 32])
            .expect("PD record");
        let payload = encode_pd_operation(&record).expect("PD encoding");
        match decode_payload(&payload).expect("PD decoding") {
            ReplayOp::PdOperation(decoded) => assert_eq!(decoded, record),
            _ => panic!("expected operational record"),
        }
        let mut changed = payload.clone();
        let last = changed.len() - 1;
        changed[last] ^= 1;
        assert!(decode_payload(&changed).is_none());
        let mut changed = record;
        changed.state_digest[0] ^= 1;
        assert!(encode_pd_operation(&changed).is_err());
    }
}
