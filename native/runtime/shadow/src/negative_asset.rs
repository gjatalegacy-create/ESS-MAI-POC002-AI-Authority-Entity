//! AIGovern-Cyber v1.7.1 — Shadow verification of living Negative Knowledge.
//!
//! Shadow does not rediscover negative evidence and does not execute NPRO.
//! It verifies the canonical asset produced by Quantum, recomputes the
//! domain-separated SHA-256 token, binds the material to GCL/PD/TRL and the
//! existing action ledger, and returns a persistence classification to the
//! already sovereign verdict path.

use sha2::{Digest, Sha256};
use shadow_contracts::negative_asset::{
    NegativeAssetEnvelope, NEGATIVE_ASSET_CLASS_EXPERIMENTAL_FAILURE, NEGATIVE_ASSET_TOKEN_DOMAIN,
    NEGATIVE_FLOW_APRO, NEGATIVE_FLOW_HCP_PRO, NEGATIVE_FLOW_LIM, NEGATIVE_FLOW_MPRO,
    NEGATIVE_FLOW_NPIM, NEGATIVE_FLOW_NPRO, NEGATIVE_SPACE_XI, NEGATIVE_SPACE_YI,
};
use shadow_contracts::{NegativeKnowledgeWire, QuantumInboundWire};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq)]
pub struct ValidatedNegativeAsset {
    /// The exact validated wire material. Ownership is transferred once from
    /// process_bridge into the Shadow-only gateway sidecar; no payload clone.
    pub wire: NegativeKnowledgeWire,
    pub token_sha256: [u8; 32],
    pub token_hex: String,
    pub experimental_failure: bool,
    pub target_count: usize,
    pub project_id: u64,
    pub trl_level: u8,
}

pub fn validate(
    q: &QuantumInboundWire,
    negative: NegativeKnowledgeWire,
) -> Result<ValidatedNegativeAsset, String> {
    let envelope = NegativeAssetEnvelope::decode(&negative.bytes)
        .map_err(|error| format!("Negative Knowledge asset envelope decode failed: {error:?}"))?;
    require(
        envelope.shape_is_valid(shadow_contracts::MASS_SCALE),
        "Negative Knowledge asset envelope has an invalid canonical shape",
    )?;

    let expected_token = checked_token_for(&envelope)?;
    let token_hex = hex_32(&expected_token);
    let process_binding = format!("GCL_NEGATIVE_ASSET_SHA256={token_hex}");
    require(
        negative
            .process
            .split('|')
            .any(|part| part == process_binding.as_str()),
        "Negative Knowledge process does not carry the canonical SHA-256 token",
    )?;

    bind_gcl_identity(q, &envelope)?;
    bind_module_evidence(q, &negative, &envelope)?;
    bind_scientific_failure(q, &envelope)?;
    bind_targets(q, &envelope)?;
    bind_operator_arguments(q, &envelope)?;

    Ok(ValidatedNegativeAsset {
        wire: negative,
        token_sha256: expected_token,
        token_hex,
        experimental_failure: envelope.experimental_failure
            && envelope.asset_class == NEGATIVE_ASSET_CLASS_EXPERIMENTAL_FAILURE,
        target_count: envelope.targets.len(),
        project_id: envelope.project_id,
        trl_level: envelope.trl_level,
    })
}

// Data-only consistency check. It does not mint, consume, or reconstruct a
// capability. Actual SovereignGate CAS, source GCL seals, and process admission
// remain unchanged; this binds the already reported HCP action to the envelope.
fn hcp_action_matches(
    action: Option<&[u64]>,
    input_id: Option<u64>,
    token_generation: Option<u64>,
    token_nonce: Option<u64>,
    directive_bit: Option<u8>,
    depth: &shadow_contracts::negative_asset::NegativeDepthMetricsWire,
) -> bool {
    match (
        action,
        input_id,
        token_generation,
        token_nonce,
        directive_bit,
    ) {
        (
            Some(words),
            Some(expected_id),
            Some(expected_generation),
            Some(expected_nonce),
            Some(expected_bit),
        ) => {
            let family_mask = match shadow_contracts::hcp_family_admission_mask(words) {
                Ok(mask) => mask,
                Err(_) => return false,
            };
            let [id, generation, nonce, bit, sealed, lease_id, active_families] = &words[..7]
            else {
                return false;
            };
            *id == expected_id
                && *generation == expected_generation
                && *generation == depth.hcp_token_generation
                && *nonce == expected_nonce
                && *bit == u64::from(expected_bit)
                && expected_bit <= 1
                && *sealed == 1
                && *lease_id != 0
                && *active_families == u64::from(depth.active_families)
                && *active_families <= 4
                && family_mask
                    .map(|mask| ((mask & 0b0010) != 0) == depth.npro_family_admitted)
                    .unwrap_or(true)
        }
        _ => false,
    }
}

fn checked_token_for(envelope: &NegativeAssetEnvelope) -> Result<[u8; 32], String> {
    let expected_token = token_for(envelope);
    require(
        expected_token == envelope.token_sha256,
        "Negative Knowledge SHA-256 token does not recompute in Shadow",
    )?;
    Ok(expected_token)
}

fn bind_gcl_identity(
    q: &QuantumInboundWire,
    envelope: &NegativeAssetEnvelope,
) -> Result<(), String> {
    let evidence = &q.final_evidence;
    let pd = &evidence.pd_spine;
    let input_sha256 = parse_hex_32(&evidence.light_input_sha256).ok_or_else(|| {
        "Light input SHA-256 is not canonical in the Negative Knowledge gate".to_string()
    })?;
    // V189_DERIVATIVE: SovereignGate generation is a zero-based slot index.
    // Some(0) is real first issuance; None is absent evidence and must fail.
    let token_generation = q
        .hcp_token_gen
        .ok_or_else(|| "Negative Knowledge HCP token generation is absent".to_string())?;
    let admitted = u32::from(envelope.depth.npro_family_admitted);
    let family_depth = envelope
        .lim_reasoning_depth
        .saturating_mul(envelope.depth.npro_family_average)
        / shadow_contracts::MASS_SCALE;
    let evidence_depth =
        family_depth.saturating_mul(envelope.npro_avg_weakness) / shadow_contracts::MASS_SCALE;
    let expected_depth = evidence_depth.saturating_mul(admitted);
    let depth_relation = envelope.depth.applied_depth == expected_depth
        && envelope.depth.applied_depth <= envelope.depth.requested_depth
        && envelope.depth.requested_depth == envelope.lim_reasoning_depth
        && envelope.depth.hcp_token_generation == token_generation;

    require_all(
        &[
            envelope.gcl_process_digest == pd.gcl_process_digest,
            envelope.gcl_law_seal == pd.gcl_law_seal,
            envelope.gcl_system_laws_seal == pd.gcl_system_laws_seal,
            envelope.parent_i0_digest == pd.parent_i0_digest,
            envelope.input_sha256 == input_sha256,
            envelope.depth.module_coverage == shadow_contracts::MASS_SCALE,
            depth_relation,
        ],
        "Negative Knowledge token is not continuously bound to GCL/PD/HCP depth",
    )
}

fn candidate_negative_inventory_bound(
    ledger: &shadow_contracts::candidate_measurement::CandidateLedger,
    arguments: &[shadow_contracts::negative_asset::NegativeOperatorArgumentWire],
    diagnostics: &[u8],
    fragments_admitted: u64,
) -> bool {
    use shadow_contracts::candidate_measurement::CandidatePolarity;
    let measured: Vec<_> = ledger.observations().iter()
        .filter(|entry| entry.polarity == CandidatePolarity::NegativeCondition).collect();
    let mut presence = [0u8; 4];
    for entry in &measured { presence[usize::from(entry.operator - 1)] = 1; }
    // v189 Npro::activate: four actual operator computations, in this order.
    // This inventory is not four fabricated historical -Xi/-Yi candidates.
    measured.len() == 4
        && measured.iter().enumerate().all(|(index, entry)| {
            usize::from(entry.operator) == index + 1
                && entry.fragment_refs.iter().copied()
                    .eq(0..u32::from(index == 0 && fragments_admitted != 0))
        })
        && measured.len() == arguments.len()
        && measured.iter().zip(arguments).all(|(entry, argument)| {
            entry.operator == argument.operator && entry.value == argument.weakness
        })
        && diagnostics == presence
}

fn bind_module_evidence(
    q: &QuantumInboundWire,
    negative: &NegativeKnowledgeWire,
    envelope: &NegativeAssetEnvelope,
) -> Result<(), String> {
    let evidence = &q.final_evidence;
    let stage_words = |stage: u8| -> Option<&[u64]> {
        evidence
            .quantum_action_evidence
            .iter()
            .find(|entry| entry.stage == stage)
            .map(|entry| entry.evidence_words.as_slice())
    };

    let npro_bound = match stage_words(shadow_contracts::ACTION_STAGE_NPRO) {
        Some([count, average, forbidden, consensus]) => {
            *count == envelope.operator_arguments.len() as u64
                && *average == u64::from(envelope.npro_avg_weakness)
                && *forbidden == u64::from(envelope.npro_forbidden_count)
                && *consensus == u64::from(envelope.npro_consensus)
        }
        _ => false,
    };
    let npim_bound = match stage_words(shadow_contracts::ACTION_STAGE_NPIM) {
        Some([failure, contradiction, certainty, negativity, count]) => {
            *failure == u64::from(envelope.npim_failure_density)
                && *contradiction == u64::from(envelope.npim_contradiction_strength)
                && *certainty == u64::from(envelope.npim_elimination_certainty)
                && *negativity == u64::from(envelope.npim_negativity_score)
                && *count == envelope.operator_arguments.len() as u64
        }
        _ => false,
    };
    let apro_standing = evidence
        .mpro_measurements
        .get(12..16)
        .map(|values| values.iter().map(|value| u32::from(*value)).sum::<u32>())
        .unwrap_or(u32::MAX);
    let mpro_npro_mask = evidence
        .mpro_measurements
        .get(4..8)
        .map(|values| {
            values.iter().enumerate().fold(0u8, |mask, (index, value)| {
                mask | ((*value != 0) as u8) << index
            })
        })
        .unwrap_or(u8::MAX);
    let hcp_bound = hcp_action_matches(
        stage_words(shadow_contracts::ACTION_STAGE_HCP),
        q.hcp_input_id,
        q.hcp_token_gen,
        q.hcp_token_nonce,
        q.hcp_directive_bit,
        &envelope.depth,
    );
    let lim_mass = shadow_contracts::fixed_mass(q.epistemic_mass).unwrap_or(u32::MAX);
    let wire_mass = shadow_contracts::fixed_mass(negative.mass).unwrap_or(u32::MAX);
    let (mpro_negative_measure, candidates_bound) = match evidence.candidate_measurement()? {
        Some((ledger, summary)) => {
            // NPIM already carries actual operator arguments and their exact
            // IEEE-754 material. Bind inventory and value, not just an average.
            // PRO and NPRO consume the same admitted fragment prefix in the
            // producer. Bind the negative reference to that current workset,
            // not to a fabricated fragment zero. This is ledger consistency,
            // not independent hardware attestation or permission to execute.
            let pro = shadow_contracts::ProActionEvidence::try_from_words(
                stage_words(shadow_contracts::ACTION_STAGE_PRO)
                    .ok_or("Negative Knowledge PRO workset evidence is absent")?,
            ).map_err(|error| format!("Negative Knowledge PRO workset: {error}"))?;
            let bound = candidate_negative_inventory_bound(&ledger,
                &envelope.operator_arguments, &evidence.mpro_measurements[4..8],
                pro.fragments_admitted);
            (summary.negative_observed.mean.unwrap_or(0), bound)
        }
        None => (envelope.mpro_npro_mask.count_ones()
            .saturating_mul(shadow_contracts::MASS_SCALE) / 4, true),
    };
    let expected_flow = [
        (NEGATIVE_FLOW_LIM, envelope.lim_epistemic_mass),
        (NEGATIVE_FLOW_NPRO, envelope.npro_consensus),
        (NEGATIVE_FLOW_NPIM, envelope.npim_negativity_score),
        (
            NEGATIVE_FLOW_APRO,
            envelope
                .apro_standing
                .saturating_mul(shadow_contracts::MASS_SCALE)
                / 4,
        ),
        (
            NEGATIVE_FLOW_MPRO,
            mpro_negative_measure,
        ),
        (NEGATIVE_FLOW_HCP_PRO, envelope.depth.applied_depth),
    ];
    let flow_bound = envelope
        .flow_arguments
        .iter()
        .map(|argument| (argument.module, argument.measure))
        .eq(expected_flow.iter().copied());

    require_all(
        &[
            npro_bound,
            npim_bound,
            hcp_bound,
            envelope.lim_epistemic_mass == lim_mass,
            envelope.npim_failure_density == evidence.npim_failure_density,
            envelope.npim_contradiction_strength == evidence.npim_contradiction_strength,
            envelope.npim_elimination_certainty == evidence.npim_elimination_certainty,
            envelope.npim_negativity_score == evidence.npim_negativity_score,
            envelope.npim_suggestion == evidence.npim_suggestion,
            envelope.npim_negativity_score == wire_mass,
            envelope.apro_standing == apro_standing,
            envelope.mpro_npro_mask == mpro_npro_mask,
            envelope.mpro_factic_mass == evidence.mpro_factic_mass,
            envelope.depth.active_families <= 4,
            candidates_bound,
            flow_bound,
        ],
        "Negative Knowledge module evidence is disconnected from LIM/NPRO/NPIM/APRO/MPRO/HCP",
    )
}

fn bind_scientific_failure(
    q: &QuantumInboundWire,
    envelope: &NegativeAssetEnvelope,
) -> Result<(), String> {
    match q.final_evidence.scientific_project.as_ref() {
        Some(project) => {
            let evidence_sha256 = parse_hex_32(&project.evidence_sha256)
                .ok_or_else(|| "Scientific project SHA-256 is not canonical".to_string())?;
            let expected_failure_context = match project.trl_passed {
                true => String::new(),
                false => shadow_contracts::negative_asset::canonical_scientific_failure_context(
                    &project.project_title,
                    &project.domain,
                    &project.hypothesis,
                    &project.assumptions,
                    &project.lab_test_id,
                    std::slice::from_ref(&project.findings),
                    &project.description,
                ),
            };
            require_all(
                &[
                    envelope.project_id == project.context.project_id,
                    envelope.project_evidence_sha256 == evidence_sha256,
                    envelope.trl_level == project.trl_level,
                    envelope.trl_passed == project.trl_passed,
                    envelope.experimental_failure == !project.trl_passed,
                    envelope.scientific_failure_context == expected_failure_context,
                    envelope.gcl_process_digest == project.gcl_process_digest,
                ],
                "Experimental scientific failure is not bound to the GCL project/TRL evidence",
            )
        }
        None => require_all(
            &[
                envelope.project_id == 0,
                envelope.project_evidence_sha256 == [0u8; 32],
                envelope.trl_level == 0,
                !envelope.trl_passed,
                !envelope.experimental_failure,
                envelope.scientific_failure_context.is_empty(),
            ],
            "Non-project Negative Knowledge falsely claims an experimental scientific failure",
        ),
    }
}

fn bind_targets(q: &QuantumInboundWire, envelope: &NegativeAssetEnvelope) -> Result<(), String> {
    let exact_paths = envelope
        .targets
        .iter()
        .map(|target| target.path_wire())
        .eq(q.negative_paths.iter().cloned());
    let unique_targets = envelope
        .targets
        .iter()
        .map(|target| (target.space, target.term.as_str()))
        .collect::<HashSet<_>>()
        .len()
        == envelope.targets.len();
    let supported_spaces = envelope
        .targets
        .iter()
        .all(|target| matches!(target.space, NEGATIVE_SPACE_XI | NEGATIVE_SPACE_YI));

    require_all(
        &[exact_paths, unique_targets, supported_spaces],
        "Typed -Xi/-Yi targets do not match the negative paths delivered to Shadow",
    )
}

fn bind_operator_arguments(
    q: &QuantumInboundWire,
    envelope: &NegativeAssetEnvelope,
) -> Result<(), String> {
    let canonical = envelope
        .operator_arguments
        .iter()
        .map(|argument| argument.canonical_npim_argument())
        .collect::<Vec<_>>();
    require(
        canonical == q.final_evidence.npim_arguments,
        "Negative Knowledge operator arguments are not byte-equivalent to NPIM evidence",
    )
}

fn token_for(envelope: &NegativeAssetEnvelope) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(NEGATIVE_ASSET_TOKEN_DOMAIN);
    hasher.update(envelope.canonical_body());
    let mut token = [0u8; 32];
    token.copy_from_slice(&hasher.finalize());
    token
}

fn require(condition: bool, message: &str) -> Result<(), String> {
    match condition {
        true => Ok(()),
        false => Err(message.to_string()),
    }
}

fn require_all(conditions: &[bool], message: &str) -> Result<(), String> {
    require(conditions.iter().all(|condition| *condition), message)
}

fn hex_32(value: &[u8; 32]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn parse_hex_32(value: &str) -> Option<[u8; 32]> {
    match value.len() == 64 && value.chars().all(|ch| ch.is_ascii_hexdigit()) {
        true => {}
        false => return None,
    }
    let bytes = value.as_bytes();
    let mut out = [0u8; 32];
    for index in 0..32 {
        let high = nibble(bytes[index * 2])?;
        let low = nibble(bytes[index * 2 + 1])?;
        out[index] = (high << 4) | low;
    }
    Some(out)
}

fn nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn empty_workset_is_observed_without_a_fabricated_negative_reference() {
        use shadow_contracts::candidate_measurement::{CandidateLedger, CandidateObservation, CandidatePolarity};
        use shadow_contracts::negative_asset::NegativeOperatorArgumentWire;
        let values = [10000u32, 10000, 9000, 10000];
        let observations: Vec<_> = values.iter().enumerate().map(|(index, value)|
            CandidateObservation { polarity: CandidatePolarity::NegativeCondition,
                operator: index as u8 + 1, value: *value, fragment_refs: vec![] }).collect();
        let arguments: Vec<_> = values.iter().enumerate().map(|(index, value)|
            NegativeOperatorArgumentWire { operator: index as u8 + 1, weakness: *value,
                weakness_bits: (*value as f32 / 10000.0).to_bits(),
                cause: "condition observed with no fragment".into(),
                consequence: "request the missing evidence".into() }).collect();
        let ledger = CandidateLedger::new([1;32], observations).unwrap();
        assert!(super::candidate_negative_inventory_bound(&ledger, &arguments, &[1,1,1,1], 0),
            "an actually empty workset cannot supply reference zero");
        let summary = ledger.summarize(0).unwrap();
        assert_eq!(summary.negative_observed.count, 4);
        assert_eq!(summary.negative_observed.mean, Some(9750));
        assert_eq!(summary.negative_admitted.sum, 0);
        // A changed reference cannot be made valid by keeping the same mean.
        // The reported workset determines the exact allowed reference, while
        // all four condition values and their denominator stay unchanged.
        for count in [0u64, 1, 2, 128, u64::MAX] {
            for refs in [vec![], vec![0], vec![1], vec![0,0], vec![u32::MAX]] {
                let mut entries = ledger.observations().to_vec();
                entries[0].fragment_refs = refs.clone();
                let changed = CandidateLedger::new([1;32], entries).unwrap();
                let expected = if count == 0 { refs.is_empty() } else { refs == [0] };
                assert_eq!(super::candidate_negative_inventory_bound(
                    &changed, &arguments, &[1,1,1,1], count), expected);
                assert_eq!(changed.summarize(0).unwrap(), summary);
            }
        }
    }


    #[test]
    fn candidate_negative_inventory_keeps_zero_and_rejects_same_mean_substitutions() {
        use shadow_contracts::candidate_measurement::{CandidateLedger, CandidateObservation, CandidatePolarity};
        use shadow_contracts::negative_asset::NegativeOperatorArgumentWire;
        let values = [0u32, 2000, 4000, 6000];
        let observations: Vec<_> = values.iter().enumerate().map(|(index, value)|
            CandidateObservation { polarity: CandidatePolarity::NegativeCondition,
                operator: index as u8 + 1, value: *value,
                fragment_refs: if index == 0 { vec![0] } else { vec![] } }).collect();
        let arguments: Vec<_> = values.iter().enumerate().map(|(index, value)|
            NegativeOperatorArgumentWire { operator: index as u8 + 1, weakness: *value,
                weakness_bits: (*value as f32 / 10000.0).to_bits(),
                cause: "measured operator concern".into(), consequence: "requires evidence".into() }).collect();
        let check = |entries: Vec<CandidateObservation>| {
            super::candidate_negative_inventory_bound(
                &CandidateLedger::new([1;32], entries).unwrap(), &arguments, &[1,1,1,1], 1)
        };
        assert!(check(observations.clone()));
        let mut changed = observations.clone();
        changed.remove(0); // removing an actually measured zero changes the denominator
        assert!(!check(changed));
        let mut changed = observations.clone();
        changed.swap(1,2);
        assert!(!check(changed));
        let mut changed = observations.clone();
        changed[1].value += 1000;
        changed[2].value -= 1000; // same sum and average, different individual evidence
        assert!(!check(changed));
        let mut changed = observations;
        changed[0].fragment_refs = vec![1];
        assert!(!check(changed));
    }
    use super::*;

    fn sample_envelope() -> NegativeAssetEnvelope {
        NegativeAssetEnvelope {
            version: shadow_contracts::negative_asset::NEGATIVE_ASSET_VERSION,
            module_mask: shadow_contracts::negative_asset::NEGATIVE_MODULE_REQUIRED,
            asset_class: shadow_contracts::negative_asset::NEGATIVE_ASSET_CLASS_KNOWLEDGE,
            gcl_process_digest: 1,
            gcl_law_seal: 2,
            gcl_system_laws_seal: 3,
            parent_i0_digest: 4,
            input_sha256: [5u8; 32],
            lim_epistemic_mass: 1,
            lim_reasoning_depth: 1,
            npro_avg_weakness: 1,
            npro_consensus: 1,
            npro_forbidden_count: 0,
            npim_failure_density: 1,
            npim_contradiction_strength: 1,
            npim_elimination_certainty: 1,
            npim_negativity_score: 1,
            npim_suggestion: 0,
            apro_standing: 0,
            mpro_npro_mask: 0,
            mpro_factic_mass: 1,
            depth: shadow_contracts::negative_asset::NegativeDepthMetricsWire {
                requested_depth: 1,
                applied_depth: 0,
                npro_family_average: shadow_contracts::MASS_SCALE,
                npro_family_admitted: false,
                module_coverage: shadow_contracts::MASS_SCALE,
                target_density: 0,
                active_families: 0,
                hcp_token_generation: 1,
            },
            project_id: 0,
            project_evidence_sha256: [0u8; 32],
            trl_level: 0,
            trl_passed: false,
            experimental_failure: false,
            scientific_failure_context: String::new(),
            targets: Vec::new(),
            operator_arguments: Vec::new(),
            flow_arguments: vec![
                shadow_contracts::negative_asset::NegativeFlowArgumentWire {
                    module: NEGATIVE_FLOW_LIM,
                    measure: 1,
                    claim: "LIM bound".to_string(),
                },
                shadow_contracts::negative_asset::NegativeFlowArgumentWire {
                    module: NEGATIVE_FLOW_NPRO,
                    measure: 1,
                    claim: "NPRO bound".to_string(),
                },
                shadow_contracts::negative_asset::NegativeFlowArgumentWire {
                    module: NEGATIVE_FLOW_NPIM,
                    measure: 1,
                    claim: "NPIM bound".to_string(),
                },
                shadow_contracts::negative_asset::NegativeFlowArgumentWire {
                    module: NEGATIVE_FLOW_APRO,
                    measure: 0,
                    claim: "APRO bound".to_string(),
                },
                shadow_contracts::negative_asset::NegativeFlowArgumentWire {
                    module: NEGATIVE_FLOW_MPRO,
                    measure: 0,
                    claim: "MPRO bound".to_string(),
                },
                shadow_contracts::negative_asset::NegativeFlowArgumentWire {
                    module: NEGATIVE_FLOW_HCP_PRO,
                    measure: 0,
                    claim: "HCP_PRO bound".to_string(),
                },
            ],
            token_sha256: [0u8; 32],
        }
    }

    #[test]
    fn domain_separated_token_is_stable_for_the_same_body() {
        let mut envelope = sample_envelope();
        let first = token_for(&envelope);
        let second = token_for(&envelope);
        envelope.token_sha256 = first;
        assert_eq!(first, second);
        assert_ne!(first, [0u8; 32]);
    }

    #[test]
    fn zero_generation_is_present_evidence_not_an_absence_sentinel() {
        let mut envelope = sample_envelope();
        envelope.depth.hcp_token_generation = 0;
        envelope.token_sha256 = token_for(&envelope);
        assert!(envelope.shape_is_valid(shadow_contracts::MASS_SCALE));
        assert!(checked_token_for(&envelope).is_ok());
        let action = [21, 0, 0x4843_5050_524F_0001, 0, 1, 23, 0];
        let matches = |id, generation, nonce, bit| {
            hcp_action_matches(Some(&action), id, generation, nonce, bit, &envelope.depth)
        };
        assert!(matches(Some(action[0]), Some(0), Some(action[2]), Some(0)));
        assert!(!matches(Some(action[0]), None, Some(action[2]), Some(0)));
        assert!(!matches(None, Some(0), Some(action[2]), Some(0)));
        assert!(!matches(Some(action[0]), Some(0), None, Some(0)));
        assert!(!matches(Some(action[0]), Some(0), Some(action[2]), None));
        assert!(!hcp_action_matches(
            None,
            Some(21),
            Some(0),
            Some(action[2]),
            Some(0),
            &envelope.depth
        ));
    }

    #[test]
    fn hcp_action_must_match_every_reported_field_and_sealed_lease() {
        let mut envelope = sample_envelope();
        envelope.depth.hcp_token_generation = 0;
        let action = [21, 0, 0x4843_5050_524F_0001, 0, 1, 23, 0];
        for (index, replacement) in [(0, 22), (1, 1), (2, 9), (3, 1), (4, 0), (5, 0), (6, 1)] {
            let mut altered = action;
            altered[index] = replacement;
            assert!(
                !hcp_action_matches(
                    Some(&altered),
                    Some(action[0]),
                    Some(0),
                    Some(action[2]),
                    Some(0),
                    &envelope.depth,
                ),
                "altered HCP action field {index}"
            );
        }
        envelope.depth.hcp_token_generation = 1;
        assert!(!hcp_action_matches(
            Some(&action),
            Some(action[0]),
            Some(0),
            Some(action[2]),
            Some(0),
            &envelope.depth,
        ));
    }

    #[test]
    fn extended_hcp_family_mask_binds_actual_negative_admission() {
        let mut envelope = sample_envelope();
        envelope.depth.hcp_token_generation = 0;
        fn matches(
            words: &[u64],
            depth: &shadow_contracts::negative_asset::NegativeDepthMetricsWire,
        ) -> bool {
            hcp_action_matches(Some(words), Some(21), Some(0), Some(23), Some(0), depth)
        }
        assert!(matches(&[21, 0, 23, 0, 1, 5, 0, 0], &envelope.depth));
        assert!(!matches(&[21, 0, 23, 0, 1, 5, 1, 0], &envelope.depth));
        envelope.depth.active_families = 2;
        assert!(
            !matches(&[21, 0, 23, 0, 1, 5, 2, 6], &envelope.depth),
            "NPRO admission was not reported"
        );
        envelope.depth.npro_family_admitted = true;
        assert!(matches(&[21, 0, 23, 0, 1, 5, 2, 6], &envelope.depth));
        assert!(
            !matches(&[21, 0, 23, 0, 1, 5, 2, 5], &envelope.depth),
            "NPRO bit cannot disappear"
        );
        assert!(!matches(&[21, 0, 23, 0, 1, 5, 2, 22], &envelope.depth));
    }

    #[test]
    fn zero_generation_does_not_bypass_canonical_token_recomputation() {
        let mut envelope = sample_envelope();
        envelope.depth.hcp_token_generation = 0;
        envelope.token_sha256 = token_for(&envelope);
        assert!(checked_token_for(&envelope).is_ok());
        let mut altered_body = envelope.clone();
        altered_body.depth.hcp_token_generation = 1;
        assert!(checked_token_for(&altered_body).is_err());
        envelope.token_sha256[0] ^= 1;
        assert!(checked_token_for(&envelope).is_err());
    }

    #[test]
    fn token_binds_project_target_operator_and_process_material() {
        let baseline = sample_envelope();
        let baseline_token = token_for(&baseline);

        let mut project_tamper = baseline.clone();
        project_tamper.project_evidence_sha256 = [9u8; 32];
        assert_ne!(baseline_token, token_for(&project_tamper));

        let mut target_tamper = baseline.clone();
        target_tamper.targets.push(
            shadow_contracts::negative_asset::NegativeTargetEvidenceWire {
                term: "target".to_string(),
                space: NEGATIVE_SPACE_XI,
                operator: shadow_contracts::negative_asset::NEGATIVE_OPERATOR_TRACE,
                weakness: 1,
                fragment_refs: vec![1],
                cause: "cause".to_string(),
            },
        );
        assert_ne!(baseline_token, token_for(&target_tamper));

        let mut operator_tamper = baseline.clone();
        operator_tamper.operator_arguments.push(
            shadow_contracts::negative_asset::NegativeOperatorArgumentWire {
                operator: shadow_contracts::negative_asset::NEGATIVE_OPERATOR_TRACE,
                weakness: 1,
                weakness_bits: 0.0f32.to_bits(),
                cause: "cause".to_string(),
                consequence: "consequence".to_string(),
            },
        );
        assert_ne!(baseline_token, token_for(&operator_tamper));

        let mut process_tamper = baseline;
        process_tamper.gcl_process_digest = 99;
        assert_ne!(baseline_token, token_for(&process_tamper));
    }
}
