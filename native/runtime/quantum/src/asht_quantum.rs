//! Asht Quantum: subordinate evidence-scoping muscle commanded by UCL.

use quantum_lim::candidate_weight::{
    candidate_mass, candidate_order, evaluate_parity, rank_candidates, ParityEvidence,
    ParityVerdict,
};
use quantum_lim::lab_contracts::{CollapsePhase, GjataCollapseLaw};
use quantum_lim::ultimatum_collapse_law::{AshtCommand, AshtOperatorDirective};
use sha2::{Digest, Sha256};
use shadow_contracts::{
    CandidateStatisticWire, KnowledgeSelectionRequestWire, KnowledgeSelectionResponseWire,
    MuscleEvidenceWire, SelectionRequester,
};
use std::marker::PhantomData;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BesaCompanion {
    pub session_id: String,
    pub parent_i0: String,
    pub primitive_anchor: u64,
    pub trace_id: String,
    pub split_sha256: String,
    pub hold_id: String,
    pub hold_sha256: String,
    pub selection_sha256: String,
    pub enrichment_sha256: String,
}

impl BesaCompanion {
    pub fn attestation_sha256(&self) -> String {
        shadow_contracts::sha256_hex(&[
            b"ESSMAI/BESA/ATTESTATION/V178",
            self.session_id.as_bytes(),
            self.parent_i0.as_bytes(),
            &self.primitive_anchor.to_le_bytes(),
            self.trace_id.as_bytes(),
            self.split_sha256.as_bytes(),
            self.hold_id.as_bytes(),
            self.hold_sha256.as_bytes(),
            self.selection_sha256.as_bytes(),
            self.enrichment_sha256.as_bytes(),
        ])
    }

    pub fn shape_is_valid(&self) -> bool {
        !self.session_id.is_empty()
            && shadow_contracts::is_canonical_sha256(&self.parent_i0)
            && self.primitive_anchor != 0
            && self.trace_id == self.session_id
            && shadow_contracts::is_canonical_sha256(&self.split_sha256)
            && !self.hold_id.is_empty()
            && shadow_contracts::is_canonical_sha256(&self.hold_sha256)
            && shadow_contracts::is_canonical_sha256(&self.selection_sha256)
            && shadow_contracts::is_canonical_sha256(&self.enrichment_sha256)
    }
}

pub fn read_besa_companion(
    path: &Path,
    session_id: &str,
    input_sha256: &str,
    primitive_anchor: u64,
    split: &quantum_lim::lab_contracts::PrimitiveSplit,
) -> Result<BesaCompanion, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|error| format!("Besa companion read failed: {error}"))?;
    let prefix = format!("{session_id}|");
    let line = content
        .lines()
        .filter(|candidate| candidate.starts_with(prefix.as_str()))
        .last()
        .ok_or_else(|| "Besa companion missing for session".to_string())?;
    match quantum_lim::lab_contracts::pa_wire::verify_line_generic(line, &[], 10) {
        quantum_lim::lab_contracts::pa_wire::PaWireVerdict::SealedOk => {}
        quantum_lim::lab_contracts::pa_wire::PaWireVerdict::LegacyOk => {
            return Err("Besa companion downgrade rejected".to_string())
        }
        quantum_lim::lab_contracts::pa_wire::PaWireVerdict::Corrupt(reason) => {
            return Err(format!("Besa companion corrupt: {reason}"))
        }
    }
    let body = line
        .rsplit_once("|c:")
        .map(|(value, _)| value)
        .ok_or_else(|| "Besa companion seal missing".to_string())?;
    let parts: Vec<&str> = body.split('|').collect();
    let parsed = match parts.as_slice() {
        [session, parent, pa_hex, trace, split_sha, hold_id, hold_sha, selection_sha, enrichment_sha] =>
        {
            let pa = u64::from_str_radix(pa_hex, 16)
                .map_err(|_| "Besa companion PA malformed".to_string())?;
            BesaCompanion {
                session_id: (*session).to_string(),
                parent_i0: (*parent).to_string(),
                primitive_anchor: pa,
                trace_id: (*trace).to_string(),
                split_sha256: (*split_sha).to_string(),
                hold_id: (*hold_id).to_string(),
                hold_sha256: (*hold_sha).to_string(),
                selection_sha256: (*selection_sha).to_string(),
                enrichment_sha256: (*enrichment_sha).to_string(),
            }
        }
        _ => return Err("Besa companion field count invalid".to_string()),
    };
    let expected_split_sha256 = lgc_split_sha256(split);
    match (
        parsed.shape_is_valid(),
        parsed.session_id == session_id,
        parsed.parent_i0 == input_sha256.to_ascii_lowercase(),
        parsed.primitive_anchor == primitive_anchor,
        parsed.split_sha256 == expected_split_sha256,
    ) {
        (true, true, true, true, true) => Ok(parsed),
        _ => Err("Besa companion lineage mismatch".to_string()),
    }
}

fn lgc_split_sha256(split: &quantum_lim::lab_contracts::PrimitiveSplit) -> String {
    let xi_sha = digest_terms("ESSMAI/LGC/XI/V2", &split.xi);
    let yi_sha = digest_terms("ESSMAI/LGC/YI/V2", &split.yi);
    domain_sha(
        "ESSMAI/LGC/SPLIT/V2",
        &[
            xi_sha.as_bytes(),
            yi_sha.as_bytes(),
            split.to_wire().as_bytes(),
        ],
    )
}

fn digest_terms(domain: &str, terms: &[String]) -> String {
    let mut canonical = Vec::new();
    for term in terms {
        canonical.extend_from_slice(&(term.len() as u64).to_le_bytes());
        canonical.extend_from_slice(term.as_bytes());
    }
    domain_sha(domain, &[canonical.as_slice()])
}

fn domain_sha(domain: &str, parts: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"ESSMAI-DOMAIN-SHA256-V1\0");
    hasher.update((domain.len() as u64).to_le_bytes());
    hasher.update(domain.as_bytes());
    for part in parts {
        hasher.update((part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    format!("{:x}", hasher.finalize())
}

// Canonical v1.8.9 asht_quantum.rs:319-334 reference fields and :336-344
// fragment method. The lexical domain and single-byte Xi/Yi scope are unchanged.
fn candidate_reference_sha256(
    candidate: &CandidateStatisticWire,
    reference_domain: &[u8],
    axis_or_scope: &[u8],
) -> String {
    shadow_contracts::sha256_hex(&[
        reference_domain,
        axis_or_scope,
        &candidate.candidate_id.to_le_bytes(),
        &[candidate.polarity],
        candidate.domain.as_bytes(),
        candidate.term_sha256.as_bytes(),
        candidate.source_sha256.as_bytes(),
        &[candidate.security_y],
        &[candidate.trust_x],
        &candidate.confidence_fixed.to_le_bytes(),
        &candidate.frequency.to_le_bytes(),
        &candidate.domain_count.to_le_bytes(),
        &candidate.legacy_score_fixed.to_le_bytes(),
        &candidate_mass(candidate).to_le_bytes(),
    ])
}

fn render_candidate_fragment(
    candidate: &CandidateStatisticWire,
    axis_label: &str,
    reference_sha256: &str,
) -> String {
    format!(
        "ASHT request-bound evidence axis={} domain={} polarity={} mass_fixed={} confidence_fixed={} candidate_sha256={}",
        axis_label, candidate.domain, candidate.polarity, candidate_mass(candidate),
        candidate.confidence_fixed, reference_sha256,
    )
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AshtAttestation {
    pub hold_id: String,
    pub hold_sha256: String,
    pub selection_sha256: String,
    pub relevance_sha256: String,
    pub parity: ParityVerdict,
    pub positive_mass: u64,
    pub negative_mass: u64,
    pub ranked_negative_candidates: Vec<CandidateStatisticWire>,
    /// NEW typed model origin; never stored historical candidate statistics.
    pub kie_proof: Option<shadow_contracts::kie::KieProof>,
    /// Verified before the operator lease gate; retained evidence is not
    /// permission to execute a held endpoint or a historical candidate.
    pub kie_measurement: Option<shadow_contracts::kie_measurement::KieMeasurement>,
    pub attestation_sha256: String,
    pub bounded_fragments: Vec<String>,
    pub operator_directive: AshtOperatorDirective,
}

impl AshtAttestation {
    pub fn parity_evidence(&self) -> ParityEvidence {
        ParityEvidence {
            verdict: self.parity,
            positive_mass: self.positive_mass,
            negative_mass: self.negative_mass,
        }
    }
}

pub struct Commanded;
pub struct LawGated;
pub struct ShadowHeld;

pub struct AshtQuantum<State> {
    command: AshtCommand,
    request: Option<KnowledgeSelectionRequestWire>,
    response: Option<KnowledgeSelectionResponseWire>,
    _state: PhantomData<State>,
}

impl AshtQuantum<Commanded> {
    pub fn from_command(command: AshtCommand) -> Self {
        Self {
            command,
            request: None,
            response: None,
            _state: PhantomData,
        }
    }

    pub fn commission(self) -> Result<AshtQuantum<LawGated>, String> {
        let directive = GjataCollapseLaw::issue(CollapsePhase::Reasoning);
        match (
            GjataCollapseLaw::verify(&directive, CollapsePhase::Reasoning),
            directive.seal == self.command.law_seal,
            self.command.system_laws_seal == quantum_lim::lab_contracts::system_laws_seal(),
        ) {
            (true, true, true) => Ok(AshtQuantum {
                command: self.command,
                request: None,
                response: None,
                _state: PhantomData,
            }),
            (false, _, _) => Err("Asht rejected non-Reasoning GCL directive".to_string()),
            (_, false, _) => Err("Asht rejected GCL law-seal drift".to_string()),
            (_, _, false) => Err("Asht rejected system-laws seal drift".to_string()),
        }
    }
}

impl AshtQuantum<LawGated> {
    pub fn request_evidence_candidates(
        self,
        request: KnowledgeSelectionRequestWire,
    ) -> Result<AshtQuantum<ShadowHeld>, String> {
        if self.command.route != quantum_lim::ultimatum_collapse_law::AshtEvidenceRoute::Historical
        {
            return Err("ASHT_HISTORICAL_ROUTE_COMMAND_REQUIRED".into());
        }
        match (
            request.requester,
            request.shape_is_valid(),
            request.trace_id == self.command.trace_id,
            request.split_sha256 == self.command.split_sha256,
            request.upstream_hold_sha256.as_deref() == Some(self.command.besa_hold_sha256.as_str()),
        ) {
            (SelectionRequester::AshtQuantum, true, true, true, true) => {}
            (SelectionRequester::BesaNlight, _, _, _, _) => {
                return Err("Asht cannot issue a Besa selection request".to_string())
            }
            (_, false, _, _, _) => return Err("Asht selection shape invalid".to_string()),
            (_, _, false, _, _) => return Err("Asht trace identity mismatch".to_string()),
            (_, _, _, false, _) => return Err("Asht split identity mismatch".to_string()),
            (_, _, _, _, false) => return Err("Asht lost the upstream Besa hold".to_string()),
        }
        let response = crate::shadow_process_bridge::execute_selection(&request)?;
        match response.shape_is_valid_for(&request) {
            true => Ok(AshtQuantum {
                command: self.command,
                request: Some(request),
                response: Some(response),
                _state: PhantomData,
            }),
            false => Err("Asht rejected an unbound Shadow selection response".to_string()),
        }
    }
}

impl AshtQuantum<ShadowHeld> {
    /// Data observed before attestation; no new authority or admission.
    pub fn witness_material(&self) -> Result<(Vec<u8>, Vec<u8>), String> {
        let request = self
            .request
            .as_ref()
            .ok_or("ASHT_WITNESS_REQUEST_MISSING")?;
        let response = self
            .response
            .as_ref()
            .ok_or("ASHT_WITNESS_RESPONSE_MISSING")?;
        Ok((
            shadow_contracts::encode_selection_request(request),
            shadow_contracts::encode_selection_response(response),
        ))
    }
    pub fn attest(self) -> Result<AshtAttestation, String> {
        let request = self
            .request
            .ok_or_else(|| "Asht request missing at attestation".to_string())?;
        let response = self
            .response
            .ok_or_else(|| "Asht selection missing at attestation".to_string())?;

        let projection =
            quantum_lim::request_bound_relevance::project_request_bound(&request, &response)?;
        let relevance_sha256 = projection.relevance_sha256.clone();

        let positive_candidates = projection
            .positive
            .iter()
            .map(|relevant| relevant.candidate.clone())
            .collect::<Vec<_>>();
        let mut ranked_negative_candidates = projection
            .negative
            .iter()
            .map(|relevant| relevant.candidate.clone())
            .collect::<Vec<_>>();
        rank_candidates(&mut ranked_negative_candidates);

        let parity_evidence = evaluate_parity(&positive_candidates, &ranked_negative_candidates);
        match parity_evidence.shape_is_valid(ranked_negative_candidates.len()) {
            true => {}
            false => return Err("Asht produced non-canonical negative parity evidence".to_string()),
        }

        let mut ranked_relevant = projection
            .positive
            .iter()
            .chain(projection.negative.iter())
            .collect::<Vec<_>>();
        ranked_relevant.sort_by(|left, right| {
            candidate_order(left.candidate, right.candidate)
                .then((left.axis as u8).cmp(&(right.axis as u8)))
        });

        let mut candidate_material = Vec::new();
        let mut candidate_sha256 = Vec::new();
        let mut bounded_fragments = Vec::new();

        for relevant in ranked_relevant {
            let candidate = relevant.candidate;
            let mass = candidate_mass(candidate);
            candidate_material.push(relevant.axis as u8);
            candidate_material.extend_from_slice(&candidate.candidate_id.to_le_bytes());
            candidate_material.push(candidate.polarity);
            candidate_material.extend_from_slice(&(candidate.domain.len() as u64).to_le_bytes());
            candidate_material.extend_from_slice(candidate.domain.as_bytes());
            candidate_material.extend_from_slice(candidate.term_sha256.as_bytes());
            candidate_material.extend_from_slice(candidate.source_sha256.as_bytes());
            candidate_material.push(candidate.security_y);
            candidate_material.push(candidate.trust_x);
            candidate_material.extend_from_slice(&candidate.confidence_fixed.to_le_bytes());
            candidate_material.extend_from_slice(&candidate.frequency.to_le_bytes());
            candidate_material.extend_from_slice(&candidate.domain_count.to_le_bytes());
            candidate_material.extend_from_slice(&candidate.legacy_score_fixed.to_le_bytes());
            candidate_material.extend_from_slice(&mass.to_le_bytes());

            let reference_sha256 = candidate_reference_sha256(
                candidate, b"ESSMAI/ASHT/CANDIDATE-REFERENCE/V1B1", &[relevant.axis as u8],
            );
            candidate_sha256.push(reference_sha256.clone());
            bounded_fragments.push(render_candidate_fragment(
                candidate, relevant.axis.label(), &reference_sha256,
            ));
        }
        drop(projection);

        let operator_directive = AshtOperatorDirective::new(
            self.command.trace_id.clone(),
            self.command.split_sha256.clone(),
            response.hold_sha256.clone(),
            response.selection_sha256.clone(),
            candidate_sha256,
        );
        match operator_directive.shape_is_valid() {
            true => {}
            false => return Err("Asht produced an invalid operator directive".to_string()),
        }
        let attestation_sha256 = shadow_contracts::sha256_hex(&[
            b"ESSMAI/ASHT/ATTESTATION/V1B1",
            self.command.trace_id.as_bytes(),
            self.command.split_sha256.as_bytes(),
            self.command.besa_hold_sha256.as_bytes(),
            response.hold_id.as_bytes(),
            response.hold_sha256.as_bytes(),
            response.selection_sha256.as_bytes(),
            relevance_sha256.as_bytes(),
            &[parity_evidence.verdict as u8],
            &parity_evidence.positive_mass.to_le_bytes(),
            &parity_evidence.negative_mass.to_le_bytes(),
            candidate_material.as_slice(),
            operator_directive.directive_sha256.as_bytes(),
        ]);
        Ok(AshtAttestation {
            hold_id: response.hold_id,
            hold_sha256: response.hold_sha256,
            selection_sha256: response.selection_sha256,
            relevance_sha256,
            parity: parity_evidence.verdict,
            positive_mass: parity_evidence.positive_mass,
            negative_mass: parity_evidence.negative_mass,
            ranked_negative_candidates,
            kie_proof: None,
            kie_measurement: None,
            attestation_sha256,
            bounded_fragments,
            operator_directive,
        })
    }
}

pub fn combine_muscle_evidence(besa: &BesaCompanion, asht: &AshtAttestation) -> MuscleEvidenceWire {
    let mut evidence = MuscleEvidenceWire {
        besa_hold_id: besa.hold_id.clone(),
        besa_hold_sha256: besa.hold_sha256.clone(),
        besa_selection_sha256: besa.selection_sha256.clone(),
        besa_attestation_sha256: besa.attestation_sha256(),
        asht_hold_id: asht.hold_id.clone(),
        asht_hold_sha256: asht.hold_sha256.clone(),
        asht_selection_sha256: asht.selection_sha256.clone(),
        asht_attestation_sha256: asht.attestation_sha256.clone(),
        split_sha256: besa.split_sha256.clone(),
        combined_sha256: String::new(),
    };
    evidence.combined_sha256 = evidence.recompute_combined_sha256();
    evidence
}

/// NEW evidence-origin mode; the original GCL commission is still mandatory.
/// This type contains a fresh request and real Shadow hold, never a copied
/// historical refusal capability or fabricated CandidateStatisticWire.
pub struct AshtKieHeld {
    command: AshtCommand,
    request: shadow_contracts::kie::KieSelectionRequest,
    response: shadow_contracts::kie::KieSelectionResponse,
}
impl AshtQuantum<LawGated> {
    pub fn request_kie_evidence(
        self,
        request: shadow_contracts::kie::KieSelectionRequest,
    ) -> Result<AshtKieHeld, String> {
        if self.command.route != quantum_lim::ultimatum_collapse_law::AshtEvidenceRoute::KiE {
            return Err("ASHT_KIE_ROUTE_COMMAND_REQUIRED".into());
        }
        request.validate()?;
        let q = &request.selection;
        if q.trace_id != self.command.trace_id
            || q.split_sha256 != self.command.split_sha256
            || q.upstream_hold_sha256.as_deref() != Some(self.command.besa_hold_sha256.as_str())
        {
            return Err("KIE_ASHT_FRESH_COMMAND_SCOPE".into());
        }
        let response = crate::shadow_process_bridge::execute_kie_selection(&request)?;
        response.validate_for(&request)?;
        if response.hold.law_seal != self.command.law_seal
            || response.hold.system_laws_seal != self.command.system_laws_seal
        {
            return Err("KIE_ASHT_SOURCE_LAW_SEAL".into());
        }
        Ok(AshtKieHeld {
            command: self.command,
            request,
            response,
        })
    }
}
impl AshtKieHeld {
    pub fn attest(self) -> Result<AshtAttestation, String> {
        self.response.validate_for(&self.request)?;
        let proof = self.response.proof;
        // Real provider execution on this authority side. The preceding Shadow
        // execution is a second integrity computation, not independent science.
        let parity = quantum_lim::NproEngine::kie_parity(&proof)?;
        let hold = self.response.hold;
        let refs = proof.predicate_references();
        let operator_directive = AshtOperatorDirective::new(
            self.command.trace_id.clone(),
            self.command.split_sha256.clone(),
            hold.hold_sha256.clone(),
            hold.selection_sha256.clone(),
            refs.clone(),
        );
        if !operator_directive.shape_is_valid() {
            return Err("KIE_ASHT_OPERATOR_SCOPE".into());
        }
        let relevance_sha256 = shadow_contracts::sha256_hex(&[
            b"ESSMAI/KIE/EXACT-PROPOSITION/1",
            self.request.input_text.as_bytes(),
            &shadow_contracts::kie::encode_context(&proof.context)?,
            &proof.proof_sha256,
        ]);
        let attestation_sha256 = shadow_contracts::sha256_hex(&[
            b"ESSMAI/ASHT/KIE-ATTESTATION/1",
            self.command.trace_id.as_bytes(),
            self.command.split_sha256.as_bytes(),
            self.command.besa_hold_sha256.as_bytes(),
            hold.hold_id.as_bytes(),
            hold.hold_sha256.as_bytes(),
            hold.selection_sha256.as_bytes(),
            relevance_sha256.as_bytes(),
            &[parity.verdict as u8],
            &parity.positive_mass.to_le_bytes(),
            &parity.negative_mass.to_le_bytes(),
            &proof.proof_sha256,
            operator_directive.directive_sha256.as_bytes(),
        ]);
        let candidate = science_domains::decode_candidate(&proof.candidate_bytes)
            .map_err(|e| format!("KIE_ASHT_MODEL:{e}"))?;
        let science_domains::ScienceTask::EarthRotationAngle {
            claim_radians, tolerance_radians, ..
        } = &candidate.query.task;
        let kie_measurement = shadow_contracts::kie_measurement::KieMeasurement::from_comparison(
            &proof, &shadow_contracts::pd_memory::iz_science_math::EraComparison {
                query_sha256: candidate.query_sha256, result_sha256: candidate.result_sha256,
                model_sha256: candidate.provenance.compiled_model_sha256,
                claim_radians: *claim_radians, computed_radians: candidate.computed_radians,
                residual_radians: candidate.circular_residual_radians,
                tolerance_radians: *tolerance_radians,
            })?;
        let prior_material=match &proof.prior_adjudication {
            None=>"ASHT KiE prior adjudication absent for exact scoped model proposition and evidence family".to_string(),
            Some(prior)=>format!("ASHT KiE historical whole-cycle judgment transaction={} proof={} family={} Y={} X={} reason_code=unavailable same dependent model family no new independent evidence mass",
                shadow_contracts::kie::hex(&prior.transaction_id),shadow_contracts::kie::hex(&prior.proof_sha256),
                shadow_contracts::kie::hex(&prior.evidence_family_digest),prior.y,prior.x),
        };
        // Both origins reach the existing LIM input. The shared KIP3 projection
        // retains only actual source-classified semantic evidence and applies
        // the existing same-source deduplication, without inventing new mass.
        // Conditions and unresolved origins remain in the full typed proof.
        let historical_request = shadow_contracts::decode_selection_request(&proof.historical_request)
            .map_err(|error| format!("KIE_ASHT_HISTORY_REQUEST:{error}"))?;
        let historical_response = shadow_contracts::decode_selection_response(&proof.historical_response)
            .map_err(|error| format!("KIE_ASHT_HISTORY_RESPONSE:{error}"))?;
        let lexical = quantum_lim::request_bound_relevance::project_available_bound(
            &historical_request, &historical_response,
        )?;
        let scope = proof.context.identity.key()?;
        let history_fragments = shadow_contracts::kie::with_scoped_semantic_candidates(
            &proof, |positive, negative| {
                let mut candidates = positive.iter().chain(negative.iter()).copied().collect::<Vec<_>>();
                candidates.sort_by(|left, right| candidate_order(left, right));
                candidates.into_iter().map(|candidate| {
                    let lexical_axis = lexical.positive.iter().chain(lexical.negative.iter())
                        .find(|entry| entry.candidate == candidate).map(|entry| entry.axis);
                    let (axis_label, reference) = match lexical_axis {
                        Some(axis) => (axis.label(), candidate_reference_sha256(
                            candidate, b"ESSMAI/ASHT/CANDIDATE-REFERENCE/V1B1", &[axis as u8],
                        )),
                        None => ("SCOPED_PROPOSITION", candidate_reference_sha256(
                            candidate, b"ESSMAI/ASHT/SCOPED-PROPOSITION-CANDIDATE/V1", &scope,
                        )),
                    };
                    render_candidate_fragment(candidate, axis_label, &reference)
                }).collect::<Vec<_>>()
            },
        )?;
        let mut bounded_fragments=history_fragments;
        bounded_fragments.extend([
            prior_material,
            format!("ASHT KiE MODEL_CONDITIONAL exact proposition: {}",proof.canonical_proposition),
            format!("ASHT KiS model predicate residual<tolerance mass={} reference={} computed_radians={} circular_residual_radians={}",
                proof.kis_mass,refs[0],candidate.computed_radians,candidate.circular_residual_radians),
            format!("ASHT KiC model predicate residual>tolerance mass={} reference={} equality unresolved",
                proof.kic_mass,refs[1]),
            format!("ASHT KiE source PDS1 record={} query={} same model and input no physical observation no independent P/N no historical confidence or trust",
                shadow_contracts::kie::hex(&proof.context.record_id),
                science_domains::hash_hex(&candidate.query_sha256)),
        ]);
        Ok(AshtAttestation {
            hold_id: hold.hold_id,
            hold_sha256: hold.hold_sha256,
            selection_sha256: hold.selection_sha256,
            relevance_sha256,
            parity: parity.verdict,
            positive_mass: parity.positive_mass,
            negative_mass: parity.negative_mass,
            ranked_negative_candidates: Vec::new(),
            kie_proof: Some(proof),
            kie_measurement: Some(kie_measurement),
            attestation_sha256,
            bounded_fragments,
            operator_directive,
        })
    }
}

#[cfg(test)]
mod r26_kie_history_input_tests {
    use super::*;
    use quantum_lim::ultimatum_collapse_law::AshtEvidenceRoute;
    use shadow_contracts::kie::{self, KieContext, KieHistoryAssessment, KieHistoryClass,
        KieHistoryEntry, KieHistoryProvenance, KieProof, KieSelectionRequest,
        KieSelectionResponse};
    use shadow_contracts::pd_memory::failure::relevance;

    // Executes the actual pinned numerical provider. Holds and source provenance
    // below are explicitly synthetic unit data, not native Shadow admission.
    fn held(classes: &[(bool, KieHistoryClass)], duplicate_support: bool) -> AshtKieHeld {
        use science_domains::{AdapterId, ScienceQuery, ScienceTask, TimeScale, evaluate, encode_query, encode_candidate};
        let identity = shadow_contracts::pd_memory::PdIdentity {
            owner: "r26-unit-source".into(), obligation: "model-predicate".into(),
            case_id: "unit-case".into(), scope: "astronomy".into(),
            domain: "astronomy".into(), lineage_root: "unit-lineage".into(),
        };
        let mut query = ScienceQuery {
            adapter: AdapterId::Astronomy,
            session_id: shadow_contracts::pd_memory::hex(&identity.key().unwrap()),
            case_id: identity.case_id.clone(), scope_id: identity.scope.clone(),
            obligation_id: identity.obligation.clone(), query_id: "unit-query".into(),
            input_source_id: "unit-UT1".into(),
            task: ScienceTask::EarthRotationAngle { jd_part1: 2451545.0, jd_part2: 0.0,
                time_scale: TimeScale::Ut1, claim_radians: 0.0, tolerance_radians: 0.1 },
        };
        let computed = evaluate(&query).unwrap();
        let ScienceTask::EarthRotationAngle { ref mut claim_radians, .. } = query.task;
        *claim_radians = computed.computed_radians;
        let evaluated = evaluate(&query).unwrap();
        let query_bytes = encode_query(&query).unwrap();
        let candidate_bytes = encode_candidate(&evaluated).unwrap();
        let model = science_domains::kie::evaluate_kie(&query_bytes, &candidate_bytes).unwrap();
        assert_eq!(model.relation, 0);
        let selection = KnowledgeSelectionRequestWire {
            version: shadow_contracts::PROTOCOL_VERSION,
            requester: SelectionRequester::AshtQuantum,
            purpose: shadow_contracts::SelectionPurpose::EvidenceCandidates,
            session_id: "r26-unit-attempt".into(),
            parent_i0: kie::hex(&kie::digest(model.canonical_proposition.as_bytes())),
            primitive_anchor: 1, trace_id: "r26-unit-attempt".into(),
            split_sha256: "a".repeat(64), xi: vec![model.canonical_proposition.clone()],
            yi: vec![], upstream_hold_sha256: Some("b".repeat(64)),
            evidence_sha256: "c".repeat(64),
            max_positive: shadow_contracts::SELECTION_MAX_CANDIDATES,
            max_negative: shadow_contracts::SELECTION_MAX_CANDIDATES,
        };
        let mut historical = KnowledgeSelectionResponseWire {
            version: shadow_contracts::PROTOCOL_VERSION, requester: selection.requester,
            session_id: selection.session_id.clone(), parent_i0: selection.parent_i0.clone(),
            primitive_anchor: selection.primitive_anchor, split_sha256: selection.split_sha256.clone(),
            upstream_hold_sha256: selection.upstream_hold_sha256.clone(),
            request_sha256: shadow_contracts::sha256_hex(&[b"ESSMAI/SELECTION/REQUEST/V178",
                &shadow_contracts::encode_selection_request(&selection)]),
            selection_sha256: String::new(), hold_id: "r26-synthetic-history-hold".into(),
            hold_sha256: "d".repeat(64), positive: vec![], negative: vec![],
            law_seal: 1, system_laws_seal: 1, expires_at_ns: 1,
        };
        let mut entries = Vec::new();
        for (index, (positive, class)) in classes.iter().enumerate() {
            let candidate = CandidateStatisticWire {
                candidate_id: index as u64 + 1, polarity: u8::from(*positive),
                domain: if *positive { "primitive" } else { "negative" }.into(),
                term_sha256: shadow_contracts::sha256_hex(&[
                    if *positive { relevance::TERM_PRIMITIVE_DOMAIN } else { relevance::TERM_NEGATIVE_DOMAIN },
                    model.canonical_proposition.as_bytes()]),
                source_sha256: shadow_contracts::sha256_hex(&[b"r26-synthetic-source", &(index as u64).to_le_bytes()]),
                security_y: u8::from(*positive), trust_x: u8::from(*positive),
                confidence_fixed: if *positive { 8000 } else { 2000 }, frequency: 1,
                domain_count: 1, legacy_score_fixed: if *positive { 8000 } else { 2000 },
            };
            let entry = KieHistoryEntry { polarity: candidate.polarity,
                selection_context: None,
                candidate_id: candidate.candidate_id, source_sha256: candidate.source_sha256.clone(),
                class: *class, provenance: if *class == KieHistoryClass::LegacyUnscoped { None } else {
                    Some(KieHistoryProvenance { transaction_id: [1; 32], proof_sha256: [2; 32],
                        model_family_sha256: [3; 32] }) },
            };
            if *positive { historical.positive.push(candidate); } else { historical.negative.push(candidate); }
            entries.push(entry);
        }
        if duplicate_support {
            let mut duplicate = historical.positive.first().unwrap().clone();
            duplicate.candidate_id += 100;
            let mut entry = entries.iter().find(|v| v.polarity == 1).unwrap().clone();
            entry.candidate_id = duplicate.candidate_id;
            historical.positive.push(duplicate);
            entries.push(entry);
        }
        entries.sort_by(|a, b| a.key().cmp(&b.key()));
        historical.selection_sha256 = relevance::selection_digest(&historical);
        let context = KieContext { identity, operation_id: "unit-op".into(), record_id: [4; 32] };
        let proof = KieProof {
            context: context.clone(), query_bytes, candidate_bytes,
            canonical_proposition: model.canonical_proposition.clone(),
            historical_request: shadow_contracts::encode_selection_request(&selection),
            historical_response: shadow_contracts::encode_selection_response(&historical),
            kis_mass: model.kis_mass, kic_mass: model.kic_mass, relation: model.relation,
            proof_sha256: [0; 32], prior_adjudication: None,
            history_assessment: Some(KieHistoryAssessment {
                scope_sha256: context.identity.key().unwrap(),
                proposition_sha256: kie::digest(model.canonical_proposition.as_bytes()),
                entries, scoped_evidence: vec![],
            }),
        }.seal_data().unwrap();
        let mut hold = historical.clone();
        hold.hold_id = "kie-r26-synthetic-hold".into();
        hold.positive.clear(); hold.negative.clear();
        hold.selection_sha256 = proof.selection_sha256();
        let command = AshtCommand { route: AshtEvidenceRoute::KiE,
            trace_id: selection.trace_id.clone(), split_sha256: selection.split_sha256.clone(),
            besa_hold_sha256: selection.upstream_hold_sha256.clone().unwrap(),
            law_seal: 1, system_laws_seal: 1 };
        let request = KieSelectionRequest { selection, context,
            input_text: model.canonical_proposition, historical_response: historical };
        let response = KieSelectionResponse { hold, proof };
        response.validate_for(&request).unwrap();
        AshtKieHeld { command, request, response }
    }

    // The same synthetic sources may be reached through a complete-proposition
    // index, independent of lexical Xi/Yi indexing; this is not a new source.
    fn scoped_history(mut input: AshtKieHeld, keep_lexical: bool) -> AshtKieHeld {
        let mut proof = input.response.proof;
        let assessment = proof.history_assessment.as_mut().unwrap();
        let mut historical = shadow_contracts::decode_selection_response(&proof.historical_response).unwrap();
        let mut scoped = historical.positive.iter().chain(historical.negative.iter()).map(|candidate| {
            let entry = assessment.entries.iter().find(|entry| entry.key() ==
                (candidate.polarity, candidate.candidate_id, candidate.source_sha256.as_str())).unwrap();
            shadow_contracts::kie::KieScopedEvidence {
                candidate: candidate.clone(), class: entry.class,
                provenance: entry.provenance.clone().unwrap(),
            }
        }).collect::<Vec<_>>();
        scoped.sort_by(|a, b| a.key().cmp(&b.key()));
        assessment.scoped_evidence = scoped;
        if !keep_lexical {
            historical.positive.clear();
            historical.negative.clear();
            assessment.entries.clear();
        }
        historical.selection_sha256 = relevance::selection_digest(&historical);
        proof.historical_response = shadow_contracts::encode_selection_response(&historical);
        let proof = proof.seal_data().unwrap();
        input.response.hold.selection_sha256 = proof.selection_sha256();
        input.response.proof = proof;
        input.request.historical_response = historical;
        input.response.validate_for(&input.request).unwrap();
        input
    }
    fn historical_fragments(attestation: &AshtAttestation) -> Vec<&str> {
        attestation.bounded_fragments.iter().filter(|fragment|
            fragment.starts_with("ASHT request-bound evidence ")).map(String::as_str).collect()
    }

    #[test]
    fn r26_kie_attest_retains_both_scoped_polarities_before_lim() {
        let input = held(&[(true, KieHistoryClass::ExactModelSupport),
            (false, KieHistoryClass::ExactModelCounterproof)], false);
        let original_proof = input.response.proof.clone();
        let expected_parity = quantum_lim::NproEngine::kie_parity(&original_proof).unwrap();
        let attestation = input.attest().unwrap();
        let fragments = historical_fragments(&attestation);
        assert_eq!(fragments.len(), 2, "both scoped historical polarities must reach the actual LIM input");
        assert!(fragments.iter().any(|v| v.contains("polarity=1 mass_fixed=8000 confidence_fixed=8000")));
        assert!(fragments.iter().any(|v| v.contains("polarity=0 mass_fixed=2000 confidence_fixed=2000")));
        assert_eq!(attestation.kie_proof, Some(original_proof));
        assert_eq!(attestation.parity_evidence(), expected_parity);
        assert!(attestation.bounded_fragments.iter().any(|v| v.starts_with("ASHT KiS ")));
        assert!(attestation.bounded_fragments.iter().any(|v| v.starts_with("ASHT KiC ")));
    }

    #[test]
    fn r26_kie_attest_deduplicates_scoped_history_and_keeps_conditions_out_of_candidates() {
        let input = held(&[(true, KieHistoryClass::ExactModelSupport),
            (false, KieHistoryClass::ExactModelCounterproof),
            (false, KieHistoryClass::ProcessFailure),
            (false, KieHistoryClass::LegacyUnscoped)], true);
        let original_proof = input.response.proof.clone();
        let expected_parity = quantum_lim::NproEngine::kie_parity(&original_proof).unwrap();
        let attestation = input.attest().unwrap();
        assert_eq!(historical_fragments(&attestation).len(), 2,
            "same-source duplicates and witnessed/unknown conditions must not become extra semantic candidates");
        assert_eq!(attestation.kie_proof, Some(original_proof), "conditions remain in the full typed proof");
        assert_eq!(attestation.parity_evidence(), expected_parity);
    }

    #[test]
    fn r26_kie_attest_missing_history_keeps_model_predicates_without_fake_candidates() {
        let attestation = held(&[], false).attest().unwrap();
        assert!(historical_fragments(&attestation).is_empty());
        assert!(attestation.bounded_fragments.iter().any(|v| v.starts_with("ASHT KiS ")));
        assert!(attestation.bounded_fragments.iter().any(|v| v.starts_with("ASHT KiC ")));
        assert_eq!(attestation.parity, ParityVerdict::PositivePermitted);
        assert!(attestation.ranked_negative_candidates.is_empty());
    }

    #[test]
    fn r26_kie_attest_retains_whole_proposition_history_without_forged_axis() {
        let input = scoped_history(held(&[(true, KieHistoryClass::ExactModelSupport),
            (false, KieHistoryClass::ExactModelCounterproof)], false), false);
        let original_proof = input.response.proof.clone();
        let expected_parity = quantum_lim::NproEngine::kie_parity(&original_proof).unwrap();
        let attestation = input.attest().unwrap();
        let fragments = historical_fragments(&attestation);
        assert_eq!(fragments.len(), 2, "whole-proposition evidence is actual scoped history even without lexical membership");
        assert!(fragments.iter().all(|value| value.contains("axis=SCOPED_PROPOSITION ")));
        assert_eq!(attestation.kie_proof, Some(original_proof));
        assert_eq!(attestation.parity_evidence(), expected_parity);
    }

    #[test]
    fn r26_kie_attest_same_source_in_lexical_and_proposition_indexes_is_one_candidate() {
        let input = scoped_history(held(&[(true, KieHistoryClass::ExactModelSupport),
            (false, KieHistoryClass::ExactModelCounterproof)], false), true);
        let original_proof = input.response.proof.clone();
        let attestation = input.attest().unwrap();
        let fragments = historical_fragments(&attestation);
        assert_eq!(fragments.len(), 2, "two indexes of identical source bytes must not double the measured evidence");
        assert!(fragments.iter().all(|value| value.contains("axis=Xi ")));
        assert_eq!(attestation.kie_proof, Some(original_proof));
    }
    #[test]
    fn r28_asht_retained_measurement_can_explain_hold_without_endpoint_candidates() {
        let attestation = held(&[], false).attest().unwrap();
        let proof = attestation.kie_proof.as_ref().unwrap();
        let observed = attestation.kie_measurement.as_ref().unwrap();
        observed.validate_model(proof).unwrap();
        assert_eq!(observed, &quantum_lim::kie_measurement::measure(proof).unwrap());
        let held = shadow_contracts::model_operator_measurement::ModelOperatorMeasurements::new(
            observed.clone(), false, false).unwrap().summarize().unwrap();
        assert!(!held.pro.predicate_tested && !held.npro.predicate_tested);
        assert_eq!(held.pro.candidates.count + held.npro.candidates.count, 0);
        assert!(historical_fragments(&attestation).is_empty());
    }

}