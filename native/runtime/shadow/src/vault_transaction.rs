use crate::knowledge_lineage::Lineage;
use crate::shadow_commit::KnowledgeWrite;
use crate::sovereign_ffi_gate::VerificationReceipt;
use crate::types::{ConstitutionalVerdict, LawConfirmedVerdict};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GclReinforcement {
    pub primitive_anchor: u64,
    pub hits: u32,
}

#[derive(Debug, Clone)]
pub struct VaultTransaction {
    pub(crate) exact_candidate_identity: bool,
    pub transaction_id: [u8; 32],
    pub identity_digest: [u8; 32],
    pub cycle_digest: [u8; 32],
    pub anchor_generation: u64,
    pub negative_batch_digest: [u8; 32],
    pub runtime_delta_digest: [u8; 32],
    pub vds_close_digest: [u8; 32],
    pub law0_digest: [u8; 32],
    pub verdict_kind: ConstitutionalVerdict,
    pub y: u8,
    pub x: u8,
    pub living_trust_sha256: [u8; 32],
    pub knowledge_writes: Vec<KnowledgeWrite>,
    pub lineage: Lineage,
    pub gcl_reinforcement: Option<GclReinforcement>,
    pub receipt_digest: [u8; 32],
    pub receipt: VerificationReceipt,
    pub(crate) native_cycle: Option<crate::native_cycle::NativeCycleSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommitStatus {
    Committed,
    AlreadyCommitted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurableCommit {
    pub transaction_id: [u8; 32],
    pub cycle_digest: [u8; 32],
    pub status: CommitStatus,
    pub receipt: VerificationReceipt,
    pub(crate) native_cycle: Option<crate::native_cycle::NativeCycleSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultError {
    Io(String),
    LockPoisoned(&'static str),
    ConflictingReplay,
    CorruptTransaction(String),
}

impl std::fmt::Display for VaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(message) => write!(f, "vault I/O: {message}"),
            Self::LockPoisoned(name) => write!(f, "vault lock poisoned: {name}"),
            Self::ConflictingReplay => write!(f, "same identity me material të ndryshëm"),
            Self::CorruptTransaction(message) => write!(f, "transaction corrupt: {message}"),
        }
    }
}

impl std::error::Error for VaultError {}

impl VaultTransaction {
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        identity_digest: [u8; 32],
        cycle_digest: [u8; 32],
        anchor_generation: u64,
        negative_batch_digest: [u8; 32],
        runtime_delta_digest: [u8; 32],
        vds_close_digest: [u8; 32],
        law0_digest: [u8; 32],
        verdict: &LawConfirmedVerdict,
        knowledge_writes: Vec<KnowledgeWrite>,
        lineage: Lineage,
        gcl_reinforcement: Option<GclReinforcement>,
        receipt: VerificationReceipt,
    ) -> Self {
        let supreme = verdict.as_supreme();
        let receipt_digest = verification_receipt_digest(&receipt);
        let mut transaction = Self {
            exact_candidate_identity: true,
            transaction_id: [0u8; 32],
            identity_digest,
            cycle_digest,
            anchor_generation,
            negative_batch_digest,
            runtime_delta_digest,
            vds_close_digest,
            law0_digest,
            verdict_kind: verdict.kind(),
            y: supreme.verified,
            x: supreme.primitive,
            living_trust_sha256: supreme.living_trust_sha256,
            knowledge_writes,
            lineage,
            gcl_reinforcement,
            receipt_digest,
            receipt,
            native_cycle: None,
        };
        transaction.transaction_id = transaction.compute_transaction_id();
        transaction
    }

    pub(crate) fn with_native_cycle(
        mut self,
        snapshot: crate::native_cycle::NativeCycleSnapshot,
    ) -> Result<Self, VaultError> {
        snapshot.validate(&self)?;
        self.native_cycle = Some(snapshot);
        self.transaction_id = self.compute_transaction_id();
        self.validate()?;
        Ok(self)
    }

    pub(crate) fn validate_for_live(&self) -> Result<(), VaultError> {
        if !self.exact_candidate_identity {
            return Err(VaultError::CorruptTransaction("LIVE_STORAGE_POLICY_REQUIRED".into()));
        }
        self.validate()
    }

    #[cfg(test)]
    pub(crate) fn legacy_projection_fixture(mut self) -> Self {
        self.exact_candidate_identity = false;
        self.transaction_id = self.compute_transaction_id();
        self
    }

    pub fn validate(&self) -> Result<(), VaultError> {
        if let Some(snapshot) = &self.native_cycle {
            snapshot.validate(self)?;
        }
        let pair_ok = match self.verdict_kind {
            ConstitutionalVerdict::Constructive => (self.y, self.x) == (1, 1),
            ConstitutionalVerdict::RigorousNegative => (self.y, self.x) == (0, 0),
        };
        let receipt_pair_ok = (self.receipt.y_verdict, self.receipt.x_verdict) == (self.y, self.x);
        let receipt_trust_ok = self.receipt.living_trust_sha256 == self.living_trust_sha256;
        let receipt_digest_ok = verification_receipt_digest(&self.receipt) == self.receipt_digest;
        let transaction_id_ok = self.compute_transaction_id() == self.transaction_id;
        let identity_present = self.identity_digest != [0u8; 32];
        let cycle_present = self.cycle_digest != [0u8; 32];
        match (
            pair_ok,
            receipt_pair_ok,
            receipt_trust_ok,
            receipt_digest_ok,
            transaction_id_ok,
            identity_present,
            cycle_present,
        ) {
            (true, true, true, true, true, true, true) => Ok(()),
            _ => Err(VaultError::CorruptTransaction(
                "Y/X, receipt, SHA-256 ose transaction identity nuk përputhet".to_string(),
            )),
        }
    }

    fn compute_transaction_id(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(b"ESSMAI_SHADOW_TX_V1");
        if self.exact_candidate_identity {
            hasher.update(b"OPL2S_EXACT_CANDIDATE_PROJECTION_V1");
        }
        hasher.update(self.identity_digest);
        hasher.update(self.cycle_digest);
        hasher.update(self.anchor_generation.to_le_bytes());
        hasher.update(self.negative_batch_digest);
        hasher.update(self.runtime_delta_digest);
        hasher.update(self.vds_close_digest);
        hasher.update(self.law0_digest);
        hasher.update([match self.verdict_kind {
            ConstitutionalVerdict::Constructive => 1,
            ConstitutionalVerdict::RigorousNegative => 2,
        }]);
        hasher.update([self.y, self.x]);
        hasher.update(self.living_trust_sha256);
        hasher.update(self.lineage.input_id.to_le_bytes());
        hasher.update(self.lineage.chain_mask.to_le_bytes());
        hasher.update([self.lineage.chain_count]);
        hasher.update(self.lineage.proof_len.to_le_bytes());
        hasher.update(self.lineage.derivation.to_le_bytes());
        hasher.update([u8::from(self.lineage.cap_sealed)]);
        match self.gcl_reinforcement {
            Some(reinforcement) => {
                hasher.update([1]);
                hasher.update(reinforcement.primitive_anchor.to_le_bytes());
                hasher.update(reinforcement.hits.to_le_bytes());
            }
            None => hasher.update([0]),
        }
        hasher.update((self.knowledge_writes.len() as u64).to_le_bytes());
        self.knowledge_writes
            .iter()
            .for_each(|write| hasher.update(write.digest()));
        hasher.update(self.receipt_digest);
        if let Some(snapshot) = &self.native_cycle {
            hasher.update(b"ESSMAI_NATIVE_CYCLE_SNAPSHOT_V2");
            hasher.update(snapshot.request_sha256);
            hasher.update((snapshot.response.len() as u64).to_le_bytes());
            hasher.update(&snapshot.response);
            hasher.update((snapshot.prepared.len() as u64).to_le_bytes());
            hasher.update(&snapshot.prepared);
            if !snapshot.kie_proof.is_empty() {
                hasher.update(b"ESSMAI_NATIVE_KIE_PROOF_V1");
                hasher.update((snapshot.kie_proof.len() as u64).to_le_bytes());
                hasher.update(&snapshot.kie_proof);
            }
            if let Some((binding,evidence))=snapshot.model_judgment {
                hasher.update(b"OPL2S_MODEL_JUDGMENT_V5");
                hasher.update(binding);hasher.update(evidence.to_le_bytes());
            }
        }
        hasher.finalize().into()
    }
}

pub fn verification_receipt_digest(receipt: &VerificationReceipt) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"ESSMAI_VERIFICATION_RECEIPT_V1");
    hasher.update((receipt.session_id.len() as u64).to_le_bytes());
    hasher.update(receipt.session_id.as_bytes());
    hasher.update((receipt.parent_i0.len() as u64).to_le_bytes());
    hasher.update(receipt.parent_i0.as_bytes());
    hasher.update(receipt.primitive_anchor.to_le_bytes());
    hasher.update(receipt.xy_digest.to_le_bytes());
    hasher.update(receipt.pd_binding_digest.to_le_bytes());
    hasher.update(receipt.pd_continuum_activation_digest.to_le_bytes());
    hasher.update(receipt.living_trust_digest.to_le_bytes());
    hasher.update(receipt.living_trust_sha256);
    hasher.update([receipt.y_verdict, receipt.x_verdict]);
    hasher.update(receipt.seal.to_le_bytes());
    hasher.update(receipt.generation.to_le_bytes());
    hasher.update((receipt.receipt_id.len() as u64).to_le_bytes());
    hasher.update(receipt.receipt_id.as_bytes());
    hasher.finalize().into()
}

pub fn sha256_parts(parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    parts.iter().for_each(|part| hasher.update(part));
    hasher.finalize().into()
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use crate::shadow_commit::KnowledgeWrite;
    use crate::types::{KnowledgeBand, ShadowPath, SupremeVerdict};

    pub(crate) fn sample_supreme(y: u8, x: u8) -> SupremeVerdict {
        SupremeVerdict {
            verified: y,
            primitive: x,
            knowledge_band: KnowledgeBand::Fact,
            lgc_law: 0x03,
            legacy_score: 1.0,
            lgc_dsl: String::from("LGC::TEST"),
            living_trust_sha256: [9u8; 32],
            living_trust_intensity: 1,
            living_trust_kind: 1,
            living_trust_action_state: 1,
            living_trust_action_mask: 1,
            living_trust_required_action_mask: 1,
            living_trust_system_laws_seal: 1,
            living_trust_sovereign_flags: 1,
            living_trust_sovereign_value: 1,
            project_id: 0,
            project_status: shadow_contracts::PROJECT_STATUS_NONE,
            project_context_digest: 0,
            project_evidence_digest: 0,
            project_context_sha256: [0u8; 32],
            project_evidence_sha256: [0u8; 32],
            novel_factualized: false,
            novel_trl_level: 0,
            novel_proof_score: 0,
            novel_rejection: 0,
        }
    }

    pub(crate) fn sample_transaction() -> VaultTransaction {
        sample_transaction_with([1u8; 32], [2u8; 32], b"committed-payload")
    }

    pub(crate) fn sample_transaction_with(
        identity_digest: [u8; 32],
        cycle_digest: [u8; 32],
        raw_bytes: &[u8],
    ) -> VaultTransaction {
        let confirmed = LawConfirmedVerdict::confirm(sample_supreme(1, 1), false)
            .expect("sample constructive verdict");
        let receipt = VerificationReceipt {
            session_id: String::from("session-test"),
            parent_i0: String::from("i0-test"),
            primitive_anchor: 41,
            xy_digest: 42,
            pd_binding_digest: 43,
            pd_continuum_activation_digest: 44,
            living_trust_digest: 45,
            living_trust_sha256: [9u8; 32],
            y_verdict: 1,
            x_verdict: 1,
            seal: 46,
            generation: 47,
            receipt_id: String::from("receipt-test"),
        };
        VaultTransaction::build(
            identity_digest,
            cycle_digest,
            3,
            [4u8; 32],
            [5u8; 32],
            [6u8; 32],
            [7u8; 32],
            &confirmed,
            vec![KnowledgeWrite::Primitive {
                input_id: 41,
                vector_points: 1.0,
                temporal_score: 1.0,
                mass: 1.0,
                seal: 0xA451,
                path: ShadowPath::Fast,
                domain: String::from("test"),
                raw_bytes: raw_bytes.to_vec(),
                ts_ns: 48,
            }],
            Lineage {
                input_id: 41,
                chain_mask: 0x07,
                chain_count: 3,
                proof_len: 1,
                derivation: 49,
                cap_sealed: true,
            },
            None,
            receipt,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn r35_projection_policy_is_digest_bound_and_legacy_cannot_write_live() {
        let current = test_support::sample_transaction();
        assert!(current.validate_for_live().is_ok());
        let legacy = current.clone().legacy_projection_fixture();
        assert!(legacy.validate().is_ok());
        assert!(legacy.validate_for_live().unwrap_err().to_string().contains("LIVE_STORAGE_POLICY_REQUIRED"));
        assert_ne!(legacy.transaction_id, current.transaction_id);
        let mut changed = current;
        changed.exact_candidate_identity = false;
        assert!(changed.validate().is_err());
        changed = legacy;
        changed.exact_candidate_identity = true;
        assert!(changed.validate().is_err());
    }

    #[test]
    fn transaction_roundtrip_validation_is_closed() {
        let tx = test_support::sample_transaction();
        assert_eq!(tx.validate(), Ok(()));
        assert_eq!((tx.y, tx.x), (1, 1));
    }

    #[test]
    fn receipt_pair_tampering_is_rejected() {
        let mut tx = test_support::sample_transaction();
        tx.receipt.x_verdict = 0;
        assert!(matches!(
            tx.validate(),
            Err(VaultError::CorruptTransaction(_))
        ));
    }

    #[test]
    fn write_set_tampering_is_rejected() {
        let mut tx = test_support::sample_transaction();
        match tx.knowledge_writes.first_mut().expect("sample write") {
            KnowledgeWrite::Primitive {
                input_id: _input_id,
                vector_points: _vector_points,
                temporal_score: _temporal_score,
                mass: _mass,
                seal: _seal,
                path: _path,
                domain: _domain,
                raw_bytes,
                ts_ns: _ts_ns,
            } => raw_bytes.push(0xFF),
            KnowledgeWrite::Negative {
                input_id: _input_id,
                raw_bytes: _raw_bytes,
                context: _context,
                ts_ns: _ts_ns,
            } => panic!("sample write kind"),
        }
        assert!(matches!(
            tx.validate(),
            Err(VaultError::CorruptTransaction(_))
        ));
    }

    #[test]
    fn empty_identity_or_cycle_is_rejected() {
        let mut identity = test_support::sample_transaction();
        identity.identity_digest = [0u8; 32];
        assert!(matches!(
            identity.validate(),
            Err(VaultError::CorruptTransaction(_))
        ));
        let mut cycle = test_support::sample_transaction();
        cycle.cycle_digest = [0u8; 32];
        assert!(matches!(
            cycle.validate(),
            Err(VaultError::CorruptTransaction(_))
        ));
    }
}
