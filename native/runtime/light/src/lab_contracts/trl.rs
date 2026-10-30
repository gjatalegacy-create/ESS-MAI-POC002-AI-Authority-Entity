// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  LAB_CONTRACTS/TRL.RS — Kontrata e Dëshmisë TRL (e re, minimale)      ║
// ║  GJATA LEGACY™ — AIGovern-Cyber (AIGovern-Cyber Light: kontrata të përdorura)        ║
// ║                                                                          ║
// ║  Kjo kontratë është GJUHA E PËRBASHKËT e matjes TRL — jo presioni.      ║
// ║  Presioni jeton te PressureReport (pressure.rs), JO këtu.             ║
// ║                                                                          ║
// ║  TrlEvidence tregon:                                                   ║
// ║    • çfarë u pa / mat / provua                                        ║
// ║    • sa thellë u testua                                               ║
// ║    • a u kalua cross-domain                                           ║
// ║    • a u ndikua nga presioni (vetëm flamur, JO raporti)              ║
// ║                                                                          ║
// ║  E pavarur nga strukturat e vjetra të secilës platformë: minimale,    ║
// ║  universale, e transferueshme. Zero logjikë platforme. Zero if/else.  ║
// ╚══════════════════════════════════════════════════════════════════════════╝

// ════════════════════════════════════════════════════════════════════════════
// TRL-VEKTOR (v1.4.4) — SHTYLLA DEEPTECH E AIGovern-Cyber
// Deeptech matet me TRL. AIGovern-Cyber e mban si VEKTOR tre-dimensional: hyrja
// (Light), arsyetimi (Quantum), verifikimi (Shadow) — secila platformë mban
// dimensionin e vet; pjekuria e sistemit = minimumi (zinxhiri i dobët flet).
// Kjo është ura drejt laboratorëve realë (DigitalLab TRL1–TRL3 në Quantum,
// TrlVerdict deri TRL4 në Shadow) dhe drejt gjuhës së EIC/Startup Albania.
// ════════════════════════════════════════════════════════════════════════════

/// Vektori TRL i një prove — tre dimensione, zero if/else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrlVector;

impl TrlVector {
    /// v1.4.5 (audit p.4): TRL nga një besueshmëri [0,1] → nivel 0..4.
    /// Ligji i pragjeve (deeptech: TRL rritet me provën): zero if — match
    /// mbi krahasime të renditura. 0 = pa sinjal; 4 = provë e fortë.
    pub fn trl_from_confidence(conf: f32) -> u8 {
        let c = match conf.is_finite() {
            true => conf,
            false => 0.0,
        };
        match (c >= 0.90, c >= 0.75, c >= 0.50, c >= 0.25) {
            (true, _, _, _) => 4,
            (false, true, _, _) => 3,
            (false, false, true, _) => 2,
            (false, false, false, true) => 1,
            _ => 0,
        }
    }
}

#[cfg(test)]
mod trl_vector_tests {
    use super::TrlVector;

    #[test]
    fn trl_from_confidence_follows_thresholds() {
        assert_eq!(TrlVector::trl_from_confidence(0.95), 4);
        assert_eq!(TrlVector::trl_from_confidence(0.80), 3);
        assert_eq!(TrlVector::trl_from_confidence(0.60), 2);
        assert_eq!(TrlVector::trl_from_confidence(0.30), 1);
        assert_eq!(TrlVector::trl_from_confidence(0.10), 0);
        assert_eq!(TrlVector::trl_from_confidence(f32::NAN), 0);
    }
}
