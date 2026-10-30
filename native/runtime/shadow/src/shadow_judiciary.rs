// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  SHADOW_JUDICIARY.RS — S.JUDICIARY                                     ║
// ║  Gjata Legacy™  |  Arkitekt: Bledar Gjata                             ║
// ║  Porta e parafundit. Mbledh TË GJITHA verdiktet — vendos 0/1 final.  ║
// ║  NUK shikon primitive_flags. E kalon në shadow_gj_legacy.            ║
// ╚══════════════════════════════════════════════════════════════════════════╝

use crate::types::{ShadowNode, ShadowPassage, ShadowVerdict};

pub struct ShadowJudiciary;

impl ShadowJudiciary {
    pub(crate) fn judge_model(passage: &ShadowPassage,
        case: &crate::case_reasoning::VerifiedModelCase, ts_ns: u64) -> ShadowVerdict {
        let expected = crate::sovereign_guard::expected_path(passage);
        let exact = passage.verdicts.iter().map(|v| v.node)
            .eq(expected[..expected.len()-1].iter().copied());
        case.review(&passage.package, Self::judge(passage, ts_ns), &[
            exact, passage.verdicts.iter().all(|v| v.verdict == 1),
            passage.multi_verification_verdict() == 1, case.admissible(&passage.package)])
    }

    /// Mbledh të gjitha verdiktet e akumuluara dhe jep 0/1 final.
    ///
    /// ZERO if/else — shumëzim vektorial i scoreve.
    /// Nëse çdo nod ka score > 0.5 → judiciary_score i lartë → 1.
    pub fn judge(passage: &ShadowPassage, ts_ns: u64) -> ShadowVerdict {
        match passage.verdicts.is_empty() {
            true => return ShadowVerdict::from_score(ShadowNode::Judiciary, 0.0, 0, ts_ns),
            false => {}
        }

        let n = passage.verdicts.len() as f32;

        // Score agregat i peshuar — nodi i fundit peshon më shumë
        let weighted_sum: f32 = passage.verdicts.iter()
            .enumerate()
            .map(|(i, v)| {
                let weight = 0.5 + 0.5 * (i as f32 / n.max(1.0));
                v.score * weight
            })
            .sum();

        let weight_total: f32 = (0..passage.verdicts.len())
            .map(|i| 0.5 + 0.5 * (i as f32 / n.max(1.0)))
            .sum();

        let judiciary_score = (weighted_sum / weight_total.max(0.001)).clamp(0.0, 1.0);

        // Numëro nodët që kanë kaluar — threshold 0.5
        let passed: u32 = passage.verdicts.iter()
            .map(|v| (v.score >= 0.5) as u32)
            .sum();

        let pass_ratio = passed as f32 / n;

        // Score finale = mesatar gjeometrik, i mbyllur nga porta e
        // multi-verifikimit. Mungesa/dështimi i portës nuk mund të mesatarizohet.
        let multi_gate = passage.multi_verification_verdict() as f32;
        let final_score = (judiciary_score * pass_ratio).sqrt().clamp(0.0, 1.0) * multi_gate;

        ShadowVerdict::from_score(ShadowNode::Judiciary, final_score, passed as u8, ts_ns)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{LightEnvelope, PassPackage, ShadowPath};

    #[test]
    fn missing_multi_verification_is_fail_closed() {
        let mut passage = ShadowPassage::new(
            1,
            PassPackage::default(),
            LightEnvelope::default(),
            ShadowPath::Fast,
        );
        passage.push_verdict(ShadowVerdict::from_score(ShadowNode::Router, 0.9, 0, 1));
        passage.push_verdict(ShadowVerdict::from_score(ShadowNode::Matrix, 0.9, 0, 2));
        let verdict = ShadowJudiciary::judge(&passage, 3);
        assert_eq!(verdict.verdict, 0);
    }

    #[test]
    fn passing_multi_verification_allows_judiciary_score() {
        let mut passage = ShadowPassage::new(
            1,
            PassPackage::default(),
            LightEnvelope::default(),
            ShadowPath::Fast,
        );
        passage.push_verdict(ShadowVerdict::from_score(ShadowNode::Router, 0.9, 0, 1));
        passage.push_verdict(ShadowVerdict::from_score(ShadowNode::Matrix, 0.9, 0, 2));
        passage.push_multi_verification(
            crate::lab_contracts::verdict::VerificationState::PressureTested,
            0.5,
            0.9,
            1,
            0xA9,
            3,
        );
        let verdict = ShadowJudiciary::judge(&passage, 4);
        assert_eq!(verdict.verdict, 1);
    }
    #[test]
    fn r33_each_required_node_and_exact_order_remain_mandatory() {
        let (proof,request)=crate::case_reasoning::tests::judgment_material();
        let case=crate::case_reasoning::VerifiedModelCase::from_request(&proof,&request).unwrap().unwrap();
        for defect in 0..9 {
            let pkg=PassPackage {package_id:request.quantum.session_id.clone(),
                final_evidence_digest:request.quantum.final_evidence.recompute_digest(),..Default::default()};
            let mut p=ShadowPassage::new(1,pkg,LightEnvelope::default(),ShadowPath::Standard);
            let nodes=[ShadowNode::Router,ShadowNode::Matrix,ShadowNode::Gen5,ShadowNode::Type,
                ShadowNode::Sovereign,ShadowNode::MultiVerification];
            for node in nodes {
                p.push_verdict(ShadowVerdict {node,score:0.01,verdict:1,reason_code:0x80,ts_ns:1});
            }
            match defect {
                0=>{},
                1..=6=>p.verdicts[defect-1].verdict=0,
                7=>p.verdicts.swap(0,1),
                _=>{p.verdicts.pop();},
            }
            let result=ShadowJudiciary::judge_model(&p,&case,2);
            assert_eq!(result.verdict,u8::from(defect==0),"defect={defect}");
            assert!(result.score<0.70);
        }
    }

}
