// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  SOVEREIGN/MOD.RS — Ligji 0-COPY Sovereign (Light Platform)           ║
// ║  GJATA LEGACY™ — Light Platform                                   ║
// ║                                                                          ║
// ║  Ligjet matematike + capability tokens + ring buffer 0-copy.          ║
// ║  IDENTIK me Quantum sovereign — vula 500 ndër platforma.              ║
// ║  Light përdor sovereign për të mbrojtur kalimet FFI (light_buss,      ║
// ║  buss_legacy) me CapHandle një-përdorimësh.                          ║
// ╚══════════════════════════════════════════════════════════════════════════╝

pub mod laws;
pub mod lgc_gate;

// Ligjet matematike + vulat
pub use laws::{verify_500, SEAL_APUPK, SEAL_COORDINATOR, SEAL_MINI_EVOLVER, SEAL_SNB};

// Capability tokens (ligji FFI që mbijeton)
pub use lgc_gate::{CapHandle, SovErr, SovereignGate};

// Ring buffer 0-copy + EpistemicVector

// Regjistri qendror i vulave
