//! R15: theory tests for the materialized D-BAB operator and its downstream gates
//! (F1 evidence (1): a missing lane or rejected evidence is a Gap, BESA cannot consume a
//! Gap, drift after binding invalidates the obligation, and no step creates a verdict).
use super::common::*;
use super::dbab::*;
use super::downstream::*;
use std::collections::BTreeSet;

const ISSUER: &str = "host-fixture";

fn case() -> CaseIdentity {
    CaseIdentity {
        cycle_id: "cycle-01".into(),
        i0_digest: Digest32::of_bytes(b"i0"),
        scope: "scope-01".into(),
        revision: 1,
        law_digest: Digest32::of_bytes(b"KANUN"),
    }
}

fn component(id: &str, statement: &str, critical: bool) -> Component {
    Component { id: id.into(), statement: statement.into(), critical, evidence: Vec::new() }
}

fn context(id: &str) -> ContextItem {
    ContextItem { id: id.into(), description: format!("context {id}"), state: ContextState::Established, evidence: Vec::new() }
}

fn record(n: &mut usize, role: EvidenceRole, claim: Digest32) -> EvidenceRecord {
    *n += 1;
    EvidenceRecord {
        record_id: format!("rec-{n}"),
        case: case(),
        role,
        issuer: ISSUER.into(),
        claim_digest: claim,
        issued_at: 10,
        expires_at: 100,
    }
}

fn raw_input() -> DbabInput {
    DbabInput {
        case: case(),
        xi: vec![component("xi-1", "claimed component", false)],
        yi: vec![component("yi-1", "claimed outcome", false)],
        negative_xi: vec![component("nxi-1", "excluded component", true)],
        negative_yi: vec![component("nyi-1", "excluded outcome", false)],
        conditions: vec![context("cond-1")],
        circumstances: vec![context("circ-1")],
        consequences: vec![context("cons-1")],
        constraints: vec![
            Constraint { negative_id: "nxi-1".into(), description: "must not hold".into(), condition_ids: vec!["cond-1".into()] },
            Constraint { negative_id: "nyi-1".into(), description: "must not follow".into(), condition_ids: vec!["cond-1".into()] },
        ],
        confrontation_manifest: EvidenceRef { record_id: "manifest-pending".into(), record_digest: Digest32::of_bytes(b"pending") },
    }
}

/// Every component, context and the manifest carries one host-approved record.
fn approve(mut input: DbabInput, records: &mut Vec<EvidenceRecord>, n: &mut usize) -> DbabInput {
    let case = input.case.clone();
    for (lane, components) in [
        (Lane::Xi, &mut input.xi),
        (Lane::Yi, &mut input.yi),
        (Lane::NegativeXi, &mut input.negative_xi),
        (Lane::NegativeYi, &mut input.negative_yi),
    ] {
        for c in components.iter_mut() {
            let r = record(n, EvidenceRole::Component, component_claim(&case, lane, c).unwrap());
            c.evidence = vec![r.reference().unwrap()];
            records.push(r);
        }
    }
    for (family, role, items) in [
        (ContextFamily::Condition, EvidenceRole::Condition, &mut input.conditions),
        (ContextFamily::Circumstance, EvidenceRole::Circumstance, &mut input.circumstances),
        (ContextFamily::Consequence, EvidenceRole::Consequence, &mut input.consequences),
    ] {
        for item in items.iter_mut() {
            let r = record(n, role, context_claim(&case, family, item).unwrap());
            item.evidence = vec![r.reference().unwrap()];
            records.push(r);
        }
    }
    let m = record(n, EvidenceRole::ConfrontationManifest, manifest_claim(&input).unwrap());
    input.confrontation_manifest = m.reference().unwrap();
    records.push(m);
    input
}

fn catalog(records: &[EvidenceRecord]) -> EvidenceCatalog {
    EvidenceCatalog::new(records.to_vec(), BTreeSet::from([ISSUER.to_string()])).unwrap()
}

fn approved() -> (DbabInput, Vec<EvidenceRecord>) {
    let mut records = Vec::new();
    let mut n = 0;
    let input = approve(raw_input(), &mut records, &mut n);
    (input, records)
}

fn bound(outcome: BindingOutcome) -> BoundObligation {
    match outcome {
        BindingOutcome::Bound(b) => b,
        BindingOutcome::Gap(g) => panic!("unexpected gap: {:?}", g.issues),
    }
}

fn gap(outcome: BindingOutcome) -> GapEvidence {
    match outcome {
        BindingOutcome::Gap(g) => g,
        BindingOutcome::Bound(_) => panic!("unexpected binding"),
    }
}

#[test]
fn bound_obligation_comes_only_from_host_approved_evidence_and_arms_nothing() {
    let (input, records) = approved();
    let obligation = bound(bind(input, &catalog(&records), 20));
    let receipt = obligation.receipt();
    assert!(receipt.positive_present && receipt.negative_present);
    assert_eq!(receipt.critical_negative_count, 1);
    assert!(!receipt.downstream_armed);
    assert_eq!(obligation.requirements().len(), 5, "two negative constraints and three contexts");
}

#[test]
fn an_empty_negative_lane_is_a_gap_and_besa_refuses_it() {
    let mut raw = raw_input();
    raw.negative_yi.clear();
    raw.constraints.retain(|c| c.negative_id != "nyi-1");
    let mut records = Vec::new();
    let mut n = 0;
    let input = approve(raw, &mut records, &mut n);
    let current = input.clone();
    let verifier = catalog(&records);
    let outcome = bind(input, &verifier, 20);
    let g = match &outcome {
        BindingOutcome::Gap(g) => g.clone(),
        BindingOutcome::Bound(_) => panic!("a missing negative lane must not bind"),
    };
    assert!(g.issues.iter().any(|i| i.code == BindingIssueCode::EmptyLane && i.path == "negative_yi"));
    assert_eq!(g.question.scope, "scope-01");
    assert!(!g.known_evidence.is_empty(), "supplied provenance is kept for the next question");
    let mut besa = BesaGateway::new(4).unwrap();
    assert_eq!(besa.arm(outcome, &current, &verifier, 20).err(), Some(DownstreamError::BindingFailed));
    let continuation = gap_to_continuation(g);
    assert!(!continuation.final_verdict_created);
    assert!(!continuation.next_i0_exact_content_predicted);
    assert_eq!(continuation.next_i0_scope, "scope-01");
}

#[test]
fn missing_or_tampered_evidence_is_a_named_gap() {
    let (mut input, records) = approved();
    input.xi[0].evidence.clear();
    let g = gap(bind(input, &catalog(&records), 20));
    assert!(g.issues.iter().any(|i| i.code == BindingIssueCode::MissingEvidence && i.path == "xi[0].evidence"));

    let (mut input, records) = approved();
    input.xi[0].statement = "a different claimed component".into();
    let g = gap(bind(input, &catalog(&records), 20));
    assert!(g.issues.iter().any(|i| i.code == BindingIssueCode::EvidenceRejected));

    let (input, records) = approved();
    assert!(matches!(bind(input, &catalog(&records), 100), BindingOutcome::Gap(_)), "expired evidence never binds");
}

#[test]
fn drift_after_binding_invalidates_the_obligation() {
    let (input, records) = approved();
    let verifier = catalog(&records);
    let mut drifted = input.clone();
    drifted.negative_xi[0].statement = "drifted after binding".into();
    let outcome = bind(input, &verifier, 20);
    let mut besa = BesaGateway::new(4).unwrap();
    assert_eq!(besa.arm(outcome, &drifted, &verifier, 20).err(), Some(DownstreamError::ObligationInvalidated));
}

#[test]
fn the_full_chain_reaches_shadow_review_without_any_verdict() {
    let (input, mut records) = approved();
    let current = input.clone();
    let first = catalog(&records);
    let obligation = bound(bind(input, &first, 20));
    let binding_digest = obligation.binding_digest();
    let requirement_ids: Vec<String> = obligation.requirements().iter().map(|r| r.id.clone()).collect();
    let mut n = records.len();
    let mut claims = Vec::new();
    for id in &requirement_ids {
        let mut claim = FulfilmentEvidence { requirement_id: id.clone(), statement: format!("fulfils {id}"), evidence: Vec::new() };
        let r = record(&mut n, EvidenceRole::Fulfilment, fulfilment_claim(&case(), binding_digest, &claim).unwrap());
        claim.evidence = vec![r.reference().unwrap()];
        records.push(r);
        claims.push(claim);
    }
    let verifier = catalog(&records);
    let mut besa = BesaGateway::new(4).unwrap();
    let promise = besa.arm(BindingOutcome::Bound(obligation), &current, &verifier, 20).unwrap();
    let receipt = promise.receipt();
    assert!(receipt.downstream_armed && !receipt.final_verdict_created);
    let job = promise.asht_handoff(&current, &verifier, 20).unwrap();
    assert_eq!(job.requirements().len(), 5);
    let mut incomplete = claims.clone();
    incomplete.pop();
    let job_again = {
        let (input2, _) = approved();
        let mut besa2 = BesaGateway::new(4).unwrap();
        besa2.arm(bind(input2, &verifier, 20), &current, &verifier, 20).unwrap().asht_handoff(&current, &verifier, 20).unwrap()
    };
    assert!(matches!(job_again.prepare_shadow_review(&current, incomplete, &verifier, 20), Err(DownstreamError::MissingRequirement(_))));
    let review = job.prepare_shadow_review(&current, claims, &verifier, 20).unwrap();
    let text = format!("{review:?}");
    assert!(text.contains("AwaitingIndependentVerification"));
    assert!(text.contains("final_verdict_created: false"));
}

#[test]
fn a_cycle_is_armed_once() {
    let (input, records) = approved();
    let current = input.clone();
    let verifier = catalog(&records);
    let mut besa = BesaGateway::new(4).unwrap();
    besa.arm(bind(input, &verifier, 20), &current, &verifier, 20).unwrap();
    let (again, _) = approved();
    assert_eq!(besa.arm(bind(again, &verifier, 20), &current, &verifier, 20).err(), Some(DownstreamError::Replay));
    assert_eq!(besa.armed_cycles(), 1);
}
