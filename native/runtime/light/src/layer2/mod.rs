// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  LAYER2/MOD.RS — Light Layer 2 (Active Trust Laboratory)            ║
// ║  GJATA LEGACY™ — AIGovern-Cyber Light                                         ║
// ║                                                                          ║
// ║  Shtresa e dytë laboratorike e Light-it: besim aktiv, gjurmë verifikimi,║
// ║  forcim me mini algoritëm. Presion i BUTË (jo sulm). Light mbetet       ║
// ║  veshi, goja dhe ndërtuesi i besimit — por më aktiv dhe i mprehtë.      ║
// ║                                                                          ║
// ║  Prodhon TrustContext + VerificationTrace. Zero if/else.              ║
// ╚══════════════════════════════════════════════════════════════════════════╝

pub mod active_trust; // Active Trust Layer (prodhon TrustContext)
pub mod verification_trace; // Verification Trace + Mini Algorithm Hardening

// ── Ri-eksportim ────────────────────────────────────────────────────────────
pub use active_trust::ReliabilitySignals;
