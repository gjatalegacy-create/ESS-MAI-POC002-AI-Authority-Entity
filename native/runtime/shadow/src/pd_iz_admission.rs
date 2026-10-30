//! POC-NEW same-Vault operational iZ admission.
//! Shadow validates references and observed ASHT holds, never reasons as a PD.
//! New material remains operational; no knowledge/verdict authority is issued.
use crate::knowledge_vault::KnowledgeVault;
use shadow_contracts::pd_memory::{self as pd, iz};
use std::path::PathBuf;

pub(crate) fn map_snapshot(
    context: &iz::IzContext,
    head: Option<&pd::PdOperationRecord>,
) -> Result<Option<iz::IzMapSnapshot>, String> {
    context.validate()?;
    match (context.parent.as_ref(), head) {
        (None, None) => Ok(None),
        (Some(parent), Some(record)) => {
            if record.request.identity != context.identity {
                return Err("IZ_MAP_FOREIGN_IDENTITY".into());
            }
            if !iz::source_is_iz(&record.request.source_input)
                && !pd::late_refusal::source_is_late(&record.request.source_input) {
                return Err("IZ_MAP_LEGACY_POLICY_REVIEW_REQUIRED".into());
            }
            let actual = iz::IzParent::from_record(record)?;
            if *parent != actual {
                return Err("IZ_MAP_STALE_OR_FOREIGN_PARENT".into());
            }
            let prior = iz::context_from_record(record)?;
            if prior.origin_i0 != context.origin_i0 {
                return Err("IZ_MAP_ORIGIN_CHANGED".into());
            }
            let snapshot = iz::IzMapSnapshot {
                identity_key: context.identity.key()?,
                head: actual.clone(),
                exact_record: actual,
                status: iz::IzMaterialStatus::OperationalNoVerdict,
            };
            snapshot.validate()?;
            Ok(Some(snapshot))
        }
        (None, Some(_)) => Err("IZ_MAP_EXISTING_HEAD_REQUIRES_PARENT".into()),
        (Some(_), None) => Err("IZ_MAP_PARENT_NOT_FOUND".into()),
    }
}

pub(crate) fn validate_native_admission(
    request: &pd::PdOperationRequest,
    vault: &KnowledgeVault,
    handoff_root: impl FnOnce() -> Result<PathBuf, String>,
    now_ns: impl FnOnce() -> Result<u64, String>,
) -> Result<(), String> {
    request.validate()?;
    let source = iz::decode_source(&request.source_input)?;
    // Resolve exact operation before ephemeral expiry or current-head CAS.
    // A historical successful append remains the same historical append.
    if let Some(prior) = vault.pd_operation(&request.identity, &request.operation_id)
        .map_err(|e| format!("IZ_OPERATION_LOOKUP:{e:?}"))? {
        return if prior.request == *request { Ok(()) }
        else { Err("IZ_OPERATION_CONFLICT".into()) };
    }
    // Exact historical retries returned above. New native writes use the
    // current actual-user-input policy; old PIZ1 history remains readable.
    if source.context.version == iz::IZ_VERSION {
        return Err("IZ_LEGACY_NEW_WRITE_DISABLED_USE_ICX2".into());
    }
    if source.context.version == iz::IZ_NATIVE_CONTINUITY_VERSION {
        vault.validate_pd_native_inquiry(request).map_err(|e| format!("IZ_NATIVE_HISTORY:{e:?}"))?;
    } else {
        let head = vault.pd_head(&request.identity)
            .map_err(|e| format!("IZ_HEAD_LOOKUP:{e:?}"))?;
        let mapped = map_snapshot(&source.context, head.as_ref())?;
        if mapped != source.context.map { return Err("IZ_ADMISSION_MAP_MISMATCH".into()); }
        iz::validate_parent(&source, head.as_ref())?;
        let (revision, state) = head.as_ref()
            .map(|r| (r.revision,r.state_digest)).unwrap_or((0,[0;32]));
        if request.expected_revision != revision || request.previous_state != state {
            return Err("IZ_CHECKPOINT_STALE_HEAD".into());
        }
        validate_checkpoint_transition(&source.context,&request.checkpoint,revision,head.as_ref())?;
    }
    // Every new cycle executed a NEW actual ASHT selection. Parent continuity
    // must never substitute for this attempt's real Shadow-staged response.
    let store = crate::selection_hold::SelectionHoldStore::under_handoff(handoff_root()?);
    crate::pd_failure_admission::verify_initial_hold(
        &store,&source.attempt.before.request,&source.attempt.response,now_ns()?)
        .map_err(|e| format!("IZ_ACTUAL_ASHT_HOLD:{e}"))
}

/// Version is explicit in the retained source, never guessed from bytes or clocks.
/// Old checkpoint bytes remain untouched; only their actual policy prefix is removed.
pub(crate) fn checkpoint_raw<'a>(context:&iz::IzContext,bytes:&'a [u8])->Result<&'a [u8],String>{
    context.validate()?;
    bytes.strip_prefix(context.checkpoint_prefix()).ok_or("IZ_CHECKPOINT_POLICY_VERSION".into())
}
pub(crate) fn validate_checkpoint_transition(
    context:&iz::IzContext,current:&[u8],expected_revision:u64,
    previous:Option<&pd::PdOperationRecord>,
)->Result<(),String>{
    let current=checkpoint_raw(context,current)?;
    let previous_raw=previous.map(iz::checkpoint_raw).transpose()?;
    // A PLF1 NoVerdict advances the operational CAS revision without appending
    // a user turn. Read the actual preserved cognitive count, including after
    // later PIZ replies; the independent storage CAS still uses revision/state.
    if previous.map(|record|record.revision).unwrap_or(0)!=expected_revision {
        return Err("IZ_CHECKPOINT_STALE_HEAD".into());
    }
    let session=pd::hex(&context.identity.key()?);
    let previous_turns=previous_raw.map(|raw|shadow_contracts::pd_checkpoint_data::checkpoint_metadata(raw,&session)
        .map(|metadata|u64::from(metadata.total_turns))).transpose()?.unwrap_or(0);
    shadow_contracts::pd_checkpoint_data::validate_user_checkpoint_transition(
        current,&session,&context.user_text,
        previous_turns,previous_raw).map_err(|e|format!("IZ_CHECKPOINT_TRANSITION:{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest,Sha256};
    fn context(version:u16)->iz::IzContext {
        let origin="Analizo motorin industrial".to_string();
        iz::IzContext{version,identity:pd::PdIdentity{owner:"test-owner".into(),obligation:"exact distinction".into(),
            case_id:"test-case".into(),scope:"WORK→NURA".into(),domain:"WORK→NURA".into(),
            lineage_root:pd::hex(&Sha256::digest(origin.as_bytes()).into())},operation_id:"initial".into(),
            origin_i0:origin.clone(),user_text:origin,parent:None,answer:iz::IzAnswer::default(),map:None,native_anchor:None,native_map:None}
    }
    #[test]
    fn checkpoint_policy_is_explicit_and_old_bytes_are_never_rewritten(){
        let old=context(iz::IZ_VERSION);let current=context(iz::IZ_ACTUAL_INPUT_VERSION);
        let legacy=[old.checkpoint_prefix(),b"retained-checkpoint-bytes"].concat();
        let actual=[current.checkpoint_prefix(),b"retained-checkpoint-bytes"].concat();
        let legacy_before=legacy.clone();let actual_before=actual.clone();
        assert_eq!(checkpoint_raw(&old,&legacy).unwrap(),b"retained-checkpoint-bytes");
        assert_eq!(checkpoint_raw(&current,&actual).unwrap(),b"retained-checkpoint-bytes");
        assert_eq!(checkpoint_raw(&old,&actual).unwrap_err(),"IZ_CHECKPOINT_POLICY_VERSION");
        assert_eq!(checkpoint_raw(&current,&legacy).unwrap_err(),"IZ_CHECKPOINT_POLICY_VERSION");
        let mut unsupported=current.clone();unsupported.version=4;
        assert!(checkpoint_raw(&unsupported,&legacy).is_err());
        assert_eq!(legacy,legacy_before);assert_eq!(actual,actual_before);
        // A matching prefix alone is insufficient: the original full schema
        // and rederived PD cognition remain mandatory for admission/replay.
        assert!(validate_checkpoint_transition(&old,&legacy,0,None).is_err());
        assert!(validate_checkpoint_transition(&current,&actual,0,None).is_err());
    }
    #[test]
    fn an_unknown_v2_reply_does_not_inherit_or_invent_a_supplied_field(){
        let mut reply=context(iz::IZ_ACTUAL_INPUT_VERSION);
        reply.operation_id="unknown-reply".into();reply.user_text="Nuk e di dallimin.".into();
        reply.parent=Some(iz::IzParent{record_id:[1;32],revision:1,state_digest:[2;32],question_sha256:[3;32]});
        reply.validate().unwrap();assert!(reply.answer.is_empty());
        // No raw input is silently replaced by origin/history in v2.
        assert_eq!(reply.current_i0(),"Nuk e di dallimin.");
        let legacy_bytes=[iz::IZ_CHECKPOINT_PREFIX,b"wrong-policy"].concat();
        assert!(checkpoint_raw(&reply,&legacy_bytes).is_err());
        assert!(reply.answer.is_empty());
    }
}
