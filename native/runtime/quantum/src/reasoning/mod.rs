// Reasoning — elimination + semantic graph + convergence + territories.
// Teorite 27, 30, 31, 34.
pub mod convergence;
pub mod elimination;
pub mod semantic_graph;
pub mod territories;

pub use convergence::{BestCandidate, Convergence, FinalOutput, SelectionMethod, Transfer};
pub use elimination::{
    ElimCandidate, Elimination, THRESHOLD_GLOBAL, THRESHOLD_LOCAL, THRESHOLD_REGIONAL,
};
pub use semantic_graph::{Relation, SemanticEdge, SemanticGraph, SemanticNode};
pub use territories::{ContradictionPolicy, EliminationMode, ReasoningTerritories, Territory};
