//! Data-only presentation of one bounded model comparison in the current case.
//! Margins remain radians. This record supplies neither confidence nor authority.
use crate::{kie::KieProof, model_operator_measurement::{ModelOperatorMeasurements, ModelOperatorSummary}};
use sha2::{Digest, Sha256};

pub const PROOF_SOURCE: &str = "KIE_CASE_V1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelReasoning {
    cycle: String,
    binding: [u8; 32],
    model: ModelOperatorMeasurements,
}
impl ModelReasoning {
    pub fn new(cycle: &str, proof: &KieProof, model: ModelOperatorMeasurements)
        -> Result<Self, String> {
        proof.validate()?;
        model.validate_model(proof)?;
        let request = crate::decode_selection_request(&proof.historical_request)
            .map_err(|e| format!("CASE_HISTORY:{e}"))?;
        if cycle.is_empty() || cycle != request.session_id || cycle != request.trace_id {
            return Err("CASE_CURRENT_CYCLE_MISMATCH".into());
        }
        let identity = proof.context.identity.key()?;
        let bytes = model.encode()?;
        let mut hash = Sha256::new();
        for part in [
            b"OPL2S/CASE/MODEL/1".as_slice(), cycle.as_bytes(), identity.as_slice(),
            proof.context.operation_id.as_bytes(), proof.context.record_id.as_slice(),
            proof.canonical_proposition.as_bytes(), proof.proof_sha256.as_slice(),
            bytes.as_slice(),
        ] {
            hash.update((part.len() as u64).to_le_bytes());
            hash.update(part);
        }
        Ok(Self { cycle: cycle.into(), binding: hash.finalize().into(), model })
    }
    pub fn summary(&self) -> Result<ModelOperatorSummary, String> { self.model.summarize() }
    pub fn model(&self) -> &ModelOperatorMeasurements { &self.model }
    pub fn into_model(self) -> ModelOperatorMeasurements { self.model }
    pub fn binding(&self) -> [u8; 32] { self.binding }
    pub fn cycle(&self) -> &str { &self.cycle }
    pub fn statement(&self) -> String {
        // The constructor validates the private measurement; this cannot become
        // an independent probability or a historical candidate by formatting.
        let s = self.summary().expect("validated case model");
        format!("binding={}; P={}; N={}; tested={}:{}; unresolved={}; S_rad_bits={:016x}; C_rad_bits={:016x}; scope=DECLARED_MODEL",
            crate::pd_memory::hex(&self.binding),
            s.pro.candidates.count, s.npro.candidates.count,
            u8::from(s.pro.predicate_tested), u8::from(s.npro.predicate_tested),
            u8::from(s.unresolved),
            self.model.measurement().positive_margin_radians().to_bits(),
            self.model.measurement().counter_margin_radians().to_bits())
    }
    pub fn wire_record(&self) -> String { format!("{PROOF_SOURCE}|{}|1", self.statement()) }
    pub fn verify_record(&self, chain: &[String]) -> Result<(), String> {
        let prefix = format!("{PROOF_SOURCE}|");
        let entries: Vec<_> = chain.iter().filter(|entry| entry.starts_with(&prefix)).collect();
        if entries.len() != 1 || entries[0] != &self.wire_record() {
            return Err("CASE_MODEL_PROOF_CHAIN_MISMATCH".into());
        }
        Ok(())
    }
}
