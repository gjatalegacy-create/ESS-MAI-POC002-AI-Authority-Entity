//! Nbes Shadow: subordinate hold-verification muscle commanded by VDS.

use crate::selection_hold::SelectionHoldStore;
use crate::lab_contracts::{CollapsePhase, GjataCollapseLaw, SupremeDirective};
use shadow_contracts::MuscleEvidenceWire;
use std::marker::PhantomData;

#[derive(Debug, Clone)]
pub struct NbesScopeEvidence {
    pub session_id: String,
    pub primitive_anchor: u64,
    pub muscle_evidence: MuscleEvidenceWire,
}

#[derive(Debug, Clone)]
pub struct NbesCommand {
    pub(crate) session_id: String,
    pub(crate) primitive_anchor: u64,
    pub(crate) combined_sha256: String,
    pub(crate) law_seal: u64,
    pub(crate) system_laws_seal: u64,
}

#[derive(Debug, Clone)]
pub struct NbesAttestation {
    session_id: String,
    primitive_anchor: u64,
    combined_sha256: String,
    besa_hold_id: String,
    asht_hold_id: String,
    verified_bit: u8,
}

impl NbesAttestation {
    pub fn verifies_for(&self, scope: &NbesScopeEvidence) -> bool {
        self.verified_bit == 1
            && self.session_id == scope.session_id
            && self.primitive_anchor == scope.primitive_anchor
            && self.combined_sha256 == scope.muscle_evidence.combined_sha256
            && self.besa_hold_id == scope.muscle_evidence.besa_hold_id
            && self.asht_hold_id == scope.muscle_evidence.asht_hold_id
    }

    pub fn bit(&self) -> u8 { self.verified_bit }

    pub(crate) fn verify_case_report(&self, case: &crate::case_reasoning::VerifiedModelCase)
        -> Result<(), String> {
        if self.verified_bit != 1 || self.session_id != case.model().cycle() {
            return Err("NBES_CASE_OBLIGATION_SCOPE".into());
        }
        let sides=case.model().summary()?;
        println!("[NBES/CASE] {} support_executed={} counter_executed={}",
            shadow_contracts::pd_memory::hex(&case.model().binding()),
            sides.pro.predicate_tested, sides.npro.predicate_tested);
        Ok(())
    }

    pub fn hold_ids(&self) -> (&str, &str) {
        (&self.besa_hold_id, &self.asht_hold_id)
    }

    #[cfg(test)]
    pub(crate) fn direct_test(scope: &NbesScopeEvidence) -> Self {
        Self {
            session_id: scope.session_id.clone(),
            primitive_anchor: scope.primitive_anchor,
            combined_sha256: scope.muscle_evidence.combined_sha256.clone(),
            besa_hold_id: scope.muscle_evidence.besa_hold_id.clone(),
            asht_hold_id: scope.muscle_evidence.asht_hold_id.clone(),
            verified_bit: 1,
        }
    }
}

pub struct Commanded;
pub struct LawGated;

pub struct NbesShadow<State> {
    command: NbesCommand,
    _state: PhantomData<State>,
}

impl NbesShadow<Commanded> {
    pub fn from_command(command: NbesCommand) -> Self {
        Self { command, _state: PhantomData }
    }

    pub fn commission(
        self,
        directive: &SupremeDirective,
    ) -> Result<NbesShadow<LawGated>, String> {
        match (
            GjataCollapseLaw::verify(directive, CollapsePhase::Verification),
            directive.seal == self.command.law_seal,
            self.command.system_laws_seal == crate::lab_contracts::system_laws_seal(),
        ) {
            (true, true, true) => Ok(NbesShadow {
                command: self.command,
                _state: PhantomData,
            }),
            (false, _, _) => Err("Nbes rejected non-Verification GCL directive".to_string()),
            (_, false, _) => Err("Nbes rejected GCL law-seal drift".to_string()),
            (_, _, false) => Err("Nbes rejected system-laws seal drift".to_string()),
        }
    }
}

impl NbesShadow<LawGated> {
    pub fn verify_holds(
        self,
        store: &SelectionHoldStore,
        scope: &NbesScopeEvidence,
        now_ns: u64,
    ) -> Result<NbesAttestation, String> {
    let delivery_scope = scope.muscle_evidence.combined_sha256.as_str();
    crate::noi_caller::invoke_string("SHADOW_NBES_VERIFY", &delivery_scope, || {
        match (
            self.command.session_id == scope.session_id,
            self.command.primitive_anchor == scope.primitive_anchor,
            self.command.combined_sha256 == scope.muscle_evidence.combined_sha256,
        ) {
            (true, true, true) => {}
            (false, _, _) => return Err("Nbes session scope mismatch".to_string()),
            (_, false, _) => return Err("Nbes primitive-anchor scope mismatch".to_string()),
            (_, _, false) => return Err("Nbes muscle digest scope mismatch".to_string()),
        }
        let _holds = store.verify_muscle_evidence(
            &scope.session_id,
            scope.primitive_anchor,
            &scope.muscle_evidence,
            now_ns,
        )?;
        Ok(NbesAttestation {
            session_id: scope.session_id.clone(),
            primitive_anchor: scope.primitive_anchor,
            combined_sha256: scope.muscle_evidence.combined_sha256.clone(),
            besa_hold_id: scope.muscle_evidence.besa_hold_id.clone(),
            asht_hold_id: scope.muscle_evidence.asht_hold_id.clone(),
            verified_bit: 1,
        })
    
    })
}
}
