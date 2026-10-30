// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  PROGRESSIVE_DEBATIC / mod.rs                                            ║
// ║  GJATA LEGACY™ — AIGovern-Cyber Quantum                                         ║
// ║                                                                          ║
// ║  Shtresë gërmimi kognitiv (PD): kod real ekzekutiv në default mode, e    ║
// ║  përshtatur me ligjet sovrane (ZERO if/else, pa varësi të jashtme, fail- ║
// ║  closed, id-të me FNV-1a të sistemit).                                    ║
// ║                                                                          ║
// ║  Rrjedha: LIM → PD → LIM → ... (PD ushqen LIM-in e dytë; vazhdon nëpër   ║
// ║  PRO/SRK/PIM si është). PD prodhon EpistemicTrace + GeniusSignal +        ║
// ║  PdCognitivePackage; fushat relevante përkthehen për UI nga pd_light      ║
// ║  (shtresë gjuhësore në Light). PD NUK ka rrjedhë direkte te PIM.          ║
// ║                                                                          ║
// ║  Nën-modulet:                                                            ║
// ║    types           — kontratat prepared/pending/verified/next-i₀         ║
// ║    genius_detector — detektori i 3 sinjaleve (returns/energy/shape)      ║
// ║    core            — vetëm motori kognitiv (ingest/trace/output)         ║
// ║    seal            — vetëm autoriteti SEAL_PD + LgcToken privat          ║
// ║    runtime         — PD→Spine9→L1/2/3→receipt→output+iZ→next i₀         ║
// ╚══════════════════════════════════════════════════════════════════════════╝

// V189_DERIVATIVE: operational PD state; no authority tokens.
pub mod checkpoint;
pub mod core;
pub mod genius_detector;
pub mod runtime;
pub mod seal;
pub mod types;

// Re-eksporte për konsumatorin (pipeline / main).
pub use self::core::{build_package, PdSession, ProgressiveDebatic};
pub use genius_detector::GeniusDetector;
pub use types::{
    pd_id, CognitiveSignal, DebateMode, EpistemicTrace, GeniusSignal, PdCognitivePackage,
    PdContinuationBasis, PdContinuumClosure, PdEngineOutput, PdError, PdFinalization,
    PdIzCompletion, PdNextI0, PdPendingNextI0, PdPreClassification, PdPreSealReceipt,
    PdPreparedCycle, PdResponseType, PdSpineCycle, PdTurn, PdVerificationCompletion,
    PdVerifiedOutput, PressureBudget, EXPECTED_SHADOW_VERIFICATION_SEAL, PD_SEAL,
};
