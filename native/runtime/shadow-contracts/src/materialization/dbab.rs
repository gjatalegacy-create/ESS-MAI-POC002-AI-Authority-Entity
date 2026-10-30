// Source: ess-mai-dbab-zero-noi/src/dbab.rs (Bledar Gjata / Gjata Legacy,
// Apache-2.0), integrated into AIGovern-Cyber.
// Provenance and adaptations: ../../../SOURCE_NOTES.md.
//! New POC-native D-BAB binding operator, not an inherited source extraction.
//!
//! A digest binds content; the host's `EvidenceVerifier` supplies authority. In
//! particular, a host-approved confrontation manifest commits the full inventory
//! before binding. Rehashing a caller's reduced inventory cannot authorize it.
use super::common::{
    digest, valid_id, valid_text, verify_references, CaseIdentity, Digest32, EvidenceError,
    EvidenceRef, EvidenceRole, EvidenceVerifier, MAX_ITEMS, MAX_REFERENCES, MAX_TEXT_BYTES,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const MAX_ISSUES: usize = 64;
const MAX_KNOWN_REFERENCES: usize = 7 * MAX_ITEMS * MAX_REFERENCES + 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lane { Xi, Yi, NegativeXi, NegativeYi }

impl Lane {
    fn negative(self) -> bool { matches!(self, Self::NegativeXi | Self::NegativeYi) }
    fn name(self) -> &'static str {
        match self { Self::Xi => "xi", Self::Yi => "yi", Self::NegativeXi => "negative_xi", Self::NegativeYi => "negative_yi" }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextFamily { Condition, Circumstance, Consequence }

impl ContextFamily {
    fn name(self) -> &'static str {
        match self { Self::Condition => "condition", Self::Circumstance => "circumstance", Self::Consequence => "consequence" }
    }
    fn role(self) -> EvidenceRole {
        match self { Self::Condition => EvidenceRole::Condition, Self::Circumstance => EvidenceRole::Circumstance, Self::Consequence => EvidenceRole::Consequence }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextState { Established, Unresolved, Contradicted }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    pub id: String,
    pub statement: String,
    pub critical: bool,
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContextItem {
    pub id: String,
    pub description: String,
    pub state: ContextState,
    pub evidence: Vec<EvidenceRef>,
}

/// A constraint is always mandatory; there is no informational-only variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Constraint {
    pub negative_id: String,
    pub description: String,
    pub condition_ids: Vec<String>,
}

/// Raw, untrusted DTO. Deserialize does not grant an obligation capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DbabInput {
    pub case: CaseIdentity,
    pub xi: Vec<Component>,
    pub yi: Vec<Component>,
    pub negative_xi: Vec<Component>,
    pub negative_yi: Vec<Component>,
    pub conditions: Vec<ContextItem>,
    pub circumstances: Vec<ContextItem>,
    pub consequences: Vec<ContextItem>,
    pub constraints: Vec<Constraint>,
    pub confrontation_manifest: EvidenceRef,
}

impl DbabInput {
    fn lanes(&self) -> [(Lane, &[Component]); 4] {
        [(Lane::Xi, &self.xi), (Lane::Yi, &self.yi), (Lane::NegativeXi, &self.negative_xi), (Lane::NegativeYi, &self.negative_yi)]
    }
    fn contexts(&self) -> [(ContextFamily, &[ContextItem]); 3] {
        [(ContextFamily::Condition, &self.conditions), (ContextFamily::Circumstance, &self.circumstances), (ContextFamily::Consequence, &self.consequences)]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum RequirementKind {
    NegativeConstraint { negative_id: String, critical: bool, condition_ids: Vec<String> },
    Context { family: ContextFamily, context_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    pub id: String,
    pub description: String,
    pub kind: RequirementKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingIssueCode {
    InvalidIdentity, ResourceLimit, InvalidId, InvalidText, EmptyLane, DuplicateId,
    InvalidCriticalFlag, MissingEvidence, DuplicateReference, EvidenceRejected,
    MissingContext, UnresolvedContext, ContradictedContext, UnknownNegative,
    DuplicateConstraint, MissingConstraint, MissingCriticalConstraint,
    EmptyConditionReferences, UnknownCondition, DuplicateConditionReference,
    ManifestRejected, CaseDrift, ContextDrift, InputDrift, InternalEncoding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BindingIssue {
    pub code: BindingIssueCode,
    pub path: String,
    pub evidence_error: Option<EvidenceError>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScopedEvidenceQuestion {
    pub scope: String,
    pub prompt: String,
    pub issue_code: BindingIssueCode,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GapEvidence {
    pub case: CaseIdentity,
    pub known_evidence: Vec<EvidenceRef>,
    pub issues: Vec<BindingIssue>,
    pub question: ScopedEvidenceQuestion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingResult { Bound }

/// A report, not a downstream capability or final epistemic verdict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BindingReceipt {
    pub cycle_id: String,
    pub positive_present: bool,
    pub negative_present: bool,
    pub xi_count: usize,
    pub yi_count: usize,
    pub negative_xi_count: usize,
    pub negative_yi_count: usize,
    pub conditions_bound: bool,
    pub circumstances_bound: bool,
    pub consequences_bound: bool,
    pub critical_negative_count: usize,
    pub binding_result: BindingResult,
    pub binding_digest: Digest32,
    pub context_digest: Digest32,
    pub downstream_armed: bool,
}

/// Immutable, non-Clone, non-Deserialize capability. Only `bind` constructs it.
/// Downstream consumers take ownership when promoting it to the next stage.
///
/// ```compile_fail
/// use shadow_contracts::materialization::dbab::BoundObligation;
/// let forged: BoundObligation = serde_json::from_str("{}").unwrap();
/// ```
#[derive(Debug)]
pub struct BoundObligation {
    input: DbabInput,
    binding_digest: Digest32,
    context_digest: Digest32,
    requirements: Vec<Requirement>,
}

#[derive(Debug)]
pub enum BindingOutcome { Bound(BoundObligation), Gap(GapEvidence) }

impl BoundObligation {
    pub fn input(&self) -> &DbabInput { &self.input }
    pub fn case(&self) -> &CaseIdentity { &self.input.case }
    pub fn binding_digest(&self) -> Digest32 { self.binding_digest }
    pub fn context_digest(&self) -> Digest32 { self.context_digest }
    pub fn requirements(&self) -> &[Requirement] { &self.requirements }

    pub fn receipt(&self) -> BindingReceipt {
        BindingReceipt {
            cycle_id: self.input.case.cycle_id.clone(),
            positive_present: !self.input.xi.is_empty() && !self.input.yi.is_empty(),
            negative_present: !self.input.negative_xi.is_empty() && !self.input.negative_yi.is_empty(),
            xi_count: self.input.xi.len(), yi_count: self.input.yi.len(),
            negative_xi_count: self.input.negative_xi.len(), negative_yi_count: self.input.negative_yi.len(),
            conditions_bound: !self.input.conditions.is_empty(),
            circumstances_bound: !self.input.circumstances.is_empty(),
            consequences_bound: !self.input.consequences.is_empty(),
            critical_negative_count: self.input.negative_xi.iter().chain(&self.input.negative_yi).filter(|x| x.critical).count(),
            binding_result: BindingResult::Bound, binding_digest: self.binding_digest,
            context_digest: self.context_digest, downstream_armed: false,
        }
    }

    /// Rechecks all provenance against the current host and time. Any changed raw
    /// value or reference requires a new explicit binding; nothing is rewritten.
    pub fn validate_for(&self, current: &DbabInput, verifier: &dyn EvidenceVerifier, now: u64) -> Result<(), GapEvidence> {
        let mut issues = validate_input(current, verifier, now);
        if issues.is_empty() && current != &self.input {
            let code = if current.case != self.input.case {
                BindingIssueCode::CaseDrift
            } else if context_digest(current).ok() != Some(self.context_digest) {
                BindingIssueCode::ContextDrift
            } else { BindingIssueCode::InputDrift };
            push_issue(&mut issues, code, "input", None);
        }
        if issues.is_empty() { Ok(()) } else {
            // The retained identity is the bounded original obligation's identity.
            // The caller's possibly oversized identity is never cloned here.
            Err(make_gap(self.input.case.clone(), known_evidence(current), issues))
        }
    }
}

/// Exact content claim: changing the lane, case, id, text or criticality requires
/// fresh host-approved evidence. References are intentionally not self-hashed.
pub fn component_claim(case: &CaseIdentity, lane: Lane, component: &Component) -> Result<Digest32, EvidenceError> {
    case.validate()?;
    if !valid_id(&component.id) || !valid_text(&component.statement) || (component.critical && !lane.negative()) {
        return Err(EvidenceError::InvalidInput);
    }
    digest("ESS-MAI/D-BAB/component/v1", &(case, lane, &component.id, &component.statement, component.critical))
}

pub fn context_claim(case: &CaseIdentity, family: ContextFamily, item: &ContextItem) -> Result<Digest32, EvidenceError> {
    case.validate()?;
    if !valid_id(&item.id) || !valid_text(&item.description) { return Err(EvidenceError::InvalidInput); }
    digest("ESS-MAI/D-BAB/context-item/v1", &(case, family, &item.id, &item.description, item.state))
}

/// The manifest includes the entire ordered confrontation inventory, context,
/// constraints and their evidence references. It excludes its own reference to
/// avoid circular hashing. The host must approve this claim independently.
pub fn manifest_claim(input: &DbabInput) -> Result<Digest32, EvidenceError> {
    bounded_shape(input)?;
    digest("ESS-MAI/D-BAB/confrontation-manifest/v1", &(
        &input.case, &input.xi, &input.yi, &input.negative_xi, &input.negative_yi,
        &input.conditions, &input.circumstances, &input.consequences, &input.constraints,
    ))
}

pub fn context_digest(input: &DbabInput) -> Result<Digest32, EvidenceError> {
    bounded_shape(input)?;
    digest("ESS-MAI/D-BAB/context/v1", &(
        &input.case, &input.conditions, &input.circumstances, &input.consequences, &input.constraints,
    ))
}

/// The only authority-producing D-BAB operation. Invalid input produces a gap
/// with retained known provenance, never a synthetic obligation.
pub fn bind(input: DbabInput, verifier: &dyn EvidenceVerifier, now: u64) -> BindingOutcome {
    let mut issues = validate_input(&input, verifier, now);
    if !issues.is_empty() {
        let refs = known_evidence(&input);
        return BindingOutcome::Gap(make_gap(input.case, refs, issues));
    }
    let requirements = build_requirements(&input);
    let hashes = context_digest(&input).and_then(|context_hash| {
        digest("ESS-MAI/D-BAB/bound-obligation/v1", &(&input, &requirements))
            .map(|binding_hash| (context_hash, binding_hash))
    });
    match hashes {
        Ok((context_hash, binding_hash)) => BindingOutcome::Bound(BoundObligation {
            input, binding_digest: binding_hash, context_digest: context_hash, requirements,
        }),
        Err(error) => {
            push_issue(&mut issues, BindingIssueCode::InternalEncoding, "input", Some(error));
            let refs = known_evidence(&input);
            BindingOutcome::Gap(make_gap(input.case, refs, issues))
        }
    }
}

/// Size-check before hashing, maps, clones, or calls to host code. Helpers also
/// enforce this bound so a raw manifest request cannot allocate unlimited JSON.
fn bounded_shape(input: &DbabInput) -> Result<(), EvidenceError> {
    if input.case.cycle_id.len() > 128 || input.case.scope.len() > 128
        || input.confrontation_manifest.record_id.len() > 128 || input.constraints.len() > MAX_ITEMS * 2 {
        return Err(EvidenceError::ResourceLimit);
    }
    for (_, lane) in input.lanes() {
        if lane.len() > MAX_ITEMS { return Err(EvidenceError::ResourceLimit); }
        for component in lane {
            if component.id.len() > 128 || component.statement.len() > MAX_TEXT_BYTES { return Err(EvidenceError::ResourceLimit); }
            bounded_refs(&component.evidence)?;
        }
    }
    for (_, items) in input.contexts() {
        if items.len() > MAX_ITEMS { return Err(EvidenceError::ResourceLimit); }
        for item in items {
            if item.id.len() > 128 || item.description.len() > MAX_TEXT_BYTES { return Err(EvidenceError::ResourceLimit); }
            bounded_refs(&item.evidence)?;
        }
    }
    for constraint in &input.constraints {
        if constraint.negative_id.len() > 128 || constraint.description.len() > MAX_TEXT_BYTES
            || constraint.condition_ids.len() > MAX_ITEMS || constraint.condition_ids.iter().any(|id| id.len() > 128) {
            return Err(EvidenceError::ResourceLimit);
        }
    }
    Ok(())
}

fn bounded_refs(refs: &[EvidenceRef]) -> Result<(), EvidenceError> {
    if refs.len() > MAX_REFERENCES || refs.iter().any(|r| r.record_id.len() > 128) {
        Err(EvidenceError::ResourceLimit)
    } else { Ok(()) }
}

fn push_issue(issues: &mut Vec<BindingIssue>, code: BindingIssueCode, path: impl Into<String>, evidence_error: Option<EvidenceError>) {
    if issues.len() < MAX_ISSUES { issues.push(BindingIssue { code, path: path.into(), evidence_error }); }
}

fn check_refs<'a>(refs: &'a [EvidenceRef], path: &str, seen: &mut BTreeSet<&'a str>, issues: &mut Vec<BindingIssue>) {
    if refs.is_empty() { push_issue(issues, BindingIssueCode::MissingEvidence, path, Some(EvidenceError::MissingRecord)); }
    for (index, reference) in refs.iter().enumerate() {
        if !valid_id(&reference.record_id) || reference.record_digest.is_zero() {
            push_issue(issues, BindingIssueCode::EvidenceRejected, format!("{path}[{index}]"), Some(EvidenceError::InvalidRecord));
        }
        if !seen.insert(&reference.record_id) {
            push_issue(issues, BindingIssueCode::DuplicateReference, format!("{path}[{index}]"), Some(EvidenceError::DuplicateRecord));
        }
    }
}

fn validate_input(input: &DbabInput, verifier: &dyn EvidenceVerifier, now: u64) -> Vec<BindingIssue> {
    let mut issues = Vec::new();
    if let Err(error) = bounded_shape(input) {
        push_issue(&mut issues, BindingIssueCode::ResourceLimit, "input", Some(error));
        return issues;
    }
    if let Err(error) = input.case.validate() {
        push_issue(&mut issues, BindingIssueCode::InvalidIdentity, "case", Some(error));
    }
    let mut ids = BTreeSet::new();
    let mut references = BTreeSet::new();
    let mut negatives = BTreeMap::new();
    for (lane, components) in input.lanes() {
        if components.is_empty() { push_issue(&mut issues, BindingIssueCode::EmptyLane, lane.name(), None); }
        for (index, component) in components.iter().enumerate() {
            let path = format!("{}[{index}]", lane.name());
            if !valid_id(&component.id) { push_issue(&mut issues, BindingIssueCode::InvalidId, format!("{path}.id"), None); }
            if !valid_text(&component.statement) { push_issue(&mut issues, BindingIssueCode::InvalidText, format!("{path}.statement"), None); }
            if !ids.insert(component.id.as_str()) { push_issue(&mut issues, BindingIssueCode::DuplicateId, format!("{path}.id"), None); }
            if component.critical && !lane.negative() { push_issue(&mut issues, BindingIssueCode::InvalidCriticalFlag, format!("{path}.critical"), None); }
            if lane.negative() { negatives.insert(component.id.as_str(), component.critical); }
            check_refs(&component.evidence, &format!("{path}.evidence"), &mut references, &mut issues);
        }
    }
    for (family, items) in input.contexts() {
        if items.is_empty() { push_issue(&mut issues, BindingIssueCode::MissingContext, family.name(), None); }
        for (index, item) in items.iter().enumerate() {
            let path = format!("{}[{index}]", family.name());
            if !valid_id(&item.id) { push_issue(&mut issues, BindingIssueCode::InvalidId, format!("{path}.id"), None); }
            if !valid_text(&item.description) { push_issue(&mut issues, BindingIssueCode::InvalidText, format!("{path}.description"), None); }
            if !ids.insert(item.id.as_str()) { push_issue(&mut issues, BindingIssueCode::DuplicateId, format!("{path}.id"), None); }
            match item.state {
                ContextState::Established => {},
                ContextState::Unresolved => push_issue(&mut issues, BindingIssueCode::UnresolvedContext, format!("{path}.state"), None),
                ContextState::Contradicted => push_issue(&mut issues, BindingIssueCode::ContradictedContext, format!("{path}.state"), None),
            }
            check_refs(&item.evidence, &format!("{path}.evidence"), &mut references, &mut issues);
        }
    }
    let condition_ids: BTreeSet<&str> = input.conditions.iter().map(|c| c.id.as_str()).collect();
    let mut covered_negatives = BTreeSet::new();
    for (index, constraint) in input.constraints.iter().enumerate() {
        let path = format!("constraints[{index}]");
        if !valid_id(&constraint.negative_id) { push_issue(&mut issues, BindingIssueCode::InvalidId, format!("{path}.negative_id"), None); }
        if !valid_text(&constraint.description) { push_issue(&mut issues, BindingIssueCode::InvalidText, format!("{path}.description"), None); }
        if !negatives.contains_key(constraint.negative_id.as_str()) { push_issue(&mut issues, BindingIssueCode::UnknownNegative, format!("{path}.negative_id"), None); }
        if !covered_negatives.insert(constraint.negative_id.as_str()) { push_issue(&mut issues, BindingIssueCode::DuplicateConstraint, path.clone(), None); }
        if constraint.condition_ids.is_empty() { push_issue(&mut issues, BindingIssueCode::EmptyConditionReferences, format!("{path}.condition_ids"), None); }
        let mut seen_conditions = BTreeSet::new();
        for (condition_index, id) in constraint.condition_ids.iter().enumerate() {
            let condition_path = format!("{path}.condition_ids[{condition_index}]");
            if !valid_id(id) { push_issue(&mut issues, BindingIssueCode::InvalidId, &condition_path, None); }
            if !condition_ids.contains(id.as_str()) { push_issue(&mut issues, BindingIssueCode::UnknownCondition, &condition_path, None); }
            if !seen_conditions.insert(id) { push_issue(&mut issues, BindingIssueCode::DuplicateConditionReference, &condition_path, None); }
        }
    }
    for (id, critical) in negatives {
        if !covered_negatives.contains(id) {
            push_issue(&mut issues, if critical { BindingIssueCode::MissingCriticalConstraint } else { BindingIssueCode::MissingConstraint }, format!("constraints/{id}"), None);
        }
    }
    check_refs(std::slice::from_ref(&input.confrontation_manifest), "confrontation_manifest", &mut references, &mut issues);
    // Do not ask a host to approve malformed or incomplete claims.
    if !issues.is_empty() { return issues; }

    for (lane, components) in input.lanes() {
        for (index, component) in components.iter().enumerate() {
            let verified = component_claim(&input.case, lane, component).and_then(|claim| {
                verify_references(verifier, &component.evidence, &input.case, EvidenceRole::Component, claim, now)
            });
            if let Err(error) = verified {
                push_issue(&mut issues, BindingIssueCode::EvidenceRejected, format!("{}[{index}].evidence", lane.name()), Some(error));
            }
        }
    }
    for (family, items) in input.contexts() {
        for (index, item) in items.iter().enumerate() {
            let verified = context_claim(&input.case, family, item).and_then(|claim| {
                verify_references(verifier, &item.evidence, &input.case, family.role(), claim, now)
            });
            if let Err(error) = verified {
                push_issue(&mut issues, BindingIssueCode::EvidenceRejected, format!("{}[{index}].evidence", family.name()), Some(error));
            }
        }
    }
    let verified = manifest_claim(input).and_then(|claim| {
        verify_references(verifier, std::slice::from_ref(&input.confrontation_manifest), &input.case, EvidenceRole::ConfrontationManifest, claim, now)
    });
    if let Err(error) = verified { push_issue(&mut issues, BindingIssueCode::ManifestRejected, "confrontation_manifest", Some(error)); }
    issues
}

fn build_requirements(input: &DbabInput) -> Vec<Requirement> {
    let negatives: BTreeMap<&str, bool> = input.negative_xi.iter().chain(&input.negative_yi).map(|c| (c.id.as_str(), c.critical)).collect();
    let mut requirements = Vec::new();
    for constraint in &input.constraints {
        requirements.push(Requirement {
            id: format!("negative:{}", constraint.negative_id), description: constraint.description.clone(),
            kind: RequirementKind::NegativeConstraint {
                negative_id: constraint.negative_id.clone(),
                critical: negatives.get(constraint.negative_id.as_str()).copied().unwrap_or(false),
                condition_ids: constraint.condition_ids.clone(),
            },
        });
    }
    for (family, items) in input.contexts() {
        for item in items {
            requirements.push(Requirement {
                id: format!("context:{}:{}", family.name(), item.id),
                description: item.description.clone(),
                kind: RequirementKind::Context { family, context_id: item.id.clone() },
            });
        }
    }
    requirements
}

/// Preserve bounded, well-formed references even for oversized/invalid raw input.
/// Presence here records what was supplied, not a successful verification claim.
fn known_evidence(input: &DbabInput) -> Vec<EvidenceRef> {
    let mut known = BTreeMap::<&str, &EvidenceRef>::new();
    fn retain<'a>(known: &mut BTreeMap<&'a str, &'a EvidenceRef>, reference: &'a EvidenceRef) {
        if known.len() < MAX_KNOWN_REFERENCES && valid_id(&reference.record_id) && !reference.record_digest.is_zero() {
            known.entry(reference.record_id.as_str()).or_insert(reference);
        }
    }
    retain(&mut known, &input.confrontation_manifest);
    for (_, lane) in input.lanes() {
        for component in lane.iter().take(MAX_ITEMS) {
            for reference in component.evidence.iter().take(MAX_REFERENCES) { retain(&mut known, reference); }
        }
    }
    for (_, items) in input.contexts() {
        for item in items.iter().take(MAX_ITEMS) {
            for reference in item.evidence.iter().take(MAX_REFERENCES) { retain(&mut known, reference); }
        }
    }
    known.into_values().cloned().collect()
}

fn make_gap(case: CaseIdentity, known_evidence: Vec<EvidenceRef>, issues: Vec<BindingIssue>) -> GapEvidence {
    let first = issues.first();
    let code = first.map_or(BindingIssueCode::InternalEncoding, |issue| issue.code);
    let path = first.map_or_else(|| "input".to_string(), |issue| issue.path.clone());
    let scope = if valid_id(&case.scope) { case.scope.clone() } else { "invalid-case-identity".to_string() };
    let prompt = format!("For scope {scope}, what verified evidence resolves {code:?} at {path} before a new i0 and explicit binding?");
    GapEvidence { case, known_evidence, issues, question: ScopedEvidenceQuestion { scope, prompt, issue_code: code, path } }
}
