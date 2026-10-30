// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  SHADOW_MATRIX.RS — S.MATRIX                                           ║
// ║  Gjata Legacy™  |  Arkitekt: Bledar Gjata                             ║
// ║                                                                          ║
// ║  Sistematizon arsyetimet nga Quantum. Ndërton dosjen e çështjes.      ║
// ║  Konsulton KnowledgeVault për kontekst (lexon — NUK shkruan).         ║
// ║  ZERO if/else. Scoring vektorial i pastër.                            ║
// ║                                                                          ║
// ║  NUK shikon primitive_flags (500) — i kalon verbërisht në passage.   ║
// ╚══════════════════════════════════════════════════════════════════════════╝

use crate::knowledge_vault::KnowledgeVault;
use crate::types::{PassPackage, ShadowError, ShadowNode, ShadowVerdict};
use std::sync::Arc;

pub struct SystematizedCase {
    pub reasoning_strength:  f32,
    pub scientific_support:  f32,
    pub sovereign_conflict:  f32,
    pub model_evidence: Option<shadow_contracts::model_operator_measurement::ModelOperatorSummary>,
    pub negative_pattern:    f32,
    pub chain_coherence:     f32,
    pub candidate_top_score: f32,
    pub final_evidence_digest: u64,
    pub pd_gcl_process_digest: u64,
    pub spine_completion_digest: u64,
}

impl SystematizedCase {
    /// Score finale e dosjes — vektor i peshuar.
    pub fn case_score(&self) -> f32 {
        (self.reasoning_strength  * 0.30
       + self.scientific_support  * 0.25
       + self.chain_coherence     * 0.25
       + (1.0 - self.sovereign_conflict) * 0.10
       + (1.0 - self.negative_pattern)   * 0.10)
        .clamp(0.0, 1.0)
    }
}

pub struct ShadowMatrix;

impl ShadowMatrix {
    /// Sistematizon PassPackage me kontekst nga KnowledgeVault.
    /// ZERO if/else — çdo dimension është projektion numerik.
    pub fn systematize(
        pkg:   &PassPackage,
        vault: &Arc<KnowledgeVault>,
        ts_ns: u64,
    ) -> (SystematizedCase, ShadowVerdict) {

        // ── Konsulto vault (lexim) ────────────────────────────────────────
        let sci_support   = vault.query_scientific_similarity(&pkg.candidate_scores);
        let sov_conflict  = vault.check_sovereign_conflict(pkg.reasoning_strength());
        let neg_pattern   = vault.query_negative_pattern(pkg.epistemic_mass);

        Self::with_dimensions(pkg, sci_support, sov_conflict, neg_pattern, ts_ns)
    }

    /// R24: same source equations, over only the presented, owner-resolved
    /// counterproofs. No Vault handle is available during this computation.
    /// This lightweight route has no ScientificEntry or SovereignEntry inputs:
    /// their empty-set result remains zero, exactly as in the source. KiE model
    /// checks and the current sovereign gates remain separate and mandatory.
    pub(crate) fn systematize_current(
        pkg: &PassPackage,
        evidence: &crate::kie_history::VerifiedCounterproofProjection,
        ts_ns: u64,
    ) -> Result<(SystematizedCase, ShadowVerdict), ShadowError> {
        let negative_pattern = evidence.negative_pattern(pkg.epistemic_mass)
            .map_err(ShadowError::SealInvalid)?;
        Ok(Self::with_dimensions(pkg, 0.0, 0.0, negative_pattern, ts_ns))
    }

    pub(crate) fn systematize_case(
        pkg: &PassPackage, history: &crate::kie_history::VerifiedCounterproofProjection,
        model: &crate::case_reasoning::VerifiedModelCase, ts_ns: u64,
    ) -> Result<(SystematizedCase, ShadowVerdict), ShadowError> {
        if !model.matches(pkg) { return Err(ShadowError::SealInvalid("MATRIX_CASE_SCOPE".into())); }
        let (mut case, mut verdict) = Self::systematize_current(pkg, history, ts_ns)?;
        case.model_evidence = Some(model.model().summary().map_err(ShadowError::SealInvalid)?);
        // A counterproof or unperformed obligation cannot be averaged away.
        let complete = model.failure_modes().is_empty();
        if model.uses_model_judgment() {
            verdict = model.review(pkg, verdict, &[complete, model.positive_count() == 1,
                model.history_permits_positive()]);
        } else {
            verdict.verdict &= u8::from(complete);
            verdict.score *= f32::from(u8::from(complete));
        }
        println!("[MATRIX/CASE] {} legacy_score={:.6} model_obligations_complete={}",
            model.model().statement(), verdict.score, complete);
        Ok((case, verdict))
    }

    fn with_dimensions(
        pkg: &PassPackage, sci_support: f32, sov_conflict: f32,
        neg_pattern: f32, ts_ns: u64,
    ) -> (SystematizedCase, ShadowVerdict) {

        // ── Reasoning strength nga PassPackage ────────────────────────────
        let r_strength = pkg.reasoning_strength();

        // ── Chain coherence ────────────────────────────────────────────────
        let chain_coh = pkg.chain_coherence.clamp(0.0, 1.0);

        // ── Top candidate score ────────────────────────────────────────────
        let top_score = pkg.candidate_scores.iter()
            .cloned()
            .fold(0.0f32, f32::max)
            .clamp(0.0, 1.0);

        let case = SystematizedCase {
            reasoning_strength:  r_strength,
            scientific_support:  sci_support,
            sovereign_conflict:  sov_conflict,
            model_evidence: None,
            negative_pattern:    neg_pattern,
            chain_coherence:     chain_coh,
            candidate_top_score: top_score,
            final_evidence_digest: pkg.final_evidence_digest,
            pd_gcl_process_digest: pkg.pd_gcl_process_digest,
            spine_completion_digest: pkg.spine_completion_digest,
        };

        let score = case.case_score();

        // Verdict: material i mjaftueshëm + nuk bie ndesh me sovereign
        // ZERO if/else — shumëzim binar
        let material_ok  = (r_strength > 0.30) as u32;
        let no_conflict  = (sov_conflict < 0.90) as u32;
        let verdict_u8   = (material_ok & no_conflict) as u8;

        let mut verdict = ShadowVerdict::from_score(
            ShadowNode::Matrix,
            score,
            0,
            ts_ns,
        );
        // Override me vendim të llogaritur — branchless AND
        verdict.verdict = verdict_u8;

        (case, verdict)
    }
}

#[cfg(test)]
mod current_cycle_tests {
    use super::*;
    use crate::types::NegativeContext;

    #[test]
    fn r24_unrelated_history_does_not_change_current_matrix() {
        let empty = Arc::new(KnowledgeVault::new());
        let with_history = Arc::new(KnowledgeVault::new());
        // Storage-only fixture for a different proposition, not native admission.
        with_history.write_negative(771, b"unrelated proposition".to_vec(), NegativeContext {
            failure_reason_code: 0x777,
            process: "another-case".into(), mass_at_fail: 0.5, frequency: 10,
        }, 1).unwrap();
        let package = PassPackage {
            package_id: "current-case".into(), epistemic_mass: 0.5,
            ibe_score: 0.8, causal_integrity: 0.8, conservation_ok: true,
            chain_coherence: 0.9, candidate_scores: vec![0.8],
            ..PassPackage::default()
        };
        let (proof, _) = crate::kie_history::test_support::empty_current_projection();
        let first_evidence = empty.presented_matrix_counterproofs(&proof).unwrap();
        let second_evidence = with_history.presented_matrix_counterproofs(&proof).unwrap();
        let (first, _) = ShadowMatrix::systematize_current(&package, &first_evidence, 2).unwrap();
        let (second, _) = ShadowMatrix::systematize_current(&package, &second_evidence, 2).unwrap();
        assert_eq!(first.case_score().to_bits(), second.case_score().to_bits(),
            "unselected history must not change the current evidence judgment");
        assert_eq!(with_history.negative_count(), 1, "history remains available for retrieval");
        let (old_empty, _) = ShadowMatrix::systematize(&package, &empty, 2);
        let (old_history, _) = ShadowMatrix::systematize(&package, &with_history, 2);
        assert_ne!(old_empty.case_score().to_bits(), old_history.case_score().to_bits(),
            "legacy interpretation is deliberately preserved");
    }

    #[test]
    fn r24_matrix_preserves_source_equations_and_strict_thresholds() {
        for negative in [0.0f32, 0.25, 1.0] {
            let package = PassPackage { ibe_score: 0.8, causal_integrity: 0.6,
                conservation_ok: true, chain_coherence: 0.7, ..PassPackage::default() };
            let (case, verdict) = ShadowMatrix::with_dimensions(&package, 0.0, 0.0, negative, 7);
            let r = 0.8f32 * 0.35 + 0.6f32 * 0.35 + 0.30;
            let expected = r * 0.30 + 0.0 * 0.25 + 0.7 * 0.25
                + 1.0 * 0.10 + (1.0 - negative) * 0.10;
            assert_eq!(case.case_score().to_bits(), expected.to_bits());
            assert_eq!(verdict.verdict, 1);
        }
        let threshold = PassPackage { conservation_ok: true, ..PassPackage::default() };
        assert_eq!(ShadowMatrix::with_dimensions(&threshold, 0.0, 0.0, 0.0, 7).1.verdict, 0);
        let strong = PassPackage { ibe_score: 1.0, causal_integrity: 1.0,
            conservation_ok: true, ..PassPackage::default() };
        assert_eq!(ShadowMatrix::with_dimensions(&strong, 0.0, 0.9, 0.0, 7).1.verdict, 0);
    }
}
