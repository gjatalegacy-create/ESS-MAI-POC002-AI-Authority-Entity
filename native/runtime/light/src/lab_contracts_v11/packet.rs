// ╔══════════════════════════════════════════════════════════════════════════╗
// AIGovern-Cyber Light retains the context and payload kind used by input interpretation.
// ║  GJATA LEGACY™ — AIGovern-Cyber (AIGovern-Cyber Light: kontrata të përdorura)        ║
// ║                                                                          ║
// AIGovern-Cyber Light retains the context and payload kind used by input interpretation.
// ║  platformat — boshti i përbashkët: trace_id, claim_id, session_id,     ║
// ║  territory, packet_id, wisdom_ref.                                     ║
// ║                                                                          ║
// ║  Mban vulën 500 (konsistencë me gjithë sistemin). Zero logjikë         ║
// ║  platforme. Zero if/else.                                            ║
// ╚══════════════════════════════════════════════════════════════════════════╝

/// Boshti i identifikuesve që kalon nëpër të tri platformat (Shtresa 3).
#[derive(Debug, Clone, PartialEq)]
pub struct Layer3Context {
    /// Gjurma (lidh gjithë rrjedhën).
    pub trace_id: u64,
    /// Pretendimi.
    pub claim_id: u64,
    /// Sesioni.
    pub session_id: u64,
    /// Territori/domeni.
    pub territory: String,
    /// Identifikuesi i paketës.
    pub packet_id: u64,
    /// Referenca te urtësia (wisdom note e lidhur).
    pub wisdom_ref: u64,
}

impl Layer3Context {
    /// new — krijon kontekstin e Shtresës 3. Zero if.
    pub fn new(
        trace_id: u64,
        claim_id: u64,
        session_id: u64,
        territory: String,
        packet_id: u64,
        wisdom_ref: u64,
    ) -> Self {
        Self {
            trace_id,
            claim_id,
            session_id,
            territory,
            packet_id,
            wisdom_ref,
        }
    }

    /// has_wisdom — a është lidhur me një shënim urtësie? Zero if.
    pub fn has_wisdom(&self) -> bool {
        self.wisdom_ref != 0
    }
}

/// Lloji i ngarkesës që paketa e Shtresës 3 mbart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer3PayloadKind {
    NeuralLaw, // interpretimi neural (Light)
}

impl Layer3PayloadKind {
    pub fn label(self) -> &'static str {
        match self {
            Layer3PayloadKind::NeuralLaw => "NEURAL_LAW",
        }
    }
}
