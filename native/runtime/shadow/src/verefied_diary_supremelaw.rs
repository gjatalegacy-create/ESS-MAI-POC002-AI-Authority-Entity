// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  VEREFIED_DIARY_SUPREMELAW — MUSKULI I SHADOW-it (Verification Collapse) ║
// ║  Gjata Legacy™  |  I ushqyer nga gjata_collapse_law (Direktiva Supreme)  ║
// ║                                                                          ║
// ║  KY MUSKUL I SHPJEGON SISTEMIT SE ÇFARË GJYKOHET — NË GJUHË KODI:        ║
// ║  JO vetëm rrjedha Y (outputi). Në bazë të i0 gjykohet PROCESI i          ║
// ║  ultimatum_collapse_law: propozimi i Quantum kundrejt indicies së        ║
// ║  Light-it. Struktura NUK duhet të bjerë:                                 ║
// ║                                                                          ║
// ║    X (i0/PA+Xi‖Yi nga Light) → i besueshëm?   [inputi gjykohet]          ║
// ║    PROCESI (ultimatumi: XY|refuzim, faktik, negativat) → i ligjshëm?     ║
// ║    Y (outputi siç e ka rrjedha) → i vulosur?  [rrjedha ekzistuese]       ║
// ║                                                                          ║
// ║  GJENDJET (simbolet e fazës Verification):                               ║
// ║    AwaitingAnchor   — ditari pret sinjalin e Light                       ║
// ║    AnchorTrusted    — X/i0: PA+(Xi,Yi) u besua si indicie                ║
// ║    ProposalReceived — Y-propozimi: XY (ose refuzimi) i Quantum erdhi     ║
// ║    ProcessJudged    — PROCESI u gjykua mbi i0 (jo vetëm Y)               ║
// ║    StructureHeld    — struktura QËNDROI (negativat u vulosën si aset)    ║
// ║    DiaryVerdict     — D ∈ {0,1}: ditari i verifikuar vulosi              ║
// ║                                                                          ║
// ║  INVARIANTI: "Asgjë nuk pranohet pa Sovereign_Verification_Collapse."    ║
// ║  Shadow s'krijon ide, s'arsyeton nga e para — PROVON ose RRËZON.        ║
// ║  Stil kerneli: repr(u8), match TOTAL, ditar i shkruar (white-box).       ║
// ╚══════════════════════════════════════════════════════════════════════════╝

use crate::flow_stage::{
    AnchorTrusted, AwaitingAnchor, DiaryVerdict, FlowBreach, FlowStage, ProcessJudged,
    ProposalReceived, StructureHeld,
};
use crate::lab_contracts::gjata_collapse_law::law_seal;
use crate::lab_contracts::{CollapsePhase, GjataCollapseLaw, SupremeDirective};
use std::marker::PhantomData;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VdsState {
    AwaitingAnchor   = 0,
    AnchorTrusted    = 1,
    ProposalReceived = 2,
    ProcessJudged    = 3,
    StructureHeld    = 4,
    DiaryVerdict     = 5,
}

impl VdsState {
    pub fn label(&self) -> &'static str {
        match self {
            Self::AwaitingAnchor   => "AWAITING_ANCHOR",
            Self::AnchorTrusted    => "ANCHOR_TRUSTED(X/i0)",
            Self::ProposalReceived => "PROPOSAL_RECEIVED(Y)",
            Self::ProcessJudged    => "PROCESS_JUDGED(ultimatum⊢i0)",
            Self::StructureHeld    => "STRUCTURE_HELD",
            Self::DiaryVerdict     => "DIARY_VERDICT(D)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VdsEvent {
    TrustAnchor,     // PA+(Xi,Yi) nga Light — inputi X/i0 u besua
    ReceiveProposal, // XY | refuzimi i mbartur — Y-propozimi erdhi
    JudgeProcess,    // gjykimi i PROCESIT të ultimatum-it mbi i0
    HoldStructure,   // negativat u vulosën; struktura s'ra
    SealDiary,       // verdikti sovran doli — ditari vulos D
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VdsBreach {
    pub from:  VdsState,
    pub event: VdsEvent,
    pub text:  &'static str,
}

/// Forma e provës primitive që GCL/AIGovern-Cyber i kërkon muskulit Shadow.
/// Nuk është verdict dhe nuk del në wire; lidh vetëm i₀/PA, Xi/Yi,
/// transformimin Quantum dhe vazhdimësinë e të njëjtit proces GCL.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GclPrimitiveMode {
    Bound = 1,
    Refused = 2,
    LegacyCompatible = 3,
    DirectTest = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GclPrimitiveVerification {
    pub mode: GclPrimitiveMode,
    pub state: VdsState,
    pub directive_seal: u64,
    pub primitive_anchor: u64,
    pub parent_i0_digest: u64,
    pub proposal_digest: u64,
    pub pd_binding_digest: u64,
    pub pd_continuum_activation_digest: u64,
    pub anchor_present: bool,
    pub parent_i0_matches: bool,
    pub xi_verified: bool,
    pub yi_verified: bool,
    pub proposal_received: bool,
    pub transformation_traced: bool,
    pub gcl_continuity: bool,
    pub nbes_verified: bool,
    pub structure_held: bool,
}

impl GclPrimitiveVerification {
    pub fn primitive_bit(&self) -> u8 {
        match self.mode {
            GclPrimitiveMode::Bound => (
                self.anchor_present
                    & self.parent_i0_matches
                    & self.xi_verified
                    & self.yi_verified
            ) as u8,
            GclPrimitiveMode::Refused => (
                self.anchor_present
                    & self.parent_i0_matches
            ) as u8,
            GclPrimitiveMode::LegacyCompatible => 0,
            GclPrimitiveMode::DirectTest => 1,
        }
    }

    pub fn proposal_bit(&self) -> u8 {
        match self.mode {
            GclPrimitiveMode::Refused => 0,
            GclPrimitiveMode::Bound
            | GclPrimitiveMode::LegacyCompatible
            | GclPrimitiveMode::DirectTest => self.proposal_received as u8,
        }
    }

    pub fn transformation_bit(&self) -> u8 {
        match self.mode {
            GclPrimitiveMode::LegacyCompatible => 0,
            GclPrimitiveMode::DirectTest => 1,
            GclPrimitiveMode::Bound | GclPrimitiveMode::Refused => (
                self.transformation_traced
                    & (self.parent_i0_digest != 0)
                    & (self.proposal_digest != 0)
                    & (self.pd_binding_digest != 0)
                    & (self.pd_continuum_activation_digest != 0)
            ) as u8,
        }
    }

    pub fn gcl_bit(&self) -> u8 {
        let directive_ok = self.directive_seal == law_seal();
        match self.mode {
            GclPrimitiveMode::LegacyCompatible => 0,
            GclPrimitiveMode::DirectTest => directive_ok as u8,
            GclPrimitiveMode::Bound | GclPrimitiveMode::Refused =>
                (directive_ok & self.gcl_continuity & self.nbes_verified & self.structure_held) as u8,
        }
    }

    pub fn final_bit(&self) -> u8 {
        self.primitive_bit()
            & self.proposal_bit()
            & self.transformation_bit()
            & self.gcl_bit()
    }

    #[cfg(test)]
    pub fn direct_test(primitive_anchor: u64) -> Self {
        Self {
            mode: GclPrimitiveMode::DirectTest,
            state: VdsState::StructureHeld,
            directive_seal: law_seal(),
            primitive_anchor,
            parent_i0_digest: primitive_anchor.wrapping_add(1),
            proposal_digest: primitive_anchor.wrapping_add(2),
            pd_binding_digest: primitive_anchor.wrapping_add(3),
            pd_continuum_activation_digest: primitive_anchor.wrapping_add(4),
            anchor_present: primitive_anchor != 0,
            parent_i0_matches: true,
            xi_verified: true,
            yi_verified: true,
            proposal_received: true,
            transformation_traced: true,
            gcl_continuity: true,
            nbes_verified: true,
            structure_held: true,
        }
    }
}

pub struct VerefiedDiarySupremelaw {
    state: VdsState,
    directive_seal: u64,
}

impl VerefiedDiarySupremelaw {
    pub fn commission(d: SupremeDirective) -> Result<Self, VdsBreach> {
        match GjataCollapseLaw::verify(&d, CollapsePhase::Verification) {
            true  => Ok(Self { state: VdsState::AwaitingAnchor, directive_seal: d.seal }),
            false => Err(VdsBreach {
                from:  VdsState::AwaitingAnchor,
                event: VdsEvent::TrustAnchor,
                text:  "direktivë e pavlefshme: muskuli i Shadow kërkon fazën Verification të vulosur",
            }),
        }
    }

    /// TABELA E LIGJIT — gjykimi strukturor, match TOTAL.
    fn transition(state: VdsState, event: VdsEvent) -> Result<VdsState, VdsBreach> {
        match (state, event) {
            (VdsState::AwaitingAnchor,   VdsEvent::TrustAnchor)     => Ok(VdsState::AnchorTrusted),
            (VdsState::AnchorTrusted,    VdsEvent::ReceiveProposal) => Ok(VdsState::ProposalReceived),
            (VdsState::ProposalReceived, VdsEvent::JudgeProcess)    => Ok(VdsState::ProcessJudged),
            (VdsState::ProcessJudged,    VdsEvent::HoldStructure)   => Ok(VdsState::StructureHeld),
            (VdsState::StructureHeld,    VdsEvent::SealDiary)       => Ok(VdsState::DiaryVerdict),
            (from, event) => Err(VdsBreach {
                from, event,
                text: "rend i shkelur: X-besim→Y-propozim→gjykim-procesi→strukturë→ditar-D",
            }),
        }
    }

    pub fn advance(&mut self, event: VdsEvent) -> Result<VdsState, VdsBreach> {
        let next = Self::transition(self.state, event)?;
        self.state = next;
        Ok(next)
    }

    pub fn state(&self) -> VdsState {
        self.state
    }

    pub fn is_fulfilled(&self) -> bool {
        matches!(self.state, VdsState::DiaryVerdict)
    }


    /// Pajtueshmëria historike mbetet e deklaruar, por lind vetëm nga një
    /// muskul VDS i komisionuar dhe i vulosur realisht nga GCL/AIGovern-Cyber.
    pub fn attest_legacy_compatible(&self) -> GclPrimitiveVerification {
        GclPrimitiveVerification {
            mode: GclPrimitiveMode::LegacyCompatible,
            state: self.state,
            directive_seal: self.directive_seal,
            primitive_anchor: 0,
            parent_i0_digest: 0,
            proposal_digest: 0,
            pd_binding_digest: 0,
            pd_continuum_activation_digest: 0,
            anchor_present: false,
            parent_i0_matches: true,
            xi_verified: false,
            yi_verified: false,
            proposal_received: true,
            transformation_traced: true,
            gcl_continuity: true,
            nbes_verified: false,
            structure_held: true,
        }
    }

    /// Krijon provën primitive vetëm pasi procesi është gjykuar dhe struktura
    /// është mbajtur. Kjo provë ushqen multi-verifikimin si forcë e GCL-së.
    pub fn attest_structure(
        &self,
        mode: GclPrimitiveMode,
        primitive_anchor: u64,
        parent_i0_digest: u64,
        proposal_digest: u64,
        pd_binding_digest: u64,
        pd_continuum_activation_digest: u64,
        anchor_present: bool,
        parent_i0_matches: bool,
        xi_verified: bool,
        yi_verified: bool,
        proposal_received: bool,
        transformation_traced: bool,
        gcl_continuity: bool,
        nbes_verified: bool,
    ) -> Result<GclPrimitiveVerification, VdsBreach> {
        match self.state {
            VdsState::StructureHeld => Ok(GclPrimitiveVerification {
                mode,
                state: self.state,
                directive_seal: self.directive_seal,
                primitive_anchor,
                parent_i0_digest,
                proposal_digest,
                pd_binding_digest,
                pd_continuum_activation_digest,
                anchor_present,
                parent_i0_matches,
                xi_verified,
                yi_verified,
                proposal_received,
                transformation_traced,
                gcl_continuity,
                nbes_verified,
                structure_held: true,
            }),
            from => Err(VdsBreach {
                from,
                event: VdsEvent::HoldStructure,
                text: "prova primitive kërkon ProcessJudged→StructureHeld nën direktivën GCL",
            }),
        }
    }

    /// Rreshti i ditarit — dëshmia e shkruar e gjykimit (white-box).
    pub fn diary_line(&self, session_id: &str) -> String {
        format!("[VDS-DIARY] session={} gjendja={}", session_id, self.state.label())
    }
}


/// Compile-time promotion of the existing VDS runtime machine.
///
/// `ShadowFlow` owns exactly one `VerefiedDiarySupremelaw`. The generic stage
/// only restricts which next transition is callable; VDS remains the runtime
/// witness and the sole diary state machine.
pub struct ShadowFlow<S: FlowStage> {
    runtime: VerefiedDiarySupremelaw,
    _stage: PhantomData<S>,
}

impl ShadowFlow<AwaitingAnchor> {
    pub fn commission(directive: SupremeDirective) -> Result<Self, FlowBreach> {
        let runtime = VerefiedDiarySupremelaw::commission(directive)
            .map_err(|breach| FlowBreach::WrongPhase(breach.text))?;
        Ok(Self {
            runtime,
            _stage: PhantomData,
        })
    }

    pub fn trust_anchor(
        self,
        directive: &SupremeDirective,
    ) -> Result<ShadowFlow<AnchorTrusted>, FlowBreach> {
        self.transition(
            directive,
            VdsEvent::TrustAnchor,
            FlowBreach::AnchorUntrusted,
        )
    }

    pub fn attest_legacy_compatible(&self) -> GclPrimitiveVerification {
        self.runtime.attest_legacy_compatible()
    }
}

impl ShadowFlow<AnchorTrusted> {
    pub fn receive_proposal(
        self,
        directive: &SupremeDirective,
    ) -> Result<ShadowFlow<ProposalReceived>, FlowBreach> {
        self.transition(
            directive,
            VdsEvent::ReceiveProposal,
            FlowBreach::ProposalInvalid,
        )
    }
}

impl ShadowFlow<ProposalReceived> {
    pub fn command_nbes(
        &self,
        directive: &SupremeDirective,
        scope: &crate::nbes_shadow::NbesScopeEvidence,
    ) -> Result<crate::nbes_shadow::NbesCommand, FlowBreach> {
        match (
            GjataCollapseLaw::verify(directive, CollapsePhase::Verification),
            scope.primitive_anchor != 0,
            scope.muscle_evidence.verifies_internal(),
        ) {
            (true, true, true) => Ok(crate::nbes_shadow::NbesCommand {
                session_id: scope.session_id.clone(),
                primitive_anchor: scope.primitive_anchor,
                combined_sha256: scope.muscle_evidence.combined_sha256.clone(),
                law_seal: directive.seal,
                system_laws_seal: crate::lab_contracts::system_laws_seal(),
            }),
            _ => Err(FlowBreach::ProcessContradiction(
                "VDS refused to command Nbes after ProposalReceived",
            )),
        }
    }

    pub fn judge_process(
        self,
        directive: &SupremeDirective,
        nbes: &crate::nbes_shadow::NbesAttestation,
        scope: &crate::nbes_shadow::NbesScopeEvidence,
    ) -> Result<ShadowFlow<ProcessJudged>, FlowBreach> {
        match nbes.verifies_for(scope) {
            true => {}
            false => return Err(FlowBreach::ProcessContradiction(
                "VDS refused an unbound Nbes attestation",
            )),
        }
        self.transition(
            directive,
            VdsEvent::JudgeProcess,
            FlowBreach::ProcessContradiction,
        )
    }
}

impl ShadowFlow<ProcessJudged> {
    pub fn hold_structure(
        self,
        directive: &SupremeDirective,
    ) -> Result<ShadowFlow<StructureHeld>, FlowBreach> {
        self.transition(
            directive,
            VdsEvent::HoldStructure,
            FlowBreach::StructureBroken,
        )
    }
}

impl ShadowFlow<StructureHeld> {
    #[allow(clippy::too_many_arguments)]
    pub fn attest_structure(
        &self,
        mode: GclPrimitiveMode,
        primitive_anchor: u64,
        parent_i0_digest: u64,
        proposal_digest: u64,
        pd_binding_digest: u64,
        pd_continuum_activation_digest: u64,
        anchor_present: bool,
        parent_i0_matches: bool,
        xi_verified: bool,
        yi_verified: bool,
        proposal_received: bool,
        transformation_traced: bool,
        gcl_continuity: bool,
        nbes_verified: bool,
    ) -> Result<GclPrimitiveVerification, FlowBreach> {
        self.runtime
            .attest_structure(
                mode,
                primitive_anchor,
                parent_i0_digest,
                proposal_digest,
                pd_binding_digest,
                pd_continuum_activation_digest,
                anchor_present,
                parent_i0_matches,
                xi_verified,
                yi_verified,
                proposal_received,
                transformation_traced,
                gcl_continuity,
                nbes_verified,
            )
            .map_err(|breach| FlowBreach::StructureBroken(breach.text))
    }

    pub fn diary_verdict(
        self,
        directive: &SupremeDirective,
    ) -> Result<ShadowFlow<DiaryVerdict>, FlowBreach> {
        self.transition(
            directive,
            VdsEvent::SealDiary,
            FlowBreach::ThroneRefusal,
        )
    }
}

impl ShadowFlow<DiaryVerdict> {
    pub fn diary_line(&self, session_id: &str) -> String {
        // Output-i i vulosur mbetet byte-identik me VDS-në ekzistuese.
        // StageTag-et mbeten dëshmi compile-time mbi të njëjtën VDS.
        self.runtime.diary_line(session_id)
    }
}

impl<S: FlowStage> ShadowFlow<S> {
    fn transition<N: FlowStage>(
        mut self,
        directive: &SupremeDirective,
        event: VdsEvent,
        breach_map: fn(&'static str) -> FlowBreach,
    ) -> Result<ShadowFlow<N>, FlowBreach> {
        match GjataCollapseLaw::verify(directive, CollapsePhase::Verification) {
            true => {}
            false => {
                return Err(FlowBreach::WrongPhase(
                    "çdo tranzicion ShadowFlow kërkon direktivën Verification të vulosur",
                ))
            }
        }
        match S::TAG as u8 == self.runtime.state() as u8 {
            true => {}
            false => {
                return Err(FlowBreach::StructureBroken(
                    "tipi ShadowFlow dhe gjendja runtime e VDS-së u shkëputën",
                ))
            }
        }
        let next = self
            .runtime
            .advance(event)
            .map_err(|breach| breach_map(breach.text))?;
        match N::TAG as u8 == next as u8 {
            true => Ok(ShadowFlow {
                runtime: self.runtime,
                _stage: PhantomData,
            }),
            false => Err(FlowBreach::StructureBroken(
                "tranzicioni VDS nuk përputhet me tipin pasues ShadowFlow",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lab_contracts::{GjataCollapseLaw as Law, CollapsePhase as Phase};

    fn nbes_fixture() -> (
        crate::nbes_shadow::NbesScopeEvidence,
        crate::nbes_shadow::NbesAttestation,
    ) {
        let mut muscle_evidence = shadow_contracts::MuscleEvidenceWire {
            besa_hold_id: "besa-hold".into(),
            besa_hold_sha256: "11".repeat(32),
            besa_selection_sha256: "22".repeat(32),
            besa_attestation_sha256: "33".repeat(32),
            asht_hold_id: "asht-hold".into(),
            asht_hold_sha256: "44".repeat(32),
            asht_selection_sha256: "55".repeat(32),
            asht_attestation_sha256: "66".repeat(32),
            split_sha256: "77".repeat(32),
            combined_sha256: String::new(),
        };
        muscle_evidence.combined_sha256 = muscle_evidence.recompute_combined_sha256();
        let scope = crate::nbes_shadow::NbesScopeEvidence {
            session_id: "S-STAGES".into(),
            primitive_anchor: 0xA451,
            muscle_evidence,
        };
        let attestation = crate::nbes_shadow::NbesAttestation::direct_test(&scope);
        (scope, attestation)
    }

    #[test]
    fn full_structural_judgement_chain() {
        let mut m = VerefiedDiarySupremelaw::commission(Law::issue(Phase::Verification)).unwrap();
        for ev in [VdsEvent::TrustAnchor, VdsEvent::ReceiveProposal,
                   VdsEvent::JudgeProcess, VdsEvent::HoldStructure, VdsEvent::SealDiary] {
            m.advance(ev).unwrap();
        }
        assert!(m.is_fulfilled());
        assert!(m.diary_line("S1").contains("DIARY_VERDICT"));
    }

    #[test]
    fn judging_y_without_trusted_x_is_breach() {
        // Gjykimi VETËM i Y-së (pa X/i0 të besuar) është shkelje — thelbi i ligjit.
        let mut m = VerefiedDiarySupremelaw::commission(Law::issue(Phase::Verification)).unwrap();
        let b = m.advance(VdsEvent::ReceiveProposal).unwrap_err();
        assert_eq!(b.from, VdsState::AwaitingAnchor);
        assert!(b.text.contains("rend i shkelur"));
    }

    #[test]
    fn wrong_phase_refused() {
        assert!(VerefiedDiarySupremelaw::commission(Law::issue(Phase::Coordination)).is_err());
    }

    #[test]
    fn bound_primitive_proof_requires_full_gcl_state_chain() {
        let mut m = VerefiedDiarySupremelaw::commission(
            Law::issue(Phase::Verification),
        ).unwrap();
        for event in [
            VdsEvent::TrustAnchor,
            VdsEvent::ReceiveProposal,
            VdsEvent::JudgeProcess,
            VdsEvent::HoldStructure,
        ] {
            m.advance(event).unwrap();
        }
        let proof = m.attest_structure(
            GclPrimitiveMode::Bound,
            0xA451,
            0xA452,
            0xA453,
            0xA454,
            0xA455,
            true,
            true,
            true,
            true,
            true,
            true,
            true,
            true,
        ).unwrap();
        assert_eq!(proof.state, VdsState::StructureHeld);
        assert_eq!(proof.final_bit(), 1);
    }

    fn require_ok<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
        match result {
            Ok(value) => value,
            Err(error) => panic!("pritej Ok, u mor Err: {error:?}"),
        }
    }

    #[test]
    fn typestate_chain_is_the_existing_vds_chain() {
        let directive = Law::issue(Phase::Verification);
        let flow = require_ok(ShadowFlow::<AwaitingAnchor>::commission(directive));
        let flow = require_ok(flow.trust_anchor(&directive));
        let flow = require_ok(flow.receive_proposal(&directive));
        let (scope, nbes) = nbes_fixture();
        let flow = require_ok(flow.judge_process(&directive, &nbes, &scope));
        let flow = require_ok(flow.hold_structure(&directive));
        let flow = require_ok(flow.diary_verdict(&directive));

        let final_line = flow.diary_line("S-STAGES");
        assert!(final_line.contains("DIARY_VERDICT"));
    }

    #[test]
    fn typestate_diary_output_remains_byte_identical_to_runtime_vds() {
        let directive = Law::issue(Phase::Verification);
        let flow = require_ok(ShadowFlow::<AwaitingAnchor>::commission(directive));
        let flow = require_ok(flow.trust_anchor(&directive));
        let flow = require_ok(flow.receive_proposal(&directive));
        let (scope, nbes) = nbes_fixture();
        let flow = require_ok(flow.judge_process(&directive, &nbes, &scope));
        let flow = require_ok(flow.hold_structure(&directive));
        let flow = require_ok(flow.diary_verdict(&directive));

        let mut runtime = require_ok(VerefiedDiarySupremelaw::commission(directive));
        for event in [
            VdsEvent::TrustAnchor,
            VdsEvent::ReceiveProposal,
            VdsEvent::JudgeProcess,
            VdsEvent::HoldStructure,
            VdsEvent::SealDiary,
        ] {
            require_ok(runtime.advance(event));
        }

        assert_eq!(flow.diary_line("S-TYPE"), runtime.diary_line("S-TYPE"));
    }

    #[test]
    fn typestate_reverifies_the_directive_at_every_transition() {
        let verification = Law::issue(Phase::Verification);
        let wrong_phase = Law::issue(Phase::Reasoning);
        let flow = require_ok(ShadowFlow::<AwaitingAnchor>::commission(verification));

        match flow.trust_anchor(&wrong_phase) {
            Err(FlowBreach::WrongPhase(detail)) => assert!(!detail.is_empty()),
            Err(other) => panic!("breach i gabuar: {other}"),
            Ok(_unexpected_flow) => {
                panic!("tranzicioni me fazë të gabuar nuk duhet të kalojë")
            }
        }
    }

    #[test]
    fn refusal_is_processed_but_cannot_become_positive_proposal() {
        let mut m = VerefiedDiarySupremelaw::commission(
            Law::issue(Phase::Verification),
        ).unwrap();
        for event in [
            VdsEvent::TrustAnchor,
            VdsEvent::ReceiveProposal,
            VdsEvent::JudgeProcess,
            VdsEvent::HoldStructure,
        ] {
            m.advance(event).unwrap();
        }
        let proof = m.attest_structure(
            GclPrimitiveMode::Refused,
            0xA451,
            0xA452,
            0xA453,
            0xA454,
            0xA455,
            true,
            true,
            false,
            false,
            true,
            true,
            true,
            true,
        ).unwrap();
        assert_eq!(proof.primitive_bit(), 1);
        assert_eq!(proof.proposal_bit(), 0);
        assert_eq!(proof.final_bit(), 0);
    }
}
