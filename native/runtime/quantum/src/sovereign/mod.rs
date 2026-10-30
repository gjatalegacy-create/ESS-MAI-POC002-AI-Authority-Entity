// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  SOVEREIGN/MOD.RS — Ligji 0-COPY Sovereign (adaptuar në Quantum)      ║
// ║  GJATA LEGACY™ — Quantum Platform                                     ║
// ║                                                                          ║
// ║  Ligjet matematike + capability tokens + ring buffer 0-copy.          ║
// ║  eBPF/Hardware → RingBuffer → LgcBridge → EpistemicVector → LIM       ║
// ╚══════════════════════════════════════════════════════════════════════════╝

pub mod laws;
pub mod lgc_gate;
pub mod ring;
pub mod seal_registry;
pub mod sha256; // SHA-256 i vërtetë (FIPS 180-4) për Digital Lab // Regjistri qendror i vulave (anti-përplasje)

// Ligjet matematike
pub use laws::{
    admit4, dot8, elim, fnv, gate, module_seal, SEAL_EBPF, SEAL_LIM, SEAL_PD, SEAL_PIM, SEAL_PRO,
    SEAL_SRK,
};

// SHA-256 (FIPS 180-4)
pub use sha256::{sha256, sha256_hex};

// Capability tokens (LAW_3 mbijeton FFI)
pub use lgc_gate::{CapHandle, LgcGate, LgcToken, SovErr, SovereignGate};

// Ring buffer 0-copy + EpistemicVector
pub use ring::{
    EbpfPacket, Ev, LgcBridge, RingBuffer, RingConsumer, RingProducer, RING_MASK, RING_N,
};

// Regjistri qendror i vulave (anti-përplasje)
pub use seal_registry::{
    build_registry, CollisionReport, Platform, SealEntry, SealRegistry, REGISTRY_SIZE,
};
