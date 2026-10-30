//! Explicit KiE native entry: transport and canonical rendering.
//! Light does not evaluate the model or issue scientific/Shadow acceptance.
use crate::pd_native_followup;
use crate::pd_operational_bridge::{self as bridge, NativePeer};
use sha2::{Digest, Sha256};
use shadow_contracts::kie::KieContext;
use shadow_contracts::pd_memory::{self as pd, linked_science};
use shadow_contracts::pd_recovery::{self as recovery, RecoveryLookup};
use std::path::PathBuf;

pub struct KieCycleArgs {
    pub parent_record: PathBuf,
    pub output_reference: PathBuf,
    pub predecessor: Option<PathBuf>,
}
pub struct KieCycleInput {
    pub canonical_text: String,
    pub context: KieContext,
    pub output_reference: PathBuf,
    pub predecessor: Option<RecoveryLookup>,
}
pub fn parse_args(args: &[String]) -> Result<Option<KieCycleArgs>, String> {
    if args.get(1).map(String::as_str) != Some("--kie-cycle-once") {
        return Ok(None);
    }
    if !(args.len() == 4 || args.len() == 5) {
        return Err("LIGHT_KIE_USAGE: --kie-cycle-once PARENT_PDS1_RECORD OUTPUT_REFERENCE [PREDECESSOR_LOOKUP]".into());
    }
    if args[2..].iter().any(|value| value.is_empty()) {
        return Err("LIGHT_KIE_EMPTY_PATH".into());
    }
    Ok(Some(KieCycleArgs {
        parent_record: PathBuf::from(&args[2]),
        output_reference: PathBuf::from(&args[3]),
        predecessor: args.get(4).map(PathBuf::from),
    }))
}

fn validate_query_scope(
    query: &science_domains::ScienceQuery,
    identity: &pd::PdIdentity,
    candidate_key: &pd::Hash,
) -> Result<(), String> {
    let query_bytes = science_domains::encode_query(query).map_err(|e| e.to_string())?;
    let query_hash: pd::Hash = Sha256::digest(&query_bytes).into();
    if query.adapter != science_domains::AdapterId::Astronomy
        || query.session_id != pd::hex(&identity.key()?)
        || query.case_id != identity.case_id
        || query.scope_id != identity.scope
        || query.obligation_id != identity.obligation
        || query_hash != *candidate_key
    {
        return Err("LIGHT_KIE_QUERY_SCOPE_OR_KEY".into());
    }
    Ok(())
}
pub fn validate_territory(context: &KieContext, actual: &str) -> Result<(), String> {
    if actual != context.identity.scope || actual != context.identity.domain {
        return Err("LIGHT_KIE_TERRITORY_MISMATCH".into());
    }
    Ok(())
}
fn bind_predecessor(
    context: Option<&shadow_contracts::pd_memory::iz::IzContext>,
    supplied: Option<RecoveryLookup>,
) -> Result<Option<RecoveryLookup>, String> {
    if let Some(context) = context {
        context.validate()?;
        if let Some(anchor) = &context.native_anchor {
            if supplied.as_ref().is_some_and(|value| value != &anchor.predecessor) {
                return Err("LIGHT_KIE_PREDECESSOR_SOURCE_CONFLICT".into());
            }
            return Ok(Some(anchor.predecessor.clone()));
        }
    }
    Ok(supplied)
}
pub fn prepare(args: KieCycleArgs) -> Result<KieCycleInput, String> {
    if args.output_reference.exists() {
        return Err("LIGHT_KIE_OUTPUT_EXISTS".into());
    }
    let supplied = bridge::read_bounded(&args.parent_record, pd::MAX_REQUEST_BYTES + 256)?;
    let parent = pd::decode_record(&supplied)?;
    linked_science::validate_science_record(&parent)?;
    let science_query = linked_science::science_query_from_source(&parent.request.source_input)?;
    let context = KieContext {
        identity: parent.request.identity.clone(),
        operation_id: parent.request.operation_id.clone(),
        record_id: parent.record_id,
    };
    context.validate()?;
    // Resolve the supplied reference against actual Shadow-owned storage before
    // entering coordination. A self-consistent local record is insufficient.
    let response = bridge::native_exchange(
        NativePeer::Shadow,
        "--pd-head-once",
        &pd::encode_lookup(&context.lookup())?,
        pd::MAX_REQUEST_BYTES + 256,
    )?;
    let actual =
        pd::decode_optional_record(&response)?.ok_or("LIGHT_KIE_DURABLE_SOURCE_MISSING")?;
    if pd::encode_record(&actual)? != supplied {
        return Err("LIGHT_KIE_DURABLE_SOURCE_MISMATCH".into());
    }
    let query = science_domains::decode_query(&science_query)
        .map_err(|error| format!("LIGHT_KIE_QUERY:{error}"))?;
    validate_query_scope(&query, &context.identity, &parent.request.candidate_key)?;
    // Pure rendering from explicit query values; no model computation in Light.
    let canonical_text = science_domains::kie::canonical_proposition(&query)
        .map_err(|error| format!("LIGHT_KIE_CANONICAL:{error}"))?;
    crate::light_coordinator::LightRequest::try_new(&canonical_text)
        .map_err(|error| format!("LIGHT_KIE_INPUT:{error}"))?;
    let supplied_predecessor = args.predecessor.as_ref().map(|path| {
        recovery::decode_lookup(&bridge::read_bounded(path, 68)?)
    }).transpose()?;
    let science_context = if shadow_contracts::pd_memory::iz_science::source_is_science(&parent.request.source_input) {
        Some(shadow_contracts::pd_memory::iz::context_from_record(&parent)?)
    } else { None };
    let predecessor = bind_predecessor(science_context.as_ref(), supplied_predecessor)?;
    if let Some(reference) = &predecessor {
        let bundle = pd_native_followup::fetch(reference)?;
        if let Some(anchor) = science_context.as_ref().and_then(|context| context.native_anchor.as_ref()) {
            if &shadow_contracts::pd_prepared::PdContinuationLink::from_bundle(&bundle)? != anchor {
                return Err("LIGHT_KIE_NATIVE_ANCHOR_MISMATCH".into());
            }
        }
    }
    Ok(KieCycleInput {
        canonical_text,
        context,
        output_reference: args.output_reference,
        predecessor,
    })
}
/// Only called after the shared native runtime has returned bind_completed.
/// Exact 68-byte lookup; create_new + sync_all through the existing publisher.
pub fn publish(input: &KieCycleInput, reference: &RecoveryLookup) -> Result<(), String> {
    let bytes = recovery::encode_lookup(reference)?;
    if bytes.len() != 68 {
        return Err("LIGHT_KIE_REFERENCE_SIZE".into());
    }
    bridge::publish_new(&input.output_reference, &bytes)?;
    println!(
        "{}",
        serde_json::json!({
            "kind": "KiENativeCycleReference",
            "scope": "MODEL_CONDITIONAL",
            "source_record_id": pd::hex(&input.context.record_id),
            "native_identity": pd::hex(&reference.identity),
            "request_sha256": pd::hex(&reference.request_sha256),
            "output_reference": input.output_reference.to_string_lossy(),
            "light_decides": false
        })
    );
    Ok(())
}

/// Raw PDR1 output is typed, never disguised as the native lookup. The process
/// bridge already resolved this exact operation from the Shadow storage owner.
pub fn publish_operational(input:&KieCycleInput, record:&pd::PdOperationRecord) -> Result<(),String> {
    record.validate()?;
    let source=pd::late_refusal::validate_operation(&record.request)?;
    let request=shadow_contracts::decode_cycle_request(&source.attempt).map_err(|e|e.to_string())?;
    if source.context!=input.context
        || request.quantum.final_evidence.light_input_bytes!=input.canonical_text.as_bytes() {
        return Err("LIGHT_KIE_OPERATIONAL_SOURCE_MISMATCH".into());
    }
    let question=pd::late_refusal::render_iz(record)?;
    bridge::publish_new(&input.output_reference,&pd::encode_record(record)?)?;
    println!("{}",serde_json::json!({
        "kind":"KiEOperationalRecord","status":"OperationalNoVerdict",
        "output_format":"PDR1","output_record":input.output_reference.to_string_lossy(),
        "source_record_id":pd::hex(&input.context.record_id),"record_id":pd::hex(&record.record_id),
        "question":question,"knowledge_verdict":null,
        "knowledge_write":0,"spine9_verified":false,"authority_transferred":false,"light_decides":false
    }));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn identity() -> pd::PdIdentity {
        pd::PdIdentity {
            owner: "operator".into(),
            obligation: "same scientific inquiry".into(),
            case_id: "case-a".into(),
            scope: "WORK→NURA".into(),
            domain: "WORK→NURA".into(),
            lineage_root: "lineage-a".into(),
        }
    }
    fn query(identity: &pd::PdIdentity) -> science_domains::ScienceQuery {
        science_domains::ScienceQuery {
            adapter: science_domains::AdapterId::Astronomy,
            session_id: pd::hex(&identity.key().unwrap()),
            case_id: identity.case_id.clone(),
            scope_id: identity.scope.clone(),
            obligation_id: identity.obligation.clone(),
            query_id: "explicit-model-query".into(),
            input_source_id: "regression-only".into(),
            task: science_domains::ScienceTask::EarthRotationAngle {
                jd_part1: 2400000.5,
                jd_part2: 54388.0,
                time_scale: science_domains::TimeScale::Ut1,
                claim_radians: 0.4022837240028158,
                tolerance_radians: 1e-12,
            },
        }
    }
    #[test]
    fn cli_requires_exact_explicit_files_and_does_not_capture_default_mode() {
        for args in [
            vec!["light", "--kie-cycle-once"],
            vec!["light", "--kie-cycle-once", "parent"],
            vec![
                "light",
                "--kie-cycle-once",
                "parent",
                "output",
                "prior",
                "extra",
            ],
            vec!["light", "--kie-cycle-once", "", "output"],
        ] {
            assert!(parse_args(&args.into_iter().map(String::from).collect::<Vec<_>>()).is_err());
        }
        let args = ["light", "--kie-cycle-once", "parent", "output", "prior"].map(String::from);
        let parsed = parse_args(&args).unwrap().unwrap();
        assert_eq!(parsed.predecessor, Some(PathBuf::from("prior")));
        assert!(parse_args(&["light".into()]).unwrap().is_none());
        assert!(
            parse_args(&["light".into(), "--pd-resume-native".into(), "prior".into()])
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn explicit_provider_does_not_relabel_parent_and_foreign_scope_or_key_refused() {
        let identity = identity();
        let query = query(&identity);
        let key = Sha256::digest(science_domains::encode_query(&query).unwrap()).into();
        validate_query_scope(&query, &identity, &key).unwrap();
        let rendered = science_domains::kie::canonical_proposition(&query).unwrap();
        assert!(rendered.starts_with("MODEL_CONDITIONAL"));
        assert!(rendered.contains("no observation"));
        for field in 0..4 {
            let mut changed = query.clone();
            match field {
                0 => changed.session_id.push('x'),
                1 => changed.case_id.push('x'),
                2 => changed.scope_id.push('x'),
                _ => changed.obligation_id.push('x'),
            };
            assert!(validate_query_scope(&changed, &identity, &key).is_err());
        }
        assert!(validate_query_scope(&query, &identity, &[7; 32]).is_err());
        assert_eq!(identity.domain, "WORK→NURA");
        let context = KieContext {
            identity: identity.clone(),
            operation_id: "science-op".into(),
            record_id: [1; 32],
        };
        let actual = crate::nura_core::Domain::detect(&rendered);
        assert_eq!(actual.label(), "WORK→NURA");
        validate_territory(&context, actual.label()).unwrap();
        assert!(validate_territory(&context, "HOME→ALISAH").is_err());
    }
    #[test]
    fn native_science_uses_source_anchor_and_rejects_other_predecessor() {
        let mut id = identity();
        let user = "motor era00".to_string();
        id.lineage_root = pd::hex(&Sha256::digest(user.as_bytes()).into());
        let predecessor = RecoveryLookup { identity: [3;32], request_sha256: [4;32] };
        let context = shadow_contracts::pd_memory::iz::IzContext {
            version: shadow_contracts::pd_memory::iz::IZ_NATIVE_CONTINUITY_VERSION,
            identity: id.clone(), operation_id: "science".into(), origin_i0: user.clone(), user_text: user,
            parent: None, answer: Default::default(), map: None, native_map: None,
            native_anchor: Some(shadow_contracts::pd_prepared::PdContinuationLink {
                cognitive_session_id: pd::hex(&id.key().unwrap()), territory: id.scope,
                predecessor: predecessor.clone(), predecessor_transaction_id: [5;32],
                predecessor_continuation_sha256: [6;32], predecessor_next_i0: "next-source-id".into(), clock_millis: true,
            }),
        };
        assert_eq!(bind_predecessor(Some(&context), None).unwrap(), Some(predecessor.clone()));
        assert_eq!(bind_predecessor(Some(&context), Some(predecessor.clone())).unwrap(), Some(predecessor.clone()));
        let mut changed = predecessor; changed.request_sha256 = [7;32];
        assert!(bind_predecessor(Some(&context), Some(changed)).is_err());
    }

}
