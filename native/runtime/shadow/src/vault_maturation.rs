//! Durable materialization of the existing Primitive -> Legacy predicate.
//! New engineering composition: a projection command, not an evidence writer.
use super::*;
use crate::legacy_maturation::LegacyReason;
use crate::legacy_observer::LegacyBirth;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaturationRecord {
    pub(crate) count: u32,
    pub(crate) plan_digest: [u8; 32],
}

/// Only this Vault module constructs the borrowed permission, after deriving a
/// plan from its accepted rows. No external payload, hit count or verdict enters.
#[derive(Debug)]
pub struct MaturationAppend<'a> {
    record: &'a MaturationRecord,
    backend: &'a dyn VaultBackend,
}
impl<'a> MaturationAppend<'a> {
    pub(crate) fn record_for<B: VaultBackend + ?Sized>(self, backend: &B)
        -> Result<&'a MaturationRecord, VaultError> {
        if !same_backend(self.backend, backend) {
            return Err(VaultError::CorruptTransaction("MATURATION_APPEND_WRONG_BACKEND".into()));
        }
        Ok(self.record)
    }
}

struct PreparedRow {
    position: usize,
    score: f32,
    reason: LegacyReason,
}
fn prepare(rows: &[PrimitiveEntry]) -> Result<(MaturationRecord, Vec<PreparedRow>), ShadowError> {
    let count = rows.iter().filter(|entry| entry.ready_for_legacy()).count();
    let count_u32 = u32::try_from(count)
        .map_err(|_| ShadowError::VaultWriteError("MATURATION_COUNT_BOUND".into()))?;
    let mut prepared = Vec::new();
    prepared.try_reserve_exact(count)
        .map_err(|error| ShadowError::VaultWriteError(format!("MATURATION_PLAN_ALLOCATION: {error}")))?;
    let mut hash = Sha256::new();
    hash.update(b"OPL2S_MATURATION_PROJECTION_V1");
    hash.update(count_u32.to_le_bytes());
    for (position, entry) in rows.iter().enumerate().filter(|(_, entry)| entry.ready_for_legacy()) {
        let score = entry.legacy_score();
        let reason = entry.legacy_reason();
        hash.update((position as u64).to_le_bytes());
        hash.update(entry.input_id.to_le_bytes());
        hash.update(candidate_content_sha256(&entry.raw_bytes));
        hash.update(entry.vector_points.to_bits().to_le_bytes());
        hash.update(entry.temporal_score.to_bits().to_le_bytes());
        hash.update(entry.real_hits.to_le_bytes());
        hash.update(entry.added_at.to_le_bytes());
        hash.update(score.to_bits().to_le_bytes());
        hash.update(((entry.input_id as u32).wrapping_mul(0xA4B1)).to_le_bytes());
        hash.update((entry.primitive_chain.len() as u64).to_le_bytes());
        for trace in &entry.primitive_chain {
            hash.update((trace.domain.len() as u64).to_le_bytes());
            hash.update(trace.domain.as_bytes());
            hash.update(trace.timestamp.to_le_bytes());
            hash.update(trace.mass.to_bits().to_le_bytes());
            hash.update([trace.path_taken.as_u8()]);
            hash.update(trace.lgc_seal.to_le_bytes());
        }
        hash.update((reason.domain_count as u64).to_le_bytes());
        hash.update(reason.real_hits.to_le_bytes());
        hash.update(reason.maturity_score.to_bits().to_le_bytes());
        hash.update(reason.chain_strength.to_bits().to_le_bytes());
        hash.update([u8::from(reason.cross_domain_ok), u8::from(reason.maturation_ok)]);
        // Raw access_count is soft metadata. For an admitted eligible row:
        // >=5 distinct traces imply >=4 persisted merges; real_hits>=3 implies
        // >=1 persisted confirmation, so durable access_count>=5. The unchanged
        // observer clamps access_count/5 at 1. Soft accesses cannot change reason.
        // Bind that actual derived reason, never a snapshot of the soft counter.
        prepared.push(PreparedRow { position, score, reason });
    }
    Ok((MaturationRecord { count: count_u32, plan_digest: hash.finalize().into() }, prepared))
}

impl KnowledgeVault {
    /// None is a live projection request; Some is semantic replay of its record.
    /// Both derive the same plan under the existing owner and unchanged gates.
    pub(super) fn run_maturation_action(&self, expected: Option<&MaturationRecord>)
        -> Result<u32, ShadowError> {
        let _commit = self.commit_lock.lock()
            .map_err(|_| ShadowError::VaultWriteError("MATURATION_COMMIT_LOCK".into()))?;
        let mut primitive = self.primitive.write()
            .map_err(|_| ShadowError::VaultWriteError("MATURATION_PRIMITIVE_LOCK".into()))?;
        let mut legacy = self.legacy.write()
            .map_err(|_| ShadowError::VaultWriteError("MATURATION_LEGACY_LOCK".into()))?;
        let (record, prepared) = prepare(&primitive)?;
        if let Some(expected) = expected {
            if expected != &record || record.count == 0 {
                return Err(ShadowError::VaultWriteError(format!(
                    "MATURATION_PLAN_MISMATCH expected_count={} actual_count={} expected_sha={} actual_sha={}",
                    expected.count, record.count,
                    shadow_contracts::pd_memory::hex(&expected.plan_digest),
                    shadow_contracts::pd_memory::hex(&record.plan_digest))));
            }
        }
        if record.count == 0 { return Ok(0); }
        legacy.try_reserve_for_mutation(prepared.len())
            .map_err(|error| ShadowError::VaultWriteError(format!("MATURATION_LEGACY_ALLOCATION: {error}")))?;
        let mut births = self.observer.prepare_births(prepared.len())
            .map_err(ShadowError::VaultWriteError)?;
        if expected.is_none() {
            crate::noi_caller::invoke("SHADOW_PERSIST_MATURATION",
                &shadow_contracts::pd_memory::hex(&record.plan_digest), || {
                    self.backend.append_maturation(MaturationAppend {
                        record: &record, backend: self.backend.as_ref(),
                    }).map_err(|error| ShadowError::VaultWriteError(error.to_string()))
                })?;
        }
        // All fallible checks/reservations and live WAL sync precede these moves.
        // Holding both stores and observer guards prevents partial reader views.
        for (removed, row) in prepared.into_iter().enumerate() {
            let entry = primitive.remove(row.position - removed);
            births.push(LegacyBirth {
                origin_primitive_id: entry.input_id, crystallized_at: entry.added_at,
                legacy_score: row.score, reason: row.reason,
            });
            legacy.push(LegacyEntry {
                origin_primitive_id: entry.input_id, raw_bytes: entry.raw_bytes,
                crystallized_at: entry.added_at, legacy_score: row.score,
                immutable_seal: (entry.input_id as u32).wrapping_mul(0xA4B1),
            });
        }
        Ok(record.count)
    }
}
