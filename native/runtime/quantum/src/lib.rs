#![warn(clippy::unwrap_used)] // HAPI 7.4: bën .unwrap() të dukshëm (path kritike → .expect/error handling)
                              // ╔══════════════════════════════════════════════════════════════════════════╗
                              // ║  LIB.RS — QUANTUM PLATFORM  (PERSOSMËRI)                          ║
                              // ║  GJATA LEGACY™ — AIGovern-Cyber Quantum Platform                             ║
                              // ║                                                                          ║
                              // ║  Pipeline:  Sovereign(0-copy) → LIM → HW_REAL → PRO → SRK → PIM       ║
                              // ║  Komunikimet:                                                         ║
                              // ║    bridge_light   ↔ Light    (QuantumInput/Output serialize)      ║
                              // ║    bridge_shadow  → Shadow   (QuantumInbound + vula 500)          ║
                              // ╚══════════════════════════════════════════════════════════════════════════╝

// ── Ligji 0-COPY Sovereign ───────────────────────────────────────────────
pub use shadow_contracts::boundary::gcl_project as gcl_project_contract; // R15: single source
pub mod hcp_pro; // LAYER 1: HCP_PRO controller aktiv orkestrimi (token mint/burn)
pub mod lab_contracts; // LAYER 2: gjuha e përbashkët (byte-for-byte identik në 3 platformat)
pub mod lab_contracts_v11; // LAYER 3: gjuha e zgjeruar (byte-for-byte identik në 3)
pub mod layer2; // LAYER 2: Deep Probe Lab (Pressure Engine + Abyssal Probe)
pub mod layer3; // LAYER 3: HPRO + HPIM + quantum_wisdom
pub use shadow_contracts::boundary::living_trust as living_trust_contract; // R15: single source
pub use shadow_contracts::boundary::pd_continuum as pd_continuum_contract; // R15: single source
pub use shadow_contracts::boundary::pd_spine as pd_spine_contract; // R15: single source
pub mod phase9_integration; // FAZA 9: lidh quantum_spine me rrjedhën (5 ligjet)
pub mod project_workspace_router;
pub mod quantum_spine; // PATCHIM: kurrizi vertikal (L3→L2→L3→L2)
pub mod sovereign; // v1.6.5: project-only storage/conversation orientation

// ── KUSHTETUTA + KONTROLLI (Teorite 52, 32, 35) ──────────────────────────
pub mod control;
pub mod governance; // recursion control, override detection
pub mod laws; // 5 ligjet sovereign // pressure states + score system

// ── ARSYETIMI (Teorite 27, 30, 31, 34) ───────────────────────────────────
pub mod reasoning; // elimination 3-nivel + semantic graph + convergence + territories
pub mod candidate_weight; // Phase 1B.1: deterministic candidate mass + negative parity
pub mod kie_measurement;
pub mod candidate_measurement;
pub mod request_bound_relevance; // Phase 1B.0: exact Xi/Yi-bound candidate projection

// ── MEMORIA + DIJA (Teorite 22, 23, 25, 26) ──────────────────────────────
// Quantum NUK mban memory persistente — kontrata mohuese + ephemeral.
pub mod memory;

// ── EKSPLORIMI (Teorite 7, 11, 13, 14) ────────────────────────────────────
pub mod exploration; // horizon manifold + frontier detector + state machine

// ── ORKESTRATORI (Lidhja e Plotë e Pipeline-it) ──────────────────────────
pub mod hardening;
pub mod orchestrator; // zemra që lidh të gjitha modulet sipas arkitekturës // patch përforcues: konsensus + kalibrim + konflikt + qëndrueshmëri

// ── LIM ──────────────────────────────────────────────────────────────────
pub mod lim;
pub mod lim_classifier;
pub mod lim_collector;
pub mod lim_measurer;
pub mod lim_types;
pub mod tokenizer;

// ── Hardware (adapter i simuluar — i ruajtur për krahasim) ───────────────
pub mod hardware;
pub mod lim_hw_bridge;

// ── Hardware REAL (lexon RAM/CPU/termik të vërtetë) ──────────────────────
pub mod hw_real;
pub mod lim_hw_real_bridge;

// ── PRO ──────────────────────────────────────────────────────────────────
pub mod pro;
pub mod pro_nk_gate;
pub mod pro_operator;
pub mod pro_types;
pub mod runtime_pulse; // PULSI REAL i organeve — presume lexon jetën, jo premtimin (v1.4.3)
pub mod token_forge; // BURIMI I DYTË i token-it — farka e pavarur (v1.4.3)

// ── NPRO + NPIM (Eliminimi Paralel + Negative Knowledge) ─────────────────
// NPRO: 4 operatorët MBRAPSHT → eliminimet + WeaknessSignal
// NPIM: paketon negative knowledge me argumenta për Shadow
pub mod apro; // APRO: argumentuesi (familja e 4-t e MPRO 16)
pub mod negative_asset; // v1.7.1: typed -Xi/-Yi binding + GCL SHA-256 living asset
pub mod npim;
pub mod npro;
pub mod npro_hardening;
pub mod npro_lim_bridge;
pub mod npro_operator;
pub mod ultimatum_collapse_law; // MUSKULI i Quantum — i ushqyer nga gjata_collapse_law // forcimi i NPRO/NPIM (konsensus+ceiling+severity)

// ── SRK ──────────────────────────────────────────────────────────────────
pub mod srk;
pub mod srk_ibe;
pub mod srk_types;

// ── PIM ──────────────────────────────────────────────────────────────────
pub mod lab; // Digital Lab — TRL pipeline për matjen e saktë (Teoria 44)
pub mod lab_integration;
pub mod pim; // Digital Lab i lidhur PERFEKT me sistemin (ligje+trace+VNK)

// ── PROGRESSIVE_DEBATIC (PD) ──────────────────────────────────────────────
// Shtresë gërmimi kognitiv e rikthyer nga      (pseudo → kod real ekzekutiv).
// Rrjedha: LIM → PD → LIM → PRO(+npro/apro…) → PIM(+npim…). Zero if/else.
pub mod progressive_debatic;
// NEW: explicit operational continuity; no constitutional verdict.
pub mod pd_operational;
pub mod pd_failure;
pub mod pd_iz_science; // PDS2 reuses existing PD and same Vault.
pub mod pd_iz; // NEW canonical operational inquiry, no epistemic verdict.
pub mod pd_prepared; // data-only native prepared checkpoint.

// ── URAT E KOMUNIKIMIT ────────────────────────────────────────────────────
pub mod bridge_light;
pub mod bridge_shadow; // Quantum → Shadow // Light  ↔ Quantum

// ════════════════════════════════════════════════════════════════════════
// RIEKSPORTE
// ════════════════════════════════════════════════════════════════════════

// Sovereign (ligji 0-copy)
pub use sovereign::{
    admit4, dot8, elim, gate, module_seal, CapHandle, EbpfPacket, Ev, LgcBridge, RingBuffer,
    RingConsumer, RingProducer, SovereignGate, SEAL_EBPF, SEAL_LIM, SEAL_PD, SEAL_PIM, SEAL_PRO,
    SEAL_SRK,
};

// Kushtetuta + Kontrolli
pub use control::{
    PressureConfig, PressureContext, PressureDetector, PressureState, ScoreFusion, ScoreVector,
    Scorer, ShadowState, Weights,
};
pub use governance::Governance;
pub use laws::{Destination, LawContext, RecursionKind, SovereignConstitution};

// Arsyetimi
pub use reasoning::{
    BestCandidate, Convergence, ElimCandidate, Elimination, EliminationMode, FinalOutput,
    ReasoningTerritories, Relation, SelectionMethod, SemanticGraph, Territory, Transfer,
};

// Memoria + Dija (Quantum s'mban memory persistente)
pub use memory::{
    CognitiveTrace, EventType, KnowledgeLineage, LineageContext, MemoryBoundary, MemoryScope,
    MemoryViolation, ModuleReputation, NegativeEntry, OptimalEntry, Outcome, RawCognitiveTrace,
    RepOutcome, ShadowResult, ShadowVerdict,
};

// Eksplorimi
pub use exploration::{
    AdvanceResult, CeilingBreach, CognitiveSignature, CycleResult, ExplorationHorizon,
    FrontierState, QuantumPhase, QuantumStateMachine, UnknownBoundaries,
};

// Orkestratori
pub use hardening::{
    ConfidenceCalibration, CrossOperatorConsensus, Hardening, HardeningReport, PronoConflict,
    StabilityMargin,
};
pub use orchestrator::{PipelineConfig, PipelineReport, QuantumOrchestrator};

// Digital Lab
pub use lab::{
    Concept, DigitalLab, FilterGate, Hypothesis, RejectionReason, SimulationEngine, Trl3Result,
    TrlEvidence, THRESHOLD_TRL3,
};
pub use lab_integration::{
    EpistemicTrlBridge, IntegratedLabResult, LabNegativeKnowledge, LabSystemBridge,
};

// LIM
pub use lim::LinearInfoMetricEngine;
pub use lim_types::{EpistemicScaffold, EpistemicState, LimError, LinearInfoMetric, Operator};

// Hardware
pub use hardware::ResourceSnapshot;
pub use hw_real::{kernel_hw_available, DepthHint as RealDepthHint};
pub use lim_hw_bridge::{HardwareScaffold, LimHwBridge};
pub use lim_hw_real_bridge::{LimHwRealBridge, RealHardwareScaffold};

// PRO
pub use pro::ProEngine;
pub use pro_nk_gate::KnowledgeVault;
pub use pro_types::{PROCandidate, ProResult};

// NPRO + NPIM
pub use npim::{
    NegativeArgument, NegativeKnowledgePackage, NegativePassMetric, NegativeSuggestion,
    NegativityProfile,
};
pub use npro::{NegativeElimination, NproEngine, NproResult, WeaknessSignal, WEAKNESS_CEILING};
pub use npro_hardening::{
    AdaptiveCeiling, EvidenceWeighting, NpimHardening, NproHardening, NproHardeningReport,
    SeverityEscalation, WeaknessConsensus,
};
pub use npro_lim_bridge::NproLimBridge;

// SRK
pub use srk::ScientificReasoningKernel;
pub use srk_types::ReasoningPackage;
pub use ultimatum_collapse_law::AshtOperatorDirective;

// PIM
pub use pim::{PassInfoMetric, PassPackage};

// Urat
pub use bridge_light::{
    LightBridgeError, LightQuantumBridge, OutputVerdict, QuantumInput, QuantumOutput,
};
pub use bridge_shadow::{
    LightInbound, NegativeInbound, NpimShadowBridge, QuantumInbound, QuantumShadowBridge,
    ShadowLightResponse, SupremeOutcome,
};

pub mod pd_failure_bridge;

pub mod pd_native_bridge;

// R3 same-Vault dialogue origin, restored only in Quantum.
pub mod pd_kie_origin;
