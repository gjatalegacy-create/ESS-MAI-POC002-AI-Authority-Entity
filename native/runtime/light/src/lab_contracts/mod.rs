// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  LAB_CONTRACTS/MOD.RS — Gjuha e Përbashkët e Tre Laboratorëve         ║
// ║  GJATA LEGACY™ — AIGovern-Cyber (AIGovern-Cyber Light: nënndarje e përdorur)      ║
// ║                                                                          ║
// ║  Unified Layer 2 Verification Laboratory Framework — themeli.          ║
// ║                                                                          ║
// ║  Kjo dosje është GJUHA, jo FORCA dhe jo ROLI:                          ║
// ║    • lab_contracts = gjuhë e përbashkët (kjo dosje)                    ║
// ║    • Layer 2       = forcë (specifike për platformë)                   ║
// ║    • Platforma     = rol (Light/Quantum/Shadow)                        ║
// ║                                                                          ║
// ║  Mban VETËM kontrata (tipe): matje, gjendje, evidencë, vendim, gjurmë,  ║
// ║  mesazh, histori verifikimi. ASNJË logjikë platforme. ASNJË presion     ║
// ║  (presioni jeton te PressureReport + Layer 2 i secilës platformë).     ║
// ║                                                                          ║
// ║  AIGovern-Cyber Light ruan nënndarjen që përdor; ndryshimet gjurmohen në       ║
// ║  Q7/provenance. Source-i origjinal ruhet vetëm për lexim.    ║
// ╚══════════════════════════════════════════════════════════════════════════╝

pub mod collapse; // Kolapsi 3-fazor — formula e gdhendur si kontratë
pub mod domains; // CoreDomain — regjistri kanonik i domeneve bërthamë
pub mod first_guardian;
pub mod gcl_presume; // GATISHMËRIA NËN LIGJ — ping/rol/status nën CollapsePhase (v1.4.1)
pub mod gjata_collapse_law; // LIGJI KUSHTETUES: AIGovern-Cyber == gjata_collapse_law (Law 0)
pub use shadow_contracts::boundary::pa_wire; // R15: single source
pub mod rrjedha; // INTELIGJENCA E DËSHTIMEVE — pse + shënim + kufi (kurrë bllokuese)
pub mod trl; // Actual main confidence conversion.
pub mod trust; // TrustContext — konteksti i besimit (Light prodhon)
pub use shadow_contracts::boundary::verification_receipt; // R15: single source

// ── Ri-eksportim i sheshtë (gjuha e plotë në një vend) ──────────────────────
pub use collapse::{CollapsePhase, PrimitiveSplit};
pub use domains::normalize_domain_key;
pub use gjata_collapse_law::{proclaim_law, system_laws_seal, GjataCollapseLaw, SupremeDirective};
#[cfg(test)]
pub use trust::{TrustContext, TrustGuidance};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trust_context_empty_unknown() {
        let t = TrustContext::empty(5, "physics".to_string());
        assert!(!t.has_history());
        assert_eq!(t.guidance, TrustGuidance::Unknown);
        assert_eq!(t.combined_strength(), 0.0);
    }

    #[test]
    fn trust_context_combined_strength() {
        let t = TrustContext {
            trace_id: 1,
            domain: "d".to_string(),
            trust_score: 1.0,
            reliability_score: 1.0,
            verification_score: 1.0,
            history_refs: vec![1, 2],
            guidance: TrustGuidance::Recommended,
        };
        assert!(t.has_history());
        // 1.0*0.40 + 1.0*0.25 + 1.0*0.35 = 1.0
        assert!((t.combined_strength() - 1.0).abs() < 1e-6);
    }
}
