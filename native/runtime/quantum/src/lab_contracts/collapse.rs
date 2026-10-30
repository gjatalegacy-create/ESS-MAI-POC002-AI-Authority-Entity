// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  LAB_CONTRACTS/COLLAPSE.RS — Kontrata e Kolapsit Sovran                  ║
// ║  GJATA LEGACY™ — AIGovern-Cyber (BYTE-FOR-BYTE IDENTIK në 3 platformat)         ║
// ║                                                                          ║
// ║  FORMULA E GDHENDUR:                                                     ║
// ║    i0 → PA → (Xi, Yi) → +(-Xi)+(-Yi) → XiYi → XY → D ∈ {0,1}            ║
// ║                                                                          ║
// ║  KOLAPSI 3-FAZOR (formula NUK është modul — është vetë rrjedha):        ║
// ║    Light   = KOLAPS KOORDINIMI  — i0 gjurmohet, PA lind, split Xi/Yi    ║
// ║    Quantum = KOLAPS ARSYETIMI   — LIM mat 8 akset, shënon rrugët        ║
// ║              negative, NPRO eliminon (-Xi)+(-Yi), PRO kërkon i~,        ║
// ║              MPRO/PIM nxjerrin e paketojnë XY, NPIM paketon -X&-Y       ║
// ║    Shadow  = KOLAPS VERIFIKIMI  — XY ↔ (Xi,Yi) në pritje → D ∈ {0,1}   ║
// ║                                                                          ║
// ║  LIGJI: kolapsi është MONOTON — hapësira vetëm ngushtohet, kurrë        ║
// ║  s'zgjerohet; kur zbrazet → REFUZIM (D=0), kurrë shpikje përgjigjeje.   ║
// ║  Rrugët e eliminuara RUHEN si dije negative (aset, jo mbeturinë).       ║
// ║                                                                          ║
// ║  LIGJE KODI: zero if/else (vetëm match), zero varësi, zero clone i      ║
// ║  panevojshëm. Vetëm tipe + wire-format — asnjë reasoning këtu.          ║
// ╚══════════════════════════════════════════════════════════════════════════╝

/// Faza e kolapsit ka një identitet fizik të vetëm në gcl-constitution.
/// Ky re-export ruan path-in ekzistues `lab_contracts::collapse::CollapsePhase`.
pub use gcl_constitution::CollapsePhase;

/// R15: the Xi/Yi split wire has one definition, shared by the three platforms.
pub use shadow_contracts::boundary::split_wire::PrimitiveSplit;

/// Rrugë e shënuar NEGATIVE gjatë kolapsit të arsyetimit:
/// LIM (nën pragjet e 8 akseve) e përcakton; NPRO e eliminon; NPIM e paketon;
/// Shadow e ruan si NegativeKnowledgeRecord (dije negative = ASET).
#[derive(Debug, Clone, PartialEq)]
pub struct NegativePath {
    /// Termi/rruga që u eliminua (-Xi ose -Yi).
    pub term: String,
    /// Hapësira nga u eliminua: "Xi" | "Yi".
    pub space: &'static str,
    /// Arsyeja e shkurtër (aksi LIM ose operatori që e rrëzoi).
    pub reason: String,
}

impl NegativePath {
    /// Exact negative term/reason transport; no loss of '~' or whitespace.
    pub fn to_wire(&self) -> String {
        shadow_contracts::collapse_wire::encode_negative(&self.term, self.space, &self.reason)
    }

    pub fn from_wire(wire: &str) -> Option<Self> {
        let (term, space, reason) = shadow_contracts::collapse_wire::decode_negative(wire)?;
        Some(Self { term, space, reason })
    }
}

/// Rezultati i kolapsit të arsyetimit (Quantum → Shadow):
/// ose çifti i mbijetuar XY me masën e matur (MPRO/PIM), ose refuzim i
/// ndershëm me arsye (hapësira u zbraz — sistemi NUK shpik përgjigje).
#[derive(Debug, Clone, PartialEq)]
pub enum CollapseOutcome {
    /// XiYi → XY: çifti minimal që mbijetoi eliminimin, gati për verifikim.
    Proposed { x: String, y: String, mass: f32 },
    /// Hapësira u zbraz ose s'pati material: kolaps i refuzuar (→ D=0).
    Refused { reason: String },
}

impl CollapseOutcome {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Proposed { .. } => "XY_PROPOZUAR",
            Self::Refused { .. } => "KOLAPS_REFUZUAR",
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// TESTE — kontrata e kolapsit provohet identikisht nga të tre platformat
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_path_wire_roundtrip() {
        let n = NegativePath {
            term: "pinguin".into(),
            space: "Xi",
            reason: "semantic_integrity<prag".into(),
        };
        let back = NegativePath::from_wire(&n.to_wire()).expect("parse");
        assert_eq!(back, n);
        assert!(NegativePath::from_wire("a~KEQ~b").is_none()); // hapësirë e panjohur
    }

    #[test]
    fn outcome_labels_binary_semantics() {
        let p = CollapseOutcome::Proposed {
            x: "flamuri".into(),
            y: "shqipëria".into(),
            mass: 0.83,
        };
        let r = CollapseOutcome::Refused {
            reason: "XiYi=∅".into(),
        };
        assert_eq!(p.label(), "XY_PROPOZUAR");
        assert_eq!(r.label(), "KOLAPS_REFUZUAR");
    }

    #[test]
    fn phases_cover_three_platforms() {
        assert_eq!(CollapsePhase::Coordination.label(), "KOLAPS_KOORDINIMI");
        assert_eq!(CollapsePhase::Reasoning.label(), "KOLAPS_ARSYETIMI");
        assert_eq!(CollapsePhase::Verification.label(), "KOLAPS_VERIFIKIMI");
    }
}

#[cfg(test)]
mod r15_negative_preservation {
    use super::*;
    #[test]
    fn negative_term_and_reason_are_exact() {
        for term in ["flamur,", "a~b", " kuq|zi ", "a\nb"] {
            let n = NegativePath { term: term.into(), space: "Xi", reason: " mungon~provë|e saktë\n ".into() };
            assert_eq!(NegativePath::from_wire(&n.to_wire()), Some(n));
        }
    }
}
