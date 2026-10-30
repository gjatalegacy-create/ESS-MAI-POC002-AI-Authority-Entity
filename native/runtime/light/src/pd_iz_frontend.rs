//! Canonical iZ transport under the existing Light/BESA path.
//! pd_mp resolves references; only Vault owns material. Light issues no verdict.
use crate::pd_operational_bridge::{self as bridge, NativePeer};
use sha2::{Digest, Sha256};
use shadow_contracts::pd_memory::{self as pd, iz};
use std::path::PathBuf;

/// Explicit caller labels only; the existing iZ contract validates exact spans.
#[derive(Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
struct LiteralAnswerFields {
    positive: String,
    negative: String,
    distinction: String,
    source_reference: String,
    verification: String,
}
#[derive(Default)]
pub(crate) struct LiteralAnswer(LiteralAnswerFields);
impl<'de> serde::Deserialize<'de> for LiteralAnswer {
    fn deserialize<D:serde::Deserializer<'de>>(deserializer:D)->Result<Self,D::Error> {
        struct Object;
        impl<'de> serde::de::Visitor<'de> for Object {
            type Value=LiteralAnswer;
            fn expecting(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result {
                f.write_str("an object of literal iZ fields")
            }
            fn visit_map<M:serde::de::MapAccess<'de>>(self,map:M)->Result<Self::Value,M::Error> {
                <LiteralAnswerFields as serde::Deserialize>::deserialize(serde::de::value::MapAccessDeserializer::new(map))
                    .map(LiteralAnswer)
            }
        }
        deserializer.deserialize_map(Object)
    }
}
impl From<LiteralAnswer> for iz::IzAnswer {
    fn from(value: LiteralAnswer) -> Self {
        let value=value.0;
        Self {
            positive: value.positive, negative: value.negative,
            distinction: value.distinction, source_reference: value.source_reference,
            verification: value.verification,
        }
    }
}

pub struct IzCycleArgs {
    pub context_file: PathBuf,
    pub output_record: PathBuf,
}
pub struct IzCycleInput {
    pub context: iz::IzContext,
    pub output_record: PathBuf,
}
pub fn parse_args(args: &[String]) -> Result<Option<IzCycleArgs>, String> {
    if args.get(1).map(String::as_str) != Some("--iz-cycle-once") {
        return Ok(None);
    }
    if args.len() != 4 || args[2..].iter().any(String::is_empty) {
        return Err("LIGHT_IZ_USAGE: --iz-cycle-once CONTEXT_FILE OUTPUT_RECORD".into());
    }
    Ok(Some(IzCycleArgs {
        context_file: PathBuf::from(&args[2]),
        output_record: PathBuf::from(&args[3]),
    }))
}
pub fn prepare(args: IzCycleArgs) -> Result<IzCycleInput, String> {
    if args.output_record.exists() {
        return Err("LIGHT_IZ_OUTPUT_EXISTS".into());
    }
    let context = iz::decode_context(&bridge::read_bounded(
        &args.context_file,
        iz::MAX_CONTEXT_BYTES,
    )?)?;
    validate_origin(context.current_i0())?;
    validate_territory(
        &context,
        crate::nura_core::Domain::detect(context.current_i0()).label(),
    )?;
    Ok(IzCycleInput {
        context,
        output_record: args.output_record,
    })
}
/// Old records remain readable/replayable; all new runtime writes bind actual i0.
pub fn require_current_input_mode(context: &iz::IzContext) -> Result<(), String> {
    context.validate()?;
    if !matches!(context.version, iz::IZ_ACTUAL_INPUT_VERSION | iz::IZ_NATIVE_CONTINUITY_VERSION) {
        return Err("IZ_LEGACY_NEW_WRITE_DISABLED_USE_ICX2".into());
    }
    Ok(())
}
fn validate_origin(origin: &str) -> Result<(), String> {
    let parsed = crate::light_coordinator::LightRequest::try_new(origin)
        .map_err(|e| format!("LIGHT_IZ_ORIGIN:{e}"))?;
    if parsed.text() != origin {
        return Err("LIGHT_IZ_ORIGIN_NOT_EXACT_LIGHT_INPUT".into());
    }
    Ok(())
}
pub fn validate_territory(context: &iz::IzContext, actual: &str) -> Result<(), String> {
    if context.identity.scope != actual || context.identity.domain != actual {
        return Err("LIGHT_IZ_TERRITORY_MISMATCH".into());
    }
    Ok(())
}
fn lookup(identity: &pd::PdIdentity, operation: &str) -> Result<Vec<u8>, String> {
    pd::encode_lookup(&pd::PdLookup {
        identity: identity.clone(),
        operation_id: Some(operation.into()),
    })
}
fn bind_record(context: &iz::IzContext, record: &pd::PdOperationRecord) -> Result<(), String> {
    record.validate()?;
    let source = iz::validate_operation(&record.request)?;
    if record.request.identity != context.identity
        || record.request.operation_id != context.operation_id
        || !iz::same_intent(context, &source.context)
    {
        return Err("LIGHT_IZ_OPERATION_CONFLICT".into());
    }
    Ok(())
}
/// Exact durable replay before any fresh physical attempt or PD ingestion.
/// A marker/stdout/exit code is never a substitute for these Vault-owned bytes.
pub fn exact_operation(context: &iz::IzContext) -> Result<Option<pd::PdOperationRecord>, String> {
    context.validate()?;
    let bytes = bridge::native_exchange(
        NativePeer::Shadow,
        "--pd-head-once",
        &lookup(&context.identity, &context.operation_id)?,
        pd::MAX_REQUEST_BYTES + 256,
    )?;
    let record = pd::decode_optional_record(&bytes)?;
    if let Some(record) = &record {
        bind_record(context, record)?;
    }
    Ok(record)
}
fn bind_map(context: &iz::IzContext, map: Option<&iz::IzMapSnapshot>) -> Result<(), String> {
    context.validate()?;
    match (context.parent.as_ref(), map) {
        (None, None) => Ok(()),
        (Some(parent), Some(map)) => {
            map.validate()?;
            if map.identity_key != context.identity.key()?
                || map.head != *parent
                || map.exact_record != *parent
                || context.map.as_ref().is_some_and(|supplied| supplied != map)
            {
                return Err("LIGHT_IZ_MAP_BINDING_OR_STALE_PARENT".into());
            }
            Ok(())
        }
        (None, Some(_)) => Err("LIGHT_IZ_INITIAL_SCOPE_ALREADY_EXISTS".into()),
        (Some(_), None) => Err("LIGHT_IZ_PARENT_MAP_MISSING".into()),
    }
}
fn bind_parent(context: &iz::IzContext, record: &pd::PdOperationRecord) -> Result<(), String> {
    let prior = iz::context_from_record(record)?;
    if record.request.identity != context.identity
        || context.parent.as_ref() != Some(&iz::IzParent::from_record(record)?)
        || prior.origin_i0 != context.origin_i0
    {
        return Err("LIGHT_IZ_EXACT_VAULT_PARENT_BINDING".into());
    }
    Ok(())
}
fn bind_field(
    context: &iz::IzContext,
    reference: &iz::IzFieldRef,
    record: &pd::PdOperationRecord,
) -> Result<(), String> {
    let prior = iz::context_from_record(record)?;
    record.validate()?;
    let iz::IzFieldOrigin::Prior {
        record_id,
        revision,
        operation_id,
        value_sha256,
    } = &reference.origin
    else {
        return Err("LIGHT_IZ_UNRESOLVED_CURRENT_FIELD".into());
    };
    let value = prior.answer.get(reference.field);
    let digest: pd::Hash = Sha256::digest(value.as_bytes()).into();
    if record.request.identity != context.identity
        || record.record_id != *record_id
        || record.revision != *revision
        || record.request.operation_id != *operation_id
        || prior.origin_i0 != context.origin_i0
        || value.is_empty()
        || digest != *value_sha256
    {
        return Err("LIGHT_IZ_VAULT_FIELD_REFERENCE_BINDING".into());
    }
    Ok(())
}
/// Called only inside BesaNlight<LawGated>, after command/request validation.
/// The neutral primitive carries the caller-selected map request; it decides nothing.
pub(crate) fn hydrate_under_besa(context: &mut iz::IzContext) -> Result<(), String> {
    hydrate_with(context, |mode, input, limit| {
        bridge::native_exchange(NativePeer::Shadow, mode, input, limit)
    })
}
fn hydrate_with(
    context: &mut iz::IzContext,
    mut exchange: impl FnMut(&str, &[u8], usize) -> Result<Vec<u8>, String>,
) -> Result<(), String> {
    context.validate()?;
    if context.version == iz::IZ_NATIVE_CONTINUITY_VERSION {
        return hydrate_native_with(context, exchange);
    }
    let pulse = ess_mai_zero_noi::trigger(&*context);
    let bytes = iz::encode_context(pulse.request())?;
    let identity = pd::hex(&context.identity.key()?);
    eprintln!("[BESA/0-NOI] map_begin identity={identity} authority=0");
    let map = iz::decode_map_snapshot(&exchange(
        "--pd-iz-map-once",
        &bytes,
        iz::MAX_CONTEXT_BYTES,
    )?)?;
    bind_map(context, map.as_ref())?;
    eprintln!(
        "[BESA/0-NOI] map_done identity={identity} revision={} authority=0",
        map.as_ref().map(|m| m.head.revision).unwrap_or(0)
    );
    let Some(map) = map else {
        return Ok(());
    };
    // Mutate only resolved metadata, never the original inquiry or user answer.
    let mut mapped = context.clone();
    mapped.map = Some(map);
    let pulse = ess_mai_zero_noi::trigger(&mapped);
    let bytes = iz::encode_context(pulse.request())?;
    eprintln!("[BESA/0-NOI] exact_vault_read_begin identity={identity} authority=0");
    let parent = pd::decode_optional_record(&exchange(
        "--pd-iz-read-once",
        &bytes,
        pd::MAX_REQUEST_BYTES + 256,
    )?)?
    .ok_or("LIGHT_IZ_EXACT_VAULT_PARENT_MISSING")?;
    bind_parent(&mapped, &parent)?;
    eprintln!("[BESA/Vault] exact_read_bound identity={identity} record={} revision={} status=OperationalNoVerdict",pd::hex(&parent.record_id),parent.revision);
    hydrate_fields(&mapped, &parent, &mut exchange)?;
    *context = mapped;
    Ok(())
}
/// Exact predecessor supplied by the context; no latest-topic inference.
pub fn native_predecessor(context: &iz::IzContext) -> Result<Option<shadow_contracts::pd_recovery::RecoveryLookup>, String> {
    context.validate()?;
    Ok(context.native_anchor.as_ref().map(|anchor| anchor.predecessor.clone()))
}
fn bind_native_map(context: &iz::IzContext, map: &iz::IzNativeMapSnapshot) -> Result<(), String> {
    map.validate()?;
    if map.identity_key != context.identity.key()?
        || context.native_anchor.as_ref() != Some(&map.native_anchor)
        || context.parent != map.operational_head
        || context.native_map.as_ref().is_some_and(|supplied| supplied != map)
    { return Err("LIGHT_IZ_NATIVE_MAP_BINDING_OR_STALE_PARENT".into()); }
    Ok(())
}
fn hydrate_native_with(
    context: &mut iz::IzContext,
    mut exchange: impl FnMut(&str, &[u8], usize) -> Result<Vec<u8>, String>,
) -> Result<(), String> {
    let mut mapped = context.clone();
    let pulse = ess_mai_zero_noi::trigger(&*context);
    let request = iz::encode_context(pulse.request())?;
    let identity = pd::hex(&context.identity.key()?);
    eprintln!("[BESA/0-NOI] native_map_begin identity={identity} authority=0");
    let map = iz::decode_native_map_snapshot(&exchange(
        "--pd-iz-native-map-once", &request, iz::MAX_CONTEXT_BYTES)?)?;
    bind_native_map(context, &map)?;
    mapped.native_map = Some(map);
    let pulse = ess_mai_zero_noi::trigger(&mapped);
    let request = iz::encode_context(pulse.request())?;
    eprintln!("[BESA/0-NOI] native_exact_read_begin identity={identity} authority=0");
    let bundle = shadow_contracts::pd_recovery::decode_bundle(&exchange(
        "--pd-iz-native-read-once", &request, shadow_contracts::pd_recovery::MAX_BUNDLE_BYTES)?)?;
    let parent = if mapped.parent.is_some() {
        let pulse = ess_mai_zero_noi::trigger(&mapped);
        let request = iz::encode_context(pulse.request())?;
        eprintln!("[BESA/0-NOI] exact_vault_read_begin identity={identity} authority=0");
        let record = pd::decode_optional_record(&exchange(
            "--pd-iz-read-once", &request, pd::MAX_REQUEST_BYTES + 256)?)?
            .ok_or("LIGHT_IZ_EXACT_VAULT_PARENT_MISSING")?;
        bind_parent(&mapped, &record)?;
        Some(record)
    } else { None };
    iz::validate_native_binding(&mapped, &bundle, parent.as_ref())?;
    if let Some(parent) = &parent { hydrate_fields(&mapped, parent, &mut exchange)?; }
    eprintln!("[BESA/Vault] native_and_operational_references_bound identity={identity} authority=0");
    *context = mapped;
    Ok(())
}
fn hydrate_fields(
    mapped: &iz::IzContext,
    parent: &pd::PdOperationRecord,
    exchange: &mut impl FnMut(&str, &[u8], usize) -> Result<Vec<u8>, String>,
) -> Result<(), String> {
    let identity = pd::hex(&mapped.identity.key()?);
    // At most five reference lookups. Material is read from the same Vault,
    // checked against each digest, then discarded; no second warehouse exists.
    for reference in iz::frontier_for_context(&mapped, &parent)? {
        let iz::IzFieldOrigin::Prior { operation_id, .. } = &reference.origin else {
            return Err("LIGHT_IZ_FRONTIER_NOT_REFERENCE".into());
        };
        if operation_id == &parent.request.operation_id {
            bind_field(&mapped, &reference, &parent)?;
        } else {
            let request = lookup(&mapped.identity, operation_id)?;
            let pulse = ess_mai_zero_noi::trigger(&request);
            eprintln!(
                "[BESA/0-NOI] field_vault_read_begin identity={identity} field={:?} authority=0",
                reference.field
            );
            let encoded = exchange(
                "--pd-head-once",
                pulse.request(),
                pd::MAX_REQUEST_BYTES + 256,
            )?;
            let actual =
                pd::decode_optional_record(&encoded)?.ok_or("LIGHT_IZ_VAULT_FIELD_MISSING")?;
            bind_field(&mapped, &reference, &actual)?;
        }
        if let iz::IzFieldOrigin::Prior {
            record_id,
            revision,
            ..
        } = &reference.origin
        {
            eprintln!("[BESA/Vault] field_reference_bound identity={identity} field={:?} record={} revision={revision} status=OperationalNoVerdict",reference.field,pd::hex(record_id));
        }
    }
    Ok(())
}
pub fn publish(input: &IzCycleInput, record: &pd::PdOperationRecord) -> Result<(), String> {
    bind_record(&input.context, record)?;
    bridge::publish_new(&input.output_record, &pd::encode_record(record)?)?;
    println!("{}", record.request.scoped_question);
    bridge::print_operational(record)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context() -> iz::IzContext {
        let origin = "Analizo motorin industrial dhe sensorin termik".to_string();
        let lineage = pd::hex(&Sha256::digest(origin.as_bytes()).into());
        iz::IzContext {
            version: 1,
            identity: pd::PdIdentity {
                owner: "owner".into(),
                obligation: "exact inquiry".into(),
                case_id: "case-a".into(),
                scope: "WORK→NURA".into(),
                domain: "WORK→NURA".into(),
                lineage_root: lineage,
            },
            operation_id: "initial".into(),
            origin_i0: origin.clone(),
            user_text: origin,
            parent: None,
            answer: iz::IzAnswer::default(),
            map: None,
            native_anchor: None,
            native_map: None,
        }
    }
    fn parent() -> iz::IzParent {
        iz::IzParent {
            record_id: [1; 32],
            revision: 3,
            state_digest: [2; 32],
            question_sha256: [3; 32],
        }
    }
    #[test]
    fn cli_is_explicit_and_origin_keeps_existing_light_bound() {
        assert!(parse_args(&["light".into()]).unwrap().is_none());
        assert!(parse_args(&[
            "light".into(),
            "--iz-cycle-once".into(),
            "in".into(),
            "out".into()
        ])
        .unwrap()
        .is_some());
        for rest in [
            vec![],
            vec!["in"],
            vec!["", "out"],
            vec!["in", "out", "extra"],
        ] {
            let mut args = vec!["light".into(), "--iz-cycle-once".into()];
            args.extend(rest.into_iter().map(String::from));
            assert!(parse_args(&args).is_err());
        }
        assert!(validate_origin(&"a".repeat(513)).is_err());
        assert!(validate_origin(" motor").is_err());
        assert!(validate_origin("motor\nsensor").is_err());
        validate_origin("motor").unwrap();
        assert!(validate_territory(&context(), "HOME→ALISAH").is_err());
    }
    #[test]
    fn empty_map_does_not_invent_a_vault_record_or_read_candidates() {
        let mut c = context();
        let original = c.clone();
        let mut calls = Vec::new();
        hydrate_with(&mut c, |mode, bytes, _| {
            calls.push(mode.to_string());
            assert_eq!(iz::decode_context(bytes).unwrap(), original);
            iz::encode_map_snapshot(None)
        })
        .unwrap();
        assert_eq!(calls, vec!["--pd-iz-map-once"]);
        assert_eq!(c, original);
    }
    #[test]
    fn foreign_or_stale_map_fails_before_vault_read_and_does_not_mutate_input() {
        let mut c = context();
        c.parent = Some(parent());
        c.operation_id = "reply".into();
        c.user_text = "Mjeti i kuq".into();
        c.answer.positive = "Mjeti i kuq".into();
        let original = c.clone();
        for foreign in [true, false] {
            let mut calls = 0;
            let mut map = iz::IzMapSnapshot {
                identity_key: c.identity.key().unwrap(),
                head: parent(),
                exact_record: parent(),
                status: iz::IzMaterialStatus::OperationalNoVerdict,
            };
            if foreign {
                map.identity_key = [9; 32]
            } else {
                map.head.revision += 1;
                map.exact_record = map.head.clone();
            }
            let err = hydrate_with(&mut c, |mode, _, _| {
                calls += 1;
                assert_eq!(mode, "--pd-iz-map-once");
                iz::encode_map_snapshot(Some(&map))
            })
            .unwrap_err();
            assert!(err.contains("MAP_BINDING_OR_STALE_PARENT"));
            assert_eq!(calls, 1);
            assert_eq!(c, original);
        }
        assert!(bind_map(&c, None).is_err());
    }
    #[test]
    fn malformed_map_transport_is_not_an_authority_refusal_or_commit() {
        let mut c = context();
        let original = c.clone();
        assert!(hydrate_with(&mut c, |_, _, _| Ok(b"malformed".to_vec())).is_err());
        assert_eq!(c, original);
        assert_eq!(
            hydrate_with(&mut c, |_, _, _| Err("TRANSPORT_TIMEOUT".into())).unwrap_err(),
            "TRANSPORT_TIMEOUT"
        );
        assert_eq!(c, original);
    }
    #[test]
    fn new_runtime_cycle_requires_actual_user_mode_but_unknown_reply_is_not_fabricated() {
        let mut c = context();
        assert_eq!(
            require_current_input_mode(&c).unwrap_err(),
            "IZ_LEGACY_NEW_WRITE_DISABLED_USE_ICX2"
        );
        c.version = 2;
        c.parent = Some(parent());
        c.operation_id = "unknown-reply".into();
        c.user_text = "Nuk e di nese motori ka prove.".into();
        require_current_input_mode(&c).unwrap();
        assert_eq!(c.current_i0(), c.user_text);
        assert_ne!(c.current_i0(), c.origin_i0);
        assert!(c.answer.is_empty());
        assert!(c.answer_spans().unwrap().is_empty());
        validate_origin(c.current_i0()).unwrap();
        validate_territory(&c, crate::nura_core::Domain::detect(c.current_i0()).label()).unwrap();
        c.answer.positive = "a statement the user did not give".into();
        assert!(require_current_input_mode(&c).is_err());
    }
    fn native_context() -> iz::IzContext {
        let mut c = context();
        c.version = iz::IZ_NATIVE_CONTINUITY_VERSION;
        c.parent = Some(parent());
        c.operation_id = "after-native".into();
        c.user_text = "motor era00".into();
        c.native_anchor = Some(shadow_contracts::pd_prepared::PdContinuationLink {
            cognitive_session_id: pd::hex(&c.identity.key().unwrap()), territory: c.identity.scope.clone(),
            predecessor: shadow_contracts::pd_recovery::RecoveryLookup { identity: [4;32], request_sha256: [5;32] },
            predecessor_transaction_id: [6;32], predecessor_continuation_sha256: [7;32],
            predecessor_next_i0: "source-next-i0".into(), clock_millis: true,
        });
        c.validate().unwrap();
        c
    }
    #[test]
    fn native_map_is_exact_and_failure_never_partially_hydrates_context() {
        // Structural transport fixture, not a native receipt or admitted material.
        let original = native_context();
        let map = iz::IzNativeMapSnapshot {
            identity_key: original.identity.key().unwrap(),
            native_anchor: original.native_anchor.clone().unwrap(),
            operational_head: original.parent.clone(), checkpoint_base: iz::IzCheckpointBase::NativeT2,
        };
        bind_native_map(&original, &map).unwrap();
        for variation in 0..4 {
            let mut c = original.clone();
            let mut changed = map.clone();
            match variation {
                0 => { changed.identity_key = [9;32]; changed.native_anchor.cognitive_session_id = pd::hex(&changed.identity_key); },
                1 => changed.native_anchor.predecessor.request_sha256 = [9;32],
                2 => changed.operational_head.as_mut().unwrap().revision += 1,
                _ => { c.native_map = Some(map.clone()); changed.checkpoint_base = iz::IzCheckpointBase::Operational(parent()); },
            }
            let before = c.clone();
            let mut calls = 0;
            assert!(hydrate_with(&mut c, |mode, _, _| {
                calls += 1;
                assert_eq!(mode, "--pd-iz-native-map-once");
                iz::encode_native_map_snapshot(&changed)
            }).is_err());
            assert_eq!(calls, 1);
            assert_eq!(c, before);
        }
        let mut c = original.clone();
        let mut calls = Vec::new();
        assert!(hydrate_with(&mut c, |mode, bytes, _| {
            calls.push(mode.to_owned());
            let request = iz::decode_context(bytes).unwrap();
            if mode == "--pd-iz-native-map-once" { iz::encode_native_map_snapshot(&map) }
            else {
                assert_eq!(request.native_map, Some(map.clone()));
                Ok(b"not a native receipt".to_vec())
            }
        }).is_err());
        assert_eq!(calls, ["--pd-iz-native-map-once", "--pd-iz-native-read-once"]);
        assert_eq!(c, original);
        assert_eq!(native_predecessor(&c).unwrap(), c.native_anchor.map(|a| a.predecessor));
    }

}

#[cfg(test)]
mod literal_answer_codec_regression {
    use super::*;
    #[test]
    fn literal_field_codec_has_no_implicit_labels_or_authority() {
        let empty:iz::IzAnswer=serde_json::from_str::<LiteralAnswer>("{}").unwrap().into();
        assert!(empty.is_empty());
        let answer:iz::IzAnswer=serde_json::from_str::<LiteralAnswer>(
            r#"{"positive":"Provë,","source_reference":"user report"}"#).unwrap().into();
        assert_eq!(answer.positive,"Provë,");
        assert_eq!(answer.source_reference,"user report");
        assert!(answer.negative.is_empty());
        for invalid in [
            r#"{"positive":null}"#,r#"{"positive":1}"#,r#"{"positive":true}"#,
            r#"{"positive":"a","positive":"b"}"#,r#"{"verdict":"1:1"}"#,
            r#"{"authority":"Shadow"}"#,r#"{"unknown":""}"#,"[]","null",
        ] {
            assert!(serde_json::from_str::<LiteralAnswer>(invalid).is_err(),"{invalid}");
        }
    }
}
