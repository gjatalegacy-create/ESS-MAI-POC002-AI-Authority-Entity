// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  LAYER2/ACTIVE_TRUST.RS — Active Trust Layer (Light)                ║
// ║  GJATA LEGACY™ — AIGovern-Cyber Light                            (LAYER 2)    ║
// ║                                                                          ║
// ║  Light mbetet veshi, goja dhe ndërtuesi i besimit — por Layer 2 e bën   ║
// ║  AKTIV: nuk pret Shadow-in për të ndërtuar besim. Parashikon, peshon,    ║
// ║  pasuron output-in me histori, orienton drejt rrugëve më të besueshme.  ║
// ║                                                                          ║
// ║  PRESIONI I LIGHT-IT është i BUTË (jo sulm si NPRO):                   ║
// ║    • nuk shtrydh idenë                                                 ║
// ║    • e ngarkon me histori, e bën të gjurmueshme                        ║
// ║    • e forcon me mini algoritmin (LightMiniEvolver ekzistues)          ║
// ║    • e orienton (Recommended/Neutral/Risky/Unknown)                    ║
// ║                                                                          ║
// ║  Prodhon TrustContext (kontrata e përbashkët). KUFIRI: Light NUK        ║
// ║  vendos, NUK bën presion të hapur, NUK gjykon. Zero if/else.          ║
// ╚══════════════════════════════════════════════════════════════════════════╝

use crate::lab_contracts::trust::{TrustContext, TrustGuidance};
use crate::layer2::verification_trace::VerificationTrace;
use crate::mini_evolver::LightMiniEvolver;

// ─────────────────────────────────────────────────────────────────────────────
// KONSTANTET — pragjet e orientimit të butë
// ─────────────────────────────────────────────────────────────────────────────

/// Mbi këtë trust score → Recommended.
pub const TRUST_RECOMMENDED: f32 = 0.70;
/// Nën këtë trust score (me histori) → Risky.
pub const TRUST_RISKY: f32 = 0.35;

// ─────────────────────────────────────────────────────────────────────────────
// RELIABILITY METRICS — sinjalet e besueshmërisë nga historiku
// ─────────────────────────────────────────────────────────────────────────────

/// Sinjalet që Light mbledh nga historiku i një primitivi/rruge.
#[derive(Debug, Clone, Copy)]
pub struct ReliabilitySignals {
    /// Sa herë është përdorur (nga LightMiniEvolver.get_usage_count).
    pub usage_count: u32,
    /// Skori mesatar i suksesit [0,1] (nga get_average_vector_score).
    pub avg_success: f32,
    /// Në sa domene ka qenë i qëndrueshëm.
    pub domain_count: u32,
    /// Sa herë është verifikuar nga Shadow (histori verifikimi).
    pub verification_count: u32,
}

impl ReliabilitySignals {
    /// empty — pa histori (primitiv i ri). Zero if.
    pub fn empty() -> Self {
        Self {
            usage_count: 0,
            avg_success: 0.0,
            domain_count: 0,
            verification_count: 0,
        }
    }

    /// has_history — a ka ndonjë histori përdorimi? Zero if.
    pub fn has_history(&self) -> bool {
        self.usage_count > 0
    }

    /// Ndërton sinjale vetëm kur Mini Evolver dhe VerificationTrace provojnë
    /// të njëjtat ngjarje terminale, SHA-256 të plota, rend dhe route.
    /// Çdo drift refuzohet; besimi nuk pasurohet me histori të pjesshme.
    pub fn from_runtime(
        evolver: &LightMiniEvolver,
        knowledge_id: u64,
        trace: Option<&VerificationTrace>,
    ) -> Result<Self, &'static str> {
        match knowledge_id {
            0 => return Err("RELIABILITY_ROUTE_ZERO"),
            _ => {}
        }

        let events = evolver.events_for(knowledge_id);
        let (events, trace) = match (events, trace) {
            (None, None) => return Ok(Self::empty()),
            (Some(events), Some(trace)) => (events, trace),
            (None, Some(_)) => return Err("RELIABILITY_MINI_HISTORY_MISSING"),
            (Some(_), None) => return Err("RELIABILITY_TRACE_MISSING"),
        };

        match trace.trace_id() == knowledge_id {
            true => {}
            false => return Err("RELIABILITY_TRACE_ROUTE_MISMATCH"),
        }
        let route_sha256 = evolver
            .route_sha256(knowledge_id)
            .ok_or("RELIABILITY_ROUTE_SHA256_MISSING")?;
        let route_handle = match crate::lgc_algorithm::is_canonical_sha256(route_sha256) {
            true => u64::from_str_radix(&route_sha256[..16], 16)
                .map_err(|_| "RELIABILITY_ROUTE_SHA256_DECODE")?,
            false => return Err("RELIABILITY_ROUTE_SHA256_INVALID"),
        };
        match route_handle == knowledge_id {
            true => {}
            false => return Err("RELIABILITY_ROUTE_SHA256_HANDLE_MISMATCH"),
        }
        let trace_route_sha256 = trace
            .route_sha256()
            .ok_or("RELIABILITY_TRACE_ROUTE_SHA256_MISSING")?;
        match trace_route_sha256 == route_sha256 {
            true => {}
            false => return Err("RELIABILITY_TRACE_ROUTE_SHA256_MISMATCH"),
        }
        match events.len() == trace.total_count() {
            true => {}
            false => return Err("RELIABILITY_EVENT_COUNT_MISMATCH"),
        }
        match events.is_empty() {
            true => return Err("RELIABILITY_EMPTY_HISTORY_SHAPE"),
            false => {}
        }

        let aligned = events
            .iter()
            .zip(trace.steps().iter())
            .all(|(event, step)| {
                let evidence_identity_bound = event
                    .evidence_sha256
                    .as_deref()
                    .map(|sha256| {
                        crate::lgc_algorithm::is_canonical_sha256(sha256)
                            && u64::from_str_radix(&sha256[..16], 16).ok()
                                == Some(event.evidence_ref)
                            && evolver.evidence_sha256(knowledge_id, event.evidence_ref)
                                == Some(sha256)
                            && step.evidence_sha256() == Some(sha256)
                    })
                    .unwrap_or(false);
                event.knowledge_id == knowledge_id
                    && event.evidence_ref == step.evidence_ref()
                    && evidence_identity_bound
                    && event.module_name == step.domain()
                    && event.timestamp_ns == step.at()
                    && event.survived == step.succeeded()
                    && event.vector_score == u8::from(step.succeeded()) as f32
                    && event.linear_mass.is_finite()
                    && (0.0..=1.0).contains(&event.linear_mass)
            });
        match aligned {
            true => {}
            false => return Err("RELIABILITY_EVIDENCE_LINEAGE_MISMATCH"),
        }

        let avg_success = trace.survival_rate();
        let mini_average = evolver
            .get_average_vector_score(knowledge_id)
            .ok_or("RELIABILITY_MINI_AVERAGE_MISSING")?;
        match (mini_average - avg_success).abs() <= f32::EPSILON {
            true => {}
            false => return Err("RELIABILITY_SURVIVAL_MISMATCH"),
        }

        let usage_count = u32::try_from(events.len()).map_err(|_| "RELIABILITY_USAGE_OVERFLOW")?;
        let domain_count = u32::try_from(trace.distinct_success_domains())
            .map_err(|_| "RELIABILITY_DOMAIN_OVERFLOW")?;
        let verification_count =
            u32::try_from(trace.success_count()).map_err(|_| "RELIABILITY_VERIFY_OVERFLOW")?;

        Ok(Self {
            usage_count,
            avg_success,
            domain_count,
            verification_count,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ACTIVE TRUST LAYER — prodhon TrustContext
// ─────────────────────────────────────────────────────────────────────────────

pub struct ActiveTrustLayer;

impl ActiveTrustLayer {
    /// build_context — ndërton TrustContext nga sinjalet e historikut.
    ///
    /// DY shtresa paralele (sipas orientimit):
    ///   • trust_score        → sa shpesh funksionon (nga avg_success)
    ///   • verification_score → sa herë u verifikua/mbijetoi (nga verif_count)
    /// Zero if — formula + match.
    pub fn build_context(
        trace_id: u64,
        domain: String,
        signals: &ReliabilitySignals,
        history_refs: Vec<u64>,
    ) -> TrustContext {
        // trust_score: sukses i peshuar nga vëllimi i përdorimit.
        // Pak përdorime me sukses të fortë ende vlejnë (proactive trust building).
        let usage_factor = (signals.usage_count as f32 / 5.0).clamp(0.0, 1.0);
        let trust_score =
            (signals.avg_success * 0.7 + signals.avg_success * usage_factor * 0.3).clamp(0.0, 1.0);

        // reliability_score: stabiliteti ndër-domenesh.
        let reliability_score = (signals.domain_count as f32 / 5.0).clamp(0.0, 1.0);

        // verification_score: sa herë mbijetoi verifikimin (rigorozitet).
        let verification_score = (signals.verification_count as f32 / 3.0).clamp(0.0, 1.0);

        // Orientimi i butë (jo vendim).
        let guidance = Self::orient(trust_score, signals.has_history());

        TrustContext {
            trace_id,
            domain,
            trust_score,
            reliability_score,
            verification_score,
            history_refs,
            guidance,
        }
    }

    /// orient — orientimi i butë nga trust score + prania e historisë.
    /// Zero if — match tuple.
    ///
    /// Pa histori → Unknown. Me histori: i lartë → Recommended,
    /// i ulët → Risky, mes → Neutral.
    fn orient(trust_score: f32, has_history: bool) -> TrustGuidance {
        let high = trust_score >= TRUST_RECOMMENDED;
        let low = trust_score < TRUST_RISKY;

        match (has_history, high, low) {
            (false, _, _) => TrustGuidance::Unknown,
            (true, true, _) => TrustGuidance::Recommended,
            (true, false, true) => TrustGuidance::Risky,
            (true, false, false) => TrustGuidance::Neutral,
        }
    }

    /// soft_priority — prioriteti i butë [0,1] (jo vendim — vetëm shtytje).
    /// Rrugët me besueshmëri më të lartë marrin prioritet më të lartë. Zero if.
    pub fn soft_priority(ctx: &TrustContext) -> f32 {
        ctx.combined_strength()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer2::verification_trace::VerificationTrace;
    use crate::mini_evolver::LightMiniEvolver;

    fn strong_signals() -> ReliabilitySignals {
        ReliabilitySignals {
            usage_count: 10,
            avg_success: 0.9,
            domain_count: 4,
            verification_count: 3,
        }
    }

    fn full_identity(handle: u64, tail_byte: u8) -> String {
        format!("{handle:016x}{}", format!("{tail_byte:02x}").repeat(24))
    }

    #[test]
    fn empty_signals_unknown_guidance() {
        let ctx = ActiveTrustLayer::build_context(
            1,
            "physics".to_string(),
            &ReliabilitySignals::empty(),
            vec![],
        );
        assert_eq!(ctx.guidance, TrustGuidance::Unknown);
        assert_eq!(ctx.trust_score, 0.0);
    }

    #[test]
    fn strong_signals_recommended() {
        let ctx = ActiveTrustLayer::build_context(
            1,
            "physics".to_string(),
            &strong_signals(),
            vec![1, 2],
        );
        assert_eq!(ctx.guidance, TrustGuidance::Recommended);
        assert!(ctx.trust_score >= TRUST_RECOMMENDED);
        assert!(ctx.has_history());
    }

    #[test]
    fn risky_when_low_success_with_history() {
        let signals = ReliabilitySignals {
            usage_count: 5,
            avg_success: 0.2,
            domain_count: 1,
            verification_count: 0,
        };
        let ctx = ActiveTrustLayer::build_context(1, "d".to_string(), &signals, vec![1]);
        assert_eq!(ctx.guidance, TrustGuidance::Risky);
    }

    #[test]
    fn neutral_when_mid_success() {
        let signals = ReliabilitySignals {
            usage_count: 5,
            avg_success: 0.5,
            domain_count: 2,
            verification_count: 1,
        };
        let ctx = ActiveTrustLayer::build_context(1, "d".to_string(), &signals, vec![1]);
        assert_eq!(ctx.guidance, TrustGuidance::Neutral);
    }

    #[test]
    fn two_layers_distinct() {
        // trust_score (sukses) dhe verification_score (rigorozitet) janë të ndara.
        let ctx = ActiveTrustLayer::build_context(1, "d".to_string(), &strong_signals(), vec![1]);
        // Të dyja jo-zero, por të llogaritura veçmas.
        assert!(ctx.trust_score > 0.0);
        assert!(ctx.verification_score > 0.0);
    }

    #[test]
    fn soft_priority_reflects_strength() {
        let strong =
            ActiveTrustLayer::build_context(1, "d".to_string(), &strong_signals(), vec![1]);
        let weak = ActiveTrustLayer::build_context(
            1,
            "d".to_string(),
            &ReliabilitySignals::empty(),
            vec![],
        );
        assert!(ActiveTrustLayer::soft_priority(&strong) > ActiveTrustLayer::soft_priority(&weak));
    }

    #[test]
    fn runtime_signals_require_byte_aligned_terminal_history() {
        let mut mini = LightMiniEvolver::new();
        let route_sha256 = full_identity(7, 0x22);
        let evidence_sha256 = full_identity(11, 0x33);
        mini.record_verified_cycle_bound_at(
            7,
            &route_sha256,
            11,
            &evidence_sha256,
            "physics",
            0.8,
            true,
            100,
        )
        .expect("mini evidence");
        let mut trace = VerificationTrace::new_bound(7, &route_sha256).expect("bound route");
        trace
            .record_bound_evidence(11, &evidence_sha256, "physics".to_string(), true, 100)
            .expect("verification evidence");
        let signals =
            ReliabilitySignals::from_runtime(&mini, 7, Some(&trace)).expect("aligned evidence");
        assert_eq!(signals.usage_count, 1);
        assert_eq!(signals.avg_success, 1.0);
        assert_eq!(signals.domain_count, 1);
        assert_eq!(signals.verification_count, 1);

        let route_drift_sha256 = full_identity(7, 0x44);
        let mut route_drift =
            VerificationTrace::new_bound(7, &route_drift_sha256).expect("drifted route");
        route_drift
            .record_bound_evidence(11, &evidence_sha256, "physics".to_string(), true, 100)
            .expect("drifted route evidence");
        assert_eq!(
            ReliabilitySignals::from_runtime(&mini, 7, Some(&route_drift))
                .expect_err("same handle with route drift must fail"),
            "RELIABILITY_TRACE_ROUTE_SHA256_MISMATCH"
        );

        let evidence_drift_sha256 = full_identity(11, 0x55);
        let mut evidence_drift =
            VerificationTrace::new_bound(7, &route_sha256).expect("bound route");
        evidence_drift
            .record_bound_evidence(11, &evidence_drift_sha256, "physics".to_string(), true, 100)
            .expect("drifted evidence");
        assert_eq!(
            ReliabilitySignals::from_runtime(&mini, 7, Some(&evidence_drift))
                .expect_err("same handle with evidence drift must fail"),
            "RELIABILITY_EVIDENCE_LINEAGE_MISMATCH"
        );

        let drifted_ref_sha256 = full_identity(12, 0x66);
        let mut drifted_ref = VerificationTrace::new_bound(7, &route_sha256).expect("bound route");
        drifted_ref
            .record_bound_evidence(12, &drifted_ref_sha256, "physics".to_string(), true, 100)
            .expect("drifted evidence ref");
        assert_eq!(
            ReliabilitySignals::from_runtime(&mini, 7, Some(&drifted_ref))
                .expect_err("drifted evidence must fail"),
            "RELIABILITY_EVIDENCE_LINEAGE_MISMATCH"
        );
    }

    #[test]
    fn runtime_signals_are_empty_only_without_both_histories() {
        let mini = LightMiniEvolver::new();
        let signals =
            ReliabilitySignals::from_runtime(&mini, 7, None).expect("new route is unknown");
        assert_eq!(signals.usage_count, 0);
        let trace = VerificationTrace::new(7);
        assert_eq!(
            ReliabilitySignals::from_runtime(&mini, 7, Some(&trace))
                .expect_err("one-sided history must fail"),
            "RELIABILITY_MINI_HISTORY_MISSING"
        );
    }
}
