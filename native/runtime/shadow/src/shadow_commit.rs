use crate::knowledge_lineage::Lineage;
use crate::types::{LawConfirmedVerdict, NegativeContext, ShadowPath};
use sha2::{Digest, Sha256};

/// Write-set i dijes i prodhuar nga ShadowGjLegacy. Nuk kryen I/O.
#[derive(Debug, Clone)]
pub enum KnowledgeWrite {
    Primitive {
        input_id: u64,
        vector_points: f32,
        temporal_score: f32,
        mass: f32,
        seal: u32,
        path: ShadowPath,
        domain: String,
        raw_bytes: Vec<u8>,
        ts_ns: u64,
    },
    Negative {
        input_id: u64,
        raw_bytes: Vec<u8>,
        context: NegativeContext,
        ts_ns: u64,
    },
}

impl KnowledgeWrite {
    /// SHA-256 kanonik i gjithë materialit të shkrimit. Ky digest lidh jo vetëm
    /// payload-in, por edhe kontekstin, masën, path-in dhe kohën me transaction-in.
    pub fn digest(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        match self {
            Self::Primitive {
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
                hasher.update(b"ESSMAI_KNOWLEDGE_WRITE_PRIMITIVE_V1");
                hasher.update(input_id.to_le_bytes());
                hasher.update(vector_points.to_bits().to_le_bytes());
                hasher.update(temporal_score.to_bits().to_le_bytes());
                hasher.update(mass.to_bits().to_le_bytes());
                hasher.update(seal.to_le_bytes());
                hasher.update([path.as_u8()]);
                hasher.update((domain.len() as u64).to_le_bytes());
                hasher.update(domain.as_bytes());
                hasher.update((raw_bytes.len() as u64).to_le_bytes());
                hasher.update(raw_bytes);
                hasher.update(ts_ns.to_le_bytes());
            }
            Self::Negative {
                input_id,
                raw_bytes,
                context,
                ts_ns,
            } => {
                hasher.update(b"ESSMAI_KNOWLEDGE_WRITE_NEGATIVE_V1");
                hasher.update(input_id.to_le_bytes());
                hasher.update(context.failure_reason_code.to_le_bytes());
                hasher.update((context.process.len() as u64).to_le_bytes());
                hasher.update(context.process.as_bytes());
                hasher.update(context.mass_at_fail.to_bits().to_le_bytes());
                hasher.update(context.frequency.to_le_bytes());
                hasher.update((raw_bytes.len() as u64).to_le_bytes());
                hasher.update(raw_bytes);
                hasher.update(ts_ns.to_le_bytes());
            }
        }
        hasher.finalize().into()
    }
}

/// Kandidati suprem është rezultat i pastër: verdict i mbyllur + write-set + lineage.
/// Vetëm KnowledgeVault::commit_transaction lejohet ta materializojë.
#[derive(Debug, Clone)]
pub struct SupremeCandidate {
    pub verdict: LawConfirmedVerdict,
    pub knowledge_write: KnowledgeWrite,
    pub lineage: Lineage,
}
