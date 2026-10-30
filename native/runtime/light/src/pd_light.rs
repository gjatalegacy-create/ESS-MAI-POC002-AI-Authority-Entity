// ╔══════════════════════════════════════════════════════════════════════════╗
// ║ pd_light.rs — adapteri i verifikuar PD Quantum → PD Light → Nura       ║
// ║ AIGovern-Cyber v1.6.4 — korrier kontekstual iZ, pa procedim Layer 1/2/3       ║
// ║                                                                          ║
// ║ Light nuk arsyeton. Ai rillogarit vetëm provën kushtetuese të receipt-it║
// ║ dhe Vulën e Gjallë ×3, mbyll `PD output + iZ → next i₀`, pastaj         ║
// ║ projekton surface-in e mbyllur për Nura.                                 ║
// ╚══════════════════════════════════════════════════════════════════════════╝

use crate::living_trust_contract::{self as living_trust, LivingTrustProof, LivingTrustSeal};
use crate::pd_continuum_contract::{self as continuum, PdContinuumBasis, PdContinuumCompletion};

/// 45 fushat e trupit të wire-it v1.6.4 (CRC-ja është fusha e 46-të jashtë
/// këtij tipi dhe hiqet nga `pa_wire::open_body` para parsimit).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdRenderInput {
    pub session_id: String,
    pub dominant_concept: String,
    pub accumulated_mass: u32,
    pub structural_coherence: u32,
    pub genius_score: u32,
    pub estimated_trl: u8,
    pub probe_hint: String,
    pub parent_i0: String,
    pub next_i0_id: String,
    pub y_verdict: u8,
    pub x_verdict: u8,
    pub verification_seal: u64,
    pub verification_generation: u64,
    pub verification_receipt_id: String,
    pub primitive_anchor: u64,
    pub xy_digest: u64,
    pub pd_binding_digest: u64,
    pub pd_continuum_activation_digest: u64,
    pub question_increment_digest: u64,
    pub pd_output_id: String,
    pub pd_output_digest: u64,
    pub iz_id: String,
    pub iz_digest: u64,
    pub continuum_completion_digest: u64,
    pub continuation_basis: String,
    pub living_trust_action_state: u64,
    pub living_trust_action_mask: u64,
    pub living_trust_required_action_mask: u64,
    pub living_trust_scientific_project_sha256: [u8; 32],
    pub project_id: u64,
    pub project_context_sha256: [u8; 32],
    pub project_status: u8,
    pub novel_factualized: bool,
    pub novel_trl_level: u8,
    pub novel_proof_score: u32,
    pub novel_rejection: u8,
    pub living_trust_knowledge_band: u8,
    pub living_trust_lgc_law: u32,
    pub living_trust_system_laws_seal: u64,
    pub living_trust_sovereign_flags: u32,
    pub living_trust_sovereign_value: u32,
    pub living_trust_sha256: [u8; 32],
    pub living_trust_intensity: u32,
    pub living_trust_kind: u8,
    pub living_trust_digest: u64,
}

/// Vetëm `PdLight::deliver` mund ta krijojë pasi të jenë verifikuar Shadow dhe
/// formula e kontinuumit. Nura merr këtë tip, jo një String të lirë.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPdSurface(String);

impl VerifiedPdSurface {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Pamja e tipizuar e receipt-it terminal që QUANTUM e ka marrë nga SHADOW dhe
/// e ka transportuar në handoff-in ekzistues PD. Ky tip nuk krijon autoritet të
/// ri: ndërtohet vetëm nga `PdRenderInput` pasi receipt-i, Living Trust-i dhe
/// kontinuumi janë riverifikuar plotësisht nga `is_verification_complete`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdTerminalReceipt {
    session_id: String,
    parent_i0: String,
    primitive_anchor: u64,
    xy_digest: u64,
    pd_binding_digest: u64,
    pd_continuum_activation_digest: u64,
    question_increment_digest: u64,
    pd_output_digest: u64,
    iz_digest: u64,
    continuum_completion_digest: u64,
    next_i0_id: String,
    y_verdict: u8,
    x_verdict: u8,
    verification_seal: u64,
    verification_generation: u64,
    verification_receipt_id: String,
    living_trust_sha256: [u8; 32],
    living_trust_digest: u64,
    system_laws_seal: u64,
    continuation_basis: PdContinuumBasis,
}

impl PdTerminalReceipt {
    fn from_verified(input: &PdRenderInput, basis: PdContinuumBasis) -> Self {
        Self {
            session_id: input.session_id.clone(),
            parent_i0: input.parent_i0.clone(),
            primitive_anchor: input.primitive_anchor,
            xy_digest: input.xy_digest,
            pd_binding_digest: input.pd_binding_digest,
            pd_continuum_activation_digest: input.pd_continuum_activation_digest,
            question_increment_digest: input.question_increment_digest,
            pd_output_digest: input.pd_output_digest,
            iz_digest: input.iz_digest,
            continuum_completion_digest: input.continuum_completion_digest,
            next_i0_id: input.next_i0_id.clone(),
            y_verdict: input.y_verdict,
            x_verdict: input.x_verdict,
            verification_seal: input.verification_seal,
            verification_generation: input.verification_generation,
            verification_receipt_id: input.verification_receipt_id.clone(),
            living_trust_sha256: input.living_trust_sha256,
            living_trust_digest: input.living_trust_digest,
            system_laws_seal: input.living_trust_system_laws_seal,
            continuation_basis: basis,
        }
    }

    pub fn receipt_id(&self) -> &str {
        &self.verification_receipt_id
    }

    /// Identitet lokal SHA-256 i materialit terminal të verifikuar. Ky digest
    /// hyn në historinë e LIGHT-it; ai nuk zëvendëson receipt-id e SHADOW-it.
    pub fn evidence_sha256(&self) -> String {
        let primitive_anchor = self.primitive_anchor.to_le_bytes();
        let xy_digest = self.xy_digest.to_le_bytes();
        let pd_binding_digest = self.pd_binding_digest.to_le_bytes();
        let activation = self.pd_continuum_activation_digest.to_le_bytes();
        let question_increment = self.question_increment_digest.to_le_bytes();
        let pd_output = self.pd_output_digest.to_le_bytes();
        let iz = self.iz_digest.to_le_bytes();
        let completion = self.continuum_completion_digest.to_le_bytes();
        let verdict = [self.y_verdict, self.x_verdict];
        let verification_seal = self.verification_seal.to_le_bytes();
        let generation = self.verification_generation.to_le_bytes();
        let living_trust_digest = self.living_trust_digest.to_le_bytes();
        let system_laws = self.system_laws_seal.to_le_bytes();
        crate::lgc_algorithm::compute_domain_sha(
            "ESSMAI/LIGHT/SHADOW_TERMINAL_RECEIPT",
            &[
                self.session_id.as_bytes(),
                self.parent_i0.as_bytes(),
                &primitive_anchor,
                &xy_digest,
                &pd_binding_digest,
                &activation,
                &question_increment,
                &pd_output,
                &iz,
                &completion,
                self.next_i0_id.as_bytes(),
                &verdict,
                &verification_seal,
                &generation,
                self.verification_receipt_id.as_bytes(),
                &self.living_trust_sha256,
                &living_trust_digest,
                &system_laws,
                self.continuation_basis.label().as_bytes(),
            ],
        )
    }

    /// Lidh receipt-in e verifikuar me të njëjtin cikël që LIGHT dërgoi dhe me
    /// envelope-in terminal që QUANTUM ktheu. LIGHT nuk rillogarit XY dhe nuk
    /// nxjerr verdikt: ai krahason vetëm identitetin i₀/PA dhe bitet e receipt-it.
    pub fn closes_light_cycle(
        &self,
        response: &crate::light_coordinator::LightResponse,
        result: &crate::quantum_bridge::QuantumResult,
    ) -> bool {
        let expected_parent = format!("i0-{}", response.primitive_trace.id_hex());
        let expected_seal = crate::sovereign::laws::module_seal(
            crate::lab_contracts::verification_receipt::VERIFICATION_SEAL_NAME,
        );
        let expected_receipt = crate::lab_contracts::verification_receipt::receipt_id(
            &self.session_id,
            &self.parent_i0,
            self.primitive_anchor,
            self.xy_digest,
            self.pd_binding_digest,
            self.pd_continuum_activation_digest,
            &self.living_trust_sha256,
            self.y_verdict,
            self.x_verdict,
            self.verification_generation,
            self.verification_seal,
        );
        let verdict_bound = matches!(
            (
                result.verdict.as_str(),
                self.y_verdict,
                self.x_verdict,
                self.continuation_basis,
            ),
            ("OPTIMAL", 1, 1, PdContinuumBasis::VerifiedPositive)
                | ("HOLD", 0, 0, PdContinuumBasis::VerifiedNegativeRebuild)
        );
        let formula_laws_bound = response
            .legacy
            .as_ref()
            .map(|legacy| legacy.receipt.system_laws_seal == self.system_laws_seal)
            == Some(true);
        result.is_bound_to(&response.trace_id)
            && self.session_id == response.trace_id
            && self.session_id == result.trace_id
            && self.parent_i0 == expected_parent
            && self.primitive_anchor == response.primitive_trace.id
            && self.verification_seal == expected_seal
            && self.verification_generation != 0
            && self.xy_digest != 0
            && self.pd_binding_digest != 0
            && self.pd_continuum_activation_digest != 0
            && self.question_increment_digest != 0
            && self.pd_output_digest != 0
            && self.iz_digest != 0
            && self.continuum_completion_digest != 0
            && !self.next_i0_id.is_empty()
            && self.living_trust_sha256 != [0u8; 32]
            && self.living_trust_digest != 0
            && self.system_laws_seal == crate::lab_contracts::system_laws_seal()
            && crate::lab_contracts::verification_receipt::is_canonical_receipt_id(
                &self.verification_receipt_id,
            )
            && self.verification_receipt_id == expected_receipt
            && verdict_bound
            && result.is_verified == (self.y_verdict == 1 && self.x_verdict == 1)
            && formula_laws_bound
            && crate::lgc_algorithm::is_canonical_sha256(&self.evidence_sha256())
    }

    #[cfg(test)]
    pub(crate) fn fixture_for(
        response: &crate::light_coordinator::LightResponse,
        result: &crate::quantum_bridge::QuantumResult,
        marker: u64,
    ) -> Self {
        let bounded_marker = marker.max(1);
        let (y_verdict, x_verdict, continuation_basis) = match result.verdict.as_str() {
            "OPTIMAL" => (1, 1, PdContinuumBasis::VerifiedPositive),
            _ => (0, 0, PdContinuumBasis::VerifiedNegativeRebuild),
        };
        let parent_i0 = format!("i0-{}", response.primitive_trace.id_hex());
        let verification_seal = crate::sovereign::laws::module_seal(
            crate::lab_contracts::verification_receipt::VERIFICATION_SEAL_NAME,
        );
        let living_trust_sha256 = [u8::try_from((bounded_marker % 254) + 1).unwrap_or(1); 32];
        let verification_receipt_id = crate::lab_contracts::verification_receipt::receipt_id(
            &response.trace_id,
            &parent_i0,
            response.primitive_trace.id,
            bounded_marker,
            bounded_marker.wrapping_add(1),
            bounded_marker.wrapping_add(2),
            &living_trust_sha256,
            y_verdict,
            x_verdict,
            bounded_marker,
            verification_seal,
        );
        Self {
            session_id: response.trace_id.clone(),
            parent_i0,
            primitive_anchor: response.primitive_trace.id,
            xy_digest: bounded_marker,
            pd_binding_digest: bounded_marker.wrapping_add(1),
            pd_continuum_activation_digest: bounded_marker.wrapping_add(2),
            question_increment_digest: bounded_marker.wrapping_add(3),
            pd_output_digest: bounded_marker.wrapping_add(4),
            iz_digest: bounded_marker.wrapping_add(5),
            continuum_completion_digest: bounded_marker.wrapping_add(6),
            next_i0_id: format!("i0-next-{bounded_marker:016x}"),
            y_verdict,
            x_verdict,
            verification_seal,
            verification_generation: bounded_marker,
            verification_receipt_id,
            living_trust_sha256,
            living_trust_digest: bounded_marker.wrapping_add(7),
            system_laws_seal: crate::lab_contracts::system_laws_seal(),
            continuation_basis,
        }
    }
}

/// Sinjali kontekstual i iZ. Nuk përmban reasoning të ri: është një pasqyrë
/// e fakteve tashmë të verifikuara nga Shadow dhe kontinuumi PD.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdUiContinuitySignal {
    pub session_id: String,
    pub iz_id: String,
    pub next_i0_id: String,
    pub continuation_basis: String,
    pub accumulated_mass: u32,
    pub structural_coherence: u32,
    pub genius_score: u32,
    pub y_verdict: u8,
    pub x_verdict: u8,
    pub living_trust_intensity: u32,
    pub living_trust_kind: u8,
    pub living_trust_digest: u64,
    pub project_id: u64,
    pub project_context_sha256: [u8; 32],
    pub project_status: u8,
    pub novel_trl_level: u8,
    pub continuity_digest: u64,
}

impl PdUiContinuitySignal {
    fn from_verified(input: &PdRenderInput) -> Self {
        let continuity_digest = crate::pd_spine_contract::digest_parts(&[
            input.session_id.as_bytes(),
            input.iz_id.as_bytes(),
            &input.iz_digest.to_le_bytes(),
            input.next_i0_id.as_bytes(),
            input.continuation_basis.as_bytes(),
            &input.continuum_completion_digest.to_le_bytes(),
            &input.living_trust_digest.to_le_bytes(),
            &input.living_trust_intensity.to_le_bytes(),
            &[input.living_trust_kind],
            &input.project_id.to_le_bytes(),
            &input.project_context_sha256,
            &[input.project_status, input.novel_trl_level],
            &[input.y_verdict, input.x_verdict],
            b"PD_LIGHT_IZ_UI_CONTINUITY_V164",
        ]);
        Self {
            session_id: input.session_id.clone(),
            iz_id: input.iz_id.clone(),
            next_i0_id: input.next_i0_id.clone(),
            continuation_basis: input.continuation_basis.clone(),
            accumulated_mass: input.accumulated_mass,
            structural_coherence: input.structural_coherence,
            genius_score: input.genius_score,
            y_verdict: input.y_verdict,
            x_verdict: input.x_verdict,
            living_trust_intensity: input.living_trust_intensity,
            living_trust_kind: input.living_trust_kind,
            living_trust_digest: input.living_trust_digest,
            project_id: input.project_id,
            project_context_sha256: input.project_context_sha256,
            project_status: input.project_status,
            novel_trl_level: input.novel_trl_level,
            continuity_digest,
        }
    }
}

/// Dorëzimi i vetëm i PD Light: Nura surface + sinjal faktik për UI-në
/// emocionale të vjetër. Të dy dalin paralelisht nga i njëjti iZ i verifikuar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPdDelivery {
    pub surface: VerifiedPdSurface,
    pub ui_signal: PdUiContinuitySignal,
    pub terminal_receipt: PdTerminalReceipt,
}

impl VerifiedPdDelivery {
    /// Porta pre-dispatch/post-process: receipt-i duhet të vijë nga atomi që
    /// ende është i komituar dhe i gatshëm, përpara mutacionit terminal Evolve.
    pub fn closes_pre_dispatch_cycle(
        &self,
        response: &crate::light_coordinator::LightResponse,
        result: &crate::quantum_bridge::QuantumResult,
    ) -> bool {
        response.formula_anchor_valid()
            && response.ready_for_dispatch().is_ok()
            && self.terminal_receipt.closes_light_cycle(response, result)
    }
}

pub struct PdLight;

impl PdLight {
    pub fn deliver(input: &PdRenderInput) -> Option<VerifiedPdDelivery> {
        let basis = PdContinuumBasis::from_label(&input.continuation_basis);
        match (input.is_verification_complete(), basis) {
            (true, Some(verified_basis)) => Some(VerifiedPdDelivery {
                surface: VerifiedPdSurface(Self::render_verified(input)),
                ui_signal: PdUiContinuitySignal::from_verified(input),
                terminal_receipt: PdTerminalReceipt::from_verified(input, verified_basis),
            }),
            _ => None,
        }
    }

    fn render_verified(input: &PdRenderInput) -> String {
        let continuum_line = format!(
            "PD output={} + iZ={} → next i₀={}",
            input.pd_output_id, input.iz_id, input.next_i0_id,
        );
        let project_line = match input.project_status {
            shadow_contracts::PROJECT_STATUS_NOVEL_FACTUAL => format!(
                " Projekti {} u shpall NOVEL faktik TRL{} (proof={}.{:04}).",
                input.project_id,
                input.novel_trl_level,
                input.novel_proof_score / 10_000,
                input.novel_proof_score % 10_000,
            ),
            shadow_contracts::PROJECT_STATUS_RIGOROUS_NEGATIVE => format!(
                " Projekti {} u mbyll si negative rigoroze e verifikuar.",
                input.project_id,
            ),
            shadow_contracts::PROJECT_STATUS_HOLD => format!(
                " Projekti {} mbetet nën GCL në HOLD (TRL{}, rejection={}).",
                input.project_id, input.novel_trl_level, input.novel_rejection,
            ),
            _ => String::new(),
        };
        let rendered = match (input.dominant_concept.as_str(), input.probe_hint.is_empty()) {
            ("~pd_probe~", false) => format!(
                "{}; vazhdimi determinist ({}): {}",
                continuum_line, input.continuation_basis, input.probe_hint,
            ),
            ("~pd_probe~", true) => continuum_line,
            (_, false) => format!(
                "{} {} Vazhdimi determinist ({}): {}",
                Self::render_package(input),
                continuum_line,
                input.continuation_basis,
                input.probe_hint,
            ),
            (_, true) => format!("{} {}", Self::render_package(input), continuum_line),
        };
        format!("{}{}", rendered, project_line)
    }

    fn render_package(input: &PdRenderInput) -> String {
        format!(
            "Gërmimi epistemik nxori konceptin '{}'. \
             Masa: {}. Koherenca: {} ({}). \
             Pjekuria: {} (TRL{}). Forca e sinjalit: {}. \
             Verifikimi: Y={} → X={}.",
            input.dominant_concept,
            Self::mass_decimal(input.accumulated_mass),
            Self::coherence_phrase(input.structural_coherence),
            Self::mass_decimal(input.structural_coherence),
            Self::maturity_phrase(input.estimated_trl),
            input.estimated_trl,
            Self::mass_decimal(input.genius_score),
            input.y_verdict,
            input.x_verdict,
        )
    }

    fn maturity_phrase(trl: u8) -> &'static str {
        match trl {
            0 => "ide në lindje",
            1 => "koncept që po merr formë",
            2 => "hipotezë e strukturuar",
            _ => "përtej fushës së PD",
        }
    }

    fn coherence_phrase(c: u32) -> &'static str {
        match c {
            x if x >= 6_600 => "e fortë",
            x if x >= 3_300 => "mesatare",
            _ => "në formim",
        }
    }

    fn mass_decimal(raw: u32) -> String {
        let bounded = raw.min(crate::pd_spine_contract::MASS_SCALE);
        format!(
            "{}.{:04}",
            bounded / crate::pd_spine_contract::MASS_SCALE,
            bounded % crate::pd_spine_contract::MASS_SCALE,
        )
    }

    /// Parsimi i handoff-it autoritativ v1.6.4. Formatet e vjetra refuzohen që një handoff
    /// pa output/iZ të mos paraqitet si surface i plotë i Nura-s.
    pub fn parse_handoff(line: &str) -> Option<PdRenderInput> {
        let p: Vec<&str> = line.split('|').collect();
        match p.as_slice() {
            [session, concept, mass, coherence, genius, trl, question, parent_i0, next_i0, y, x, seal, generation, receipt, basis, anchor, xy, binding, activation, question_increment, output_id, output_digest, iz_id, iz_digest, completion_digest, trust_action_state, trust_action_mask, trust_required_action_mask, trust_project_sha256, project_id, project_context_sha256, project_status, novel_factualized, novel_trl_level, novel_proof_score, novel_rejection, trust_knowledge_band, trust_lgc_law, trust_system_laws_seal, trust_sovereign_flags, trust_sovereign_value, trust_sha256, trust_intensity, trust_kind, trust_digest] =>
            {
                let parsed = (
                    mass.trim().parse::<u32>(),
                    coherence.trim().parse::<u32>(),
                    genius.trim().parse::<u32>(),
                    trl.trim().parse::<u8>(),
                    y.trim().parse::<u8>(),
                    x.trim().parse::<u8>(),
                    u64::from_str_radix(seal.trim(), 16),
                    generation.trim().parse::<u64>(),
                    u64::from_str_radix(anchor.trim(), 16),
                    u64::from_str_radix(xy.trim(), 16),
                    u64::from_str_radix(binding.trim(), 16),
                    u64::from_str_radix(activation.trim(), 16),
                    u64::from_str_radix(question_increment.trim(), 16),
                    u64::from_str_radix(output_digest.trim(), 16),
                    u64::from_str_radix(iz_digest.trim(), 16),
                    u64::from_str_radix(completion_digest.trim(), 16),
                    u64::from_str_radix(trust_action_state.trim(), 16),
                    u64::from_str_radix(trust_action_mask.trim(), 16),
                    u64::from_str_radix(trust_required_action_mask.trim(), 16),
                    crate::gcl_project_contract::parse_sha256(trust_project_sha256.trim()),
                    u64::from_str_radix(project_id.trim(), 16),
                    crate::gcl_project_contract::parse_sha256(project_context_sha256.trim()),
                    project_status.trim().parse::<u8>(),
                    novel_factualized.trim().parse::<u8>(),
                    novel_trl_level.trim().parse::<u8>(),
                    novel_proof_score.trim().parse::<u32>(),
                    novel_rejection.trim().parse::<u8>(),
                    trust_knowledge_band.trim().parse::<u8>(),
                    u32::from_str_radix(trust_lgc_law.trim(), 16),
                    u64::from_str_radix(trust_system_laws_seal.trim(), 16),
                    u32::from_str_radix(trust_sovereign_flags.trim(), 16),
                    u32::from_str_radix(trust_sovereign_value.trim(), 16),
                    living_trust::parse_hex_32(trust_sha256.trim()),
                    trust_intensity.trim().parse::<u32>(),
                    trust_kind.trim().parse::<u8>(),
                    u64::from_str_radix(trust_digest.trim(), 16),
                );
                match parsed {
                    (
                        Ok(m),
                        Ok(c),
                        Ok(g),
                        Ok(t),
                        Ok(yv),
                        Ok(xv),
                        Ok(s),
                        Ok(gen),
                        Ok(pa),
                        Ok(xyd),
                        Ok(bind),
                        Ok(act),
                        Ok(qd),
                        Ok(od),
                        Ok(izd),
                        Ok(cd),
                        Ok(action),
                        Ok(action_mask),
                        Ok(required_action_mask),
                        Some(project_sha256),
                        Ok(pid),
                        Some(pcontext_sha256),
                        Ok(pstatus),
                        Ok(pfactual),
                        Ok(ptrl),
                        Ok(pscore),
                        Ok(prejection),
                        Ok(band),
                        Ok(law),
                        Ok(laws),
                        Ok(flags),
                        Ok(value),
                        Some(sha),
                        Ok(intensity),
                        Ok(kind),
                        Ok(td),
                    ) => Some(PdRenderInput {
                        session_id: session.trim().to_string(),
                        dominant_concept: concept.trim().to_string(),
                        accumulated_mass: m,
                        structural_coherence: c,
                        genius_score: g,
                        estimated_trl: t,
                        probe_hint: question.trim().to_string(),
                        parent_i0: parent_i0.trim().to_string(),
                        next_i0_id: next_i0.trim().to_string(),
                        y_verdict: yv,
                        x_verdict: xv,
                        verification_seal: s,
                        verification_generation: gen,
                        verification_receipt_id: receipt.trim().to_string(),
                        primitive_anchor: pa,
                        xy_digest: xyd,
                        pd_binding_digest: bind,
                        pd_continuum_activation_digest: act,
                        question_increment_digest: qd,
                        pd_output_id: output_id.trim().to_string(),
                        pd_output_digest: od,
                        iz_id: iz_id.trim().to_string(),
                        iz_digest: izd,
                        continuum_completion_digest: cd,
                        continuation_basis: basis.trim().to_string(),
                        living_trust_action_state: action,
                        living_trust_action_mask: action_mask,
                        living_trust_required_action_mask: required_action_mask,
                        living_trust_scientific_project_sha256: project_sha256,
                        project_id: pid,
                        project_context_sha256: pcontext_sha256,
                        project_status: pstatus,
                        novel_factualized: pfactual == 1,
                        novel_trl_level: ptrl,
                        novel_proof_score: pscore,
                        novel_rejection: prejection,
                        living_trust_knowledge_band: band,
                        living_trust_lgc_law: law,
                        living_trust_system_laws_seal: laws,
                        living_trust_sovereign_flags: flags,
                        living_trust_sovereign_value: value,
                        living_trust_sha256: sha,
                        living_trust_intensity: intensity,
                        living_trust_kind: kind,
                        living_trust_digest: td,
                    }),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

impl PdRenderInput {
    pub fn is_verification_complete(&self) -> bool {
        let basis = match PdContinuumBasis::from_label(&self.continuation_basis) {
            Some(v) => v,
            None => return false,
        };
        let bits = crate::lab_contracts::verification_receipt::constitutional_pair(
            self.y_verdict,
            self.x_verdict,
        );
        let expected_seal = crate::sovereign::laws::module_seal(
            crate::lab_contracts::verification_receipt::VERIFICATION_SEAL_NAME,
        );
        let expected_receipt = crate::lab_contracts::verification_receipt::receipt_id(
            &self.session_id,
            &self.parent_i0,
            self.primitive_anchor,
            self.xy_digest,
            self.pd_binding_digest,
            self.pd_continuum_activation_digest,
            &self.living_trust_sha256,
            self.y_verdict,
            self.x_verdict,
            self.verification_generation,
            self.verification_seal,
        );
        let trust_proof = LivingTrustProof {
            action_state: self.living_trust_action_state,
            action_mask: self.living_trust_action_mask,
            required_action_mask: self.living_trust_required_action_mask,
            verified: self.y_verdict,
            primitive: self.x_verdict,
            knowledge_band: self.living_trust_knowledge_band,
            lgc_law: self.living_trust_lgc_law,
            system_laws_seal: self.living_trust_system_laws_seal,
            scientific_project_sha256: self.living_trust_scientific_project_sha256,
            scientific_project_verdict_sha256: crate::gcl_project_contract::verdict_sha256_or_zero(
                crate::gcl_project_contract::ProjectVerdictMaterial {
                    project_id: self.project_id,
                    project_status: self.project_status,
                    project_context_sha256: self.project_context_sha256,
                    project_evidence_sha256: self.living_trust_scientific_project_sha256,
                    novel_factualized: self.novel_factualized,
                    novel_trl_level: self.novel_trl_level,
                    novel_proof_score: self.novel_proof_score,
                    novel_rejection: self.novel_rejection,
                },
            ),
            sovereign_flags: self.living_trust_sovereign_flags,
        };
        let expected_trust =
            match living_trust::compute_with_intensity(trust_proof, self.living_trust_intensity) {
                Some(value) => value,
                None => return false,
            };
        let received_trust = LivingTrustSeal {
            identity_sha256: self.living_trust_sha256,
            intensity: self.living_trust_intensity,
            kind: self.living_trust_kind,
            sovereign_value: self.living_trust_sovereign_value,
        };
        let trust_complete = expected_trust == received_trust
            && self.living_trust_system_laws_seal
                == crate::lab_contracts::gjata_collapse_law::system_laws_seal()
            && self.living_trust_digest == received_trust.identity_digest();
        let project_absent = self.living_trust_scientific_project_sha256 == [0u8; 32]
            && self.project_id == 0
            && self.project_context_sha256 == [0u8; 32];
        let project_present = self.living_trust_scientific_project_sha256 != [0u8; 32]
            && self.project_id != 0
            && self.project_context_sha256 != [0u8; 32];
        let project_complete = match (
            self.project_status,
            self.novel_factualized,
            self.y_verdict,
            self.x_verdict,
        ) {
            (shadow_contracts::PROJECT_STATUS_NONE, false, _, _) => {
                project_absent
                    && self.novel_trl_level == 0
                    && self.novel_proof_score == 0
                    && self.novel_rejection == shadow_contracts::NOVEL_REJECTION_NONE
            }
            (shadow_contracts::PROJECT_STATUS_NOVEL_FACTUAL, true, 1, 1) => {
                project_present
                    && self.novel_trl_level == 4
                    && (self.living_trust_lgc_law & 0x40) != 0
            }
            (shadow_contracts::PROJECT_STATUS_RIGOROUS_NEGATIVE, false, 0, 0) => project_present,
            (shadow_contracts::PROJECT_STATUS_HOLD, false, _, _) => {
                project_present && (self.living_trust_lgc_law & 0x40) == 0
            }
            _ => false,
        };
        let expected_output = continuum::derive_pd_output_digest(
            self.pd_continuum_activation_digest,
            self.pd_binding_digest,
            self.question_increment_digest,
            &self.verification_receipt_id,
            &self.probe_hint,
            self.y_verdict,
            self.x_verdict,
            basis,
        );
        let expected_iz = continuum::derive_iz_digest(
            self.pd_continuum_activation_digest,
            self.question_increment_digest,
            self.pd_output_digest,
            self.living_trust_digest,
            &self.verification_receipt_id,
            basis,
        );
        let contract = PdContinuumCompletion::new(
            self.pd_continuum_activation_digest,
            self.question_increment_digest,
            self.pd_output_digest,
            self.iz_digest,
            self.living_trust_digest,
            self.next_i0_id.clone(),
            &self.verification_receipt_id,
            basis,
        );
        let basis_bound = matches!(
            (basis, self.y_verdict, self.x_verdict),
            (PdContinuumBasis::VerifiedPositive, 1, 1)
                | (PdContinuumBasis::VerifiedNegativeRebuild, 0, 0)
        );
        bits && trust_complete
            && project_complete
            && basis_bound
            && !self.session_id.is_empty()
            && !self.parent_i0.is_empty()
            && self.verification_seal == expected_seal
            && self.verification_generation != 0
            && self.primitive_anchor != 0
            && self.xy_digest != 0
            && self.pd_binding_digest != 0
            && self.pd_continuum_activation_digest != 0
            && self.question_increment_digest != 0
            && self.verification_receipt_id == expected_receipt
            && self.pd_output_digest == expected_output
            && self.pd_output_id == continuum::pd_output_id(expected_output)
            && self.iz_digest == expected_iz
            && self.iz_id == continuum::iz_id(expected_iz)
            && self.next_i0_id == continuum::derive_next_i0_id(expected_output, expected_iz)
            && self.continuum_completion_digest == contract.completion_digest
            && contract.closes_formula(&self.verification_receipt_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_for(session_id: String, parent_i0: String, primitive_anchor: u64) -> PdRenderInput {
        let seal = crate::sovereign::laws::module_seal(
            crate::lab_contracts::verification_receipt::VERIFICATION_SEAL_NAME,
        );
        let generation = 7;
        let xy_digest = 0xBB;
        let pd_binding_digest = 0xCC;
        let activation = 0xDD;
        let question_increment = 0xEE;
        let basis = PdContinuumBasis::VerifiedPositive;
        let trust_proof = LivingTrustProof {
            action_state: 0x1122_3344_5566_7788,
            action_mask: living_trust::REQUIRED_ACTION_MASK,
            required_action_mask: living_trust::REQUIRED_ACTION_MASK,
            verified: 1,
            primitive: 1,
            knowledge_band: 2,
            lgc_law: 0x47,
            system_laws_seal: crate::lab_contracts::gjata_collapse_law::system_laws_seal(),
            scientific_project_sha256: [0u8; 32],
            scientific_project_verdict_sha256: [0u8; 32],
            sovereign_flags: 0xA451,
        };
        let trust = living_trust::compute_with_intensity(trust_proof, 7_500).unwrap();
        let trust_digest = trust.identity_digest();
        let receipt = crate::lab_contracts::verification_receipt::receipt_id(
            &session_id,
            &parent_i0,
            primitive_anchor,
            xy_digest,
            pd_binding_digest,
            activation,
            &trust.identity_sha256,
            1,
            1,
            generation,
            seal,
        );
        let output_digest = continuum::derive_pd_output_digest(
            activation,
            pd_binding_digest,
            question_increment,
            &receipt,
            "Cila është prova?",
            1,
            1,
            basis,
        );
        let iz_digest = continuum::derive_iz_digest(
            activation,
            question_increment,
            output_digest,
            trust_digest,
            &receipt,
            basis,
        );
        let next_i0 = continuum::derive_next_i0_id(output_digest, iz_digest);
        let completion = PdContinuumCompletion::new(
            activation,
            question_increment,
            output_digest,
            iz_digest,
            trust_digest,
            next_i0.clone(),
            &receipt,
            basis,
        );
        PdRenderInput {
            session_id,
            dominant_concept: "bidirectional_elimination".to_string(),
            accumulated_mass: 5_100,
            structural_coherence: 4_200,
            genius_score: 6_300,
            estimated_trl: 1,
            probe_hint: "Cila është prova?".to_string(),
            parent_i0,
            next_i0_id: next_i0,
            y_verdict: 1,
            x_verdict: 1,
            verification_seal: seal,
            verification_generation: generation,
            verification_receipt_id: receipt,
            primitive_anchor,
            xy_digest,
            pd_binding_digest,
            pd_continuum_activation_digest: activation,
            question_increment_digest: question_increment,
            pd_output_id: continuum::pd_output_id(output_digest),
            pd_output_digest: output_digest,
            iz_id: continuum::iz_id(iz_digest),
            iz_digest,
            continuum_completion_digest: completion.completion_digest,
            continuation_basis: basis.label().to_string(),
            living_trust_action_state: trust_proof.action_state,
            living_trust_action_mask: trust_proof.action_mask,
            living_trust_required_action_mask: trust_proof.required_action_mask,
            living_trust_scientific_project_sha256: trust_proof.scientific_project_sha256,
            project_id: 0,
            project_context_sha256: [0u8; 32],
            project_status: shadow_contracts::PROJECT_STATUS_NONE,
            novel_factualized: false,
            novel_trl_level: 0,
            novel_proof_score: 0,
            novel_rejection: 0,
            living_trust_knowledge_band: trust_proof.knowledge_band,
            living_trust_lgc_law: trust_proof.lgc_law,
            living_trust_system_laws_seal: trust_proof.system_laws_seal,
            living_trust_sovereign_flags: trust_proof.sovereign_flags,
            living_trust_sovereign_value: trust.sovereign_value,
            living_trust_sha256: trust.identity_sha256,
            living_trust_intensity: trust.intensity,
            living_trust_kind: trust.kind,
            living_trust_digest: trust_digest,
        }
    }

    fn sample() -> PdRenderInput {
        sample_for("sess-1".to_string(), "i0-parent-A".to_string(), 0xAA)
    }

    fn completed_response() -> crate::light_coordinator::LightResponse {
        let coordinator = crate::light_coordinator::LightCoordinator::new();
        let mut response = coordinator.receive(crate::light_coordinator::LightRequest::new(
            "Analizo motorin industrial të kuq dhe sensorin termik",
        ));
        let legacy = response.legacy.as_ref().expect("Legacy coordination");
        let evidence = crate::alnur_karina_athar::HandoffEvidence {
            binding_sha256: legacy.receipt.binding_sha256.clone(),
            receipt_sha256: legacy.receipt.receipt_sha256.clone(),
            besa_attestation_sha256: legacy.besa_attestation.attestation_sha256(),
            besa_enrichment_sha256: legacy.besa_attestation.enrichment_sha256.clone(),
            commit_sha256: "44".repeat(32),
            primitive_anchor_sha256: "55".repeat(32),
            receipt_written: true,
            besa_written: true,
            primitive_anchor_written: true,
            commit_written: true,
            flushed: true,
            kodunik_verified: true,
        };
        response
            .complete_legacy_handoff(&evidence)
            .expect("evidence-bound handoff");
        response
    }

    fn terminal_result(
        response: &crate::light_coordinator::LightResponse,
        positive: bool,
    ) -> crate::quantum_bridge::QuantumResult {
        let (verdict, potentiality, axis_score, evolve_sfx, algorithm_state) = match positive {
            true => ("OPTIMAL", 0.9, 90.0, "VFYKNG", "SEALED"),
            false => ("HOLD", 0.2, -80.0, "NEGKNG", "SHADOW_VERIFY"),
        };
        crate::quantum_bridge::QuantumResult {
            trace_id: response.trace_id.clone(),
            verdict: verdict.to_string(),
            potentiality,
            axis_score,
            seal: format!("QNT:{}:0123456789abcdef", response.trace_id),
            evolve_sfx: evolve_sfx.to_string(),
            algorithm_state: algorithm_state.to_string(),
            territory: "INDUSTRIAL".to_string(),
            depth_hint: "DEEP".to_string(),
            content: "terminal runtime result".to_string(),
            ts: 100,
            pd_continuity: None,
            is_verified: positive,
        }
    }

    #[test]
    fn complete_continuum_reaches_nura_surface() {
        let input = sample();
        assert!(input.is_verification_complete());
        let out = PdLight::render(&input).expect("surface e verifikuar");
        assert!(out.as_str().contains("PD output="));
        assert!(out.as_str().contains("+ iZ="));
    }

    #[test]
    fn candidate_or_iz_tamper_is_rejected() {
        let mut input = sample();
        input.iz_digest ^= 1;
        assert!(!input.is_verification_complete());
        assert!(PdLight::render(&input).is_none());
    }

    #[test]
    fn living_trust_tamper_is_rejected() {
        let mut input = sample();
        input.living_trust_sha256[0] ^= 1;
        assert!(!input.is_verification_complete());
        assert!(PdLight::render(&input).is_none());
    }

    #[test]
    fn verified_output_text_tamper_is_rejected() {
        let mut input = sample();
        input.probe_hint.push_str(" e ndryshuar");
        assert!(!input.is_verification_complete());
        assert!(PdLight::render(&input).is_none());
    }

    #[test]
    fn delivery_routes_same_verified_iz_to_nura_and_emotional_ui() {
        let input = sample();
        let delivery = PdLight::deliver(&input).expect("delivery i verifikuar");
        assert_eq!(delivery.surface, PdLight::render(&input).unwrap());
        assert_eq!(delivery.ui_signal.iz_id, input.iz_id);
        assert_eq!(delivery.ui_signal.next_i0_id, input.next_i0_id);
        assert_ne!(delivery.ui_signal.continuity_digest, 0);
    }

    #[test]
    fn terminal_receipt_closes_only_the_same_light_gcl_cycle_and_verdict() {
        let response = completed_response();
        let input = sample_for(
            response.trace_id.clone(),
            format!("i0-{}", response.primitive_trace.id_hex()),
            response.primitive_trace.id,
        );
        let delivery = PdLight::deliver(&input).expect("verified terminal receipt");
        let positive = terminal_result(&response, true);
        assert!(delivery.closes_pre_dispatch_cycle(&response, &positive));
        assert!(delivery
            .terminal_receipt
            .closes_light_cycle(&response, &positive));

        let negative = terminal_result(&response, false);
        assert!(!delivery
            .terminal_receipt
            .closes_light_cycle(&response, &negative));

        let mut drifted = delivery.terminal_receipt.clone();
        let original_sha256 = drifted.evidence_sha256();
        drifted.xy_digest ^= 1;
        assert_ne!(drifted.evidence_sha256(), original_sha256);
        assert!(!drifted.closes_light_cycle(&response, &positive));
    }

    #[test]
    fn continuum_basis_must_match_the_constitutional_shadow_pair() {
        let mut input = sample();
        input.continuation_basis = PdContinuumBasis::VerifiedNegativeRebuild
            .label()
            .to_string();
        assert!(!input.is_verification_complete());
        assert!(PdLight::deliver(&input).is_none());
    }

    #[test]
    fn round_trip_45_body_fields() {
        let e = sample();
        let line = format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{:016x}|{}|{}|{}|{:016x}|{:016x}|{:016x}|{:016x}|{:016x}|{}|{:016x}|{}|{:016x}|{:016x}|{:016x}|{:016x}|{:016x}|{}|{:016x}|{}|{}|{}|{}|{}|{}|{}|{:08x}|{:016x}|{:08x}|{:08x}|{}|{}|{}|{:016x}",
            e.session_id, e.dominant_concept, e.accumulated_mass,
            e.structural_coherence, e.genius_score, e.estimated_trl,
            e.probe_hint, e.parent_i0, e.next_i0_id, e.y_verdict, e.x_verdict,
            e.verification_seal, e.verification_generation,
            e.verification_receipt_id, e.continuation_basis,
            e.primitive_anchor, e.xy_digest, e.pd_binding_digest,
            e.pd_continuum_activation_digest, e.question_increment_digest,
            e.pd_output_id, e.pd_output_digest, e.iz_id, e.iz_digest,
            e.continuum_completion_digest, e.living_trust_action_state,
            e.living_trust_action_mask, e.living_trust_required_action_mask,
            crate::gcl_project_contract::sha256_hex(&e.living_trust_scientific_project_sha256), e.project_id,
            crate::gcl_project_contract::sha256_hex(&e.project_context_sha256), e.project_status,
            e.novel_factualized as u8, e.novel_trl_level,
            e.novel_proof_score, e.novel_rejection,
            e.living_trust_knowledge_band, e.living_trust_lgc_law,
            e.living_trust_system_laws_seal, e.living_trust_sovereign_flags,
            e.living_trust_sovereign_value,
            living_trust::hex_32(&e.living_trust_sha256),
            e.living_trust_intensity, e.living_trust_kind,
            e.living_trust_digest,
        );
        assert_eq!(line.split('|').count(), 45);
        assert_eq!(PdLight::parse_handoff(&line), Some(e));
    }
}

#[cfg(test)]
impl PdLight {
    pub fn render(input: &PdRenderInput) -> Option<VerifiedPdSurface> {
        Self::deliver(input).map(|delivery| delivery.surface)
    }
}
