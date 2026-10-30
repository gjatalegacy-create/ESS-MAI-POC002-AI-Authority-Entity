//! Typed current KiE arithmetic. This is evidence data, never confidence or authority.
//! The circular residual uses the existing ERA arithmetic validator. Margin
//! units remain radians: S=(tau-r)+, C=(r-tau)+; equality is unresolved.
//! Live and replay owners must compare this value with their own provider run.
use crate::{kie::KieProof, pd_memory::{hex, iz_science_math::{comparison_code, EraComparison}}};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KieMeasurement {
    hashes: [[u8; 32]; 5], // proof, query, candidate bytes, model, computed result
    values: [u64; 6], // exact f64 bits: claim, output, residual, tolerance, S, C
}
impl KieMeasurement {
    pub const WIRE_BYTES: usize = 208;

    pub fn from_comparison(proof: &KieProof, value: &EraComparison) -> Result<Self, String> {
        let positive = (value.tolerance_radians - value.residual_radians).max(0.0);
        let counter = (value.residual_radians - value.tolerance_radians).max(0.0);
        let result = Self {
            hashes: [proof.proof_sha256, value.query_sha256,
                Sha256::digest(&proof.candidate_bytes).into(), value.model_sha256, value.result_sha256],
            values: [value.claim_radians, value.computed_radians, value.residual_radians,
                value.tolerance_radians, positive, counter].map(f64::to_bits),
        };
        result.validate_model(proof)?;
        Ok(result)
    }
    fn comparison(&self) -> EraComparison {
        EraComparison {
            query_sha256: self.hashes[1], result_sha256: self.hashes[4], model_sha256: self.hashes[3],
            claim_radians: f64::from_bits(self.values[0]), computed_radians: f64::from_bits(self.values[1]),
            residual_radians: f64::from_bits(self.values[2]), tolerance_radians: f64::from_bits(self.values[3]),
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.hashes.contains(&[0;32]) { return Err("KIE_MEASUREMENT_UNBOUND".into()); }
        let value = self.comparison();
        comparison_code(&value)?;
        let expected = [(value.tolerance_radians-value.residual_radians).max(0.0),
            (value.residual_radians-value.tolerance_radians).max(0.0)].map(f64::to_bits);
        if self.values[4..] != expected { return Err("KIE_MEASUREMENT_MARGIN".into()); }
        Ok(())
    }
    /// Binding and arithmetic only; the owner still executes the installed model.
    pub fn validate_model(&self, proof: &KieProof) -> Result<(), String> {
        self.validate()?;
        proof.validate()?;
        let query: [u8;32] = Sha256::digest(&proof.query_bytes).into();
        let candidate: [u8;32] = Sha256::digest(&proof.candidate_bytes).into();
        if self.hashes[0] != proof.proof_sha256 || self.hashes[1] != query || self.hashes[2] != candidate
            || comparison_code(&self.comparison())? != proof.relation {
            return Err("KIE_MEASUREMENT_PROOF_BINDING".into());
        }
        Ok(())
    }
    pub fn relation(&self) -> Result<u8, String> { comparison_code(&self.comparison()) }
    pub fn positive_margin_radians(&self) -> f64 { f64::from_bits(self.values[4]) }
    pub fn counter_margin_radians(&self) -> f64 { f64::from_bits(self.values[5]) }
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut out=Vec::with_capacity(Self::WIRE_BYTES);
        for hash in self.hashes { out.extend_from_slice(&hash); }
        for value in self.values { out.extend_from_slice(&value.to_le_bytes()); }
        Ok(out)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len()!=Self::WIRE_BYTES { return Err("KIE_MEASUREMENT_LENGTH".into()); }
        let mut hashes=[[0;32];5];
        for (hash, part) in hashes.iter_mut().zip(bytes[..160].chunks_exact(32)) { hash.copy_from_slice(part); }
        let mut values=[0;6];
        for (value, part) in values.iter_mut().zip(bytes[160..].chunks_exact(8)) {
            *value=u64::from_le_bytes(part.try_into().map_err(|_|"KIE_MEASUREMENT_LENGTH")?);
        }
        let result=Self{hashes,values};
        result.validate()?;
        Ok(result)
    }
    pub fn render(&self) -> Result<String,String> {
        self.validate()?;
        let v=self.values.map(f64::from_bits);
        let relation=match comparison_code(&self.comparison())? {
            0=>"KiS=1 KiC=0 U=0", 1=>"KiS=0 KiC=1 U=0", _=>"KiS=0 KiC=0 U=1",
        };
        Ok(format!("MODEL_RESULT MODEL_COMPUTED: ERA={:.17e} rad; pretendimi={:.17e} rad; residual_rad={:.17e}; toleranca={:.17e} rad. {}. positive_margin_rad={:.17e}; counter_margin_rad={:.17e}. F64hex ERA={:016x} pretendimi={:016x} r={:016x} toleranca={:016x} positive_margin={:016x} counter_margin={:016x}. Keto jane largesi numerike nga kufiri i deklaruar, jo confidence, mase historike ose prova te pavarura. Mbështetja ose kundërshtimi vlen vetëm nën modelin dhe hyrjet e deklaruara. model_sha256={}; result_sha256={}. ",
            v[1],v[0],v[2],v[3],relation,v[4],v[5],self.values[1],self.values[0],self.values[2],
            self.values[3],self.values[4],self.values[5],hex(&self.hashes[3]),hex(&self.hashes[4])))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(relation:u8) -> (KieProof, KieMeasurement) {
        let proof=crate::iz_frontier::test_proof(relation);
        let tolerance=match relation {0=>0.5,1=>0.125,_=>0.25};
        let actual=EraComparison {query_sha256:Sha256::digest(&proof.query_bytes).into(),
            result_sha256:[8;32],model_sha256:[9;32],claim_radians:0.0,computed_radians:0.25,
            residual_radians:0.25,tolerance_radians:tolerance};
        let measurement=KieMeasurement::from_comparison(&proof,&actual).unwrap();
        (proof,measurement)
    }
    #[test]
    fn r27_numeric_codec_preserves_both_margins_and_equality() {
        for relation in 0..3 {
            let (proof,m)=fixture(relation);
            assert_eq!(KieMeasurement::decode(&m.encode().unwrap()).unwrap(),m);
            m.validate_model(&proof).unwrap();
            let expected=match relation {0=>[0.25,0.0],1=>[0.0,0.125],_=>[0.0,0.0]};
            assert_eq!(m.values[4..],expected.map(f64::to_bits));
            let text=m.render().unwrap();
            assert!(text.contains("MODEL_RESULT MODEL_COMPUTED"));
            assert!(text.contains("positive_margin_rad=")&&text.contains("counter_margin_rad="));
            assert!(text.contains("jo confidence"));
        }
    }
    #[test]
    fn r27_numeric_tamper_or_other_proof_cannot_rebind_a_measurement() {
        let (proof,m)=fixture(0);
        let bytes=m.encode().unwrap();
        for len in [0,159,207,209] {
            assert!(KieMeasurement::decode(&vec![0;len]).is_err());
        }
        for index in [160+16,160+32,160+40] {
            let mut altered=bytes.clone(); altered[index]^=1;
            assert!(KieMeasurement::decode(&altered).is_err());
        }
        let mut changed=proof.clone();
        changed.query_bytes.push(3);
        let changed=changed.seal_data().unwrap();
        assert!(m.validate_model(&changed).is_err());
        let mut changed=m.clone(); changed.hashes[3]=[0;32];
        assert!(changed.validate().is_err());
    }
}

