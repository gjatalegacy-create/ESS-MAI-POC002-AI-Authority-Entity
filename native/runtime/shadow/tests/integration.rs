// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  tests/integration.rs — SHADOW PLATFORM                               ║
// ║  Gjata Legacy™  |  Arkitekt: Bledar Gjata                             ║
// ║                                                                          ║
// ║  Teston INVARIANTËT DETERMINISTË brenda target-it binar Shadow.       ║
// ║  v1.5.9: përfshihet nga main.rs; nuk krijon crate/library anashkaluese. ║
// ║                                                                          ║
// ║  SHËNIM: të gjitha kontrollet që prekin kernelin janë në NJË test të   ║
// ║  vetëm (sekuencial) sepse gjendja e ngrirjes/statistikave të kernelit  ║
// ║  C është globale — ekzekutimi paralel do ta ndërhynte.                ║
// ║                                                                          ║
// ║  Funksionon nën default (C FFI) dhe `--no-default-features --features pure_rust`.
// ╚══════════════════════════════════════════════════════════════════════════╝

use crate::{KnowledgeBand, LightEnvelope, PassPackage, Shadow, ShadowError};

// ── Ndërtues ────────────────────────────────────────────────────────────────

/// Paketë Quantum e fortë (të gjitha dimensionet të larta).
fn strong_pkg(id: &str, mass: f32, suggested: u8) -> PassPackage {
    PassPackage {
        package_id: id.to_string(),
        epistemic_mass: mass,
        evidence_density: 0.95,
        logical_coherence: 0.95,
        causal_integrity: 0.95,
        convergence_strength: 0.95,
        reproducibility: 0.95,
        potentiality_score: 0.95,
        ibe_score: 0.95,
        chain_coherence: 0.95,
        conservation_ok: true,
        candidate_scores: vec![0.90, 0.92, 0.91],
        suggested_verdict: suggested,
        raw_bytes: b"epistemic-payload".to_vec(),
        final_evidence_digest: 0,
        quantum_action_state: 0x1122_3344_5566_7788,
        quantum_action_mask: crate::living_trust_contract::REQUIRED_ACTION_MASK,
        quantum_required_action_mask: crate::living_trust_contract::REQUIRED_ACTION_MASK,
        pd_gcl_process_digest: 0,
        spine_completion_digest: 0,
        scientific_project: None,
    }
}

/// Envelope Light me vulën 500 të pranishme.
fn sealed_light(session_id: &str) -> LightEnvelope {
    LightEnvelope::sealed(
        session_id,
        "INDUSTRIAL",
        vec![1, 2, 3, 4, 5],
        b"trace".to_vec(),
    )
}

/// Envelope Light me vulën e hequr (fragmentim).
fn stripped_light(session_id: &str) -> LightEnvelope {
    LightEnvelope::stripped(
        session_id,
        "INDUSTRIAL",
        vec![1, 2, 3, 4, 5],
        b"trace".to_vec(),
    )
}

fn vault_total(s: &Shadow) -> usize {
    let st = s.stats();
    st.primitive_count + st.hypothesis_count + st.fact_count + st.negative_count + st.legacy_count
}

// ── Invariantët sovranë (një test sekuencial) ───────────────────────────────

#[test]
fn sovereign_invariants() {
    let shadow = Shadow::new().expect("init Shadow");

    // §1 — Vula 500 (deterministe, aritmetikë e pastër)
    assert!(
        crate::ShadowGjLegacy::seal_check(0xA451),
        "0xA451 është vula 500"
    );
    assert!(
        !crate::ShadowGjLegacy::seal_check(0x0000),
        "0x0000 = e hequr"
    );

    // §2 — E vulosur ⇒ primitive == verified (primitive = verified & seal,
    //       seal=1 për 0xA451 në sistem jo të ngrirë)
    let r = shadow
        .ingest(strong_pkg("p1", 1.0, 2), sealed_light("sess-p1"))
        .unwrap();
    assert_eq!(
        r.verdict.primitive, r.verdict.verified,
        "input i vulosur: primitive duhet të barazohet me verified"
    );
    assert_eq!(
        r.session_id, "sess-p1",
        "session_id zhvendoset në përgjigje"
    );

    // §3 — E hequr mbi Y=1 prodhon (1,0): refuzim kushtetues, zero side-effect.
    let before_refusal = vault_total(&shadow);
    let refusal = shadow
        .ingest(strong_pkg("p2", 1.0, 2), stripped_light("sess-p2"))
        .expect_err("(1,0) duhet refuzuar");
    assert!(matches!(
        refusal,
        ShadowError::StructuralRefusal(crate::types::StructuralRefusal::NonConstitutionalPair {
            y: 1,
            x: 0
        })
    ));
    assert_eq!(vault_total(&shadow), before_refusal);

    // §4 — Banda mbetet deterministe; (0,0) pa provë negative nuk komitohet.
    let r = shadow
        .ingest(strong_pkg("p3", 1.0, 2), sealed_light("sess-p3"))
        .unwrap();
    assert_eq!(r.verdict.knowledge_band, KnowledgeBand::from_mass(1.0));
    assert_eq!(KnowledgeBand::from_mass(0.15), KnowledgeBand::Negative);
    let before_missing_proof = vault_total(&shadow);
    let missing_proof = shadow
        .ingest(strong_pkg("p4", 0.15, 0), stripped_light("sess-p4"))
        .expect_err("(0,0) pa provë negative duhet HOLD/refusal");
    assert!(matches!(
        missing_proof,
        ShadowError::StructuralRefusal(crate::types::StructuralRefusal::MissingNegativeProof)
    ));
    assert_eq!(vault_total(&shadow), before_missing_proof);

    // §5 — NJË ingest shkruan SAKTËSISHT një rekord (primitiv fresh s'promovon)
    let before = vault_total(&shadow);
    let mut p5 = strong_pkg("p5", 1.0, 2);
    p5.raw_bytes = b"epistemic-payload-p5".to_vec();
    drop(shadow.ingest(p5, sealed_light("sess-p5")).unwrap());
    let after = vault_total(&shadow);
    assert_eq!(
        after,
        before + 1,
        "çdo vendim shkruan saktësisht një rekord sovran"
    );

    // §6 — Statistikat e kernelit: passed + blocked == total (invariant)
    let st = shadow.stats();
    assert_eq!(st.passed + st.blocked, st.total, "konservim i statistikave");

    // §7 — Shkelje ligji sovran (suggested_verdict jashtë {0,1,2}) ⇒ Err
    let e = shadow
        .ingest(strong_pkg("p6", 1.0, 5), sealed_light("sess-p6"))
        .unwrap_err();
    assert!(
        matches!(e, ShadowError::SovereignViolation(_)),
        "L2: pretendim i pavlefshëm"
    );

    // §8 — Ngrirja → refuzim i fortë; pas shkrirjes → pranon sërish
    shadow.freeze(1);
    assert!(shadow.is_frozen());
    let e = shadow
        .ingest(strong_pkg("p7", 1.0, 2), sealed_light("sess-p7"))
        .unwrap_err();
    assert!(
        matches!(e, ShadowError::Frozen(_)),
        "i ngrirë → asnjë vendim i ri"
    );
    shadow.unfreeze();
    assert!(!shadow.is_frozen());
    let ok = shadow.ingest(strong_pkg("p8", 1.0, 2), sealed_light("sess-p8"));
    assert!(ok.is_ok(), "pas shkrirjes, Shadow pranon sërish");
}

#[test]
fn r31_real_shadow_pipeline_requires_critical_delivery_and_no_write_on_refusal() {
    let s=Shadow::new().unwrap();
    crate::noi_caller::test_events();
    let result=s.ingest(strong_pkg("r31-delivery",1.0,2),sealed_light("r31-session")).unwrap();
    assert_eq!(result.verdict.verified,1);
    let events=crate::noi_caller::test_events();
    let entered:Vec<_>=events.iter().filter(|(_,phase)|phase=="ENTER").map(|(site,_)|site.as_str()).collect();
    for site in ["SHADOW_MATRIX","SHADOW_JUDICIARY","SHADOW_GJ_LEGACY","SHADOW_PERSIST_T1"] {
        assert!(entered.contains(&site),"critical operation bypassed 0-NOI: {site}; {entered:?}");
    }
    let position=|name|entered.iter().position(|s|*s==name).unwrap();
    assert!(position("SHADOW_MATRIX")<position("SHADOW_JUDICIARY"));
    assert!(position("SHADOW_JUDICIARY")<position("SHADOW_GJ_LEGACY"));
    assert!(position("SHADOW_GJ_LEGACY")<position("SHADOW_PERSIST_T1"));
    let before=vault_total(&s);
    assert!(s.ingest(strong_pkg("r31-stripped",1.0,2),stripped_light("r31-stripped")).is_err());
    let refused=crate::noi_caller::test_events();
    assert!(!refused.iter().any(|(site,_)|site=="SHADOW_PERSIST_T1"));
    assert_eq!(vault_total(&s),before);
}
