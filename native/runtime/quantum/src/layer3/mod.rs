// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  LAYER3/MOD.RS — Quantum Layer 3 (Hardware Pressure + Wisdom)       ║
// ║  GJATA LEGACY™ — AIGovern-Cyber Quantum                                       ║
// ║                                                                          ║
// ║  Shtresa e tretë e Quantum-it: presioni shtrihet mbi harduerin.        ║
// ║   • HPRO  → prod_formula + control_role mbi harduer; ushqen presionin  ║
// ║   • HPIM  → paketon trace-in e HPRO për PIM (presioni s'futet te PIM)   ║
// ║   • quantum_wisdom → observon si u gjet primitiv/legacy                ║
// ║                                                                          ║
// ║  HPRO lëshon harduerin kur mbaron. Zero if/else.                      ║
// ╚══════════════════════════════════════════════════════════════════════════╝

pub mod hcp_pro_l3; // HCP_PRO aktiv paralel (ngre të gjitha familjet njëheresh)
pub mod hpim; // Hardware Pass Info Metric (paketon për PIM)
pub mod hpro; // Hardware Primitive Reasoning Origin
pub mod layer3_flow; // orkestruesi i Shtresës 3 (HPRO→HPIM + wisdom packets)
pub mod mpro;
pub mod quantum_wisdom; // observon zbulimin e primitiv/legacy // MPRO mates determinist → Score Vector → TrustContext

// ── Ri-eksportim ────────────────────────────────────────────────────────────
pub use hcp_pro_l3::{FamilySignals, HcpProL3, ParallelActivation, ProFamily};
pub use hpim::{HpimEngine, HpimPackage, HproTraceStep};
pub use hpro::{
    ControlRole, HproEngine, HproResult, ParamPriority, ProdFormula, StabilityPlan,
    HPRO_COST_PER_OP, HPRO_ENERGY_FLOOR, HPRO_RAM_CEILING, HPRO_THERMAL_FLOOR,
};
pub use layer3_flow::{QuantumLayer3Flow, QuantumLayer3Result};
pub use mpro::{MeasurementSet, Mpro, ScoreVector};
pub use quantum_wisdom::QuantumWisdom;
