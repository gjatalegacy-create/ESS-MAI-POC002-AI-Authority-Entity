// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  SHADOW_SEAL_BRIDGE.RS — PORTA E FUNDIT E LIGHT DREJT SHADOW          ║
// ║  Gjata Legacy™  |  Arkitekt: Bledar Gjata                             ║
// ║                                                                          ║
// ║  Mbart vulën 500 nga EvolveTrace → buss_legacy (i VERBËR) →           ║
// ║  shadow_gj_legacy.lgc, i cili vendos 0/1.                            ║
// ║                                                                          ║
// ║  FILOZOFIA:                                                             ║
// ║    Light NUK vendos. Light llogarit vulën dhe e TRANSPORTON.          ║
// ║    buss_legacy nuk e di ç'është 500 — e kopjon të verbër.            ║
// ║    shadow_gj_legacy është i VETMI që e kupton dhe vendos.            ║
// ║                                                                          ║
// ║  c_kernel ON  → FFI real te kernelët C (buss_legacy + shadow).        ║
// ║  c_kernel OFF → fallback pure-Rust me TË NJËJTËN logjikë.             ║
// ╚══════════════════════════════════════════════════════════════════════════╝

// ─────────────────────────────────────────────────────────────────────────────
// KONSTANTET E VULËS — identike me shadow_gj_legacy.c
// ─────────────────────────────────────────────────────────────────────────────

// ─────────────────────────────────────────────────────────────────────────────
// VERDIKTI I SHADOW-it
// ─────────────────────────────────────────────────────────────────────────────

/// Vendimi suprem i shadow_gj_legacy.lgc mbi një input.
#[derive(Debug, Clone, PartialEq)]
pub enum ShadowVerdict {
    /// 1 — vula 500 mbijetoi: input mbeti primitiv i fortë.
    PrimitiveKnowledge,
    /// 0 — vula u hoq: input u fragmentua; i verifikueshëm, jo primitiv.
    VerifiableNotPrimitive,
}

impl ShadowVerdict {
    pub fn bit(&self) -> u8 {
        match self {
            Self::PrimitiveKnowledge => 1,
            Self::VerifiableNotPrimitive => 0,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            Self::PrimitiveKnowledge => "1:PRIMITIVE_KNOWLEDGE",
            Self::VerifiableNotPrimitive => "0:VERIFIABLE_NOT_PRIMITIVE",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// FFI — buss_legacy (i verbër) + shadow_gj_legacy (autoriteti)
// Vetëm me feature c_kernel.
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "c_kernel")]
mod ffi {
    // BussLegacyMsg — ABI identike me kernel/buss_legacy.h

    // R7: reuse the exact canonical repr(C) types for the shared C symbol.

    #[link(name = "light_kernel")]
    extern "C" {}
}

// ─────────────────────────────────────────────────────────────────────────────
// LOGJIKA E AUTORITETIT (pure-Rust mirror — identike me shadow_gj_legacy.c)
// Përdoret si fallback kur c_kernel OFF, dhe si referencë e vërtetës.
// ─────────────────────────────────────────────────────────────────────────────

// ─────────────────────────────────────────────────────────────────────────────
// PORTA — dispaton vulën përmes bus-it të verbër te autoriteti
// ─────────────────────────────────────────────────────────────────────────────

// ─────────────────────────────────────────────────────────────────────────────
// TESTE
// ─────────────────────────────────────────────────────────────────────────────
