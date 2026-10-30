//! V189_DERIVATIVE: exact data adapter for a prepared native PD turn.
//! Reconstruction does not mint LgcToken, GCL authorization, a Shadow receipt or
//! a verified closure. Actual T1 admission/receipt and T2 authority remain external.
use crate::pd_continuum_contract::{
    PdContinuumActivation, PdContinuumState, PdI0Stimulus, PdQuestionIncrement,
};
use crate::pd_spine_contract::{
    FixedMass, GclProcessAuthority, PdActivationContract, PdActivationOrigin, PdCognitiveMode,
    PdLayerReceipt, PdSpineCompletion, PdSpineRequest, PdSpineState,
};
use crate::progressive_debatic::{
    core::{build_package, ProgressiveDebatic},
    types::*,
};
use sha2::{Digest, Sha256};
use shadow_contracts::pd_prepared::{
    PdContinuationLink, PdPendingData, PdPreSealData, PdPreparedEnvelope,
    PdDialogueLink, PdIzOriginLink, PREPARED_IZ_ORIGIN_VERSION, PREPARED_CONTINUITY_VERSION, PREPARED_DIALOGUE_VERSION, PREPARED_MILLIS_CONTINUITY_VERSION, PREPARED_VERSION,
};
use shadow_contracts::{PdLayerEvidenceWire, PdSpineEvidenceWire};

fn need(ok: bool, message: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(format!("PD_PREPARED_NATIVE_{message}"))
    }
}
fn response_tag(value: PdResponseType) -> u8 {
    match value {
        PdResponseType::Help => 0,
        PdResponseType::Orient => 1,
        PdResponseType::Clarify => 2,
        PdResponseType::ChallengeGentle => 3,
        PdResponseType::ChallengeWithFacts => 4,
        PdResponseType::ScaffoldBullet => 5,
        PdResponseType::ScaffoldMeasure => 6,
        PdResponseType::ScaffoldStructure => 7,
        PdResponseType::Listen => 8,
    }
}
fn response(tag: u8) -> Result<PdResponseType, String> {
    match tag {
        0 => Ok(PdResponseType::Help),
        1 => Ok(PdResponseType::Orient),
        2 => Ok(PdResponseType::Clarify),
        3 => Ok(PdResponseType::ChallengeGentle),
        4 => Ok(PdResponseType::ChallengeWithFacts),
        5 => Ok(PdResponseType::ScaffoldBullet),
        6 => Ok(PdResponseType::ScaffoldMeasure),
        7 => Ok(PdResponseType::ScaffoldStructure),
        8 => Ok(PdResponseType::Listen),
        _ => Err("PD_PREPARED_NATIVE_RESPONSE_TAG".into()),
    }
}
fn continuum_state(tag: u8) -> Result<PdContinuumState, String> {
    match tag {
        0 => Ok(PdContinuumState::I0Stimulated),
        1 => Ok(PdContinuumState::QuestionIncremented),
        2 => Ok(PdContinuumState::SpineActivated),
        3 => Ok(PdContinuumState::VerifiedOutput),
        4 => Ok(PdContinuumState::IzCompleted),
        5 => Ok(PdContinuumState::NextI0Derived),
        _ => Err("PD_PREPARED_NATIVE_CONTINUUM_STATE".into()),
    }
}
fn layer(w: &PdLayerEvidenceWire) -> PdLayerReceipt {
    PdLayerReceipt {
        layer_mask: w.layer_mask,
        activation_id: w.activation_id,
        activation_contract_digest: w.activation_contract_digest,
        gcl_process_digest: w.gcl_process_digest,
        parent_result_digest: w.parent_result_digest,
        result_material_digest: w.result_material_digest,
        result_digest: w.result_digest,
        completed: w.completed,
    }
}
fn completion(p: &PdPendingData) -> Result<PdSpineCompletion, String> {
    let s = &p.spine;
    let c = &s.continuum;
    let a = &s.activation;
    let mode = match a.mode_code {
        0 => PdCognitiveMode::Intellect,
        1 => PdCognitiveMode::Philosophy,
        2 => PdCognitiveMode::Scientific,
        3 => PdCognitiveMode::Novel,
        _ => return Err("PD_PREPARED_NATIVE_MODE".into()),
    };
    let origin = match a.origin_code {
        0 => PdActivationOrigin::NormalTurn,
        1 => PdActivationOrigin::GeniusAutonomousResearch,
        2 => PdActivationOrigin::BudgetExhausted,
        3 => PdActivationOrigin::SessionClosed,
        _ => return Err("PD_PREPARED_NATIVE_ORIGIN".into()),
    };
    let state = match p.spine_request_state {
        3 => PdSpineState::SpineActivated,
        _ => return Err("PD_PREPARED_NATIVE_REQUEST_STATE".into()),
    };
    Ok(PdSpineCompletion {
        request: PdSpineRequest {
            contract_version: s.contract_version,
            activation_id: s.activation_id,
            session_digest: s.session_digest,
            parent_i0_digest: s.parent_i0_digest,
            trace_digest: s.trace_digest,
            turn_digest: s.turn_digest,
            created_at: s.created_at,
            state,
            continuum: PdContinuumActivation {
                contract_version: c.contract_version,
                stimulus: PdI0Stimulus {
                    contract_version: p.stimulus_contract_version,
                    session_digest: c.session_digest,
                    initial_i_digest: c.initial_i_digest,
                    user_input_digest: c.user_input_digest,
                    i0_digest: c.i0_digest,
                    stimulus_digest: c.stimulus_digest,
                    created_at: c.stimulus_created_at,
                    state: continuum_state(c.stimulus_state)?,
                },
                question_increment: PdQuestionIncrement {
                    contract_version: p.increment_contract_version,
                    stimulus_digest: p.increment_stimulus_digest,
                    unit_count: c.unit_count,
                    question_digest: c.question_digest,
                    response_kind_digest: c.response_kind_digest,
                    increment_digest: c.increment_digest,
                    created_at: c.increment_created_at,
                    state: continuum_state(c.increment_state)?,
                },
                activation_digest: c.activation_digest,
                state: continuum_state(c.activation_state)?,
            },
            activation: PdActivationContract {
                mode,
                origin,
                response_kind_digest: a.response_kind_digest,
                genius_signal_digest: a.genius_signal_digest,
                trace_mass: FixedMass(a.trace_mass),
                structural_coherence: FixedMass(a.structural_coherence),
                required_layer_mask: a.required_layer_mask,
                contract_digest: a.contract_digest,
            },
            gcl: GclProcessAuthority {
                law_seal: s.gcl_law_seal,
                system_laws_seal: s.gcl_system_laws_seal,
                phase_digest: s.gcl_phase_digest,
                process_digest: s.gcl_process_digest,
            },
        },
        layer_mask: s.layer_mask,
        layer1: layer(&s.layer1),
        layer2: layer(&s.layer2),
        layer3: layer(&s.layer3),
        evidence_digest: s.evidence_digest,
        mpro_mass: FixedMass(s.mpro_mass),
        ready_for_shadow: s.ready_for_shadow,
        completed_at: s.completed_at,
    })
}

/// Supply the actual encoded base ShadowCycleRequest. Its existing spine wire
/// carries original layer material, avoiding fabricated or digest-only layers.
pub fn from_pending(
    pending: &PdPendingNextI0,
    checkpoint: Vec<u8>,
    request_bytes: &[u8],
) -> Result<PdPreparedEnvelope, String> {
    let request = shadow_contracts::decode_cycle_request(request_bytes)
        .map_err(|e| format!("PD_PREPARED_NATIVE_SOURCE_CODEC: {e:?}"))?;
    let source_request_sha256 = Sha256::digest(request_bytes).into();
    let env = from_parts(
        pending,
        checkpoint,
        request.quantum.final_evidence.pd_spine,
        source_request_sha256,
    )?;
    env.validate_for_request(request_bytes)?;
    Ok(env)
}
/// R3: preserve cognitive history while binding new native attempt authority.
/// The link must originate from an explicit Shadow recovery lookup. Shadow
/// independently checks it against its own stored predecessor before T1.
pub fn from_pending_with_continuity(
    pending: &PdPendingNextI0,
    checkpoint: Vec<u8>,
    request_bytes: &[u8],
    link: PdContinuationLink,
) -> Result<PdPreparedEnvelope, String> {
    let request = shadow_contracts::decode_cycle_request(request_bytes)
        .map_err(|e| format!("PD_PREPARED_NATIVE_SOURCE_CODEC: {e:?}"))?;
    let env = from_parts_scoped(
        pending,
        checkpoint,
        request.quantum.final_evidence.pd_spine,
        Sha256::digest(request_bytes).into(),
        Some(link),
        None,
        None,
    )?;
    env.validate_for_request(request_bytes)?;
    Ok(env)
}
/// Initial transition restores actual PDS1 cognition; Shadow independently
/// resolves the referenced record and rechecks append-only history before T1.
pub fn from_pending_with_dialogue(
    pending:&PdPendingNextI0, checkpoint:Vec<u8>, request_bytes:&[u8],
    context:shadow_contracts::kie::KieContext,
    continuity:Option<PdContinuationLink>,
)->Result<PdPreparedEnvelope,String> {
    let request=shadow_contracts::decode_cycle_request(request_bytes)
        .map_err(|e|format!("PD_PREPARED_NATIVE_SOURCE_CODEC: {e:?}"))?;
    let env=from_parts_scoped(pending,checkpoint,request.quantum.final_evidence.pd_spine,
        Sha256::digest(request_bytes).into(),continuity,Some(PdDialogueLink::new(context)?),None)?;
    env.validate_for_request(request_bytes)?;Ok(env)
}
/// R4: source PIZ2 is resolved from Shadow and remains operational history.
/// Only the normal native pipeline produces the terminal receipt and T1/T2.
pub fn from_pending_with_iz(
    pending: &PdPendingNextI0, checkpoint: Vec<u8>, request_bytes: &[u8],
    link: PdIzOriginLink,
) -> Result<PdPreparedEnvelope, String> {
    let request=shadow_contracts::decode_cycle_request(request_bytes)
        .map_err(|e|format!("PD_PREPARED_NATIVE_SOURCE_CODEC: {e:?}"))?;
    let env=from_parts_scoped(pending,checkpoint,request.quantum.final_evidence.pd_spine,
        Sha256::digest(request_bytes).into(),link.context.native_anchor.clone(),None,Some(link))?;
    env.validate_for_request(request_bytes)?;
    Ok(env)
}
fn from_parts(
    pending: &PdPendingNextI0,
    checkpoint: Vec<u8>,
    spine: PdSpineEvidenceWire,
    source_request_sha256: [u8; 32],
) -> Result<PdPreparedEnvelope, String> {
    from_parts_scoped(pending, checkpoint, spine, source_request_sha256, None, None, None)
}
fn from_parts_scoped(
    pending: &PdPendingNextI0,
    checkpoint: Vec<u8>,
    spine: PdSpineEvidenceWire,
    source_request_sha256: [u8; 32],
    continuity: Option<PdContinuationLink>,
    source_dialogue: Option<PdDialogueLink>,
    source_iz: Option<PdIzOriginLink>,
) -> Result<PdPreparedEnvelope, String> {
    let native = &pending.spine_completion.request;
    need(
        native.state == PdSpineState::SpineActivated,
        "REQUEST_STATE",
    )?;
    let p = &pending.pre_seal;
    let env = PdPreparedEnvelope {
        version: if source_iz.is_some() && continuity.is_some() {
            shadow_contracts::pd_prepared::PREPARED_IZ_CONTINUITY_VERSION
        } else if source_dialogue.is_some() && continuity.is_some() {
            shadow_contracts::pd_prepared::PREPARED_DIALOGUE_CONTINUITY_VERSION
        } else if source_iz.is_some() {
            PREPARED_IZ_ORIGIN_VERSION
        } else if source_dialogue.is_some() {
            PREPARED_DIALOGUE_VERSION
        } else if continuity.as_ref().map(|link|link.clock_millis).unwrap_or(false) {
            PREPARED_MILLIS_CONTINUITY_VERSION
        } else if continuity.is_some() {
            PREPARED_CONTINUITY_VERSION
        } else {
            PREPARED_VERSION
        },
        source_request_sha256,
        checkpoint,
        continuity,
        source_dialogue,
        source_iz,
        failure_witness: None,
        pending: PdPendingData {
            session_id: pending.session_id.clone(),
            parent_i0: pending.parent_i0.clone(),
            turn_id: pending.turn_id.clone(),
            prepared_question: pending.prepared_question.clone(),
            response_type: response_tag(pending.response_type),
            prepared_at: pending.prepared_at,
            spine,
            spine_request_state: 3,
            stimulus_contract_version: native.continuum.stimulus.contract_version,
            increment_contract_version: native.continuum.question_increment.contract_version,
            increment_stimulus_digest: native.continuum.question_increment.stimulus_digest,
            pre_seal: PdPreSealData {
                seal: p.seal,
                session_id: p.session_id.clone(),
                parent_i0: p.parent_i0.clone(),
                receipt_id: p.receipt_id.clone(),
                prepared_at: p.prepared_at,
                spine_activation_id: p.spine_activation_id,
                spine_completion_digest: p.spine_completion_digest,
                layer_mask: p.layer_mask,
                mpro_mass: p.mpro_mass,
                action_sha256: p.action_sha256,
                law_trace_sha256: p.law_trace_sha256,
            },
        },
    };
    let restored = to_pending(&env)?;
    need(
        restored.spine_completion == pending.spine_completion,
        "SPINE_FIELD_DRIFT",
    )?;
    need(
        restored.binding_digest() == pending.binding_digest(),
        "PENDING_FIELD_DRIFT",
    )?;
    Ok(env)
}

/// Reconstruct only historical data. Caller must resolve the real stored T1
/// receipt before invoking native finalize_after_verification.
pub fn to_pending(env: &PdPreparedEnvelope) -> Result<PdPendingNextI0, String> {
    env.validate()?;
    let p = &env.pending;
    let seal = &p.pre_seal;
    let native = PdPendingNextI0 {
        session_id: p.session_id.clone(),
        parent_i0: p.parent_i0.clone(),
        turn_id: p.turn_id.clone(),
        prepared_question: p.prepared_question.clone(),
        response_type: response(p.response_type)?,
        prepared_at: p.prepared_at,
        spine_completion: completion(p)?,
        pre_seal: PdPreSealReceipt {
            seal: seal.seal,
            session_id: seal.session_id.clone(),
            parent_i0: seal.parent_i0.clone(),
            receipt_id: seal.receipt_id.clone(),
            prepared_at: seal.prepared_at,
            spine_activation_id: seal.spine_activation_id,
            spine_completion_digest: seal.spine_completion_digest,
            layer_mask: seal.layer_mask,
            mpro_mass: seal.mpro_mass,
            action_sha256: seal.action_sha256,
            law_trace_sha256: seal.law_trace_sha256,
        },
    };
    need(native.spine_completion.closes_all_layers(), "LAYERS")?;
    need(
        native.spine_completion.completion_digest() == p.spine.completion_digest,
        "COMPLETION_DIGEST",
    )?;
    need(
        native.binding_digest() == p.binding_digest()?,
        "PENDING_DIGEST",
    )?;
    let mut pd = ProgressiveDebatic::new();
    pd.import_checkpoint(env.cognitive_session_id(), &env.checkpoint)?;
    let session = pd
        .session(env.cognitive_session_id())
        .ok_or("PD_PREPARED_NATIVE_CHECKPOINT_SESSION")?;
    let last = session
        .turns
        .last()
        .ok_or("PD_PREPARED_NATIVE_CHECKPOINT_TURN")?;
    let request = &native.spine_completion.request;
    need(
        request.created_at == last.user_signal.timestamp
            && request.continuum.stimulus.user_input_digest
                == crate::pd_spine_contract::digest_parts(&[last
                    .user_signal
                    .raw_content
                    .as_bytes()]),
        "CHECKPOINT_INPUT",
    )?;
    match request.activation.origin {
        PdActivationOrigin::NormalTurn => {
            need(
                p.turn_id == last.turn_id
                    && p.prepared_question == last.pd_response_hint
                    && p.response_type == response_tag(last.pd_response_type),
                "CHECKPOINT_NORMAL_PENDING",
            )?;
            need(
                request.trace_digest
                    == crate::pd_spine_contract::digest_parts(&[last
                        .user_signal
                        .signal_id
                        .as_bytes()])
                    && request.turn_digest
                        == crate::pd_spine_contract::digest_parts(&[last.turn_id.as_bytes()]),
                "CHECKPOINT_NORMAL_REFS",
            )?;
        }
        PdActivationOrigin::GeniusAutonomousResearch => {
            let genius = session
                .genius_signal
                .as_ref()
                .ok_or("PD_PREPARED_NATIVE_CHECKPOINT_GENIUS")?;
            need(
                genius.detected_at_turn == session.total_turns
                    && genius.timestamp == last.user_signal.timestamp,
                "CHECKPOINT_STALE_GENIUS",
            )?;
            let package = build_package(session, genius);
            let question = format!(
                "Cila provë konkrete e ngre konceptin '{}' në hapin tjetër?",
                package.dominant_concept
            );
            need(
                p.turn_id == genius.signal_id
                    && p.prepared_question == question
                    && p.response_type == 7,
                "CHECKPOINT_GENIUS_PENDING",
            )?;
            need(
                request.trace_digest
                    == crate::pd_spine_contract::digest_parts(&[genius.trace_ref.as_bytes()])
                    && request.turn_digest
                        == crate::pd_spine_contract::digest_parts(&[package.package_id.as_bytes()]),
                "CHECKPOINT_GENIUS_REFS",
            )?;
        }
        _ => return Err("PD_PREPARED_NATIVE_NO_PENDING_FOR_TERMINAL_OUTPUT".into()),
    }
    Ok(native)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pd_continuum_contract::PD_CONTINUUM_CONTRACT_VERSION;
    use crate::pd_spine_contract::{digest_parts, ALL_LAYERS_MASK};
    use shadow_contracts::pd_prepared::{decode_prepared, encode_prepared};
    use shadow_contracts::{PdActivationEvidenceWire, PdContinuumEvidenceWire};

    // This is an explicit unit fixture for data equivalence, never proof of a
    // production hardware/authority run. PD ingest and seal calls are real.
    pub(super) fn fixture() -> (PdPreparedEnvelope, PdPendingNextI0) {
        let mut pd = ProgressiveDebatic::new();
        pd.open_session_sealed("s1", "science", DebateMode::Scientific, 10, PD_SEAL)
            .unwrap();
        let cycle = pd
            .ingest_for_spine_sealed(
                "s1",
                "i0-00000000000000a1",
                "Which evidence establishes the measured rotation?",
                10,
                PD_SEAL,
            )
            .unwrap();
        prepared_from_cycle(&pd, cycle, "s1", "s1", "i0-00000000000000a1", 10, None)
    }
    pub(super) fn prepared_from_cycle(
        pd: &ProgressiveDebatic,
        cycle: PdSpineCycle,
        cognitive: &str,
        attempt: &str,
        parent: &str,
        ts: u64,
        link: Option<PdContinuationLink>,
    ) -> (PdPreparedEnvelope, PdPendingNextI0) {
        prepared_from_cycle_with_dialogue(pd,cycle,cognitive,attempt,parent,ts,link,None)
    }
    pub(super) fn prepared_from_cycle_with_dialogue(
        pd:&ProgressiveDebatic,cycle:PdSpineCycle,cognitive:&str,attempt:&str,parent:&str,ts:u64,
        link:Option<PdContinuationLink>,dialogue:Option<PdDialogueLink>,
    )->(PdPreparedEnvelope,PdPendingNextI0) {
        prepared_from_cycle_with_links(pd,cycle,cognitive,attempt,parent,ts,link,dialogue,None)
    }
    pub(super) fn prepared_from_cycle_with_links(
        pd:&ProgressiveDebatic,cycle:PdSpineCycle,cognitive:&str,attempt:&str,parent:&str,ts:u64,
        link:Option<PdContinuationLink>,dialogue:Option<PdDialogueLink>,iz:Option<PdIzOriginLink>,
    )->(PdPreparedEnvelope,PdPendingNextI0) {
        let r = &cycle.request;
        let materials = [
            b"unit layer one".to_vec(),
            b"unit layer two".to_vec(),
            b"unit layer three".to_vec(),
        ];
        let l1 = PdLayerReceipt::new(
            1,
            r.activation_id,
            r.activation.contract_digest,
            r.gcl.process_digest,
            0,
            digest_parts(&[&materials[0]]),
            true,
        );
        let l2 = PdLayerReceipt::new(
            2,
            r.activation_id,
            r.activation.contract_digest,
            r.gcl.process_digest,
            l1.result_digest,
            digest_parts(&[&materials[1]]),
            true,
        );
        let l3 = PdLayerReceipt::new(
            4,
            r.activation_id,
            r.activation.contract_digest,
            r.gcl.process_digest,
            l2.result_digest,
            digest_parts(&[&materials[2]]),
            true,
        );
        let completion = PdSpineCompletion {
            request: r.clone(),
            layer_mask: ALL_LAYERS_MASK,
            layer1: l1,
            layer2: l2,
            layer3: l3,
            evidence_digest: 17,
            mpro_mass: FixedMass(6500),
            ready_for_shadow: true,
            completed_at: ts,
        };
        let prepared = pd
            .prepare_after_spine_sealed(attempt, parent, cycle, completion, ts, PD_SEAL)
            .unwrap();
        let pending = prepared.pending_next.unwrap();
        let c = &pending.spine_completion;
        let r = &c.request;
        let cont = &r.continuum;
        let a = &r.activation;
        let lw = |l: &PdLayerReceipt, material: Vec<u8>| PdLayerEvidenceWire {
            layer_mask: l.layer_mask,
            activation_id: l.activation_id,
            activation_contract_digest: l.activation_contract_digest,
            gcl_process_digest: l.gcl_process_digest,
            parent_result_digest: l.parent_result_digest,
            result_material: material,
            result_material_digest: l.result_material_digest,
            result_digest: l.result_digest,
            completed: l.completed,
        };
        let wire = PdSpineEvidenceWire {
            contract_version: r.contract_version,
            activation_id: r.activation_id,
            session_digest: r.session_digest,
            parent_i0_digest: r.parent_i0_digest,
            trace_digest: r.trace_digest,
            turn_digest: r.turn_digest,
            created_at: r.created_at,
            continuum: PdContinuumEvidenceWire {
                contract_version: PD_CONTINUUM_CONTRACT_VERSION,
                session_digest: cont.stimulus.session_digest,
                initial_i_digest: cont.stimulus.initial_i_digest,
                user_input_digest: cont.stimulus.user_input_digest,
                i0_digest: cont.stimulus.i0_digest,
                stimulus_digest: cont.stimulus.stimulus_digest,
                stimulus_created_at: cont.stimulus.created_at,
                stimulus_state: 0,
                unit_count: cont.question_increment.unit_count,
                question_digest: cont.question_increment.question_digest,
                response_kind_digest: cont.question_increment.response_kind_digest,
                increment_digest: cont.question_increment.increment_digest,
                increment_created_at: cont.question_increment.created_at,
                increment_state: 1,
                activation_digest: cont.activation_digest,
                activation_state: 2,
            },
            activation: PdActivationEvidenceWire {
                mode_code: match a.mode {
                    PdCognitiveMode::Intellect => 0,
                    PdCognitiveMode::Philosophy => 1,
                    PdCognitiveMode::Scientific => 2,
                    PdCognitiveMode::Novel => 3,
                },
                origin_code: match a.origin {
                    PdActivationOrigin::NormalTurn => 0,
                    PdActivationOrigin::GeniusAutonomousResearch => 1,
                    PdActivationOrigin::BudgetExhausted => 2,
                    PdActivationOrigin::SessionClosed => 3,
                },
                response_kind_digest: a.response_kind_digest,
                genius_signal_digest: a.genius_signal_digest,
                trace_mass: a.trace_mass.raw(),
                structural_coherence: a.structural_coherence.raw(),
                required_layer_mask: a.required_layer_mask,
                contract_digest: a.contract_digest,
            },
            continuum_activation_digest: cont.activation_digest,
            activation_contract_digest: a.contract_digest,
            gcl_law_seal: r.gcl.law_seal,
            gcl_system_laws_seal: r.gcl.system_laws_seal,
            gcl_phase_digest: r.gcl.phase_digest,
            gcl_process_digest: r.gcl.process_digest,
            layer_mask: c.layer_mask,
            layer1: lw(&c.layer1, materials[0].clone()),
            layer2: lw(&c.layer2, materials[1].clone()),
            layer3: lw(&c.layer3, materials[2].clone()),
            evidence_digest: c.evidence_digest,
            mpro_mass: c.mpro_mass.raw(),
            ready_for_shadow: c.ready_for_shadow,
            completed_at: c.completed_at,
            completion_digest: c.completion_digest(),
        };
        let checkpoint = pd.export_checkpoint(cognitive).unwrap();
        let env = from_parts_scoped(&pending, checkpoint, wire, [7; 32], link, dialogue, iz).unwrap();
        (env, pending)
    }
    #[test]
    fn portable_data_roundtrip_preserves_native_pending_and_checkpoint() {
        let (env, native) = fixture();
        let bytes = encode_prepared(&env).unwrap();
        let decoded = decode_prepared(&bytes).unwrap();
        assert_eq!(decoded, env);
        let restored = to_pending(&decoded).unwrap();
        assert_eq!(restored.spine_completion, native.spine_completion);
        assert_eq!(restored.binding_digest(), native.binding_digest());
        let back = from_parts(
            &restored,
            env.checkpoint.clone(),
            env.pending.spine.clone(),
            env.source_request_sha256,
        )
        .unwrap();
        assert_eq!(back, env);
    }
    #[test]
    fn question_layers_versions_state_and_preseal_tampering_are_rejected() {
        let (env, _) = fixture();
        let mut bad = env.clone();
        bad.pending.prepared_question.push('?');
        assert!(encode_prepared(&bad).is_err());
        let mut bad = env.clone();
        bad.pending.spine.layer2.result_material.push(0);
        assert!(encode_prepared(&bad).is_err());
        let mut bad = env.clone();
        bad.pending.stimulus_contract_version ^= 1;
        assert!(encode_prepared(&bad).is_err());
        let mut bad = env.clone();
        bad.pending.spine_request_state = 7;
        assert!(encode_prepared(&bad).is_err());
        let mut bad = env.clone();
        bad.pending.pre_seal.action_sha256[0] ^= 1;
        assert!(encode_prepared(&bad).is_err());
        let mut bad = env.clone();
        bad.pending.response_type = 255;
        assert!(encode_prepared(&bad).is_err());
        let mut bad = env.clone();
        bad.pending.spine.mpro_mass = 10001;
        assert!(encode_prepared(&bad).is_err());
        let mut bad = env.clone();
        bad.checkpoint[0] ^= 1;
        assert!(to_pending(&bad).is_err());
        assert!(env
            .validate_for_request(b"not the bound source request")
            .is_err());
    }
    #[test]
    fn truncated_oversize_and_tampered_frames_never_decode() {
        let (env, _) = fixture();
        let bytes = encode_prepared(&env).unwrap();
        for end in [0, 8, 42, bytes.len() / 2, bytes.len() - 1] {
            assert!(decode_prepared(&bytes[..end]).is_err());
        }
        let mut bad = bytes.clone();
        bad[42] ^= 1;
        assert!(decode_prepared(&bad).is_err());
        let mut bad = bytes;
        bad.push(0);
        assert!(decode_prepared(&bad).is_err());
    }
}

/// Compare every native closure field with data rederived by Shadow from stored T1.
/// This comparison does not grant authority: the caller must retrieve durable T2
/// itself and still run the original finalize_after_verification guard.
pub fn verify_continuation(
    data: &shadow_contracts::pd_prepared::ContinuationData,
    closure: &PdContinuumClosure,
) -> Result<(), String> {
    data.validate()?;
    let basis = match data.basis {
        1 => PdContinuationBasis::VerifiedPositive,
        2 => PdContinuationBasis::VerifiedNegativeRebuild,
        _ => return Err("PD_CONTINUATION_NATIVE_BASIS".into()),
    };
    let contract_basis = match data.basis {
        1 => crate::pd_continuum_contract::PdContinuumBasis::VerifiedPositive,
        _ => crate::pd_continuum_contract::PdContinuumBasis::VerifiedNegativeRebuild,
    };
    let n = &closure.next_i0;
    let o = &closure.output;
    let z = &closure.iz;
    let c = &closure.contract;
    macro_rules! eq_field {
        ($actual:expr,$expected:expr,$label:literal) => {
            need($actual == $expected, concat!("CONTINUATION_", $label))?;
        };
    }
    eq_field!(n.session_id, data.session_id, "NEXT_SESSION_ID");
    eq_field!(n.parent_i0, data.parent_i0, "NEXT_PARENT_I0");
    eq_field!(n.question, data.question, "NEXT_QUESTION");
    eq_field!(n.y_verdict, data.y_verdict, "NEXT_Y_VERDICT");
    eq_field!(n.x_verdict, data.x_verdict, "NEXT_X_VERDICT");
    eq_field!(
        n.verification_seal,
        data.verification_seal,
        "NEXT_VERIFICATION_SEAL"
    );
    eq_field!(
        n.verification_generation,
        data.verification_generation,
        "NEXT_VERIFICATION_GENERATION"
    );
    eq_field!(
        n.verification_receipt_id,
        data.verification_receipt_id,
        "NEXT_VERIFICATION_RECEIPT_ID"
    );
    eq_field!(
        n.primitive_anchor,
        data.primitive_anchor,
        "NEXT_PRIMITIVE_ANCHOR"
    );
    eq_field!(n.xy_digest, data.xy_digest, "NEXT_XY_DIGEST");
    eq_field!(
        n.pd_binding_digest,
        data.pd_binding_digest,
        "NEXT_PD_BINDING_DIGEST"
    );
    eq_field!(
        n.continuum_activation_digest,
        data.continuum_activation_digest,
        "NEXT_CONTINUUM_ACTIVATION_DIGEST"
    );
    eq_field!(
        n.question_increment_digest,
        data.question_increment_digest,
        "NEXT_QUESTION_INCREMENT_DIGEST"
    );
    eq_field!(n.pd_output_id, data.pd_output_id, "NEXT_PD_OUTPUT_ID");
    eq_field!(
        n.pd_output_digest,
        data.pd_output_digest,
        "NEXT_PD_OUTPUT_DIGEST"
    );
    eq_field!(n.iz_id, data.iz_id, "NEXT_IZ_ID");
    eq_field!(n.iz_digest, data.iz_digest, "NEXT_IZ_DIGEST");
    eq_field!(n.iz_sha256, data.iz_sha256, "NEXT_IZ_SHA256");
    eq_field!(n.next_i0_id, data.next_i0_id, "NEXT_NEXT_I0_ID");
    eq_field!(
        n.living_trust_sha256,
        data.living_trust_sha256,
        "NEXT_LIVING_TRUST_SHA256"
    );
    eq_field!(
        n.living_trust_intensity,
        data.living_trust_intensity,
        "NEXT_LIVING_TRUST_INTENSITY"
    );
    eq_field!(
        n.living_trust_kind,
        data.living_trust_kind,
        "NEXT_LIVING_TRUST_KIND"
    );
    eq_field!(
        n.living_trust_sovereign_value,
        data.living_trust_sovereign_value,
        "NEXT_LIVING_TRUST_SOVEREIGN_VALUE"
    );
    eq_field!(
        n.living_trust_digest,
        data.living_trust_digest,
        "NEXT_LIVING_TRUST_DIGEST"
    );
    eq_field!(
        n.continuum_completion_digest,
        data.continuum_completion_digest,
        "NEXT_CONTINUUM_COMPLETION_DIGEST"
    );
    eq_field!(n.basis, basis, "NEXT_BASIS");
    eq_field!(o.output_id, data.pd_output_id, "OUTPUT_OUTPUT_ID");
    eq_field!(
        o.output_digest,
        data.pd_output_digest,
        "OUTPUT_OUTPUT_DIGEST"
    );
    eq_field!(o.session_id, data.session_id, "OUTPUT_SESSION_ID");
    eq_field!(o.parent_i0, data.parent_i0, "OUTPUT_PARENT_I0");
    eq_field!(o.turn_id, data.turn_id, "OUTPUT_TURN_ID");
    eq_field!(o.question, data.question, "OUTPUT_QUESTION");
    eq_field!(o.mpro_mass, data.mpro_mass, "OUTPUT_MPRO_MASS");
    eq_field!(o.y_verdict, data.y_verdict, "OUTPUT_Y_VERDICT");
    eq_field!(o.x_verdict, data.x_verdict, "OUTPUT_X_VERDICT");
    eq_field!(
        o.verification_receipt_id,
        data.verification_receipt_id,
        "OUTPUT_VERIFICATION_RECEIPT_ID"
    );
    eq_field!(
        response_tag(o.response_type),
        data.response_type,
        "OUTPUT_RESPONSE_TYPE"
    );
    eq_field!(z.iz_id, data.iz_id, "IZ_IZ_ID");
    eq_field!(z.iz_digest, data.iz_digest, "IZ_IZ_DIGEST");
    eq_field!(z.iz_sha256, data.iz_sha256, "IZ_IZ_SHA256");
    eq_field!(z.origin_i0, data.parent_i0, "IZ_ORIGIN_I0");
    eq_field!(
        z.question_increment_digest,
        data.question_increment_digest,
        "IZ_QUESTION_INCREMENT_DIGEST"
    );
    eq_field!(
        z.pd_output_digest,
        data.pd_output_digest,
        "IZ_PD_OUTPUT_DIGEST"
    );
    eq_field!(
        z.verification_receipt_id,
        data.verification_receipt_id,
        "IZ_VERIFICATION_RECEIPT_ID"
    );
    eq_field!(
        z.living_trust_sha256,
        data.living_trust_sha256,
        "IZ_LIVING_TRUST_SHA256"
    );
    eq_field!(
        z.living_trust_intensity,
        data.living_trust_intensity,
        "IZ_LIVING_TRUST_INTENSITY"
    );
    eq_field!(
        z.living_trust_kind,
        data.living_trust_kind,
        "IZ_LIVING_TRUST_KIND"
    );
    eq_field!(
        z.living_trust_sovereign_value,
        data.living_trust_sovereign_value,
        "IZ_LIVING_TRUST_SOVEREIGN_VALUE"
    );
    eq_field!(
        z.living_trust_digest,
        data.living_trust_digest,
        "IZ_LIVING_TRUST_DIGEST"
    );
    eq_field!(z.basis, basis, "IZ_BASIS");
    eq_field!(
        c.contract_version,
        crate::pd_continuum_contract::PD_CONTINUUM_CONTRACT_VERSION,
        "CONTRACT_VERSION"
    );
    eq_field!(
        c.activation_digest,
        data.continuum_activation_digest,
        "CONTRACT_ACTIVATION"
    );
    eq_field!(
        c.question_increment_digest,
        data.question_increment_digest,
        "CONTRACT_INCREMENT"
    );
    eq_field!(c.pd_output_digest, data.pd_output_digest, "CONTRACT_OUTPUT");
    eq_field!(c.iz_digest, data.iz_digest, "CONTRACT_IZ");
    eq_field!(
        c.living_trust_digest,
        data.living_trust_digest,
        "CONTRACT_TRUST"
    );
    eq_field!(c.next_i0_id, data.next_i0_id, "CONTRACT_NEXT");
    eq_field!(
        c.verification_receipt_digest,
        crate::pd_continuum_contract::digest_parts(&[data.verification_receipt_id.as_bytes()]),
        "CONTRACT_RECEIPT"
    );
    eq_field!(c.basis, contract_basis, "CONTRACT_BASIS");
    eq_field!(
        c.output_state,
        PdContinuumState::VerifiedOutput,
        "CONTRACT_OUTPUT_STATE"
    );
    eq_field!(
        c.iz_state,
        PdContinuumState::IzCompleted,
        "CONTRACT_IZ_STATE"
    );
    eq_field!(
        c.completion_digest,
        data.continuum_completion_digest,
        "CONTRACT_COMPLETION"
    );
    eq_field!(c.state, PdContinuumState::NextI0Derived, "CONTRACT_STATE");
    need(
        c.closes_formula(&data.verification_receipt_id),
        "CONTINUATION_NATIVE_FORMULA",
    )?;
    Ok(())
}

#[cfg(test)]
mod fixture_output { include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../test_support/output_dir.rs")); }
#[cfg(test)]
mod pinned_fixture { include!(concat!(env!("CARGO_MANIFEST_DIR"), "/../test_support/prepared_fixture.rs")); }

#[cfg(test)]
mod continuation_tests {
    use super::tests::fixture;
    use super::*;

    // These fixtures are synthetic receipt data for formula differential tests.
    // They are not evidence of native Shadow adjudication or a durable T1.

    #[test]
    fn export_synthetic_prepared_continuation_fixtures() {
        use shadow_contracts::pd_prepared::{
            derive_continuation, encode_continuation, encode_prepared,
        };
        use std::io::Write;
        let output = super::fixture_output::OutputDir::new("ESSMAI_PD_PREPARED_FIXTURE_DIR", "prepared");
        let root = output.path();
        let (env, pending) = fixture();
        assert_source_formula_matrix(&env, &pending);
        for (negative, name) in [(false, "positive"), (true, "negative")] {
            let dir = root.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            let response = synthetic_response(&synthetic_completion(&pending, negative));
            let data = derive_continuation(&env, &response).unwrap();
            for (name,bytes) in [
                ("prepared.bin",encode_prepared(&env).unwrap()),
                ("response.wire",shadow_contracts::encode_cycle_response(&response)),
                ("continuation.bin",encode_continuation(&data).unwrap()),
                ("FIXTURE_NOTICE.txt",b"SYNTHETIC FORMULA/STORAGE TEST ONLY. Real PD checkpoint/pending; synthetic layers/receipt and unbound request SHA [7;32]. No native GCL admission, no real Shadow receipt, no production knowledge.\n".to_vec())
            ] {
                if name != "FIXTURE_NOTICE.txt" {
                    assert_eq!(bytes, super::pinned_fixture::bytes(dir.file_name().unwrap().to_str().unwrap(), name), "regenerated fixture drift");
                }
                let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(dir.join(name)).unwrap();
                file.write_all(&bytes).unwrap();file.sync_all().unwrap();
            }
        }
        println!("R20_MATH {}", r#"{"test":"export_synthetic_prepared_continuation_fixtures","checks":["scalar_fnv64_modulo","receipt_sha256_length_prefix","trust_intensity_generation_boundary_matrix","output_iz_next_completion_scalar_chain","iz_sha256_preseal_binding","full_prepared_response_bindings","native_finalizer_differential","canonical_continuation_roundtrip","resealed_invalid_bounds_refused","negative_requires_persistence"]}"#);
    }

    // R20 independent scalar oracle: the source FNV recurrence over bytes is
    // computed in u128 modulo 2^64 instead of calling any production digest.
    // It is an identifier formula, never a collision-proof authority primitive.
    fn scalar_digest(parts: &[&[u8]]) -> u64 {
        let mut state = 0xcbf2_9ce4_8422_2325u128;
        for byte in parts.iter().flat_map(|part| part.iter()) {
            state = ((state ^ u128::from(*byte)) * 1_099_511_628_211u128) % (1u128 << 64);
        }
        state as u64
    }
    fn scalar_sha(bytes: &[u8]) -> [u8; 32] { Sha256::digest(bytes).into() }
    fn bind_synthetic_completion(c: &mut PdVerificationCompletion) {
        c.living_trust_digest = scalar_digest(&[
            &c.living_trust_sha256, &c.living_trust_intensity.to_le_bytes(),
            &[c.living_trust_kind], &c.living_trust_sovereign_value.to_le_bytes(),
            b"GCL_LIVING_TRUST_TO_IZ_V164",
        ]);
        // Independent construction of the source receipt SHA preimage. Fixed
        // width integers are little-endian; text lengths count UTF-8 bytes.
        let mut receipt = b"ESS_MAI_VERIFICATION_RECEIPT_SHA256_V162".to_vec();
        receipt.extend_from_slice(&0x0001_0602u32.to_le_bytes());
        for text in [&c.session_id, &c.parent_i0] {
            receipt.extend_from_slice(&(text.len() as u64).to_le_bytes());
            receipt.extend_from_slice(text.as_bytes());
        }
        for value in [c.primitive_anchor, c.xy_digest, c.pd_binding_digest,
            c.pd_continuum_activation_digest] {
            receipt.extend_from_slice(&value.to_le_bytes());
        }
        receipt.extend_from_slice(&c.living_trust_sha256);
        receipt.extend_from_slice(&[c.y_verdict,c.x_verdict]);
        receipt.extend_from_slice(&c.verification_generation.to_le_bytes());
        receipt.extend_from_slice(&c.verification_seal.to_le_bytes());
        c.verification_receipt_id = scalar_sha(&receipt).iter().map(|v|format!("{v:02x}")).collect();
    }
    pub(super) fn assert_source_formula_matrix(env: &PdPreparedEnvelope, pending: &PdPendingNextI0) {
        use shadow_contracts::pd_prepared::{decode_continuation, derive_continuation, encode_continuation, encode_prepared};
        use crate::pd_continuum_contract::PD_CONTINUUM_CONTRACT_VERSION;
        let encoded_prepared = encode_prepared(env).unwrap();
        let version = PD_CONTINUUM_CONTRACT_VERSION.to_le_bytes();
        // These 30 rows are synthetic receipt/formula checks, not 30 independent
        // scientific observations and not a native positive-admission proof.
        for negative in [false,true] {
            for intensity in [0,1,5000,9999,10000] {
                for generation in [1,4,u64::MAX] {
                    let mut completion = synthetic_completion(pending,negative);
                    completion.living_trust_intensity = intensity;
                    completion.verification_generation = generation;
                    bind_synthetic_completion(&mut completion);
                    let response = synthetic_response(&completion);
                    let actual = derive_continuation(env,&response).unwrap();
                    let native = ProgressiveDebatic::new().finalize_after_verification(pending.clone(),completion.clone());
                    let closure = match native {
                        PdFinalization::Released(value) if !negative => value,
                        PdFinalization::RebuiltFromNegative(value) if negative => value,
                        other => panic!("synthetic formula/native finalizer divergence: {other:?}"),
                    };
                    verify_continuation(&actual,&closure).unwrap();
                    let expected_question = if negative {
                        format!("Cila provë konkrete duhet të zëvendësojë bazën e rrëzuar: {}",pending.prepared_question)
                    }else{pending.prepared_question.clone()};
                    let label = if negative {"VERIFIED_NEGATIVE_REBUILD"}else{"VERIFIED_POSITIVE"};
                    let activation = pending.spine_completion.request.continuum.activation_digest;
                    let increment = pending.spine_completion.request.continuum.question_increment.increment_digest;
                    let output = scalar_digest(&[&version,&activation.to_le_bytes(),
                        &pending.binding_digest().to_le_bytes(),&increment.to_le_bytes(),
                        completion.verification_receipt_id.as_bytes(),expected_question.as_bytes(),
                        &[completion.y_verdict],&[completion.x_verdict],label.as_bytes(),b"PD_VERIFIED_OUTPUT"]);
                    let iz = scalar_digest(&[&version,&activation.to_le_bytes(),&increment.to_le_bytes(),
                        &output.to_le_bytes(),&completion.living_trust_digest.to_le_bytes(),
                        completion.verification_receipt_id.as_bytes(),label.as_bytes(),b"PD_COMPLETES_IZ_WITH_LIVING_TRUST_V161"]);
                    let next = format!("i0-{:016x}",scalar_digest(&[&version,&output.to_le_bytes(),&iz.to_le_bytes(),b"PD_OUTPUT_PLUS_IZ_TO_NEXT_I0"]));
                    let complete = scalar_digest(&[&version,&activation.to_le_bytes(),&increment.to_le_bytes(),
                        &output.to_le_bytes(),&iz.to_le_bytes(),&completion.living_trust_digest.to_le_bytes(),
                        next.as_bytes(),completion.verification_receipt_id.as_bytes(),label.as_bytes(),b"PD_CONTINUUM_COMPLETE"]);
                    let mut material = b"ESS_MAI_IZ_V157".to_vec();
                    for value in [activation,increment,output] { material.extend_from_slice(&value.to_le_bytes()); }
                    material.extend_from_slice(completion.verification_receipt_id.as_bytes());
                    material.extend_from_slice(&completion.living_trust_sha256);
                    material.extend_from_slice(&intensity.to_le_bytes());
                    material.push(completion.living_trust_kind);
                    material.extend_from_slice(&500u32.to_le_bytes());
                    material.extend_from_slice(&completion.living_trust_digest.to_le_bytes());
                    material.extend_from_slice(&pending.pre_seal.action_sha256);
                    material.extend_from_slice(&pending.pre_seal.law_trace_sha256);
                    assert_eq!(actual.question,expected_question);
                    assert_eq!(actual.pd_output_digest,output);
                    assert_eq!(actual.pd_output_id,format!("pdo-{output:016x}"));
                    assert_eq!(actual.iz_digest,iz);
                    assert_eq!(actual.iz_id,format!("iZ-{iz:016x}"));
                    assert_eq!(actual.next_i0_id,next); // A continuation identifier, not predicted user words.
                    assert_eq!(actual.continuum_completion_digest,complete);
                    assert_eq!(actual.iz_sha256,scalar_sha(&material));
                    assert_eq!(actual.living_trust_intensity,intensity);
                    assert_eq!(actual.verification_generation,generation);
                    assert_eq!(actual.prepared_sha256,scalar_sha(&encoded_prepared));
                    assert_eq!(actual.source_request_sha256,env.source_request_sha256);
                    assert_eq!(actual.shadow_response_sha256,scalar_sha(&shadow_contracts::encode_cycle_response(&response)));
                    actual.validate_for(env,&response).unwrap();
                    let bytes=encode_continuation(&actual).unwrap();
                    assert_eq!(decode_continuation(&bytes).unwrap(),actual);
                    assert_eq!(derive_continuation(env,&response).unwrap(),actual);
                }
            }
            for defect in 0..5 {
                let mut invalid=synthetic_completion(pending,negative);
                match defect {
                    0=>invalid.living_trust_intensity=10_001,
                    1=>invalid.living_trust_intensity=u32::MAX,
                    2=>invalid.verification_generation=0,
                    3=>invalid.x_verdict^=1,
                    _=>invalid.living_trust_sha256=[0;32],
                }
                // All ordinary checksums are consistent: semantic bounds, not
                // an accidentally stale digest, must reject these data-only rows.
                bind_synthetic_completion(&mut invalid);
                assert!(derive_continuation(env,&synthetic_response(&invalid)).is_err());
                assert!(matches!(ProgressiveDebatic::new().finalize_after_verification(pending.clone(),invalid),PdFinalization::Rejected{..}));
            }
        }
        let mut no_negative=synthetic_response(&synthetic_completion(pending,true));
        no_negative.negative_persisted=false;
        assert!(derive_continuation(env,&no_negative).unwrap_err().contains("NEGATIVE_PERSISTENCE"));
        assert_eq!(encode_prepared(env).unwrap(),encoded_prepared);
    }

    pub(super) fn synthetic_completion(
        pending: &PdPendingNextI0,
        negative: bool,
    ) -> PdVerificationCompletion {
        let pair = if negative { 0 } else { 1 };
        let trust = crate::living_trust_contract::LivingTrustSeal {
            identity_sha256: [0x31; 32],
            intensity: 7321,
            kind: if negative { 2 } else { 1 },
            sovereign_value: 500,
        };
        let mut c = PdVerificationCompletion {
            session_id: pending.session_id.clone(),
            parent_i0: pending.parent_i0.clone(),
            primitive_anchor: u64::from_str_radix(
                pending.parent_i0.strip_prefix("i0-").unwrap(),
                16,
            )
            .unwrap(),
            xy_digest: 0xb2,
            pd_binding_digest: pending.binding_digest(),
            pd_continuum_activation_digest: pending
                .spine_completion
                .request
                .continuum
                .activation_digest,
            y_verdict: pair,
            x_verdict: pair,
            verification_seal: shadow_contracts::fnv1a64(b"SHADOW_VERIFICATION_OUTPUT")
                ^ shadow_contracts::fnv1a64(b"GJATA_LEGACY"),
            verification_generation: 4,
            verification_receipt_id: String::new(),
            living_trust_sha256: trust.identity_sha256,
            living_trust_intensity: trust.intensity,
            living_trust_kind: trust.kind,
            living_trust_sovereign_value: trust.sovereign_value,
            living_trust_digest: trust.identity_digest(),
        };
        c.verification_receipt_id = crate::lab_contracts::verification_receipt::receipt_id(
            &c.session_id,
            &c.parent_i0,
            c.primitive_anchor,
            c.xy_digest,
            c.pd_binding_digest,
            c.pd_continuum_activation_digest,
            &c.living_trust_sha256,
            c.y_verdict,
            c.x_verdict,
            c.verification_generation,
            c.verification_seal,
        );
        c
    }
    pub(super) fn synthetic_response(
        c: &PdVerificationCompletion,
    ) -> shadow_contracts::ShadowCycleResponse {
        use shadow_contracts::{ShadowCycleResponse, ShadowVerdictWire, VerificationReceiptWire};
        let hex = crate::living_trust_contract::hex_32(&c.living_trust_sha256);
        ShadowCycleResponse {
            session_id: c.session_id.clone(),
            negative_persisted: c.y_verdict == 0,
            verification_receipt: VerificationReceiptWire {
                session_id: c.session_id.clone(),
                parent_i0: c.parent_i0.clone(),
                primitive_anchor: c.primitive_anchor,
                xy_digest: c.xy_digest,
                pd_binding_digest: c.pd_binding_digest,
                pd_continuum_activation_digest: c.pd_continuum_activation_digest,
                living_trust_digest: c.living_trust_digest,
                living_trust_sha256: hex.clone(),
                y_verdict: c.y_verdict,
                x_verdict: c.x_verdict,
                seal: c.verification_seal,
                generation: c.verification_generation,
                receipt_id: c.verification_receipt_id.clone(),
            },
            verdict: ShadowVerdictWire {
                verified: c.y_verdict,
                primitive: c.x_verdict,
                knowledge_band: if c.y_verdict == 1 { 2 } else { 0 },
                lgc_law: 500,
                legacy_score: 0.7321,
                lgc_dsl: "SYNTHETIC FORMULA TEST ONLY".into(),
                living_trust_action_state: 1,
                living_trust_action_mask: 1,
                living_trust_required_action_mask: 1,
                living_trust_system_laws_seal: 1,
                living_trust_sovereign_flags: 1,
                living_trust_sovereign_value: 500,
                living_trust_sha256: hex,
                living_trust_intensity: c.living_trust_intensity,
                living_trust_kind: c.living_trust_kind,
                project_id: 0,
                project_status: 0,
                project_context_digest: 0,
                project_evidence_digest: 0,
                project_context_sha256: "0".repeat(64),
                project_evidence_sha256: "0".repeat(64),
                novel_factualized: false,
                novel_trl_level: 0,
                novel_proof_score: 0,
                novel_rejection: 0,
            },
        }
    }
    #[test]
    fn witnessed_adapter_preserves_legacy_finalization_for_both_verdicts() {
        let (env, pending) = fixture();
        assert!(env.failure_witness.is_none());
        let pd = ProgressiveDebatic::new();
        for negative in [false, true] {
            let completion = synthetic_completion(&pending, negative);
            let response = synthetic_response(&completion);
            let legacy = match pd.finalize_after_verification(pending.clone(), completion.clone()) {
                PdFinalization::Released(value) | PdFinalization::RebuiltFromNegative(value) => value,
                other => panic!("legacy formula fixture rejected: {other:?}"),
            };
            let witnessed = finalize_witnessed(&pd, pending.clone(), completion, &env, &response)
                .expect("legacy data uses unchanged finalization");
            let expected = shadow_contracts::pd_prepared::derive_continuation(&env, &response).unwrap();
            verify_continuation(&expected, &legacy).unwrap();
            verify_continuation(&expected, &witnessed).unwrap();
            assert_eq!(legacy.output.question, witnessed.output.question);
            assert_eq!(legacy.iz.iz_sha256, witnessed.iz.iz_sha256);
        }
    }

    #[test]
    fn witnessed_adapter_rejects_detached_pending_and_completion() {
        let (env, pending) = fixture();
        let pd = ProgressiveDebatic::new();
        let completion = synthetic_completion(&pending, true);
        let response = synthetic_response(&completion);
        let mut changed = pending.clone();
        changed.prepared_at += 1;
        let error = finalize_witnessed(&pd, changed, completion.clone(), &env, &response).unwrap_err();
        assert!(error.contains("WITNESSED_PENDING_MISMATCH"));
        let mut changed = completion.clone();
        changed.xy_digest ^= 1;
        let error = finalize_witnessed(&pd, pending.clone(), changed, &env, &response).unwrap_err();
        assert!(error.contains("WITNESSED_COMPLETION_MISMATCH"));
        let mut changed_response = response;
        changed_response.verification_receipt.receipt_id.push('!');
        assert!(finalize_witnessed(&pd, pending, completion, &env, &changed_response).is_err());
    }

    #[test]
    fn negative_question_override_never_changes_verdict_or_skips_receipt_gate() {
        // Formula test with existing synthetic completion, not native admission.
        let (_, pending) = fixture();
        let pd = ProgressiveDebatic::new();
        let supplied = "Which scoped material satisfies the observed missing condition?".to_string();
        let positive = synthetic_completion(&pending, false);
        match pd.finalize_after_verification_with_question(pending.clone(), positive, Some(supplied.clone())) {
            PdFinalization::Released(value) => assert_eq!(value.output.question, pending.prepared_question),
            other => panic!("positive formula fixture rejected: {other:?}"),
        }
        let negative = synthetic_completion(&pending, true);
        match pd.finalize_after_verification_with_question(pending.clone(), negative.clone(), Some(supplied.clone())) {
            PdFinalization::RebuiltFromNegative(value) => assert_eq!(value.output.question, supplied),
            other => panic!("negative formula fixture rejected: {other:?}"),
        }
        let mut unsealed = negative;
        unsealed.verification_receipt_id.push('!');
        assert!(matches!(
            pd.finalize_after_verification_with_question(pending, unsealed, Some(supplied)),
            PdFinalization::Rejected { .. }
        ));
    }

    #[test]
    fn r33_current_positive_output_keeps_receipt_gate_and_exact_text() {
        // Formula fixture only. Real native T1/T2 is verified separately.
        let (_, pending) = fixture();
        let pd=ProgressiveDebatic::new();
        let supplied="Current model output + exact missing condition?".to_string();
        let completion=synthetic_completion(&pending,false);
        match pd.finalize_after_verification_with_current_question(
            pending.clone(),completion.clone(),Some(supplied.clone())) {
            PdFinalization::Released(value) => assert_eq!(value.output.question,supplied),
            other => panic!("valid formula receipt refused: {other:?}"),
        }
        let mut invalid=completion;
        invalid.verification_receipt_id.push('!');
        assert!(matches!(pd.finalize_after_verification_with_current_question(
            pending,invalid,Some(supplied)),PdFinalization::Rejected{..}));
    }

    // Canonical synthetic witness wire for the existing formula fixture. It has
    // no actual Shadow request/receipt and is not native admission evidence.
    fn synthetic_failure_witness(env: &PdPreparedEnvelope) -> shadow_contracts::pd_failure_witness::PdFailureWitness {
        use sha2::{Digest, Sha256};
        let metadata = shadow_contracts::pd_checkpoint_data::checkpoint_metadata(
            &env.checkpoint, env.cognitive_session_id(),
        ).unwrap();
        let reason = "pa material primitiv: Xi=∅ ∧ Yi=∅";
        let mut body = b"ESSPFW01".to_vec();
        body.extend_from_slice(&1_u16.to_le_bytes());
        body.extend_from_slice(&env.source_request_sha256);
        body.extend_from_slice(&Sha256::digest(metadata.last_user_text.as_bytes()));
        body.extend_from_slice(&[3; 32]); // synthetic conditions digest
        body.extend_from_slice(&env.pending.spine.trace_digest.to_le_bytes());
        body.extend_from_slice(&0_u32.to_le_bytes()); // no negative paths
        body.push(1);
        body.extend_from_slice(&(reason.len() as u32).to_le_bytes());
        body.extend_from_slice(reason.as_bytes());
        body.push(1); // explicit model assessment, not an observation
        body.extend_from_slice(&[5; 32]); // synthetic proof digest
        body.push(0); // KiS supports under the declared model
        body.extend_from_slice(&10_000_u32.to_le_bytes());
        body.extend_from_slice(&0_u32.to_le_bytes());
        for _ in 0..2 { body.extend_from_slice(&0_u32.to_le_bytes()); }
        for _ in 0..2 { body.extend_from_slice(&0_u64.to_le_bytes()); }
        body.push(0); // no historical veto
        let digest = Sha256::digest(&body);
        body.extend_from_slice(&digest);
        shadow_contracts::pd_failure_witness::PdFailureWitness::decode(&body).unwrap()
    }

    #[test]
    fn witnessed_wrapper_keeps_legacy_preseal_and_checkpoint_exact() {
        use shadow_contracts::pd_prepared::{decode_prepared, derive_continuation, encode_prepared};
        let (mut env, pending) = fixture();
        let base = encode_prepared(&env).unwrap();
        let original_checkpoint = env.checkpoint.clone();
        let original_binding = pending.binding_digest();
        let original_preseal = pending.pre_seal.receipt_id.clone();
        env.failure_witness = Some(synthetic_failure_witness(&env));
        let wrapped = encode_prepared(&env).unwrap();
        assert_eq!(&wrapped[..8], b"ESSPDW01");
        let base_len = u32::from_le_bytes(wrapped[10..14].try_into().unwrap()) as usize;
        assert_eq!(&wrapped[14..14 + base_len], base.as_slice());
        let decoded = decode_prepared(&wrapped).unwrap();
        let recovered_pending = to_pending(&decoded).unwrap();
        assert_eq!(decoded.checkpoint, original_checkpoint);
        assert_eq!(recovered_pending.prepared_question, pending.prepared_question);
        assert_eq!(recovered_pending.binding_digest(), original_binding);
        assert_eq!(recovered_pending.pre_seal.receipt_id, original_preseal);
        assert_eq!(recovered_pending.spine_completion, pending.spine_completion);
        let completion = synthetic_completion(&pending, true);
        let response = synthetic_response(&completion);
        let closure = finalize_witnessed(&ProgressiveDebatic::new(), pending, completion, &decoded, &response).unwrap();
        let expected = derive_continuation(&decoded, &response).unwrap();
        verify_continuation(&expected, &closure).unwrap();
        assert!(closure.output.question.contains("KiS mbeshtet"));
        assert!(closure.output.question.contains("nuk eshte vete kunderprove"));
        assert!(closure.output.question.contains("nuk parashikon i0"));
        assert!(!closure.output.question.contains("KiC kundershton"));
        assert_eq!((expected.y_verdict, expected.x_verdict), (0, 0));
        let mut unwrapped = decoded;
        unwrapped.failure_witness = None;
        assert_eq!(encode_prepared(&unwrapped).unwrap(), base);
    }

    #[test]
    fn witnessed_finalizer_rejects_changed_cause_model_and_wrapper() {
        use shadow_contracts::pd_prepared::{decode_prepared, encode_prepared};
        let (mut env, pending) = fixture();
        env.failure_witness = Some(synthetic_failure_witness(&env));
        let completion = synthetic_completion(&pending, true);
        let response = synthetic_response(&completion);
        let mut changed = env.clone();
        changed.failure_witness.as_mut().unwrap().refusal = Some("claimed different failure".into());
        assert!(finalize_witnessed(&ProgressiveDebatic::new(), pending.clone(), completion.clone(), &changed, &response).is_err());
        let mut changed = env.clone();
        changed.failure_witness.as_mut().unwrap().model.as_mut().unwrap().relation = 1;
        assert!(finalize_witnessed(&ProgressiveDebatic::new(), pending, completion, &changed, &response).is_err());
        let mut wrapped = encode_prepared(&env).unwrap();
        let last = wrapped.len() - 1;
        wrapped[last] ^= 1;
        assert!(decode_prepared(&wrapped).is_err());
    }

    #[test]
    fn shared_positive_and_negative_derivation_matches_every_native_closure_field() {
        use shadow_contracts::pd_prepared::{
            decode_continuation, derive_continuation, encode_continuation,
        };
        let (env, pending) = fixture();
        let engine = ProgressiveDebatic::new();
        let mut result_digests = Vec::new();
        for negative in [false, true] {
            let completion = synthetic_completion(&pending, negative);
            let response = synthetic_response(&completion);
            let data = derive_continuation(&env, &response).unwrap();
            let native = engine.finalize_after_verification(pending.clone(), completion);
            let closure = match native {
                PdFinalization::Released(c) if !negative => c,
                PdFinalization::RebuiltFromNegative(c) if negative => c,
                other => panic!("source finalizer disagrees: {other:?}"),
            };
            verify_continuation(&data, &closure).unwrap();
            data.validate_for(&env, &response).unwrap();
            let bytes = encode_continuation(&data).unwrap();
            assert_eq!(decode_continuation(&bytes).unwrap(), data);
            result_digests.push((
                data.pd_output_digest,
                data.iz_digest,
                data.next_i0_id.clone(),
            ));
            if negative {
                assert_eq!(
                    data.question,
                    format!(
                        "Cila provë konkrete duhet të zëvendësojë bazën e rrëzuar: {}",
                        pending.prepared_question
                    )
                );
            } else {
                assert_eq!(data.question, pending.prepared_question);
            }
            let mut changed = closure.clone();
            changed.output.question.push('?');
            assert!(verify_continuation(&data, &changed).is_err());
            let mut changed = closure.clone();
            changed.iz.iz_sha256[0] ^= 1;
            assert!(verify_continuation(&data, &changed).is_err());
            let mut changed = closure.clone();
            changed.next_i0.living_trust_intensity -= 1;
            assert!(verify_continuation(&data, &changed).is_err());
            let mut changed = closure;
            changed.contract.output_state = PdContinuumState::I0Stimulated;
            assert!(verify_continuation(&data, &changed).is_err());
            for end in [0, 8, 42, bytes.len() / 2, bytes.len() - 1] {
                assert!(decode_continuation(&bytes[..end]).is_err());
            }
            let mut changed = bytes.clone();
            changed[42] ^= 1;
            assert!(decode_continuation(&changed).is_err());
            let mut changed = bytes;
            changed.push(0);
            assert!(decode_continuation(&changed).is_err());
        }
        assert_ne!(result_digests[0], result_digests[1]);
        assert!(decode_continuation(&vec![
            0;
            shadow_contracts::pd_prepared::MAX_CONTINUATION_BYTES
                + 1
        ])
        .is_err());
    }
    #[test]
    fn shared_derivation_rejects_wrong_receipts_mixed_pairs_and_persistence_claims() {
        use shadow_contracts::pd_prepared::derive_continuation;
        let (env, pending) = fixture();
        let completion = synthetic_completion(&pending, false);
        let response = synthetic_response(&completion);
        for mutate in [
            (|r: &mut shadow_contracts::ShadowCycleResponse| r.session_id.push('!'))
                as fn(&mut shadow_contracts::ShadowCycleResponse),
            |r| r.verification_receipt.parent_i0.push('!'),
            |r| r.verification_receipt.generation = 0,
            |r| r.verification_receipt.seal ^= 1,
            |r| r.verification_receipt.pd_binding_digest ^= 1,
            |r| r.verification_receipt.pd_continuum_activation_digest ^= 1,
            |r| r.verification_receipt.living_trust_digest ^= 1,
            |r| r.verdict.living_trust_intensity = 10_001,
            |r| r.verdict.living_trust_kind = 2,
            |r| r.verdict.living_trust_sovereign_value = 499,
            |r| r.verification_receipt.receipt_id.push('0'),
            |r| r.verdict.living_trust_sha256 = "00".repeat(32),
            |r| {
                r.verification_receipt.x_verdict = 0;
                r.verdict.primitive = 0;
            },
        ] {
            let mut changed = response.clone();
            mutate(&mut changed);
            assert!(derive_continuation(&env, &changed).is_err());
        }
        // Synthetic receipt fixture, not a new authority: source Shadow may
        // persist a failed experimental parent while the final verdict is positive.
        let mut positive_with_experimental_negative = response.clone();
        positive_with_experimental_negative.negative_persisted = true;
        let preserved = derive_continuation(&env, &positive_with_experimental_negative).unwrap();
        let original = derive_continuation(&env, &response).unwrap();
        assert_eq!(preserved.basis, original.basis);
        assert_eq!(preserved.next_i0_id, original.next_i0_id);
        assert_eq!(preserved.question, original.question);
        assert_ne!(preserved.shadow_response_sha256, original.shadow_response_sha256);
        let mut mixed = completion;
        mixed.x_verdict = 0;
        assert!(matches!(
            ProgressiveDebatic::new().finalize_after_verification(pending.clone(), mixed),
            PdFinalization::Rejected { .. }
        ));
        let mut negative = synthetic_response(&synthetic_completion(&pending, true));
        negative.negative_persisted = false;
        assert!(derive_continuation(&env, &negative).is_err());
    }
    #[test]
    fn continuation_is_bound_to_full_stored_prepared_and_response_not_only_digest_tuple() {
        use shadow_contracts::pd_prepared::derive_continuation;
        let (env, pending) = fixture();
        let response = synthetic_response(&synthetic_completion(&pending, false));
        let data = derive_continuation(&env, &response).unwrap();
        let mut altered = data.clone();
        altered.iz_sha256[0] ^= 1;
        // iZ SHA requires the stored pre-seal material, so validate_for is required.
        assert!(altered.validate().is_ok());
        assert!(altered.validate_for(&env, &response).is_err());
        let mut changed_env = env.clone();
        changed_env.source_request_sha256[0] ^= 1;
        assert!(data.validate_for(&changed_env, &response).is_err());
        let mut changed_env = env.clone();
        changed_env.checkpoint.push(0);
        assert!(data.validate_for(&changed_env, &response).is_err());
        let mut changed_response = response;
        changed_response.verdict.lgc_dsl.push('!');
        assert!(data.validate_for(&env, &changed_response).is_err());
    }
}

#[cfg(test)]
mod checkpoint_data_tests {
    use super::tests::fixture;
    use super::*;
    use shadow_contracts::pd_checkpoint_data::validate_checkpoint;
    fn reseal(mut body: Vec<u8>) -> Vec<u8> {
        let h = Sha256::digest(&body);
        body.extend_from_slice(&h);
        body
    }
    #[test]
    fn portable_validator_matches_real_import_on_valid_states_and_every_resealed_single_byte_mutation(
    ) {
        let (env, _) = fixture();
        validate_checkpoint(&env.checkpoint, "s1").unwrap();
        let mut engine = ProgressiveDebatic::new();
        engine.import_checkpoint("s1", &env.checkpoint).unwrap();
        for (i,content) in [
   "rotation",
   "rotation grows with measured energy and rotation returns?",
   "rotation connects orbital periods with measurable timing variation and greater structure!!",
   "rotation returns with distinct periods and energy plus an independently measured invariant!"
  ].iter().enumerate() {
   engine.ingest_sealed("s1",content,20+i as u64,PD_SEAL).unwrap();
   let bytes=engine.export_checkpoint("s1").unwrap();
   validate_checkpoint(&bytes,"s1").unwrap();
   let mut fresh=ProgressiveDebatic::new();
   fresh.import_checkpoint("s1",&bytes).unwrap();
   assert_eq!(fresh.export_checkpoint("s1").unwrap(),bytes);
  }
        // Every payload byte is altered and the checksum deliberately recomputed.
        // Agreement must come from schema/state invariants, not a checksum shortcut.
        let body = &env.checkpoint[..env.checkpoint.len() - 32];
        let mut rejected = 0;
        for index in 0..body.len() {
            let mut mutation = body.to_vec();
            mutation[index] ^= 1;
            let mutation = reseal(mutation);
            let portable = validate_checkpoint(&mutation, "s1").is_ok();
            let mut native = ProgressiveDebatic::new();
            let original = native.import_checkpoint("s1", &mutation).is_ok();
            assert_eq!(
                portable, original,
                "data/native acceptance drift at byte {index}"
            );
            rejected += usize::from(!portable);
        }
        assert!(rejected > body.len() / 2);
        assert!(validate_checkpoint(&env.checkpoint, "another-session").is_err());
    }
    #[test]
    fn prepared_admission_rejects_changed_hint_even_when_generic_snapshot_remains_importable() {
        let (mut env, _) = fixture();
        let hint = env.pending.prepared_question.as_bytes();
        let mut body = env.checkpoint[..env.checkpoint.len() - 32].to_vec();
        let start = body
            .windows(hint.len())
            .position(|w| w == hint)
            .expect("native stored response hint");
        assert!(body[start].is_ascii_alphabetic());
        body[start] = if body[start] == b'Z' { b'Y' } else { b'Z' };
        env.checkpoint = reseal(body);
        // The generic schema validates historical response data, not redeciding it.
        validate_checkpoint(&env.checkpoint, "s1").unwrap();
        ProgressiveDebatic::new()
            .import_checkpoint("s1", &env.checkpoint)
            .unwrap();
        assert!(env.validate().is_err());
        assert!(to_pending(&env).is_err());
    }
    #[test]
    fn full_shared_parser_refuses_truncation_trailing_schema_policy_and_unbacked_lengths() {
        let (env, _) = fixture();
        for end in [
            0,
            8,
            16,
            48,
            env.checkpoint.len() / 2,
            env.checkpoint.len() - 1,
        ] {
            assert!(validate_checkpoint(&env.checkpoint[..end], "s1").is_err());
        }
        for offset in [8, 12, 16] {
            let mut body = env.checkpoint[..env.checkpoint.len() - 32].to_vec();
            body[offset] = 255;
            assert!(validate_checkpoint(&reseal(body), "s1").is_err());
        }
        let mut body = env.checkpoint[..env.checkpoint.len() - 32].to_vec();
        body.extend_from_slice(&[0]);
        assert!(validate_checkpoint(&reseal(body), "s1").is_err());
        // Header: magic8 + schema4 + policy4 + active1 + detector5*u32.
        let mut body = env.checkpoint[..env.checkpoint.len() - 32].to_vec();
        body[37..41].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(validate_checkpoint(&reseal(body), "s1").is_err());
    }
}

/// Finalize data already admitted by Shadow. This helper neither authenticates
/// a caller-supplied response nor issues authority: the production caller must
/// obtain the response/T1 through the existing Shadow process and durable lookup.
/// A witnessed question may refine only a sealed 0:0 continuation. The pending
/// question, checkpoint and pre-seal remain byte-for-byte historical material.
pub fn finalize_witnessed(
    pd: &ProgressiveDebatic,
    pending: PdPendingNextI0,
    completion: PdVerificationCompletion,
    envelope: &PdPreparedEnvelope,
    response: &shadow_contracts::ShadowCycleResponse,
) -> Result<PdContinuumClosure, String> {
    let expected = shadow_contracts::pd_prepared::derive_continuation(envelope, response)?;
    let reconstructed = to_pending(envelope)?;
    let a = &pending.pre_seal;
    let b = &reconstructed.pre_seal;
    need(
        pending.session_id == reconstructed.session_id
            && pending.parent_i0 == reconstructed.parent_i0
            && pending.turn_id == reconstructed.turn_id
            && pending.prepared_question == reconstructed.prepared_question
            && pending.response_type == reconstructed.response_type
            && pending.prepared_at == reconstructed.prepared_at
            && pending.spine_completion == reconstructed.spine_completion
            && a.seal == b.seal
            && a.session_id == b.session_id
            && a.parent_i0 == b.parent_i0
            && a.receipt_id == b.receipt_id
            && a.prepared_at == b.prepared_at
            && a.spine_activation_id == b.spine_activation_id
            && a.spine_completion_digest == b.spine_completion_digest
            && a.layer_mask == b.layer_mask
            && a.mpro_mass == b.mpro_mass
            && a.action_sha256 == b.action_sha256
            && a.law_trace_sha256 == b.law_trace_sha256,
        "WITNESSED_PENDING_MISMATCH",
    )?;
    need(
        completion.session_id == expected.session_id
            && completion.parent_i0 == expected.parent_i0
            && completion.primitive_anchor == expected.primitive_anchor
            && completion.xy_digest == expected.xy_digest
            && completion.pd_binding_digest == expected.pd_binding_digest
            && completion.pd_continuum_activation_digest == expected.continuum_activation_digest
            && completion.y_verdict == expected.y_verdict
            && completion.x_verdict == expected.x_verdict
            && completion.verification_seal == expected.verification_seal
            && completion.verification_generation == expected.verification_generation
            && completion.verification_receipt_id == expected.verification_receipt_id
            && completion.living_trust_sha256 == expected.living_trust_sha256
            && completion.living_trust_intensity == expected.living_trust_intensity
            && completion.living_trust_kind == expected.living_trust_kind
            && completion.living_trust_sovereign_value == expected.living_trust_sovereign_value
            && completion.living_trust_digest == expected.living_trust_digest,
        "WITNESSED_COMPLETION_MISMATCH",
    )?;
    let negative_question = if envelope.failure_witness.is_some() {
        let question = shadow_contracts::pd_prepared::continuation_question(envelope, response)?;
        need(question == expected.question, "WITNESSED_QUESTION_DERIVATION")?;
        Some(question)
    } else {
        None
    };
    let closure = match pd.finalize_after_verification_with_current_question(
        pending, completion, negative_question,
    ) {
        PdFinalization::Released(value) | PdFinalization::RebuiltFromNegative(value) => value,
        PdFinalization::Rejected { reason } => {
            return Err(format!("PD_WITNESSED_SOURCE_FINALIZER: {reason}"));
        }
    };
    verify_continuation(&expected, &closure)?;
    Ok(closure)
}

/// Resume finalization from an exact historical Shadow bundle. The caller must
/// fetch this bundle from Shadow's durable lookup, not construct it as authority.
/// No ingest, new admission, budget reset or replacement receipt occurs.
pub fn finalize_recovered(
    bundle: &shadow_contracts::pd_recovery::RecoveryBundle,
) -> Result<PdContinuumClosure, String> {
    let (envelope, response, durable) = bundle.validated()?;
    let durable = durable.ok_or("PD_RECOVERY_T2_NOT_DURABLE")?;
    let mut pd = ProgressiveDebatic::new();
    pd.import_checkpoint(envelope.cognitive_session_id(), &envelope.checkpoint)?;
    let pending = to_pending(&envelope)?;
    let receipt = &response.verification_receipt;
    let completion = PdVerificationCompletion {
        session_id: receipt.session_id.clone(),
        parent_i0: receipt.parent_i0.clone(),
        primitive_anchor: receipt.primitive_anchor,
        xy_digest: receipt.xy_digest,
        pd_binding_digest: receipt.pd_binding_digest,
        pd_continuum_activation_digest: receipt.pd_continuum_activation_digest,
        y_verdict: receipt.y_verdict,
        x_verdict: receipt.x_verdict,
        verification_seal: receipt.seal,
        verification_generation: receipt.generation,
        verification_receipt_id: receipt.receipt_id.clone(),
        living_trust_sha256: durable.living_trust_sha256,
        living_trust_intensity: durable.living_trust_intensity,
        living_trust_kind: durable.living_trust_kind,
        living_trust_sovereign_value: durable.living_trust_sovereign_value,
        living_trust_digest: durable.living_trust_digest,
    };
    let closure = finalize_witnessed(&pd, pending, completion, &envelope, &response)?;
    verify_continuation(&durable, &closure)?;
    Ok(closure)
}

/// Restore a specifically named accepted predecessor for a new, explicit PD
/// continuation. expected_next_i0 is its logical output+iZ continuation id,
/// NOT a newly computed physical Law-0 anchor. Never relabel a source session
/// with a new attempt id; integration of a distinct topic/attempt schema is a
/// separate caller contract. In v2 expected_session_id is the immutable cognitive
/// history ID; the historical closure keeps its original physical attempt ID.
/// The caller must still use native sealed ingest.
pub fn restore_for_followup(
    bundle: &shadow_contracts::pd_recovery::RecoveryBundle,
    expected_session_id: &str,
    expected_territory: &str,
    expected_next_i0: &str,
    input_timestamp: u64,
) -> Result<ProgressiveDebatic, String> {
    let closure = finalize_recovered(bundle)?;
    let (envelope, _, _) = bundle.validated()?;
    need(
        envelope.cognitive_session_id() == expected_session_id,
        "RECOVERY_SESSION",
    )?;
    need(
        closure.next_i0.next_i0_id == expected_next_i0,
        "RECOVERY_PREDECESSOR_LINK",
    )?;
    let mut pd = ProgressiveDebatic::new();
    pd.import_checkpoint(expected_session_id, &envelope.checkpoint)?;
    let session = pd
        .session(expected_session_id)
        .ok_or("PD_RECOVERY_SESSION_MISSING")?;
    need(
        !expected_territory.is_empty() && session.territory == expected_territory,
        "RECOVERY_TERRITORY",
    )?;
    need(
        input_timestamp >= session.last_activity_at && input_timestamp >= session.started_at,
        "RECOVERY_TIME_REWIND",
    )?;
    need(pd.active && session.active, "RECOVERY_SESSION_CLOSED")?;
    Ok(pd)
}

#[cfg(test)]
mod recovery_tests {
    use super::continuation_tests::{synthetic_completion, synthetic_response};
    use super::tests::fixture;
    use super::*;
    use shadow_contracts::pd_prepared::{
        derive_continuation, encode_continuation, encode_prepared,
    };
    use shadow_contracts::pd_recovery::{self as recovery, RecoveryBundle, RecoveryLookup};
    pub(super) fn bundle(negative: bool) -> RecoveryBundle {
        let (env, pending) = fixture();
        let response = synthetic_response(&synthetic_completion(&pending, negative));
        let continuation = derive_continuation(&env, &response).unwrap();
        RecoveryBundle {
            version: recovery::RECOVERY_VERSION,
            identity: recovery::native_identity(
                &pending.session_id,
                &pending.parent_i0,
                response.verification_receipt.primitive_anchor,
            )
            .unwrap(),
            request_sha256: env.source_request_sha256,
            transaction_id: [9; 32],
            prepared: encode_prepared(&env).unwrap(),
            response: shadow_contracts::encode_cycle_response(&response),
            continuation: Some(encode_continuation(&continuation).unwrap()),
        }
    }
    #[test]
    fn exact_recovery_roundtrip_matches_source_finalization_for_both_constitutional_pairs() {
        for negative in [false, true] {
            let bundle = bundle(negative);
            let lookup = RecoveryLookup {
                identity: bundle.identity,
                request_sha256: bundle.request_sha256,
            };
            let bytes = recovery::encode_bundle(&bundle).unwrap();
            let decoded = recovery::decode_bundle(&bytes).unwrap();
            assert_eq!(decoded, bundle);
            decoded.validate_for(&lookup).unwrap();
            let closure = finalize_recovered(&decoded).unwrap();
            let (_, _, continuation) = decoded.validated().unwrap();
            verify_continuation(&continuation.unwrap(), &closure).unwrap();
            let mut wrong = lookup;
            wrong.identity[0] ^= 1;
            assert!(decoded.validate_for(&wrong).is_err());
            let mut missing = bundle.clone();
            missing.continuation = None;
            assert!(missing.validated().is_ok());
            assert!(finalize_recovered(&missing)
                .unwrap_err()
                .contains("T2_NOT_DURABLE"));
            for end in [0, 8, 100, bytes.len() / 2, bytes.len() - 1] {
                assert!(recovery::decode_bundle(&bytes[..end]).is_err());
            }
            let mut bad = bytes.clone();
            bad[100] ^= 1;
            assert!(recovery::decode_bundle(&bad).is_err());
            let mut bad = bytes;
            bad.push(0);
            assert!(recovery::decode_bundle(&bad).is_err());
        }
    }
    #[test]
    fn restored_history_changes_next_real_pd_ingest_without_relabeling_or_budget_reset() {
        let bundle = bundle(false);
        let before = bundle.prepared.clone();
        let closure = finalize_recovered(&bundle).unwrap();
        let next = &closure.next_i0.next_i0_id;
        let mut restored = restore_for_followup(&bundle, "s1", "science", next, 20).unwrap();
        let historical = restored.session("s1").unwrap().clone();
        assert_eq!(historical.total_turns, 1);
        let content = "rotation ".repeat(24);
        restored
            .ingest_for_spine_sealed("s1", next, &content, 20, PD_SEAL)
            .unwrap();
        let mut fresh = ProgressiveDebatic::new();
        fresh
            .open_session_sealed("s1", "science", DebateMode::Scientific, 20, PD_SEAL)
            .unwrap();
        fresh
            .ingest_for_spine_sealed("s1", next, &content, 20, PD_SEAL)
            .unwrap();
        let a = restored.session("s1").unwrap();
        let b = fresh.session("s1").unwrap();
        assert_eq!(a.total_turns, 2);
        assert_eq!(b.total_turns, 1);
        assert_eq!(a.started_at, historical.started_at);
        assert_eq!(
            a.turns[0].user_signal.raw_content,
            historical.turns[0].user_signal.raw_content
        );
        assert!(a
            .turns
            .last()
            .unwrap()
            .user_signal
            .returns_to
            .iter()
            .any(|c| c == "rotation"));
        assert!(b.turns.last().unwrap().user_signal.returns_to.is_empty());
        assert_ne!(
            a.turns.last().unwrap().pd_response_hint,
            b.turns.last().unwrap().pd_response_hint
        );
        assert!(
            a.pressure_budget.challenges_issued >= historical.pressure_budget.challenges_issued
        );
        assert_eq!(bundle.prepared, before); // stored predecessor was never rewritten
        assert!(restore_for_followup(&bundle, "new-attempt", "science", next, 20).is_err());
        assert!(restore_for_followup(&bundle, "s1", "other-topic", next, 20).is_err());
        assert!(
            restore_for_followup(&bundle, "s1", "science", &closure.next_i0.parent_i0, 20).is_err()
        );
        assert!(restore_for_followup(&bundle, "s1", "science", next, 9).is_err());
    }
    #[test]
    fn changed_predecessor_receipt_source_or_continuation_cannot_be_recovered() {
        let value = bundle(false);
        for mutate in [
            (|b: &mut RecoveryBundle| b.identity[0] ^= 1) as fn(&mut RecoveryBundle),
            |b| b.request_sha256[0] ^= 1,
            |b| b.transaction_id = [0; 32],
            |b| b.prepared[50] ^= 1,
            |b| b.response[20] ^= 1,
            |b| b.continuation.as_mut().unwrap()[50] ^= 1,
        ] {
            let mut bad = value.clone();
            mutate(&mut bad);
            assert!(recovery::encode_bundle(&bad).is_err());
            assert!(finalize_recovered(&bad).is_err());
        }
    }
}

#[cfg(test)]
mod continuity_v2_tests {
    use super::continuation_tests::{assert_source_formula_matrix, synthetic_completion, synthetic_response};
    use super::tests::prepared_from_cycle;
    use super::*;
    use shadow_contracts::pd_prepared::{
        decode_prepared, derive_continuation, encode_continuation, encode_prepared,
    };
    use shadow_contracts::pd_recovery::{native_identity, RecoveryBundle, RECOVERY_VERSION};

    fn next(
        attempt: &str,
    ) -> (
        RecoveryBundle,
        ProgressiveDebatic,
        PdPreparedEnvelope,
        PdPendingNextI0,
    ) {
        next_with_parent(attempt, "i0-00000000000000b2")
    }
    fn next_with_parent(
        attempt: &str,
        parent: &str,
    ) -> (
        RecoveryBundle,
        ProgressiveDebatic,
        PdPreparedEnvelope,
        PdPendingNextI0,
    ) {
        let prior = super::recovery_tests::bundle(false);
        next_from_prior(prior,attempt,parent)
    }
    fn next_from_prior(
        prior: RecoveryBundle, attempt: &str, parent: &str,
    ) -> (RecoveryBundle,ProgressiveDebatic,PdPreparedEnvelope,PdPendingNextI0) {
        let link = PdContinuationLink::from_bundle(&prior).unwrap();
        let mut pd =
            restore_for_followup(&prior, "s1", "science", &link.predecessor_next_i0, 20).unwrap();
        let cycle = pd
            .ingest_for_spine_with_history_sealed(
                "s1",
                attempt,
                parent,
                &"rotation ".repeat(24),
                20,
                PD_SEAL,
            )
            .unwrap();
        let (env, pending) = prepared_from_cycle(&pd, cycle, "s1", attempt, parent, 20, Some(link));
        (prior, pd, env, pending)
    }

    fn assert_restored_lineage(prior:&RecoveryBundle,pd:&ProgressiveDebatic,env:&PdPreparedEnvelope) {
        let old_bytes=prior.prepared.clone();
        let old=decode_prepared(&old_bytes).unwrap();
        let link=PdContinuationLink::from_bundle(prior).unwrap();
        let before=restore_for_followup(prior,"s1","science",&link.predecessor_next_i0,20).unwrap();
        let previous=before.session("s1").unwrap();
        let current=pd.session("s1").unwrap();
        assert_eq!(current.total_turns,previous.total_turns.checked_add(1).unwrap());
        assert_eq!(current.turns.len(),previous.turns.len()+1);
        assert_eq!(current.trace.signals.len(),previous.trace.signals.len()+1);
        assert_eq!(current.session_id,previous.session_id);
        assert_eq!(current.territory,previous.territory);
        assert_eq!(current.started_at,previous.started_at);
        assert_eq!(current.trace.trace_id,previous.trace.trace_id);
        assert!(current.last_activity_at>=previous.last_activity_at);
        assert!(current.pressure_budget.challenges_issued>=previous.pressure_budget.challenges_issued);
        for (historical,retained) in previous.turns.iter().zip(&current.turns) {
            assert_eq!(retained.turn_id,historical.turn_id);
            assert_eq!(retained.turn_index,historical.turn_index);
            assert_eq!(retained.user_signal.raw_content.as_bytes(),historical.user_signal.raw_content.as_bytes());
            assert_eq!(retained.user_signal.timestamp,historical.user_signal.timestamp);
            assert_eq!(retained.challenge_issued,historical.challenge_issued);
        }
        let expected_user="rotation ".repeat(24);
        assert_eq!(current.turns.last().unwrap().user_signal.raw_content.as_bytes(),expected_user.as_bytes());
        assert_ne!(env.pending.session_id,env.cognitive_session_id());
        assert!(pd.session(&env.pending.session_id).is_none());
        env.validate_predecessor(prior).unwrap();
        shadow_contracts::pd_checkpoint_data::validate_user_checkpoint_transition(
            &env.checkpoint,env.cognitive_session_id(),&expected_user,
            u64::from(previous.total_turns),Some(&old.checkpoint)).unwrap();
        // Exactly n+1 user turns: neither an iZ question nor a logical next-i0
        // identifier becomes an extra user utterance during restore/finalization.
        assert_eq!(before.export_checkpoint("s1").unwrap(),old.checkpoint);
        assert_eq!(encode_prepared(&old).unwrap(),old_bytes);
        let at_boundary=restore_for_followup(prior,"s1","science",&link.predecessor_next_i0,previous.last_activity_at);
        assert!(at_boundary.is_ok());
        assert!(previous.last_activity_at>0);
        assert!(restore_for_followup(prior,"s1","science",&link.predecessor_next_i0,previous.last_activity_at-1).is_err());
        assert!(restore_for_followup(prior,"other-session","science",&link.predecessor_next_i0,20).is_err());
        assert!(restore_for_followup(prior,"s1","other-scope",&link.predecessor_next_i0,20).is_err());
        assert!(restore_for_followup(prior,"s1","science","i0-wrong-logical-predecessor",20).is_err());
        let mut missing_t2=prior.clone();missing_t2.continuation=None;
        assert!(restore_for_followup(&missing_t2,"s1","science",&link.predecessor_next_i0,20).is_err());
        let mut reset=env.clone();reset.checkpoint=old.checkpoint;
        assert!(reset.validate_predecessor(prior).is_err());
        assert_eq!(prior.prepared,old_bytes);
    }

    #[test]
    fn native_attempt_is_fresh_while_history_and_v1_bytes_remain_unchanged() {
        let (prior, pd, env, pending) = next("attempt-2");
        let historic = prior.prepared.clone();
        env.validate_predecessor(&prior).unwrap();
        assert_eq!(env.version, PREPARED_CONTINUITY_VERSION);
        assert_eq!(env.cognitive_session_id(), "s1");
        assert_eq!(pending.session_id, "attempt-2");
        assert_eq!(pending.pre_seal.session_id, "attempt-2");
        assert_eq!(pending.parent_i0, "i0-00000000000000b2");
        assert_eq!(
            pending.spine_completion.request.session_digest,
            crate::pd_spine_contract::digest_parts(&[b"attempt-2"])
        );
        let old = decode_prepared(&historic).unwrap();
        assert_eq!(old.version, PREPARED_VERSION);
        assert!(old.continuity.is_none());
        assert_eq!(encode_prepared(&old).unwrap(), historic);
        let current = pd.session("s1").unwrap();
        assert_eq!(current.total_turns, 2);
        assert_eq!(current.started_at, 10);
        assert!(pd.session("attempt-2").is_none());
        assert!(current.turns[1]
            .user_signal
            .returns_to
            .iter()
            .any(|v| v == "rotation"));
        let bytes = encode_prepared(&env).unwrap();
        let decoded = decode_prepared(&bytes).unwrap();
        assert_eq!(decoded, env);
        assert_eq!(
            to_pending(&decoded).unwrap().binding_digest(),
            pending.binding_digest()
        );
        assert_eq!(prior.prepared, historic);
    }

    #[test]
    fn foreign_predecessor_stale_attempt_missing_t2_and_checkpoint_reset_refuse() {
        let (prior, _, env, _) = next("attempt-2");
        for field in 0..4 {
            let mut changed = env.clone();
            let link = changed.continuity.as_mut().unwrap();
            match field {
                0 => link.predecessor.identity[0] ^= 1,
                1 => link.predecessor_transaction_id[0] ^= 1,
                2 => link.predecessor_continuation_sha256[0] ^= 1,
                _ => link.predecessor_next_i0.push('x'),
            }
            assert!(changed.validate_predecessor(&prior).is_err());
        }
        let (_, _, reused, _) = next("s1");
        assert!(reused
            .validate_predecessor(&prior)
            .unwrap_err()
            .contains("ATTEMPT_REUSED"));
        let mut no_t2 = prior.clone();
        no_t2.continuation = None;
        assert!(PdContinuationLink::from_bundle(&no_t2).is_err());
        let mut renamed = env.clone();
        renamed.continuity.as_mut().unwrap().cognitive_session_id = "attempt-2".into();
        assert!(renamed.validate().is_err());
        let mut reset = env;
        reset.checkpoint = decode_prepared(&prior.prepared).unwrap().checkpoint;
        assert!(reset.validate_predecessor(&prior).is_err());
    }

    #[test]
    fn v2_recovery_retains_cognitive_identity_and_cannot_reuse_prior_receipt() {
        let (prior, pd, env, pending) = next("attempt-2");
        let old_completion = synthetic_completion(
            &to_pending(&decode_prepared(&prior.prepared).unwrap()).unwrap(),
            false,
        );
        assert!(matches!(
            pd.finalize_after_verification(pending.clone(), old_completion),
            PdFinalization::Rejected { .. }
        ));
        for negative in [false, true] {
            let completion = synthetic_completion(&pending, negative);
            let response = synthetic_response(&completion);
            let continuation = derive_continuation(&env, &response).unwrap();
            let bundle = RecoveryBundle {
                version: RECOVERY_VERSION,
                identity: native_identity(
                    "attempt-2",
                    &pending.parent_i0,
                    completion.primitive_anchor,
                )
                .unwrap(),
                request_sha256: env.source_request_sha256,
                transaction_id: [10; 32],
                prepared: encode_prepared(&env).unwrap(),
                response: shadow_contracts::encode_cycle_response(&response),
                continuation: Some(encode_continuation(&continuation).unwrap()),
            };
            let closure = finalize_recovered(&bundle).unwrap();
            assert_eq!(closure.next_i0.session_id, "attempt-2");
            verify_continuation(&continuation, &closure).unwrap();
            let restored =
                restore_for_followup(&bundle, "s1", "science", &closure.next_i0.next_i0_id, 30)
                    .unwrap();
            assert_eq!(restored.session("s1").unwrap().total_turns, 2);
            assert!(restored.session("attempt-2").is_none());
            assert!(restore_for_followup(
                &bundle,
                "attempt-2",
                "science",
                &closure.next_i0.next_i0_id,
                30
            )
            .is_err());
        }
    }

    #[test]
    fn distinct_attempt_entry_keeps_the_original_seal_guard_and_same_id_behavior() {
        let prior = super::recovery_tests::bundle(false);
        let link = PdContinuationLink::from_bundle(&prior).unwrap();
        let mut a =
            restore_for_followup(&prior, "s1", "science", &link.predecessor_next_i0, 20).unwrap();
        let mut b =
            restore_for_followup(&prior, "s1", "science", &link.predecessor_next_i0, 20).unwrap();
        let before = a.export_checkpoint("s1").unwrap();
        assert!(a
            .ingest_for_spine_with_history_sealed(
                "s1",
                "attempt-2",
                "i0-00000000000000b2",
                "rotation",
                20,
                PD_SEAL ^ 1
            )
            .is_err());
        assert_eq!(a.export_checkpoint("s1").unwrap(), before);
        let old = a
            .ingest_for_spine_sealed("s1", "i0-00000000000000b2", "rotation", 20, PD_SEAL)
            .unwrap();
        let new = b
            .ingest_for_spine_with_history_sealed(
                "s1",
                "s1",
                "i0-00000000000000b2",
                "rotation",
                20,
                PD_SEAL,
            )
            .unwrap();
        assert_eq!(old.request, new.request);
        assert_eq!(
            a.export_checkpoint("s1").unwrap(),
            b.export_checkpoint("s1").unwrap()
        );
    }
    #[test]
    fn export_synthetic_followup_fixtures() {
        use std::io::Write;
        let output = super::fixture_output::OutputDir::new("ESSMAI_PD_PREPARED_FIXTURE_DIR", "prepared");
        let root = output.path();
        // Source restore behavior must preserve history after either synthetic
        // constitutional pair, without creating or accepting any real receipt.
        for prior_negative in [false,true] {
            let(prior,pd,env,pending)=next_from_prior(super::recovery_tests::bundle(prior_negative),
                "math-followup-attempt","i0-00000000000000b2");
            assert_restored_lineage(&prior,&pd,&env);
            assert_source_formula_matrix(&env,&pending);
        }
        for (negative, name, attempt, parent) in [
            (
                false,
                "followup-positive",
                "attempt-2",
                "i0-00000000000000b2",
            ),
            (
                true,
                "followup-negative",
                "attempt-2",
                "i0-00000000000000b2",
            ),
            (false, "stale-positive", "attempt-3", "i0-00000000000000c3"),
        ] {
            let (prior, pd, env, pending) = next_with_parent(attempt, parent);
            assert_restored_lineage(&prior,&pd,&env);
            let dir = root.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            let response = synthetic_response(&synthetic_completion(&pending, negative));
            let continuation = derive_continuation(&env, &response).unwrap();
            for (file,bytes) in [
                ("prepared.bin",encode_prepared(&env).unwrap()),
                ("response.wire",shadow_contracts::encode_cycle_response(&response)),
                ("continuation.bin",encode_continuation(&continuation).unwrap()),
                ("FIXTURE_NOTICE.txt",b"SYNTHETIC v2 FORMULA/STORAGE TEST ONLY. Real restored PD history/new native Spine/preseal; synthetic layer/Shadow receipt. Prior is positive s1/a1 fixture with requestSHA[7;32] and synthetic transaction_id[9;32]. Storage fixture must bind actual stored prior transaction before child derivation. No real GCL admission.\n".to_vec()),
            ] {
                if file != "FIXTURE_NOTICE.txt" {
                    assert_eq!(bytes, super::pinned_fixture::bytes(name, file), "regenerated followup fixture drift");
                }
                let mut f=std::fs::OpenOptions::new().write(true).create_new(true).open(dir.join(file)).unwrap();
                f.write_all(&bytes).unwrap();f.sync_all().unwrap();
            }
        }
        println!("R20_MATH {}", r#"{"test":"export_synthetic_followup_fixtures","checks":["scalar_output_iz_chain_after_both_prior_pairs","exact_user_turn_count_n_plus_one","immutable_history_prefix_bytes","stable_cognitive_identity_distinct_attempt","pressure_budget_not_reset","source_checkpoint_roundtrip","timestamp_boundary_no_rewind","foreign_scope_and_predecessor_refused","predecessor_t2_material_required","reset_checkpoint_refused","exported_predecessor_unchanged"]}"#);
    }
}


#[cfg(test)]
mod dialogue_clock_tests {
    use super::*;
    use super::tests::prepared_from_cycle_with_dialogue;
    use shadow_contracts::pd_prepared::{encode_prepared,decode_prepared,derive_continuation,encode_continuation};
    // Actual PD ingest/checkpoint; context ID and Shadow receipt are synthetic
    // data fixtures. This test is not native KiE/GCL acceptance evidence.
    #[test]
    fn dialogue_v3_preserves_cognition_and_v4_propagates_explicit_millisecond_clock() {
        let context=shadow_contracts::kie::KieContext {
            identity:shadow_contracts::pd_memory::PdIdentity {
                owner:"fixture".into(),obligation:"model".into(),case_id:"source".into(),
                scope:"science".into(),domain:"science".into(),lineage_root:"root".into(),
            },operation_id:"source-op".into(),record_id:[5;32],
        };
        let link=PdDialogueLink::new(context).unwrap();
        let cognitive=link.cognitive_session_id.clone();
        let mut pd=ProgressiveDebatic::new();
        pd.open_session_sealed(&cognitive,"science",DebateMode::Scientific,1000,PD_SEAL).unwrap();
        pd.ingest_sealed(&cognitive,"rotation source question",1000,PD_SEAL).unwrap();
        let before=pd.export_checkpoint(&cognitive).unwrap();
        let text="rotation model proposition";
        let cycle=pd.ingest_for_spine_with_history_sealed(&cognitive,"native-one","i0-00000000000000a1",text,1001,PD_SEAL).unwrap();
        let (env,pending)=prepared_from_cycle_with_dialogue(&pd,cycle,&cognitive,"native-one","i0-00000000000000a1",1001,None,Some(link));
        assert_eq!(env.version,PREPARED_DIALOGUE_VERSION);
        assert!(env.clock_is_millis());
        assert_eq!(env.cognitive_session_id(),cognitive);
        shadow_contracts::pd_checkpoint_data::validate_user_checkpoint_transition(&env.checkpoint,&cognitive,text,1,Some(&before)).unwrap();
        let bytes=encode_prepared(&env).unwrap();
        assert_eq!(decode_prepared(&bytes).unwrap(),env);
        let mut changed=env.clone();
        changed.source_dialogue.as_mut().unwrap().cognitive_session_id="renamed".into();
        assert!(changed.validate().is_err());
        let response=super::continuation_tests::synthetic_response(&super::continuation_tests::synthetic_completion(&pending,false));
        let continuation=derive_continuation(&env,&response).unwrap();
        let bundle=shadow_contracts::pd_recovery::RecoveryBundle {
            version:shadow_contracts::pd_recovery::RECOVERY_VERSION,
            identity:shadow_contracts::pd_recovery::native_identity(&env.pending.session_id,&env.pending.parent_i0,response.verification_receipt.primitive_anchor).unwrap(),
            request_sha256:env.source_request_sha256,transaction_id:[9;32],prepared:bytes.clone(),
            response:shadow_contracts::encode_cycle_response(&response),continuation:Some(encode_continuation(&continuation).unwrap()),
        };
        let continuation_link=PdContinuationLink::from_bundle(&bundle).unwrap();
        assert!(continuation_link.clock_millis);
        let cycle=pd.ingest_for_spine_with_history_sealed(&cognitive,"native-two","i0-00000000000000b2","rotation model followup",1002,PD_SEAL).unwrap();
        let (next,_)=prepared_from_cycle_with_dialogue(&pd,cycle,&cognitive,"native-two","i0-00000000000000b2",1002,Some(continuation_link),None);
        assert_eq!(next.version,PREPARED_MILLIS_CONTINUITY_VERSION);
        assert!(next.clock_is_millis());
        next.validate_predecessor(&bundle).unwrap();
        let encoded=encode_prepared(&next).unwrap();
        assert_eq!(decode_prepared(&encoded).unwrap(),next);
        let mut wrong=next.clone();
        wrong.version=PREPARED_CONTINUITY_VERSION;
        assert!(wrong.validate().is_err());
        wrong=next;
        wrong.continuity.as_mut().unwrap().clock_millis=false;
        assert!(wrong.validate().is_err());
        assert_eq!(bundle.prepared,bytes,"historical v3 checkpoint was not rewritten");
    }
}


#[cfg(test)]
mod iz_v5_tests {
    use super::*;
    use shadow_contracts::pd_memory::{self as pd,iz};
    use shadow_contracts::pd_prepared::{encode_prepared,decode_prepared};
    use shadow_contracts::{KnowledgeSelectionRequestWire,KnowledgeSelectionResponseWire,
        CandidateStatisticWire,SelectionRequester,SelectionPurpose,PROTOCOL_VERSION,SELECTION_MAX_CANDIDATES};
    fn hash(bytes:&[u8])->[u8;32]{Sha256::digest(bytes).into()}
    fn initial()->iz::IzContext {
        let text="Which rotation measurement distinguishes alpha from beta?".to_string();
        iz::IzContext{version:iz::IZ_ACTUAL_INPUT_VERSION,identity:pd::PdIdentity {
            owner:"v5-data-fixture".into(),obligation:"exact distinction".into(),case_id:"fixture-case".into(),
            scope:"science".into(),domain:"science".into(),lineage_root:pd::hex(&hash(text.as_bytes()))},
            operation_id:"first".into(),origin_i0:text.clone(),user_text:text,parent:None,
            answer:iz::IzAnswer::default(),map:None,native_anchor:None,native_map:None}
    }
    // Explicit synthetic selection/hold/candidate DATA. Never inserted into a
    // knowledge store; not proof of source ASHT execution or GCL admission.
    fn pair(context:&iz::IzContext,present:bool)->(Vec<u8>,Vec<u8>){
        use pd::failure::relevance;
        let request=KnowledgeSelectionRequestWire{version:PROTOCOL_VERSION,
            requester:SelectionRequester::AshtQuantum,purpose:SelectionPurpose::EvidenceCandidates,
            session_id:"native-iz".into(),trace_id:"native-iz".into(),
            parent_i0:pd::hex(&hash(context.current_i0().as_bytes())),primitive_anchor:0xa1,
            split_sha256:"11".repeat(32),xi:vec!["alpha".into()],yi:vec!["beta".into()],
            upstream_hold_sha256:Some("22".repeat(32)),evidence_sha256:"33".repeat(32),
            max_positive:SELECTION_MAX_CANDIDATES,max_negative:SELECTION_MAX_CANDIDATES};
        let bytes=shadow_contracts::encode_selection_request(&request);
        let candidate=|positive:bool|CandidateStatisticWire{candidate_id:if positive{1}else{2},
            polarity:u8::from(positive),domain:if positive{"primitive"}else{"negative-process"}.into(),
            term_sha256:shadow_contracts::sha256_hex(&[
                if positive{relevance::TERM_PRIMITIVE_DOMAIN}else{relevance::TERM_NEGATIVE_DOMAIN},
                if positive{b"alpha"}else{b"beta"}]),source_sha256:"55".repeat(32),
            security_y:u8::from(positive),trust_x:u8::from(positive),confidence_fixed:7500,
            frequency:2,domain_count:1,legacy_score_fixed:5000};
        let mut response=KnowledgeSelectionResponseWire{version:PROTOCOL_VERSION,requester:request.requester,
            session_id:request.session_id.clone(),parent_i0:request.parent_i0.clone(),primitive_anchor:request.primitive_anchor,
            split_sha256:request.split_sha256.clone(),upstream_hold_sha256:request.upstream_hold_sha256.clone(),
            request_sha256:shadow_contracts::sha256_hex(&[b"ESSMAI/SELECTION/REQUEST/V178",&bytes]),
            selection_sha256:String::new(),hold_id:"synthetic-not-native".into(),hold_sha256:"44".repeat(32),
            positive:if present{vec![candidate(true)]}else{vec![]},negative:if present{vec![candidate(false)]}else{vec![]},
            law_seal:1,system_laws_seal:2,expires_at_ns:3};
        response.selection_sha256=relevance::selection_digest(&response);
        (bytes,shadow_contracts::encode_selection_response(&response))
    }
    fn parent_record()->pd::PdOperationRecord {
        let context=initial();let cognitive=pd::hex(&context.identity.key().unwrap());
        let mut engine=ProgressiveDebatic::new();
        engine.open_session_sealed(&cognitive,"science",DebateMode::Scientific,1000,PD_SEAL).unwrap();
        engine.ingest_sealed(&cognitive,&context.user_text,1000,PD_SEAL).unwrap();
        let (q,r)=pair(&context,false);
        let attempt=iz::IzBefore::new(&context,q).unwrap().activate().unwrap()
            .complete(r,pd::failure::MISSING_POSITIVE.into()).unwrap();
        let activation=iz::assembly_activation(&context,&attempt,&[]).unwrap();
        let source=iz::compile_exact_iz(context.clone(),attempt,vec![],activation).unwrap();
        let mut checkpoint=context.checkpoint_prefix().to_vec();checkpoint.extend_from_slice(&engine.export_checkpoint(&cognitive).unwrap());
        pd::PdOperationRecord::construct(pd::PdOperationRequest{identity:context.identity,operation_id:context.operation_id,
            source_input:iz::encode_source(&source).unwrap(),expected_revision:0,previous_state:[0;32],
            witness:iz::build_witness(&source).unwrap(),checkpoint,scoped_question:source.question_spec.question.clone(),
            candidate:vec![],candidate_key:[0;32],lgc_steps:iz::build_lgc_steps(&source).unwrap()},[0;32]).unwrap()
    }
    fn build(parent:Option<&pd::PdOperationRecord>)->(PdPreparedEnvelope,PdPendingNextI0){
        let mut context=initial();
        if let Some(parent)=parent {
            context.operation_id="next".into();context.user_text="The measured alpha rotation differs from beta by two degrees.".into();
            let reference=iz::IzParent::from_record(parent).unwrap();context.parent=Some(reference.clone());
            context.map=Some(iz::IzMapSnapshot{identity_key:context.identity.key().unwrap(),head:reference.clone(),
                exact_record:reference,status:iz::IzMaterialStatus::OperationalNoVerdict});
        }
        // Actual producer preparation uses only this explicit source record.
        let prepared=match crate::pd_iz::prepare_with(&context,&mut |mode,bytes| {
            assert_eq!(mode,"--pd-head-once");let query=pd::decode_lookup(bytes)?;
            let found=parent.filter(|r|query.operation_id.as_ref().map(|op|op==&r.request.operation_id).unwrap_or(true));
            pd::encode_optional_record(found)
        }).unwrap(){crate::pd_iz::Preparation::Ready(ready)=>ready,_=>panic!("fixture unexpectedly replayed")};
        let (q,r)=pair(&context,true);
        let activated=crate::pd_iz::before_action(&prepared,q).unwrap();
        let promoted=crate::pd_iz::prepare_promotion(prepared,activated,r,&"77".repeat(32)).unwrap();
        let now=if parent.is_some(){1001}else{1000};
        let mut engine=crate::pd_iz::restore_for_promotion(&promoted,"science",now).unwrap();
        let cognitive=promoted.link.cognitive_session_id.clone();
        let cycle=engine.ingest_for_spine_with_history_sealed(&cognitive,"native-iz","i0-00000000000000a1",
            &context.user_text,now,PD_SEAL).unwrap();
        super::tests::prepared_from_cycle_with_links(&engine,cycle,&cognitive,"native-iz","i0-00000000000000a1",
            now,None,None,Some(promoted.link))
    }
    #[test]
    fn v5_restores_actual_piz2_history_and_adds_exactly_one_user_turn(){
        let parent=parent_record();let before=pd::encode_record(&parent).unwrap();
        let (env,pending)=build(Some(&parent));env.validate_iz_source(&parent).unwrap();
        assert_eq!(env.version,PREPARED_IZ_ORIGIN_VERSION);assert!(env.clock_is_millis());
        let mut restored=ProgressiveDebatic::new();restored.import_checkpoint(env.cognitive_session_id(),&env.checkpoint).unwrap();
        let state=restored.session(env.cognitive_session_id()).unwrap();
        assert_eq!(state.total_turns,2);assert_eq!(state.started_at,1000);assert_eq!(state.last_activity_at,1001);
        assert_eq!(state.turns[0].user_signal.raw_content,initial().user_text);
        assert_eq!(state.turns[1].user_signal.raw_content,env.source_iz.as_ref().unwrap().context.user_text);
        assert_eq!(to_pending(&env).unwrap().binding_digest(),pending.binding_digest());
        let bytes=encode_prepared(&env).unwrap();assert_eq!(decode_prepared(&bytes).unwrap(),env);
        assert_eq!(pd::encode_record(&parent).unwrap(),before,"source history was not rewritten");
        assert!(env.validate_iz_initial().is_err());
    }
    #[test]
    fn v5_initial_has_one_real_turn_without_inventing_a_parent(){
        let (env,_)=build(None);env.validate_iz_initial().unwrap();
        let link=env.source_iz.as_ref().unwrap();assert!(link.context.parent.is_none());assert_eq!(link.prior_lgc_head,[0;32]);
        let meta=shadow_contracts::pd_checkpoint_data::checkpoint_metadata(&env.checkpoint,env.cognitive_session_id()).unwrap();
        assert_eq!(meta.total_turns,1);assert_eq!(meta.last_user_text,initial().user_text);
        assert_eq!(decode_prepared(&encode_prepared(&env).unwrap()).unwrap(),env);
        assert!(env.validate_iz_source(&parent_record()).is_err());
    }
    #[test]
    fn v5_common_t2_formula_and_real_pd_finalizer_agree_for_both_synthetic_receipts(){
        use shadow_contracts::pd_prepared::{derive_continuation,encode_continuation};
        use shadow_contracts::pd_recovery::{self as recovery,RecoveryBundle};
        let parent=parent_record();
        for source in [None,Some(&parent)] {
            let (env,pending)=build(source);
            for negative in [false,true] {
                // Receipt/transaction IDs and layer completion remain synthetic.
                // Actual PD finalization and common T2 formulas are exercised;
                // this test does not claim a native Shadow decision or durable T1.
                let response=super::continuation_tests::synthetic_response(
                    &super::continuation_tests::synthetic_completion(&pending,negative));
                let continuation=derive_continuation(&env,&response).unwrap();
                let bundle=RecoveryBundle{version:recovery::RECOVERY_VERSION,
                    identity:recovery::native_identity(&pending.session_id,&pending.parent_i0,
                        response.verification_receipt.primitive_anchor).unwrap(),
                    request_sha256:env.source_request_sha256,transaction_id:[9;32],
                    prepared:encode_prepared(&env).unwrap(),response:shadow_contracts::encode_cycle_response(&response),
                    continuation:Some(encode_continuation(&continuation).unwrap())};
                export_storage_fixture_if_requested(&bundle,source,negative);
                let encoded=recovery::encode_bundle(&bundle).unwrap();
                let decoded=recovery::decode_bundle(&encoded).unwrap();assert_eq!(decoded,bundle);
                let closure=finalize_recovered(&decoded).unwrap();verify_continuation(&continuation,&closure).unwrap();
                let output=pd::lgc::PdLgcOutput::Native(decoded.clone());
                assert_eq!(pd::lgc::decode_output(&pd::lgc::encode_output(&output).unwrap()).unwrap(),output);
                let mut incomplete=decoded.clone();incomplete.continuation=None;
                assert!(pd::lgc::encode_output(&pd::lgc::PdLgcOutput::Native(incomplete.clone())).is_err());
                assert!(finalize_recovered(&incomplete).is_err());
                let mut changed=decoded;changed.request_sha256[0]^=1;
                assert!(recovery::encode_bundle(&changed).is_err());
            }
        }
    }
    fn export_storage_fixture_if_requested(
        bundle:&shadow_contracts::pd_recovery::RecoveryBundle,parent:Option<&pd::PdOperationRecord>,negative:bool,
    ) {
        // Tests always run and assert. Export is an explicit developer request,
        // never a production port, implicit fixture prerequisite or ignored test.
        let Some(root)=std::env::var_os("ESSMAI_V5_TEST_FIXTURE_EXPORT") else {return};
        let root=std::path::PathBuf::from(root);assert!(root.is_absolute());
        let name=format!("{}-{}",if parent.is_some(){"parent"}else{"initial"},if negative{"negative"}else{"positive"});
        let dir=root.join(name);std::fs::create_dir(&dir).unwrap();
        let write=|name:&str,bytes:&[u8]|{use std::io::Write;
            let mut file=std::fs::OpenOptions::new().create_new(true).write(true).open(dir.join(name)).unwrap();
            file.write_all(bytes).unwrap();file.sync_all().unwrap();};
        write("prepared.bin",&bundle.prepared);write("response.wire",&bundle.response);
        write("continuation.bin",bundle.continuation.as_ref().unwrap());
        if let Some(parent)=parent {write("parent.record",&pd::encode_record(parent).unwrap());}
        write("NOTICE.txt",b"SYNTHETIC STORAGE FIXTURE. Actual PD checkpoint/pending; synthetic selection, layer and receipt data. No native GCL admission and no accepted scientific P/N. Request SHA [7;32], transaction placeholder [9;32].\n");
        let (env,_,continuation)=bundle.validated().unwrap();let continuation=continuation.unwrap();
        let mut engine=restore_for_followup(bundle,env.cognitive_session_id(),"science",&continuation.next_i0_id,1002).unwrap();
        let cycle=engine.ingest_for_spine_with_history_sealed(env.cognitive_session_id(),"native-iz-followup",
            "i0-00000000000000b2","The followup preserves measured alpha and beta history.",1002,PD_SEAL).unwrap();
        let (child,pending)=super::tests::prepared_from_cycle_with_links(&engine,cycle,env.cognitive_session_id(),
            "native-iz-followup","i0-00000000000000b2",1002,
            Some(PdContinuationLink::from_bundle(bundle).unwrap()),None,None);
        child.validate_predecessor(bundle).unwrap();assert_eq!(child.version,PREPARED_MILLIS_CONTINUITY_VERSION);
        let response=super::continuation_tests::synthetic_response(&super::continuation_tests::synthetic_completion(&pending,negative));
        let continuation=shadow_contracts::pd_prepared::derive_continuation(&child,&response).unwrap();
        write("child.prepared.bin",&encode_prepared(&child).unwrap());
        write("child.response.wire",&shadow_contracts::encode_cycle_response(&response));
        write("child.continuation.bin",&shadow_contracts::pd_prepared::encode_continuation(&continuation).unwrap());
    }
    #[test]
    fn v5_initial_restore_preserves_original_native_territory_mode_policy(){
        for (territory,expected) in [("WORK→NURA",DebateMode::Intellect),("science",DebateMode::Scientific),
            ("philosophy",DebateMode::Philosophy)] {
            let mut context=initial();context.identity.scope=territory.into();context.identity.domain=territory.into();
            let prepared=match crate::pd_iz::prepare_with(&context,&mut|_,_|pd::encode_optional_record(None)).unwrap(){
                crate::pd_iz::Preparation::Ready(ready)=>ready,_=>panic!("new fixture unexpectedly replayed")};
            let (q,r)=pair(&context,true);let activated=crate::pd_iz::before_action(&prepared,q).unwrap();
            let origin=crate::pd_iz::prepare_promotion(prepared,activated,r,&"77".repeat(32)).unwrap();
            let engine=crate::pd_iz::restore_for_promotion(&origin,territory,1000).unwrap();
            let state=engine.session(&origin.link.cognitive_session_id).unwrap();
            assert_eq!(state.pressure_budget.current_mode,expected);assert_eq!(state.total_turns,0);
        }
    }
    #[test]
    fn v5_source_lgc_input_clock_and_policy_ablations_fail(){
        let parent=parent_record();let (env,_)=build(Some(&parent));
        let mut wrong=env.clone();wrong.source_iz.as_mut().unwrap().prior_lgc_head[0]^=1;
        assert!(wrong.validate_iz_source(&parent).is_err());
        let mut wrong=env.clone();wrong.source_iz.as_mut().unwrap().current_input_sha256[0]^=1;
        assert!(wrong.validate().is_err());
        let mut wrong=env.clone();wrong.source_iz.as_mut().unwrap().cognitive_session_id="other-case".into();
        assert!(wrong.validate().is_err());
        let mut wrong=env.clone();wrong.source_iz.as_mut().unwrap().context.user_text.push_str(" substituted");
        assert!(wrong.validate().is_err());
        let mut wrong=env.clone();wrong.version=PREPARED_CONTINUITY_VERSION;
        assert!(wrong.validate().is_err());
        let mut wrong=env.clone();wrong.pending.spine.created_at=1;
        assert!(wrong.validate().is_err());
        let mut source=parent.clone();source.request.checkpoint[0]^=1;
        assert!(env.validate_iz_source(&source).is_err());
        assert!(env.validate_for_request(b"unbound physical attempt").is_err());
        let bytes=encode_prepared(&env).unwrap();let mut tampered=bytes.clone();tampered[40]^=1;
        assert!(decode_prepared(&tampered).is_err());assert!(decode_prepared(&bytes[..bytes.len()-1]).is_err());
    }
}
