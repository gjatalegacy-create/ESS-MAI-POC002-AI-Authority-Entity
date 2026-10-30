//! PDF1, reported missing-history dialogue, no verdict authority.
//! pd_mp is the persistent PD map of record references. Raw user/support/refusal
//! material belongs to the single Vault record, never a second map warehouse.
//! Shared relevance source: quantum/src/request_bound_relevance.rs (pre-move)
//! SHA256 1508fe707992aa55b602eff9aaf00a9a0610afe4cf28b08c8b7654b42ae523bc. Algorithm moved unchanged except namespace qualification.
use super::*;
use crate::{
    KnowledgeSelectionRequestWire, KnowledgeSelectionResponseWire, SelectionPurpose,
    SelectionRequester,
};

pub const MAX_SELECTION_REQUEST_BYTES: usize = 64 * 1024;
pub const MAX_SELECTION_RESPONSE_BYTES: usize = 128 * 1024;
pub const FAILURE_CHECKPOINT_PREFIX: &[u8] = b"PD_FAILURE_USER_ONLY_V1\0";
pub const MISSING_POSITIVE: &str = "request-bound relevance found no exact positive candidate";
pub const MISSING_NEGATIVE: &str = "request-bound relevance found no exact negative candidate";

pub mod relevance {
    use crate::{
        CandidateStatisticWire, KnowledgeSelectionRequestWire, KnowledgeSelectionResponseWire,
        SELECTION_MAX_CANDIDATES,
    };
    use std::collections::BTreeSet;

    #[doc(hidden)]
    pub const TERM_PRIMITIVE_DOMAIN: &[u8] = b"ESSMAI/SELECTION/TERM/PRIMITIVE";
    #[doc(hidden)]
    pub const TERM_LEGACY_DOMAIN: &[u8] = b"ESSMAI/SELECTION/TERM/LEGACY";
    #[doc(hidden)]
    pub const TERM_NEGATIVE_DOMAIN: &[u8] = b"ESSMAI/SELECTION/TERM/NEGATIVE";
    const RELEVANCE_DOMAIN: &[u8] = b"ESSMAI/ASHT/REQUEST-BOUND-RELEVANCE/V1";
    const DIRECTIVE_CANDIDATE_LIMIT: usize = 128;

    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum RelevanceAxis {
        Xi = 1,
        Yi = 2,
    }

    impl RelevanceAxis {
        pub fn label(self) -> &'static str {
            match self {
                Self::Xi => "Xi",
                Self::Yi => "Yi",
            }
        }
    }

    #[derive(Debug)]
    pub struct RelevantCandidate<'a> {
        pub candidate: &'a CandidateStatisticWire,
        pub axis: RelevanceAxis,
    }

    #[derive(Debug)]
    pub struct RequestBoundProjection<'a> {
        pub positive: Vec<RelevantCandidate<'a>>,
        pub negative: Vec<RelevantCandidate<'a>>,
        pub relevance_sha256: String,
    }

    impl RequestBoundProjection<'_> {
        pub fn total_len(&self) -> usize {
            self.positive.len().saturating_add(self.negative.len())
        }
    }

    #[derive(Debug)]
    struct AxisHashes {
        primitive: BTreeSet<String>,
        legacy: BTreeSet<String>,
        negative: BTreeSet<String>,
    }

    impl AxisHashes {
        fn from_terms(terms: &[String]) -> Self {
            Self {
                primitive: hash_terms(terms, TERM_PRIMITIVE_DOMAIN),
                legacy: hash_terms(terms, TERM_LEGACY_DOMAIN),
                negative: hash_terms(terms, TERM_NEGATIVE_DOMAIN),
            }
        }

        fn overlaps(&self, other: &Self) -> bool {
            !self.primitive.is_disjoint(&other.primitive)
                || !self.legacy.is_disjoint(&other.legacy)
                || !self.negative.is_disjoint(&other.negative)
        }
    }

    fn hash_terms(terms: &[String], domain: &[u8]) -> BTreeSet<String> {
        terms
            .iter()
            .map(|term| crate::sha256_hex(&[domain, term.as_bytes()]))
            .collect()
    }

    fn axis_for_hash(
        term_sha256: &str,
        xi_hashes: &BTreeSet<String>,
        yi_hashes: &BTreeSet<String>,
    ) -> Result<Option<RelevanceAxis>, String> {
        match (
            xi_hashes.contains(term_sha256),
            yi_hashes.contains(term_sha256),
        ) {
            (true, false) => Ok(Some(RelevanceAxis::Xi)),
            (false, true) => Ok(Some(RelevanceAxis::Yi)),
            (false, false) => Ok(None),
            (true, true) => Err("request-bound relevance rejected Xi/Yi hash overlap".to_string()),
        }
    }

    fn positive_axis(
        candidate: &CandidateStatisticWire,
        xi: &AxisHashes,
        yi: &AxisHashes,
    ) -> Result<Option<RelevanceAxis>, String> {
        match candidate.domain.as_str() {
            "primitive" => {
                axis_for_hash(candidate.term_sha256.as_str(), &xi.primitive, &yi.primitive)
            }
            "legacy" => axis_for_hash(candidate.term_sha256.as_str(), &xi.legacy, &yi.legacy),
            _ => Err("request-bound relevance rejected an unknown positive domain".to_string()),
        }
    }

    fn negative_axis(
        candidate: &CandidateStatisticWire,
        xi: &AxisHashes,
        yi: &AxisHashes,
    ) -> Result<Option<RelevanceAxis>, String> {
        axis_for_hash(candidate.term_sha256.as_str(), &xi.negative, &yi.negative)
    }

    fn append_candidate_material(material: &mut Vec<u8>, relevant: &RelevantCandidate<'_>) {
        let candidate = relevant.candidate;
        material.push(relevant.axis as u8);
        material.extend_from_slice(&candidate.candidate_id.to_le_bytes());
        material.push(candidate.polarity);
        material.extend_from_slice(&(candidate.domain.len() as u64).to_le_bytes());
        material.extend_from_slice(candidate.domain.as_bytes());
        material.extend_from_slice(candidate.term_sha256.as_bytes());
        material.extend_from_slice(candidate.source_sha256.as_bytes());
        material.push(candidate.security_y);
        material.push(candidate.trust_x);
        material.extend_from_slice(&candidate.confidence_fixed.to_le_bytes());
        material.extend_from_slice(&candidate.frequency.to_le_bytes());
        material.extend_from_slice(&candidate.domain_count.to_le_bytes());
        material.extend_from_slice(&candidate.legacy_score_fixed.to_le_bytes());
    }

    #[doc(hidden)]
    pub fn selection_digest(response: &KnowledgeSelectionResponseWire) -> String {
        let mut material = Vec::new();
        for candidate in response.positive.iter().chain(response.negative.iter()) {
            material.extend_from_slice(&candidate.candidate_id.to_le_bytes());
            material.push(candidate.polarity);
            material.extend_from_slice(&(candidate.domain.len() as u64).to_le_bytes());
            material.extend_from_slice(candidate.domain.as_bytes());
            material.extend_from_slice(candidate.term_sha256.as_bytes());
            material.extend_from_slice(candidate.source_sha256.as_bytes());
            material.push(candidate.security_y);
            material.push(candidate.trust_x);
            material.extend_from_slice(&candidate.confidence_fixed.to_le_bytes());
            material.extend_from_slice(&candidate.frequency.to_le_bytes());
            material.extend_from_slice(&candidate.domain_count.to_le_bytes());
            material.extend_from_slice(&candidate.legacy_score_fixed.to_le_bytes());
        }
        crate::sha256_hex(&[b"ESSMAI/SELECTION/CANDIDATES/V178", material.as_slice()])
    }

    fn response_is_bound(
        request: &KnowledgeSelectionRequestWire,
        response: &KnowledgeSelectionResponseWire,
    ) -> bool {
        let request_bytes = crate::encode_selection_request(request);
        let request_sha256 =
            crate::sha256_hex(&[b"ESSMAI/SELECTION/REQUEST/V178", request_bytes.as_slice()]);
        request_sha256 == response.request_sha256
            && selection_digest(response) == response.selection_sha256
    }

    /// Produces the complete, exact-byte request-relevant subset of the Shadow
    /// projection. The function is deliberately conservative:
    ///
    /// - both request limits must be the protocol maximum;
    /// - a response length equal to its limit is treated as potentially truncated;
    /// - Xi/Yi terms are never normalized or semantically guessed;
    /// - both polarities must contain request-bound material;
    /// - the relevant set must fit the existing Asht directive capacity.
    ///
    /// Any unprovable condition is terminal and returns `Err`.
    pub fn project_request_bound<'a>(
        request: &KnowledgeSelectionRequestWire,
        response: &'a KnowledgeSelectionResponseWire,
    ) -> Result<RequestBoundProjection<'a>, String> {
        let projection = project_available_bound(request, response)?;
        require_both_polarities(&projection)?;
        Ok(projection)
    }

    /// R15: the both-polarities rule held once. Exact strings and priority are
    /// unchanged, because PDF1 replay compares them.
    pub fn require_both_polarities(projection: &RequestBoundProjection<'_>) -> Result<(), String> {
        match (
            projection.positive.is_empty(),
            projection.negative.is_empty(),
        ) {
            (false, false) => Ok(()),
            (true, _) => Err("request-bound relevance found no exact positive candidate".into()),
            (_, true) => Err("request-bound relevance found no exact negative candidate".into()),
        }
    }

    /// NEW KiE parallel observation: SAME source shape, complete enumeration,
    /// exact Xi/Yi relevance and capacity checks. It reports missing polarities;
    /// it never grants the historical route permission to advance without them.
    pub fn project_available_bound<'a>(
        request: &KnowledgeSelectionRequestWire,
        response: &'a KnowledgeSelectionResponseWire,
    ) -> Result<RequestBoundProjection<'a>, String> {
        match (
            request.shape_is_valid(),
            response.shape_is_valid_for(request),
            response_is_bound(request, response),
        ) {
            (true, true, true) => {}
            (false, _, _) => {
                return Err("request-bound relevance rejected request shape".to_string())
            }
            (_, false, _) => {
                return Err("request-bound relevance rejected response shape".to_string())
            }
            (_, _, false) => {
                return Err("request-bound relevance rejected response digest binding".to_string())
            }
        }

        match (
            request.max_positive == SELECTION_MAX_CANDIDATES,
            request.max_negative == SELECTION_MAX_CANDIDATES,
        ) {
            (true, true) => {}
            _ => {
                return Err(
                    "request-bound relevance requires the complete protocol candidate limits"
                        .to_string(),
                )
            }
        }

        match (
            response.positive.len() < usize::from(request.max_positive),
            response.negative.len() < usize::from(request.max_negative),
        ) {
            (true, true) => {}
            (false, _) => {
                return Err(
                    "request-bound relevance cannot prove complete positive enumeration"
                        .to_string(),
                )
            }
            (_, false) => {
                return Err(
                    "request-bound relevance cannot prove complete negative enumeration"
                        .to_string(),
                )
            }
        }

        let xi = AxisHashes::from_terms(&request.xi);
        let yi = AxisHashes::from_terms(&request.yi);
        match xi.overlaps(&yi) {
            true => {
                return Err("request-bound relevance rejected non-disjoint Xi/Yi terms".to_string())
            }
            false => {}
        }

        let mut positive = Vec::new();
        for candidate in &response.positive {
            match positive_axis(candidate, &xi, &yi)? {
                Some(axis) => positive.push(RelevantCandidate { candidate, axis }),
                None => {}
            }
        }

        let mut negative = Vec::new();
        for candidate in &response.negative {
            match negative_axis(candidate, &xi, &yi)? {
                Some(axis) => negative.push(RelevantCandidate { candidate, axis }),
                None => {}
            }
        }

        let total = positive.len().saturating_add(negative.len());
        match total <= DIRECTIVE_CANDIDATE_LIMIT {
            true => {}
            false => {
                return Err(
                    "request-bound relevance exceeds the existing Asht directive capacity"
                        .to_string(),
                )
            }
        }

        let mut material = Vec::new();
        for relevant in positive.iter().chain(negative.iter()) {
            append_candidate_material(&mut material, relevant);
        }
        let relevance_sha256 = crate::sha256_hex(&[
            RELEVANCE_DOMAIN,
            request.evidence_sha256.as_bytes(),
            request.split_sha256.as_bytes(),
            response.request_sha256.as_bytes(),
            response.selection_sha256.as_bytes(),
            material.as_slice(),
        ]);

        Ok(RequestBoundProjection {
            positive,
            negative,
            relevance_sha256,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingSelection {
    Positive,
    Negative,
}
impl MissingSelection {
    pub fn reason(self) -> &'static str {
        match self {
            Self::Positive => MISSING_POSITIVE,
            Self::Negative => MISSING_NEGATIVE,
        }
    }
}

/// Reproduce the exact original missing-pair result, not a caller-chosen failure.
/// Digest/shape/enumeration/scope faults never enter this continuation route.
pub fn validate_missing_selection(
    request_bytes: &[u8],
    response_bytes: &[u8],
    refusal: &str,
) -> Result<MissingSelection, String> {
    let (request, response) = decode_selection_pair(request_bytes, response_bytes)?;
    let observed = match relevance::project_request_bound(&request, &response) {
        Ok(_) => return Err("PD_FAILURE_PAIR_ALREADY_PRESENT".into()),
        Err(error) => error,
    };
    let missing = match observed.as_str() {
        MISSING_POSITIVE => MissingSelection::Positive,
        MISSING_NEGATIVE => MissingSelection::Negative,
        _ => return Err(format!("PD_FAILURE_NON_MISSING_REFUSAL:{observed}")),
    };
    if refusal != observed {
        return Err("PD_FAILURE_REASON_MISMATCH".into());
    }
    Ok(missing)
}

pub fn decode_selection_pair(
    request_bytes: &[u8],
    response_bytes: &[u8],
) -> Result<
    (
        KnowledgeSelectionRequestWire,
        KnowledgeSelectionResponseWire,
    ),
    String,
> {
    if request_bytes.len() > MAX_SELECTION_REQUEST_BYTES
        || response_bytes.len() > MAX_SELECTION_RESPONSE_BYTES
    {
        return Err("PD_FAILURE_SELECTION_LIMIT".into());
    }
    let request = crate::decode_selection_request(request_bytes)
        .map_err(|e| format!("PD_FAILURE_REQUEST:{e}"))?;
    let response = crate::decode_selection_response(response_bytes)
        .map_err(|e| format!("PD_FAILURE_RESPONSE:{e}"))?;
    if crate::encode_selection_request(&request) != request_bytes
        || crate::encode_selection_response(&response) != response_bytes
    {
        return Err("PD_FAILURE_SELECTION_NONCANONICAL".into());
    }
    if request.requester != SelectionRequester::AshtQuantum
        || request.purpose != SelectionPurpose::EvidenceCandidates
    {
        return Err("PD_FAILURE_WRONG_PREDECESSOR".into());
    }
    Ok((request, response))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureDialogueInput {
    pub identity: PdIdentity,
    pub operation_id: String,
    pub user_text: String,
    pub selection_request: Vec<u8>,
    pub selection_response: Vec<u8>,
    pub refusal: String,
    pub in_reply_to: Hash,
    /// USER-REPORTED only. These are not admitted positive/negative knowledge.
    pub positive: String,
    pub negative: String,
    pub distinction: String,
    pub source_reference: String,
}
impl FailureDialogueInput {
    pub fn validate(&self) -> Result<(), String> {
        self.identity.validate()?;
        text_valid(&self.operation_id, 1024)?;
        text_valid(&self.user_text, MAX_TEXT_BYTES)?;
        text_valid(&self.refusal, 1024)?;
        validate_missing_selection(
            &self.selection_request,
            &self.selection_response,
            &self.refusal,
        )?;
        let (selection, _) =
            decode_selection_pair(&self.selection_request, &self.selection_response)?;
        if self.identity.case_id != selection.session_id
            || self.identity.lineage_root != selection.parent_i0
        {
            return Err("PD_FAILURE_SOURCE_IDENTITY_MISMATCH".into());
        }
        let supplied = [
            &self.positive,
            &self.negative,
            &self.distinction,
            &self.source_reference,
        ];
        if self.in_reply_to == [0; 32] {
            if supplied.iter().any(|s| !s.is_empty()) {
                return Err("PD_FAILURE_INITIAL_ANSWER_NOT_ALLOWED".into());
            }
        } else {
            for value in supplied {
                text_valid(value, MAX_TEXT_BYTES)?;
            }
        }
        Ok(())
    }
}

/// Semantic consistency for native admission and replay projection. This is
/// not caller authentication; it prevents a valid PDF1 from being relabelled
/// as a transport fault, authority refusal or unrelated condition.
pub fn validate_failure_operation(
    request: &PdOperationRequest,
) -> Result<FailureDialogueInput, String> {
    request.validate()?;
    let input = super::linked_science::decode_dialogue_source(&request.source_input)?;
    let missing = validate_missing_selection(
        &input.selection_request,
        &input.selection_response,
        &input.refusal,
    )?;
    let expected_code = match missing {
        MissingSelection::Positive => "MISSING_EXACT_POSITIVE",
        MissingSelection::Negative => "MISSING_EXACT_NEGATIVE",
    };
    if request.witness.report_class != ReportClass::MissingConditions
        || request.witness.permitted_next != NextAction::AcquireScopedEvidence
        || request.witness.reason != input.refusal
        || request.witness.condition_code != expected_code
        || request.witness.attempt_trace != input.operation_id
    {
        return Err("PD_FAILURE_WITNESS_INCONSISTENT".into());
    }
    let is_linked = request.source_input.starts_with(b"PDS1");
    let candidate_valid = if is_linked {
        !request.candidate.is_empty() && request.candidate_key != [0; 32]
    } else {
        request.candidate.is_empty() && request.candidate_key == [0; 32]
    };
    if !candidate_valid || !request.checkpoint.starts_with(FAILURE_CHECKPOINT_PREFIX) {
        return Err("PD_FAILURE_STORAGE_POLICY".into());
    }
    Ok(input)
}

pub fn encode_failure_input(input: &FailureDialogueInput) -> Result<Vec<u8>, String> {
    input.validate()?;
    let mut out = b"PDF1".to_vec();
    identity_out(&mut out, &input.identity)?;
    put_text(&mut out, &input.operation_id)?;
    put_text(&mut out, &input.user_text)?;
    put_bytes(
        &mut out,
        &input.selection_request,
        MAX_SELECTION_REQUEST_BYTES,
    )?;
    put_bytes(
        &mut out,
        &input.selection_response,
        MAX_SELECTION_RESPONSE_BYTES,
    )?;
    put_text(&mut out, &input.refusal)?;
    out.extend_from_slice(&input.in_reply_to);
    for text in [
        &input.positive,
        &input.negative,
        &input.distinction,
        &input.source_reference,
    ] {
        put_text(&mut out, text)?;
    }
    if out.len() > MAX_SOURCE_BYTES {
        return Err("PD_SOURCE_LIMIT".into());
    }
    Ok(out)
}
pub fn decode_failure_input(bytes: &[u8]) -> Result<FailureDialogueInput, String> {
    if bytes.len() > MAX_SOURCE_BYTES {
        return Err("PD_SOURCE_LIMIT".into());
    }
    let mut reader = Reader { bytes, pos: 0 };
    if reader.take(4)? != b"PDF1" {
        return Err("PD_FAILURE_VERSION".into());
    }
    let input = FailureDialogueInput {
        identity: identity_in(&mut reader)?,
        operation_id: reader.text()?,
        user_text: reader.text()?,
        selection_request: reader.bytes(MAX_SELECTION_REQUEST_BYTES)?,
        selection_response: reader.bytes(MAX_SELECTION_RESPONSE_BYTES)?,
        refusal: reader.text()?,
        in_reply_to: reader.hash()?,
        positive: reader.text()?,
        negative: reader.text()?,
        distinction: reader.text()?,
        source_reference: reader.text()?,
    };
    reader.end()?;
    if encode_failure_input(&input)? != bytes {
        return Err("PD_FAILURE_NONCANONICAL".into());
    }
    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;
    fn fixture() -> FailureDialogueInput {
        let request = KnowledgeSelectionRequestWire {
            version: PROTOCOL_VERSION,
            requester: SelectionRequester::AshtQuantum,
            purpose: SelectionPurpose::EvidenceCandidates,
            session_id: "case-A".into(),
            parent_i0: "i0-0000000000000007".into(),
            primitive_anchor: 7,
            trace_id: "trace-A".into(),
            split_sha256: "11".repeat(32),
            xi: vec!["alpha".into()],
            yi: vec!["beta".into()],
            upstream_hold_sha256: Some("22".repeat(32)),
            evidence_sha256: "33".repeat(32),
            max_positive: SELECTION_MAX_CANDIDATES,
            max_negative: SELECTION_MAX_CANDIDATES,
        };
        let request_bytes = encode_selection_request(&request);
        let response = KnowledgeSelectionResponseWire {
            version: PROTOCOL_VERSION,
            requester: request.requester,
            session_id: request.session_id.clone(),
            parent_i0: request.parent_i0.clone(),
            primitive_anchor: 7,
            split_sha256: request.split_sha256.clone(),
            upstream_hold_sha256: request.upstream_hold_sha256.clone(),
            request_sha256: crate::sha256_hex(&[b"ESSMAI/SELECTION/REQUEST/V178", &request_bytes]),
            selection_sha256: crate::sha256_hex(&[b"ESSMAI/SELECTION/CANDIDATES/V178", &[]]),
            hold_id: "hold-A".into(),
            hold_sha256: "44".repeat(32),
            positive: vec![],
            negative: vec![],
            law_seal: 1,
            system_laws_seal: 2,
            expires_at_ns: 3,
        };
        FailureDialogueInput {
            identity: PdIdentity {
                owner: "owner".into(),
                obligation: "obligation".into(),
                case_id: request.session_id.clone(),
                scope: "scope".into(),
                domain: "physics".into(),
                lineage_root: request.parent_i0.clone(),
            },
            operation_id: "failed-initial".into(),
            user_text: "Si dallohet alpha nga beta?".into(),
            selection_request: request_bytes,
            selection_response: encode_selection_response(&response),
            refusal: MISSING_POSITIVE.into(),
            in_reply_to: [0; 32],
            positive: String::new(),
            negative: String::new(),
            distinction: String::new(),
            source_reference: String::new(),
        }
    }
    #[test]
    fn pdf1_roundtrip_and_answer_fields_remain_user_reported() {
        let mut input = fixture();
        let bytes = encode_failure_input(&input).unwrap();
        assert_eq!(decode_failure_input(&bytes).unwrap(), input);
        input.in_reply_to = [9; 32];
        assert!(encode_failure_input(&input).is_err());
        input.positive = "pretendim mbeshtetes".into();
        input.negative = "kundershtim".into();
        input.distinction = "dallimi".into();
        input.source_reference = "raport lokal".into();
        assert_eq!(
            decode_failure_input(&encode_failure_input(&input).unwrap()).unwrap(),
            input
        );
        input.in_reply_to = [0; 32];
        assert!(input.validate().is_err());
    }
    #[test]
    fn unrelated_refusal_scope_tamper_and_malformed_selection_cannot_unlock_failure() {
        let mut input = fixture();
        input.refusal = "transport timeout".into();
        assert!(input.validate().is_err());
        let mut input = fixture();
        input.identity.case_id.push('x');
        assert!(input.validate().is_err());
        let mut input = fixture();
        input.identity.lineage_root.push('x');
        assert!(input.validate().is_err());
        let mut input = fixture();
        input.selection_response[10] ^= 1;
        assert!(input.validate().is_err());
        let mut input = fixture();
        let (mut request, mut response) =
            decode_selection_pair(&input.selection_request, &input.selection_response).unwrap();
        request.yi = request.xi.clone();
        input.selection_request = encode_selection_request(&request);
        response.request_sha256 =
            crate::sha256_hex(&[b"ESSMAI/SELECTION/REQUEST/V178", &input.selection_request]);
        input.selection_response = encode_selection_response(&response);
        assert!(input.validate().unwrap_err().contains("NON_MISSING"));
    }
    #[test]
    fn pdf1_codec_bounds_versions_and_trailing_bytes_fail_closed() {
        let input = fixture();
        let mut bytes = encode_failure_input(&input).unwrap();
        bytes.push(0);
        assert!(decode_failure_input(&bytes).is_err());
        bytes[0] = b'X';
        assert!(decode_failure_input(&bytes).is_err());
        assert!(decode_failure_input(&vec![0; MAX_SOURCE_BYTES + 1]).is_err());
        let mut input = fixture();
        input.selection_request = vec![0; MAX_SELECTION_REQUEST_BYTES + 1];
        assert!(input.validate().is_err());
    }
    #[test]
    fn pd_record_accepts_pdf1_only_with_exact_source_identity_and_lgc_binding() {
        let input = fixture();
        let mut request = super::super::tests::request();
        request.identity = input.identity.clone();
        request.operation_id = input.operation_id.clone();
        request.source_input = encode_failure_input(&input).unwrap();
        request.witness.input_sha256 = Sha256::digest(&request.source_input).into();
        let step = LgcStep::new(
            0,
            request.witness.input_sha256,
            "D-BAB",
            "actual missing condition",
            [8; 32],
        )
        .unwrap();
        request.witness.lgc_parent = step.step_digest;
        request.lgc_steps = vec![step];
        request.candidate.clear();
        request.candidate_key = [0; 32];
        assert!(request.validate().is_ok());
        request.operation_id.push('x');
        assert!(request.validate().is_err());
    }
    #[test]
    fn exact_missing_negative_is_eligible_but_complete_pair_is_not() {
        let mut input = fixture();
        let (_, mut response) =
            decode_selection_pair(&input.selection_request, &input.selection_response).unwrap();
        response.positive.push(CandidateStatisticWire {
            candidate_id: 1,
            polarity: 1,
            domain: "primitive".into(),
            term_sha256: crate::sha256_hex(&[relevance::TERM_PRIMITIVE_DOMAIN, b"alpha"]),
            source_sha256: "55".repeat(32),
            security_y: 1,
            trust_x: 1,
            confidence_fixed: 7500,
            frequency: 2,
            domain_count: 1,
            legacy_score_fixed: 5000,
        });
        response.selection_sha256 = relevance::selection_digest(&response);
        input.selection_response = encode_selection_response(&response);
        input.refusal = MISSING_NEGATIVE.into();
        assert_eq!(
            validate_missing_selection(
                &input.selection_request,
                &input.selection_response,
                &input.refusal
            )
            .unwrap(),
            MissingSelection::Negative
        );
        input.validate().unwrap();
        response.negative.push(CandidateStatisticWire {
            candidate_id: 2,
            polarity: 0,
            domain: "negative-process".into(),
            term_sha256: crate::sha256_hex(&[relevance::TERM_NEGATIVE_DOMAIN, b"beta"]),
            source_sha256: "66".repeat(32),
            security_y: 0,
            trust_x: 0,
            confidence_fixed: 7500,
            frequency: 2,
            domain_count: 1,
            legacy_score_fixed: 5000,
        });
        response.selection_sha256 = relevance::selection_digest(&response);
        input.selection_response = encode_selection_response(&response);
        assert!(input
            .validate()
            .unwrap_err()
            .contains("PAIR_ALREADY_PRESENT"));
    }
}
