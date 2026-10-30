// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  GCL_PRESUME — GATISHMËRIA NËN LIGJ (AIGovern-Cyber Light, nënndarje e përdorur)         ║
// ║  GJATA LEGACY™ | Arkitekt: Bledar Gjata                                  ║
// ║                                                                          ║
// ║  VENDI NË HIERARKI: gjata_collapse_law është KOMANDANTI — state machine  ║
// ║  horizontale që i komandon tre makinat e platformave (Light: Coordination║
// ║  → i₀→PA→split; Quantum: Reasoning → PRO/NPRO/MPRO/APRO→PIM/NPIM;        ║
// ║  Shadow: Verification → ankora+XY→Y→X). GCL_PRESUME VARET prej tij:      ║
// ║  merr CollapsePhase si urdhër dhe mban NË GATISHMËRI edhe modulet që     ║
// ║  s'kanë impakt në rrjedhën e kolapsit — ping→përgjigje, zero arsyetim.   ║
// ║                                                                          ║
// ║  NDËRFAQJA (siç e ka caktuar Arkitekti): çdo modul — edhe anësor —       ║
// ║  përgjigjet me gcl_role() + gcl_status(). Regjistrimi bëhet me fn        ║
// ║  pointer të pastër (zero gjendje, zero alokim të fshehtë).               ║
// ║                                                                          ║
// ║  LIGJET: zero varësi të jashtme · zero if/else (vetëm match) ·           ║
// ║  KURRË bllokues (sweep-i informon; NotReady i domosdoshëm → shënim       ║
// ║  në rrjedha me PSE, runtime vazhdon) · regjistri i kufizuar (idempotent).║
// ╚══════════════════════════════════════════════════════════════════════════╝

use super::collapse::CollapsePhase;
use std::sync::{Mutex, OnceLock};

/// Roli i një moduli ndaj ligjit GCL — fjalori i formulës, i gdhendur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GclRole {
    // ── Light (Coordination) ────────────────────────────────────────────────
    /// i₀ — kapja e indicies primitive (lingua/gjurma).
    I0Capture,
    /// i₀ → Xi + Yi — ndarja kolaps në dy hapësira.
    SplitXiYi,
    /// Eksporti i ankorës së vulosur drejt urës (pa_wire, shkruesi).
    AnchorExport,
    // ── Quantum (Reasoning) ─────────────────────────────────────────────────
    // ── Shadow (Verification) ───────────────────────────────────────────────
    // ── Anësorët ────────────────────────────────────────────────────────────
    /// Modul pa impakt në rrjedhën e kolapsit — por I GATSHËM të përgjigjet.
    Support(&'static str),
}

impl GclRole {
    pub fn code(&self) -> &'static str {
        match self {
            GclRole::I0Capture => "I0_CAPTURE",
            GclRole::SplitXiYi => "SPLIT_XI_YI",
            GclRole::AnchorExport => "ANCHOR_EXPORT",
            GclRole::Support(_) => "SUPPORT",
        }
    }

    /// Matrica e domosdoshmërisë: cilat role DUHET të jenë Ready në cilën
    /// fazë të komandantit. Support s'është i domosdoshëm KURRË — vetëm
    /// i gatshëm të përgjigjet (kjo është pikërisht detyra e presume-it).
    pub fn required_in(&self, phase: CollapsePhase) -> bool {
        match (phase, self) {
            (
                CollapsePhase::Coordination,
                GclRole::I0Capture | GclRole::SplitXiYi | GclRole::AnchorExport,
            ) => true,
            _ => false,
        }
    }
}

/// Përgjigjja e ping-ut — gjendja e modulit ndaj ligjit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GclReadiness {
    Ready,
    Degraded(&'static str),
    NotReady(&'static str),
}

impl GclReadiness {
    pub fn code(&self) -> &'static str {
        match self {
            GclReadiness::Ready => "READY",
            GclReadiness::Degraded(_) => "DEGRADED",
            GclReadiness::NotReady(_) => "NOT_READY",
        }
    }
    pub fn is_ready(&self) -> bool {
        match self {
            GclReadiness::Ready => true,
            _ => false,
        }
    }
}

/// Ping-u është fn pointer i pastër — zero gjendje, zero arsyetim në modul.
pub type StatusFn = fn() -> GclReadiness;

struct Entry {
    name: &'static str,
    role: GclRole,
    status: StatusFn,
}

fn registry() -> &'static Mutex<Vec<Entry>> {
    static R: OnceLock<Mutex<Vec<Entry>>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(Vec::new()))
}

/// Regjistrim IDEMPOTENT sipas emrit (regjistri i kufizuar — s'rritet me
/// thirrje të përsëritura; kyçi i helmuar rikuperohet me ligjin e njëjtë).
pub fn register(name: &'static str, role: GclRole, status: StatusFn) {
    let mut guard = match registry().lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    match guard.iter().any(|e| e.name == name) {
        true => {} // tashmë i regjistruar — heshtje e qëllimshme (idempotent)
        false => guard.push(Entry { name, role, status }),
    }
}

/// Rreshti i një moduli në sweep — për shtypje, teste dhe raporte.
#[derive(Debug, Clone, Copy)]
pub struct SweepLine {
    pub role: GclRole,
    pub readiness: GclReadiness,
    pub required: bool,
}

/// Statistika e sweep-it — kthehet te thirrësi, KURRË s'urdhëron.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SweepStats {
    pub total: usize,
    pub ready: usize,
    pub degraded: usize,
    pub required_not_ready: usize,
}

/// SWEEP-i i gatishmërisë nën urdhrin e komandantit (CollapsePhase):
/// ping-on ÇDO modul të regjistruar, shtyp gjendjen, dhe për rolet e
/// DOMOSDOSHME të fazës që s'janë Ready → shënim në rrjedha me PSE
/// (klasa Other — gatishmëria s'është korrupsion transporti). Best-effort,
/// zero bllokim: verdikti i ekzekutimit mbetet i platformës.
pub fn sweep(
    platform: &'static str,
    phase: CollapsePhase,
    rrjedha_dir: &str,
) -> (SweepStats, Vec<SweepLine>) {
    let guard = match registry().lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };
    let mut lines: Vec<SweepLine> = Vec::with_capacity(guard.len());
    let mut stats = SweepStats {
        total: 0,
        ready: 0,
        degraded: 0,
        required_not_ready: 0,
    };
    println!(
        "  [GCL_PRESUME] {} — {} : sweep i gatishmërisë ({} module)",
        platform,
        phase.label(),
        guard.len()
    );
    for e in guard.iter() {
        let readiness = (e.status)();
        let required = e.role.required_in(phase);
        stats.total += 1;
        match readiness {
            GclReadiness::Ready => {
                stats.ready += 1;
            }
            GclReadiness::Degraded(_) => {
                stats.degraded += 1;
            }
            GclReadiness::NotReady(_) => {}
        }
        let flamuri = match required {
            true => "†",
            false => " ",
        };
        println!(
            "  [GCL_PRESUME]   {}{} ({}) → {}",
            flamuri,
            e.name,
            e.role.code(),
            readiness.code()
        );
        match (required, readiness.is_ready()) {
            (true, false) => {
                stats.required_not_ready += 1;
                let arsyeja = match readiness {
                    GclReadiness::Degraded(a) | GclReadiness::NotReady(a) => a,
                    GclReadiness::Ready => "", // e paarritshme në këtë degë
                };
                let _ = super::rrjedha::note(
                    rrjedha_dir,
                    platform,
                    "gcl_presume::sweep",
                    super::rrjedha::RrjedhaClass::Other,
                    &format!(
                        "rol i domosdoshëm JO gati: {} ({}) në {} — {}",
                        e.name,
                        e.role.code(),
                        phase.label(),
                        arsyeja
                    ),
                );
                eprintln!(
                    "  [GCL_PRESUME] ALARM: {} ({}) i domosdoshëm për {} → {}",
                    e.name,
                    e.role.code(),
                    phase.label(),
                    readiness.code()
                );
            }
            _ => {}
        }
        lines.push(SweepLine {
            role: e.role,
            readiness,
            required,
        });
    }
    println!(
        "  [GCL_PRESUME] {} — përfundim: {}/{} READY, {} DEGRADED, {} të domosdoshëm JO gati",
        platform, stats.ready, stats.total, stats.degraded, stats.required_not_ready
    );
    (stats, lines)
}

// ════════════════════════════════════════════════════════════════════════════
// TESTE — kontrata provohet në SECILËN platformë (nënndarja e përdorur në AIGovern-Cyber Light).
// KUJDES: regjistri është global në binarin e testeve — emrat unikë,
// pohimet kërkojnë rreshtin TONË, kurrë totale absolute të brishta.
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod gcl_presume_tests {
    use super::*;

    fn gati() -> GclReadiness {
        GclReadiness::Ready
    }
    fn jo_gati() -> GclReadiness {
        GclReadiness::NotReady("test_presume_anchor_not_ready_unik")
    }
    fn tdir() -> String {
        std::env::temp_dir().to_string_lossy().into_owned()
    }
    fn registered_name_count(name: &str) -> usize {
        let guard = registry()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.iter().filter(|entry| entry.name == name).count()
    }

    #[test]
    fn role_codes_are_stable_and_greppable() {
        assert_eq!(GclRole::I0Capture.code(), "I0_CAPTURE");
        assert_eq!(GclRole::SplitXiYi.code(), "SPLIT_XI_YI");
        assert_eq!(GclRole::AnchorExport.code(), "ANCHOR_EXPORT");
        assert_eq!(GclRole::Support("x").code(), "SUPPORT");
    }

    #[test]
    fn required_matrix_follows_the_law() {
        use CollapsePhase::*;
        // Each retained Light role belongs only to Coordination.
        for role in [
            GclRole::I0Capture,
            GclRole::SplitXiYi,
            GclRole::AnchorExport,
        ] {
            assert!(role.required_in(Coordination));
            assert!(!role.required_in(Reasoning));
            assert!(!role.required_in(Verification));
        }
    }

    #[test]
    fn support_is_never_required_but_always_pingable() {
        use CollapsePhase::*;
        let s = GclRole::Support("test_presume_support_unik");
        assert!(!s.required_in(Coordination));
        assert!(!s.required_in(Reasoning));
        assert!(!s.required_in(Verification));
        register("test_presume_support_unik", s, gati);
        let (_, lines) = sweep("TEST", Reasoning, &tdir());
        let yni = lines
            .iter()
            .find(|line| line.role == s)
            .expect("registered support must answer the sweep");
        assert!(yni.readiness.is_ready());
        assert!(!yni.required);
    }

    #[test]
    fn register_is_idempotent_by_name() {
        const NAME: &str = "test_presume_idem_unik";
        register(NAME, GclRole::Support("a"), gati);
        assert_eq!(registered_name_count(NAME), 1);
        register(NAME, GclRole::Support("a"), gati);
        assert_eq!(registered_name_count(NAME), 1);
    }

    #[test]
    fn required_not_ready_is_flagged_and_noted() {
        register("test_presume_anchor_unik", GclRole::AnchorExport, jo_gati);
        let (stats, lines) = sweep("TEST", CollapsePhase::Coordination, &tdir());
        let yni = lines
            .iter()
            .find(|line| line.role == GclRole::AnchorExport && line.readiness == jo_gati())
            .expect("registered anchor must answer the sweep");
        assert!(yni.required, "anchor export is required in Coordination");
        assert!(!yni.readiness.is_ready());
        assert!(stats.required_not_ready >= 1);
        let (_, other_phase) = sweep("TEST", CollapsePhase::Reasoning, &tdir());
        let yni2 = other_phase
            .iter()
            .find(|line| line.role == GclRole::AnchorExport && line.readiness == jo_gati())
            .expect("registered anchor remains observable in another phase");
        assert!(!yni2.required);
    }
}
