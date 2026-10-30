//! NEW R3 KiE origin acquisition. This is a subordinate evidence adapter:
//! actual PDS1 Vault data -> pinned model -> original stored ASHT hold.
//! NBES/VDS still issue their own fresh attestations and final decisions.
use crate::{selection_hold::SelectionHoldStore, KnowledgeVault};
use shadow_contracts::{
    kie::*, pd_memory, KnowledgeSelectionResponseWire, SelectionRequester, ShadowCycleRequest,
};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub(crate) fn validate_proof_model(proof: &KieProof) -> Result<(), String> {
    proof.validate()?;
    let actual = science_domains::kie::evaluate_kie(&proof.query_bytes, &proof.candidate_bytes)
        .map_err(|e| format!("KIE_MODEL:{e}"))?;
    if proof.canonical_proposition != actual.canonical_proposition
        || proof.relation != actual.relation
        || proof.kis_mass != actual.kis_mass
        || proof.kic_mass != actual.kic_mass
    {
        return Err("KIE_MODEL_PREDICATE_MISMATCH".into());
    }
    let query =
        science_domains::decode_query(&proof.query_bytes).map_err(|e| format!("KIE_QUERY:{e}"))?;
    let id = &proof.context.identity;
    if query.session_id != pd_memory::hex(&id.key()?)
        || query.case_id != id.case_id
        || query.scope_id != id.scope
        || query.obligation_id != id.obligation
    {
        return Err("KIE_SOURCE_QUERY_SCOPE".into());
    }
    Ok(())
}
/// Uses a record obtained from the ACTUAL Vault. Public hashes alone never
/// establish this origin; caller-supplied relevance prose is not consumed.
fn proof_from_record(
    context: &KieContext,
    record: &pd_memory::PdOperationRecord,
    request: &shadow_contracts::KnowledgeSelectionRequestWire,
    response: &KnowledgeSelectionResponseWire,
) -> Result<KieProof, String> {
    record.validate()?;
    if record.record_id != context.record_id
        || record.request.identity != context.identity
        || record.request.operation_id != context.operation_id
    {
        return Err("KIE_DURABLE_SOURCE_MISMATCH".into());
    }
    pd_memory::linked_science::validate_science_record(record)?;
    let science_query = pd_memory::linked_science::science_query_from_source(&record.request.source_input)?;
    let candidate = science_domains::decode_candidate(&record.request.candidate)
        .map_err(|e| format!("KIE_SOURCE_CANDIDATE:{e}"))?;
    if candidate.query_sha256 != record.request.candidate_key
        || science_domains::encode_query(&candidate.query)
            .map_err(|e| format!("KIE_SOURCE_QUERY:{e}"))?
            != science_query
    {
        return Err("KIE_SOURCE_QUERY_CANDIDATE_BINDING".into());
    }
    let actual =
        science_domains::kie::evaluate_kie(&science_query, &record.request.candidate)
            .map_err(|e| format!("KIE_SOURCE_MODEL:{e}"))?;
    let proof = KieProof {
        context: context.clone(),
        query_bytes: science_query,
        candidate_bytes: record.request.candidate.clone(),
        canonical_proposition: actual.canonical_proposition,
        historical_request: shadow_contracts::encode_selection_request(request),
        historical_response: shadow_contracts::encode_selection_response(response),
        kis_mass: actual.kis_mass,
        kic_mass: actual.kic_mass,
        relation: actual.relation,
        proof_sha256: [0; 32],
        prior_adjudication: None,
        history_assessment: None,
    }
    .seal_data()?;
    validate_proof_model(&proof)?;
    Ok(proof)
}
pub(crate) fn resolve_proof(
    vault: &KnowledgeVault,
    context: &KieContext,
    request: &shadow_contracts::KnowledgeSelectionRequestWire,
    response: &KnowledgeSelectionResponseWire,
) -> Result<KieProof, String> {
    context.validate()?;
    let record = vault
        .pd_operation(&context.identity, &context.operation_id)
        .map_err(|e| format!("KIE_VAULT_LOOKUP:{e:?}"))?
        .ok_or("KIE_DURABLE_SOURCE_MISSING")?;
    let mut proof=proof_from_record(context, &record, request, response)?;
    proof.history_assessment=Some(vault.kie_history_assessment(&proof)?);
    proof.seal_data()
}
fn staged_path(root: &Path, id: &str) -> Result<PathBuf, String> {
    if !id.starts_with("kie-")
        || id.len() > 80
        || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err("KIE_HOLD_ID".into());
    }
    Ok(root.join("selection_holds").join(format!("{id}.kie")))
}
fn stage_bytes(q: &KieSelectionRequest, s: &KieSelectionResponse) -> Result<Vec<u8>, String> {
    let qb = encode_request(q)?;
    let sb = encode_response(s)?;
    let mut bytes = b"KIH1".to_vec();
    bytes.extend_from_slice(&(qb.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&qb);
    bytes.extend_from_slice(&sb);
    Ok(bytes)
}
fn decode_staged(bytes: &[u8]) -> Result<(KieSelectionRequest, KieSelectionResponse), String> {
    if bytes.len() > 2 * MAX_WIRE_BYTES + 8 || !bytes.starts_with(b"KIH1") {
        return Err("KIE_STAGE_VERSION_LIMIT".into());
    }
    let n = u32::from_le_bytes(
        bytes
            .get(4..8)
            .ok_or("KIE_STAGE_TRUNCATED")?
            .try_into()
            .map_err(|_| "KIE_STAGE_LENGTH")?,
    ) as usize;
    if n > MAX_WIRE_BYTES {
        return Err("KIE_STAGE_LENGTH_LIMIT".into());
    }
    let end = 8usize.checked_add(n).ok_or("KIE_STAGE_OVERFLOW")?;
    let q = decode_request(bytes.get(8..end).ok_or("KIE_STAGE_TRUNCATED")?)?;
    let s = decode_response(bytes.get(end..).ok_or("KIE_STAGE_TRUNCATED")?)?;
    s.validate_for(&q)?;
    Ok((q, s))
}
fn publish_staged(
    root: &Path,
    q: &KieSelectionRequest,
    s: &KieSelectionResponse,
) -> Result<(), String> {
    let target = staged_path(root, &s.hold.hold_id)?;
    fs::create_dir_all(target.parent().ok_or("KIE_STAGE_PARENT")?)
        .map_err(|e| format!("KIE_STAGE_DIRECTORY:{e}"))?;
    let temp = target.with_extension(format!("{}.tmp", std::process::id()));
    let bytes = stage_bytes(q, s)?;
    let mut f = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)
        .map_err(|e| format!("KIE_STAGE_OPEN:{e}"))?;
    f.write_all(&bytes)
        .and_then(|_| f.sync_all())
        .map_err(|e| format!("KIE_STAGE_SYNC:{e}"))?;
    drop(f);
    // No-clobber publication. Existing files never become silently overwritten.
    fs::hard_link(&temp, &target).map_err(|e| format!("KIE_STAGE_PUBLISH:{e}"))?;
    fs::remove_file(&temp).map_err(|e| format!("KIE_STAGE_TEMP_CLEANUP:{e}"))?;
    Ok(())
}
/// Called only by Shadow's mediated main/lib selection endpoint. The store
/// receives a genuinely staged hold, not invented historical P/N statistics.
pub(crate) fn select(
    vault: &KnowledgeVault,
    q: &KieSelectionRequest,
    handoff_root: &Path,
    now_ns: u64,
) -> Result<KieSelectionResponse, String> {
    q.validate()?;
    let proof = attach_current_adjudication(
        vault,
        resolve_proof(vault, &q.context, &q.selection, &q.historical_response)?,
    )?;
    if q.input_text != proof.canonical_proposition {
        return Err("KIE_EXACT_PROPOSITION_REQUIRED".into());
    }
    let store = SelectionHoldStore::under_handoff(handoff_root);
    let historical = store.load_bound(
        &q.historical_response.hold_id,
        &q.selection.session_id,
        q.selection.primitive_anchor,
        &q.selection.split_sha256,
        &q.historical_response.hold_sha256,
    )?;
    if historical != q.historical_response
        || historical.law_seal != crate::lab_contracts::gjata_collapse_law::law_seal()
        || historical.system_laws_seal != crate::lab_contracts::system_laws_seal()
    {
        return Err("KIE_ACTUAL_HISTORICAL_HOLD_MISMATCH".into());
    }
    if historical.expires_at_ns < now_ns {
        return Err("KIE_HISTORICAL_HOLD_EXPIRED".into());
    }
    // The actual predecessor BESA hold must exist in this native attempt.
    // Its ID is not in the ASHT request; NBES will bind both exact IDs later.
    let request_sha256 = shadow_contracts::sha256_hex(&[
        b"ESSMAI/SELECTION/REQUEST/V178",
        &shadow_contracts::encode_selection_request(&q.selection),
    ]);
    let selection_sha256 = proof.selection_sha256();
    let id = shadow_contracts::sha256_hex(&[
        b"ESSMAI/KIE/HOLD-ID/1",
        request_sha256.as_bytes(),
        selection_sha256.as_bytes(),
    ]);
    let mut hold = KnowledgeSelectionResponseWire {
        version: shadow_contracts::PROTOCOL_VERSION,
        requester: SelectionRequester::AshtQuantum,
        session_id: q.selection.session_id.clone(),
        parent_i0: q.selection.parent_i0.clone(),
        primitive_anchor: q.selection.primitive_anchor,
        split_sha256: q.selection.split_sha256.clone(),
        upstream_hold_sha256: q.selection.upstream_hold_sha256.clone(),
        request_sha256,
        selection_sha256,
        hold_id: format!("kie-{}", &id[..32]),
        hold_sha256: String::new(),
        positive: Vec::new(),
        negative: Vec::new(),
        law_seal: crate::lab_contracts::gjata_collapse_law::law_seal(),
        system_laws_seal: crate::lab_contracts::system_laws_seal(),
        expires_at_ns: now_ns
            .checked_add(15 * 60 * 1_000_000_000)
            .ok_or("KIE_HOLD_EXPIRY_OVERFLOW")?
            .min(historical.expires_at_ns),
    };
    hold.hold_sha256 = SelectionHoldStore::recompute_hold_sha256(&hold);
    let result = KieSelectionResponse { hold, proof };
    result.validate_for(q)?;
    // Sidecar first, then the original store's durable hold. A partial failure
    // leaves no successful hold response. No epistemic store has been written.
    publish_staged(handoff_root, q, &result)?;
    let receipt = store.stage(&result.hold)?;
    if receipt.hold_id != result.hold.hold_id
        || receipt.hold_sha256 != result.hold.hold_sha256
        || !receipt.record_path.exists()
    {
        return Err("KIE_HOLD_STAGE_RECEIPT".into());
    }
    Ok(result)
}
/// Pre-admission data check for a NEW native cycle. Exact durable retries must
/// return before this ephemeral-hold check; they do not acquire new authority.
pub(crate) fn verify_cycle(
    vault: &KnowledgeVault,
    cycle: &ShadowCycleRequest,
    handoff_root: &Path,
) -> Result<Option<KieProof>, String> {
    let evidence = &cycle.quantum.final_evidence;
    let muscle = &evidence.muscle_evidence;
    if !muscle.asht_hold_id.starts_with("kie-") {
        return Ok(None);
    }
    let path = staged_path(handoff_root, &muscle.asht_hold_id)?;
    let f = OpenOptions::new()
        .read(true)
        .open(path)
        .map_err(|e| format!("KIE_STAGE_READ:{e}"))?;
    let mut bytes = Vec::new();
    f.take((2 * MAX_WIRE_BYTES + 9) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("KIE_STAGE_READ:{e}"))?;
    let (q, s) = decode_staged(&bytes)?;
    if cycle.quantum.session_id != q.selection.session_id
        || cycle.light.session_id != q.selection.session_id
        || evidence.light_input_bytes != q.input_text.as_bytes()
        || evidence.light_input_sha256 != hex(&digest(q.input_text.as_bytes()))
        || muscle.asht_hold_sha256 != s.hold.hold_sha256
        || muscle.asht_selection_sha256 != s.hold.selection_sha256
        || muscle.split_sha256 != q.selection.split_sha256
        || q.selection.upstream_hold_sha256.as_deref() != Some(muscle.besa_hold_sha256.as_str())
    {
        return Err("KIE_CYCLE_SCOPE".into());
    }
    let store = SelectionHoldStore::under_handoff(handoff_root);
    let historical = store.load_bound(
        &q.historical_response.hold_id,
        &q.selection.session_id,
        q.selection.primitive_anchor,
        &q.selection.split_sha256,
        &q.historical_response.hold_sha256,
    )?;
    if historical != q.historical_response
        || historical.law_seal != crate::lab_contracts::gjata_collapse_law::law_seal()
        || historical.system_laws_seal != crate::lab_contracts::system_laws_seal()
    {
        return Err("KIE_ACTUAL_HISTORICAL_HOLD_MISMATCH".into());
    }
    let actual_hold = store.load_bound(
        &s.hold.hold_id,
        &q.selection.session_id,
        q.selection.primitive_anchor,
        &q.selection.split_sha256,
        &s.hold.hold_sha256,
    )?;
    if actual_hold != s.hold {
        return Err("KIE_ACTUAL_HOLD_MISMATCH".into());
    }
    let actual = attach_current_adjudication(
        vault,
        resolve_proof(vault, &q.context, &q.selection, &q.historical_response)?,
    )?;
    if actual != s.proof {
        return Err("KIE_DURABLE_PROOF_MISMATCH".into());
    }
    validate_proposal_binding(
        &actual,
        cycle.quantum.xy_x.is_some()
            || cycle.quantum.xy_y.is_some()
            || cycle.quantum.xy_mass.is_some(),
        cycle.quantum.collapse_refused.is_some(),
    )?;
    Ok(Some(actual))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn context() -> KieContext {
        KieContext {
            identity: pd_memory::PdIdentity {
                owner: "owner".into(),
                obligation: "model".into(),
                case_id: "case".into(),
                scope: "scope".into(),
                domain: "science".into(),
                lineage_root: "root".into(),
            },
            operation_id: "op".into(),
            record_id: [3; 32],
        }
    }
    #[test]
    fn self_consistent_hashes_do_not_replace_a_real_vault_source() {
        let vault = KnowledgeVault::new();
        let (q, s) = test_history("model");
        let q = shadow_contracts::decode_selection_request(&q).unwrap();
        let s = shadow_contracts::decode_selection_response(&s).unwrap();
        assert_eq!(
            resolve_proof(&vault, &context(), &q, &s).unwrap_err(),
            "KIE_DURABLE_SOURCE_MISSING"
        );
    }
    #[test]
    fn actual_model_rejects_a_relabelled_or_fabricated_predicate() {
        let c = context();
        let q = science_domains::ScienceQuery {
            adapter: science_domains::AdapterId::Astronomy,
            session_id: pd_memory::hex(&c.identity.key().unwrap()),
            case_id: c.identity.case_id.clone(),
            scope_id: c.identity.scope.clone(),
            obligation_id: c.identity.obligation.clone(),
            query_id: "query".into(),
            input_source_id: "caller".into(),
            task: science_domains::ScienceTask::EarthRotationAngle {
                jd_part1: 2451545.0,
                jd_part2: 0.0,
                time_scale: science_domains::TimeScale::Ut1,
                claim_radians: 0.0,
                tolerance_radians: 0.1,
            },
        };
        let qb = science_domains::encode_query(&q).unwrap();
        let cb =
            science_domains::encode_candidate(&science_domains::evaluate(&q).unwrap()).unwrap();
        let e = science_domains::kie::evaluate_kie(&qb, &cb).unwrap();
        let p = KieProof {
            context: c,
            query_bytes: qb,
            candidate_bytes: cb,
            historical_request: test_history(&e.canonical_proposition).0,
            historical_response: test_history(&e.canonical_proposition).1,
            canonical_proposition: e.canonical_proposition,
            kis_mass: e.kis_mass,
            kic_mass: e.kic_mass,
            relation: e.relation,
            proof_sha256: [0; 32],
            prior_adjudication: None,
            history_assessment: None,
        }
        .seal_data()
        .unwrap();
        validate_proof_model(&p).unwrap();
        assert!(validate_proposal_binding(&p, true, false).is_err());
        assert!(validate_proposal_binding(&p, false, true).is_ok());
        let mut invented = p.clone();
        invented.prior_adjudication = Some(KieAdjudicatedRef {
            identity_digest: [1; 32],
            transaction_id: [2; 32],
            request_sha256: [3; 32],
            proof_sha256: [4; 32],
            evidence_family_digest: [5; 32],
            y: 0,
            x: 0,
            reason_code: None,
        });
        let invented = invented.seal_data().unwrap();
        assert_eq!(
            validate_prior_adjudication(&KnowledgeVault::new(), &invented).unwrap_err(),
            "KIE_PRIOR_T1_MISSING"
        );
        let current = attach_current_adjudication(&KnowledgeVault::new(), p.clone()).unwrap();
        assert!(current.prior_adjudication.is_none());
        let mut renamed = p.clone();
        renamed.canonical_proposition = "industrial claim alpha is true".into();
        assert!(renamed.seal_data().is_err());
        let mut false_support = p;
        false_support.relation = 0;
        false_support.kis_mass = 10000;
        false_support.kic_mass = 0;
        assert!(validate_proof_model(&false_support.seal_data().unwrap()).is_err());
    }
}
/// Data-only fixture utility scoped to crate tests; no authoritative hold issued.
#[cfg(test)]
fn test_history(text: &str) -> (Vec<u8>, Vec<u8>) {
    let q = shadow_contracts::KnowledgeSelectionRequestWire {
        version: shadow_contracts::PROTOCOL_VERSION,
        requester: shadow_contracts::SelectionRequester::AshtQuantum,
        purpose: shadow_contracts::SelectionPurpose::EvidenceCandidates,
        session_id: "attempt".into(),
        parent_i0: hex(&digest(text.as_bytes())),
        primitive_anchor: 1,
        trace_id: "attempt".into(),
        split_sha256: "a".repeat(64),
        xi: vec![text.into()],
        yi: vec![],
        upstream_hold_sha256: Some("b".repeat(64)),
        evidence_sha256: "c".repeat(64),
        max_positive: shadow_contracts::SELECTION_MAX_CANDIDATES,
        max_negative: shadow_contracts::SELECTION_MAX_CANDIDATES,
    };
    let qb = shadow_contracts::encode_selection_request(&q);
    let mut s = shadow_contracts::KnowledgeSelectionResponseWire {
        version: shadow_contracts::PROTOCOL_VERSION,
        requester: q.requester,
        session_id: q.session_id.clone(),
        parent_i0: q.parent_i0.clone(),
        primitive_anchor: q.primitive_anchor,
        split_sha256: q.split_sha256.clone(),
        upstream_hold_sha256: q.upstream_hold_sha256.clone(),
        request_sha256: shadow_contracts::sha256_hex(&[b"ESSMAI/SELECTION/REQUEST/V178", &qb]),
        selection_sha256: String::new(),
        hold_id: "historical-fixture".into(),
        hold_sha256: "d".repeat(64),
        positive: vec![],
        negative: vec![],
        law_seal: 1,
        system_laws_seal: 1,
        expires_at_ns: 1,
    };
    s.selection_sha256 = shadow_contracts::pd_memory::failure::relevance::selection_digest(&s);
    (qb, shadow_contracts::encode_selection_response(&s))
}

fn reference_from_commit(
    commit: &crate::vault_transaction::DurableCommit,
    family: pd_memory::Hash,
) -> Result<KieAdjudicatedRef, String> {
    let snapshot = commit
        .native_cycle
        .as_ref()
        .ok_or("KIE_PRIOR_SNAPSHOT_MISSING")?;
    let prior = decode_proof(&snapshot.kie_proof)?;
    let r = &commit.receipt;
    let reference = KieAdjudicatedRef {
        identity_digest: crate::native_cycle::identity_digest(
            &r.session_id,
            &r.parent_i0,
            r.primitive_anchor,
        ),
        transaction_id: commit.transaction_id,
        request_sha256: snapshot.request_sha256,
        proof_sha256: prior.proof_sha256,
        evidence_family_digest: family,
        y: r.y_verdict,
        x: r.x_verdict,
        reason_code: None,
    };
    reference.validate()?;
    Ok(reference)
}
/// New selection consumes the actual map, never a caller-asserted prior verdict.
fn attach_current_adjudication(
    vault: &KnowledgeVault,
    mut proof: KieProof,
) -> Result<KieProof, String> {
    let prior = vault
        .kie_adjudicated(
            &proof.context.identity,
            &proof.canonical_proposition,
            &proof.query_bytes,
        )
        .map_err(|e| format!("KIE_HISTORICAL_LOOKUP:{e:?}"))?;
    proof.prior_adjudication = prior
        .map(|p| reference_from_commit(&p.commit, p.evidence_family_digest))
        .transpose()?;
    let proof = proof.seal_data()?;
    validate_prior_adjudication(vault, &proof)?;
    Ok(proof)
}
/// Exact prior-T1 verification for new admission AND WAL replay. It deliberately
/// does not compare with the latest map head, which changes after each commit.
/// The old receipt remains data; it is never used as the new cycle's authority.
pub(crate) fn validate_prior_adjudication(
    vault: &KnowledgeVault,
    proof: &KieProof,
) -> Result<(), String> {
    let Some(reference) = &proof.prior_adjudication else {
        return Ok(());
    };
    reference.validate()?;
    let commit = vault
        .committed_identity(&reference.identity_digest)
        .map_err(|e| format!("KIE_PRIOR_LOOKUP:{e:?}"))?
        .ok_or("KIE_PRIOR_T1_MISSING")?;
    let snapshot = commit
        .native_cycle
        .as_ref()
        .ok_or("KIE_PRIOR_SNAPSHOT_MISSING")?;
    let prior = decode_proof(&snapshot.kie_proof)?;
    validate_proof_model(&prior)?;
    let current_model = science_domains::decode_candidate(&proof.candidate_bytes)
        .map_err(|e| format!("KIE_CURRENT_MODEL:{e}"))?;
    let prior_model = science_domains::decode_candidate(&prior.candidate_bytes)
        .map_err(|e| format!("KIE_PRIOR_MODEL:{e}"))?;
    let family = crate::native_cycle::kie_family(&current_model);
    if prior.context.identity != proof.context.identity
        || prior.canonical_proposition != proof.canonical_proposition
        || crate::native_cycle::kie_family(&prior_model) != family
        || reference_from_commit(&commit, family)? != *reference
    {
        return Err("KIE_PRIOR_ACTUAL_T1_BINDING".into());
    }
    Ok(())
}
/// Independent Shadow gate. A critical model/history proof cannot be paired
/// with a caller-supplied positive XY proposal merely by recomputing its hashes.
fn validate_proposal_binding(
    proof: &KieProof,
    has_xy: bool,
    has_refusal: bool,
) -> Result<(), String> {
    if !combined_evaluation(proof)?.permits_positive && (has_xy || !has_refusal) {
        return Err("KIE_COMMON_FILTER_FORBIDS_POSITIVE_PROPOSAL".into());
    }
    Ok(())
}
/// Current model measurement; no confidence, candidate count or authority is synthesized.
pub(crate) fn measure_current_model(proof: &shadow_contracts::kie::KieProof)
    -> Result<shadow_contracts::kie_measurement::KieMeasurement, String> {
    proof.validate()?;
    let measured = science_domains::kie::measure_kie(&proof.query_bytes, &proof.candidate_bytes)
        .map_err(|e| format!("KIE_MEASURED_MODEL:{e}"))?;
    let actual = measured.candidate;
    let science_domains::ScienceTask::EarthRotationAngle { claim_radians, tolerance_radians, .. } = actual.query.task;
    shadow_contracts::kie_measurement::KieMeasurement::from_comparison(proof,
        &shadow_contracts::pd_memory::iz_science_math::EraComparison {
            query_sha256:actual.query_sha256, result_sha256:actual.result_sha256,
            model_sha256:actual.provenance.compiled_model_sha256, claim_radians,
            computed_radians:actual.computed_radians,
            residual_radians:actual.circular_residual_radians, tolerance_radians,
        })
}

