// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  LAYER3/MOD.RS — Light Layer 3 (Neural Law + Wisdom)              ║
// ║  GJATA LEGACY™ — AIGovern-Cyber Light                                         ║
// ║                                                                          ║
// ║  Shtresa e tretë e Light-it: ligji neural interpretues + urtësia.       ║
// ║   • neural_law   → i jep kuptim çdo lëvizjeje/hyrjeje/daljeje/ligji      ║
// ║                    (sistemi flet si një trup) — POR kurrë s'vendos       ║
// ║   • light_wisdom → observon SI u formua besueshmëria                    ║
// ║                                                                          ║
// ║  Light interpreton, kurrë s'vendos. Zero if/else.                     ║
// ╚══════════════════════════════════════════════════════════════════════════╝

pub mod layer3_flow; // orkestruesi i Shtresës 3 (UI→gjurmim→urtësi→magazinë)
pub mod neural_law; // ligji neural interpretues (sistemi flet si një trup)
pub mod ui_channel; // kanali UI (input me gjurmim + output gati) // kanali pip→Light(trace)→Shadow

// ── Ri-eksportim ────────────────────────────────────────────────────────────

pub use ui_channel::UiInput;
