// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  scale_harduer.rs   — Hapi 10                                          ║
// ║  act_parallel.rs    — Hapi 11                                          ║
// ║  quantum_softhard.rs — Hapi 12                                         ║
// ║  GJATA LEGACY™— Quantum Platform                                      ║
// ╚══════════════════════════════════════════════════════════════════════════╝

use crate::hw_real::ffi::{
    hw_lgc_open, HwLgcError, S_ACT_PARALLEL, S_QUANTUM_SOFTHARD, S_SCALE_HARDUER,
};
use crate::hw_real::territories::{DepthHint, PressureEnvelope};

// ════════════════════════════════════════════════════════════════════════════
// SCALE HARDUER TERRITORY — Hapi 10
//
// Komunikon me LIM: merr depth_hint → vendos % të hapësirës
// Formula:
//   depth_budget = available_bytes × DEPTH_TO_PCT[depth_level]
//   nëse aplikacion tjetër ka nevojë → lëshon exces dinamikisht
//
// DEPTH_TO_PCT:
//   Shallow  → 0.00 (nuk aktivizohet)
//   Medium   → 0.50 (50% e locked)
//   Deep     → 0.80 (80% e locked)
//   VeryDeep → 1.00 (100% e locked)
// ════════════════════════════════════════════════════════════════════════════

const DEPTH_TO_PCT: [f32; 4] = [0.0, 0.50, 0.80, 1.00];

fn depth_to_idx(hint: DepthHint) -> usize {
    match hint {
        DepthHint::Shallow => 0,
        DepthHint::Medium => 1,
        DepthHint::Deep => 2,
        DepthHint::VeryDeep => 3,
    }
}

/// Hardware budget i alokuar për reasoning
#[derive(Debug, Clone)]
pub struct HwBudget {
    /// bytes alokuar për reasoning ∈ [0, available_bytes]
    pub allocated_bytes: u64,
    /// % e hapësirës locked të përdorur ∈ [0.0, 1.0]
    pub pct_used: f32,
    /// thellësia e arsyetimit
    pub depth_hint: DepthHint,
    /// bytes totale të disponueshme
    pub available_total: u64,
}

impl HwBudget {
    /// Invariant: allocated_bytes ≤ available_total
    pub fn is_valid(&self) -> bool {
        self.allocated_bytes <= self.available_total && self.pct_used >= 0.0 && self.pct_used <= 1.0
    }
}

pub struct ScaleHarduer;

impl ScaleHarduer {
    /// Vendos saktësisht sa % të hapësirës do të përdoret
    /// bazuar në thellësinë e kërkuar nga LIM.
    pub fn allocate(
        envelope: &PressureEnvelope,
        depth_hint: DepthHint,
    ) -> Result<HwBudget, HwLgcError> {
        let tok = hw_lgc_open(S_SCALE_HARDUER, S_SCALE_HARDUER)?;
        let _ = tok;

        let available = envelope.available_bytes();
        let idx = depth_to_idx(depth_hint);
        let pct = DEPTH_TO_PCT[idx];

        // Formula: allocated = available × pct
        let raw_alloc = (available as f64 * pct as f64) as u64;

        // ══ LIGJI I MENAXHERIT (R E A L control, i Arkitektit): GJITHMONË
        // ≥10% e hapësirës mbetet E LIRË. Kërkesa mbi kufi SHKURTOHET me
        // fleksibilitet (kurrë refuzim/bllokim). Total-i i plotë s'mbërrin
        // deri këtu (envelope = pas-reserve) → dyshemeja zbatohet mbi
        // available (konservativ; qeverisja e plotë total-aware = HwManager).
        let lease = crate::hw_real::hw_manager::HwManager::govern(
            0, available, raw_alloc, /*cores*/ 1, /*par*/ 1,
        );
        let allocated = lease.granted_bytes;
        match lease.flexed {
            true => println!(
                "  [HW_MGR]   {} (fleksibilitet — dyshemeja 10% u ruajt)",
                lease.report()
            ),
            false => {}
        }

        // available==0 → 0; ndryshe → raporti. Branchless.
        let avail_nonzero = (available != 0) as u32 as f32;
        let pct_actual = avail_nonzero * (allocated as f32 / available.max(1) as f32);

        let budget = HwBudget {
            allocated_bytes: allocated,
            pct_used: pct_actual.clamp(0.0, 1.0),
            depth_hint,
            available_total: available,
        };

        debug_assert!(
            budget.is_valid(),
            "HwBudget invariant violated: allocated={} total={}",
            budget.allocated_bytes,
            budget.available_total
        );

        Ok(budget)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// ACT PARALLEL ARGUMENTS — Hapi 11
//
// Pyet LIM sa thellë do të shkojë reasoning.
// LIM kthen DepthHint.
// Act_Parallel vendos sa nga 4 operatorët e PRO ekzekutohen paralel.
//
// Formula:
//   cost_per_op_bytes = COST_PER_OPERATOR_MB × 1024²
//   n_parallel        = min(floor(allocated / cost_per_op), 4); zero means no grant
//
// PRO operatorët: REVERSE, MIRROR, CONTRAST, TRACE
// ════════════════════════════════════════════════════════════════════════════

/// Kostoja e memories për operator (MB)
/// Estimate bazë — kalibrohet me profiling real
const COST_PER_OPERATOR_MB: u64 = 64; // 64MB për operator

/// 4 operatorët e PRO — identikë me      ekzistues
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProOp {
    Reverse,  // Pearl do-calculus
    Mirror,   // Gentner structure-mapping
    Contrast, // Hegel dialectic
    Trace,    // Peirce abduction
}

impl ProOp {
    pub const ALL: [ProOp; 4] = [ProOp::Reverse, ProOp::Mirror, ProOp::Contrast, ProOp::Trace];

    pub fn name(self) -> &'static str {
        match self {
            ProOp::Reverse => "REVERSE",
            ProOp::Mirror => "MIRROR",
            ProOp::Contrast => "CONTRAST",
            ProOp::Trace => "TRACE",
        }
    }

    /// Pesha e operatorit — identike me     
    pub fn weight(self) -> f32 {
        match self {
            ProOp::Reverse => 1.00,
            ProOp::Mirror => 0.95,
            ProOp::Contrast => 0.90,
            ProOp::Trace => 0.92,
        }
    }
}

/// Vendimi i paralelizmit për PRO
#[derive(Debug, Clone)]
pub struct ParallelDecision {
    /// numri i operatorëve paralel ∈ [1, 4]
    pub n_parallel: u8,
    /// operatorët që do të ekzekutohen
    pub operators: Vec<ProOp>,
    /// bytes të alokuara për këtë vendim
    pub budget_used: u64,
    /// nëse e gjitha kapaciteti është e nevojshme
    pub full_parallel: bool,
}

impl ParallelDecision {
    /// Exact operator count and cost; the caller's budget is checked separately.
    pub fn is_valid(&self) -> bool {
        self.n_parallel >= 1
            && self.n_parallel <= 4
            && self.operators.len() == self.n_parallel as usize
            && self.budget_used == u64::from(self.n_parallel) * COST_PER_OPERATOR_MB * 1024 * 1024
            && self.full_parallel == (self.n_parallel == 4)
    }

    pub fn is_valid_for(&self, budget: &HwBudget) -> bool {
        self.is_valid() && budget.is_valid() && self.budget_used <= budget.allocated_bytes
    }
}

pub struct ActParallel;

impl ActParallel {
    /// Vendos paralelizmin e PRO bazuar në budget + depth_hint nga LIM.
    pub fn decide(budget: &HwBudget) -> Result<ParallelDecision, HwLgcError> {
        let tok = hw_lgc_open(S_ACT_PARALLEL, S_ACT_PARALLEL)?;
        let _ = tok;

        let cost_per_op = COST_PER_OPERATOR_MB * 1024 * 1024;

        // n*C <= B, with n <= 4. Insufficient memory cannot fund one operator.
        let n_parallel = (budget.allocated_bytes / cost_per_op).min(4) as u8;
        match budget.is_valid() && n_parallel != 0 {
            true => {}
            false => return Err(HwLgcError::InvariantViolation("PARALLEL_BUDGET_INSUFFICIENT_OR_INVALID")),
        }

        // Zgjidh operatorët me prioritet (me peshë më të lartë fillimisht)
        // REVERSE(1.00) → MIRROR(0.95) → TRACE(0.92) → CONTRAST(0.90)
        let priority_order = [ProOp::Reverse, ProOp::Mirror, ProOp::Trace, ProOp::Contrast];

        let operators: Vec<ProOp> = priority_order
            .iter()
            .take(n_parallel as usize)
            .copied()
            .collect();

        let decision = ParallelDecision {
            n_parallel,
            budget_used: n_parallel as u64 * cost_per_op,
            full_parallel: n_parallel == 4,
            operators,
        };

        debug_assert!(
            decision.is_valid_for(budget),
            "ParallelDecision invariant violated: n={}",
            decision.n_parallel
        );

        Ok(decision)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// QUANTUM SOFTHARD — Hapi 12
//
// Kontraktuesi final midis softuerit dhe harduerit.
// Krijon ResourceContract dhe jep GO/HOLD/LOW_PAS sinjalin te PRO.
//
// Pse te PRO? Sepse LIM ka mbaruar (ka vendosur kufijtë epistemikë).
// PRO është ku 4 operatorët ndajnë:
//   - dija pozitive (Zone::Optimal → +100)
//   - dija negative (Zone::Negative → -100)
//   - hipotezat e forta (kandidatë potencialë)
//   - hipotezat e dobëta (potencialë me mundësi të ulët)
//
// GO signal → PRO aktivizohet me ResourceContract
// ════════════════════════════════════════════════════════════════════════════

/// Sinjali final i kontratës
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ContractSignal {
    /// PRO aktivizohet me budget të plotë
    Go,
    /// Prit — sistem nën presion kritik
    Hold,
    /// PRO aktivizohet me kapacitet të reduktuar (Pressure::Overload)
    LowPas,
}

impl ContractSignal {
    pub fn label(self) -> &'static str {
        match self {
            ContractSignal::Go => "GO",
            ContractSignal::Hold => "HOLD",
            ContractSignal::LowPas => "LOW_PAS",
        }
    }
}

/// ResourceContract — kontrata finale software ↔ hardware
/// Kalon nga Quantum_Softhard te PRO si parametër
#[derive(Debug, Clone)]
pub struct ResourceContract {
    /// sinjali kryesor
    pub signal: ContractSignal,
    /// budget hardware i alokuar
    pub budget: HwBudget,
    /// vendimi i paralelizmit
    pub parallel: ParallelDecision,
    /// LGC seal i kontratës — compile-time
    pub seal: u64,
    /// timestamp ephemeral
    pub ts_ns: u64,
    /// nëse hardware sysfs është i disponueshëm
    pub hw_real: bool,
}

impl ResourceContract {
    pub fn is_go(&self) -> bool {
        matches!(self.signal, ContractSignal::Go)
    }

    pub fn n_operators(&self) -> u8 {
        self.parallel.n_parallel
    }

    pub fn operators(&self) -> &[ProOp] {
        &self.parallel.operators
    }
}

/// Kriteret për ContractSignal
/// GO:     energy_margin > 0.20 AND allocated > min_threshold AND !under_pressure_critical
/// HOLD:   energy_margin < 0.05 OR sistem kritik
/// LOW_PAS: gjendje ndërmjetëse
fn evaluate_signal(budget: &HwBudget, envelope: &PressureEnvelope, energy: f32) -> ContractSignal {
    // Prioritet i ruajtur: HOLD para LOW_PAS para GO. Zero if — match guards.
    // HOLD: energji kritike ose asgjë e alokuar.
    match energy < 0.05 || budget.allocated_bytes == 0 {
        true => return ContractSignal::Hold,
        false => {}
    }

    // LOW_PAS: presion i lartë ose energji e ulët ose pak memorie.
    let pct_available = budget.pct_used;
    match envelope.under_pressure || energy < 0.20 || pct_available < 0.25 {
        true => return ContractSignal::LowPas,
        false => {}
    }

    // GO: gjithçka OK
    ContractSignal::Go
}

pub struct QuantumSofthard;

impl QuantumSofthard {
    /// Krijon ResourceContract — kontrata finale.
    /// Thirret nga Orchestrator, output kalon direkt te PRO.
    pub fn evaluate(
        budget: HwBudget,
        parallel: ParallelDecision,
        envelope: &PressureEnvelope,
        energy: f32,
        hw_real: bool,
    ) -> Result<ResourceContract, HwLgcError> {
        let tok = hw_lgc_open(S_QUANTUM_SOFTHARD, S_QUANTUM_SOFTHARD)?;
        let _ = tok;

        match parallel.is_valid_for(&budget) {
            true => {}
            false => return Err(HwLgcError::InvariantViolation("RESOURCE_CONTRACT_PARALLEL_BUDGET_INVALID")),
        }
        let signal = evaluate_signal(&budget, envelope, energy);

        // seal i kontratës — XOR budget+parallel+signal
        let seal = S_QUANTUM_SOFTHARD
            ^ (budget.allocated_bytes.wrapping_mul(31))
            ^ (signal as u64 * 0xDEAD_BEEF_CAFE);

        Ok(ResourceContract {
            signal,
            budget,
            parallel,
            seal,
            ts_ns: 0, // Orchestrator do ta vendosë
            hw_real,
        })
    }
}


#[cfg(test)]
mod audit_hw_boundary_tests {
    use super::*;
    const C: u64 = COST_PER_OPERATOR_MB * 1024 * 1024;

    fn budget(bytes: u64) -> HwBudget {
        HwBudget { allocated_bytes: bytes, pct_used: 1.0,
            depth_hint: DepthHint::Deep, available_total: bytes }
    }

    fn envelope(bytes: u64) -> PressureEnvelope {
        PressureEnvelope { locked_bytes: bytes, released_bytes: 0, total_reserved: bytes,
            demand_bytes: bytes, under_pressure: false, depth_hint: DepthHint::Deep, ts_ns: 1 }
    }

    #[test]
    fn operator_budget_cannot_grant_an_unfunded_operator() {
        for bytes in [0, C / 2, C - 1] {
            assert!(ActParallel::decide(&budget(bytes)).is_err(), "allocated={bytes}");
        }
    }

    #[test]
    fn operator_budget_valid_boundaries_keep_exact_cost_and_order() {
        let order = [ProOp::Reverse, ProOp::Mirror, ProOp::Trace, ProOp::Contrast];
        for bytes in [C, C + 1, 2 * C - 1, 2 * C, 3 * C, 4 * C, 10 * C, u64::MAX] {
            let b = budget(bytes);
            let decision = ActParallel::decide(&b).expect("funded operators");
            let n = (bytes / C).min(4) as usize;
            assert_eq!(decision.n_parallel as usize, n);
            assert_eq!(decision.operators, order[..n]);
            assert_eq!(decision.budget_used, n as u64 * C);
            assert!(decision.budget_used <= b.allocated_bytes);
            assert_eq!(decision.full_parallel, n == 4);
            assert!(decision.is_valid());
        }
    }

    #[test]
    fn operator_budget_final_contract_rejects_overcommit() {
        let decision = ActParallel::decide(&budget(2 * C)).unwrap();
        assert!(QuantumSofthard::evaluate(budget(C), decision, &envelope(C), 1.0, true).is_err());
    }

    #[test]
    fn operator_budget_rejects_tampered_cost_and_invalid_budget() {
        let mut decision = ActParallel::decide(&budget(C)).unwrap();
        decision.budget_used = 0;
        assert!(!decision.is_valid());
        assert!(QuantumSofthard::evaluate(budget(C), decision, &envelope(C), 1.0, true).is_err());
        let mut invalid = budget(C);
        invalid.available_total = C - 1;
        assert!(ActParallel::decide(&invalid).is_err());
    }
}
