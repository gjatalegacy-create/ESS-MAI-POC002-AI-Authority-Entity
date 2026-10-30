//! AIGovern-Cyber v1.7.1 — living Negative Knowledge lifecycle in Quantum.
//!
//! The module closes the operator-to-term discontinuity without changing the
//! authority order. NPRO still measures, NPIM still packages, HCP_PRO still
//! orchestrates depth, and Shadow remains the sole verifier and persistent
//! writer. The output retains typed operator observations and the original
//! primitive spaces, plus a GCL-bound SHA-256 asset envelope. An observation
//! alone is not a proposition-level scientific refutation.

use sha2::{Digest, Sha256};
use shadow_contracts::negative_asset::{
    NegativeAssetEnvelope, NegativeDepthMetricsWire, NegativeFlowArgumentWire,
    NegativeOperatorArgumentWire, NegativeTargetEvidenceWire,
    NEGATIVE_ASSET_CLASS_EXPERIMENTAL_FAILURE, NEGATIVE_ASSET_CLASS_KNOWLEDGE,
    NEGATIVE_ASSET_TOKEN_DOMAIN, NEGATIVE_ASSET_VERSION, NEGATIVE_FLOW_APRO, NEGATIVE_FLOW_HCP_PRO,
    NEGATIVE_FLOW_LIM, NEGATIVE_FLOW_MPRO, NEGATIVE_FLOW_NPIM, NEGATIVE_FLOW_NPRO,
    NEGATIVE_FLOW_REQUIRED, NEGATIVE_MODULE_REQUIRED, NEGATIVE_OPERATOR_CONTRAST,
    NEGATIVE_OPERATOR_MIRROR, NEGATIVE_OPERATOR_REVERSE, NEGATIVE_OPERATOR_TRACE,
    NEGATIVE_SPACE_XI, NEGATIVE_SPACE_YI,
};

use crate::lab_contracts::collapse::{NegativePath, PrimitiveSplit};
use crate::npim::NegativeKnowledgePackage;
use crate::npro::{NegativeElimination, NproResult};

#[derive(Debug, Clone, PartialEq)]
pub struct NegativeBinding {
    pub targets: Vec<NegativeTargetEvidenceWire>,
    pub paths: Vec<NegativePath>,
    /// R10: original Xi/Yi retained verbatim. The legacy field name does not
    /// authorize deletion; unresolved observations stop proposal in main.
    pub surviving_split: PrimitiveSplit,
}

impl NegativeBinding {
    pub fn target_density(&self, original: &PrimitiveSplit) -> f32 {
        let total = original.xi.len().saturating_add(original.yi.len()).max(1) as f32;
        (self.targets.len() as f32 / total).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HcpNegativeDepthMetrics {
    pub requested_depth: u32,
    pub applied_depth: u32,
    pub npro_family_average: u32,
    pub npro_family_admitted: bool,
    pub module_coverage: u32,
    pub target_density: u32,
    pub active_families: u32,
    pub hcp_token_generation: u64,
}

impl HcpNegativeDepthMetrics {
    pub fn try_measure(
        reasoning_depth_allowance: f32,
        npro_family_average: f32,
        npro_family_admitted: bool,
        npro_evidence_strength: f32,
        target_density: f32,
        active_families: usize,
        hcp_token_generation: u64,
    ) -> Option<Self> {
        match unit_values_are_valid(&[
            reasoning_depth_allowance,
            npro_family_average,
            npro_evidence_strength,
            target_density,
        ]) {
            true => Some(Self::measure(
                reasoning_depth_allowance,
                npro_family_average,
                npro_family_admitted,
                npro_evidence_strength,
                target_density,
                active_families,
                hcp_token_generation,
            )),
            false => None,
        }
    }

    pub fn measure(
        reasoning_depth_allowance: f32,
        npro_family_average: f32,
        npro_family_admitted: bool,
        npro_evidence_strength: f32,
        target_density: f32,
        active_families: usize,
        hcp_token_generation: u64,
    ) -> Self {
        let admitted = u32::from(npro_family_admitted);
        let requested_depth = fixed(reasoning_depth_allowance);
        let family_average = fixed(npro_family_average);
        let evidence_strength = fixed(npro_evidence_strength);
        let family_depth =
            requested_depth.saturating_mul(family_average) / shadow_contracts::MASS_SCALE;
        let evidence_depth =
            family_depth.saturating_mul(evidence_strength) / shadow_contracts::MASS_SCALE;
        Self {
            requested_depth,
            applied_depth: evidence_depth.saturating_mul(admitted),
            npro_family_average: family_average,
            npro_family_admitted,
            module_coverage: shadow_contracts::MASS_SCALE,
            target_density: fixed(target_density),
            active_families: active_families.min(4) as u32,
            hcp_token_generation,
        }
    }

    pub fn to_wire(self) -> NegativeDepthMetricsWire {
        NegativeDepthMetricsWire {
            requested_depth: self.requested_depth,
            applied_depth: self.applied_depth,
            npro_family_average: self.npro_family_average,
            npro_family_admitted: self.npro_family_admitted,
            module_coverage: self.module_coverage,
            target_density: self.target_density,
            active_families: self.active_families,
            hcp_token_generation: self.hcp_token_generation,
        }
    }
}

pub struct NegativeAssetInputs<'a> {
    pub gcl_process_digest: u64,
    pub gcl_law_seal: u64,
    pub gcl_system_laws_seal: u64,
    pub parent_i0_digest: u64,
    pub input_sha256_hex: &'a str,
    pub lim_epistemic_mass: f32,
    pub lim_reasoning_depth: f32,
    pub npro: &'a NproResult,
    pub npro_consensus: f32,
    pub npim: &'a NegativeKnowledgePackage,
    pub npim_suggestion: u8,
    pub apro_standing: u32,
    pub npro_measurements: [u8; 4],
    pub mpro_factic_mass: u32,
    pub depth: HcpNegativeDepthMetrics,
    pub project_id: u64,
    pub project_evidence_sha256_hex: &'a str,
    pub project_failure_context: &'a str,
    pub trl_level: u8,
    pub trl_passed: bool,
    pub binding: &'a NegativeBinding,
}

pub struct NegativeAssetBuilder;

impl NegativeAssetBuilder {
    pub fn bind_negative_spaces(
        split: &PrimitiveSplit,
        fragments: &[String],
        npro: &NproResult,
    ) -> NegativeBinding {
        let xi_targets = split.xi.iter().filter_map(|term| {
            strongest_target(
                term,
                NEGATIVE_SPACE_XI,
                fragments,
                &npro.eliminations,
                &npro.weakness_signal.forbidden_paths,
                npro.weakness_signal.weakness_ceiling,
            )
        });
        let yi_targets = split.yi.iter().filter_map(|term| {
            strongest_target(
                term,
                NEGATIVE_SPACE_YI,
                fragments,
                &npro.eliminations,
                &npro.weakness_signal.forbidden_paths,
                npro.weakness_signal.weakness_ceiling,
            )
        });
        let targets = xi_targets.chain(yi_targets).collect::<Vec<_>>();

        // NPRO measures operator weakness. Its lexical binding records what
        // needs evidence; it cannot establish a semantic counterproof for a
        // supplied term. Keep both primitive spaces and every observation.
        let surviving_split = split.clone();
        let paths = targets
            .iter()
            .map(|target| NegativePath {
                term: target.term.clone(),
                space: match target.space {
                    NEGATIVE_SPACE_XI => "Xi",
                    NEGATIVE_SPACE_YI => "Yi",
                    _ => "Invalid",
                },
                reason: target.path_reason(),
            })
            .collect();

        NegativeBinding {
            targets,
            paths,
            surviving_split,
        }
    }

    pub fn build(inputs: NegativeAssetInputs<'_>) -> Option<NegativeAssetEnvelope> {
        let numeric_shape_is_valid = unit_values_are_valid(&[
            inputs.lim_epistemic_mass,
            inputs.lim_reasoning_depth,
            inputs.npro.weakness_signal.avg_weakness,
            inputs.npro.weakness_signal.weakness_ceiling,
            inputs.npro_consensus,
            inputs.npim.negativity.failure_density,
            inputs.npim.negativity.contradiction_strength,
            inputs.npim.negativity.elimination_certainty,
            inputs.npim.negativity.negativity_score,
        ]) && inputs
            .npro
            .eliminations
            .iter()
            .all(|entry| unit_values_are_valid(&[entry.weakness_score]))
            && inputs
                .npim
                .arguments
                .iter()
                .all(|entry| unit_values_are_valid(&[entry.weakness_score]))
            && inputs
                .npim
                .target_arguments
                .iter()
                .all(|entry| unit_values_are_valid(&[entry.weakness_score]));
        match numeric_shape_is_valid {
            true => {}
            false => return None,
        }
        let input_sha256 = parse_hex_32(inputs.input_sha256_hex)?;
        let project_evidence_sha256 = match inputs.project_id {
            0 => [0u8; 32],
            _ => parse_hex_32(inputs.project_evidence_sha256_hex)?,
        };
        let experimental_failure = inputs.project_id != 0 && !inputs.trl_passed;
        let asset_class = match experimental_failure {
            true => NEGATIVE_ASSET_CLASS_EXPERIMENTAL_FAILURE,
            false => NEGATIVE_ASSET_CLASS_KNOWLEDGE,
        };
        let mpro_npro_mask = inputs
            .npro_measurements
            .iter()
            .enumerate()
            .fold(0u8, |mask, (index, value)| {
                mask | ((*value != 0) as u8) << index
            });
        let packaged_targets = inputs
            .npim
            .target_arguments
            .iter()
            .map(
                |argument| match fixed(argument.weakness_score) == argument.weakness_fixed {
                    true => Some(NegativeTargetEvidenceWire {
                        term: argument.term.clone(),
                        space: argument.space,
                        operator: argument.operator,
                        weakness: argument.weakness_fixed,
                        fragment_refs: argument.fragment_refs.clone(),
                        cause: argument.cause.clone(),
                    }),
                    false => None,
                },
            )
            .collect::<Option<Vec<_>>>()?;
        match packaged_targets == inputs.binding.targets {
            true => {}
            false => return None,
        }

        let operator_arguments = inputs
            .npim
            .arguments
            .iter()
            .map(|argument| {
                operator_code(&argument.operator).map(|operator| NegativeOperatorArgumentWire {
                    operator,
                    weakness: fixed(argument.weakness_score),
                    weakness_bits: argument.weakness_score.to_bits(),
                    cause: argument.cause.clone(),
                    consequence: argument.consequence.clone(),
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let flow_arguments = inputs
            .npim
            .flow_arguments
            .iter()
            .map(|argument| {
                flow_module_code(argument.module).map(|module| NegativeFlowArgumentWire {
                    module,
                    measure: argument.measure,
                    claim: argument.claim.to_string(),
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let flow_order_is_canonical = flow_arguments
            .iter()
            .map(|argument| argument.module)
            .eq(NEGATIVE_FLOW_REQUIRED.iter().copied());
        match flow_order_is_canonical {
            true => {}
            false => return None,
        }

        let mut envelope = NegativeAssetEnvelope {
            version: NEGATIVE_ASSET_VERSION,
            module_mask: NEGATIVE_MODULE_REQUIRED,
            asset_class,
            gcl_process_digest: inputs.gcl_process_digest,
            gcl_law_seal: inputs.gcl_law_seal,
            gcl_system_laws_seal: inputs.gcl_system_laws_seal,
            parent_i0_digest: inputs.parent_i0_digest,
            input_sha256,
            lim_epistemic_mass: fixed(inputs.lim_epistemic_mass),
            lim_reasoning_depth: fixed(inputs.lim_reasoning_depth),
            npro_avg_weakness: fixed(inputs.npro.weakness_signal.avg_weakness),
            npro_consensus: fixed(inputs.npro_consensus),
            npro_forbidden_count: inputs.npro.weakness_signal.forbidden_paths.len() as u32,
            npim_failure_density: fixed(inputs.npim.negativity.failure_density),
            npim_contradiction_strength: fixed(inputs.npim.negativity.contradiction_strength),
            npim_elimination_certainty: fixed(inputs.npim.negativity.elimination_certainty),
            npim_negativity_score: fixed(inputs.npim.negativity.negativity_score),
            npim_suggestion: inputs.npim_suggestion,
            apro_standing: inputs.apro_standing,
            mpro_npro_mask,
            mpro_factic_mass: inputs.mpro_factic_mass,
            depth: inputs.depth.to_wire(),
            project_id: inputs.project_id,
            project_evidence_sha256,
            trl_level: match inputs.project_id {
                0 => 0,
                _ => inputs.trl_level.min(3),
            },
            trl_passed: match inputs.project_id {
                0 => false,
                _ => inputs.trl_passed,
            },
            experimental_failure,
            scientific_failure_context: match experimental_failure {
                true => inputs.project_failure_context.trim().to_string(),
                false => String::new(),
            },
            targets: packaged_targets,
            operator_arguments,
            flow_arguments,
            token_sha256: [0u8; 32],
        };
        let body = envelope.canonical_body();
        let mut hasher = Sha256::new();
        hasher.update(NEGATIVE_ASSET_TOKEN_DOMAIN);
        hasher.update(&body);
        envelope.token_sha256.copy_from_slice(&hasher.finalize());
        Some(envelope)
    }

    pub fn token_hex(envelope: &NegativeAssetEnvelope) -> String {
        envelope
            .token_sha256
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

// SOURCE-DERIVED: Light Normalizer::content_words returns lowercased
// whitespace tokens with their punctuation intact (lingua.rs183-192).
// Xi/Yi retains that exact token identity; absence must use the same boundary.
// This checks occurrence only, not truth, relevance, causality or authority.
fn canonical_primitive_term_present(fragment: &str, canonical_term: &str) -> bool {
    fragment
        .split_whitespace()
        .any(|token| token.to_lowercase() == canonical_term)
}

fn strongest_target(
    term: &str,
    space: u8,
    fragments: &[String],
    eliminations: &[NegativeElimination],
    forbidden_paths: &[String],
    ceiling: f32,
) -> Option<NegativeTargetEvidenceWire> {
    let fragment_presence = fragments
        .iter()
        .map(|fragment| {
            canonical_primitive_term_present(fragment, term)
        })
        .collect::<Vec<_>>();
    let frequency = fragment_presence.iter().filter(|present| **present).count() as f32
        / fragments.len().max(1) as f32;

    eliminations
        .iter()
        .filter(|elimination| {
            forbidden_paths
                .iter()
                .any(|path| path == &elimination.operator)
        })
        .filter_map(|elimination| {
            let operator = operator_code(&elimination.operator)?;
            let factor = operator_factor(
                operator,
                frequency,
                &fragment_presence,
                &elimination.fragment_refs,
            );
            let weakness = (elimination.weakness_score * factor).clamp(0.0, 1.0);
            match weakness > ceiling {
                true => Some(NegativeTargetEvidenceWire {
                    term: term.to_string(),
                    space,
                    operator,
                    weakness: fixed(weakness),
                    fragment_refs: matching_refs(&fragment_presence, &elimination.fragment_refs),
                    cause: format!(
                        "{}; term_factor={:.4}; gcl_ceiling={:.4}",
                        elimination.cause, factor, ceiling,
                    ),
                }),
                false => None,
            }
        })
        .max_by(|left, right| {
            left.weakness
                .cmp(&right.weakness)
                .then_with(|| right.operator.cmp(&left.operator))
        })
}

fn operator_factor(
    operator: u8,
    frequency: f32,
    fragment_presence: &[bool],
    fragment_refs: &[usize],
) -> f32 {
    let referenced = match fragment_refs.is_empty() {
        true => 1.0 - frequency,
        false => {
            let hits = fragment_refs
                .iter()
                .filter(|index| fragment_presence.get(**index).copied().unwrap_or(false))
                .count() as f32;
            hits / fragment_refs.len().max(1) as f32
        }
    };
    let discrimination = (4.0 * frequency * (1.0 - frequency)).clamp(0.0, 1.0);
    (match operator {
        NEGATIVE_OPERATOR_REVERSE => referenced,
        NEGATIVE_OPERATOR_MIRROR => 1.0 - frequency,
        NEGATIVE_OPERATOR_CONTRAST => 1.0 - discrimination,
        NEGATIVE_OPERATOR_TRACE => 1.0 - frequency,
        _ => 0.0,
    })
    .clamp(0.0, 1.0)
}

fn matching_refs(fragment_presence: &[bool], declared: &[usize]) -> Vec<u32> {
    let declared_matches = declared
        .iter()
        .filter(|index| fragment_presence.get(**index).copied().unwrap_or(false))
        .filter_map(|index| u32::try_from(*index).ok())
        .collect::<Vec<_>>();
    match declared_matches.is_empty() {
        true => fragment_presence
            .iter()
            .enumerate()
            .filter(|(_, present)| **present)
            .filter_map(|(index, _)| u32::try_from(index).ok())
            .collect(),
        false => declared_matches,
    }
}

fn flow_module_code(name: &str) -> Option<u8> {
    match name {
        "LIM" => Some(NEGATIVE_FLOW_LIM),
        "NPRO" => Some(NEGATIVE_FLOW_NPRO),
        "NPIM" => Some(NEGATIVE_FLOW_NPIM),
        "APRO" => Some(NEGATIVE_FLOW_APRO),
        "MPRO" => Some(NEGATIVE_FLOW_MPRO),
        "HCP_PRO" => Some(NEGATIVE_FLOW_HCP_PRO),
        _ => None,
    }
}

pub fn operator_code(name: &str) -> Option<u8> {
    match name {
        "Reverse" | "NReverse" | "REVERSE" => Some(NEGATIVE_OPERATOR_REVERSE),
        "Mirror" | "NMirror" | "MIRROR" => Some(NEGATIVE_OPERATOR_MIRROR),
        "Contrast" | "NContrast" | "CONTRAST" => Some(NEGATIVE_OPERATOR_CONTRAST),
        "Trace" | "NTrace" | "TRACE" => Some(NEGATIVE_OPERATOR_TRACE),
        _ => None,
    }
}

fn fixed(value: f32) -> u32 {
    shadow_contracts::fixed_mass(value).unwrap_or(0)
}

fn unit_values_are_valid(values: &[f32]) -> bool {
    values
        .iter()
        .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
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
    use super::*;
    use crate::npro::NproEngine;
    use crate::npro_hardening::NproHardening;


    #[test]
    fn encoded_negative_paths_keep_typed_reason_and_exact_primitive_terms() {
        let split = PrimitiveSplit {
            xi: vec!["dëshmi,~|shqip".into()],
            yi: vec!["claim,~|other".into()],
        };
        let fragments = vec![format!("{} {}", split.xi[0], split.yi[0])];
        let raw = NproEngine::activate(&fragments, 0.2, 0.2);
        let (npro, _) = NproHardening::strengthen_npro(raw);
        let binding = NegativeAssetBuilder::bind_negative_spaces(&split, &fragments, &npro);
        assert!(!binding.targets.is_empty());
        assert_eq!(binding.surviving_split, split);
        assert_eq!(binding.paths.len(), binding.targets.len());
        for (path, target) in binding.paths.iter().zip(&binding.targets) {
            assert_eq!(path.term, target.term);
            assert_eq!(path.reason, target.path_reason());
            assert!(path.reason.contains("^-1:gcl_target:weakness="));
            assert_eq!(path.to_wire(), target.path_wire(), "Shadow's typed-envelope comparison stays exact");
            assert!(path.to_wire().starts_with("~np2:"));
            assert_eq!(NegativePath::from_wire(&path.to_wire()).unwrap(), *path);
        }
    }
    #[test]
    fn operator_evidence_binds_to_concrete_terms_and_preserves_space() {
        let fragments = vec!["isolated claim".to_string()];
        let split = PrimitiveSplit {
            xi: vec!["isolated".to_string()],
            yi: vec!["claim".to_string()],
        };
        let raw = NproEngine::activate(&fragments, 0.2, 0.2);
        let (npro, _) = NproHardening::strengthen_npro(raw);
        let binding = NegativeAssetBuilder::bind_negative_spaces(&split, &fragments, &npro);
        assert!(!binding.targets.is_empty());
        assert_eq!(binding.surviving_split, split);
        assert_eq!(binding.paths.len(), binding.targets.len());
    }

    #[test]
    fn r10_operator_observations_preserve_primitive_candidates() {
        // A low operator score witnesses uncertainty, not a scientific
        // counterproof that authorizes deleting the supplied Xi/Yi terms.
        let fragments = vec!["isolated claim".to_string()];
        let split = PrimitiveSplit {
            xi: vec!["isolated".to_string()],
            yi: vec!["claim".to_string()],
        };
        let raw = NproEngine::activate(&fragments, 0.2, 0.2);
        let (npro, _) = NproHardening::strengthen_npro(raw);
        let binding = NegativeAssetBuilder::bind_negative_spaces(&split, &fragments, &npro);
        assert!(!binding.targets.is_empty(), "the uncertainty must remain witnessed");
        assert_eq!(binding.paths.len(), binding.targets.len());
        assert_eq!(binding.surviving_split, split,
            "R10_OPERATOR_OBSERVATIONS_MUST_NOT_DELETE_ORIGINAL_XI_YI");
    }

    #[test]
    fn hcp_negative_depth_uses_exact_fixed_point_admission() {
        let depth = HcpNegativeDepthMetrics::measure(0.8, 0.8, true, 0.7, 0.5, 4, 2);
        assert_eq!(depth.requested_depth, 8_000);
        assert_eq!(depth.npro_family_average, 8_000);
        assert_eq!(depth.applied_depth, 4_480);
        assert!(depth.npro_family_admitted);
    }

    #[test]
    fn operator_codes_accept_existing_and_legacy_names() {
        assert_eq!(operator_code("Reverse"), Some(NEGATIVE_OPERATOR_REVERSE));
        assert_eq!(operator_code("NReverse"), Some(NEGATIVE_OPERATOR_REVERSE));
        assert_eq!(operator_code("unknown"), None);
    }
    #[test]
    fn primitive_presence_keeps_source_punctuation_and_unicode_identity() {
        let fragment = "CLAIM=ABC MODEL/INPUT MODEL_CONDITIONAL residual<tol observation. PËRMBLEDHJE";
        for term in ["claim=abc", "model/input", "model_conditional", "residual<tol", "observation.", "përmbledhje"] {
            assert!(canonical_primitive_term_present(fragment, term), "{term}");
        }
        for absent in ["claim=abd", "abc", "input", "conditional", "residual", "observation", "përmbledh", "claim=abc model/input"] {
            assert!(!canonical_primitive_term_present(fragment, absent), "{absent}");
        }
        assert!(canonical_primitive_term_present("  CLAIM=ABC\tother\n", "claim=abc"));
        assert!(!canonical_primitive_term_present("prefixclaim=abc claim=abcd", "claim=abc"));
        assert!(!canonical_primitive_term_present("", "claim=abc"));
    }

    #[test]
    fn exact_token_presence_drives_real_target_factor_without_changing_gates() {
        // Controlled operator fixture: no GCL command or knowledge acceptance.
        let fragments = vec!["CLAIM=ABC motor".into(), "other material".into()];
        let contrast = NegativeElimination {
            elimination_id: "test-contrast".into(), operator: "Contrast".into(),
            weakness_score: 1.0, cause: "controlled contrast weakness".into(), fragment_refs: vec![],
        };
        // Exact occurrence in one of two fragments gives contrast factor 0;
        // the old punctuation splitter falsely gave frequency 0 and factor 1.
        assert!(strongest_target("claim=abc", NEGATIVE_SPACE_XI, &fragments,
            &[contrast.clone()], &["Contrast".into()], 0.65).is_none());
        // True absence still follows the original contrast rule unchanged.
        assert!(strongest_target("claim=abd", NEGATIVE_SPACE_XI, &fragments,
            &[contrast], &["Contrast".into()], 0.65).is_some());
        let reverse = NegativeElimination {
            elimination_id: "test-reverse".into(), operator: "Reverse".into(),
            weakness_score: 1.0, cause: "controlled reverse weakness".into(), fragment_refs: vec![0],
        };
        let target = strongest_target("claim=abc", NEGATIVE_SPACE_XI, &fragments,
            &[reverse.clone()], &["Reverse".into()], 0.65).unwrap();
        assert_eq!(target.fragment_refs, vec![0]);
        assert_eq!(target.weakness, 10000);
        // Occurrence alone never overrides forbidden-operator or ceiling gates.
        assert!(strongest_target("claim=abc", NEGATIVE_SPACE_XI, &fragments,
            &[reverse.clone()], &[], 0.65).is_none());
        assert!(strongest_target("claim=abc", NEGATIVE_SPACE_XI, &fragments,
            &[reverse], &["Reverse".into()], 1.0).is_none());
    }

}
