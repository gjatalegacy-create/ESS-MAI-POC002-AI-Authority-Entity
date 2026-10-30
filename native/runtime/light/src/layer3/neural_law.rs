// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  LAYER3/NEURAL_LAW.RS — Ligji Neural Interpretues (Light)            ║
// ║  GJATA LEGACY™ — AIGovern-Cyber Light                            (LAYER 3)    ║
// ║                                                                          ║
// ║  Ligji neural i jep KUPTIM çdo lëvizjeje, çdo hyrjeje/daljeje dhe çdo    ║
// ║  ligji — algoritmi i tërë sistemit AIGovern-Cyber tani flet si NJË TRUP.       ║
// ║  Kjo shtresë e tretë e forcon Light-in si interpretues universal.       ║
// ║                                                                          ║
// ║  POR LIGHT KURRË NUK VENDOS. Ligji neural vetëm INTERPRETON:           ║
// ║   • jep kuptim për çdo hyrje (çfarë është, sa e fortë, ku shkon)        ║
// ║   • jep kuptim për çdo dalje (çfarë u prodhua, me sa besueshmëri)       ║
// ║   • jep kuptim për çdo lëvizje ndër-platformë (kush, pse, çfarë)        ║
// ║                                                                          ║
// ║  Vendimi mbetet te Shadow. Presioni te Quantum. Light INTERPRETON.     ║
// ║  Zero if/else — formula + match.                                      ║
// ╚══════════════════════════════════════════════════════════════════════════╝

use crate::lab_contracts_v11::packet::{Layer3Context, Layer3PayloadKind};

// ─────────────────────────────────────────────────────────────────────────────
// MOVEMENT MEANING — kuptimi i një lëvizjeje
// ─────────────────────────────────────────────────────────────────────────────

/// Drejtimi i një lëvizjeje në sistem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowDirection {
    /// Hyrje në sistem.
    Inbound,
}

/// Kuptimi i interpretuar i një lëvizjeje — çfarë do të thotë, jo çfarë vendoset.
#[derive(Debug, Clone, PartialEq)]
pub struct MovementMeaning {
    /// Konteksti i lëvizjes.
    pub trace_id: u64,
    pub claim_id: u64,
    /// Drejtimi.
    pub direction: FlowDirection,
    /// Interpretimi (çfarë do të thotë kjo lëvizje).
    pub meaning: String,
    /// Sa e qartë/koherente është lëvizja [0,1] (interpretim, jo vendim).
    pub clarity: f32,
}

// ─────────────────────────────────────────────────────────────────────────────
// NEURAL LAW — interpreton, kurrë s'vendos
// ─────────────────────────────────────────────────────────────────────────────

pub struct NeuralLaw;

impl NeuralLaw {
    /// interpret_movement — i jep kuptim një lëvizjeje në sistem.
    ///
    /// Nga lloji i ngarkesës dhe konteksti, prodhon interpretimin.
    /// KURRË s'vendos — vetëm shpjegon çfarë do të thotë. Zero if — match.
    pub fn interpret_movement(
        context: &Layer3Context,
        payload_kind: Layer3PayloadKind,
        direction: FlowDirection,
    ) -> MovementMeaning {
        // Kuptimi nga lloji i ngarkesës.
        let meaning = Self::meaning_for(payload_kind, direction);

        // Qartësia: a ka kontekst të plotë (wisdom_ref + territory)?
        let has_wisdom = context.has_wisdom();
        let has_territory = !context.territory.is_empty();
        let clarity_base = (has_wisdom as u32 as f32) * 0.5 + (has_territory as u32 as f32) * 0.5;
        let clarity = clarity_base.clamp(0.0, 1.0);

        MovementMeaning {
            trace_id: context.trace_id,
            claim_id: context.claim_id,
            direction,
            meaning,
            clarity,
        }
    }

    /// meaning_for — kuptimi nga lloji + drejtimi. Zero if — match tuple.
    fn meaning_for(kind: Layer3PayloadKind, direction: FlowDirection) -> String {
        let kind_str = kind.label();
        // Interpretim i strukturuar (jo vendim).
        match direction {
            FlowDirection::Inbound => format!("hyrje e tipit {} — material për përpunim", kind_str),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_context() -> Layer3Context {
        Layer3Context::new(1, 2, 3, "physics".to_string(), 4, 5)
    }

    #[test]
    fn interprets_inbound() {
        let m = NeuralLaw::interpret_movement(
            &full_context(),
            Layer3PayloadKind::NeuralLaw,
            FlowDirection::Inbound,
        );
        assert_eq!(m.direction, FlowDirection::Inbound);
        assert!(m.meaning.contains("hyrje"));
        assert!(m.clarity > 0.0);
    }

    #[test]
    fn clarity_reflects_context() {
        // Kontekst i plotë → qartësi e lartë.
        let full = NeuralLaw::interpret_movement(
            &full_context(),
            Layer3PayloadKind::NeuralLaw,
            FlowDirection::Inbound,
        );
        // Kontekst pa wisdom + pa territory → qartësi e ulët.
        let empty_ctx = Layer3Context::new(1, 2, 3, "".to_string(), 4, 0);
        let empty = NeuralLaw::interpret_movement(
            &empty_ctx,
            Layer3PayloadKind::NeuralLaw,
            FlowDirection::Inbound,
        );
        assert!(full.clarity > empty.clarity);
    }

    #[test]
    fn interpretation_never_decides() {
        // Ligji neural prodhon vetëm kuptim — asnjë verdikt 0/1.
        let m = NeuralLaw::interpret_movement(
            &full_context(),
            Layer3PayloadKind::NeuralLaw,
            FlowDirection::Inbound,
        );
        // Rezultati është interpretim (meaning + clarity), jo vendim.
        assert!(!m.meaning.is_empty());
    }
}
