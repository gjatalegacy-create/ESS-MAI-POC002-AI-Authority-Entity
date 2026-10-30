// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  LAB_CONTRACTS_V11/MOD.RS — Gjuha e Shtresës 3 (    )               ║
// ║  GJATA LEGACY™ — AIGovern-Cyber (AIGovern-Cyber Light: nënndarje e përdorur)      ║
// ║                                                                          ║
// ║  Zgjerimi i gjuhës për Shtresën 3, PA prekur lab_contracts (    ).     ║
// AIGovern-Cyber Light retains Layer3Context and the NeuralLaw payload kind.
// ║  ZERO logjikë platforme. ZERO presion. AIGovern-Cyber ruan vetëm pjesën e përdorur.    ║
// ║                                                                          ║
// ║  AIGovern-Cyber: përzgjedhja dhe heqjet gjurmohen në Q7/provenance.         ║
// ╚══════════════════════════════════════════════════════════════════════════╝

pub mod packet;

// ── Ri-eksportim i sheshtë ──────────────────────────────────────────────────
#[cfg(test)]
pub use packet::Layer3Context;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer3_context_no_wisdom() {
        let ctx = Layer3Context::new(1, 2, 3, "d".to_string(), 4, 0);
        assert!(!ctx.has_wisdom()); // wisdom_ref=0
    }
}
