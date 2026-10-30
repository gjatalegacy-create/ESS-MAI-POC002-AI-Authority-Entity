// AIGovern-Cyber v1.6.0: Ky është LIGHT EMOTIONAL/INTERPRETATION SPINE për UI-në e
// vjetër. Nuk është PD Spine 9 i Quantum dhe nuk ekzekuton Layer 1/2/3 të PD.
// Ai pasqyron gjendjen e sistemit dhe mbetet paralel me Nura-n.

// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  LIGHT_SPINE.RS — Kurrizi Vertikal i Light-it (PATCHIM)              ║
// ║  GJATA LEGACY™ — AIGovern-Cyber Light                            (SPINE)       ║
// ║                                                                          ║
// ║  PATCHIM I THELLË: lidh shtresat e Light-it në NJË rrjedhë interpretimi. ║
// ║  Para patchimit, ui_channel (L3) dhe active_trust (L2) ishin të           ║
// ║  shkëputur — ky kurriz i bashkon: inputi nga UI merr gjurmim, kuptim     ║
// ║  dhe besim aktiv në një rrjedhë.                                       ║
// ║                                                                          ║
// ║  RRJEDHA E PLOTË E LIGHT-it (si një trup):                           ║
// ║    1. UI Channel (L3)      → input merr gjurmim në hyrje               ║
// ║    2. Neural Law (L3)      → kuptim i lëvizjes                         ║
// ║    3. Active Trust (L2)    → besim aktiv nga historiku                 ║
// ║    4. Verification Trace (L2) → gjurma e verifikimit                   ║
// ║                                                                          ║
// ║  KUFIRI: Light interpreton/transporton/observon — KURRË s'vendos.       ║
// ║  Kurrizi vetëm orkestron rrjedhën ekzistuese. Zero if/else.           ║
// ╚══════════════════════════════════════════════════════════════════════════╝

use crate::lab_contracts::trust::TrustContext;
use crate::layer2::active_trust::{ActiveTrustLayer, ReliabilitySignals};
use crate::layer2::verification_trace::{MiniAlgorithmHardening, VerificationTrace};
use crate::layer3::layer3_flow::LightLayer3Flow;
use crate::layer3::ui_channel::{TracedInput, UiInput};

// ─────────────────────────────────────────────────────────────────────────────
// LIGHT SPINE RESULT — rezultati i rrjedhës vertikale
// ─────────────────────────────────────────────────────────────────────────────

/// Rezultati i plotë i kurrizit të Light-it — gjurmim + kuptim + besim.
#[derive(Debug, Clone)]
pub struct LightSpineResult {
    /// Inputi i gjurmuar me kuptim (L3).
    pub traced_input: TracedInput,
    /// Konteksti i besimit aktiv (L2).
    pub trust_context: TrustContext,
    /// Prioriteti i butë i rrugës [0,1].
    pub soft_priority: f32,
    /// Numri i rezultateve terminale që forcuan këtë interpretim.
    pub verification_evidence_count: u32,
    /// true vetëm kur route/evidence SHA-256 dhe history_refs janë të lidhura.
    pub evidence_hardened: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// LIGHT SPINE — orkestron rrjedhën vertikale L3→L3→L2→L2
// ─────────────────────────────────────────────────────────────────────────────

pub struct LightSpine;

impl LightSpine {
    /// Varianti runtime: lidh VerificationTrace faktike me të njëjtin
    /// TrustContext pa ndryshuar rolin interpretues të Light-it.
    pub fn interpret_full_with_evidence(
        input: UiInput,
        trace_id: u64,
        signals: &ReliabilitySignals,
        territory: String,
        history_refs: Vec<u64>,
        at: u64,
        verification_trace: Option<&VerificationTrace>,
        evidence_route_key: Option<u64>,
    ) -> LightSpineResult {
        // HAP 1+2 (L3): UI input merr gjurmim + kuptim (via layer3_flow).
        // Vlerësimi i besimit fillestar nga sinjalet.
        let l3 = LightLayer3Flow::ingest_from_ui(input, trace_id, at);
        let traced_input = l3.traced_input;

        // HAP 3 (L2): Active Trust ndërton kontekstin e besimit nga historiku.
        let mut trust_context =
            ActiveTrustLayer::build_context(trace_id, territory, signals, history_refs);

        // HAP 4 (L2): forcimi pranohet vetëm kur route/evidence SHA-256 dhe
        // history_refs janë të njëjtat prova që ndërtuan TrustContext-in.
        let (verification_evidence_count, evidence_hardened) = match verification_trace {
            Some(trace) => {
                let route_bound = evidence_route_key == Some(trace.trace_id());
                let history_bound = route_bound
                    && trace.full_identity_valid()
                    && trace.total_count() > 0
                    && trace.total_count() <= crate::ess_mai_system::limits::RING_BUFFER_SIZE
                    && trace.history_refs() == trust_context.history_refs;
                match history_bound {
                    true => {
                        MiniAlgorithmHardening::harden_context(&mut trust_context, trace);
                        (trace.total_count() as u32, true)
                    }
                    false => (0, false),
                }
            }
            None => (0, false),
        };

        // HAP 5 (L2): Prioriteti i butë (jo vendim — vetëm shtytje).
        let soft_priority = ActiveTrustLayer::soft_priority(&trust_context);

        LightSpineResult {
            traced_input,
            trust_context,
            soft_priority,
            verification_evidence_count,
            evidence_hardened,
        }
    }

    /// guidance_label — orientimi i butë për UI (jo vendim). Zero if.
    pub fn guidance_label(result: &LightSpineResult) -> &'static str {
        result.trust_context.guidance.label()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lab_contracts::trust::TrustGuidance;

    fn strong_signals() -> ReliabilitySignals {
        ReliabilitySignals {
            usage_count: 10,
            avg_success: 0.9,
            domain_count: 4,
            verification_count: 3,
        }
    }

    #[test]
    fn full_flow_connects_ui_and_trust() {
        let input = UiInput::new("explore with Nura".to_string(), "s1".to_string(), false);
        let result = LightSpine::interpret_full(
            input,
            42,
            &strong_signals(),
            "physics".to_string(),
            vec![1, 2],
            1000,
        );
        // L3 input u gjurmua.
        assert_eq!(result.traced_input.trace_id, 42);
        // L2 besimi u ndërtua.
        assert!(result.trust_context.trust_score > 0.0);
        assert_eq!(result.trust_context.guidance, TrustGuidance::Recommended);
        // Prioriteti i butë llogaritet.
        assert!(result.soft_priority > 0.0);
    }

    #[test]
    fn light_interprets_not_decides() {
        // Çdo gjë: gjurmim + kuptim + besim — asnjë vendim.
        let input = UiInput::new("test".to_string(), "s".to_string(), false);
        let result =
            LightSpine::interpret_full(input, 1, &strong_signals(), "d".to_string(), vec![1], 100);
        // Rezultati është interpretim, jo verdikt.
        assert!(!result.traced_input.meaning.meaning.is_empty());
        let label = LightSpine::guidance_label(&result);
        assert!(!label.is_empty());
    }

    #[test]
    fn empty_history_unknown_guidance() {
        let input = UiInput::new("q".to_string(), "s".to_string(), false);
        let result = LightSpine::interpret_full(
            input,
            1,
            &ReliabilitySignals::empty(),
            "d".to_string(),
            vec![],
            100,
        );
        // Pa histori → Unknown (besim s'mund të ndërtohet ende).
        assert_eq!(result.trust_context.guidance, TrustGuidance::Unknown);
    }

    #[test]
    fn verified_trace_hardens_only_the_same_runtime_history() {
        let input = UiInput::new("q".to_string(), "s".to_string(), false);
        let route_sha256 = format!("{:016x}{}", 7_u64, format!("{:02x}", 0x22_u8).repeat(24));
        let evidence_sha256 = format!("{:016x}{}", 11_u64, format!("{:02x}", 0x33_u8).repeat(24));
        let mut trace = VerificationTrace::new_bound(7, &route_sha256).expect("bound route");
        trace
            .record_bound_evidence(11, &evidence_sha256, "physics".to_string(), true, 100)
            .expect("terminal evidence");
        let signals = ReliabilitySignals {
            usage_count: 1,
            avg_success: 1.0,
            domain_count: 1,
            verification_count: 1,
        };
        let result = LightSpine::interpret_full_with_evidence(
            input,
            7,
            &signals,
            "physics".to_string(),
            vec![11],
            1000,
            Some(&trace),
            Some(7),
        );
        assert!(result.evidence_hardened);
        assert_eq!(result.verification_evidence_count, 1);
        assert_eq!(result.trust_context.verification_score, 1.0);
        assert!(result.soft_priority > 0.0);

        let mut unbound_trace = VerificationTrace::new(7);
        unbound_trace
            .record_evidence(11, "physics".to_string(), true, 100)
            .expect("compatibility evidence");
        let rejected = LightSpine::interpret_full_with_evidence(
            UiInput::new("q".to_string(), "s".to_string(), false),
            7,
            &signals,
            "physics".to_string(),
            vec![11],
            1001,
            Some(&unbound_trace),
            Some(7),
        );
        assert!(!rejected.evidence_hardened);
        assert_eq!(rejected.verification_evidence_count, 0);
    }
}

#[cfg(test)]
impl LightSpine {
    pub fn interpret_full(
        input: UiInput,
        trace_id: u64,
        signals: &ReliabilitySignals,
        territory: String,
        history_refs: Vec<u64>,
        at: u64,
    ) -> LightSpineResult {
        Self::interpret_full_with_evidence(
            input,
            trace_id,
            signals,
            territory,
            history_refs,
            at,
            None,
            None,
        )
    }
}
