#![warn(clippy::unwrap_used)] // HAPI 7.4: bën .unwrap() të dukshëm (path kritike → .expect/error handling)

// ── RUNTIME HARDENING (      ): runtime_mode dhe dev_simulation MUTEKSKLUZIVË ──
// Garanton që dev_simulation NUK mund të ndikojë rrugën e prodhimit kur
// runtime_mode është aktiv. Aktivizimi i të dyve bashkë → gabim kompilimi.
#[cfg(all(feature = "runtime_mode", feature = "dev_simulation"))]
compile_error!(
    "ESS-MAI: 'runtime_mode' dhe 'dev_simulation' nuk mund të jenë aktive bashkë. \
     Përdor runtime_mode për prodhim (default) OSE dev_simulation për testim lokal \
     (--no-default-features --features dev_simulation), kurrë të dyja."
);

// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  MAIN.RS — BOOT ORCHESTRATOR + QUANTUM RUNTIME                        ║
// ║  GJATA LEGACY™ — Light Platform                                   ║
// ║                                                                          ║
// ║  Light → Quantum → Shadow main → Quantum → Light → output user.        ║
// ║  Light nuk prodhon verdikt Shadow paralel.                              ║
// ╚══════════════════════════════════════════════════════════════════════════╝

// ── GRUPI 1 — core ───────────────────────────────────────────────────────
mod ess_manifest;
mod legalgocrypt;
mod lgc_algorithm;
mod lgc_bridge;

// ── GRUPI 2 — Light Platform ─────────────────────────────────────────────
mod alnur_karina_athar; // MUSKULI i Light — i ushqyer nga gjata_collapse_law
mod besa_nlight; // v1.7.8: muskul vartës i AKA-s, pas Xi/Yi dhe para kollapsit
mod ess_mai_system;
mod evolve_trace;
mod light_coordinator;
mod lingua;
mod nura_core;
mod pd_light; // shtresa gjuhësore: përkthen strukturën e PD/Quantum për UI
mod primitive_split; // KOLAPS KOORDINIMI: i0 → PA → (Xi, Yi) — fragmenti i formulës në Light
mod shadow_selection_bridge;
mod software_contract;
mod trace_info; // v1.7.8: transport procesi për përzgjedhje Shadow

// ── GRUPI 3 — Quantum Bridge () ─────────────────────────────────
mod kie_frontend;
mod pd_failure_frontend;
mod pd_iz_frontend;
mod pd_iz_science_frontend;
mod pd_lgc_frontend;
mod noi_caller;
mod pd_native_followup;
mod pd_operational_bridge;
mod process_deadline;
mod project_process_bridge;
mod quantum_bridge; // v1.6.4: Light/APUPK → Shadow main project witness

// ── GRUPI 4 — tipet historike të Shadow Seal Bridge ─────────────────────
// Moduli ruhet për kompatibilitet/teste; dispatch-i lokal nuk ka caller
// prodhues. Verdikti real kthehet vetëm nga Quantum pas Shadow main.
mod shadow_seal_bridge;

// ── GRUPI 5 — SOVEREIGN 0-COPY + FFI (E RE  — integrim) ──────────────
// Ligji 0-copy: CapHandle + LgcToken + AtomicBool single-use. Vula 500.
use shadow_contracts::boundary::gcl_project as gcl_project_contract; // R15: single source
mod lab_contracts; // LAYER 2: nënbashkësia aktive e gjuhës për Light
mod lab_contracts_v11; // LAYER 3: nënbashkësia aktive e gjuhës së shtresës 3
mod layer2; // LAYER 2: Active Trust Lab (TrustContext + Verification Trace)
mod layer3; // LAYER 3: Neural Law + light_wisdom
mod light_spine; // PATCHIM: kurrizi vertikal (L3→L3→L2→L2)
use shadow_contracts::boundary::living_trust as living_trust_contract; // R15: single source
use shadow_contracts::boundary::pd_continuum as pd_continuum_contract; // R15: single source
use shadow_contracts::boundary::pd_spine as pd_spine_contract; // R15: single source
mod phase9_integration; // FAZA 9: lidh light_spine me rrjedhën (Light s'vendos)
mod sovereign; // v1.6.4: projekti shkencor si proces brenda GCL

// ── GRUPI 6 —  Mini-Algoritmi Mbështetës (integruar) ─────────────
// APUPK (projektet e përdoruesit) + SNB (bug tracking) + Mini Evolver (usage).
mod algorithm_continuum;
mod apupk;
mod mini_evolver;
mod snb;

// ── GRUPI 7 — Urat Sovrane (lidhja e  me Quantum/Shadow) ─────────
mod sovereign_bridges;

// ── GRUPI 8 — Përforcimi i Light (health + audit + guards) ──────────────
mod light_hardening;

use std::io::{self, BufRead, Read, Write};
use std::time::Instant;

use ess_manifest::EssManifest;
use lgc_algorithm::{compute_sha256, generate_kodunik};
use lgc_bridge::{LgcBridge, LGC_SEAL_PRIMITIVE_FLAGS};
use light_coordinator::{
    LightCoordinator, LightInputError, LightRequest, LightResponse, MAX_LIGHT_INPUT_BYTES,
};
use quantum_bridge::buss_init;

// ─────────────────────────────────────────────────────────────────────────
// BOOT STATUS — i njëjtë me
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
enum BootEvidence {
    Verified,
    Info,
    Warning(String),
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BootStep {
    key: String,
    detail: String,
    evidence: BootEvidence,
}

struct BootStatus {
    steps: Vec<BootStep>,
    start: Instant,
}

impl BootStatus {
    fn new() -> Self {
        Self {
            steps: Vec::new(),
            start: Instant::now(),
        }
    }

    fn ok(&mut self, step: &str) {
        self.verified(step, "");
    }

    fn verified(&mut self, key: &str, detail: &str) {
        self.steps.push(BootStep {
            key: key.to_string(),
            detail: detail.to_string(),
            evidence: BootEvidence::Verified,
        });
        let suffix = match detail.is_empty() {
            true => String::new(),
            false => format!(" — {detail}"),
        };
        tracing::info!("BOOT VERIFIED {}{}", key, suffix);
    }

    fn fail(&mut self, key: &str, err: &str) {
        self.steps.push(BootStep {
            key: key.to_string(),
            detail: String::new(),
            evidence: BootEvidence::Failed(err.to_string()),
        });
        tracing::error!("BOOT FAIL {} — {}", key, err);
    }

    fn warn(&mut self, key: &str, msg: &str) {
        self.steps.push(BootStep {
            key: key.to_string(),
            detail: String::new(),
            evidence: BootEvidence::Warning(msg.to_string()),
        });
        tracing::warn!("BOOT WARN {} — {}", key, msg);
    }

    /// Etiketë informative: nuk është dhe nuk mund të bëhet provë kritike.
    fn info(&mut self, key: &str, detail: &str) {
        self.steps.push(BootStep {
            key: key.to_string(),
            detail: detail.to_string(),
            evidence: BootEvidence::Info,
        });
        tracing::info!("BOOT INFO {} — {}", key, detail);
    }

    fn critical_ok(&self) -> bool {
        // `moral` dhe `buss` janë etiketa domeni. Prova reale e transportit
        // është `quantum_buss`; etiketat nuk promovohen në autoritet.
        let critical = ["constitution", "lgc_kernel", "quantum_buss", "ess_manifest"];
        critical.iter().all(|required| {
            self.steps
                .iter()
                .filter(|step| step.key == *required)
                .map(|step| &step.evidence)
                .eq(std::iter::once(&BootEvidence::Verified))
        })
    }

    fn runtime_ok(&self) -> bool {
        self.critical_ok()
            && self
                .steps
                .iter()
                .all(|step| !matches!(&step.evidence, BootEvidence::Failed(_)))
    }

    fn report(&self) -> String {
        let ms = self.start.elapsed().as_millis();
        let active = self.runtime_ok();
        let mut out = format!(
            "\n{0}\n  NURA LEGACY — BOOT REPORT  ({1}ms)\n{0}\n",
            "=".repeat(58),
            ms
        );
        for step in &self.steps {
            let (icon, evidence_detail) = match &step.evidence {
                BootEvidence::Verified => ("VER", String::new()),
                BootEvidence::Info => ("INFO", String::new()),
                BootEvidence::Warning(message) => ("WARN", message.clone()),
                BootEvidence::Failed(error) => ("FAIL", error.clone()),
            };
            let detail = [step.detail.as_str(), evidence_detail.as_str()]
                .into_iter()
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
                .join("; ");
            let suffix = match detail.is_empty() {
                true => String::new(),
                false => format!("  [{detail}]"),
            };
            out += &format!("  {:<4} {:<40}{}\n", icon, step.key, suffix);
        }
        out += &"=".repeat(58);
        out += match active {
            true => "\n  SISTEM AKTIV — Light→Quantum→Shadow main→Quantum→Light\n",
            false => "\n  BOOT I PAVERIFIKUAR — HALT\n",
        };
        out += &"=".repeat(58);
        out
    }
}

// ─────────────────────────────────────────────────────────────────────────
// BOOT HAPAT — 13 + 1 (quantum_buss)
// ─────────────────────────────────────────────────────────────────────────

fn step_constitution(st: &mut BootStatus) {
    let chain = "ALLAH:BLEDAR_GJATA:GJATA_LEGACY:SHADOW_GJ_LEGACY:NURA:ALISAH:AMMY";
    let sha = compute_sha256(chain.as_bytes());
    match sha.len() == 64 {
        true => st.ok("constitution"),
        false => st.fail("constitution", "SHA invalid"),
    }
}

fn step_moral(st: &mut BootStatus) {
    st.info("moral", "etiketë domeni");
}

fn step_lgc_kernel(st: &mut BootStatus, bridge: &LgcBridge) {
    let sha = compute_sha256(b"ESS_MAI_BOOT_TEST");
    let sealed = bridge.check_with_flags("BOOT", &sha, 0.1, LGC_SEAL_PRIMITIVE_FLAGS);
    let stripped = bridge.check_with_flags("BOOT", &sha, 0.1, 0);

    // Freeze provohet mbi të njëjtën portë dhe shkrirja tentohet gjithmonë.
    let freeze = bridge.freeze(1);
    let frozen = bridge.check_with_flags("BOOT", &sha, 0.1, LGC_SEAL_PRIMITIVE_FLAGS);
    let unfreeze = bridge.unfreeze();
    let recovered = bridge.check_with_flags("BOOT", &sha, 0.1, LGC_SEAL_PRIMITIVE_FLAGS);

    let known_vectors_hold = sealed.pass
        && !stripped.pass
        && freeze.is_ok()
        && !frozen.pass
        && unfreeze.is_ok()
        && recovered.pass;
    match known_vectors_hold {
        true => st.verified(
            "lgc_kernel",
            &format!(
                "backend={}; sealed=PASS; stripped=BLOCK; freeze=BLOCK; recovery=PASS",
                bridge.backend_label()
            ),
        ),
        false => st.fail(
            "lgc_kernel",
            &format!(
                "backend={}; sealed={}:{}; stripped={}:{}; freeze={:?}; frozen={}:{}; unfreeze={:?}; recovered={}:{}",
                bridge.backend_label(),
                sealed.pass,
                sealed.reason,
                stripped.pass,
                stripped.reason,
                freeze,
                frozen.pass,
                frozen.reason,
                unfreeze,
                recovered.pass,
                recovered.reason,
            ),
        ),
    }
}

fn step_buss(st: &mut BootStatus) {
    // BUSS: etiketë informative — verifikimi real i bus-it C ndodh te
    // step_quantum_buss; Shadow main thirret nga Quantum, jo paralelisht nga Light.
    st.info("buss", "3 stacione: LIGHT/SHADOW/QUANTUM");
}

fn step_ess_manifest(st: &mut BootStatus) {
    let ess = EssManifest::instance();
    let count = ess.count();
    let root = ess.root_sha();
    match count > 0 && !root.is_empty() {
        true => st.verified("ess_manifest", &format!("{count} module")),
        false => st.fail("ess_manifest", "Registry bosh"),
    }
}

fn step_nura(st: &mut BootStatus) {
    st.info(
        "nura/alisah/ammy",
        "domenet e NURA — routing real në nura_core",
    );
}

fn step_kodunik_test(st: &mut BootStatus) {
    let sha = compute_sha256(b"NURA_LEGACY_ESS_MAI_V1");
    let kod = generate_kodunik(&sha, "NURA");
    match kod.starts_with("KDS:") {
        true => st.ok(&format!("kodunik_pipeline ({})", &kod[..20.min(kod.len())])),
        false => st.fail("kodunik_pipeline", "Format invalid"),
    }
}

// ── HAPI 10 (): Quantum Bus ─────────────────────────────────────
fn step_quantum_buss(st: &mut BootStatus) {
    match quantum_bridge::production_process_ready() {
        Ok(()) => st.verified("quantum_buss", "Light stdin/stdout → Quantum process"),
        Err(error) => st.fail("quantum_buss", &format!("runtime process: {error}")),
    }

    // Kerneli C mbetet compatibility lokale për API-t historike. Ai nuk
    // pretendohet më si transport ndër-proces dhe nuk zëvendëson procesin real.
    match buss_init() {
        Ok(()) => st.ok("local_c_buss (compatibility queue initialized)"),
        Err(error) => st.warn(
            "local_c_buss",
            &format!("compatibility queue unavailable: {error}"),
        ),
    }
}

/// Porta fail-closed e sweep-it Coordination. GCL kërkon saktësisht tri role:
/// kapjen e i₀, split-in Xi/Yi dhe eksportin e ankorës. Modulet Support mbeten
/// të dukshme, por nuk fitojnë autoritet bllokues që kontrata nuk ua jep.
fn gcl_coordination_gate(
    stats: &crate::lab_contracts::gcl_presume::SweepStats,
    lines: &[crate::lab_contracts::gcl_presume::SweepLine],
) -> bool {
    use crate::lab_contracts::gcl_presume::GclRole;
    let required_roles = [
        GclRole::I0Capture,
        GclRole::SplitXiYi,
        GclRole::AnchorExport,
    ];
    let complete = required_roles.iter().all(|required_role| {
        lines
            .iter()
            .filter(|line| {
                line.role == *required_role && line.required && line.readiness.is_ready()
            })
            .count()
            == 1
    });
    stats.required_not_ready == 0 && complete
}

// ── HAPAT E RINJ  — Sovereign +  algoritmet ───────────────────

fn step_sovereign(st: &mut BootStatus) {
    use sovereign::{verify_500, SovereignGate, SEAL_COORDINATOR};
    // Provo capability cycle: issue → validate (një herë).
    let mut gate = SovereignGate::new();
    let handle = gate.issue(SEAL_COORDINATOR);
    let first = gate.validate(handle).is_ok();
    let replay = gate.validate(handle).is_err(); // klon → refuzim
                                                 // Vula 500 invariant.
    let seal_ok = verify_500(0xA451);
    // Të gjitha duhet të kalojnë (produkt boolean, zero if).
    let all_ok = first && replay && seal_ok;
    match all_ok {
        true => st.ok("sovereign (0-copy: CapHandle single-use + vula 500)"),
        false => st.warn("sovereign", "capability cycle dështoi"),
    }
}

fn step_v0415_algorithms(st: &mut BootStatus) {
    use apupk::ApupkCoordinator;
    use mini_evolver::LightMiniEvolver;
    use snb::SnbAlgorithm;
    // Inicializo të 3 algoritmet (boot check).
    let _apupk = ApupkCoordinator::new();
    let _snb = SnbAlgorithm::new();
    let _evolver = LightMiniEvolver::new();
    st.ok(" (APUPK + SNB + Mini Evolver — integruar)");
}

fn step_sovereign_bridges(st: &mut BootStatus) {
    use apupk::{ApupkCoordinator, ProjectUpload};
    use mini_evolver::LightMiniEvolver;
    use sovereign::SovereignGate;
    use sovereign_bridges::{ApupkShadowBridge, MiniEvolverQuantumBridge};
    // Provo një kalim të vulosur APUPK → Shadow.
    let mut gate = SovereignGate::new();
    let coord = ApupkCoordinator::new();
    let project = ProjectUpload {
        project_id: 0,
        user_id: 0,
        project_name: "boot_check".to_string(),
        content: "".to_string(),
    };
    let (_pkg, transfer) = ApupkShadowBridge::prepare_sealed(&mut gate, &coord, project);
    let apupk_ok = transfer.is_valid();
    // Provo Mini Evolver → Quantum.
    let mut gate2 = SovereignGate::new();
    let evolver = LightMiniEvolver::new();
    let (_sum, t2) = MiniEvolverQuantumBridge::summarize_sealed(&mut gate2, &evolver, 0);
    let mini_ok = t2.is_valid();
    match apupk_ok && mini_ok {
        true => st.ok("urat sovrane (APUPK→Shadow, SNB→Shadow, Mini→Quantum)"),
        false => st.warn("urat sovrane", "vula 500 dështoi"),
    }
}

// ── HAPI 15 (E RE ): Përforcimi i Light ──────────────────────────────
fn step_light_hardening(st: &mut BootStatus) {
    use light_hardening::LightHardening;
    use sovereign::SovereignGate;
    use sovereign_bridges::{ApupkShadowBridge, MiniEvolverQuantumBridge, SnbShadowBridge};

    let mut apupk_gate = SovereignGate::new();
    let apupk = apupk::ApupkCoordinator::new();
    let project = apupk::ProjectUpload {
        project_id: 1,
        user_id: 1,
        project_name: "GCL hardening probe".to_string(),
        content: "i0".to_string(),
    };
    let (_, apupk_transfer) = ApupkShadowBridge::prepare_sealed(&mut apupk_gate, &apupk, project);
    let apupk_ready =
        apupk_transfer.is_valid() && apupk_transfer.destination == "shadow_APUPK_memory";

    let mut snb = snb::SnbAlgorithm::new();
    snb.report_bug("light_hardening");
    let mut snb_gate = SovereignGate::new();
    let (snb_ready, snb_seal) = match SnbShadowBridge::prepare_sealed(&mut snb_gate, &snb) {
        Some((report, transfer)) => (
            transfer.is_valid()
                && transfer.destination == "shadow_SNB"
                && report.module_name == "light_hardening",
            transfer.seal_500,
        ),
        None => (false, 0),
    };

    let mut mini = mini_evolver::LightMiniEvolver::new();
    mini.on_algorithm_step(1, "light_hardening", 0.5, 0.5);
    let mut mini_gate = SovereignGate::new();
    let (summary, mini_transfer) =
        MiniEvolverQuantumBridge::summarize_sealed(&mut mini_gate, &mini, 1);
    let mini_ready = mini_transfer.is_valid()
        && mini_transfer.destination == "quantum_knowledge_trace"
        && summary.usage_count == 1
        && mini.total_events() == 1;
    let seals = [apupk_transfer.seal_500, snb_seal, mini_transfer.seal_500];

    let report = LightHardening::run_full(
        apupk_ready,
        snb_ready,
        mini_ready,
        algorithm_continuum::is_complete(),
        &seals,
    );
    // overall_score == 10_000 → të 4 dimensionet kaluan.
    let perfect = report.overall_score == crate::pd_spine_contract::MASS_SCALE;
    match perfect {
        true => st.ok(&format!(
            "light_hardening (health={} sovereign={} no_decision={} route={})",
            crate::pd_spine_contract::FixedMass::new(report.health.health_score).decimal_4(),
            report.sovereign_ok,
            report.no_decision_ok,
            algorithm_continuum::route_signature(),
        )),
        false => st.warn(
            "light_hardening",
            &format!(
                "overall_score={}",
                crate::pd_spine_contract::FixedMass::new(report.overall_score).decimal_4()
            ),
        ),
    }
}

// ─────────────────────────────────────────────────────────────────────────
// RUNTIME LOOP — Light → Quantum → User
// ─────────────────────────────────────────────────────────────────────────

/// Lexon saktësisht një frame UTF-8 të përfunduar me newline. `take()` kufizon
/// leximin dhe allokimin e parë në `limit + CRLF`; kërkesa nuk kopjohet derisa
/// madhësia, framing-u dhe encoding-u të jenë provuar.
fn read_light_request<R: BufRead>(reader: &mut R) -> Result<Option<LightRequest>, LightInputError> {
    let mut frame = Vec::new();
    let observed = {
        let mut bounded = reader.take((MAX_LIGHT_INPUT_BYTES as u64).saturating_add(2));
        bounded
            .read_until(b'\n', &mut frame)
            .map_err(|error| LightInputError::Io(error.kind()))?
    };
    match observed {
        0 => return Ok(None),
        _ => {}
    }

    match frame.last().copied() {
        Some(b'\n') => {
            frame.pop();
            match frame.last().copied() {
                Some(b'\r') => {
                    frame.pop();
                }
                _ => {}
            }
        }
        _ => {
            return match frame.len() > MAX_LIGHT_INPUT_BYTES {
                true => Err(LightInputError::TooLarge {
                    limit: MAX_LIGHT_INPUT_BYTES,
                    observed_at_least: frame.len(),
                }),
                false => Err(LightInputError::IncompleteFrame {
                    observed_bytes: frame.len(),
                }),
            }
        }
    }

    match frame.len() > MAX_LIGHT_INPUT_BYTES {
        true => {
            return Err(LightInputError::TooLarge {
                limit: MAX_LIGHT_INPUT_BYTES,
                observed_at_least: frame.len(),
            })
        }
        false => {}
    }
    let text = String::from_utf8(frame).map_err(|error| LightInputError::InvalidUtf8 {
        valid_up_to: error.utf8_error().valid_up_to(),
    })?;
    LightRequest::try_from_string(text).map(Some)
}

fn run_interactive(
    coordinator: &LightCoordinator,
    native_parent: Option<shadow_contracts::pd_recovery::RecoveryLookup>,
) {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let _ = run_reader(coordinator, native_parent, &mut input, None);
}
// The one-shot KiE entry consumes the same runtime loop and every original gate.
// Only an actually bound terminal result populates this return value.
enum ReaderOutcome {
    Native(shadow_contracts::pd_recovery::RecoveryLookup),
    Operational(shadow_contracts::pd_memory::PdOperationRecord),
}
fn run_reader(
    coordinator: &LightCoordinator,
    native_parent: Option<shadow_contracts::pd_recovery::RecoveryLookup>,
    input: &mut impl BufRead,
    kie_context: Option<&shadow_contracts::kie::KieContext>,
) -> Option<ReaderOutcome> {
    run_reader_with_iz(coordinator, native_parent, input, kie_context, None)
}
fn run_reader_with_iz(
    coordinator: &LightCoordinator,
    mut native_parent: Option<shadow_contracts::pd_recovery::RecoveryLookup>,
    mut input: &mut impl BufRead,
    kie_context: Option<&shadow_contracts::kie::KieContext>,
    mut iz_context: Option<&mut shadow_contracts::pd_memory::iz::IzContext>,
) -> Option<ReaderOutcome> {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    let mut completed = None;
    let mut runtime_evidence = phase9_integration::RuntimeEvidenceSpine::new();

    writeln!(
        out,
        "\nNURA LEGACY —  aktiv. Shkruaj pyetjen (CTRL+C per dalje):"
    )
    .ok();
    writeln!(out, "{}", "─".repeat(58)).ok();

    // Light retains only a current question reference during this process.
    // Durable history/material remain in Shadow's Vault and pd_mp index.
    let mut pending_pd: Option<shadow_contracts::pd_memory::PdOperationRecord> = None;
    let mut pending_iz: Option<shadow_contracts::pd_memory::lgc::PdLgcRef> = None;
    loop {
        let mut request = match read_light_request(&mut input) {
            Ok(Some(request)) => request,
            Ok(None) => break,
            Err(LightInputError::Empty) => {
                eprintln!("[LIGHT INPUT] REFUSED: {}", LightInputError::Empty);
                continue;
            }
            Err(error) => {
                eprintln!("[LIGHT INPUT] FAIL-CLOSED: {error}");
                break;
            }
        };
        let mut declared_context = None;
        if let Some(reference) = pending_iz.as_ref() {
            if let Some(path) = request.text().strip_prefix("/science ") {
                let linked = pd_operational_bridge::read_bounded(std::path::Path::new(path.trim()),48*1024)
                    .and_then(|bytes| pd_iz_science_frontend::interactive(reference,&bytes));
                match linked {
                    Ok(record) => {
                        writeln!(out,"{}",record.request.scoped_question).ok();
                        pd_operational_bridge::print_operational(&record).ok();
                        // Only a result consuming this question closes its field route.
                        // An exact historical retry cannot rewind the current frontier.
                        if record.previous_record == reference.record_id {
                            completed=Some(ReaderOutcome::Operational(record));
                            pending_iz=None;
                        }
                    },
                    Err(error)=>eprintln!("[iZ/KiE] answer not confirmed: {error}"),
                }
                out.flush().ok();
                continue;
            }
            if let Some(path) = request.text().strip_prefix("/answer ") {
                let prepared=pd_operational_bridge::read_bounded(std::path::Path::new(path.trim()),48*1024)
                    .and_then(|bytes|pd_lgc_frontend::interactive_answer(reference,&bytes));
                match prepared {
                    Ok(context)=> {
                        match pd_lgc_frontend::exact(&context) {
                            Ok(Some(shadow_contracts::pd_memory::lgc::PdLgcOutput::Operational(record)))=>{
                                writeln!(out,"{}",record.request.scoped_question).ok();
                                pd_operational_bridge::print_operational(&record).ok();
                                // Idempotent delivery must not move the displayed current frontier backwards.
                                out.flush().ok();
                                continue;
                            },
                            Ok(Some(shadow_contracts::pd_memory::lgc::PdLgcOutput::Native(_)))=>{
                                eprintln!("[iZ] exact native result already committed");
                                continue;
                            },
                            Ok(None)=>{},
                            Err(error)=>{eprintln!("[iZ] answer not confirmed: {error}");continue;},
                        }
                        request=match LightRequest::try_new(context.current_i0()) {
                            Ok(value)=>value,
                            Err(error)=>{eprintln!("[iZ] input refused: {error}");continue;},
                        };
                        declared_context=Some(context);
                    },
                    Err(error)=>{eprintln!("[iZ] answer not confirmed: {error}");continue;},
                }
            } else {
                match pd_lgc_frontend::interactive_reply(reference,request.text()) {
                    Ok(context)=>declared_context=Some(context),
                    Err(error)=>{eprintln!("[iZ] answer not confirmed: {error}");continue;},
                }
            }
        } else if pending_pd.is_none()
            && (request.text().starts_with("/answer ") || request.text().starts_with("/science ")) {
            eprintln!("[iZ] LIGHT_IZ_CURRENT_QUESTION_REQUIRED");
            continue;
        }
        let text = request.text();
        if let Some(parent) = pending_pd.as_ref() {
            if let Some(path) = text.strip_prefix("/science ") {
                let linked = pd_operational_bridge::read_bounded(
                    std::path::Path::new(path.trim()),
                    48 * 1024,
                )
                .and_then(|bytes| pd_failure_frontend::science_answer_json(parent, &bytes));
                match linked {
                    Ok(record) => {
                        writeln!(out, "{}", record.request.scoped_question).ok();
                        pd_operational_bridge::print_operational(&record).ok();
                        pending_pd = Some(record);
                    }
                    Err(error) => eprintln!("[PD/KiE] no record committed: {error}"),
                }
                continue;
            }
            // Four explicit user distinctions; no generated answer enters PD.
            let mut fields = vec![text.to_string()];
            for prompt in [
                "Cili shembull ose rezultat e kundershton pohimin?",
                "Cili dallim konkret ndan ate qe eshte nga ajo qe nuk eshte?",
                "Cili eshte burimi ose vezhgimi yt? Nese nuk ke prove, thuaje qarte.",
            ] {
                writeln!(out, "{prompt}").ok();
                out.flush().ok();
                match read_light_request(&mut input) {
                    Ok(Some(answer)) => fields.push(answer.text().to_string()),
                    _ => break,
                }
            }
            if fields.len() != 4 {
                writeln!(
                    out,
                    "Pyetja mbetet e ruajtur; pergjigjja me kater pjese nuk u perfundua."
                )
                .ok();
                break;
            }
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(parent.record_id);
            for field in &fields {
                hasher.update((field.len() as u64).to_le_bytes());
                hasher.update(field.as_bytes());
            }
            let operation = format!("reply-{:x}", hasher.finalize());
            match pd_failure_frontend::answer_record(
                parent, &operation, &fields[0], &fields[1], &fields[2], &fields[3],
            ) {
                Ok(record) => {
                    writeln!(out, "{}", record.request.scoped_question).ok();
                    writeln!(out, "Cili shembull ose rezultat e mbeshtet pohimin?").ok();
                    pd_operational_bridge::print_operational(&record).ok();
                    pending_pd = Some(record);
                }
                Err(reason) => {
                    writeln!(out, "Pergjigjja nuk u konfirmua ne Vault: {reason}").ok();
                }
            }
            continue;
        }

        // A native reference alone must not lose the inquiry path on the next
        // ordinary user turn. Reuse ICX3 under the same case; BESA resolves its
        // current map before ASHT, and Shadow remains the only storage owner.
        let automatic_native_inquiry = iz_context.is_none() && kie_context.is_none() && native_parent.is_some();
        let mut implicit_context = if declared_context.is_some() {
            declared_context
        } else if automatic_native_inquiry {
            match pd_lgc_frontend::interactive_native_reply(text, native_parent.as_ref().expect("checked native predecessor")) {
                Ok(context) => Some(context),
                Err(reason) => { eprintln!("[PD_MAP] native inquiry not started: {reason}"); break; }
            }
        } else { None };
        let mut cycle_iz_context = iz_context.as_deref_mut().or(implicit_context.as_mut());

        // ── HAPI 1: Light try_receive() ───────────────────────────────────
        let mut response =
            match coordinator.try_receive_with_iz(&request, cycle_iz_context.as_deref_mut()) {
                Ok(response) => response,
                Err(error) => {
                    eprintln!("[LIGHT i₀] FAIL-CLOSED: {error}");
                    break;
                }
            };

        if let Some(context) = cycle_iz_context.as_deref() {
            if let Err(error) = pd_iz_frontend::validate_territory(context, &response.domain) {
                eprintln!("[Operational_iZ] {error}");
                break;
            }
        }
        if let Some(context) = kie_context {
            if let Err(error) = kie_frontend::validate_territory(context, &response.domain) {
                eprintln!("[KiE] {error}");
                break;
            }
        }
        match response.status.is_ok() {
            false => {
                writeln!(
                    out,
                    "[LIGHT ERR] {} detail={:?}",
                    response.status.label(),
                    response.status
                )
                .ok();
                continue;
            }
            true => {}
        }

        writeln!(
            out,
            "[TRACE] {} | domain={} | contract={}...",
            response.trace_id,
            response.domain,
            &response.contract_id[..16.min(response.contract_id.len())]
        )
        .ok();

        // ── PA (Primitive Anchor) — përdoret GJURMA E NJËJTË e krijuar në
        // LightCoordinator::try_receive. Rikrijimi i TraceInfo këtu do të lindte
        // një i₀ të dytë (ID/timestamp/seq tjetër) dhe do të ndante Quantum nga
        // Shadow. Primitive Trace bartet; nuk rindërtohet.
        match response.formula_anchor_valid() {
            true => {}
            false => {
                eprintln!(
                    "  [PA→QUANTUM] FAIL-CLOSED: ankora GCL (i₀/PA/split/contract/receipt) nuk është autoritative"
                );
                continue;
            }
        }
        match response.ready_for_handoff() {
            Ok(()) => {}
            Err(reason) => {
                eprintln!("  [LEGACY] FAIL-CLOSED: eksporti nuk u autorizua ({reason})");
                continue;
            }
        }
        let handoff_evidence = match export_primitive_anchor(&response) {
            Ok(evidence) => evidence,
            Err(reason) => {
                eprintln!("  [PA→QUANTUM] FAIL-CLOSED: {reason}");
                continue;
            }
        };
        match response.complete_legacy_handoff(&handoff_evidence) {
            Ok(()) => {}
            Err(reason) => {
                eprintln!("  [LEGACY] FAIL-CLOSED: handoff-i nuk u mbyll ({reason})");
                continue;
            }
        }
        println!(
            "  [KOLAPS/1] KOORDINIMI: Xi={} Yi={} — hapësirat primitive lindën",
            response.primitive_split.xi.len(),
            response.primitive_split.yi.len()
        );
        match response.legacy.as_ref() {
            Some(legacy) => println!(
                "  [LAW0]     before={} after={} receipt={}...",
                legacy.receipt.law_before,
                legacy.receipt.law_after,
                &legacy.receipt.receipt_sha256[..16],
            ),
            None => {
                eprintln!("  [LAW0] FAIL-CLOSED: receipt mungon pas handoff-it");
                continue;
            }
        }

        // ── HAPI 2: dispatch_to_quantum() ────────────────────────────────
        write!(out, "[QUANTUM] Duke dërguar... ").ok();
        out.flush().ok();

        // ── SPINE 9 (FORTIFIKUAR) — kurrizi vertikal i Light-it aktivizohet:
        // UI→gjurmim→kuptim (L3) → besim aktiv (L2). Faza 9 e lidh dhe VERIFIKON
        // në runtime ligjin kushtetues "Light s'vendos" (output = interpretim/
        // besim, KURRË verdikt). Zero if — match. Fail-safe: s'e ndal rrjedhën,
        // e ushqen me shtresën e besimit që deri tani rrinte e fjetur.
        let ui_input = layer3::UiInput {
            text: text.to_string(),
            session_id: response.trace_id.clone(),
            deep_research: false,
        };
        let evidence_snapshot = match runtime_evidence.snapshot(&response) {
            Ok(snapshot) => Some(snapshot),
            Err(reason) => {
                writeln!(
                    out,
                    "\n  [RUNTIME_EVIDENCE] DEGRADED: {reason} — trust historik u mbyll"
                )
                .ok();
                None
            }
        };
        let empty_signals = layer2::ReliabilitySignals::empty();
        let (rel_signals, history_refs, verification_trace, evidence_route_key) =
            match &evidence_snapshot {
                Some(snapshot) => (
                    &snapshot.signals,
                    // Phase9 zotëron Vec-in, ndërsa snapshot-i duhet të mbetet
                    // gjallë që VerificationTrace të huazohet nga i njëjti atom.
                    snapshot.history_refs.clone(),
                    snapshot.verification_trace.as_ref(),
                    Some(snapshot.route_key),
                ),
                None => (&empty_signals, Vec::new(), None, None),
            };
        let phase9 = phase9_integration::Phase9Integration::interpret_with_evidence_spine(
            ui_input,
            response.primitive_trace.id,
            rel_signals,
            response.domain.clone(),
            history_refs,
            response.primitive_trace.ts_ns,
            response.formula_anchor_valid(),
            verification_trace,
            evidence_route_key,
        );
        // VERIFIKIM RUNTIME i ligjit: output-i i Light NUK është verdikt.
        let light_lawful = phase9_integration::Phase9Integration::output_is_not_verdict(&phase9);
        match (phase9.law_check.is_passed(), light_lawful, &phase9.spine) {
            (true, true, Some(sp)) => {
                writeln!(out, "\n  [LIGHT_EMOTIONAL_SPINE]   besim={:.3} prioritet_i_butë={:.3} udhëzim={} evidencë={} hardened={} (Light interpreton, s'vendos)",
                    sp.trust_context.trust_score, sp.soft_priority,
                    light_spine::LightSpine::guidance_label(sp),
                    sp.verification_evidence_count,
                    sp.evidence_hardened).ok();
            }
            (_, false, _) => {
                // Strukturalisht i pamundur (Light s'ka API vendimi); fail-closed
                // dëshmi: nëse ndonjëherë do prishej, e kapim, s'e maskojmë.
                writeln!(out, "\n  [LIGHT_EMOTIONAL_SPINE]   SHKELJE: output-i i Light u perceptua si verdikt — kufiri u ruajt").ok();
            }
            _ => {
                writeln!(
                    out,
                    "\n  [LIGHT_EMOTIONAL_SPINE]   besim s'u ndërtua (ligjet s'kaluan)"
                )
                .ok();
            }
        }

        // Cursor-i fiksohet para procesit. QUANTUM shkruan receipt-in e plotë
        // SHADOW→PD përpara envelope-it Q→LIGHT; një envelope pa append të ri,
        // unik dhe të lidhur me këtë i₀ kthehet në BridgeError fail-closed.
        let pd_cursor = capture_pd_handoff_cursor();
        let raw_dispatch = coordinator.dispatch_to_quantum_with_iz(
            &response,
            text,
            native_parent.as_ref(),
            kie_context,
            cycle_iz_context.as_deref(),
        );
        // This durable inquiry returns before native receipt/Evolve terminal
        // logic: operational continuity cannot imply either 0:0 or 1:1.
        if let light_coordinator::DispatchResult::Operational(record) = raw_dispatch {
            match shadow_contracts::pd_memory::late_refusal::render_iz(&record) {
                Ok(question) => {
                    writeln!(out, "{question}").ok();
                    completed = Some(ReaderOutcome::Operational(record));
                    if automatic_native_inquiry { pending_iz = None; continue; }
                },
                Err(reason) => eprintln!("[Operational_iZ] stored late-refusal rendering failed: {reason}"),
            }
            break;
        }
        if !matches!(
            &raw_dispatch,
            light_coordinator::DispatchResult::Verified(_)
        ) {
            if let Some(context) = cycle_iz_context.as_deref() {
                // Exit/diagnostic output never proves a committed refusal.
                // Resolve the exact operation from Shadow before showing iZ.
                eprintln!("[Operational_iZ] native terminal unavailable: {raw_dispatch:?}; resolving exact Vault operation");
                if automatic_native_inquiry {
                    match pd_iz_frontend::exact_operation(context) {
                        Ok(Some(record)) => {
                            writeln!(out, "{}", record.request.scoped_question).ok();
                            pd_operational_bridge::print_operational(&record).ok();
                            pending_iz = match pd_lgc_frontend::display_reference(&record,&mut out) {
                                Ok(reference)=>Some(reference),
                                Err(reason)=>{eprintln!("[iZ] reference unavailable: {reason}");break;},
                            };
                            completed = Some(ReaderOutcome::Operational(record));
                            continue;
                        },
                        Ok(None) => eprintln!("[Operational_iZ] no exact durable inquiry was committed"),
                        Err(reason) => eprintln!("[Operational_iZ] exact inquiry read refused: {reason}"),
                    }
                }
                break;
            }
            match pd_failure_frontend::recover_native_refusal(&response, text) {
                Ok(Some(record)) => {
                    // A recorded missing condition is an inquiry, not a 0/0 verdict.
                    // Preserve the failed authority's closure and let the user supply the distinction.
                    writeln!(out, "{}", record.request.scoped_question).ok();
                    writeln!(out, "Cili shembull ose rezultat e mbeshtet pohimin?").ok();
                    pd_operational_bridge::print_operational(&record).ok();
                    pending_pd = Some(record);
                    continue;
                }
                Ok(None) => {}
                Err(reason) => eprintln!("[PD_MAP] inquiry recovery unavailable: {reason}"),
            }
        }
        let (dispatch, pd_delivery) = match (raw_dispatch, pd_cursor) {
            (light_coordinator::DispatchResult::Verified(result), Ok(cursor)) => {
                match read_pd_delivery_since(&cursor, &response, &result) {
                    Ok(delivery) => {
                        let receipt_id = delivery.terminal_receipt.receipt_id();
                        writeln!(
                            out,
                            "\n  [SHADOW_RECEIPT] id={}... i₀/PA/verdict/Trust=fiksuar",
                            &receipt_id[..16.min(receipt_id.len())],
                        )
                        .ok();
                        (
                            light_coordinator::DispatchResult::Verified(result),
                            Some(delivery),
                        )
                    }
                    Err(reason) => (
                        light_coordinator::DispatchResult::BridgeError(format!(
                            "QBRIDGE:SHADOW_RECEIPT:{reason}"
                        )),
                        None,
                    ),
                }
            }
            (light_coordinator::DispatchResult::Verified(_), Err(reason)) => (
                light_coordinator::DispatchResult::BridgeError(format!(
                    "QBRIDGE:SHADOW_RECEIPT:{reason}"
                )),
                None,
            ),
            (other, _) => (other, None),
        };

        // HAPI 3 — përdor EvolveTrace-n autoritative brenda LegacyCoordination.
        let (output, shadow_verdict) =
            coordinator.handle_quantum_response(&dispatch, &mut response);
        let terminal_evidence_bound = match &evidence_snapshot {
            Some(snapshot) => phase9_integration::RuntimeEvidenceSpine::terminal_evidence_is_bound(
                &dispatch,
                &response,
                snapshot,
                &shadow_verdict,
                pd_delivery
                    .as_ref()
                    .map(|delivery| &delivery.terminal_receipt),
            ),
            None => false,
        };
        match &dispatch {
            light_coordinator::DispatchResult::Verified(result) => {
                match (
                    terminal_evidence_bound,
                    evidence_snapshot.as_ref(),
                    pd_delivery.as_ref(),
                ) {
                    (true, Some(snapshot), Some(delivery)) => match runtime_evidence
                        .record_terminal(
                            &response,
                            snapshot,
                            result,
                            &delivery.terminal_receipt,
                            true,
                        ) {
                        Ok(record) => {
                            writeln!(
                                out,
                                "\n  [RUNTIME_EVIDENCE] route={:016x} route_sha256={} ref={:016x} evidence_sha256={} survived={} retained={}",
                                record.route_key,
                                record.route_sha256,
                                record.evidence_ref,
                                record.evidence_sha256,
                                record.survived,
                                record.retained_count,
                            )
                            .ok();
                        }
                        Err(reason) => {
                            writeln!(out, "\n  [RUNTIME_EVIDENCE] REFUSED: {reason}").ok();
                        }
                    },
                    _ => {
                        writeln!(
                            out,
                            "\n  [RUNTIME_EVIDENCE] REFUSED: terminali nuk u lidh me EvolveTrace + receipt-in SHADOW"
                        )
                        .ok();
                    }
                }
            }
            light_coordinator::DispatchResult::Timeout(reason) => {
                writeln!(
                    out,
                    "\n  [RUNTIME_EVIDENCE] NOT_RECORDED: timeout transporti ({reason})"
                )
                .ok();
            }
            light_coordinator::DispatchResult::BridgeError(reason) => {
                writeln!(
                    out,
                    "\n  [RUNTIME_EVIDENCE] NOT_RECORDED: gabim ure ({reason})"
                )
                .ok();
            }
            light_coordinator::DispatchResult::Operational(_) => {
                unreachable!("operational records return before terminal evidence");
            }
        }

        if terminal_evidence_bound {
            if let light_coordinator::DispatchResult::Verified(result) = &dispatch {
                match pd_native_followup::bind_completed(
                    result,
                    &response.trace_id,
                    &response.domain,
                ) {
                    Ok(reference) => {
                        if let Ok(encoded) =
                            shadow_contracts::pd_followup::encode_reference(&reference)
                        {
                            writeln!(out, "[PD_TOPIC_REFERENCE] {encoded}").ok();
                        }
                        completed = Some(ReaderOutcome::Native(reference.clone()));
                        native_parent = Some(reference);
                        pending_iz = None;
                    }
                    Err(error) => {
                        eprintln!("[PD_MAP] continuation reference refused: {error}");
                        break;
                    }
                }
            }
        }
        // Raport i shkurtër
        writeln!(out, "{}", dispatch.algorithm_state()).ok();
        writeln!(out, "[SFX] {}", dispatch.evolve_sfx()).ok();
        match response.legacy.as_ref() {
            Some(legacy) => {
                writeln!(
                    out,
                    "[SEAL] flags=0x{:04X} strength={:.3}",
                    legacy.evolve.primitive_flags(),
                    legacy.evolve.seal_strength()
                )
                .ok();
            }
            None => {
                writeln!(out, "[SEAL] unavailable — Legacy receipt missing").ok();
            }
        }
        writeln!(out, "[SHADOW_GJ_LEGACY] {}", shadow_verdict.label()).ok();
        writeln!(out, "\n{}\n{}", "─".repeat(58), output).ok();
        // Minimal Light output: the same receipt-verified PD surface is displayed once.
        if let Some(delivery) = pd_delivery.as_ref() {
            writeln!(out, "{}", coordinator.core.speak_pd(&delivery.surface)).ok();
        }
        writeln!(out, "{}\n", "─".repeat(58)).ok();
    }
    completed
}

// ─────────────────────────────────────────────────────────────────────────
// PA EXPORT — handoff minimal drejt Shadow (file, jo mekanizëm i ri).
// FORMAT PACP (KOLAPS KOORDINIMI — formula i0→PA→(Xi,Yi) e gdhendur):
//   session_id|pa_id_hex|ts_ns|xi:a,b,c|yi:d,e|c:<crc> — 6 fusha.
// Shadow e lexon (feed_primitive_anchors, pranon legacy 3/5 dhe PACP 6) dhe mban
// PA + (Xi,Yi) NË PRITJE për propozimin XY të Quantum. Quantum e lexon të
// NJËJTIN skedar për (Xi,Yi) — një burim i vetëm i së vërtetës për split-in.
// Dështimi i çdo shkrimi NDAL handoff-in e këtij cikli. PA publikohet vetëm
// pasi receipt-i dhe commit-i janë sinkronizuar; Quantum nuk thirret pa evidence.
// ─────────────────────────────────────────────────────────────────────────
/// ZGJIDHJA e cwd-dependencës (#8): path-et e handoff zgjidhen nga
/// ESSMAI_HANDOFF_DIR (dosja e përbashkët e të tre binarëve) — fallback relativ.
/// Zero unwrap — match.
fn handoff_path(file: &str) -> String {
    match std::env::var("ESSMAI_HANDOFF_DIR") {
        Ok(dir) => {
            // BOOT SOVRAN: dosja krijohet po të mungojë dhe kanonikalizohet
            // në ABSOLUTE — zero varësi nga cwd, gjurmueshmëri e përsëritshme.
            match std::fs::create_dir_all(&dir) {
                Ok(()) => {}
                Err(e) => {
                    eprintln!("[BOOT] FATAL (fail-closed): ESSMAI_HANDOFF_DIR='{dir}' s'u krijua dot ({e}).");
                    std::process::exit(1);
                }
            }
            let abs = match std::fs::canonicalize(&dir) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("[BOOT] FATAL (fail-closed): ESSMAI_HANDOFF_DIR='{dir}' s'u kanonikalizua ({e}).");
                    std::process::exit(1);
                }
            };
            let joined = abs.join(file);
            joined.to_string_lossy().into_owned()
        }
        Err(_) => {
            // ZERO IMPROVIZIM: pa rrugë të deklaruar s'ka nisje — rrugët
            // relative (cwd) dëmtojnë determinizmin dhe gjurmën e auditimit.
            eprintln!("[BOOT] FATAL (fail-closed): ESSMAI_HANDOFF_DIR mungon.");
            eprintln!("[BOOT] Cakto dosjen e përbashkët të urave, p.sh.:");
            eprintln!("[BOOT]   Linux/macOS:  export ESSMAI_HANDOFF_DIR=/opt/essmai/handoff");
            eprintln!("[BOOT]   Windows:      $env:ESSMAI_HANDOFF_DIR='C:\\essmai\\handoff'");
            std::process::exit(1);
        }
    }
}

/// URA 8 (v1.4.5): shkruan një dimension të vektorit TRL — rresht i vulosur
/// `sid|dim|lvl|c:` në trl_vector.txt. Best-effort, kurrë bllokues.
fn shkruaj_trl_dim(session_id: &str, dim: &str, lvl: u8) {
    let body = format!("{}|{}|{}", session_id, dim, lvl);
    let line = crate::lab_contracts::pa_wire::seal_body(&body);
    use std::io::Write as _;
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(handoff_path("trl_vector.txt"))
    {
        Ok(mut f) => match f.write_all(line.as_bytes()).and_then(|_| f.flush()) {
            Ok(()) => {}
            Err(e) => eprintln!("  [TRL_VEKTOR] ALARM: dimensioni '{dim}' s'u shkrua ({e})"),
        },
        Err(e) => eprintln!("  [TRL_VEKTOR] ALARM: trl_vector.txt s'u hap ({e})"),
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{:02x}", byte)).collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DurableAppend {
    line_sha256: String,
}

impl DurableAppend {
    fn is_synced(&self) -> bool {
        crate::lgc_algorithm::is_canonical_sha256(&self.line_sha256)
    }
}

fn append_and_sync(path: impl AsRef<std::path::Path>, line: &str) -> Result<DurableAppend, String> {
    use std::io::Write as _;
    let path = path.as_ref();
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("{}: open failed: {error}", path.display()))?;
    file.write_all(line.as_bytes())
        .map_err(|error| format!("{}: write failed: {error}", path.display()))?;
    file.flush()
        .map_err(|error| format!("{}: flush failed: {error}", path.display()))?;
    file.sync_data()
        .map_err(|error| format!("{}: sync_data failed: {error}", path.display()))?;
    drop(file);

    // Shkrimi nuk quhet provë vetëm sepse syscall-i u kthye me sukses.
    // Rreshti i saktë rilexohet pas sync_data; çdo mospërputhje mbyl handoff-in.
    let expected = line.trim_end_matches('\n');
    let persisted = std::fs::read_to_string(path)
        .map_err(|error| format!("{}: read-back failed: {error}", path.display()))?;
    match persisted.lines().last() == Some(expected) {
        true => {}
        false => {
            return Err(format!(
                "{}: durable read-back did not match appended line",
                path.display()
            ))
        }
    }
    Ok(DurableAppend {
        line_sha256: crate::lgc_algorithm::compute_domain_sha(
            "ESSMAI/LIGHT/HANDOFF/LINE",
            &[line.as_bytes()],
        ),
    })
}

fn export_primitive_anchor(
    response: &LightResponse,
) -> Result<crate::alnur_karina_athar::HandoffEvidence, String> {
    response.ready_for_handoff()?;
    let legacy = response
        .legacy
        .as_ref()
        .ok_or_else(|| "legacy coordination missing from GCL formula anchor".to_string())?;
    let session_id = response.trace_id.as_str();
    let trace = &response.primitive_trace;
    let split = &response.primitive_split;
    let receipt = &legacy.receipt;
    let besa = &legacy.besa_attestation;
    let dir = std::env::var("ESSMAI_HANDOFF_DIR")
        .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().into_owned());

    let lineage_ok = session_id == trace.label()
        && receipt.trace_id == session_id
        && receipt.trace_origin == trace.id
        && receipt.primitive_anchor == trace.id_hex()
        && receipt.split_wire == split.to_wire()
        && besa.trace_id == session_id
        && besa.split_sha256 == receipt.split_sha256;
    match lineage_ok {
        true => {}
        false => {
            let reason = "session/trace/PA/split lineage mismatch";
            let _ = crate::lab_contracts::rrjedha::note(
                &dir,
                "LIGHT",
                "main::export_primitive_anchor",
                crate::lab_contracts::rrjedha::RrjedhaClass::WireInvariant,
                reason,
            );
            return Err(reason.to_string());
        }
    }

    let kodunik_verified = match receipt.verify_runtime() {
        Ok(()) => true,
        Err(error) => {
            return Err(format!("legacy receipt invalid before export: {error}"));
        }
    };

    let pa_line = crate::lab_contracts::pa_wire::encode_line_verified(
        session_id,
        &trace.id_hex(),
        trace.ts_ns,
        &split.to_wire(),
    )
    .map_err(|reason| format!("PA source verification failed: {reason}"))?;

    // Companion receipt: envelope-i ruhet hex që delimituesit e formatit Legacy
    // të mos deformojnë kontratën e urës tekstuale.
    let envelope_hex = hex_encode(receipt.envelope.as_bytes());
    let receipt_body = format!(
        "{}|{}|{}|{}|{}|{}",
        session_id,
        receipt.receipt_sha256,
        receipt.binding_sha256,
        receipt.envelope_integrity_sha256,
        receipt.kodunik.code,
        envelope_hex,
    );
    let receipt_line = crate::lab_contracts::pa_wire::seal_body_verified(&receipt_body, &[])
        .map_err(|reason| format!("Legacy receipt source verification failed: {reason}"))?;

    // Rendi kushtetues: receipt → Besa → commit → PA. PA është barriera e
    // publikimit që lexon Quantum; asnjë fragment i paplotë nuk bëhet dispatch.
    let receipt_write = append_and_sync(handoff_path("light_lgc_receipt.txt"), &receipt_line)?;
    let receipt_line_sha256 = receipt_write.line_sha256.clone();

    // Kontrata ekzistuese Quantum Besa ka saktësisht nëntë fusha. Prova e
    // zgjeruar −Xi/−Yi është palosur në enrichment_sha256, jo në wire shape.
    let besa_body = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}",
        session_id,
        receipt.input_sha256,
        receipt.primitive_anchor,
        besa.trace_id,
        besa.split_sha256,
        besa.hold_id,
        besa.hold_sha256,
        besa.selection_sha256,
        besa.enrichment_sha256,
    );
    let besa_line = crate::lab_contracts::pa_wire::seal_body_verified(&besa_body, &[])
        .map_err(|reason| format!("Besa companion source verification failed: {reason}"))?;
    let besa_write = append_and_sync(handoff_path("light_besa_export.txt"), &besa_line)?;
    let besa_line_sha256 = besa_write.line_sha256.clone();

    let primitive_anchor_sha256 = crate::lgc_algorithm::compute_domain_sha(
        "ESSMAI/LIGHT/HANDOFF/LINE",
        &[pa_line.as_bytes()],
    );
    let besa_attestation_sha256 = besa.attestation_sha256();
    let commit_body = format!(
        "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|COMMITTED",
        session_id,
        trace.id_hex(),
        receipt.receipt_sha256,
        receipt.binding_sha256,
        receipt.split_sha256,
        besa_attestation_sha256,
        besa.enrichment_sha256,
        receipt_line_sha256,
        besa_line_sha256,
        primitive_anchor_sha256,
    );
    let commit_line = crate::lab_contracts::pa_wire::seal_body_verified(&commit_body, &[])
        .map_err(|reason| format!("Legacy commit source verification failed: {reason}"))?;
    let commit_write = append_and_sync(handoff_path("light_lgc_handoff_commit.txt"), &commit_line)?;
    let primitive_anchor_write = append_and_sync(handoff_path("light_pa_export.txt"), &pa_line)?;

    // TRL është kanal ndihmës. Nuk përcakton vlefshmërinë e handoff-it.
    let text = format!("{} {}", split.xi.join(" "), split.yi.join(" "));
    let (_lang, confidence) = crate::lingua::LangDetector::detect(&text);
    let level = crate::lab_contracts::trl::TrlVector::trl_from_confidence(confidence);
    shkruaj_trl_dim(session_id, "in", level);

    let receipt_written = receipt_write.is_synced();
    let besa_written = besa_write.is_synced();
    let primitive_anchor_written = primitive_anchor_write.is_synced();
    let commit_written = commit_write.is_synced();
    let flushed = receipt_written && besa_written && primitive_anchor_written && commit_written;

    Ok(crate::alnur_karina_athar::HandoffEvidence {
        binding_sha256: receipt.binding_sha256.clone(),
        receipt_sha256: receipt.receipt_sha256.clone(),
        besa_attestation_sha256,
        besa_enrichment_sha256: besa.enrichment_sha256.clone(),
        commit_sha256: commit_write.line_sha256.clone(),
        primitive_anchor_sha256: primitive_anchor_write.line_sha256.clone(),
        receipt_written,
        besa_written,
        primitive_anchor_written,
        commit_written,
        flushed,
        kodunik_verified,
    })
}

// ─────────────────────────────────────────────────────────────────────────
// pd_light reader — receipt-i terminal ekzistues Quantum→Light.
//
// Para nisjes së procesit Quantum fiksohet një cursor i skedarit. Pas kthimit
// pranohet vetëm një rresht i RI, i vulosur, për session-in aktual dhe vetëm kur
// prefix-i historik mbetet byte-për-byte i pandryshuar. Kjo mbyll replay/stale
// pa shtuar wire, modul ose storage të ri.
// ─────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, PartialEq, Eq)]
struct PdHandoffCursor {
    prefix_len: usize,
    prefix_sha256: String,
}

impl PdHandoffCursor {
    fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        let boundary_valid = match bytes.last() {
            Some(last) => *last == b'\n',
            None => true,
        };
        match boundary_valid {
            true => Ok(Self {
                prefix_len: bytes.len(),
                prefix_sha256: compute_sha256(bytes),
            }),
            false => Err(String::from("PD_CURSOR_PREFIX_NOT_LINE_ALIGNED")),
        }
    }
}

fn capture_pd_handoff_cursor() -> Result<PdHandoffCursor, String> {
    match std::fs::read(handoff_path("quantum_pd_export.txt")) {
        Ok(bytes) => PdHandoffCursor::from_bytes(&bytes),
        Err(error) => match error.kind() {
            std::io::ErrorKind::NotFound => PdHandoffCursor::from_bytes(&[]),
            _ => Err(format!("PD_CURSOR_READ_FAILED:{error}")),
        },
    }
}

fn fresh_pd_line<'a>(
    current: &'a [u8],
    cursor: &PdHandoffCursor,
    session_id: &str,
) -> Result<&'a str, String> {
    match current.len() > cursor.prefix_len {
        true => {}
        false => return Err(String::from("PD_RECEIPT_FRESH_APPEND_MISSING")),
    }
    let prefix_sha256 = compute_sha256(&current[..cursor.prefix_len]);
    match prefix_sha256 == cursor.prefix_sha256 {
        true => {}
        false => return Err(String::from("PD_RECEIPT_PREFIX_DRIFT")),
    }
    let appended = std::str::from_utf8(&current[cursor.prefix_len..])
        .map_err(|_| String::from("PD_RECEIPT_APPEND_NOT_UTF8"))?;
    match appended.ends_with('\n') {
        true => {}
        false => return Err(String::from("PD_RECEIPT_APPEND_TRUNCATED")),
    }
    let session_prefix = format!("{session_id}|");
    let mut matching = appended
        .lines()
        .filter(|line| line.starts_with(session_prefix.as_str()));
    let line = matching
        .next()
        .ok_or_else(|| String::from("PD_RECEIPT_CURRENT_SESSION_MISSING"))?;
    match matching.next() {
        Some(_) => return Err(String::from("PD_RECEIPT_CURRENT_SESSION_DUPLICATE")),
        None => {}
    }
    match crate::lab_contracts::pa_wire::verify_line_generic(line, &[6, 7], 46) {
        crate::lab_contracts::pa_wire::PaWireVerdict::SealedOk => Ok(line),
        crate::lab_contracts::pa_wire::PaWireVerdict::LegacyOk => {
            // Ruhet gjurma historike e formatit vetëm në audit ledger;
            // terminal_authority=false e ndalon të hyjë te Evolve/Trust/UI.
            let dir = std::env::var("ESSMAI_HANDOFF_DIR")
                .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().into_owned());
            let _ = crate::lab_contracts::rrjedha::note(
                &dir,
                "LIGHT",
                "main::fresh_pd_line",
                crate::lab_contracts::rrjedha::RrjedhaClass::TransportDowngrade,
                &format!(
                    "LEGACY_PRANUAR kind=PD bridge=quantum_pd_export session={session_id} fresh_append=true terminal_authority=false"
                ),
            );
            Err(String::from("PD_RECEIPT_LEGACY_DOWNGRADE"))
        }
        crate::lab_contracts::pa_wire::PaWireVerdict::Corrupt(reason) => {
            Err(format!("PD_RECEIPT_WIRE_INVALID:{reason}"))
        }
    }
}

fn read_pd_delivery_since(
    cursor: &PdHandoffCursor,
    response: &LightResponse,
    result: &quantum_bridge::QuantumResult,
) -> Result<pd_light::VerifiedPdDelivery, String> {
    let current = std::fs::read(handoff_path("quantum_pd_export.txt"))
        .map_err(|error| format!("PD_RECEIPT_READ_FAILED:{error}"))?;
    let line = fresh_pd_line(&current, cursor, &response.trace_id)?;
    let body = line
        .rsplit_once("|c:")
        .map(|(body, _)| body)
        .ok_or_else(|| String::from("PD_RECEIPT_CRC_FIELD_MISSING"))?;
    let input = pd_light::PdLight::parse_handoff(body)
        .ok_or_else(|| String::from("PD_RECEIPT_BODY_INVALID"))?;
    let delivery = pd_light::PdLight::deliver(&input)
        .ok_or_else(|| String::from("PD_RECEIPT_CONSTITUTIONAL_PROOF_INVALID"))?;
    match delivery.closes_pre_dispatch_cycle(response, result) {
        true => Ok(delivery),
        false => Err(String::from("PD_RECEIPT_NOT_BOUND_TO_LIGHT_GCL_CYCLE")),
    }
}

// ─────────────────────────────────────────────────────────────────────────
// MAIN
// ─────────────────────────────────────────────────────────────────────────

fn main() {
    tracing_subscriber::fmt()
        .with_target(false)
        .with_level(true)
        .init();

    // ══ BOOT: sistemi E LEXON ligjin — gjata_collapse_law shpallet i gjallë.
    crate::lab_contracts::proclaim_law("LIGHT");

    // Edhe porta one-shot e projekteve hyn vetëm pasi LIGHT ka shpallur GCL.
    // Vetë bridge-i riverifikon një direktivë Coordination para çdo APUPK.
    let args = std::env::args().collect::<Vec<_>>();
    let lgc_args = match pd_lgc_frontend::parse_args(&args) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("[LGC] {error}");
            std::process::exit(64);
        }
    };
    let iz_args = match pd_iz_frontend::parse_args(&args) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("[Operational_iZ] {error}");
            std::process::exit(64);
        }
    };
    let kie_args = match kie_frontend::parse_args(&args) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("[KiE] {error}");
            std::process::exit(64);
        }
    };
    if let Some(code) = pd_iz_science_frontend::dispatch(&args) {
        std::process::exit(code);
    }
    if let Some(code) = crate::pd_operational_bridge::dispatch(&args) {
        std::process::exit(code);
    }
    match crate::project_process_bridge::dispatch_project_route(&args) {
        Some(code) => std::process::exit(code),
        None => {}
    }

    // ══ GCL_PRESUME (v1.4.1): gatishmëria nën ligj — komandanti (CollapsePhase)
    // urdhëron, presume ping-on; edhe modulet anësore përgjigjen me rol+status,
    // zero arsyetim shtesë në to. Vetë sweep-i mbetet informues sipas kontratës;
    // Light fail-close vetëm tri rolet që kontrata i shënon REQUIRED në Coordination.
    {
        use crate::lab_contracts::gcl_presume as presume;
        use crate::lab_contracts::CollapsePhase;
        fn gati_env() -> presume::GclReadiness {
            match (
                std::env::var("ESSMAI_HANDOFF_DIR"),
                crate::lab_contracts::first_guardian::known_vector_lock(),
                crate::lab_contracts::first_guardian::wire_roundtrip_lock(),
            ) {
                (Ok(_), true, true) => presume::GclReadiness::Ready,
                (Err(_), _, _) => presume::GclReadiness::NotReady("ESSMAI_HANDOFF_DIR mungon"),
                (_, false, _) => presume::GclReadiness::NotReady("FNV known-vector dështoi"),
                (_, _, false) => presume::GclReadiness::NotReady("PA wire roundtrip dështoi"),
            }
        }
        fn gati_gcl() -> presume::GclReadiness {
            let directive =
                crate::lab_contracts::GjataCollapseLaw::issue(CollapsePhase::Coordination);
            match crate::lab_contracts::GjataCollapseLaw::verify(
                &directive,
                CollapsePhase::Coordination,
            ) {
                true => presume::GclReadiness::Ready,
                false => presume::GclReadiness::NotReady("GCL Coordination directive invalid"),
            }
        }
        fn gati_i0() -> presume::GclReadiness {
            let raw = b"ESSMAI/LIGHT/I0/READINESS";
            let trace = match crate::trace_info::TraceInfo::try_new(raw) {
                Ok(trace) => trace,
                Err(_) => {
                    return presume::GclReadiness::NotReady(
                        "i₀ capture nuk fitoi identitet instance/clock të vlefshëm",
                    )
                }
            };
            let directive =
                crate::lab_contracts::GjataCollapseLaw::issue(CollapsePhase::Coordination);
            let label = trace.label();
            let instance_suffix = format!(
                "P{:08x}_{:016x}",
                trace.process_id, trace.process_started_ns,
            );
            let captured = trace.raw_len == raw.len()
                && trace.id == (crate::trace_info::fnv64(raw) ^ trace.ts_ns)
                && trace.is_valid()
                && label.contains(&trace.id_hex())
                && label.ends_with(&instance_suffix);
            match captured
                && crate::lab_contracts::GjataCollapseLaw::verify(
                    &directive,
                    CollapsePhase::Coordination,
                ) {
                true => presume::GclReadiness::Ready,
                false => presume::GclReadiness::NotReady(
                    "i₀ capture ose GCL Coordination self-check dështoi",
                ),
            }
        }
        fn gati_split() -> presume::GclReadiness {
            let query =
                crate::lingua::LinguaCore::new().parse_input("motor industrial kuq sensor termik");
            let split = crate::primitive_split::split_primitive(&query);
            let disjoint = split.xi.iter().all(|term| !split.yi.contains(term));
            let mut actual = split
                .xi
                .iter()
                .chain(split.yi.iter())
                .cloned()
                .collect::<Vec<_>>();
            let mut expected = query.domain_terms;
            actual.sort();
            expected.sort();
            match split.has_material() && disjoint && actual == expected {
                true => presume::GclReadiness::Ready,
                false => presume::GclReadiness::NotReady("Xi/Yi invariant dështoi"),
            }
        }
        fn gati_lgc() -> presume::GclReadiness {
            let sha = crate::lgc_algorithm::compute_sha256(b"GCL/LIGHT/READINESS");
            match crate::lgc_algorithm::generate_kodunik_checked(&sha, "GCL") {
                Ok(kodunik) => match kodunik.verify(&sha) {
                    true => presume::GclReadiness::Ready,
                    false => presume::GclReadiness::NotReady("KODUNIK self-check dështoi"),
                },
                Err(_) => presume::GclReadiness::NotReady("KODUNIK nuk u gjenerua"),
            }
        }
        fn gati_software_contract() -> presume::GclReadiness {
            match crate::ess_mai_system::communication::is_authorized(
                "light_coordinator",
                "quantum_platform",
            ) {
                true => presume::GclReadiness::Ready,
                false => presume::GclReadiness::NotReady(
                    "software contract route Light→Quantum nuk autorizohet",
                ),
            }
        }
        fn gati_living_trust() -> presume::GclReadiness {
            use crate::living_trust_contract::{LivingTrustProof, REQUIRED_ACTION_MASK};
            let proof = LivingTrustProof {
                action_state: 1,
                action_mask: REQUIRED_ACTION_MASK,
                required_action_mask: REQUIRED_ACTION_MASK,
                verified: 1,
                primitive: 1,
                knowledge_band: 1,
                lgc_law: crate::lab_contracts::gjata_collapse_law::law_seal() as u32,
                system_laws_seal: crate::lab_contracts::system_laws_seal(),
                scientific_project_sha256: [0u8; 32],
                scientific_project_verdict_sha256: [0u8; 32],
                sovereign_flags: 0xA451,
            };
            match crate::living_trust_contract::compute(proof, 0.5) {
                Some(seal) => match seal.verifies(proof, 0.5) {
                    true => presume::GclReadiness::Ready,
                    false => presume::GclReadiness::NotReady(
                        "Living Trust known-proof nuk u riverifikua",
                    ),
                },
                None => {
                    presume::GclReadiness::NotReady("Living Trust known-proof nuk u materializua")
                }
            }
        }
        fn gati_pd_surface() -> presume::GclReadiness {
            match crate::pd_light::PdLight::parse_handoff("not-a-pd-handoff").is_none() {
                true => presume::GclReadiness::Ready,
                false => {
                    presume::GclReadiness::NotReady("PD Light parser pranoi material të pavlefshëm")
                }
            }
        }
        fn gati_nura() -> presume::GclReadiness {
            let core = crate::nura_core::NuraCore::new();
            match matches!(
                core.route("motor industrial sensor"),
                crate::nura_core::Domain::Work
            ) {
                true => presume::GclReadiness::Ready,
                false => presume::GclReadiness::NotReady("Nura routing self-check dështoi"),
            }
        }
        fn gati_trl_input() -> presume::GclReadiness {
            match (
                crate::lab_contracts::trl::TrlVector::trl_from_confidence(0.95),
                crate::lab_contracts::trl::TrlVector::trl_from_confidence(0.0),
            ) {
                (4, 0) => presume::GclReadiness::Ready,
                _ => presume::GclReadiness::NotReady("TRL input thresholds driftuan"),
            }
        }
        fn gati_project_contract() -> presume::GclReadiness {
            let content_sha256 = "11".repeat(32);
            let trace_id = crate::gcl_project_contract::project_trace_id(1, 1, "GCL readiness");
            let context = crate::gcl_project_contract::context_sha256(
                &crate::gcl_project_contract::ProjectContextMaterial {
                    project_id: 1,
                    user_id: 1,
                    trace_id,
                    revision: 0,
                    project_title: "GCL readiness",
                    content_sha256: &content_sha256,
                    light_sovereign_flags: 0xA451,
                },
            );
            match crate::gcl_project_contract::is_hex_32(&context)
                && crate::gcl_project_contract::seal_is_500(0xA451)
            {
                true => presume::GclReadiness::Ready,
                false => {
                    presume::GclReadiness::NotReady("GCL project contract canonicalization dështoi")
                }
            }
        }
        fn shadow_seal_historical() -> presume::GclReadiness {
            presume::GclReadiness::Degraded(
                "compatibility-only; verdikti prodhues vjen nga Shadow main përmes Quantum",
            )
        }
        presume::register("lingua_i0", presume::GclRole::I0Capture, gati_i0);
        presume::register("primitive_split", presume::GclRole::SplitXiYi, gati_split);
        presume::register(
            "pa_export(pa_wire)",
            presume::GclRole::AnchorExport,
            gati_env,
        );
        presume::register(
            "pd_light_surface",
            presume::GclRole::Support("sipërfaqe"),
            gati_pd_surface,
        );
        presume::register(
            "light_coordinator",
            presume::GclRole::Support("koordinim"),
            gati_gcl,
        );
        presume::register(
            "runtime_evidence_spine",
            presume::GclRole::Support("evidence-trust"),
            crate::phase9_integration::RuntimeEvidenceSpine::gcl_status,
        );
        presume::register(
            "lgc_algorithm",
            presume::GclRole::Support("legacy"),
            gati_lgc,
        );
        presume::register(
            "software_contract",
            presume::GclRole::Support("kontratë"),
            gati_software_contract,
        );
        presume::register(
            "living_trust_contract",
            presume::GclRole::Support("trust"),
            gati_living_trust,
        );
        presume::register(
            "gcl_project_contract",
            presume::GclRole::Support("project-trust-continuity"),
            gati_project_contract,
        );
        presume::register(
            "quantum_bridge",
            presume::GclRole::Support("urë-quantum"),
            crate::quantum_bridge::gcl_status,
        );
        presume::register(
            "shadow_selection_bridge",
            presume::GclRole::Support("selection-read-only"),
            crate::shadow_selection_bridge::gcl_status,
        );
        presume::register(
            "shadow_seal_bridge",
            presume::GclRole::Support("compatibility"),
            shadow_seal_historical,
        );
        presume::register(
            "nura_core",
            presume::GclRole::Support("bërthamë"),
            gati_nura,
        );
        // ZGJERIMI v1.4.2 (audit A6 + rek.3): TRL-input hyn në apel — Light-ana
        // e vektorit TRL (input_trl), gatishmëri për ta ushqyer verifikimin.
        presume::register(
            "trl_input(lingua)",
            presume::GclRole::Support("trl-in"),
            gati_trl_input,
        );
        let dir_rrjedhe = std::env::var("ESSMAI_HANDOFF_DIR")
            .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().into_owned());

        // ══ FRYMA E PARË (v1.4.4, paradigma ESSMAI_FIRST_GUARDIAN): organet
        // e Light-it rrahin NË KËTË makinë para se të shërbejnë — dëshmi e vulosur.
        use crate::lab_contracts::first_guardian as fryma;
        fn org_env() -> bool {
            match std::env::var("ESSMAI_HANDOFF_DIR") {
                Ok(_) => true,
                Err(_) => false,
            }
        }
        fn org_gjuha() -> bool {
            // lingua: organi i i₀-s — zbulon gjuhën dhe nxjerr terma realë
            // (API-t EKZISTUESE: LangDetector::detect + Normalizer::content_words).
            let teksti = "flamur kuq shqiponje";
            let (lang, _besim) = crate::lingua::LangDetector::detect(teksti);
            let terma = crate::lingua::Normalizer::content_words(teksti, &lang);
            !terma.is_empty()
        }
        // v1.4.5: fryma hyn edhe në apel — READY vetëm nga fryma e PLOTË.
        presume::register(
            "fryma_e_pare",
            presume::GclRole::Support("frymë"),
            crate::lab_contracts::first_guardian::breath_status,
        );
        let _ = fryma::attest(
            "LIGHT",
            &dir_rrjedhe,
            &[
                fryma::RiteCheck {
                    name: "fnv_known_vector",
                    run: fryma::known_vector_lock,
                    critical: true,
                },
                fryma::RiteCheck {
                    name: "pa_wire_roundtrip",
                    run: fryma::wire_roundtrip_lock,
                    critical: true,
                },
                fryma::RiteCheck {
                    name: "handoff_env",
                    run: org_env,
                    critical: false,
                },
                fryma::RiteCheck {
                    name: "lingua_i0",
                    run: org_gjuha,
                    critical: false,
                },
            ],
        );
        let (coordination_stats, coordination_lines) =
            presume::sweep("LIGHT", CollapsePhase::Coordination, &dir_rrjedhe);
        match gcl_coordination_gate(&coordination_stats, &coordination_lines) {
            true => {}
            false => {
                tracing::error!(
                    "BOOT GCL COORDINATION DËSHTOI — i₀/split/anchor jo të plota ose të dyfishta"
                );
                std::process::exit(1);
            }
        }
    }

    tracing::info!("{}", "=".repeat(58));
    tracing::info!("  NURA LEGACY —  BOOT (Light→Quantum→Shadow main)");
    tracing::info!("{}", "=".repeat(58));

    let mut st = BootStatus::new();

    // ── BOOT SEQUENCE ─────────────────────────────────────────────────────
    step_constitution(&mut st); // 1. Kushtetuta
    step_moral(&mut st); // 2. Moral Core
    let bridge = LgcBridge::new();
    step_lgc_kernel(&mut st, &bridge); // 3. LGC Kernel
    step_buss(&mut st); // 4. BUSS 3 stacione
    step_ess_manifest(&mut st); // 5. ESS Manifest
    step_nura(&mut st); // 6. NURA/ALISAH/AMMY (domenet e NURA)
    step_kodunik_test(&mut st); // 7. KODUNIK
    step_quantum_buss(&mut st); // 8. Quantum Bus
    step_sovereign(&mut st); // 9. Sovereign 0-copy
    step_v0415_algorithms(&mut st); // 10. algoritmet
    step_sovereign_bridges(&mut st); // 11. Urat sovrane
    step_light_hardening(&mut st); // 12. Përforcimi i Light

    println!("{}", st.report());

    match st.runtime_ok() {
        false => {
            tracing::error!("BOOT KRITIKE DESHTOI — HALT");
            std::process::exit(1);
        }
        true => {}
    }

    // Sweep-i i dytë pas boot-it provon edhe gjendjen reale të Quantum bus.
    // Autoriteti bllokues mbetet vetëm te tri rolet REQUIRED të Coordination.
    let gcl_dir = std::env::var("ESSMAI_HANDOFF_DIR")
        .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().into_owned());
    let (gcl_stats, gcl_lines) = crate::lab_contracts::gcl_presume::sweep(
        "LIGHT",
        crate::lab_contracts::CollapsePhase::Coordination,
        &gcl_dir,
    );
    match gcl_coordination_gate(&gcl_stats, &gcl_lines) {
        true => {}
        false => {
            release_quantum_process();
            tracing::error!("GCL COORDINATION POST-BOOT DËSHTOI — HALT");
            std::process::exit(1);
        }
    }

    // ── RUNTIME ───────────────────────────────────────────────────────────
    let coordinator = LightCoordinator::new();
    if let Some(lgc_args) = lgc_args {
        let result = (|| {
            let mut input = pd_lgc_frontend::prepare(lgc_args)?;
            if let Some(existing) = pd_lgc_frontend::exact(&input.context)? {
                return pd_lgc_frontend::publish(&input, &existing);
            }
            let mut reader =
                std::io::Cursor::new(format!("{}\n", input.context.current_i0()).into_bytes());
            let native_parent = pd_iz_frontend::native_predecessor(&input.context)?;
            let native = run_reader_with_iz(
                &coordinator,
                native_parent,
                &mut reader,
                None,
                Some(&mut input.context),
            );
            let output = match native {
                Some(ReaderOutcome::Native(lookup)) => pd_lgc_frontend::completed(&input.context, &lookup)?,
                Some(ReaderOutcome::Operational(record)) => shadow_contracts::pd_memory::lgc::PdLgcOutput::Operational(record),
                None => pd_lgc_frontend::exact(&input.context)?.ok_or("LIGHT_LGC_NO_DURABLE_RESULT")?,
            };
            pd_lgc_frontend::publish(&input, &output)
        })();
        if let Err(error) = result {
            release_quantum_process();
            eprintln!("[LGC] {error}");
            std::process::exit(70);
        }
    } else if let Some(iz_args) = iz_args {
        let result = (|| {
            let mut input = pd_iz_frontend::prepare(iz_args)?;
            if let Some(existing) = pd_lgc_frontend::exact(&input.context)? {
                return match existing {
                    shadow_contracts::pd_memory::lgc::PdLgcOutput::Operational(record) => {
                        pd_iz_frontend::publish(&input, &record)
                    }
                    native => pd_lgc_frontend::publish(
                        &pd_lgc_frontend::LgcInput {
                            context: input.context,
                            output: input.output_record,
                        },
                        &native,
                    ),
                };
            }
            pd_iz_frontend::require_current_input_mode(&input.context)?;
            let mut reader =
                std::io::Cursor::new(format!("{}\n", input.context.current_i0()).into_bytes());
            let native_parent = pd_iz_frontend::native_predecessor(&input.context)?;
            if let Some(outcome) = run_reader_with_iz(
                &coordinator,
                native_parent,
                &mut reader,
                None,
                Some(&mut input.context),
            ) {
                let native = match outcome {
                    ReaderOutcome::Native(lookup) => pd_lgc_frontend::completed(&input.context, &lookup)?,
                    ReaderOutcome::Operational(record) => return pd_iz_frontend::publish(&input, &record),
                };
                return pd_lgc_frontend::publish(
                    &pd_lgc_frontend::LgcInput {
                        context: input.context,
                        output: input.output_record,
                    },
                    &native,
                );
            }
            let record=pd_iz_frontend::exact_operation(&input.context)?
                .ok_or("LIGHT_IZ_NO_DURABLE_OPERATION: no operational success or authority verdict inferred")?;
            pd_iz_frontend::publish(&input, &record)
        })();
        if let Err(error) = result {
            release_quantum_process();
            eprintln!("[Operational_iZ] {error}");
            std::process::exit(70);
        }
    } else if let Some(kie_args) = kie_args {
        let result = (|| {
            let input = kie_frontend::prepare(kie_args)?;
            let mut reader =
                std::io::Cursor::new(format!("{}\n", input.canonical_text).into_bytes());
            let completed = run_reader(
                &coordinator,
                input.predecessor.clone(),
                &mut reader,
                Some(&input.context),
            )
            .ok_or("LIGHT_KIE_NO_BOUND_TERMINAL_RESULT")?;
            match completed {
                ReaderOutcome::Native(reference) => kie_frontend::publish(&input, &reference),
                ReaderOutcome::Operational(record) => kie_frontend::publish_operational(&input, &record),
            }
        })();
        if let Err(error) = result {
            release_quantum_process();
            eprintln!("[KiE] {error}");
            std::process::exit(70);
        }
    } else {
        let native_parent = match pd_native_followup::initial(&args) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("[PD_MAP] {error}");
                std::process::exit(70);
            }
        };
        run_interactive(&coordinator, native_parent);
    }

    // ── SHUTDOWN ──────────────────────────────────────────────────────────
    release_quantum_process();
    tracing::info!("Sistema mbyllur — quantum_buss destroyed.");
}

#[cfg(test)]
mod v168_light_tests {
    use super::*;
    use std::io::Cursor;
    use std::sync::Mutex;

    static HANDOFF_ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn bounded_ingress_accepts_exact_limit_with_lf_or_crlf() {
        for ending in [b"\n".as_slice(), b"\r\n".as_slice()] {
            let mut bytes = vec![b'a'; MAX_LIGHT_INPUT_BYTES];
            bytes.extend_from_slice(ending);
            let mut cursor = Cursor::new(bytes);
            let request = read_light_request(&mut cursor)
                .expect("exact byte limit must be readable")
                .expect("one complete request");
            assert_eq!(request.raw_len(), MAX_LIGHT_INPUT_BYTES);
            assert_eq!(request.text().len(), MAX_LIGHT_INPUT_BYTES);
        }
    }

    #[test]
    fn bounded_ingress_refuses_limit_plus_one_before_request_copy() {
        let mut bytes = vec![b'a'; MAX_LIGHT_INPUT_BYTES + 1];
        bytes.push(b'\n');
        let mut cursor = Cursor::new(bytes);
        match read_light_request(&mut cursor) {
            Err(LightInputError::TooLarge {
                limit: MAX_LIGHT_INPUT_BYTES,
                observed_at_least,
            }) => assert_eq!(observed_at_least, MAX_LIGHT_INPUT_BYTES + 1),
            other => panic!("expected bounded rejection, received {other:?}"),
        }
    }

    #[test]
    fn bounded_ingress_refuses_empty_invalid_utf8_and_incomplete_frames() {
        let mut empty = Cursor::new(b"\n".to_vec());
        assert!(matches!(
            read_light_request(&mut empty),
            Err(LightInputError::Empty)
        ));

        let mut invalid_utf8 = Cursor::new(vec![0xff, b'\n']);
        assert!(matches!(
            read_light_request(&mut invalid_utf8),
            Err(LightInputError::InvalidUtf8 { valid_up_to: 0 })
        ));

        let mut incomplete = Cursor::new(b"unterminated".to_vec());
        assert!(matches!(
            read_light_request(&mut incomplete),
            Err(LightInputError::IncompleteFrame { observed_bytes: 12 })
        ));
    }

    #[cfg(feature = "runtime_mode")]
    #[test]
    fn runtime_boot_uses_real_c_kernel() {
        let _guard = crate::lgc_bridge::phase1_test_lock();
        let bridge = LgcBridge::new();
        assert_eq!(bridge.backend_label(), "C_KERNEL");
        let mut status = BootStatus::new();
        step_lgc_kernel(&mut status, &bridge);
        assert_eq!(status.steps.len(), 1);
        assert!(matches!(&status.steps[0].evidence, BootEvidence::Verified));
    }

    fn record_remaining_critical_steps(status: &mut BootStatus) {
        status.verified("lgc_kernel", "fixture");
        status.verified("quantum_buss", "fixture");
        status.verified("ess_manifest", "fixture");
    }

    #[test]
    fn boot_critical_steps_require_one_exact_verified_evidence() {
        let mut informational = BootStatus::new();
        informational.info("constitution", "label only");
        record_remaining_critical_steps(&mut informational);
        assert!(!informational.critical_ok());

        let mut prefixed = BootStatus::new();
        prefixed.verified("constitution_fallback", "not the exact key");
        record_remaining_critical_steps(&mut prefixed);
        assert!(!prefixed.critical_ok());

        let mut exact = BootStatus::new();
        exact.verified("constitution", "SHA verified");
        record_remaining_critical_steps(&mut exact);
        assert!(exact.critical_ok());
        assert!(exact.runtime_ok());

        exact.fail("noncritical_probe", "a failure cannot be ignored");
        assert!(exact.critical_ok());
        assert!(!exact.runtime_ok());

        exact.warn("constitution", "duplicate evidence");
        assert!(!exact.critical_ok());
    }

    #[test]
    fn lgc_block_or_missing_backend_can_never_be_reported_verified() {
        let _guard = crate::lgc_bridge::phase1_test_lock();
        let unavailable = LgcBridge::unavailable_for_test();
        let mut failed = BootStatus::new();
        step_lgc_kernel(&mut failed, &unavailable);
        assert_eq!(failed.steps.len(), 1);
        assert_eq!(failed.steps[0].key, "lgc_kernel");
        assert!(matches!(&failed.steps[0].evidence, BootEvidence::Failed(_)));

        let simulation = LgcBridge::rust_simulation_for_test();
        let mut verified = BootStatus::new();
        step_lgc_kernel(&mut verified, &simulation);
        assert_eq!(verified.steps.len(), 1);
        assert_eq!(verified.steps[0].key, "lgc_kernel");
        assert!(matches!(
            &verified.steps[0].evidence,
            BootEvidence::Verified
        ));
    }

    fn sealed_pd_fixture(session_id: &str) -> String {
        let mut fields = vec![session_id.to_string()];
        fields.extend((1..45).map(|index| format!("f{index}")));
        crate::lab_contracts::pa_wire::seal_body_verified(&fields.join("|"), &[6, 7])
            .expect("45-field PD fixture must seal")
    }

    #[test]
    fn coordination_gate_requires_one_ready_copy_of_each_constitutional_role() {
        use crate::lab_contracts::gcl_presume::{GclReadiness, GclRole, SweepLine, SweepStats};
        let stats = SweepStats {
            total: 3,
            ready: 3,
            degraded: 0,
            required_not_ready: 0,
        };
        let lines = vec![
            SweepLine {
                role: GclRole::I0Capture,
                readiness: GclReadiness::Ready,
                required: true,
            },
            SweepLine {
                role: GclRole::SplitXiYi,
                readiness: GclReadiness::Ready,
                required: true,
            },
            SweepLine {
                role: GclRole::AnchorExport,
                readiness: GclReadiness::Ready,
                required: true,
            },
        ];
        assert!(gcl_coordination_gate(&stats, &lines));

        let mut duplicate = lines.clone();
        duplicate.push(SweepLine {
            role: GclRole::AnchorExport,
            readiness: GclReadiness::Ready,
            required: true,
        });
        assert!(!gcl_coordination_gate(&stats, &duplicate));
        assert!(!gcl_coordination_gate(&stats, &lines[..2]));

        let not_ready = SweepStats {
            required_not_ready: 1,
            ..stats
        };
        assert!(!gcl_coordination_gate(&not_ready, &lines));
    }

    #[test]
    fn pd_cursor_accepts_only_a_fresh_unique_sealed_session_line() {
        let historical = sealed_pd_fixture("historical");
        let cursor =
            PdHandoffCursor::from_bytes(historical.as_bytes()).expect("aligned historical prefix");
        let current_line = sealed_pd_fixture("current");
        let current = format!("{historical}{current_line}");
        let accepted =
            fresh_pd_line(current.as_bytes(), &cursor, "current").expect("fresh sealed line");
        assert_eq!(accepted, current_line.trim_end_matches('\n'));
        assert_eq!(
            fresh_pd_line(historical.as_bytes(), &cursor, "current")
                .expect_err("stale file cannot satisfy the current cycle"),
            "PD_RECEIPT_FRESH_APPEND_MISSING"
        );

        let duplicate = format!("{historical}{current_line}{current_line}");
        assert_eq!(
            fresh_pd_line(duplicate.as_bytes(), &cursor, "current")
                .expect_err("duplicate current-session receipts must fail"),
            "PD_RECEIPT_CURRENT_SESSION_DUPLICATE"
        );
    }

    #[test]
    fn pd_cursor_rejects_prefix_rewrite_before_the_fresh_receipt() {
        let historical = sealed_pd_fixture("historical");
        let cursor =
            PdHandoffCursor::from_bytes(historical.as_bytes()).expect("aligned historical prefix");
        let mut rewritten = historical.into_bytes();
        rewritten[0] ^= 1;
        rewritten.extend_from_slice(sealed_pd_fixture("current").as_bytes());
        assert_eq!(
            fresh_pd_line(&rewritten, &cursor, "current").expect_err("rewritten history must fail"),
            "PD_RECEIPT_PREFIX_DRIFT"
        );
    }

    #[test]
    fn legacy_handoff_is_written_synced_and_gates_dispatch() {
        let _guard = HANDOFF_ENV_LOCK.lock().expect("handoff test mutex");
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir()
            .join(format!("essmai-v168-light-{}-{unique}", std::process::id(),));
        std::fs::create_dir_all(&dir).expect("create test handoff directory");
        let previous = std::env::var_os("ESSMAI_HANDOFF_DIR");
        std::env::set_var("ESSMAI_HANDOFF_DIR", &dir);

        let coordinator = LightCoordinator::new();
        let mut response = coordinator.receive(light_coordinator::LightRequest::new(
            "Analizo motorin industrial të kuq dhe sensorin termik",
        ));
        assert!(response.status.is_ok(), "{}", response.status.label());
        assert!(response.ready_for_dispatch().is_err());

        let evidence = export_primitive_anchor(&response).expect("durable export");

        assert!(evidence.receipt_written);
        assert!(evidence.besa_written);
        assert!(evidence.primitive_anchor_written);
        assert!(evidence.commit_written);
        assert!(evidence.flushed);
        assert!(evidence.kodunik_verified);
        assert!(crate::lgc_algorithm::is_canonical_sha256(
            &evidence.besa_attestation_sha256
        ));
        assert!(crate::lgc_algorithm::is_canonical_sha256(
            &evidence.commit_sha256
        ));
        assert!(crate::lgc_algorithm::is_canonical_sha256(
            &evidence.primitive_anchor_sha256
        ));
        for file in [
            "light_lgc_receipt.txt",
            "light_lgc_handoff_commit.txt",
            "light_besa_export.txt",
            "light_pa_export.txt",
        ] {
            let bytes = std::fs::read(dir.join(file)).expect("handoff file");
            assert!(!bytes.is_empty(), "{file} must contain a sealed line");
        }

        response
            .complete_legacy_handoff(&evidence)
            .expect("complete evidence-bound handoff");
        response
            .ready_for_dispatch()
            .expect("dispatch gate opens after durable handoff");

        match previous {
            Some(value) => std::env::set_var("ESSMAI_HANDOFF_DIR", value),
            None => std::env::remove_var("ESSMAI_HANDOFF_DIR"),
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn mismatched_split_writes_nothing() {
        let _guard = HANDOFF_ENV_LOCK.lock().expect("handoff test mutex");
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "essmai-v168-light-mismatch-{}-{unique}",
            std::process::id(),
        ));
        std::fs::create_dir_all(&dir).expect("create test handoff directory");
        let previous = std::env::var_os("ESSMAI_HANDOFF_DIR");
        std::env::set_var("ESSMAI_HANDOFF_DIR", &dir);

        let coordinator = LightCoordinator::new();
        let mut response = coordinator.receive(light_coordinator::LightRequest::new(
            "Analizo motorin industrial të kuq dhe sensorin termik",
        ));
        response.primitive_split = crate::lab_contracts::PrimitiveSplit {
            xi: vec!["deformed".to_string()],
            yi: vec!["axis".to_string()],
        };
        assert!(export_primitive_anchor(&response).is_err());
        assert!(!dir.join("light_pa_export.txt").exists());
        assert!(!dir.join("light_lgc_receipt.txt").exists());
        assert!(!dir.join("light_lgc_handoff_commit.txt").exists());
        assert!(!dir.join("light_besa_export.txt").exists());

        match previous {
            Some(value) => std::env::set_var("ESSMAI_HANDOFF_DIR", value),
            None => std::env::remove_var("ESSMAI_HANDOFF_DIR"),
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}


fn release_quantum_process() {
    match crate::noi_caller::invoke("LIGHT_RELEASE_PROCESS", "light-runtime", || {
        quantum_bridge::buss_destroy();
        Ok(())
    }) {
        Ok(()) => {},
        Err(reason) => eprintln!("[LIGHT_RELEASE_PROCESS] {reason}"),
    }
}
