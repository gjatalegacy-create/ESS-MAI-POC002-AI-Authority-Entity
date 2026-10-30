//! V189_DERIVATIVE: portable historical prepared-PD data.
//! No live token, verifier, knowledge commit or continuation authorization.
//! Source: Quantum progressive_debatic/types.rs:258-332 and existing spine wire.
use crate::{digest_parts, PdSpineEvidenceWire, Reader, Writer};
use sha2::{Digest, Sha256};

#[cfg(test)]
std::thread_local! {
    static CONTINUATION_TEXT_FORMATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub const PREPARED_VERSION: u16 = 1;
pub const PREPARED_CONTINUITY_VERSION: u16 = 2;
pub const PREPARED_DIALOGUE_VERSION: u16 = 3;
pub const PREPARED_MILLIS_CONTINUITY_VERSION: u16 = 4;
pub const PREPARED_IZ_ORIGIN_VERSION: u16 = 5;
pub const PREPARED_IZ_CONTINUITY_VERSION: u16 = 6;
pub const PREPARED_DIALOGUE_CONTINUITY_VERSION: u16 = 7;
pub const MAX_CHECKPOINT_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_LAYER_BYTES: usize = 1024 * 1024;
pub const MAX_PREPARED_BYTES: usize = 12 * 1024 * 1024;
const MAGIC: &[u8; 8] = b"ESSPDP01";
const WITNESSED_MAGIC: &[u8; 8] = b"ESSPDW01";
const MAX_TEXT: usize = 64 * 1024;
pub type Hash = [u8; 32];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdPreSealData {
    pub seal: u64,
    pub session_id: String,
    pub parent_i0: String,
    pub receipt_id: String,
    pub prepared_at: u64,
    pub spine_activation_id: u64,
    pub spine_completion_digest: u64,
    pub layer_mask: u8,
    pub mpro_mass: u32,
    pub action_sha256: Hash,
    pub law_trace_sha256: Hash,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdPendingData {
    pub session_id: String,
    pub parent_i0: String,
    pub turn_id: String,
    pub prepared_question: String,
    /// Explicit stable tags in source PdResponseType declaration order, 0..=8.
    pub response_type: u8,
    pub prepared_at: u64,
    pub spine: PdSpineEvidenceWire,
    /// Preserve native fields omitted by the existing flattened spine wire.
    pub spine_request_state: u8,
    pub stimulus_contract_version: u32,
    pub increment_contract_version: u32,
    pub increment_stimulus_digest: u64,
    pub pre_seal: PdPreSealData,
}
/// R3 historical reference only. Shadow resolves this against its own durable
/// T1/T2 and topic head; none of these public data fields grants authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdContinuationLink {
    pub cognitive_session_id: String,
    pub territory: String,
    pub predecessor: crate::pd_recovery::RecoveryLookup,
    pub predecessor_transaction_id: Hash,
    pub predecessor_continuation_sha256: Hash,
    pub predecessor_next_i0: String,
    pub clock_millis: bool,
}
impl PdContinuationLink {
    pub fn validate(&self) -> Result<(), String> {
        text(&self.cognitive_session_id, 256)?;
        text(&self.territory, MAX_TEXT)?;
        text(&self.predecessor_next_i0, 256)?;
        self.predecessor.validate()?;
        need(
            self.predecessor_transaction_id != [0; 32]
                && self.predecessor_continuation_sha256 != [0; 32],
            "PREDECESSOR_UNBOUND",
        )
    }
    pub fn from_bundle(bundle: &crate::pd_recovery::RecoveryBundle) -> Result<Self, String> {
        let (envelope, _, data) = bundle.validated()?;
        let data = data.ok_or("PD_PREPARED_PREDECESSOR_T2_MISSING")?;
        let bytes = bundle
            .continuation
            .as_ref()
            .ok_or("PD_PREPARED_PREDECESSOR_T2_MISSING")?;
        let meta = crate::pd_checkpoint_data::checkpoint_metadata(
            &envelope.checkpoint,
            envelope.cognitive_session_id(),
        )?;
        let link = Self {
            cognitive_session_id: meta.session_id,
            territory: meta.territory,
            predecessor: crate::pd_recovery::RecoveryLookup {
                identity: bundle.identity,
                request_sha256: bundle.request_sha256,
            },
            predecessor_transaction_id: bundle.transaction_id,
            predecessor_continuation_sha256: sha(bytes),
            predecessor_next_i0: data.next_i0_id,
            clock_millis: envelope.clock_is_millis(),
        };
        link.validate()?;
        Ok(link)
    }
}
/// Initial native transition from durable operational PD history. This is a
/// reference, not a fabricated native predecessor or an acceptance capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdDialogueLink {
    pub context: crate::kie::KieContext,
    pub cognitive_session_id: String,
}
impl PdDialogueLink {
    pub fn new(context: crate::kie::KieContext) -> Result<Self, String> {
        let cognitive_session_id = crate::pd_memory::hex(&context.identity.key()?);
        let link = Self {
            context,
            cognitive_session_id,
        };
        link.validate()?;
        Ok(link)
    }
    pub fn validate(&self) -> Result<(), String> {
        self.context.validate()?;
        need(
            self.cognitive_session_id == crate::pd_memory::hex(&self.context.identity.key()?),
            "DIALOGUE_COGNITIVE_ID",
        )
    }
}
/// Source iZ input/history and an observed successful ASHT attempt.
/// Public historical data, never a replacement GCL/ASHT/NBES authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdIzOriginLink {
    pub context: crate::pd_memory::iz::IzContext,
    pub cognitive_session_id: String,
    pub prior_lgc_head: Hash,
    pub current_input_sha256: Hash,
    pub passed: crate::pd_memory::iz::IzPassedAttempt,
}
impl PdIzOriginLink {
    pub fn new(
        context: crate::pd_memory::iz::IzContext,
        prior_lgc_head: Hash,
        passed: crate::pd_memory::iz::IzPassedAttempt,
    ) -> Result<Self, String> {
        let link = Self {
            cognitive_session_id: crate::pd_memory::hex(&context.identity.key()?),
            current_input_sha256: sha(context.user_text.as_bytes()),
            context,
            prior_lgc_head,
            passed,
        };
        link.validate()?;
        Ok(link)
    }
    pub fn validate(&self) -> Result<(), String> {
        self.context.validate()?;
        need(
            matches!(
                self.context.version,
                crate::pd_memory::iz::IZ_ACTUAL_INPUT_VERSION
                    | crate::pd_memory::iz::IZ_NATIVE_CONTINUITY_VERSION
            ),
            "IZ_CONTEXT_VERSION",
        )?;
        need(
            self.context.identity.domain == self.context.identity.scope,
            "IZ_DOMAIN_SCOPE",
        )?;
        need(
            self.cognitive_session_id == crate::pd_memory::hex(&self.context.identity.key()?),
            "IZ_COGNITIVE_ID",
        )?;
        need(
            self.current_input_sha256 == sha(self.context.user_text.as_bytes()),
            "IZ_CURRENT_INPUT",
        )?;
        if self.context.version == crate::pd_memory::iz::IZ_NATIVE_CONTINUITY_VERSION {
            self.context.require_mapped()?;
            need(
                self.context.parent.is_some() == (self.prior_lgc_head != [0; 32]),
                "IZ_NATIVE_PARENT_LGC_POLICY",
            )?;
        } else {
            match (&self.context.parent, &self.context.map) {
                (Some(_), Some(_)) => {
                    need(self.prior_lgc_head != [0; 32], "IZ_PARENT_LGC_MISSING")?
                }
                (None, None) => need(
                    self.prior_lgc_head == [0; 32],
                    "IZ_INITIAL_LGC_MUST_BE_EMPTY",
                )?,
                _ => return Err("PD_PREPARED_IZ_PARENT_MAP_POLICY".into()),
            }
        }
        self.passed.validate(&self.context)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdPreparedEnvelope {
    pub version: u16,
    /// SHA256 of the actual encoded base ShadowCycleRequest, without this envelope.
    pub source_request_sha256: Hash,
    /// Raw generic PD checkpoint; producer policy/admission remains the caller's.
    pub checkpoint: Vec<u8>,
    pub pending: PdPendingData,
    pub continuity: Option<PdContinuationLink>,
    pub source_dialogue: Option<PdDialogueLink>,
    pub source_iz: Option<PdIzOriginLink>,
    /// Optional R10 wrapper; legacy base bytes and seal remain unchanged.
    pub failure_witness: Option<crate::pd_failure_witness::PdFailureWitness>,
}
fn sha(bytes: &[u8]) -> Hash {
    Sha256::digest(bytes).into()
}
fn need(ok: bool, message: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(format!("PD_PREPARED_{message}"))
    }
}
fn text(value: &str, max: usize) -> Result<(), String> {
    need(
        !value.trim().is_empty() && value.len() <= max && !value.contains('\0'),
        "TEXT",
    )
}
pub fn response_label(tag: u8) -> Result<&'static str, String> {
    match tag {
        0 => Ok("HELP"),
        1 => Ok("ORIENT"),
        2 => Ok("CLARIFY"),
        3 => Ok("CHALLENGE_GENTLE"),
        4 => Ok("CHALLENGE_WITH_FACTS"),
        5 => Ok("SCAFFOLD_BULLET"),
        6 => Ok("SCAFFOLD_MEASURE"),
        7 => Ok("SCAFFOLD_STRUCTURE"),
        8 => Ok("LISTEN"),
        _ => Err("PD_PREPARED_RESPONSE_TAG".into()),
    }
}
/// Exact original pd_id fold, not digest_parts. It is a historical identifier.
fn source_pd_id(parts: &[&[u8]]) -> String {
    let value = parts.iter().fold(0xcbf29ce484222325u64, |acc, part| {
        crate::fnv1a64(part) ^ acc.wrapping_mul(1099511628211)
    });
    format!("{:012x}", value & 0xFFFF_FFFF_FFFF)
}
impl PdPendingData {
    pub fn binding_digest(&self) -> Result<u64, String> {
        let p = &self.pre_seal;
        Ok(digest_parts(&[
            self.session_id.as_bytes(),
            self.parent_i0.as_bytes(),
            self.turn_id.as_bytes(),
            self.prepared_question.as_bytes(),
            response_label(self.response_type)?.as_bytes(),
            &self.prepared_at.to_le_bytes(),
            p.receipt_id.as_bytes(),
            &p.spine_activation_id.to_le_bytes(),
            &p.spine_completion_digest.to_le_bytes(),
            &p.mpro_mass.to_le_bytes(),
            &p.action_sha256,
            &p.law_trace_sha256,
            &self.spine.continuum.activation_digest.to_le_bytes(),
            &self.spine.continuum.increment_digest.to_le_bytes(),
            b"PD_CANDIDATE_BINDING_V156",
        ]))
    }
    pub fn validate(&self) -> Result<(), String> {
        for value in [&self.session_id, &self.parent_i0, &self.turn_id] {
            text(value, 256)?;
        }
        text(&self.prepared_question, MAX_TEXT)?;
        let label = response_label(self.response_type)?;
        let s = &self.spine;
        need(s.verifies_internal(), "SPINE_INTEGRITY")?;
        need(self.spine_request_state == 3, "SPINE_STATE")?; // native SpineActivated
        need(
            self.stimulus_contract_version == s.continuum.contract_version
                && self.increment_contract_version == s.continuum.contract_version
                && self.increment_stimulus_digest == s.continuum.stimulus_digest,
            "NESTED_VERSION_OR_PARENT",
        )?;
        need(
            s.session_digest == digest_parts(&[self.session_id.as_bytes()])
                && s.parent_i0_digest == digest_parts(&[self.parent_i0.as_bytes()])
                && s.continuum.session_digest == s.session_digest
                && s.continuum.i0_digest == s.parent_i0_digest,
            "PENDING_SCOPE",
        )?;
        need(
            s.continuum.question_digest == digest_parts(&[self.prepared_question.as_bytes()])
                && s.continuum.response_kind_digest == digest_parts(&[label.as_bytes()])
                && s.activation.response_kind_digest == s.continuum.response_kind_digest,
            "QUESTION_BINDING",
        )?;
        for layer in [&s.layer1, &s.layer2, &s.layer3] {
            need(
                layer.result_material.len() <= MAX_LAYER_BYTES,
                "LAYER_LIMIT",
            )?;
        }
        let p = &self.pre_seal;
        text(&p.receipt_id, 256)?;
        need(
            p.session_id == self.session_id
                && p.parent_i0 == self.parent_i0
                && p.prepared_at == self.prepared_at
                && p.spine_activation_id == s.activation_id
                && p.spine_completion_digest == s.completion_digest
                && p.layer_mask == s.layer_mask
                && p.mpro_mass == s.mpro_mass,
            "PRESEAL_BINDING",
        )?;
        let expected_seal =
            crate::fnv1a64(b"PROGRESSIVE_DEBATIC") ^ crate::fnv1a64(b"GJATA_LEGACY");
        need(p.seal == expected_seal, "PRESEAL_STATIC_MARKER")?;
        // Exact original seal.rs:132-160 data rederivation; not token issuance.
        let mut action = b"ESS_MAI_GCL_PD_ACTION_V160".to_vec();
        action.extend_from_slice(self.session_id.as_bytes());
        action.extend_from_slice(self.parent_i0.as_bytes());
        for value in [
            s.activation_id,
            s.activation.contract_digest,
            s.gcl_process_digest,
            s.layer1.result_digest,
            s.layer2.result_digest,
            s.layer3.result_digest,
            s.evidence_digest,
        ] {
            action.extend_from_slice(&value.to_le_bytes());
        }
        action.extend_from_slice(&s.mpro_mass.to_le_bytes());
        action.extend_from_slice(&s.completed_at.to_le_bytes());
        need(sha(&action) == p.action_sha256, "PRESEAL_ACTION")?;
        let law_mask: u32 = 23 | if s.activation.origin_code == 1 { 8 } else { 0 };
        let mut law = b"ESS_MAI_IS_GCL_LAW_TRACE_V160".to_vec();
        law.extend_from_slice(&p.action_sha256);
        law.extend_from_slice(&law_mask.to_le_bytes());
        law.extend_from_slice(s.activation.mode_label().ok_or("PD_PREPARED_MODE")?);
        law.extend_from_slice(s.activation.origin_label().ok_or("PD_PREPARED_ORIGIN")?);
        law.push(s.activation.required_layer_mask);
        for value in [s.gcl_law_seal, s.gcl_system_laws_seal, s.gcl_process_digest] {
            law.extend_from_slice(&value.to_le_bytes());
        }
        need(sha(&law) == p.law_trace_sha256, "PRESEAL_LAW_TRACE")?;
        let receipt = source_pd_id(&[
            self.session_id.as_bytes(),
            self.parent_i0.as_bytes(),
            &p.prepared_at.to_le_bytes(),
            &p.spine_activation_id.to_le_bytes(),
            &p.spine_completion_digest.to_le_bytes(),
            &[p.layer_mask],
            &p.mpro_mass.to_le_bytes(),
            &p.action_sha256,
            &p.law_trace_sha256,
            &p.seal.to_le_bytes(),
            b"GCL_PD_PRESEAL_V160",
        ]);
        need(receipt == p.receipt_id, "PRESEAL_RECEIPT")?;
        Ok(())
    }
}
impl PdPreparedEnvelope {
    pub fn validate(&self) -> Result<(), String> {
        match (
            self.version,
            self.continuity.as_ref(),
            self.source_dialogue.as_ref(),
            self.source_iz.as_ref(),
        ) {
            (PREPARED_VERSION, None, None, None) => {}
            (PREPARED_CONTINUITY_VERSION, Some(link), None, None) if !link.clock_millis => {
                link.validate()?;
            }
            (PREPARED_MILLIS_CONTINUITY_VERSION, Some(link), None, None) if link.clock_millis => {
                link.validate()?;
            }
            (PREPARED_DIALOGUE_VERSION, None, Some(link), None) => {
                link.validate()?;
            }
            (PREPARED_IZ_ORIGIN_VERSION, None, None, Some(link)) => {
                link.validate()?;
                need(
                    link.context.version == crate::pd_memory::iz::IZ_ACTUAL_INPUT_VERSION,
                    "IZ_LEGACY_CONTEXT_VERSION",
                )?;
            }
            (PREPARED_IZ_CONTINUITY_VERSION, Some(native), None, Some(link))
                if native.clock_millis =>
            {
                native.validate()?;
                link.validate()?;
                need(
                    link.context.version == crate::pd_memory::iz::IZ_NATIVE_CONTINUITY_VERSION
                        && link.context.native_anchor.as_ref() == Some(native),
                    "IZ_COMBINED_ANCHOR",
                )?;
            }
            (PREPARED_DIALOGUE_CONTINUITY_VERSION, Some(native), Some(link), None)
                if native.clock_millis =>
            {
                native.validate()?;
                link.validate()?;
                need(
                    native.cognitive_session_id == link.cognitive_session_id
                        && native.territory == link.context.identity.scope,
                    "DIALOGUE_COMBINED_ANCHOR",
                )?;
            }
            _ => return Err("PD_PREPARED_VERSION_OR_CONTINUITY".into()),
        }
        need(self.source_request_sha256 != [0; 32], "SOURCE_UNBOUND")?;
        need(
            !self.checkpoint.is_empty() && self.checkpoint.len() <= MAX_CHECKPOINT_BYTES,
            "CHECKPOINT_LIMIT",
        )?;
        self.pending.validate()?;
        if let Some(witness) = &self.failure_witness {
            witness.validate()?;
            let meta = crate::pd_checkpoint_data::checkpoint_metadata(&self.checkpoint, self.cognitive_session_id())?;
            need(witness.request_sha256 == self.source_request_sha256
                && witness.source_trace_digest == self.pending.spine.trace_digest
                && witness.input_sha256 == sha(meta.last_user_text.as_bytes()), "WITNESS_BINDING")?;
        }
        crate::pd_checkpoint_data::validate_prepared_checkpoint_for_session(
            &self.checkpoint,
            &self.pending,
            self.cognitive_session_id(),
        )?;
        if let Some(link) = &self.continuity {
            let meta = crate::pd_checkpoint_data::checkpoint_metadata(
                &self.checkpoint,
                self.cognitive_session_id(),
            )?;
            need(meta.territory == link.territory, "COGNITIVE_TERRITORY")?;
        }
        if let Some(link) = &self.source_dialogue {
            let meta = crate::pd_checkpoint_data::checkpoint_metadata(
                &self.checkpoint,
                self.cognitive_session_id(),
            )?;
            need(
                meta.territory == link.context.identity.scope,
                "DIALOGUE_TERRITORY",
            )?;
        }
        if let Some(link) = &self.source_iz {
            let meta = crate::pd_checkpoint_data::checkpoint_metadata(
                &self.checkpoint,
                self.cognitive_session_id(),
            )?;
            need(
                meta.territory == link.context.identity.scope,
                "IZ_TERRITORY",
            )?;
            need(
                meta.last_user_text == link.context.user_text,
                "IZ_EXACT_CURRENT_USER",
            )?;
        }
        Ok(())
    }
    /// Explicit wire policy: the original native path uses seconds, while
    /// operational dialogue uses milliseconds. Historical values are not guessed.
    pub fn clock_is_millis(&self) -> bool {
        matches!(
            self.version,
            PREPARED_DIALOGUE_VERSION
                | PREPARED_MILLIS_CONTINUITY_VERSION
                | PREPARED_IZ_ORIGIN_VERSION
                | PREPARED_IZ_CONTINUITY_VERSION
                | PREPARED_DIALOGUE_CONTINUITY_VERSION
        )
    }
    pub fn cognitive_session_id(&self) -> &str {
        if let Some(link) = &self.source_iz {
            return &link.cognitive_session_id;
        }
        if let Some(link) = &self.source_dialogue {
            return &link.cognitive_session_id;
        }
        match &self.continuity {
            Some(link) => &link.cognitive_session_id,
            None => &self.pending.session_id,
        }
    }
    /// Shadow must supply its own durable, latest source record under commit_lock.
    pub fn validate_dialogue_source(
        &self,
        source: &crate::pd_memory::PdOperationRecord,
    ) -> Result<(), String> {
        self.validate()?;
        source.validate()?;
        let link = self
            .source_dialogue
            .as_ref()
            .ok_or("PD_PREPARED_DIALOGUE_MISSING")?;
        need(
            source.record_id == link.context.record_id
                && source.request.identity == link.context.identity
                && source.request.operation_id == link.context.operation_id,
            "DIALOGUE_SOURCE_BINDING",
        )?;
        let previous = crate::pd_memory::linked_science::science_checkpoint_raw(source)?;
        let before =
            crate::pd_checkpoint_data::checkpoint_metadata(previous, self.cognitive_session_id())?;
        let current = crate::pd_checkpoint_data::checkpoint_metadata(
            &self.checkpoint,
            self.cognitive_session_id(),
        )?;
        need(before.territory == current.territory, "DIALOGUE_SCOPE")?;
        crate::pd_checkpoint_data::validate_user_checkpoint_transition(
            &self.checkpoint,
            self.cognitive_session_id(),
            &current.last_user_text,
            u64::from(before.total_turns),
            Some(previous),
        )
    }
    /// Shadow must pass its own durable, current operational parent under commit_lock.
    pub fn validate_iz_source(
        &self,
        source: &crate::pd_memory::PdOperationRecord,
    ) -> Result<(), String> {
        self.validate()?;
        source.validate()?;
        let link = self
            .source_iz
            .as_ref()
            .ok_or("PD_PREPARED_IZ_SOURCE_MISSING")?;
        need(link.context.parent.is_some(), "IZ_EXPECTED_STORED_PARENT")?;
        crate::pd_memory::iz::validate_context_parent(&link.context, source)?;
        let tip = source
            .request
            .lgc_steps
            .last()
            .ok_or("PD_PREPARED_IZ_PARENT_LGC_EMPTY")?
            .step_digest;
        need(
            tip == link.prior_lgc_head && tip == source.request.witness.lgc_parent,
            "IZ_PARENT_LGC_BINDING",
        )?;
        if self.version == PREPARED_IZ_CONTINUITY_VERSION
            && matches!(
                link.context.native_map.as_ref().map(|m| &m.checkpoint_base),
                Some(crate::pd_memory::iz::IzCheckpointBase::NativeT2)
            )
        {
            // The exact native +1 checkpoint is checked using the actual bundle
            // by validate_iz_continuation_source, never against this older CAS head.
            return Ok(());
        }
        let previous = crate::pd_memory::iz::checkpoint_raw(source)?;
        let before =
            crate::pd_checkpoint_data::checkpoint_metadata(previous, self.cognitive_session_id())?;
        need(
            before.territory == link.context.identity.scope,
            "IZ_SOURCE_SCOPE",
        )?;
        crate::pd_checkpoint_data::validate_user_checkpoint_transition(
            &self.checkpoint,
            self.cognitive_session_id(),
            &link.context.user_text,
            u64::from(before.total_turns),
            Some(previous),
        )
    }
    /// A genuine new case has no fabricated PIZ parent. Shadow additionally requires
    /// no operational or native head for this cognitive identity under commit_lock.
    pub fn validate_iz_initial(&self) -> Result<(), String> {
        self.validate()?;
        let link = self
            .source_iz
            .as_ref()
            .ok_or("PD_PREPARED_IZ_SOURCE_MISSING")?;
        need(
            self.version == PREPARED_IZ_ORIGIN_VERSION,
            "IZ_INITIAL_VERSION",
        )?;
        need(
            link.context.parent.is_none()
                && link.context.map.is_none()
                && link.prior_lgc_head == [0; 32],
            "IZ_INITIAL_PARENT_POLICY",
        )?;
        crate::pd_checkpoint_data::validate_user_checkpoint_transition(
            &self.checkpoint,
            self.cognitive_session_id(),
            &link.context.user_text,
            0,
            None,
        )
    }
    /// Pure historical checks. Production must pass the Shadow-owned bundle,
    /// and enforce topic-head CAS in the same transaction as the new T1.
    pub fn validate_predecessor(
        &self,
        bundle: &crate::pd_recovery::RecoveryBundle,
    ) -> Result<(), String> {
        self.validate()?;
        let link = self
            .continuity
            .as_ref()
            .ok_or("PD_PREPARED_CONTINUITY_MISSING")?;
        bundle.validate_for(&link.predecessor)?;
        need(
            PdContinuationLink::from_bundle(bundle)? == *link,
            "PREDECESSOR_LINK_MISMATCH",
        )?;
        let (prior, _, _) = bundle.validated()?;
        need(
            self.pending.session_id != prior.pending.session_id,
            "ATTEMPT_REUSED",
        )?;
        let before = crate::pd_checkpoint_data::checkpoint_metadata(
            &prior.checkpoint,
            prior.cognitive_session_id(),
        )?;
        let current = crate::pd_checkpoint_data::checkpoint_metadata(
            &self.checkpoint,
            self.cognitive_session_id(),
        )?;
        need(
            before.session_id == current.session_id && before.territory == current.territory,
            "COGNITIVE_LINEAGE",
        )?;
        if matches!(
            self.version,
            PREPARED_IZ_CONTINUITY_VERSION | PREPARED_DIALOGUE_CONTINUITY_VERSION
        ) {
            // An intervening durable operational turn is allowed, but the caller
            // must also invoke the corresponding exact source validation below.
            need(
                current.total_turns > before.total_turns,
                "COMBINED_HISTORY_NOT_ADVANCED",
            )?;
            return Ok(());
        }
        crate::pd_checkpoint_data::validate_user_checkpoint_transition(
            &self.checkpoint,
            self.cognitive_session_id(),
            &current.last_user_text,
            u64::from(before.total_turns),
            Some(&prior.checkpoint),
        )
    }
    /// Complete dual validation for native -> operational iZ -> native.
    /// Shadow must resolve both current heads atomically; this function grants no authority.
    pub fn validate_iz_continuation_source(
        &self,
        bundle: &crate::pd_recovery::RecoveryBundle,
        head: Option<&crate::pd_memory::PdOperationRecord>,
    ) -> Result<(), String> {
        need(
            self.version == PREPARED_IZ_CONTINUITY_VERSION,
            "IZ_COMBINED_VERSION",
        )?;
        self.validate_predecessor(bundle)?;
        let link = self
            .source_iz
            .as_ref()
            .ok_or("PD_PREPARED_IZ_SOURCE_MISSING")?;
        crate::pd_memory::iz::validate_native_binding(&link.context, bundle, head)?;
        if let Some(source) = head {
            self.validate_iz_source(source)?;
        }
        let (native, _, _) = bundle.validated()?;
        let previous = match &link
            .context
            .native_map
            .as_ref()
            .ok_or("IZ_NATIVE_MAP_REQUIRED")?
            .checkpoint_base
        {
            crate::pd_memory::iz::IzCheckpointBase::NativeT2 => native.checkpoint.as_slice(),
            crate::pd_memory::iz::IzCheckpointBase::Operational(_) => {
                crate::pd_memory::iz::checkpoint_raw(head.ok_or("IZ_OPERATIONAL_BASE_REQUIRED")?)?
            }
        };
        let before =
            crate::pd_checkpoint_data::checkpoint_metadata(previous, self.cognitive_session_id())?;
        crate::pd_checkpoint_data::validate_user_checkpoint_transition(
            &self.checkpoint,
            self.cognitive_session_id(),
            &link.context.user_text,
            u64::from(before.total_turns),
            Some(previous),
        )
    }
    /// Complete dual validation for native -> PIZ3 -> PDS3 -> unchanged KiE gates.
    pub fn validate_dialogue_continuation_source(
        &self,
        bundle: &crate::pd_recovery::RecoveryBundle,
        source: &crate::pd_memory::PdOperationRecord,
    ) -> Result<(), String> {
        need(
            self.version == PREPARED_DIALOGUE_CONTINUITY_VERSION,
            "DIALOGUE_COMBINED_VERSION",
        )?;
        self.validate_predecessor(bundle)?;
        self.validate_dialogue_source(source)?;
        need(
            crate::pd_memory::iz_science::source_is_native_science(&source.request.source_input),
            "DIALOGUE_ANCHORED_SOURCE_REQUIRED",
        )?;
        let context = crate::pd_memory::iz::context_from_record(source)?;
        need(
            context.native_anchor.as_ref() == self.continuity.as_ref(),
            "DIALOGUE_SOURCE_NATIVE_ANCHOR",
        )?;
        Ok(())
    }
    /// Verifies data binding to the actual base request. This does not execute
    /// Shadow admission or authorize its proposed knowledge transaction.
    pub fn validate_for_request(&self, bytes: &[u8]) -> Result<(), String> {
        self.validate()?;
        need(
            sha(bytes) == self.source_request_sha256,
            "SOURCE_REQUEST_HASH",
        )?;
        let request = crate::decode_cycle_request(bytes)
            .map_err(|e| format!("PD_PREPARED_SOURCE_CODEC: {e:?}"))?;
        need(
            crate::encode_cycle_request(&request) == bytes,
            "SOURCE_CANONICAL",
        )?;
        if let Some(witness) = &self.failure_witness { witness.validate_for_request(&request)?; }
        let q = &request.quantum;
        need(
            q.session_id == self.pending.session_id
                && request.light.session_id == self.pending.session_id,
            "SOURCE_SESSION",
        )?;
        need(
            q.pd_parent_i0.as_deref() == Some(self.pending.parent_i0.as_str())
                && q.pd_binding_digest == Some(self.pending.binding_digest()?)
                && q.pd_continuum_activation_digest
                    == Some(self.pending.spine.continuum.activation_digest),
            "SOURCE_PENDING_BINDING",
        )?;
        need(
            q.final_evidence.pd_spine == self.pending.spine,
            "SOURCE_SPINE_MATERIAL",
        )?;
        need(
            self.pending.spine.continuum.user_input_digest
                == digest_parts(&[&q.final_evidence.light_input_bytes]),
            "SOURCE_USER_INPUT",
        )?;
        if self.continuity.is_some() || self.source_dialogue.is_some() || self.source_iz.is_some() {
            let meta = crate::pd_checkpoint_data::checkpoint_metadata(
                &self.checkpoint,
                self.cognitive_session_id(),
            )?;
            need(
                meta.last_user_text.as_bytes() == q.final_evidence.light_input_bytes.as_slice(),
                "SOURCE_EXACT_USER_INPUT",
            )?;
            need(
                q.territory == meta.territory && request.light.territory == meta.territory,
                "SOURCE_COGNITIVE_TERRITORY",
            )?;
        }
        if let Some(link) = &self.source_iz {
            link.passed
                .validate_for_native_request(&link.context, &request)?;
        }
        Ok(())
    }
}
fn preseal_out(w: &mut Writer, p: &PdPreSealData) {
    w.u64(p.seal);
    w.string(&p.session_id);
    w.string(&p.parent_i0);
    w.string(&p.receipt_id);
    w.u64(p.prepared_at);
    w.u64(p.spine_activation_id);
    w.u64(p.spine_completion_digest);
    w.u8(p.layer_mask);
    w.u32(p.mpro_mass);
    w.bytes.extend_from_slice(&p.action_sha256);
    w.bytes.extend_from_slice(&p.law_trace_sha256);
}
fn hash_in(r: &mut Reader<'_>) -> Result<Hash, crate::WireError> {
    let mut out = [0; 32];
    out.copy_from_slice(r.take(32)?);
    Ok(out)
}
fn preseal_in(r: &mut Reader<'_>) -> Result<PdPreSealData, crate::WireError> {
    Ok(PdPreSealData {
        seal: r.u64()?,
        session_id: r.string()?,
        parent_i0: r.string()?,
        receipt_id: r.string()?,
        prepared_at: r.u64()?,
        spine_activation_id: r.u64()?,
        spine_completion_digest: r.u64()?,
        layer_mask: r.u8()?,
        mpro_mass: r.u32()?,
        action_sha256: hash_in(r)?,
        law_trace_sha256: hash_in(r)?,
    })
}
pub fn encode_prepared(env: &PdPreparedEnvelope) -> Result<Vec<u8>, String> {
    env.validate()?;
    let base = encode_prepared_base(env)?;
    if let Some(witness) = &env.failure_witness {
        let mut w = Writer::new();
        w.bytes.extend_from_slice(WITNESSED_MAGIC);
        w.u16(1);
        w.bytes(&base);
        w.bytes(&witness.encode()?);
        need(w.bytes.len() <= MAX_PREPARED_BYTES - 32, "WITNESS_ENCODE_LIMIT")?;
        let digest = sha(&w.bytes);
        w.bytes.extend_from_slice(&digest);
        Ok(w.bytes)
    } else { Ok(base) }
}
fn encode_prepared_base(env: &PdPreparedEnvelope) -> Result<Vec<u8>, String> {
    let mut w = Writer::new();
    w.bytes.extend_from_slice(MAGIC);
    w.u16(env.version);
    w.bytes.extend_from_slice(&env.source_request_sha256);
    w.bytes(&env.checkpoint);
    let p = &env.pending;
    for value in [
        &p.session_id,
        &p.parent_i0,
        &p.turn_id,
        &p.prepared_question,
    ] {
        w.string(value);
    }
    w.u8(p.response_type);
    w.u64(p.prepared_at);
    w.u8(p.spine_request_state);
    w.u32(p.stimulus_contract_version);
    w.u32(p.increment_contract_version);
    w.u64(p.increment_stimulus_digest);
    preseal_out(&mut w, &p.pre_seal);
    crate::put_pd_spine_evidence(&mut w, &p.spine);
    if let Some(link) = &env.continuity {
        w.string(&link.cognitive_session_id);
        w.string(&link.territory);
        w.bytes.extend_from_slice(&link.predecessor.identity);
        w.bytes.extend_from_slice(&link.predecessor.request_sha256);
        w.bytes.extend_from_slice(&link.predecessor_transaction_id);
        w.bytes
            .extend_from_slice(&link.predecessor_continuation_sha256);
        w.string(&link.predecessor_next_i0);
        if matches!(
            env.version,
            PREPARED_MILLIS_CONTINUITY_VERSION
                | PREPARED_IZ_CONTINUITY_VERSION
                | PREPARED_DIALOGUE_CONTINUITY_VERSION
        ) {
            w.u8(u8::from(link.clock_millis));
        }
    }
    if let Some(link) = &env.source_dialogue {
        w.string(&link.cognitive_session_id);
        let i = &link.context.identity;
        for value in [
            &i.owner,
            &i.obligation,
            &i.case_id,
            &i.scope,
            &i.domain,
            &i.lineage_root,
            &link.context.operation_id,
        ] {
            w.string(value);
        }
        w.bytes.extend_from_slice(&link.context.record_id);
    }
    if let Some(link) = &env.source_iz {
        w.bytes(&crate::pd_memory::iz::encode_context(&link.context)?);
        w.string(&link.cognitive_session_id);
        w.bytes.extend_from_slice(&link.prior_lgc_head);
        w.bytes.extend_from_slice(&link.current_input_sha256);
        w.bytes(&crate::pd_memory::iz::encode_passed(&link.passed)?);
    }
    need(w.bytes.len() <= MAX_PREPARED_BYTES - 32, "ENCODE_LIMIT")?;
    let digest = sha(&w.bytes);
    w.bytes.extend_from_slice(&digest);
    Ok(w.bytes)
}
pub fn decode_prepared(bytes: &[u8]) -> Result<PdPreparedEnvelope, String> {
    need(
        bytes.len() >= 8 + 2 + 32 + 32 && bytes.len() <= MAX_PREPARED_BYTES,
        "DECODE_LIMIT",
    )?;
    let (body, digest) = bytes.split_at(bytes.len() - 32);
    need(sha(body).as_slice() == digest, "CHECKSUM")?;
    if body.starts_with(WITNESSED_MAGIC) {
        let mut r = Reader::new(body);
        r.take(8).map_err(|e| format!("PD_WITNESS_CODEC:{e:?}"))?;
        need(r.u16().map_err(|e| format!("PD_WITNESS_CODEC:{e:?}"))? == 1, "WITNESS_VERSION")?;
        let base = r.bytes().map_err(|e| format!("PD_WITNESS_CODEC:{e:?}"))?;
        let witness = r.bytes().map_err(|e| format!("PD_WITNESS_CODEC:{e:?}"))?;
        r.finish().map_err(|e| format!("PD_WITNESS_CODEC:{e:?}"))?;
        // No nested wrappers; historical v1-v7 remains an exact sealed subrecord.
        need(base.starts_with(MAGIC), "WITNESS_BASE_MAGIC")?;
        let mut envelope = decode_prepared(&base)?;
        envelope.failure_witness = Some(crate::pd_failure_witness::PdFailureWitness::decode(&witness)?);
        envelope.validate()?;
        need(encode_prepared(&envelope)? == bytes, "WITNESS_CANONICAL")?;
        return Ok(envelope);
    }
    let parsed = (|| -> Result<PdPreparedEnvelope, crate::WireError> {
        let mut r = Reader::new(body);
        if r.take(8)? != MAGIC {
            return Err(crate::WireError::InvalidMagic);
        }
        let version = r.u16()?;
        let source_request_sha256 = hash_in(&mut r)?;
        let checkpoint = r.bytes()?;
        let session_id = r.string()?;
        let parent_i0 = r.string()?;
        let turn_id = r.string()?;
        let prepared_question = r.string()?;
        let response_type = r.u8()?;
        let prepared_at = r.u64()?;
        let spine_request_state = r.u8()?;
        let stimulus_contract_version = r.u32()?;
        let increment_contract_version = r.u32()?;
        let increment_stimulus_digest = r.u64()?;
        let pre_seal = preseal_in(&mut r)?;
        let spine = crate::get_pd_spine_evidence(&mut r)?;
        let continuity = if version == PREPARED_CONTINUITY_VERSION
            || version == PREPARED_MILLIS_CONTINUITY_VERSION
            || version == PREPARED_IZ_CONTINUITY_VERSION
            || version == PREPARED_DIALOGUE_CONTINUITY_VERSION
        {
            Some(PdContinuationLink {
                cognitive_session_id: r.string()?,
                territory: r.string()?,
                predecessor: crate::pd_recovery::RecoveryLookup {
                    identity: hash_in(&mut r)?,
                    request_sha256: hash_in(&mut r)?,
                },
                predecessor_transaction_id: hash_in(&mut r)?,
                predecessor_continuation_sha256: hash_in(&mut r)?,
                predecessor_next_i0: r.string()?,
                clock_millis: if matches!(
                    version,
                    PREPARED_MILLIS_CONTINUITY_VERSION
                        | PREPARED_IZ_CONTINUITY_VERSION
                        | PREPARED_DIALOGUE_CONTINUITY_VERSION
                ) {
                    let marker = r.u8()?;
                    if marker != 1 {
                        return Err(crate::WireError::InvalidBool(marker));
                    }
                    true
                } else {
                    false
                },
            })
        } else {
            None
        };
        let source_dialogue = if matches!(
            version,
            PREPARED_DIALOGUE_VERSION | PREPARED_DIALOGUE_CONTINUITY_VERSION
        ) {
            let cognitive_session_id = r.string()?;
            Some(PdDialogueLink {
                cognitive_session_id,
                context: crate::kie::KieContext {
                    identity: crate::pd_memory::PdIdentity {
                        owner: r.string()?,
                        obligation: r.string()?,
                        case_id: r.string()?,
                        scope: r.string()?,
                        domain: r.string()?,
                        lineage_root: r.string()?,
                    },
                    operation_id: r.string()?,
                    record_id: hash_in(&mut r)?,
                },
            })
        } else {
            None
        };
        let source_iz = if matches!(
            version,
            PREPARED_IZ_ORIGIN_VERSION | PREPARED_IZ_CONTINUITY_VERSION
        ) {
            let context = crate::pd_memory::iz::decode_context(&r.bytes()?).map_err(|_| {
                crate::WireError::InvalidEnum {
                    field: "PD_IZ_CONTEXT",
                    actual: 0,
                }
            })?;
            let cognitive_session_id = r.string()?;
            let prior_lgc_head = hash_in(&mut r)?;
            let current_input_sha256 = hash_in(&mut r)?;
            let passed = crate::pd_memory::iz::decode_passed(&r.bytes()?).map_err(|_| {
                crate::WireError::InvalidEnum {
                    field: "PD_IZ_PASSED",
                    actual: 0,
                }
            })?;
            Some(PdIzOriginLink {
                context,
                cognitive_session_id,
                prior_lgc_head,
                current_input_sha256,
                passed,
            })
        } else {
            None
        };
        r.finish()?;
        Ok(PdPreparedEnvelope {
            version,
            source_request_sha256,
            checkpoint,
            continuity,
            source_dialogue,
            source_iz,
            failure_witness: None,
            pending: PdPendingData {
                session_id,
                parent_i0,
                turn_id,
                prepared_question,
                response_type,
                prepared_at,
                spine,
                spine_request_state,
                stimulus_contract_version,
                increment_contract_version,
                increment_stimulus_digest,
                pre_seal,
            },
        })
    })()
    .map_err(|e| format!("PD_PREPARED_CODEC: {e:?}"))?;
    parsed.validate()?;
    need(encode_prepared(&parsed)? == bytes, "NON_CANONICAL")?;
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn short_oversized_and_corrupted_bytes_are_rejected_without_interpretation() {
        for bytes in [vec![], vec![0; 73], vec![0; MAX_PREPARED_BYTES + 1]] {
            assert!(decode_prepared(&bytes).is_err());
        }
    }

    fn legacy_prepared_fixtures() -> [(u16,&'static str,&'static str);2] {
        [
            (1u16,"455353504450303101000707070707070707070707070707070707070707070707070707070707070707e40200004553535044434b3101000000010000000102000000a00f0000b80b0000b80b0000a8160000020000007331010200000000020000010000000c000000616336636561313137656566000000000c000000373636366334383735353138000000003100000057686963682065766964656e63652065737461626c697368657320746865206d6561737572656420726f746174696f6e3f31000000102700002c0c0000050000000b00000065737461626c69736865730800000065766964656e6365080000006d6561737572656408000000726f746174696f6e05000000776869636800000000000000000a0000000000000008480000004b616d206d6a61667475657368c3ab6d20696e646963696520e2809420706f2065207a676a6964682074616e692c20732774c3ab206c6f6468206dc3ab206d6520707965746a652e0002000c000000613536656233613263613063020000007331010000000c000000373636366334383735353138000000003100000057686963682065766964656e63652065737461626c697368657320746865206d6561737572656420726f746174696f6e3f31000000102700002c0c0000050000000b00000065737461626c69736865730800000065766964656e6365080000006d6561737572656408000000726f746174696f6e05000000776869636800000000000000000a0000000000000001000000010b00000065737461626c6973686573050000000b00000065737461626c6973686573010000000800000065766964656e636501000000080000006d656173757265640100000008000000726f746174696f6e0100000005000000776869636801000000010000002c0c000001000000102700001027000010270000000000000000000007000000736369656e63650a000000000000000a00000000000000000007000000736369656e63650a000000000000000a00000000000000010000000aad73dd15e4253ed6c4de43e1d5a5ed6fd095f67cbe3ea79e02bfe9cbd9b7a30200000073311300000069302d303030303030303030303030303061310c000000616336636561313137656566480000004b616d206d6a61667475657368c3ab6d20696e646963696520e2809420706f2065207a676a6964682074616e692c20732774c3ab206c6f6468206dc3ab206d6520707965746a652e080a000000000000000301060100010601004af90f62a099bafbff3489b4862b59730200000073311300000069302d303030303030303030303030303061310c0000003431646237633833623065330a0000000000000037ef22becbcd8afdc7f1ab6b9b94c1180764190000dd14abf249730e816e7cd8e270143add1be77aa8aadac30818c8e986f507b5fd1d9e28ff49d8b0b14188b9b5523d326c084152e3137af75a3a5ac845a8dc6ac60006010037ef22becbcd8afd49d178b507ffd8083ddc6d5b357dd38fb9782a2c489f3f8a114043bb16f117de0a000000000000000106010049d178b507ffd808c2486094e20935b9d24b99fc32ba09093ddc6d5b357dd38f4af90f62a099bafb0a000000000000000001000000b076e84e4b86dd71a6148997cc5132c6870d80b77a07d9830a00000000000000011f0b3bfdf551e615020200a6148997cc5132c60000000000000000102700002c0c0000073f6feedf2d3239631f0b3bfdf551e6153f6feedf2d323963f158aabdc67eb1906f5ac3864cfa737c5dc14b9f24ec23105338cb62cd5bcc00070137ef22becbcd8afd3f6feedf2d3239635338cb62cd5bcc0000000000000000000e000000756e6974206c61796572206f6e65fa6b9ecb33531cd14a81dff4c05e0ca4010237ef22becbcd8afd3f6feedf2d3239635338cb62cd5bcc004a81dff4c05e0ca40e000000756e6974206c617965722074776fa067b1f8339a25206ac1cb32d372b62f010437ef22becbcd8afd3f6feedf2d3239635338cb62cd5bcc006ac1cb32d372b62f10000000756e6974206c6179657220746872656582a621bbdc945d4950075b4db125c39501110000000000000064190000010a00000000000000c7f1ab6b9b94c118cc89fc2681802ec5a472f1afddbfcde68d08e40a1dc281f0a891770b9fc89cbd","7dfa3af86eeb5a9e3f60ce6f1fb9e3e5124128c56543d250e4774b08e1ca1e41"),
            (2u16,"455353504450303102000707070707070707070707070707070707070707070707070707070707070707af0500004553535044434b3101000000010000000102000000a00f0000b80b0000b80b0000a8160000020000007331010200000000020000020000000c000000616336636561313137656566000000000c000000373636366334383735353138000000003100000057686963682065766964656e63652065737461626c697368657320746865206d6561737572656420726f746174696f6e3f31000000102700002c0c0000050000000b00000065737461626c69736865730800000065766964656e6365080000006d6561737572656408000000726f746174696f6e05000000776869636800000000000000000a0000000000000008480000004b616d206d6a61667475657368c3ab6d20696e646963696520e2809420706f2065207a676a6964682074616e692c20732774c3ab206c6f6468206dc3ab206d6520707965746a652e0002000c000000633733303961653962363438010000000c00000063336262303539383632396601000000d8000000726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20d8000000a0010000401f00000100000008000000726f746174696f6e000000000100000008000000726f746174696f6e1400000000000000074e000000506f206b7468656865736820746520c2ab726f746174696f6ec2bb20e28094206dc3ab206a6570206e6ac3ab20766574692061207368656d62756c6c204b4f4e4b5245542070c3ab722074c3ab2e0002000c000000613536656233613263613063020000007331020000000c000000373636366334383735353138000000003100000057686963682065766964656e63652065737461626c697368657320746865206d6561737572656420726f746174696f6e3f31000000102700002c0c0000050000000b00000065737461626c69736865730800000065766964656e6365080000006d6561737572656408000000726f746174696f6e05000000776869636800000000000000000a000000000000000c00000063336262303539383632396601000000d8000000726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20726f746174696f6e20d8000000a0010000401f00000100000008000000726f746174696f6e000000000100000008000000726f746174696f6e1400000000000000020000000108000000726f746174696f6e050000000b00000065737461626c6973686573010000000800000065766964656e636501000000080000006d656173757265640100000008000000726f746174696f6e0200000005000000776869636801000000020000002c0c0000401f00000200000010270000a001000058140000102700000100000008000000726f746174696f6e0000000007000000736369656e63650a000000000000001400000000000000000007000000736369656e63650a000000000000001400000000000000020000007367f259955a931719208dc1cb57e426e6e9e8da3447e37e592d6d9f8c00d1f609000000617474656d70742d321300000069302d303030303030303030303030303062320c0000006337333039616539623634384e000000506f206b7468656865736820746520c2ab726f746174696f6ec2bb20e28094206dc3ab206a6570206e6ac3ab20766574692061207368656d62756c6c204b4f4e4b5245542070c3ab722074c3ab2e0714000000000000000301060100010601007a99c5cd47a8109aff3489b4862b597309000000617474656d70742d321300000069302d303030303030303030303030303062320c0000003738333066373365313933371400000000000000a693fab775d5afd08ad9a8d11e42a91907641900001b6fc6b058f223f3a1c2ff6a9258a234f344966b035dca83ae1649279143d30723a99f34768523ab9c81b5afee00b2723fdc1a576b0591d6429f17eae352977800060100a693fab775d5afd0d1eb032df8cd282b1b976b5b3574d08f428bbd12db43739698328ed4afc18be2140000000000000001060100d1eb032df8cd282b4aa54a9ccba2f219250409667ff2ee2b1b976b5b3574d08f7a99c5cd47a8109a14000000000000000001000000209a3417125f082c05c710c9ff9ce875c80dc4c76d2d7c8e140000000000000001f5f8bc3cb835f7b302020005c710c9ff9ce8750000000000000000a0010000401f000007ca0908bba05c2706f5f8bc3cb835f7b3ca0908bba05c2706f158aabdc67eb1906f5ac3864cfa737c5dc14b9f24ec2310c39738eba913d6220701a693fab775d5afd0ca0908bba05c2706c39738eba913d62200000000000000000e000000756e6974206c61796572206f6e65fa6b9ecb33531cd16e51d9197243ae0e0102a693fab775d5afd0ca0908bba05c2706c39738eba913d6226e51d9197243ae0e0e000000756e6974206c617965722074776fa067b1f8339a2520227059bcc6d4eb250104a693fab775d5afd0ca0908bba05c2706c39738eba913d622227059bcc6d4eb2510000000756e6974206c6179657220746872656582a621bbdc945d49e394406f2396fbb5011100000000000000641900000114000000000000008ad9a8d11e42a91902000000733107000000736369656e6365e37d7455e3319f9a7a31a825ab5fa9b94da751030d9b7912667419755b8d00e7070707070707070707070707070707070707070707070707070707070707070709090909090909090909090909090909090909090909090909090909090909098ff4be682d7a83ede903802996b283f6766c949777037de2f928589573b20eec1300000069302d323232613036613436353838626636377c375bb8da8cc780feca9622f919e9d4cb4b6c46b58d1d0fea3676787b41f3ce","d4270c0733f63e099ada2858b2a41d80cd710e5eafe09f065830504f8a38bfb9"),
        ]
    }

    #[test]
    fn sealed_legacy_prepared_v1_v2_reencode_without_new_iz_bytes() {
        // Historical explicit_continuity_01 synthetic formula/storage fixtures.
        // Real PD checkpoint/pending; synthetic layers/receipts, not native GCL admission.
        for (version,encoded,digest) in legacy_prepared_fixtures() {
            let bytes=encoded.as_bytes().chunks_exact(2)
                .map(|p|u8::from_str_radix(std::str::from_utf8(p).unwrap(),16).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(hex(&sha(&bytes)),digest);
            let env=decode_prepared(&bytes).unwrap();
            assert_eq!(env.version,version);
            assert!(env.source_iz.is_none());
            assert_eq!(encode_prepared(&env).unwrap(),bytes);
            let mut invalid=env.clone();invalid.version=PREPARED_IZ_ORIGIN_VERSION;
            assert!(invalid.validate().is_err()); // v5 cannot invent an origin link
        }
    }

    fn prepared_fixture(encoded:&str)->PdPreparedEnvelope {
        let bytes=encoded.as_bytes().chunks_exact(2)
            .map(|p|u8::from_str_radix(std::str::from_utf8(p).unwrap(),16).unwrap()).collect::<Vec<_>>();
        decode_prepared(&bytes).unwrap()
    }
    fn formula_response(env:&PdPreparedEnvelope,verdict:u8)->crate::ShadowCycleResponse {
        // Only a receipt/formula fixture. This does not establish native admission.
        let p=&env.pending;
        let trust=[0x42;32]; let intensity=6250u32; let kind=if verdict==1 {1u8} else {2u8};
        let trust_digest=digest_parts(&[&trust,&intensity.to_le_bytes(),&[kind],&500u32.to_le_bytes(),b"GCL_LIVING_TRUST_TO_IZ_V164"]);
        let mut receipt=crate::VerificationReceiptWire {
            session_id:p.session_id.clone(),parent_i0:p.parent_i0.clone(),
            primitive_anchor:u64::from_str_radix(p.parent_i0.strip_prefix("i0-").unwrap(),16).unwrap(),
            xy_digest:23,pd_binding_digest:p.binding_digest().unwrap(),
            pd_continuum_activation_digest:p.spine.continuum.activation_digest,living_trust_digest:trust_digest,
            living_trust_sha256:hex(&trust),y_verdict:verdict,x_verdict:verdict,
            seal:crate::fnv1a64(b"SHADOW_VERIFICATION_OUTPUT") ^ crate::fnv1a64(b"GJATA_LEGACY"),
            generation:7,receipt_id:String::new(),
        };
        receipt.receipt_id=source_verification_receipt(&receipt,&trust);
        crate::ShadowCycleResponse {
            session_id:p.session_id.clone(),negative_persisted:verdict==0,verification_receipt:receipt,
            verdict:crate::ShadowVerdictWire {
                verified:verdict,primitive:verdict,knowledge_band:if verdict==1 {2}else{0},lgc_law:500,
                legacy_score:0.7321,lgc_dsl:"SYNTHETIC FORMULA DATA".into(),living_trust_action_state:1,
                living_trust_action_mask:1,living_trust_required_action_mask:1,living_trust_system_laws_seal:1,
                living_trust_sovereign_flags:1,living_trust_sovereign_value:500,living_trust_sha256:hex(&trust),
                living_trust_intensity:intensity,living_trust_kind:kind,project_id:0,project_status:0,
                project_context_digest:0,project_evidence_digest:0,project_context_sha256:"0".repeat(64),
                project_evidence_sha256:"0".repeat(64),novel_factualized:false,novel_trl_level:0,
                novel_proof_score:0,novel_rejection:0,
            },
        }
    }
    #[test]
    fn prerequisite_validation_does_not_form_text_and_finalization_keeps_legacy_bytes() {
        for (_,encoded,_) in legacy_prepared_fixtures() {
            let envelope=prepared_fixture(encoded);
            let original=encode_prepared(&envelope).unwrap();
            for pair in [0,1] {
                let response=formula_response(&envelope,pair);
                CONTINUATION_TEXT_FORMATIONS.with(|count|count.set(0));
                validate_continuation_prerequisites(&envelope,&response).unwrap();
                CONTINUATION_TEXT_FORMATIONS.with(|count|assert_eq!(count.get(),0));
                let data=derive_continuation(&envelope,&response).unwrap();
                CONTINUATION_TEXT_FORMATIONS.with(|count|assert_eq!(count.get(),1));
                let expected=if pair==1 {envelope.pending.prepared_question.clone()} else {
                    format!("Cila provë konkrete duhet të zëvendësojë bazën e rrëzuar: {}",envelope.pending.prepared_question)
                };
                assert_eq!(data.question.as_bytes(),expected.as_bytes());
                assert_eq!(data.y_verdict,pair); assert_eq!(data.x_verdict,pair);
                let bytes=encode_continuation(&data).unwrap();
                assert_eq!(encode_continuation(&decode_continuation(&bytes).unwrap()).unwrap(),bytes);
                assert_eq!(encode_prepared(&envelope).unwrap(),original);
                let mut changed=data;
                changed.question.push('?');
                assert!(changed.validate().is_err(),"post-T1 output/iZ formulas still bind exact text");
            }
        }
    }
    #[test]
    fn prerequisite_gate_rejects_receipt_scope_trust_and_persistence_defects_before_text() {
        let envelope=prepared_fixture(legacy_prepared_fixtures()[0].1);
        let valid=formula_response(&envelope,0);
        for mutation in 0..15 {
            let mut response=valid.clone();
            match mutation {
                0=>response.session_id.push('x'),
                1=>{response.verification_receipt.x_verdict=1;response.verdict.primitive=1;},
                2=>response.negative_persisted=false,
                3=>response.verdict.living_trust_sha256="0".repeat(64),
                4=>response.verdict.living_trust_kind=1,
                5=>response.verdict.living_trust_sovereign_value=499,
                6=>response.verification_receipt.pd_binding_digest^=1,
                7=>response.verification_receipt.pd_continuum_activation_digest^=1,
                8=>response.verification_receipt.living_trust_digest^=1,
                9=>response.verification_receipt.receipt_id="0".repeat(64),
                10=>response.verification_receipt.seal=0,
                11=>response.verification_receipt.generation=0,
                12=>response.verification_receipt.primitive_anchor=0,
                13=>response.verification_receipt.xy_digest=0,
                14=>response.verdict.living_trust_intensity=10001,
                _=>unreachable!(),
            }
            // Keep the receipt checksum consistent for these malformed scalar
            // fields, so the actual scalar guards must reject them as well.
            if (10..=13).contains(&mutation) {
                response.verification_receipt.receipt_id=source_verification_receipt(&response.verification_receipt,&[0x42;32]);
            }
            CONTINUATION_TEXT_FORMATIONS.with(|count|count.set(0));
            let before=validate_continuation_prerequisites(&envelope,&response).unwrap_err();
            assert_eq!(derive_continuation(&envelope,&response).unwrap_err(),before,"mutation={mutation}");
            CONTINUATION_TEXT_FORMATIONS.with(|count|assert_eq!(count.get(),0,"mutation={mutation}"));
        }
        // A constructive receipt may still accompany a real experimental N.
        let mut constructive=formula_response(&envelope,1);
        constructive.negative_persisted=true;
        validate_continuation_prerequisites(&envelope,&constructive).unwrap();
        assert_eq!(derive_continuation(&envelope,&constructive).unwrap().basis,1);
    }
    #[test]
    fn all_response_tags_are_explicit_and_unknown_tags_fail() {
        for tag in 0..=8 {
            assert!(!response_label(tag).unwrap().is_empty());
        }
        assert!(response_label(9).is_err());
        assert!(response_label(255).is_err());
    }
}

// V189_DERIVATIVE. Pure rederivation of source finalization data.
// This must consume a response from an admitted, durable T1 owned by Shadow.
// A caller-supplied internally consistent tuple is NOT proof of admission.
pub const CONTINUATION_VERSION: u16 = 1;
pub const MAX_CONTINUATION_BYTES: usize = 128 * 1024;
const CONTINUATION_MAGIC: &[u8; 8] = b"ESSPDC01";
const CONTINUUM_VERSION: u32 = 0x0001_0601;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuationData {
    pub version: u16,
    pub prepared_sha256: Hash,
    pub source_request_sha256: Hash,
    pub shadow_response_sha256: Hash,
    pub session_id: String,
    pub parent_i0: String,
    pub turn_id: String,
    pub question: String,
    pub response_type: u8,
    pub basis: u8,
    pub mpro_mass: u32,
    pub y_verdict: u8,
    pub x_verdict: u8,
    pub verification_seal: u64,
    pub verification_generation: u64,
    pub verification_receipt_id: String,
    pub primitive_anchor: u64,
    pub xy_digest: u64,
    pub pd_binding_digest: u64,
    pub continuum_activation_digest: u64,
    pub question_increment_digest: u64,
    pub pd_output_id: String,
    pub pd_output_digest: u64,
    pub iz_id: String,
    pub iz_digest: u64,
    pub iz_sha256: Hash,
    pub next_i0_id: String,
    pub living_trust_sha256: Hash,
    pub living_trust_intensity: u32,
    pub living_trust_kind: u8,
    pub living_trust_sovereign_value: u32,
    pub living_trust_digest: u64,
    pub continuum_completion_digest: u64,
}
fn basis_label(basis: u8) -> Result<&'static str, String> {
    match basis {
        1 => Ok("VERIFIED_POSITIVE"),
        2 => Ok("VERIFIED_NEGATIVE_REBUILD"),
        _ => Err("PD_CONTINUATION_BASIS".into()),
    }
}
fn parse_identity(text: &str) -> Result<Hash, String> {
    need(
        text.len() == 64 && text.as_bytes().iter().all(u8::is_ascii_hexdigit),
        "TRUST_HEX",
    )?;
    let mut result = [0; 32];
    for (i, slot) in result.iter_mut().enumerate() {
        *slot = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16)
            .map_err(|_| "PD_CONTINUATION_TRUST_HEX")?;
    }
    Ok(result)
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|v| format!("{v:02x}")).collect()
}
/// Exact source lab_contracts/verification_receipt.rs:37-72.
fn source_verification_receipt(r: &crate::VerificationReceiptWire, trust: &Hash) -> String {
    let mut b = b"ESS_MAI_VERIFICATION_RECEIPT_SHA256_V162".to_vec();
    b.extend_from_slice(&0x0001_0602u32.to_le_bytes());
    for text in [&r.session_id, &r.parent_i0] {
        b.extend_from_slice(&(text.len() as u64).to_le_bytes());
        b.extend_from_slice(text.as_bytes());
    }
    for v in [
        r.primitive_anchor,
        r.xy_digest,
        r.pd_binding_digest,
        r.pd_continuum_activation_digest,
    ] {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b.extend_from_slice(trust);
    b.push(r.y_verdict);
    b.push(r.x_verdict);
    b.extend_from_slice(&r.generation.to_le_bytes());
    b.extend_from_slice(&r.seal.to_le_bytes());
    hex(&sha(&b))
}
impl ContinuationData {
    /// Shape and source-formula consistency only; does not establish T1 authority.
    pub fn validate(&self) -> Result<(), String> {
        need(self.version == CONTINUATION_VERSION, "CONTINUATION_VERSION")?;
        need(
            self.prepared_sha256 != [0; 32]
                && self.source_request_sha256 != [0; 32]
                && self.shadow_response_sha256 != [0; 32],
            "CONTINUATION_BINDINGS",
        )?;
        for v in [
            &self.session_id,
            &self.parent_i0,
            &self.turn_id,
            &self.pd_output_id,
            &self.iz_id,
            &self.next_i0_id,
        ] {
            text(v, 256)?;
        }
        text(&self.question, MAX_TEXT + 128)?;
        response_label(self.response_type)?;
        let label = basis_label(self.basis)?;
        need(
            (self.basis, self.y_verdict, self.x_verdict) == (1, 1, 1)
                || (self.basis, self.y_verdict, self.x_verdict) == (2, 0, 0),
            "CONTINUATION_PAIR",
        )?;
        need(
            self.mpro_mass <= 10_000
                && self.living_trust_intensity <= 10_000
                && self.living_trust_kind == self.basis
                && self.living_trust_sovereign_value == 500
                && self.living_trust_sha256 != [0; 32],
            "CONTINUATION_TRUST",
        )?;
        need(
            self.verification_seal
                == (crate::fnv1a64(b"SHADOW_VERIFICATION_OUTPUT")
                    ^ crate::fnv1a64(b"GJATA_LEGACY"))
                && self.verification_generation != 0
                && self.primitive_anchor != 0
                && self.xy_digest != 0,
            "CONTINUATION_RECEIPT_SEAL",
        )?;
        need(
            self.verification_receipt_id.len() == 64
                && self
                    .verification_receipt_id
                    .bytes()
                    .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v)),
            "CONTINUATION_RECEIPT_HEX",
        )?;
        for v in [
            self.pd_binding_digest,
            self.continuum_activation_digest,
            self.question_increment_digest,
            self.pd_output_digest,
            self.iz_digest,
            self.living_trust_digest,
        ] {
            need(v != 0, "CONTINUATION_ZERO")?;
        }
        let trust = digest_parts(&[
            &self.living_trust_sha256,
            &self.living_trust_intensity.to_le_bytes(),
            &[self.living_trust_kind],
            &self.living_trust_sovereign_value.to_le_bytes(),
            b"GCL_LIVING_TRUST_TO_IZ_V164",
        ]);
        need(
            self.living_trust_digest == trust,
            "CONTINUATION_TRUST_DIGEST",
        )?;
        let out = digest_parts(&[
            &CONTINUUM_VERSION.to_le_bytes(),
            &self.continuum_activation_digest.to_le_bytes(),
            &self.pd_binding_digest.to_le_bytes(),
            &self.question_increment_digest.to_le_bytes(),
            self.verification_receipt_id.as_bytes(),
            self.question.as_bytes(),
            &[self.y_verdict],
            &[self.x_verdict],
            label.as_bytes(),
            b"PD_VERIFIED_OUTPUT",
        ]);
        let iz = digest_parts(&[
            &CONTINUUM_VERSION.to_le_bytes(),
            &self.continuum_activation_digest.to_le_bytes(),
            &self.question_increment_digest.to_le_bytes(),
            &out.to_le_bytes(),
            &trust.to_le_bytes(),
            self.verification_receipt_id.as_bytes(),
            label.as_bytes(),
            b"PD_COMPLETES_IZ_WITH_LIVING_TRUST_V161",
        ]);
        let next = format!(
            "i0-{:016x}",
            digest_parts(&[
                &CONTINUUM_VERSION.to_le_bytes(),
                &out.to_le_bytes(),
                &iz.to_le_bytes(),
                b"PD_OUTPUT_PLUS_IZ_TO_NEXT_I0"
            ])
        );
        let complete = digest_parts(&[
            &CONTINUUM_VERSION.to_le_bytes(),
            &self.continuum_activation_digest.to_le_bytes(),
            &self.question_increment_digest.to_le_bytes(),
            &out.to_le_bytes(),
            &iz.to_le_bytes(),
            &trust.to_le_bytes(),
            next.as_bytes(),
            self.verification_receipt_id.as_bytes(),
            label.as_bytes(),
            b"PD_CONTINUUM_COMPLETE",
        ]);
        need(
            self.pd_output_digest == out
                && self.pd_output_id == format!("pdo-{out:016x}")
                && self.iz_digest == iz
                && self.iz_id == format!("iZ-{iz:016x}")
                && self.next_i0_id == next
                && self.continuum_completion_digest == complete,
            "CONTINUATION_FORMULA",
        )?;
        let receipt = crate::VerificationReceiptWire {
            session_id: self.session_id.clone(),
            parent_i0: self.parent_i0.clone(),
            primitive_anchor: self.primitive_anchor,
            xy_digest: self.xy_digest,
            pd_binding_digest: self.pd_binding_digest,
            pd_continuum_activation_digest: self.continuum_activation_digest,
            living_trust_digest: self.living_trust_digest,
            living_trust_sha256: hex(&self.living_trust_sha256),
            y_verdict: self.y_verdict,
            x_verdict: self.x_verdict,
            seal: self.verification_seal,
            generation: self.verification_generation,
            receipt_id: self.verification_receipt_id.clone(),
        };
        need(
            source_verification_receipt(&receipt, &self.living_trust_sha256)
                == self.verification_receipt_id,
            "CONTINUATION_RECEIPT",
        )?;
        Ok(())
    }
    /// Re-derives every field including iZ SHA and both full data bindings.
    pub fn validate_for(
        &self,
        envelope: &PdPreparedEnvelope,
        response: &crate::ShadowCycleResponse,
    ) -> Result<(), String> {
        self.validate()?;
        need(
            *self == derive_continuation(envelope, response)?,
            "CONTINUATION_T1_MISMATCH",
        )
    }
}
/// Source runtime.rs:147-267 + 402-526 and pd_continuum_contract.rs:240-324.
/// This does not call an authority, persist knowledge, or create a native token.
/// Render data only. Durable continuation callers must first validate the
/// receipt via derive_continuation; this function grants no authority.
pub fn continuation_question(envelope: &PdPreparedEnvelope, response: &crate::ShadowCycleResponse) -> Result<String, String> {
    #[cfg(test)]
    CONTINUATION_TEXT_FORMATIONS.with(|count|count.set(count.get()+1));
    envelope.validate()?;
    let r = &response.verification_receipt;
    need(response.session_id == envelope.pending.session_id
        && r.session_id == envelope.pending.session_id
        && r.parent_i0 == envelope.pending.parent_i0, "QUESTION_SCOPE")?;
    match (r.y_verdict, r.x_verdict) {
        // The durable V5 policy is validated by the owner before continuation.
        // This selects output data only; it cannot grant a receipt or a write.
        (1,1) if response.verdict.lgc_dsl.contains("|MODEL_OUTPUT_V1|MODEL_JUDGMENT_V5=") =>
            envelope.failure_witness.as_ref().ok_or("PD_PREPARED_MODEL_OUTPUT_WITNESS")?
                .verified_model_question(),
        (1,1) => Ok(envelope.pending.prepared_question.clone()),
        (0,0) => match &envelope.failure_witness {
            Some(witness) => witness.negative_question(),
            None => Ok(format!("Cila provë konkrete duhet të zëvendësojë bazën e rrëzuar: {}", envelope.pending.prepared_question)),
        },
        _ => Err("PD_CONTINUATION_MIXED_VERDICT".into()),
    }
}
struct ContinuationPrerequisites {
    basis:u8,
    trust:Hash,
    trust_digest:u64,
    binding:u64,
    activation:u64,
    increment:u64,
}
fn continuation_prerequisites(
    envelope: &PdPreparedEnvelope,
    response: &crate::ShadowCycleResponse,
) -> Result<ContinuationPrerequisites, String> {
    envelope.validate()?;
    let p = &envelope.pending;
    let r = &response.verification_receipt;
    let v = &response.verdict;
    need(
        response.session_id == p.session_id
            && r.session_id == p.session_id
            && r.parent_i0 == p.parent_i0,
        "CONTINUATION_SCOPE",
    )?;
    need(
        v.verified == r.y_verdict && v.primitive == r.x_verdict,
        "CONTINUATION_VERDICT_BINDING",
    )?;
    let basis = match (r.y_verdict, r.x_verdict) {
        (1, 1) => 1,
        (0, 0) => 2,
        _ => return Err("PD_CONTINUATION_MIXED_VERDICT".into()),
    };
    need(
        // Source Shadow also stages a validated experimental negative beside
        // a constructive verdict (shadow_gateway::stage_parent_negative).
        // The durable snapshot independently requires an actual Negative write.
        basis != 2 || response.negative_persisted,
        "CONTINUATION_NEGATIVE_PERSISTENCE",
    )?;
    let trust = parse_identity(&r.living_trust_sha256)?;
    need(
        trust == parse_identity(&v.living_trust_sha256)? && trust != [0; 32],
        "CONTINUATION_TRUST_BINDING",
    )?;
    need(
        v.living_trust_intensity <= 10_000
            && v.living_trust_kind == basis
            && v.living_trust_sovereign_value == 500,
        "CONTINUATION_TRUST_FIELDS",
    )?;
    let trust_digest = digest_parts(&[
        &trust,
        &v.living_trust_intensity.to_le_bytes(),
        &[v.living_trust_kind],
        &v.living_trust_sovereign_value.to_le_bytes(),
        b"GCL_LIVING_TRUST_TO_IZ_V164",
    ]);
    let binding = p.binding_digest()?;
    let activation = p.spine.continuum.activation_digest;
    let increment = p.spine.continuum.increment_digest;
    need(
        r.pd_binding_digest == binding
            && binding != 0
            && r.pd_continuum_activation_digest == activation
            && activation != 0
            && r.living_trust_digest == trust_digest,
        "CONTINUATION_PENDING_BINDING",
    )?;
    need(
        r.receipt_id == source_verification_receipt(r, &trust),
        "CONTINUATION_RECEIPT",
    )?;
    need(
        p.spine.mpro_mass <= 10_000
            && r.seal == (crate::fnv1a64(b"SHADOW_VERIFICATION_OUTPUT") ^ crate::fnv1a64(b"GJATA_LEGACY"))
            && r.generation != 0 && r.primitive_anchor != 0 && r.xy_digest != 0,
        "CONTINUATION_RECEIPT_SEAL",
    )?;
    need(increment != 0 && trust_digest != 0, "CONTINUATION_ZERO")?;
    need(
        sha(&encode_prepared(envelope)?) != [0;32]
            && envelope.source_request_sha256 != [0;32]
            && sha(&crate::encode_cycle_response(response)) != [0;32],
        "CONTINUATION_BINDINGS",
    )?;
    // No question text is created here. The pending text is already bounded.
    // Legacy witness reason expansion is <=6*8192 bytes plus <4096 fixed text;
    // current witnesses independently validate their compositional byte bound.
    if basis == 2 && envelope.failure_witness.is_none() {
        need("Cila provë konkrete duhet të zëvendësojë bazën e rrëzuar: ".len()
            + p.prepared_question.len() <= MAX_TEXT + 128, "TEXT")?;
    }
    Ok(ContinuationPrerequisites { basis, trust, trust_digest, binding, activation, increment })
}

/// Validate receipt, persistence claim, scope, trust, pending data and output
/// capacity without forming the final question or its output/iZ digests.
/// The owner still establishes actual T1 persistence. Output-dependent digest
/// checks remain in derive_continuation after T1 and before T2/delivery.
pub fn validate_continuation_prerequisites(
    envelope: &PdPreparedEnvelope,
    response: &crate::ShadowCycleResponse,
) -> Result<(), String> {
    continuation_prerequisites(envelope,response).map(|_|())
}

pub fn derive_continuation(
    envelope: &PdPreparedEnvelope,
    response: &crate::ShadowCycleResponse,
) -> Result<ContinuationData, String> {
    let ContinuationPrerequisites { basis,trust,trust_digest,binding,activation,increment }
        = continuation_prerequisites(envelope,response)?;
    let p=&envelope.pending;
    let r=&response.verification_receipt;
    let v=&response.verdict;
    let question = continuation_question(envelope, response)?;
    let label = basis_label(basis)?;
    let output = digest_parts(&[
        &CONTINUUM_VERSION.to_le_bytes(),
        &activation.to_le_bytes(),
        &binding.to_le_bytes(),
        &increment.to_le_bytes(),
        r.receipt_id.as_bytes(),
        question.as_bytes(),
        &[r.y_verdict],
        &[r.x_verdict],
        label.as_bytes(),
        b"PD_VERIFIED_OUTPUT",
    ]);
    let iz = digest_parts(&[
        &CONTINUUM_VERSION.to_le_bytes(),
        &activation.to_le_bytes(),
        &increment.to_le_bytes(),
        &output.to_le_bytes(),
        &trust_digest.to_le_bytes(),
        r.receipt_id.as_bytes(),
        label.as_bytes(),
        b"PD_COMPLETES_IZ_WITH_LIVING_TRUST_V161",
    ]);
    let next = format!(
        "i0-{:016x}",
        digest_parts(&[
            &CONTINUUM_VERSION.to_le_bytes(),
            &output.to_le_bytes(),
            &iz.to_le_bytes(),
            b"PD_OUTPUT_PLUS_IZ_TO_NEXT_I0"
        ])
    );
    let complete = digest_parts(&[
        &CONTINUUM_VERSION.to_le_bytes(),
        &activation.to_le_bytes(),
        &increment.to_le_bytes(),
        &output.to_le_bytes(),
        &iz.to_le_bytes(),
        &trust_digest.to_le_bytes(),
        next.as_bytes(),
        r.receipt_id.as_bytes(),
        label.as_bytes(),
        b"PD_CONTINUUM_COMPLETE",
    ]);
    let mut material = b"ESS_MAI_IZ_V157".to_vec();
    for x in [activation, increment, output] {
        material.extend_from_slice(&x.to_le_bytes());
    }
    material.extend_from_slice(r.receipt_id.as_bytes());
    material.extend_from_slice(&trust);
    material.extend_from_slice(&v.living_trust_intensity.to_le_bytes());
    material.push(v.living_trust_kind);
    material.extend_from_slice(&v.living_trust_sovereign_value.to_le_bytes());
    material.extend_from_slice(&trust_digest.to_le_bytes());
    material.extend_from_slice(&p.pre_seal.action_sha256);
    material.extend_from_slice(&p.pre_seal.law_trace_sha256);
    let data = ContinuationData {
        version: CONTINUATION_VERSION,
        prepared_sha256: sha(&encode_prepared(envelope)?),
        source_request_sha256: envelope.source_request_sha256,
        shadow_response_sha256: sha(&crate::encode_cycle_response(response)),
        session_id: p.session_id.clone(),
        parent_i0: p.parent_i0.clone(),
        turn_id: p.turn_id.clone(),
        question,
        response_type: p.response_type,
        basis,
        mpro_mass: p.spine.mpro_mass,
        y_verdict: r.y_verdict,
        x_verdict: r.x_verdict,
        verification_seal: r.seal,
        verification_generation: r.generation,
        verification_receipt_id: r.receipt_id.clone(),
        primitive_anchor: r.primitive_anchor,
        xy_digest: r.xy_digest,
        pd_binding_digest: binding,
        continuum_activation_digest: activation,
        question_increment_digest: increment,
        pd_output_id: format!("pdo-{output:016x}"),
        pd_output_digest: output,
        iz_id: format!("iZ-{iz:016x}"),
        iz_digest: iz,
        iz_sha256: sha(&material),
        next_i0_id: next,
        living_trust_sha256: trust,
        living_trust_intensity: v.living_trust_intensity,
        living_trust_kind: v.living_trust_kind,
        living_trust_sovereign_value: v.living_trust_sovereign_value,
        living_trust_digest: trust_digest,
        continuum_completion_digest: complete,
    };
    data.validate()?;
    Ok(data)
}
/// Canonical bounded data framing. Decoding alone does not authorize use.
pub fn encode_continuation(data: &ContinuationData) -> Result<Vec<u8>, String> {
    data.validate()?;
    let mut w = Writer::new();
    w.bytes.extend_from_slice(CONTINUATION_MAGIC);
    w.u16(data.version);
    w.bytes.extend_from_slice(&data.prepared_sha256);
    w.bytes.extend_from_slice(&data.source_request_sha256);
    w.bytes.extend_from_slice(&data.shadow_response_sha256);
    w.string(&data.session_id);
    w.string(&data.parent_i0);
    w.string(&data.turn_id);
    w.string(&data.question);
    w.u8(data.response_type);
    w.u8(data.basis);
    w.u32(data.mpro_mass);
    w.u8(data.y_verdict);
    w.u8(data.x_verdict);
    w.u64(data.verification_seal);
    w.u64(data.verification_generation);
    w.string(&data.verification_receipt_id);
    w.u64(data.primitive_anchor);
    w.u64(data.xy_digest);
    w.u64(data.pd_binding_digest);
    w.u64(data.continuum_activation_digest);
    w.u64(data.question_increment_digest);
    w.string(&data.pd_output_id);
    w.u64(data.pd_output_digest);
    w.string(&data.iz_id);
    w.u64(data.iz_digest);
    w.bytes.extend_from_slice(&data.iz_sha256);
    w.string(&data.next_i0_id);
    w.bytes.extend_from_slice(&data.living_trust_sha256);
    w.u32(data.living_trust_intensity);
    w.u8(data.living_trust_kind);
    w.u32(data.living_trust_sovereign_value);
    w.u64(data.living_trust_digest);
    w.u64(data.continuum_completion_digest);
    need(
        w.bytes.len() <= MAX_CONTINUATION_BYTES - 32,
        "CONTINUATION_ENCODE_LIMIT",
    )?;
    let checksum = sha(&w.bytes);
    w.bytes.extend_from_slice(&checksum);
    Ok(w.bytes)
}
pub fn decode_continuation(bytes: &[u8]) -> Result<ContinuationData, String> {
    need(
        bytes.len() >= 42 && bytes.len() <= MAX_CONTINUATION_BYTES,
        "CONTINUATION_DECODE_LIMIT",
    )?;
    let (body, checksum) = bytes.split_at(bytes.len() - 32);
    need(sha(body).as_slice() == checksum, "CONTINUATION_CHECKSUM")?;
    let data = (|| -> Result<ContinuationData, crate::WireError> {
        let mut r = Reader::new(body);
        if r.take(8)? != CONTINUATION_MAGIC {
            return Err(crate::WireError::InvalidMagic);
        }
        let data = ContinuationData {
            version: r.u16()?,
            prepared_sha256: hash_in(&mut r)?,
            source_request_sha256: hash_in(&mut r)?,
            shadow_response_sha256: hash_in(&mut r)?,
            session_id: r.string()?,
            parent_i0: r.string()?,
            turn_id: r.string()?,
            question: r.string()?,
            response_type: r.u8()?,
            basis: r.u8()?,
            mpro_mass: r.u32()?,
            y_verdict: r.u8()?,
            x_verdict: r.u8()?,
            verification_seal: r.u64()?,
            verification_generation: r.u64()?,
            verification_receipt_id: r.string()?,
            primitive_anchor: r.u64()?,
            xy_digest: r.u64()?,
            pd_binding_digest: r.u64()?,
            continuum_activation_digest: r.u64()?,
            question_increment_digest: r.u64()?,
            pd_output_id: r.string()?,
            pd_output_digest: r.u64()?,
            iz_id: r.string()?,
            iz_digest: r.u64()?,
            iz_sha256: hash_in(&mut r)?,
            next_i0_id: r.string()?,
            living_trust_sha256: hash_in(&mut r)?,
            living_trust_intensity: r.u32()?,
            living_trust_kind: r.u8()?,
            living_trust_sovereign_value: r.u32()?,
            living_trust_digest: r.u64()?,
            continuum_completion_digest: r.u64()?,
        };
        r.finish()?;
        Ok(data)
    })()
    .map_err(|e| format!("PD_CONTINUATION_CODEC: {e:?}"))?;
    data.validate()?;
    need(
        encode_continuation(&data)? == bytes,
        "CONTINUATION_NON_CANONICAL",
    )?;
    Ok(data)
}
