// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  LAB/MOD.RS — Digital Lab (Teoria 44)                                ║
// ║  GJATA LEGACY™ — Quantum Platform                                     ║
// ║                                                                          ║
// ║  Matja e saktë përmes TRL Pipeline 3-fazor.                          ║
// ║  Lab MAT → prodhon TrlEvidence → PIM përdor për potentiality.          ║
// ╚══════════════════════════════════════════════════════════════════════════╝

pub mod digital_lab;
pub mod lab_types;

pub use digital_lab::{DigitalLab, FilterGate, ResultEvaluator, SimulationEngine, THRESHOLD_TRL3};
pub use lab_types::{
    lab_hash, Concept, Hypothesis, LabError, RejectionReason, SimMetrics, SimOutcome,
    SimulationResult, Trl3Result, TrlEvidence, TrlStatus,
};
