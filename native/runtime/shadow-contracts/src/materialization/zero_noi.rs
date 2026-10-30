// Source: ess-mai-dbab-zero-noi/src/zero_noi.rs (Bledar Gjata / Gjata Legacy,
// Apache-2.0), integrated into AIGovern-Cyber.
// Provenance and adaptations: ../../../SOURCE_NOTES.md.
//! New POC-native 0-NOI materialization, not an inherited production extract.
//!
//! 0-NOI checks an evidenced, active, unmet confrontation requirement and emits
//! an informational trigger. It has no knowledge store, verdict, authority,
//! persistence, candidate selection, reasoning loop, or closure operation.
//!
//! The host supplies a trusted [`EvidenceVerifier`]. Although its interface is
//! read-only, Rust permits interior mutability, I/O, and other effects inside a
//! foreign implementation. Consequently the no-mutation claim covers this
//! module and the supplied read-only catalog, not an arbitrary hostile verifier.
//! A host that accepts fabricated evidence invalidates the evidence guarantee.
//!
//! A trigger has no public constructor and is not deserializable:
//!
//! ```compile_fail
//! use shadow_contracts::materialization::zero_noi::ZeroNoiTrigger;
//! let forged = ZeroNoiTrigger {};
//! ```
//!
//! ```compile_fail
//! use shadow_contracts::materialization::zero_noi::ZeroNoiTrigger;
//! let forged: ZeroNoiTrigger = serde_json::from_str("{}").unwrap();
//! ```
//!
//! A genuine trigger still exposes no authority or persistence operation:
//!
//! ```compile_fail
//! use shadow_contracts::materialization::zero_noi::ZeroNoiTrigger;
//! fn escalate(trigger: &ZeroNoiTrigger) {
//!     trigger.authority_token();
//! }
//! ```

#![forbid(unsafe_code)]

use super::common::{
    digest, valid_id, verify_references, CaseIdentity, Digest32, EvidenceError,
    EvidenceRef, EvidenceRole, EvidenceVerifier, MAX_REFERENCES,
};
use serde::{Deserialize, Serialize};

/// A host-declared requirement. Evidence must establish every field except the
/// references themselves. Unknown JSON fields are rejected; runtime also bounds
/// the reference list to eight entries and rejects empty or duplicate evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZeroNoiInput {
    pub case: CaseIdentity,
    pub obligation_id: String,
    pub active: bool,
    pub unmet: bool,
    pub evidence: Vec<EvidenceRef>,
}

/// Domain-separated requirement claim for the host's authenticated evidence.
/// References are excluded to avoid a circular record/reference digest; case,
/// obligation identity, active status, and unmet status are all included.
pub fn requirement_claim(input: &ZeroNoiInput) -> Result<Digest32, EvidenceError> {
    #[derive(Serialize)]
    struct Requirement<'a> {
        case: &'a CaseIdentity,
        obligation_id: &'a str,
        active: bool,
        unmet: bool,
    }
    digest(
        "ESS-MAI/0-NOI/confrontation-requirement/v1",
        &Requirement {
            case: &input.case,
            obligation_id: &input.obligation_id,
            active: input.active,
            unmet: input.unmet,
        },
    )
}

/// Why activation is permitted. This formula selects no arguments or outcome.
pub const fn trigger_formula(active: bool, unmet: bool, evidence_valid: bool) -> bool {
    active && unmet && evidence_valid
}

/// Execution receipt, not a knowledge record or grant of authority.
/// Zero deltas describe this module's semantic powers; callers must separately
/// measure their host's state when recording end-to-end runtime evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ZeroNoiEvidence {
    trigger_executed: bool,
    state_delta: u64,
    knowledge_delta: u64,
    verdict_created: bool,
    authority_created: bool,
    persistence_delta: u64,
    candidate_selection: bool,
}

impl ZeroNoiEvidence {
    pub const fn trigger_executed(&self) -> bool { self.trigger_executed }
    pub const fn state_delta(&self) -> u64 { self.state_delta }
    pub const fn knowledge_delta(&self) -> u64 { self.knowledge_delta }
    pub const fn verdict_created(&self) -> bool { self.verdict_created }
    pub const fn authority_created(&self) -> bool { self.authority_created }
    pub const fn persistence_delta(&self) -> u64 { self.persistence_delta }
    pub const fn candidate_selection(&self) -> bool { self.candidate_selection }
}

/// What activation can do, independently of why it is permitted:
/// state = knowledge = verdict = authority = persistence = selection = 0;
/// trigger = 1. This is an informational receipt, never an authorization check.
pub const fn behavior_formula() -> ZeroNoiEvidence {
    ZeroNoiEvidence {
        trigger_executed: true,
        state_delta: 0,
        knowledge_delta: 0,
        verdict_created: false,
        authority_created: false,
        persistence_delta: 0,
        candidate_selection: false,
    }
}

/// Minimal neutral output. Cloning copies information and confers no capability.
/// The provenance digest identifies inputs and the supplied execution time; it
/// is not a signature, a nonce, an anti-replay mechanism, or proof of truth.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ZeroNoiTrigger {
    case: CaseIdentity,
    obligation_id: String,
    execution_provenance: Digest32,
    evidence_digest: Digest32,
    evidence: ZeroNoiEvidence,
}

impl ZeroNoiTrigger {
    pub fn case(&self) -> &CaseIdentity { &self.case }
    pub fn obligation_id(&self) -> &str { &self.obligation_id }
    pub const fn execution_provenance(&self) -> Digest32 { self.execution_provenance }
    pub const fn evidence_digest(&self) -> Digest32 { self.evidence_digest }
    pub const fn evidence(&self) -> &ZeroNoiEvidence { &self.evidence }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ZeroNoiRefusal {
    InactiveObligation,
    AlreadyMet,
    InvalidInput(EvidenceError),
    EvidenceRejected(EvidenceError),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", content = "details", rename_all = "snake_case")]
pub enum ZeroNoiOutcome {
    Triggered(ZeroNoiTrigger),
    Refused(ZeroNoiRefusal),
}

/// Deterministic activation relative to the input, time, and trusted verifier.
/// This function has no I/O, mutable global, persistence, or selection access.
/// The supplied clock value is part of the host trust boundary.
pub fn activate(
    input: &ZeroNoiInput,
    verifier: &dyn EvidenceVerifier,
    now: u64,
) -> ZeroNoiOutcome {
    match activate_checked(input, verifier, now) {
        Ok(trigger) => ZeroNoiOutcome::Triggered(trigger),
        Err(reason) => ZeroNoiOutcome::Refused(reason),
    }
}

fn activate_checked(
    input: &ZeroNoiInput,
    verifier: &dyn EvidenceVerifier,
    now: u64,
) -> Result<ZeroNoiTrigger, ZeroNoiRefusal> {
    input.case.validate().map_err(ZeroNoiRefusal::InvalidInput)?;
    if !valid_id(&input.obligation_id) {
        return Err(ZeroNoiRefusal::InvalidInput(EvidenceError::InvalidInput));
    }
    if input.evidence.len() > MAX_REFERENCES {
        return Err(ZeroNoiRefusal::InvalidInput(EvidenceError::ResourceLimit));
    }
    if !input.active {
        return Err(ZeroNoiRefusal::InactiveObligation);
    }
    if !input.unmet {
        return Err(ZeroNoiRefusal::AlreadyMet);
    }
    let claim = requirement_claim(input).map_err(ZeroNoiRefusal::InvalidInput)?;
    let verified = verify_references(
        verifier,
        &input.evidence,
        &input.case,
        EvidenceRole::ConfrontationRequirement,
        claim,
        now,
    );
    // Keep the trigger precondition distinct from its permitted behavior.
    let permitted = trigger_formula(input.active, input.unmet, verified.is_ok());
    verified.map_err(ZeroNoiRefusal::EvidenceRejected)?;
    if !permitted {
        return Err(ZeroNoiRefusal::InvalidInput(EvidenceError::InvalidInput));
    }
    let evidence_digest = digest("ESS-MAI/0-NOI/verified-references/v1", &input.evidence)
        .map_err(ZeroNoiRefusal::InvalidInput)?;
    #[derive(Serialize)]
    struct Execution {
        requirement_claim: Digest32,
        evidence_digest: Digest32,
        executed_at: u64,
    }
    let execution_provenance = digest(
        "ESS-MAI/0-NOI/execution-provenance/v1",
        &Execution { requirement_claim: claim, evidence_digest, executed_at: now },
    ).map_err(ZeroNoiRefusal::InvalidInput)?;
    Ok(ZeroNoiTrigger {
        case: input.case.clone(),
        obligation_id: input.obligation_id.clone(),
        execution_provenance,
        evidence_digest,
        evidence: behavior_formula(),
    })
}

/// Acknowledges receipt by the host's executable Light adapter. This does not
/// certify reasoning, evidence truth, successful closure, or any authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LightAcknowledgement {
    pub case: CaseIdentity,
    pub execution_provenance: Digest32,
}

/// Explicit handoff boundary to a separately implemented Light adapter.
/// This trait does not claim compatibility with an inherited production ABI.
/// Light receives only the neutral trigger and minimal execution provenance.
pub trait LightTriggerConsumer {
    type Error;
    fn receive_trigger(
        &mut self,
        trigger: &ZeroNoiTrigger,
    ) -> Result<LightAcknowledgement, Self::Error>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LightHandoffError<E> {
    Consumer(E),
    CaseMismatch,
    ProvenanceMismatch,
}

/// Forward a neutral trigger to Light and reject an acknowledgement for a
/// different case or invocation. Consumer effects belong to the host, not the
/// pure activation function. The acknowledgement grants no downstream power.
pub fn handoff<L: LightTriggerConsumer + ?Sized>(
    trigger: &ZeroNoiTrigger,
    light: &mut L,
) -> Result<LightAcknowledgement, LightHandoffError<L::Error>> {
    let acknowledgement = light.receive_trigger(trigger).map_err(LightHandoffError::Consumer)?;
    if acknowledgement.case != *trigger.case() {
        return Err(LightHandoffError::CaseMismatch);
    }
    if acknowledgement.execution_provenance != trigger.execution_provenance() {
        return Err(LightHandoffError::ProvenanceMismatch);
    }
    Ok(acknowledgement)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::common::{EvidenceCatalog, EvidenceRecord};
    use std::cell::Cell;
    use std::collections::BTreeSet;

    fn input() -> ZeroNoiInput {
        ZeroNoiInput {
            case: CaseIdentity {
                cycle_id: "cycle-01".into(),
                i0_digest: Digest32::of_bytes(b"i0"),
                scope: "scope-01".into(),
                revision: 1,
                law_digest: Digest32::of_bytes(b"KANUN"),
            },
            obligation_id: "confrontation-01".into(),
            active: true,
            unmet: true,
            evidence: Vec::new(),
        }
    }

    fn record(input: &ZeroNoiInput) -> EvidenceRecord {
        EvidenceRecord {
            record_id: "requirement-01".into(),
            case: input.case.clone(),
            role: EvidenceRole::ConfrontationRequirement,
            issuer: "host-fixture".into(),
            claim_digest: requirement_claim(input).unwrap(),
            issued_at: 10,
            expires_at: 100,
        }
    }

    fn catalog(record: EvidenceRecord) -> EvidenceCatalog {
        EvidenceCatalog::new(vec![record], BTreeSet::from(["host-fixture".into()])).unwrap()
    }

    fn fixture() -> (ZeroNoiInput, EvidenceCatalog) {
        let mut input = input();
        let record = record(&input);
        input.evidence.push(record.reference().unwrap());
        (input, catalog(record))
    }

    fn triggered(outcome: ZeroNoiOutcome) -> ZeroNoiTrigger {
        match outcome {
            ZeroNoiOutcome::Triggered(trigger) => trigger,
            ZeroNoiOutcome::Refused(reason) => panic!("unexpected refusal: {reason:?}"),
        }
    }

    #[test]
    fn trigger_formula_requires_all_three_preconditions() {
        for active in [false, true] {
            for unmet in [false, true] {
                for evidence_valid in [false, true] {
                    assert_eq!(trigger_formula(active, unmet, evidence_valid), active && unmet && evidence_valid);
                }
            }
        }
    }

    #[test]
    fn success_is_a_neutral_trigger_with_zero_other_powers() {
        let (input, catalog) = fixture();
        let trigger = triggered(activate(&input, &catalog, 20));
        assert_eq!(trigger.case(), &input.case);
        assert_eq!(trigger.obligation_id(), input.obligation_id);
        assert!(!trigger.execution_provenance().is_zero());
        assert!(!trigger.evidence_digest().is_zero());
        assert_eq!(*trigger.evidence(), behavior_formula());
        assert!(trigger.evidence().trigger_executed());
        assert_eq!(trigger.evidence().state_delta(), 0);
        assert_eq!(trigger.evidence().knowledge_delta(), 0);
        assert!(!trigger.evidence().verdict_created());
        assert!(!trigger.evidence().authority_created());
        assert_eq!(trigger.evidence().persistence_delta(), 0);
        assert!(!trigger.evidence().candidate_selection());
    }

    #[test]
    fn inactive_requirement_is_refused() {
        let (mut input, catalog) = fixture();
        input.active = false;
        assert_eq!(activate(&input, &catalog, 20), ZeroNoiOutcome::Refused(ZeroNoiRefusal::InactiveObligation));
    }

    #[test]
    fn met_requirement_is_refused() {
        let (mut input, catalog) = fixture();
        input.unmet = false;
        assert_eq!(activate(&input, &catalog, 20), ZeroNoiOutcome::Refused(ZeroNoiRefusal::AlreadyMet));
    }

    #[test]
    fn missing_evidence_cannot_produce_synthetic_success() {
        let (mut input, catalog) = fixture();
        input.evidence.clear();
        assert_eq!(activate(&input, &catalog, 20), ZeroNoiOutcome::Refused(ZeroNoiRefusal::EvidenceRejected(EvidenceError::MissingRecord)));
    }

    #[test]
    fn unknown_record_is_unprovable() {
        let (mut input, catalog) = fixture();
        input.evidence[0].record_id = "not-present".into();
        assert_eq!(activate(&input, &catalog, 20), ZeroNoiOutcome::Refused(ZeroNoiRefusal::EvidenceRejected(EvidenceError::MissingRecord)));
    }

    #[test]
    fn stale_evidence_is_refused_at_expiry_boundary() {
        let (input, catalog) = fixture();
        assert_eq!(activate(&input, &catalog, 100), ZeroNoiOutcome::Refused(ZeroNoiRefusal::EvidenceRejected(EvidenceError::Expired)));
    }

    #[test]
    fn future_evidence_is_refused() {
        let (input, catalog) = fixture();
        assert_eq!(activate(&input, &catalog, 9), ZeroNoiOutcome::Refused(ZeroNoiRefusal::EvidenceRejected(EvidenceError::NotYetValid)));
    }

    #[test]
    fn foreign_case_evidence_is_refused() {
        let (mut input, catalog) = fixture();
        input.case.cycle_id = "different-cycle".into();
        assert_eq!(activate(&input, &catalog, 20), ZeroNoiOutcome::Refused(ZeroNoiRefusal::EvidenceRejected(EvidenceError::CaseMismatch)));
    }

    #[test]
    fn wrong_claim_cannot_establish_the_requirement() {
        let (mut input, catalog) = fixture();
        input.obligation_id = "different-obligation".into();
        assert_eq!(activate(&input, &catalog, 20), ZeroNoiOutcome::Refused(ZeroNoiRefusal::EvidenceRejected(EvidenceError::ClaimMismatch)));
    }

    #[test]
    fn wrong_role_is_refused() {
        let mut input = input();
        let mut evidence = record(&input);
        evidence.role = EvidenceRole::Component;
        input.evidence.push(evidence.reference().unwrap());
        assert_eq!(activate(&input, &catalog(evidence), 20), ZeroNoiOutcome::Refused(ZeroNoiRefusal::EvidenceRejected(EvidenceError::RoleMismatch)));
    }

    #[test]
    fn tampered_evidence_digest_is_refused() {
        let (mut input, catalog) = fixture();
        input.evidence[0].record_digest = Digest32::of_bytes(b"tampered");
        assert_eq!(activate(&input, &catalog, 20), ZeroNoiOutcome::Refused(ZeroNoiRefusal::EvidenceRejected(EvidenceError::DigestMismatch)));
    }

    #[test]
    fn duplicate_and_excessive_references_are_refused() {
        let (mut input, catalog) = fixture();
        input.evidence.push(input.evidence[0].clone());
        assert_eq!(activate(&input, &catalog, 20), ZeroNoiOutcome::Refused(ZeroNoiRefusal::EvidenceRejected(EvidenceError::DuplicateRecord)));
        input.evidence = vec![input.evidence[0].clone(); MAX_REFERENCES + 1];
        assert_eq!(activate(&input, &catalog, 20), ZeroNoiOutcome::Refused(ZeroNoiRefusal::InvalidInput(EvidenceError::ResourceLimit)));
    }

    #[test]
    fn unknown_input_fields_are_rejected() {
        let mut json = serde_json::to_value(input()).unwrap();
        json.as_object_mut().unwrap().insert("authority".into(), serde_json::json!(true));
        assert!(serde_json::from_value::<ZeroNoiInput>(json).is_err());
    }

    #[test]
    fn claim_binds_every_requirement_field_but_not_references() {
        let input = input();
        let original = requirement_claim(&input).unwrap();
        let mut alternatives = Vec::new();
        let mut changed = input.clone(); changed.active = false; alternatives.push(changed);
        let mut changed = input.clone(); changed.unmet = false; alternatives.push(changed);
        let mut changed = input.clone(); changed.obligation_id.push('2'); alternatives.push(changed);
        let mut changed = input.clone(); changed.case.cycle_id.push('2'); alternatives.push(changed);
        let mut changed = input.clone(); changed.case.i0_digest = Digest32::of_bytes(b"i0-2"); alternatives.push(changed);
        let mut changed = input.clone(); changed.case.scope.push('2'); alternatives.push(changed);
        let mut changed = input.clone(); changed.case.revision += 1; alternatives.push(changed);
        let mut changed = input.clone(); changed.case.law_digest = Digest32::of_bytes(b"law-2"); alternatives.push(changed);
        for changed in alternatives { assert_ne!(requirement_claim(&changed).unwrap(), original); }
        let mut changed = input.clone();
        changed.evidence.push(record(&input).reference().unwrap());
        assert_eq!(requirement_claim(&changed).unwrap(), original);
    }

    // This spy deliberately mutates an invocation counter through &self. That
    // counter is test instrumentation, not persistent or epistemic state. It
    // also demonstrates why the foreign-verifier trust boundary is explicit.
    struct CatalogSpy {
        catalog: EvidenceCatalog,
        calls: Cell<usize>,
        knowledge: Vec<String>,
        persistent_records: Vec<String>,
        candidates: Vec<String>,
    }

    impl EvidenceVerifier for CatalogSpy {
        fn verify(&self, reference: &EvidenceRef, case: &CaseIdentity, role: EvidenceRole, claim: Digest32, now: u64) -> Result<(), EvidenceError> {
            self.calls.set(self.calls.get() + 1);
            self.catalog.verify(reference, case, role, claim, now)
        }
    }

    #[test]
    fn repeated_activation_leaves_input_and_host_semantic_state_unchanged() {
        let (input, catalog) = fixture();
        let spy = CatalogSpy {
            catalog,
            calls: Cell::new(0),
            knowledge: vec!["existing knowledge".into()],
            persistent_records: vec!["existing record".into()],
            candidates: vec!["unselected candidate".into()],
        };
        let input_before = input.clone();
        let state_before = (spy.knowledge.clone(), spy.persistent_records.clone(), spy.candidates.clone());
        let first = activate(&input, &spy, 20);
        let second = activate(&input, &spy, 20);
        assert!(matches!(&first, ZeroNoiOutcome::Triggered(_)));
        assert_eq!(first, second);
        assert_eq!(input, input_before);
        assert_eq!((spy.knowledge.clone(), spy.persistent_records.clone(), spy.candidates.clone()), state_before);
        assert_eq!(spy.calls.get(), 2);
        let later = triggered(activate(&input, &spy, 21));
        assert_ne!(triggered(first).execution_provenance(), later.execution_provenance());
    }

    #[test]
    fn inactive_or_met_requirement_does_not_call_the_host() {
        struct MustNotVerify;
        impl EvidenceVerifier for MustNotVerify {
            fn verify(&self, _: &EvidenceRef, _: &CaseIdentity, _: EvidenceRole, _: Digest32, _: u64) -> Result<(), EvidenceError> {
                panic!("inactive/met input must stop before the verifier")
            }
        }
        let (mut input, _) = fixture();
        input.active = false;
        assert!(matches!(activate(&input, &MustNotVerify, 20), ZeroNoiOutcome::Refused(ZeroNoiRefusal::InactiveObligation)));
        input.active = true;
        input.unmet = false;
        assert!(matches!(activate(&input, &MustNotVerify, 20), ZeroNoiOutcome::Refused(ZeroNoiRefusal::AlreadyMet)));
    }

    struct LightStub {
        reply: Result<LightAcknowledgement, &'static str>,
        received: usize,
    }
    impl LightTriggerConsumer for LightStub {
        type Error = &'static str;
        fn receive_trigger(&mut self, _: &ZeroNoiTrigger) -> Result<LightAcknowledgement, Self::Error> {
            self.received += 1;
            self.reply.clone()
        }
    }

    #[test]
    fn handoff_calls_light_and_checks_case_and_execution_provenance() {
        let (input, catalog) = fixture();
        let trigger = triggered(activate(&input, &catalog, 20));
        let acknowledgement = LightAcknowledgement { case: trigger.case().clone(), execution_provenance: trigger.execution_provenance() };
        let mut light = LightStub { reply: Ok(acknowledgement.clone()), received: 0 };
        assert_eq!(handoff(&trigger, &mut light), Ok(acknowledgement.clone()));
        assert_eq!(light.received, 1);
        let mut wrong_case = acknowledgement.clone();
        wrong_case.case.revision += 1;
        light.reply = Ok(wrong_case);
        assert_eq!(handoff(&trigger, &mut light), Err(LightHandoffError::CaseMismatch));
        let mut wrong_provenance = acknowledgement;
        wrong_provenance.execution_provenance = Digest32::of_bytes(b"different invocation");
        light.reply = Ok(wrong_provenance);
        assert_eq!(handoff(&trigger, &mut light), Err(LightHandoffError::ProvenanceMismatch));
        light.reply = Err("light unavailable");
        assert_eq!(handoff(&trigger, &mut light), Err(LightHandoffError::Consumer("light unavailable")));
    }
}
