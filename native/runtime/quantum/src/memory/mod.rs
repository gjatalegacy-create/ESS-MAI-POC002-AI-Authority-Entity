// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  MEMORY/MOD.RS — Kontrata e Memorjes + Dija (Teorite 22, 23, 25, 26)  ║
// ║  GJATA LEGACY™ — Quantum Platform                                     ║
// ║                                                                          ║
// ║  RREGULLI: Quantum NUK mban memory persistente.                       ║
// ║   - boundary: ephemeral-only + gardian që refuzon persistent          ║
// ║   - raw_cognitive_trace: append-only, VETËM Shadow shkruan verdict     ║
// ║   - module_reputation: ephemeral, clear() me ciklin                   ║
// ║   - knowledge_lineage: konsulton refs nga Shadow, s'ruan vetë          ║
// ╚══════════════════════════════════════════════════════════════════════════╝

pub mod boundary;
pub mod knowledge_lineage;
pub mod module_reputation;
pub mod raw_cognitive_trace;

pub use boundary::{EphemeralStore, MemoryBoundary, MemoryScope, MemoryViolation, PersistentGuard};
pub use knowledge_lineage::{
    KnowledgeLineage, LineageContext, NegativeEntry, OptimalEntry, LINEAGE_BOOST, LINEAGE_PENALTY,
};
pub use module_reputation::{ModuleReputation, RepOutcome, ReputationRecord, ShadowResult};
pub use raw_cognitive_trace::{
    CognitiveTrace, EventType, Outcome, RawCognitiveTrace, ShadowVerdict, TraceStats,
};
