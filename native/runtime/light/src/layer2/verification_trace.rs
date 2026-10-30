// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  LAYER2/VERIFICATION_TRACE.RS — Verification Trace Layer (Light)     ║
// ║  GJATA LEGACY™ — AIGovern-Cyber Light                            (LAYER 2)    ║
// ║                                                                          ║
// ║  Light ruan dhe ekspozon GJURMËN E VERIFIKIMIT: si u ndërtua besimi,    ║
// ║  sa herë mbijetoi, në cilat domene, çfarë evidence ekzistonte.         ║
// ║                                                                          ║
// ║  + Mini Algorithm Hardening: forcon vlerësimin me algoritmin e vogël    ║
// ║  ekzistues (LightMiniEvolver) — i përshtatshëm për trust/trace/soft     ║
// ║  filtering, jo për motorin e madh agresiv (ai i takon Quantum-it).     ║
// ║                                                                          ║
// ║  Prodhon gjurmë të lexueshme nga të tria platformat. Zero if/else.    ║
// ╚══════════════════════════════════════════════════════════════════════════╝

use crate::lab_contracts::trust::TrustContext;

// ─────────────────────────────────────────────────────────────────────────────
// VERIFICATION TRACE — gjurma e historisë së verifikimit
// ─────────────────────────────────────────────────────────────────────────────

/// Një hap në gjurmën e verifikimit të një rruge.
#[derive(Debug, Clone)]
pub struct TraceStep {
    /// Referenca e pandryshueshme e rezultatit terminal.
    evidence_ref: u64,
    /// Identiteti i plotë i evidencës terminale. `None` ruhet vetëm për
    /// gjurmët historike compatibility; runtime-i i Trust-it e refuzon.
    evidence_sha256: Option<String>,
    /// Domeni ku u provua.
    domain: String,
    /// A funksionoi në këtë hap.
    succeeded: bool,
    /// Vula kohore.
    at: u64,
}

/// Gjurma e plotë e verifikimit — historiku i mbijetesës së një rruge.
#[derive(Debug, Clone)]
pub struct VerificationTrace {
    /// Identifikuesi i gjurmës.
    trace_id: u64,
    /// Identiteti autoritativ SHA-256 i route-it. `trace_id` është vetëm
    /// handle compatibility dhe nuk mjafton për forcimin runtime të Trust-it.
    route_sha256: Option<String>,
    /// Hapat e historisë.
    steps: Vec<TraceStep>,
}

impl TraceStep {
    pub fn evidence_ref(&self) -> u64 {
        self.evidence_ref
    }

    pub fn evidence_sha256(&self) -> Option<&str> {
        self.evidence_sha256.as_deref()
    }

    pub fn domain(&self) -> &str {
        &self.domain
    }

    pub fn succeeded(&self) -> bool {
        self.succeeded
    }

    pub fn at(&self) -> u64 {
        self.at
    }
}

impl VerificationTrace {
    /// new_bound — hap gjurmën autoritative vetëm kur SHA-256 i plotë
    /// prodhon saktësisht handle-in ekzistues.
    pub fn new_bound(trace_id: u64, route_sha256: &str) -> Result<Self, &'static str> {
        match trace_id {
            0 => return Err("VERIFICATION_TRACE_ID_ZERO"),
            _ => {}
        }
        let route_handle = Self::sha256_handle(
            route_sha256,
            "VERIFICATION_ROUTE_SHA256_INVALID",
            "VERIFICATION_ROUTE_SHA256_DECODE",
        )?;
        match route_handle == trace_id {
            true => {}
            false => return Err("VERIFICATION_ROUTE_SHA256_HANDLE_MISMATCH"),
        }
        Ok(Self {
            trace_id,
            route_sha256: Some(route_sha256.to_string()),
            steps: Vec::new(),
        })
    }

    /// record_bound_evidence — rruga autoritative e runtime-it.
    ///
    /// Route-i dhe evidenca mbeten SHA-256 të plota në çdo hap; `u64`
    /// pranohet vetëm kur është prefiksi i verifikuar i digest-it përkatës.
    pub fn record_bound_evidence(
        &mut self,
        evidence_ref: u64,
        evidence_sha256: &str,
        domain: String,
        succeeded: bool,
        at: u64,
    ) -> Result<(), &'static str> {
        self.record_evidence_checked(evidence_ref, Some(evidence_sha256), domain, succeeded, at)
    }

    fn record_evidence_checked(
        &mut self,
        evidence_ref: u64,
        evidence_sha256: Option<&str>,
        domain: String,
        succeeded: bool,
        at: u64,
    ) -> Result<(), &'static str> {
        use crate::ess_mai_system::limits::RING_BUFFER_SIZE;

        match (self.trace_id, evidence_ref, at) {
            (0, _, _) => return Err("VERIFICATION_TRACE_ID_ZERO"),
            (_, 0, _) => return Err("VERIFICATION_EVIDENCE_REF_ZERO"),
            (_, _, 0) => return Err("VERIFICATION_EVIDENCE_TIME_ZERO"),
            _ => {}
        }
        match (self.route_sha256.is_some(), evidence_sha256.is_some()) {
            (true, false) => return Err("VERIFICATION_EVIDENCE_SHA256_REQUIRED"),
            (false, true) => return Err("VERIFICATION_ROUTE_SHA256_MISSING"),
            (true, true) => self.validate_route_identity()?,
            (false, false) => {}
        }
        let evidence_sha256 = match evidence_sha256 {
            Some(evidence_sha256) => {
                let evidence_handle = Self::sha256_handle(
                    evidence_sha256,
                    "VERIFICATION_EVIDENCE_SHA256_INVALID",
                    "VERIFICATION_EVIDENCE_SHA256_DECODE",
                )?;
                match evidence_handle == evidence_ref {
                    true => {}
                    false => return Err("VERIFICATION_EVIDENCE_SHA256_HANDLE_MISMATCH"),
                }
                Some(evidence_sha256.to_string())
            }
            None => None,
        };
        match domain.trim().is_empty() {
            true => return Err("VERIFICATION_EVIDENCE_DOMAIN_EMPTY"),
            false => {}
        }
        let same_ref = self
            .steps
            .iter()
            .find(|step| step.evidence_ref == evidence_ref);
        match same_ref {
            Some(step) => match (
                step.evidence_sha256.as_deref() == evidence_sha256.as_deref(),
                step.evidence_sha256.is_some(),
                evidence_sha256.is_some(),
            ) {
                (false, true, true) => return Err("VERIFICATION_EVIDENCE_SHA256_COLLISION"),
                _ => return Err("VERIFICATION_EVIDENCE_REPLAY"),
            },
            None => {}
        }
        match self.steps.len() < RING_BUFFER_SIZE {
            true => {}
            false => return Err("VERIFICATION_EVIDENCE_CAPACITY"),
        }
        self.steps.push(TraceStep {
            evidence_ref,
            evidence_sha256,
            domain: domain.trim().to_string(),
            succeeded,
            at,
        });
        Ok(())
    }

    fn sha256_handle(
        value: &str,
        invalid: &'static str,
        decode: &'static str,
    ) -> Result<u64, &'static str> {
        match crate::lgc_algorithm::is_canonical_sha256(value) {
            true => {}
            false => return Err(invalid),
        }
        u64::from_str_radix(&value[..16], 16).map_err(|_| decode)
    }

    fn validate_route_identity(&self) -> Result<(), &'static str> {
        let route_sha256 = self
            .route_sha256
            .as_deref()
            .ok_or("VERIFICATION_ROUTE_SHA256_MISSING")?;
        let route_handle = Self::sha256_handle(
            route_sha256,
            "VERIFICATION_ROUTE_SHA256_INVALID",
            "VERIFICATION_ROUTE_SHA256_DECODE",
        )?;
        match route_handle == self.trace_id {
            true => Ok(()),
            false => Err("VERIFICATION_ROUTE_SHA256_HANDLE_MISMATCH"),
        }
    }

    /// SHA-256 autoritativ i route-it, kur gjurma është ndërtuar nga runtime-i.
    pub fn route_sha256(&self) -> Option<&str> {
        self.route_sha256.as_deref()
    }

    pub fn trace_id(&self) -> u64 {
        self.trace_id
    }

    pub fn steps(&self) -> &[TraceStep] {
        &self.steps
    }

    /// Provon që çdo handle i gjurmës ka identitetin e plotë përkatës.
    pub fn full_identity_valid(&self) -> bool {
        let route_valid = self.validate_route_identity().is_ok();
        let evidence_valid = self.steps.iter().all(|step| {
            step.evidence_sha256.as_deref().and_then(|sha256| {
                Self::sha256_handle(
                    sha256,
                    "VERIFICATION_EVIDENCE_SHA256_INVALID",
                    "VERIFICATION_EVIDENCE_SHA256_DECODE",
                )
                .ok()
            }) == Some(step.evidence_ref)
        });
        route_valid && evidence_valid
    }

    /// success_count — sa hapa funksionuan. Zero if — filter.
    pub fn success_count(&self) -> usize {
        self.steps.iter().filter(|s| s.succeeded).count()
    }

    /// total_count — sa hapa gjithsej. Zero if.
    pub fn total_count(&self) -> usize {
        self.steps.len()
    }

    /// survival_rate — raporti i mbijetesës [0,1]. Zero if — formula me mbrojtje.
    pub fn survival_rate(&self) -> f32 {
        let total = self.steps.len().max(1) as f32;
        let success = self.success_count() as f32;
        (success / total).clamp(0.0, 1.0)
    }

    /// Domenet ku rezultati terminal mbijetoi; tentativat negative nuk
    /// pretendojnë stabilitet.
    pub fn distinct_success_domains(&self) -> usize {
        let mut domains: Vec<&String> = self
            .steps
            .iter()
            .filter(|step| step.succeeded)
            .map(|step| &step.domain)
            .collect();
        domains.sort();
        domains.dedup();
        domains.len()
    }

    /// Referencat e evidencës në të njëjtin rend si hapat terminalë.
    pub fn history_refs(&self) -> Vec<u64> {
        self.steps.iter().map(|step| step.evidence_ref).collect()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MINI ALGORITHM HARDENING — forcon vlerësimin me algoritmin e vogël
// ─────────────────────────────────────────────────────────────────────────────

pub struct MiniAlgorithmHardening;

impl MiniAlgorithmHardening {
    /// stability_score — sa i qëndrueshëm është një primitiv [0,1]. Zero if.
    ///
    /// Stabilitet = mbijetesë e lartë AND në shumë domene.
    pub fn stability_score(trace: &VerificationTrace) -> f32 {
        let survival = trace.survival_rate();
        let breadth = (trace.distinct_success_domains() as f32 / 3.0).clamp(0.0, 1.0);
        (survival * breadth).clamp(0.0, 1.0)
    }

    /// harden_context — lidh gjurmën reale me dy shtresat faktike të besimit.
    ///
    /// Nuk prek trust_score ose guidance: ato burojnë nga sinjalet e
    /// suksesit. Verifikimi është survival rate-i i matur; stabiliteti kërkon
    /// njëkohësisht mbijetesë dhe gjerësi domenesh, ndaj një histori vetëm me
    /// dështime nuk fiton masë pozitive.
    pub fn harden_context(ctx: &mut TrustContext, trace: &VerificationTrace) {
        let verification = trace.survival_rate();
        let stability = Self::stability_score(trace);
        ctx.verification_score = ctx.verification_score.max(verification);
        ctx.reliability_score = ctx.reliability_score.max(stability);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lab_contracts::trust::{TrustContext, TrustGuidance};

    fn full_identity(handle: u64, tail_byte: u8) -> String {
        format!("{handle:016x}{}", format!("{tail_byte:02x}").repeat(24))
    }

    fn sample_trace() -> VerificationTrace {
        let mut t = VerificationTrace::new(1);
        t.record_evidence(1, "physics".to_string(), true, 100)
            .unwrap();
        t.record_evidence(2, "chemistry".to_string(), true, 200)
            .unwrap();
        t.record_evidence(3, "physics".to_string(), false, 300)
            .unwrap();
        t.record_evidence(4, "biology".to_string(), true, 400)
            .unwrap();
        t
    }

    #[test]
    fn trace_counts() {
        let t = sample_trace();
        assert_eq!(t.total_count(), 4);
        assert_eq!(t.success_count(), 3);
        assert_eq!(t.distinct_domains(), 3); // physics, chemistry, biology
        assert_eq!(t.distinct_success_domains(), 3);
    }

    #[test]
    fn survival_rate_correct() {
        let t = sample_trace();
        // 3 sukses / 4 total = 0.75
        assert!((t.survival_rate() - 0.75).abs() < 1e-6);
    }

    #[test]
    fn empty_trace_safe() {
        let t = VerificationTrace::new(1);
        // Pa hapa → survival rate 0 (jo panik nga pjesëtimi).
        assert_eq!(t.survival_rate(), 0.0);
        assert_eq!(t.distinct_domains(), 0);
        assert_eq!(t.distinct_success_domains(), 0);
    }

    #[test]
    fn stability_reflects_breadth() {
        let t = sample_trace();
        let stability = MiniAlgorithmHardening::stability_score(&t);
        assert!(stability > 0.0 && stability <= 1.0);
    }

    #[test]
    fn checked_evidence_refuses_replay_and_exports_history_refs() {
        let mut trace = VerificationTrace::new(7);
        trace
            .record_evidence(11, "physics".to_string(), true, 100)
            .expect("first evidence");
        assert_eq!(
            trace.record_evidence(11, "physics".to_string(), true, 101),
            Err("VERIFICATION_EVIDENCE_REPLAY")
        );
        assert!(trace.history_refs().contains(&11));
        assert_eq!(trace.history_refs(), vec![11]);
    }

    #[test]
    fn failed_history_does_not_gain_positive_hardening() {
        let mut trace = VerificationTrace::new(7);
        trace
            .record_evidence(11, "physics".to_string(), false, 100)
            .expect("negative terminal evidence");
        let mut ctx = TrustContext::empty(7, "physics".to_string());
        MiniAlgorithmHardening::harden_context(&mut ctx, &trace);
        assert_eq!(ctx.verification_score, 0.0);
        assert_eq!(ctx.reliability_score, 0.0);
        assert_eq!(ctx.guidance, TrustGuidance::Unknown);
    }

    #[test]
    fn bound_trace_retains_full_route_and_evidence_identity() {
        let route = full_identity(7, 0x22);
        let evidence = full_identity(11, 0x33);
        let mut trace = VerificationTrace::new_bound(7, &route).expect("bound route identity");
        trace
            .record_bound_evidence(11, &evidence, "physics".to_string(), true, 100)
            .expect("bound terminal evidence");
        assert_eq!(trace.route_sha256(), Some(route.as_str()));
        assert_eq!(trace.evidence_sha256(11), Some(evidence.as_str()));
        assert!(trace.full_identity_valid());
    }

    #[test]
    fn bound_trace_refuses_partial_evidence_and_handle_drift() {
        let route = full_identity(7, 0x22);
        let route_drift = full_identity(8, 0x44);
        let evidence = full_identity(11, 0x33);
        let evidence_collision = full_identity(11, 0x55);
        let mut unbound = VerificationTrace::new(7);
        assert_eq!(
            unbound.record_bound_evidence(11, &evidence, "physics".to_string(), true, 100,),
            Err("VERIFICATION_ROUTE_SHA256_MISSING")
        );
        let mut trace = VerificationTrace::new_bound(7, &route).expect("bound route identity");
        trace
            .record_bound_evidence(11, &evidence, "physics".to_string(), true, 100)
            .expect("bound terminal evidence");
        assert_eq!(
            trace.record_evidence(12, "physics".to_string(), true, 101),
            Err("VERIFICATION_EVIDENCE_SHA256_REQUIRED")
        );
        assert_eq!(
            trace.record_bound_evidence(11, &evidence_collision, "physics".to_string(), true, 101,),
            Err("VERIFICATION_EVIDENCE_SHA256_COLLISION")
        );
        trace.route_sha256 = Some(route_drift);
        assert!(!trace.full_identity_valid());
        assert_eq!(
            VerificationTrace::new_bound(8, &route).expect_err("route handle drift must fail"),
            "VERIFICATION_ROUTE_SHA256_HANDLE_MISMATCH"
        );
    }
}

#[cfg(test)]
impl VerificationTrace {
    pub fn new(trace_id: u64) -> Self {
        Self {
            trace_id,
            route_sha256: None,
            steps: Vec::new(),
        }
    }
    pub fn distinct_domains(&self) -> usize {
        let mut domains: Vec<&String> = self.steps.iter().map(|s| &s.domain).collect();
        domains.sort();
        domains.dedup();
        domains.len()
    }
    pub fn evidence_sha256(&self, evidence_ref: u64) -> Option<&str> {
        self.steps
            .iter()
            .find(|step| step.evidence_ref == evidence_ref)?
            .evidence_sha256
            .as_deref()
    }
    pub fn record_evidence(
        &mut self,
        evidence_ref: u64,
        domain: String,
        succeeded: bool,
        at: u64,
    ) -> Result<(), &'static str> {
        self.record_evidence_checked(evidence_ref, None, domain, succeeded, at)
    }
}
