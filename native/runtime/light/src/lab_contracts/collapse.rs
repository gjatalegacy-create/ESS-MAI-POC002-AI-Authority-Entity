// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  LAB_CONTRACTS/COLLAPSE.RS — Kontrata e Kolapsit Sovran                  ║
// ║  GJATA LEGACY™ — AIGovern-Cyber (AIGovern-Cyber Light: nënndarje e përdorur)         ║
// ║                                                                          ║
// ║  FORMULA E GDHENDUR:                                                     ║
// ║    i0 → PA → (Xi, Yi) → +(-Xi)+(-Yi) → XiYi → XY → D ∈ {0,1}            ║
// ║                                                                          ║
// ║  KOLAPSI 3-FAZOR (formula NUK është modul — është vetë rrjedha):        ║
// ║    Light   = KOLAPS KOORDINIMI  — i0 gjurmohet, PA lind, split Xi/Yi    ║
// ║    Quantum = KOLAPS ARSYETIMI   — LIM mat 8 akset, shënon rrugët        ║
// ║              negative, NPRO eliminon (-Xi)+(-Yi), PRO kërkon i~,        ║
// ║              MPRO/PIM nxjerrin e paketojnë XY, NPIM paketon -X&-Y       ║
// ║    Shadow  = KOLAPS VERIFIKIMI  — XY ↔ (Xi,Yi) në pritje → D ∈ {0,1}   ║
// ║                                                                          ║
// ║  LIGJI: kolapsi është MONOTON — hapësira vetëm ngushtohet, kurrë        ║
// ║  s'zgjerohet; kur zbrazet → REFUZIM (D=0), kurrë shpikje përgjigjeje.   ║
// ║  Rrugët e eliminuara RUHEN si dije negative (aset, jo mbeturinë).       ║
// ║                                                                          ║
// ║  LIGJE KODI: zero if/else (vetëm match), zero varësi, zero clone i      ║
// ║  panevojshëm. Vetëm tipe + wire-format — asnjë reasoning këtu.          ║
// ╚══════════════════════════════════════════════════════════════════════════╝

/// Faza e kolapsit ka një identitet fizik të vetëm në gcl-constitution.
/// Ky re-export ruan path-in ekzistues `lab_contracts::collapse::CollapsePhase`.
pub use gcl_constitution::CollapsePhase;

/// R15: the Xi/Yi split wire has one definition, shared by the three platforms.
pub use shadow_contracts::boundary::split_wire::PrimitiveSplit;

// ════════════════════════════════════════════════════════════════════════════
// TESTE — kontrata e ruajtur e kolapsit provohet në AIGovern-Cyber Light
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phases_cover_three_platforms() {
        assert_eq!(CollapsePhase::Coordination.label(), "KOLAPS_KOORDINIMI");
        assert_eq!(CollapsePhase::Reasoning.label(), "KOLAPS_ARSYETIMI");
        assert_eq!(CollapsePhase::Verification.label(), "KOLAPS_VERIFIKIMI");
    }
}
