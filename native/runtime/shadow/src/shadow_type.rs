// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  SHADOW_TYPE.RS — S.TYPE                                               ║
// ║  Gjata Legacy™  |  Arkitekt: Bledar Gjata                             ║
// ║  Verifikon gjendjen e paketimit të Quantum.                           ║
// ╚══════════════════════════════════════════════════════════════════════════╝

use crate::types::{ShadowNode, ShadowPassage, ShadowVerdict};

pub struct ShadowType;

impl ShadowType {
    pub(crate) fn verify_model(passage: &ShadowPassage,
        case: &crate::case_reasoning::VerifiedModelCase, origin_session: &str, ts_ns: u64) -> ShadowVerdict {
        case.review(&passage.package, Self::verify(passage, ts_ns),
            &[case.model_only(&passage.package), case.model().cycle() == origin_session])
    }

    /// Verifikon paketimin e gjendjes nga Quantum.
    /// Score = kombinim i IBE + causal_integrity + chain_coherence.
    /// ZERO if/else.
    pub fn verify(passage: &ShadowPassage, ts_ns: u64) -> ShadowVerdict {
        let pkg = &passage.package;

        // Packaging strength — sa mirë është paketuar arsyetimi
        let ibe_c     = pkg.ibe_score.clamp(0.0, 1.0);
        let causal_c  = pkg.causal_integrity.clamp(0.0, 1.0);
        let chain_c   = pkg.chain_coherence.clamp(0.0, 1.0);
        let pot_c     = pkg.potentiality_score.clamp(0.0, 1.0);

        // Tensor: vektor peshash × vektor skoresh
        let score = (ibe_c    * 0.30
                   + causal_c * 0.30
                   + chain_c  * 0.25
                   + pot_c    * 0.15)
                   .clamp(0.0, 1.0);

        // Konsistencë me verdiktin e Matrix
        let matrix_score = passage.verdicts.first()
            .map(|v| v.score)
            .unwrap_or(0.0);

        // Penalizim i devijimit nga Matrix — zero if/else
        let deviation  = (score - matrix_score).abs();
        let final_score = (score * (1.0 - deviation * 0.3)).clamp(0.0, 1.0);

        ShadowVerdict::from_score(ShadowNode::Type, final_score, 0, ts_ns)
    }
}


#[cfg(test)]
mod model_tests {
    use super::*;
    #[test]
    fn r33_origin_identity_is_borrowed_after_gateway_moves_session() {
        let (proof,request)=crate::case_reasoning::tests::judgment_material();
        let case=crate::case_reasoning::VerifiedModelCase::from_request(&proof,&request).unwrap().unwrap();
        let pkg=crate::types::PassPackage {package_id:request.quantum.session_id.clone(),
            final_evidence_digest:request.quantum.final_evidence.recompute_digest(),..Default::default()};
        let mut light=crate::types::LightEnvelope::sealed(&request.light.session_id,"model",vec![1,2,3],vec![]);
        let owner_session=std::mem::take(&mut light.session_id);
        let p=ShadowPassage::new(1,pkg,light,crate::types::ShadowPath::Standard);
        assert!(p.light.session_id.is_empty());
        assert_eq!(ShadowType::verify_model(&p,&case,&owner_session,1).verdict,1);
        assert_eq!(ShadowType::verify_model(&p,&case,"",1).verdict,0);
        assert_eq!(ShadowType::verify_model(&p,&case,"foreign",1).verdict,0);
    }
}
