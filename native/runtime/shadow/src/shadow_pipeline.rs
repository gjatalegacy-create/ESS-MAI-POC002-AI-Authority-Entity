// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  SHADOW_PIPELINE.RS — ORKESTRATORI I VETËM (bashkon dy origjinat)     ║
// ║  Gjata Legacy™  |  Arkitekt: Bledar Gjata                             ║
// ║                                                                          ║
// ║  Pikërisht KËTU bashkohen PassPackage (Quantum) + LightEnvelope        ║
// ║  (Light) në një ShadowPassage të brendshëm — me ZHVENDOSJE (move),    ║
// ║  jo klonim (LIGJI 0). NJË pipeline; nodat ekzekutohen në sekuencën    ║
// ║  EKZAKTE të rrugës; ligjet sovrane zbatohen; pastaj autoriteti suprem.║
// ║                                                                          ║
// ║  Renditja e nodave (== renditja e enum ShadowNode):                   ║
// ║    Fast:     Router → Matrix → Judiciary                              ║
// ║    Standard: Router → Matrix → Gen5 → Type → Sovereign → Judiciary    ║
// ║    Deep:     Router → Matrix → Gen5 → Type → Temporal → Sovereign     ║
// ║              → Emergence → Consensus → Judiciary                       ║
// ╚══════════════════════════════════════════════════════════════════════════╝

use crate::knowledge_vault::KnowledgeVault;
use crate::shadow_commit::SupremeCandidate;
use crate::shadow_consensus::ShadowConsensus;
use crate::shadow_emergence::ShadowEmergence;
use crate::shadow_gen5::ShadowGen5;
use crate::shadow_gj_legacy::ShadowGjLegacy;
use crate::shadow_judiciary::ShadowJudiciary;
use crate::shadow_matrix::ShadowMatrix;
use crate::shadow_router::ShadowRouter;
use crate::shadow_sovereign::ShadowSovereign;
use crate::shadow_temporal::ShadowTemporal;
use crate::shadow_type::ShadowType;
use crate::sovereign_guard::enforce_sovereign_laws;
use crate::types::{
    LightEnvelope, NegativeContext, PassPackage, ShadowError, ShadowPassage, ShadowPath,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// ─────────────────────────────────────────────────────────────────────────────
// TIME_DEGRADED — VULA E AUDITIMIT (HARDENING)
// Ora reale (e murit) dështoi ≥1 herë gjatë këtij procesi; lineage vazhdoi me
// fallback monotonik. Vulë NGJITËSE (s'ka unmark): fallback-u kohor LEJOHET
// vetëm i shënuar — auditimi DUHET ta dijë që koha reale u degradua, edhe pse
// verdiktet mbetën të renditura.
// ─────────────────────────────────────────────────────────────────────────────
pub static TIME_DEGRADED: AtomicBool = AtomicBool::new(false);

/// Shënon degradimin e orës — thirret nga ÇDO fallback kohor i krate-s
/// (pipeline, apupk, snb). Ngjitëse: një herë true, gjithmonë true.
pub fn mark_time_degraded() {
    TIME_DEGRADED.store(true, Ordering::SeqCst);
}

/// A ka ndodhur qoftë edhe një fallback kohor gjatë këtij procesi?
pub fn time_degraded() -> bool {
    TIME_DEGRADED.load(Ordering::SeqCst)
}

/// Timestamp në nanosekonda (epoch). Burim i vetëm i kohës për pipeline-in.
///
/// FAIL-CLOSED (audit, ULTIMATUM): ora e murit para 1970 (jashtëzakonisht e
/// rrallë) NUK e humb lineage-in me 0 — kalon te fallback MONOTONIK (nanos nga
/// një pikë fikse e procesit), me bazë jo-zero që ruan renditjen dhe alarm të
/// zhurmshëm në audit. Kurrë 0 i heshtur. Zero unwrap — match.
pub fn now_ns() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(d)  => d.as_nanos() as u64,
        Err(_) => {
            eprintln!("[SHADOW_TIME] ALARM: ora e sistemit para UNIX_EPOCH — fallback monotonik (lineage i ruajtur)");
            mark_time_degraded(); // VULA E AUDITIMIT: koha reale u degradua
            static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
            let start = START.get_or_init(std::time::Instant::now);
            // Bazë jo-zero (1s) + elapsed monotonik → kurrë 0, renditje e ruajtur.
            1_000_000_000u64.saturating_add(start.elapsed().as_nanos() as u64)
        }
    }
}

/// Identifikues i qëndrueshëm nga package_id (FNV-1a 64-bit).
pub fn stable_id(package_id: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in package_id.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash | 1 /* input_id 0 i rezervuar */
}

/// Ekzekuton pipeline-in e plotë mbi dy origjinat (Quantum + Light).
///
/// LIGJI 0 (zero-copy): `pkg` dhe `light` ZHVENDOSEN brenda (move). Asnjë
/// klon i inputit. Payload-i materializohet (zhvendoset) në vault VETËM te
/// shadow_gj_legacy — kufiri i vetëm i materializimit.
///
/// Asnjë nod nuk shkruan persistent — VETËM shadow_gj_legacy (te fundi).
/// Shkelje e ligjeve sovrane → Err (pa shkrim, pa verdikt).
pub(crate) fn run_pipeline(
    pkg:   PassPackage,
    light: LightEnvelope,
    origin_session: &str,
    vault: &Arc<KnowledgeVault>,
    multi: crate::shadow_spine::MultiVerificationAttestation,
    negative_proof: Option<NegativeContext>,
    current_matrix: Option<&crate::kie_history::VerifiedCounterproofProjection>,
    current_case: Option<&crate::case_reasoning::VerifiedModelCase>,
    ts_ns: u64,
) -> Result<SupremeCandidate, ShadowError> {
    let input_id = stable_id(&pkg.package_id);
    let delivery_scope = format!("input-{input_id:016x}");

    // S.Router zgjedh rrugën (lexim para zhvendosjes).
    let path = ShadowRouter::select_path(&pkg);

    // BASHKIMI: të dy origjinat zhvendosen në bartësin e brendshëm.
    let mut passage = ShadowPassage::new(input_id, pkg, light, path);
    let model_judgment = current_case.filter(|case| case.uses_model_judgment());
    passage.model_case_binding = model_judgment.map(|case| case.model().binding());

    // ── Nodat e përbashkëta: Router, Matrix ───────────────────────────────
    let v_router = crate::noi_caller::invoke("SHADOW_ROUTER", &delivery_scope, || Ok(match model_judgment {
        Some(case) => ShadowRouter::verify_model(&passage.package, path, case, ts_ns),
        None => ShadowRouter::verdict(&passage.package, path, ts_ns),
    }))?;
    passage.push_verdict(v_router);

    let (_case, v_matrix) = crate::noi_caller::invoke("SHADOW_MATRIX", &delivery_scope, || match current_matrix {
        Some(evidence) => match current_case {
            Some(case) => ShadowMatrix::systematize_case(&passage.package, evidence, case, ts_ns),
            None => ShadowMatrix::systematize_current(&passage.package, evidence, ts_ns),
        },
        None => Ok(ShadowMatrix::systematize(&passage.package, vault, ts_ns)),
    })?;
    passage.push_verdict(v_matrix);

    // ── Degë sipas rrugës ─────────────────────────────────────────────────
    match path {
        ShadowPath::Fast => {
            // vetëm Router → Matrix → Judiciary
        }
        ShadowPath::Standard => {
            let (_g, v_gen5) = crate::noi_caller::invoke("SHADOW_GEN5", &delivery_scope, || Ok(match model_judgment {
                Some(case) => { let (g, _) = ShadowGen5::verify(&passage, ts_ns);
                    (g, ShadowGen5::verify_model(&passage, case, ts_ns)) },
                None => ShadowGen5::verify(&passage, ts_ns),
            }))?;
            passage.push_verdict(v_gen5);

            let v_type = crate::noi_caller::invoke("SHADOW_TYPE", &delivery_scope, || Ok(match model_judgment { Some(case) => ShadowType::verify_model(&passage, case, origin_session, ts_ns), None => ShadowType::verify(&passage, ts_ns) }))?;
            passage.push_verdict(v_type);

            let v_sovereign = crate::noi_caller::invoke("SHADOW_SOVEREIGN", &delivery_scope, || Ok(match model_judgment { Some(case) => ShadowSovereign::verify_model(&passage, case, ts_ns), None => ShadowSovereign::verify(&passage, ts_ns) }))?;
            passage.push_verdict(v_sovereign);
        }
        ShadowPath::Deep => {
            let (_g, v_gen5) = crate::noi_caller::invoke("SHADOW_GEN5", &delivery_scope, || Ok(match model_judgment {
                Some(case) => { let (g, _) = ShadowGen5::verify(&passage, ts_ns);
                    (g, ShadowGen5::verify_model(&passage, case, ts_ns)) },
                None => ShadowGen5::verify(&passage, ts_ns),
            }))?;
            passage.push_verdict(v_gen5);

            let v_type = crate::noi_caller::invoke("SHADOW_TYPE", &delivery_scope, || Ok(match model_judgment { Some(case) => ShadowType::verify_model(&passage, case, origin_session, ts_ns), None => ShadowType::verify(&passage, ts_ns) }))?;
            passage.push_verdict(v_type);

            let v_temporal = crate::noi_caller::invoke("SHADOW_TEMPORAL", &delivery_scope, || Ok(match model_judgment { Some(case) => ShadowTemporal::verify_model(&passage, case, origin_session, ts_ns), None => ShadowTemporal::verify(&passage, ts_ns) }))?;
            passage.push_verdict(v_temporal);

            let v_sovereign = crate::noi_caller::invoke("SHADOW_SOVEREIGN", &delivery_scope, || Ok(match model_judgment { Some(case) => ShadowSovereign::verify_model(&passage, case, ts_ns), None => ShadowSovereign::verify(&passage, ts_ns) }))?;
            passage.push_verdict(v_sovereign);

            let v_emergence = crate::noi_caller::invoke("SHADOW_EMERGENCE", &delivery_scope, || Ok(match model_judgment { Some(case) => ShadowEmergence::verify_model(&passage, case, ts_ns), None => ShadowEmergence::verify(&passage, ts_ns) }))?;
            passage.push_verdict(v_emergence);

            // NightWatch është ekzekutuar nga ShadowSpine mbi të njëjtin state.
            // Sinjali futet këtu vetëm që Consensus ta konsumojë, pa gjykim të dytë.
            match multi.watch_bit {
                Some(bit) => passage.push_watch_signal(bit, multi.watch_score),
                None => {}
            }

            let v_consensus = crate::noi_caller::invoke("SHADOW_CONSENSUS", &delivery_scope, || Ok(match model_judgment { Some(case) => ShadowConsensus::verify_model(&passage, case, ts_ns), None => ShadowConsensus::verify(&passage, ts_ns) }))?;
            passage.push_verdict(v_consensus);
        }
    }

    // ── Multi-verification state — porta L2/L3 e detyrueshme ─────────────
    passage.push_multi_verification(
        multi.state,
        multi.precedent_signal,
        multi.score,
        multi.final_bit,
        multi.reason_code,
        ts_ns,
    );

    // R15 (F1 verification (2)): a non-finite score is a typed rejection, never a verdict.
    match passage.verdicts.iter().all(|v| v.score.is_finite()) {
        true => {}
        false => {
            return Err(ShadowError::StructuralRefusal(
                crate::types::StructuralRefusal::NonFiniteScore,
            ))
        }
    }

    // ── S.Judiciary — gjykata epistemike (nodi i fundit i rrugës) ─────────
    let v_judiciary = crate::noi_caller::invoke("SHADOW_JUDICIARY", &delivery_scope, || Ok(match model_judgment { Some(case) => ShadowJudiciary::judge_model(&passage, case, ts_ns), None => ShadowJudiciary::judge(&passage, ts_ns) }))?;
    passage.push_verdict(v_judiciary);

    // ── Ligjet sovrane (5 invariantë strukturorë) → Err nëse shkelet ──────
    crate::noi_caller::invoke("SHADOW_SOVEREIGN_LAWS", &delivery_scope, || enforce_sovereign_laws(&passage))?;

    // ── Autoriteti suprem: vendos 0/1, KONSUMON passage, shkruan vault ────
    crate::noi_caller::invoke("SHADOW_GJ_LEGACY", &delivery_scope, || ShadowGjLegacy::judge_supreme(passage, negative_proof, ts_ns))
}

// ════════════════════════════════════════════════════════════════════════════
// TESTE — vula e degradimit kohor (HARDENING)
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod time_guard_tests {
    use super::*;

    #[test]
    fn time_degraded_mark_is_sticky_true() {
        // Vula është NGJITËSE: pas mark → true, përgjithmonë (s'ka unmark).
        mark_time_degraded();
        assert!(time_degraded());
        mark_time_degraded();
        assert!(time_degraded());
    }

    #[test]
    fn now_ns_never_zero_and_monotone_ordered() {
        // Burimi i kohës s'kthen kurrë 0 dhe ruan renditjen.
        let a = now_ns();
        let b = now_ns();
        assert!(a > 0);
        assert!(b >= a);
    }
}
