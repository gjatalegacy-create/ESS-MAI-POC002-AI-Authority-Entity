// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  EXPLORATION/MOD.RS — Eksplorimi (Teorite 7, 11, 13, 14)             ║
// ║  GJATA LEGACY™ — Quantum Platform                                     ║
// ║                                                                          ║
// ║   horizon: manifold deformueshëm + CognitiveSignature                 ║
// ║   frontier: Epistemic Frontier Detector me tavane absolute            ║
// ║   state_machine: QuantumPhase IDLE→...→TRANSFER                       ║
// ╚══════════════════════════════════════════════════════════════════════════╝

pub mod frontier;
pub mod horizon;
pub mod state_machine;

pub use frontier::{
    BoundaryScan, CeilingBreach, CycleResult, FrontierAxes, FrontierState, UnknownBoundaries,
    CONTRADICTION_CEILING, ENTROPY_CEILING, RECURSION_CEILING, SEMANTIC_DRIFT_CEILING,
};
pub use horizon::{CognitiveSignature, ExplorationHorizon, HorizonAxes, HorizonMultipliers};
pub use state_machine::{AdvanceResult, QuantumPhase, QuantumStateMachine};
