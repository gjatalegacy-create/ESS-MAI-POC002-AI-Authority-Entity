//! single shared source algorithm; historical test module retained.
//! Original pre-move SHA256 1508fe707992aa55b602eff9aaf00a9a0610afe4cf28b08c8b7654b42ae523bc
pub use shadow_contracts::pd_memory::failure::relevance::*;

#[cfg(test)]
mod tests {
    use super::*;
    use shadow_contracts::{
        CandidateStatisticWire, KnowledgeSelectionRequestWire, KnowledgeSelectionResponseWire,
        SelectionPurpose, SelectionRequester, PROTOCOL_VERSION, SELECTION_MAX_CANDIDATES,
    };

    fn hash(fill: char) -> String {
        std::iter::repeat(fill).take(64).collect()
    }

    fn request(xi: &[&str], yi: &[&str]) -> KnowledgeSelectionRequestWire {
        KnowledgeSelectionRequestWire {
            version: PROTOCOL_VERSION,
            requester: SelectionRequester::AshtQuantum,
            purpose: SelectionPurpose::EvidenceCandidates,
            session_id: "T1".to_string(),
            parent_i0: hash('a'),
            primitive_anchor: 7,
            trace_id: "T1".to_string(),
            split_sha256: hash('b'),
            xi: xi.iter().map(|value| (*value).to_string()).collect(),
            yi: yi.iter().map(|value| (*value).to_string()).collect(),
            upstream_hold_sha256: Some(hash('c')),
            evidence_sha256: hash('d'),
            max_positive: SELECTION_MAX_CANDIDATES,
            max_negative: SELECTION_MAX_CANDIDATES,
        }
    }

    fn candidate(id: u64, polarity: u8, domain: &str, raw_term: &str) -> CandidateStatisticWire {
        let term_domain = match (polarity, domain) {
            (1, "primitive") => TERM_PRIMITIVE_DOMAIN,
            (1, "legacy") => TERM_LEGACY_DOMAIN,
            (0, _) => TERM_NEGATIVE_DOMAIN,
            _ => TERM_PRIMITIVE_DOMAIN,
        };
        CandidateStatisticWire {
            candidate_id: id,
            polarity,
            domain: domain.to_string(),
            term_sha256: shadow_contracts::sha256_hex(&[term_domain, raw_term.as_bytes()]),
            source_sha256: hash('e'),
            security_y: u8::from(polarity == 1),
            trust_x: u8::from(polarity == 1),
            confidence_fixed: 7_500,
            frequency: 2,
            domain_count: 1,
            legacy_score_fixed: 5_000,
        }
    }

    fn response(
        request: &KnowledgeSelectionRequestWire,
        positive: Vec<CandidateStatisticWire>,
        negative: Vec<CandidateStatisticWire>,
    ) -> KnowledgeSelectionResponseWire {
        let request_bytes = shadow_contracts::encode_selection_request(request);
        let request_sha256 = shadow_contracts::sha256_hex(&[
            b"ESSMAI/SELECTION/REQUEST/V178",
            request_bytes.as_slice(),
        ]);
        let mut response = KnowledgeSelectionResponseWire {
            version: PROTOCOL_VERSION,
            requester: request.requester,
            session_id: request.session_id.clone(),
            parent_i0: request.parent_i0.clone(),
            primitive_anchor: request.primitive_anchor,
            split_sha256: request.split_sha256.clone(),
            upstream_hold_sha256: request.upstream_hold_sha256.clone(),
            request_sha256,
            selection_sha256: String::new(),
            hold_id: "hold-1".to_string(),
            hold_sha256: hash('2'),
            positive,
            negative,
            law_seal: 11,
            system_laws_seal: 12,
            expires_at_ns: 13,
        };
        response.selection_sha256 = selection_digest(&response);
        response
    }

    #[test]
    fn exact_xi_yi_projection_is_accepted() {
        let request = request(&["alpha"], &["beta"]);
        let response = response(
            &request,
            vec![candidate(1, 1, "primitive", "alpha")],
            vec![candidate(2, 0, "negative-process", "beta")],
        );
        let projection = project_request_bound(&request, &response);
        match projection {
            Ok(value) => {
                assert_eq!(value.positive.len(), 1);
                assert_eq!(value.negative.len(), 1);
                assert_eq!(value.positive[0].axis, RelevanceAxis::Xi);
                assert_eq!(value.negative[0].axis, RelevanceAxis::Yi);
                assert!(shadow_contracts::is_canonical_sha256(
                    &value.relevance_sha256
                ));
            }
            Err(error) => panic!("unexpected relevance error: {error}"),
        }
    }

    #[test]
    fn unrelated_candidates_are_not_admitted() {
        let request = request(&["alpha"], &["beta"]);
        let response = response(
            &request,
            vec![candidate(1, 1, "primitive", "unrelated")],
            vec![candidate(2, 0, "negative-process", "beta")],
        );
        assert!(project_request_bound(&request, &response).is_err());
    }

    #[test]
    fn exact_bytes_are_case_sensitive() {
        let request = request(&["Alpha"], &["beta"]);
        let response = response(
            &request,
            vec![candidate(1, 1, "primitive", "alpha")],
            vec![candidate(2, 0, "negative-process", "beta")],
        );
        assert!(project_request_bound(&request, &response).is_err());
    }

    #[test]
    fn non_disjoint_xi_yi_is_rejected() {
        let request = request(&["same"], &["same"]);
        let response = response(
            &request,
            vec![candidate(1, 1, "primitive", "same")],
            vec![candidate(2, 0, "negative-process", "same")],
        );
        assert!(project_request_bound(&request, &response).is_err());
    }

    #[test]
    fn unknown_positive_domain_is_rejected() {
        let request = request(&["alpha"], &["beta"]);
        let response = response(
            &request,
            vec![candidate(1, 1, "scientific", "alpha")],
            vec![candidate(2, 0, "negative-process", "beta")],
        );
        assert!(project_request_bound(&request, &response).is_err());
    }

    #[test]
    fn positive_boundary_equal_to_limit_is_rejected() {
        let request = request(&["alpha"], &["beta"]);
        let positive = (0..SELECTION_MAX_CANDIDATES)
            .map(|index| candidate(u64::from(index).saturating_add(1), 1, "primitive", "alpha"))
            .collect();
        let response = response(
            &request,
            positive,
            vec![candidate(500, 0, "negative-process", "beta")],
        );
        assert!(project_request_bound(&request, &response).is_err());
    }

    #[test]
    fn negative_boundary_equal_to_limit_is_rejected() {
        let request = request(&["alpha"], &["beta"]);
        let negative = (0..SELECTION_MAX_CANDIDATES)
            .map(|index| {
                candidate(
                    u64::from(index).saturating_add(1),
                    0,
                    "negative-process",
                    "beta",
                )
            })
            .collect();
        let response = response(
            &request,
            vec![candidate(500, 1, "primitive", "alpha")],
            negative,
        );
        assert!(project_request_bound(&request, &response).is_err());
    }

    #[test]
    fn protocol_maximum_limits_are_required() {
        let mut request = request(&["alpha"], &["beta"]);
        request.max_positive = 64;
        request.max_negative = 64;
        let response = response(
            &request,
            vec![candidate(1, 1, "primitive", "alpha")],
            vec![candidate(2, 0, "negative-process", "beta")],
        );
        assert!(project_request_bound(&request, &response).is_err());
    }

    #[test]
    fn missing_exact_negative_candidate_is_rejected() {
        let request = request(&["alpha"], &["beta"]);
        let response = response(
            &request,
            vec![candidate(1, 1, "primitive", "alpha")],
            vec![candidate(2, 0, "negative-process", "unrelated")],
        );
        assert!(project_request_bound(&request, &response).is_err());
    }

    #[test]
    fn relevant_set_over_directive_capacity_is_rejected() {
        let request = request(&["alpha"], &["beta"]);
        let positive = (0..64u16)
            .map(|index| candidate(u64::from(index).saturating_add(1), 1, "primitive", "alpha"))
            .collect();
        let negative = (0..65u16)
            .map(|index| {
                candidate(
                    u64::from(index).saturating_add(1000),
                    0,
                    "negative-process",
                    "beta",
                )
            })
            .collect();
        let response = response(&request, positive, negative);
        assert!(project_request_bound(&request, &response).is_err());
    }

    #[test]
    fn tampered_request_digest_is_rejected() {
        let request = request(&["alpha"], &["beta"]);
        let mut response = response(
            &request,
            vec![candidate(1, 1, "primitive", "alpha")],
            vec![candidate(2, 0, "negative-process", "beta")],
        );
        response.request_sha256 = hash('8');
        assert!(project_request_bound(&request, &response).is_err());
    }

    #[test]
    fn tampered_selection_digest_is_rejected() {
        let request = request(&["alpha"], &["beta"]);
        let mut response = response(
            &request,
            vec![candidate(1, 1, "primitive", "alpha")],
            vec![candidate(2, 0, "negative-process", "beta")],
        );
        response.selection_sha256 = hash('9');
        assert!(project_request_bound(&request, &response).is_err());
    }

    #[test]
    fn relevance_digest_is_deterministic() {
        let request = request(&["alpha"], &["beta"]);
        let response = response(
            &request,
            vec![candidate(1, 1, "primitive", "alpha")],
            vec![candidate(2, 0, "negative-process", "beta")],
        );
        let first = project_request_bound(&request, &response);
        let second = project_request_bound(&request, &response);
        match (first, second) {
            (Ok(left), Ok(right)) => assert_eq!(left.relevance_sha256, right.relevance_sha256),
            (Err(left), Err(right)) => panic!("unexpected relevance errors: {left}; {right}"),
            (Err(error), _) | (_, Err(error)) => panic!("unexpected relevance error: {error}"),
        }
    }
}
