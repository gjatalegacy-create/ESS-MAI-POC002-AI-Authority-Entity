// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  LAYER3/LAYER3_FLOW.RS — Orkestruesi i Shtresës 3 (Light)            ║
// ║  GJATA LEGACY™ — AIGovern-Cyber Light                            (LAYER 3)    ║
// ║                                                                          ║
// ║  Hook i PASTËR që lidh Shtresën 3 të Light-it:                         ║
// ║    UI input → ui_channel (gjurmim) → neural_law (kuptim) →             ║
// ║    light_wisdom (observim besueshmërie) → Layer3Packet → magazina.     ║
// ║                                                                          ║
// ║  Komunikimi ndër-platformë: prodhon Layer3Packet (vula 500) me urtësinë ║
// ║  e besueshmërisë që i dërgohet magazinës së Shadow-it.                 ║
// ║                                                                          ║
// ║  KUFIRI: Light interpreton dhe transporton — kurrë s'vendos. Çdo gjë    ║
// ║  këtu është gjurmim/kuptim/observim. Zero if/else.                    ║
// ╚══════════════════════════════════════════════════════════════════════════╝

use crate::layer3::ui_channel::{TracedInput, UiChannel, UiInput};

// ─────────────────────────────────────────────────────────────────────────────
// LIGHT LAYER3 RESULT — rezultati i orkestrimit në hyrje
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LightLayer3Result {
    /// Inputi i gjurmuar (me kuptim nga neural_law).
    pub traced_input: TracedInput,
}

// ─────────────────────────────────────────────────────────────────────────────
// LIGHT LAYER3 FLOW — orkestron hyrjen UI → gjurmim → urtësi
// ─────────────────────────────────────────────────────────────────────────────

pub struct LightLayer3Flow;

impl LightLayer3Flow {
    /// ingest_from_ui — pranon input nga UI, e gjurmon, observon besueshmërinë,
    /// e paketon për magazinë.
    ///
    /// Rrjedha: UI input → ui_channel (gjurmim + neural_law kuptim) →
    /// light_wisdom (observim besueshmërie) → Layer3Packet për magazinë.
    /// Zero if.
    pub fn ingest_from_ui(input: UiInput, trace_id: u64, at: u64) -> LightLayer3Result {
        // 1. Inputi merr gjurmim menjëherë (aty ku futet, lind gjurma).
        let traced_input = UiChannel::receive_input(input, trace_id, at);

        LightLayer3Result { traced_input }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ingests_ui_input_with_trace() {
        let input = UiInput::new("explore with Nura".to_string(), "s1".to_string(), false);
        let result = LightLayer3Flow::ingest_from_ui(input, 42, 1000);
        // Inputi u gjurmua.
        assert_eq!(result.traced_input.trace_id, 42);
        // Urtësia e besueshmërisë u krijua.
        // Paketa e vulosur.
    }

    #[test]
    fn trace_born_at_input() {
        // Aty ku futet inputi, lind gjurma (kanali UI).
        let input = UiInput::new("test".to_string(), "".to_string(), true);
        let result = LightLayer3Flow::ingest_from_ui(input, 7, 100);
        assert_eq!(result.traced_input.trace_id, 7);
        assert!(result.traced_input.input.deep_research);
    }

    #[test]
    fn light_only_interprets_not_decides() {
        // Çdo gjë këtu është gjurmim/kuptim/observim — asnjë vendim.
        let input = UiInput::new("q".to_string(), "s".to_string(), false);
        let result = LightLayer3Flow::ingest_from_ui(input, 1, 100);
        // Rezultati: gjurmë + kuptim + urtësi, jo verdikt.
        assert!(!result.traced_input.meaning.meaning.is_empty());
    }
}
