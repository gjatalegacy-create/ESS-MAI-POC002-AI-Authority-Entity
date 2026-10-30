// Hardware bridge për Quantum Platform.
// Ura LIM ↔ hardware: epistemic_mass → DepthHint → RAM budget → paralelizëm PRO.

pub mod hw_adapter;

pub use hw_adapter::{
    ActParallel, DepthHint, HwBudget, ParallelDecision, ResourceSnapshot, ScaleHarduer,
};
