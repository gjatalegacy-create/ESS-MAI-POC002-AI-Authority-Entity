//! PDS2: explicit science answer to an actual PIZ2 question.
//! One-way data bridge in the same Vault; no copied parent material, new PD,
//! evidence admission, or authority. PDS1/PDF1 remain byte-compatible.
use super::{iz, *};

pub const SCIENCE_CHECKPOINT_PREFIX_V3: &[u8] = b"PD_IZ_SCIENCE_USER_ONLY_V3\0";
pub fn source_is_science(bytes: &[u8]) -> bool {
    matches!(bytes.get(..4), Some(b"PDS2") | Some(b"PDS3") | Some(b"PDS4") | Some(b"PDS5"))
}
/// Native anchoring is preserved for both legacy and computed-output policy.
pub fn source_is_native_science(bytes: &[u8]) -> bool {
    matches!(bytes.get(..4), Some(b"PDS3") | Some(b"PDS5"))
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestionPolicy { Legacy, ComputedV1 }
impl IzScienceInput {
    pub fn checkpoint_prefix(&self) -> &'static [u8] {
        if self.context.version == iz::IZ_NATIVE_CONTINUITY_VERSION {
            SCIENCE_CHECKPOINT_PREFIX_V3
        } else {
            failure::FAILURE_CHECKPOINT_PREFIX
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IzScienceInput {
    pub question_policy: QuestionPolicy,
    pub context: iz::IzContext,
    pub parent_source_sha256: Hash,
    pub prior_lgc_head: Hash,
    pub science_query: Vec<u8>,
    /// User-proposed relevance, never an attestation.
    pub relevance_statement: String,
}
fn sha(bytes: &[u8]) -> Hash {
    Sha256::digest(bytes).into()
}
impl IzScienceInput {
    pub fn validate(&self) -> Result<(), String> {
        self.context.validate()?;
        let mapped = match self.context.version {
            iz::IZ_ACTUAL_INPUT_VERSION => self.context.map.is_some(),
            iz::IZ_NATIVE_CONTINUITY_VERSION => matches!(
                self.context.native_map.as_ref().map(|m| &m.checkpoint_base),
                Some(iz::IzCheckpointBase::Operational(_))
            ),
            _ => false,
        };
        if !mapped
            || self.context.parent.is_none()
            || self.parent_source_sha256 == [0; 32]
            || self.prior_lgc_head == [0; 32]
        {
            return Err("PDS2_EXACT_PIZ2_PARENT_REQUIRED".into());
        }
        if self.science_query.is_empty()
            || self.science_query.len() > linked_science::MAX_SCIENCE_QUERY_BYTES
        {
            return Err("PDS2_QUERY_LIMIT".into());
        }
        text_valid(&self.relevance_statement, MAX_TEXT_BYTES)?;
        Ok(())
    }
}
pub fn from_parent(
    mut context: iz::IzContext,
    parent: &PdOperationRecord,
    science_query: Vec<u8>,
    relevance_statement: String,
) -> Result<IzScienceInput, String> {
    let reference = iz::IzParent::from_record(parent)?;
    if context.version == iz::IZ_NATIVE_CONTINUITY_VERSION {
        let anchor = context
            .native_anchor
            .as_ref()
            .ok_or("PDS3_NATIVE_ANCHOR_REQUIRED")?;
        let mapped = iz::native_map_for(&context.identity, anchor, Some(parent))?;
        if context.native_map.as_ref().is_some_and(|m| m != &mapped) {
            return Err("PDS3_CONFLICTING_SUPPLIED_MAP".into());
        }
        context.native_map = Some(mapped);
    } else {
        let mapped = iz::IzMapSnapshot {
            identity_key: parent.request.identity.key()?,
            head: reference.clone(),
            exact_record: reference,
            status: iz::IzMaterialStatus::OperationalNoVerdict,
        };
        if context.map.as_ref().is_some_and(|m| m != &mapped) {
            return Err("PDS2_CONFLICTING_SUPPLIED_MAP".into());
        }
        context.map = Some(mapped);
    }
    let value = IzScienceInput {
        question_policy: QuestionPolicy::Legacy,
        context,
        parent_source_sha256: sha(&parent.request.source_input),
        prior_lgc_head: parent
            .request
            .lgc_steps
            .last()
            .ok_or("PDS2_PARENT_LGC_MISSING")?
            .step_digest,
        science_query,
        relevance_statement,
    };
    validate_parent(&value, parent)?;
    Ok(value)
}
/// New turns opt into result-aware durable output. Old sources keep exact bytes.
pub fn from_parent_computed(
    context: iz::IzContext, parent: &PdOperationRecord,
    science_query: Vec<u8>, relevance_statement: String,
) -> Result<IzScienceInput, String> {
    let mut value = from_parent(context, parent, science_query, relevance_statement)?;
    value.question_policy = QuestionPolicy::ComputedV1;
    value.validate()?;
    Ok(value)
}
pub fn encode_input(input: &IzScienceInput) -> Result<Vec<u8>, String> {
    input.validate()?;
    let mut out = match (input.question_policy, input.context.version == iz::IZ_NATIVE_CONTINUITY_VERSION) {
        (QuestionPolicy::Legacy, false) => b"PDS2".to_vec(),
        (QuestionPolicy::Legacy, true) => b"PDS3".to_vec(),
        (QuestionPolicy::ComputedV1, false) => b"PDS4".to_vec(),
        (QuestionPolicy::ComputedV1, true) => b"PDS5".to_vec(),
    };
    put_bytes(
        &mut out,
        &iz::encode_context(&input.context)?,
        iz::MAX_CONTEXT_BYTES,
    )?;
    out.extend_from_slice(&input.parent_source_sha256);
    out.extend_from_slice(&input.prior_lgc_head);
    put_bytes(
        &mut out,
        &input.science_query,
        linked_science::MAX_SCIENCE_QUERY_BYTES,
    )?;
    put_text(&mut out, &input.relevance_statement)?;
    if out.len() > MAX_SOURCE_BYTES {
        return Err("PDS2_SOURCE_LIMIT".into());
    }
    Ok(out)
}
pub fn decode_input(bytes: &[u8]) -> Result<IzScienceInput, String> {
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err("PDS2_SOURCE_LIMIT".into());
    }
    let mut r = Reader { bytes, pos: 0 };
    let magic = r.take(4)?;
    if !source_is_science(magic) {
        return Err("PDS2_VERSION".into());
    }
    let value = IzScienceInput {
        question_policy: if magic == b"PDS4" || magic == b"PDS5" { QuestionPolicy::ComputedV1 } else { QuestionPolicy::Legacy },
        context: iz::decode_context(&r.bytes(iz::MAX_CONTEXT_BYTES)?)?,
        parent_source_sha256: r.hash()?,
        prior_lgc_head: r.hash()?,
        science_query: r.bytes(linked_science::MAX_SCIENCE_QUERY_BYTES)?,
        relevance_statement: r.text()?,
    };
    r.end()?;
    value.validate()?;
    if source_is_native_science(magic) != (value.context.version == iz::IZ_NATIVE_CONTINUITY_VERSION) {
        return Err("PDS3_CONTEXT_VERSION".into());
    }
    if encode_input(&value)? != bytes {
        return Err("PDS2_NONCANONICAL".into());
    }
    Ok(value)
}
/// `parent` must come from the actual Vault owner, not a caller-supplied clone.
/// References bind data; they do not authenticate the caller.
pub fn validate_parent(input: &IzScienceInput, parent: &PdOperationRecord) -> Result<(), String> {
    input.validate()?;
    parent.validate()?;
    let prior = iz::validate_operation(&parent.request)?;
    if prior.context.version != input.context.version
        || (input.context.version == iz::IZ_NATIVE_CONTINUITY_VERSION
            && prior.context.native_anchor != input.context.native_anchor)
    {
        return Err("PDS_SAME_VERSION_ANCHOR_PARENT_REQUIRED".into());
    }
    iz::validate_context_parent(&input.context, parent)?;
    let actual = iz::IzParent::from_record(parent)?;
    let mapped = iz::IzMapSnapshot {
        identity_key: input.context.identity.key()?,
        head: actual.clone(),
        exact_record: actual,
        status: iz::IzMaterialStatus::OperationalNoVerdict,
    };
    let map_matches = if input.context.version == iz::IZ_NATIVE_CONTINUITY_VERSION {
        input.context.native_map.as_ref()
            == Some(&iz::native_map_for(
                &input.context.identity,
                input
                    .context
                    .native_anchor
                    .as_ref()
                    .ok_or("PDS3_NATIVE_ANCHOR_REQUIRED")?,
                Some(parent),
            )?)
    } else {
        input.context.map.as_ref() == Some(&mapped)
    };
    if !map_matches
        || input.parent_source_sha256 != sha(&parent.request.source_input)
        || input.prior_lgc_head
            != parent
                .request
                .lgc_steps
                .last()
                .ok_or("PDS2_PARENT_LGC_MISSING")?
                .step_digest
    {
        return Err("PDS2_PARENT_SOURCE_MAP_LGC_BINDING".into());
    }
    // validate_operation revalidates the actual retained missing-ASHT observation.
    // No new selection is claimed for this explicit material-acquisition turn.
    Ok(())
}
pub fn question(input: &IzScienceInput) -> Result<String, String> {
    input.validate()?;
    if input.question_policy != QuestionPolicy::Legacy {
        return Err("IZ_SCIENCE_COMPUTED_RESULT_REQUIRED".into());
    }
    let parent = input
        .context
        .parent
        .as_ref()
        .ok_or("PDS2_PARENT_REQUIRED")?;
    let q=format!("Materiali shkencor i kerkuar shprehimisht u llogarit offline per detyrimin {}. iZ prind={} pyetje_sha256={}; deficiti i ruajtur i historikut mbetet reference, jo rezultat i nje perzgjedhjeje te re. Hyrja dhe anotimet jane USER_REPORTED; kandidati eshte MODEL_COMPUTED, pa verdikt. Relevanca e propozuar: {}. Cilat te dhena dhe kushte te kontrollueshme lidhin kete model me detyrimin, dhe cila prove do ta kundershtonte? Vetem rrjedha ekzistuese KiE/UCL/ASHT/NBES/GCL mund te vleresoje propozimin kanonik; ky regjistrim nuk pranon K+/K-.",
        input.context.identity.obligation,hex(&parent.record_id),hex(&parent.question_sha256),input.relevance_statement);
    text_valid(&q, MAX_TEXT_BYTES)?;
    Ok(q)
}
/// Exact action payload commitments; actual execution order is also checked at producer sites.
pub fn build_steps(
    input: &IzScienceInput,
    checkpoint: &[u8],
    candidate: &[u8],
) -> Result<Vec<LgcStep>, String> {
    let legacy_question = question(input)?;
    build_steps_with_question(input, checkpoint, candidate, &legacy_question)
}
/// New policy binds exact output plus question before the operational append.
/// This is data coherence; Shadow independently recomputes the mathematical output.
pub fn build_steps_with_question(
    input: &IzScienceInput, checkpoint: &[u8], candidate: &[u8], scoped_question: &str,
) -> Result<Vec<LgcStep>, String> {
    text_valid(scoped_question, MAX_TEXT_BYTES)?;
    let source = encode_input(input)?;
    let mut bindings = vec![
        (
            "pd_mp",
            if input.context.version == iz::IZ_NATIVE_CONTINUITY_VERSION {
                "resolve exact current PIZ3 question and native-anchored LGC"
            } else {
                "resolve exact current PIZ2 question and prior LGC"
            },
            input.prior_lgc_head,
        ),
        (
            "Vault",
            if input.context.version == iz::IZ_NATIVE_CONTINUITY_VERSION {
                "read exact referenced PIZ3 missing observation"
            } else {
                "read exact referenced original PIZ2 missing observation"
            },
            input.parent_source_sha256,
        ),
        (
            "D-BAB",
            "witness retained missing condition; not a new ASHT attempt",
            input.parent_source_sha256,
        ),
        (
            "PD",
            "restore same actual-user-only cognitive history",
            input.context.parent.as_ref().unwrap().state_digest,
        ),
        (
            "science",
            "evaluate explicit scoped offline model; no authority",
            sha(&input.science_query),
        ),
        (
            "PD",
            "ingest one actual user turn only",
            sha(input.context.user_text.as_bytes()),
        ),
        ("PD", "export full continuous checkpoint", sha(checkpoint)),
        (
            "Vault",
            "request same-Vault atomic operational append",
            sha(candidate),
        ),
    ];
    if input.question_policy == QuestionPolicy::ComputedV1 {
        bindings.insert(5, ("PD", "assemble actual model result and exact evidence question", sha(scoped_question.as_bytes())));
    }
    let mut parent = sha(&source);
    let mut steps = Vec::new();
    for (i, (site, action, payload)) in bindings.into_iter().enumerate() {
        let s = LgcStep::new(i as u32, parent, site, action, payload)?;
        parent = s.step_digest;
        steps.push(s);
    }
    Ok(steps)
}
pub fn build_witness(
    input: &IzScienceInput,
    steps: &[LgcStep],
    source_sha256: Hash,
) -> Result<ConditionWitness, String> {
    Ok(ConditionWitness{predecessor:if input.context.version == iz::IZ_NATIVE_CONTINUITY_VERSION { "actual PIZ3 question and missing-ASHT observation".into() } else { "actual PIZ2 question and missing-ASHT observation".into() },
        parent_authority:"existing GCL/UCL; operational acquisition only".into(),
        failed_action:"retained exact historical Xi/Yi pair unavailable".into(),
        condition_code:"RETAINED_EXACT_PAIR_MISSING".into(),
        reason:"Original missing observation retained in exact parent; model material has no admission authority".into(),
        source_component:"pd_memory::iz_science".into(),source_sha256,
        attempt_trace:input.context.operation_id.clone(),input_sha256:sha(&encode_input(input)?),
        lgc_parent:steps.last().ok_or("PDS2_LGC_MISSING")?.step_digest,
        report_class:ReportClass::MissingConditions,permitted_next:NextAction::AcquireScopedEvidence})
}
/// Nonrecursive data validation. Full native parent/model checks are separate.
pub fn validate_operation(request: &PdOperationRequest) -> Result<IzScienceInput, String> {
    let input = decode_input(&request.source_input)?;
    let parent = input
        .context
        .parent
        .as_ref()
        .ok_or("PDS2_PARENT_REQUIRED")?;
    let steps = build_steps_with_question(&input, &request.checkpoint, &request.candidate, &request.scoped_question)?;
    let witness = build_witness(&input, &steps, request.witness.source_sha256)?;
    if request.identity != input.context.identity
        || request.operation_id != input.context.operation_id
        || request.expected_revision != parent.revision
        || request.previous_state != parent.state_digest
        || (input.question_policy == QuestionPolicy::Legacy && request.scoped_question != question(&input)?)
        || request.lgc_steps != steps
        || request.witness != witness
        || request.candidate.is_empty()
        || request.candidate_key == [0; 32]
        || request.checkpoint.len() <= input.checkpoint_prefix().len()
        || !request.checkpoint.starts_with(input.checkpoint_prefix())
    {
        return Err("PDS2_OPERATION_BINDING".into());
    }
    Ok(input)
}
/// Same validation on live append and WAL replay; a matching prefix alone is insufficient.
pub fn validate_transition(
    request: &PdOperationRequest,
    parent: &PdOperationRecord,
) -> Result<(), String> {
    let input = validate_operation(request)?;
    validate_parent(&input, parent)?;
    let previous = iz::checkpoint_raw(parent)?;
    let current = science_checkpoint_raw(request)?;
    let session = hex(&input.context.identity.key()?);
    let metadata = crate::pd_checkpoint_data::checkpoint_metadata(previous, &session)?;
    if metadata.territory != input.context.identity.scope {
        return Err("PDS2_CHECKPOINT_SCOPE".into());
    }
    crate::pd_checkpoint_data::validate_user_checkpoint_transition(
        current,
        &session,
        &input.context.user_text,
        u64::from(metadata.total_turns),
        Some(previous),
    )
}
pub fn science_checkpoint_raw(request: &PdOperationRequest) -> Result<&[u8], String> {
    let input = decode_input(&request.source_input)?;
    request
        .checkpoint
        .strip_prefix(input.checkpoint_prefix())
        .ok_or("PDS_CHECKPOINT_POLICY".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> IzScienceInput {
        let origin = "actual original user input".to_string();
        let parent = iz::IzParent {
            record_id: [1; 32],
            revision: 1,
            state_digest: [2; 32],
            question_sha256: [3; 32],
        };
        let identity = PdIdentity {
            owner: "owner".into(),
            obligation: "obligation".into(),
            case_id: "logical case".into(),
            scope: "scope".into(),
            domain: "scope".into(),
            lineage_root: hex(&sha(origin.as_bytes())),
        };
        IzScienceInput {
            question_policy: QuestionPolicy::Legacy,
            context: iz::IzContext {
                native_anchor: None,
                native_map: None,
                version: 2,
                map: Some(iz::IzMapSnapshot {
                    identity_key: identity.key().unwrap(),
                    head: parent.clone(),
                    exact_record: parent.clone(),
                    status: iz::IzMaterialStatus::OperationalNoVerdict,
                }),
                identity,
                operation_id: "new user turn".into(),
                origin_i0: origin,
                user_text: "Actual explicit query request".into(),
                parent: Some(parent),
                answer: iz::IzAnswer::default(),
            },
            parent_source_sha256: [4; 32],
            prior_lgc_head: [5; 32],
            science_query: vec![1],
            relevance_statement: "user reported, unverified".into(),
        }
    }
    #[test]
    fn bounded_canonical_codec_cannot_relax_legacy_or_detach_annotations() {
        let value = input();
        let encoded = encode_input(&value).unwrap();
        assert_eq!(decode_input(&encoded).unwrap(), value);
        for end in [0, 4, encoded.len() - 1] {
            assert!(decode_input(&encoded[..end]).is_err());
        }
        let mut extra = encoded.clone();
        extra.push(0);
        assert!(decode_input(&extra).is_err());
        assert!(decode_input(&vec![0; MAX_SOURCE_BYTES + 1]).is_err());
        let mut wrong = value.clone();
        wrong.context.version = 1;
        assert!(encode_input(&wrong).is_err());
        let mut wrong = value.clone();
        wrong.context.map = None;
        assert!(encode_input(&wrong).is_err());
        let mut wrong = value;
        wrong.context.answer.negative = "unsupplied words".into();
        assert!(encode_input(&wrong).is_err());
        assert!(linked_science::decode_linked_input(&encoded).is_err());
        assert!(failure::decode_failure_input(&encoded).is_err());
    }
    #[test]
    fn computed_policy_is_explicit_and_legacy_source_bytes_remain_exact() {
        let legacy = input();
        let original = encode_input(&legacy).unwrap();
        assert!(original.starts_with(b"PDS2"));
        let original_question = question(&legacy).unwrap();
        let mut computed = legacy.clone();
        computed.question_policy = QuestionPolicy::ComputedV1;
        let wire = encode_input(&computed).unwrap();
        assert!(wire.starts_with(b"PDS4"));
        assert_eq!(&wire[4..], &original[4..]);
        assert_eq!(decode_input(&wire).unwrap(), computed);
        assert!(question(&computed).is_err());
        let decoded = decode_input(&original).unwrap();
        assert_eq!(encode_input(&decoded).unwrap(), original);
        assert_eq!(question(&decoded).unwrap(), original_question);
        let mut mislabeled = wire;
        mislabeled[..4].copy_from_slice(b"PDS5");
        assert!(decode_input(&mislabeled).is_err());
    }
    #[test]
    fn question_is_output_and_no_candidate_can_claim_a_verdict() {
        let value = input();
        let q = question(&value).unwrap();
        assert!(q.contains("USER_REPORTED"));
        assert!(q.contains("MODEL_COMPUTED"));
        assert_ne!(q, value.context.user_text);
        assert!(value.context.answer.is_empty());
        let cp = [failure::FAILURE_CHECKPOINT_PREFIX, b"shape only"].concat();
        let candidate = vec![2];
        let steps = build_steps(&value, &cp, &candidate).unwrap();
        let witness = build_witness(&value, &steps, [8; 32]).unwrap();
        let mut r = PdOperationRequest {
            identity: value.context.identity.clone(),
            operation_id: value.context.operation_id.clone(),
            source_input: encode_input(&value).unwrap(),
            expected_revision: 1,
            previous_state: [2; 32],
            witness,
            checkpoint: cp,
            scoped_question: q,
            candidate,
            candidate_key: [9; 32],
            lgc_steps: steps,
        };
        r.validate().unwrap();
        r.witness.report_class = ReportClass::AuthorityRefused;
        assert!(r.validate().is_err());
    }
}
