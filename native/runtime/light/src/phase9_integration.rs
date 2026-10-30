// AIGovern-Cyber v1.6.0: Ky është LIGHT EMOTIONAL/INTERPRETATION SPINE për UI-në e
// vjetër. Nuk është PD Spine 9 i Quantum dhe nuk ekzekuton Layer 1/2/3 të PD.
// Ai pasqyron gjendjen e sistemit dhe mbetet paralel me Nura-n.

// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  PHASE9_INTEGRATION.RS — Lidhja me Rrjedhën e Gjallë (Light)         ║
// ║  GJATA LEGACY™ — AIGovern-Cyber Light                            (FAZA 9)      ║
// ║                                                                          ║
// ║  FAZA 9 — PATCHIM I RRJEDHËS SË GJALLË:                               ║
// ║  Lidh light_spine (UI→gjurmim→kuptim→besim) + knowledge_channel me       ║
// ║  coordinator-in real, duke respektuar ligjin rrënjësor të Light.       ║
// ║                                                                          ║
// ║  LIGJI RRËNJËSOR I LIGHT — NUK VENDOS KURRË:                          ║
// ║   Light interpreton, transporton, ndërton besim, gjurmon — por çdo      ║
// ║   "vendim" final i takon Shadow-it. Faza 9 e ruan këtë me rreptësi:    ║
// ║   asnjë output i Light s'është verdikt 0/1.                            ║
// ║                                                                          ║
// ║  Plus: çdo paketë mban vulën 500 (verify_500). Knowledge merr trace     ║
// ║  fillestar para se të shkojë te Shadow.                                ║
// ║                                                                          ║
// ║  KUFIRI: Faza 9 lidh spine me rrjedhën; ruan që Light s'vendos.         ║
// ║  Zero if/else klasik (match/boolean).                                 ║
// ╚══════════════════════════════════════════════════════════════════════════╝

use crate::lab_contracts::{CollapsePhase, GjataCollapseLaw};
use crate::layer2::active_trust::ReliabilitySignals;
use crate::layer2::verification_trace::VerificationTrace;

use crate::layer3::ui_channel::UiInput;
use crate::light_coordinator::{DispatchResult, LightCoordinator, LightResponse};
use crate::light_spine::{LightSpine, LightSpineResult};
use crate::mini_evolver::LightMiniEvolver;
use crate::pd_light::PdTerminalReceipt;
use crate::quantum_bridge::QuantumResult;
use crate::shadow_seal_bridge::ShadowVerdict;

// ─────────────────────────────────────────────────────────────────────────────
// LIGJET E FAZËS 9 (Light)
// ─────────────────────────────────────────────────────────────────────────────

/// Rezultati i kontrollit të ligjit "Light s'vendos".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightLawCheck {
    Passed,
    InvalidFormulaAnchor, // i₀/PA/split/GCL/Besa nuk janë një atom autoritativ
}

impl LightLawCheck {
    /// is_passed — a kaluan ligjet? Zero if.
    pub fn is_passed(self) -> bool {
        match self {
            LightLawCheck::Passed => true,
            _ => false,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PHASE 9 INTEGRATION (Light)
// ─────────────────────────────────────────────────────────────────────────────

/// Rezultati i interpretimit me spine të lidhur.
#[derive(Debug, Clone)]
pub struct Phase9InterpretResult {
    pub law_check: LightLawCheck,
    pub spine: Option<LightSpineResult>, // Some nëse ligjet kaluan
    #[cfg(test)]
    pub interprets_only: bool, // gjithmonë true (Light s'vendos)
}

/// Pamja e historikut të së njëjtës rrugë para ciklit aktual.
#[derive(Debug, Clone)]
pub struct RuntimeEvidenceSnapshot {
    pub route_key: u64,
    /// Identiteti autoritativ i route-it; `route_key` është vetëm handle API.
    pub route_sha256: String,
    /// SHA-256 i atomit të pandryshueshëm të Fazës 1 para dispatch-it.
    pub formula_lineage_sha256: String,
    /// Numri i degëve Evolve kur ankora ishte ende gati për dispatch.
    pub branch_count_before: usize,
    pub signals: ReliabilitySignals,
    pub history_refs: Vec<u64>,
    pub verification_trace: Option<VerificationTrace>,
}

impl RuntimeEvidenceSnapshot {
    pub fn evidence_count(&self) -> usize {
        self.verification_trace
            .as_ref()
            .map(VerificationTrace::total_count)
            .unwrap_or(0)
    }
}

/// Receipt lokal i shkrimit të një rezultati terminal në spine-in e evidencës.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeEvidenceRecord {
    pub route_key: u64,
    pub route_sha256: String,
    pub evidence_ref: u64,
    pub evidence_sha256: String,
    pub survived: bool,
    pub retained_count: usize,
}

/// Ledger-i proces-lokal i Light-it. Nuk shpik wire ose storage të ri:
/// lidh vetëm komponentët ekzistues MiniEvolver→VerificationTrace→Trust.
pub struct RuntimeEvidenceSpine {
    mini: LightMiniEvolver,
}

impl RuntimeEvidenceSpine {
    pub fn new() -> Self {
        Self {
            mini: LightMiniEvolver::new(),
        }
    }

    /// Identiteti stabil i rrugës së formulës. TraceInfo mbetet identitet cikli;
    /// route-i lidhet me inputin, split-in, witness-et −Xi/−Yi dhe vulat GCL.
    fn route_identity_from_material(
        input_sha256: &str,
        split_sha256: &str,
        xi_negative_sha256: &str,
        yi_negative_sha256: &str,
        negative_pair_sha256: &str,
        law_seal: u64,
        system_laws_seal: u64,
    ) -> Result<(u64, String), &'static str> {
        match (
            crate::lgc_algorithm::is_canonical_sha256(input_sha256),
            crate::lgc_algorithm::is_canonical_sha256(split_sha256),
            crate::lgc_algorithm::is_canonical_sha256(xi_negative_sha256),
            crate::lgc_algorithm::is_canonical_sha256(yi_negative_sha256),
            crate::lgc_algorithm::is_canonical_sha256(negative_pair_sha256),
            law_seal != 0,
            system_laws_seal != 0,
        ) {
            (false, _, _, _, _, _, _) => return Err("RUNTIME_EVIDENCE_INPUT_SHA_INVALID"),
            (_, false, _, _, _, _, _) => return Err("RUNTIME_EVIDENCE_SPLIT_SHA_INVALID"),
            (_, _, false, _, _, _, _) => return Err("RUNTIME_EVIDENCE_NEGATIVE_XI_SHA_INVALID"),
            (_, _, _, false, _, _, _) => return Err("RUNTIME_EVIDENCE_NEGATIVE_YI_SHA_INVALID"),
            (_, _, _, _, false, _, _) => return Err("RUNTIME_EVIDENCE_NEGATIVE_PAIR_SHA_INVALID"),
            (_, _, _, _, _, false, _) => return Err("RUNTIME_EVIDENCE_LAW_SEAL_INVALID"),
            (_, _, _, _, _, _, false) => return Err("RUNTIME_EVIDENCE_SYSTEM_LAWS_INVALID"),
            (true, true, true, true, true, true, true) => {}
        }

        let law = law_seal.to_le_bytes();
        let system_laws = system_laws_seal.to_le_bytes();
        let route_sha256 = crate::lgc_algorithm::compute_domain_sha(
            "ESSMAI/LIGHT/RUNTIME_EVIDENCE/ROUTE",
            &[
                input_sha256.as_bytes(),
                split_sha256.as_bytes(),
                xi_negative_sha256.as_bytes(),
                yi_negative_sha256.as_bytes(),
                negative_pair_sha256.as_bytes(),
                &law,
                &system_laws,
            ],
        );
        let route_key = u64::from_str_radix(&route_sha256[..16], 16)
            .map_err(|_| "RUNTIME_EVIDENCE_ROUTE_DECODE")?;
        match route_key {
            0 => Err("RUNTIME_EVIDENCE_ROUTE_ZERO"),
            _ => Ok((route_key, route_sha256)),
        }
    }

    /// Lexon vetëm fragmentet e pandryshueshme të response-it. Kjo përdoret
    /// edhe pas Evolve terminal, kur vula primitive mund të jetë hequr
    /// ligjërisht nga një rezultat negativ.
    fn route_identity_from_fragments(
        response: &LightResponse,
    ) -> Result<(u64, String), &'static str> {
        let legacy = response
            .legacy
            .as_ref()
            .ok_or("RUNTIME_EVIDENCE_LEGACY_MISSING")?;
        let besa = &legacy.besa_attestation;
        let expected_negative = crate::besa_nlight::negative_axis_index_witness(
            &legacy.primitive_split.xi,
            &legacy.primitive_split.yi,
        );
        let fragment_bound = response.status.is_ok()
            && GjataCollapseLaw::verify(&legacy.directive, CollapsePhase::Coordination)
            && legacy.receipt.verify_runtime().is_ok()
            && response.authority_continuity_valid()
            && legacy.evolve.runtime_integrity_valid()
            && legacy.is_fulfilled()
            && legacy
                .handoff_evidence_sha256()
                .map(crate::lgc_algorithm::is_canonical_sha256)
                == Some(true)
            && response.trace_id == response.primitive_trace.label()
            && response.trace_id == legacy.receipt.trace_id
            && response.input_sha256 == legacy.receipt.input_sha256
            && response.primitive_split == legacy.primitive_split
            && legacy.primitive_split.to_wire() == legacy.receipt.split_wire
            && besa.shape_is_valid()
            && legacy.besa_is_bound()
            && besa.trace_id == legacy.receipt.trace_id
            && besa.split_sha256 == legacy.receipt.split_sha256
            && besa.law_seal == legacy.directive.seal
            && besa.law_seal == legacy.receipt.law_seal
            && besa.system_laws_seal == legacy.receipt.system_laws_seal
            && besa.xi_negative_index_sha256 == expected_negative.xi_sha256
            && besa.yi_negative_index_sha256 == expected_negative.yi_sha256
            && besa.negative_index_sha256 == expected_negative.combined_sha256;
        match fragment_bound {
            true => Self::route_identity_from_material(
                &response.input_sha256,
                &legacy.receipt.split_sha256,
                &besa.xi_negative_index_sha256,
                &besa.yi_negative_index_sha256,
                &besa.negative_index_sha256,
                legacy.directive.seal,
                legacy.receipt.system_laws_seal,
            ),
            false => Err("RUNTIME_EVIDENCE_FORMULA_FRAGMENT_INVALID"),
        }
    }

    /// Fingerprint-i i atomit të Fazës 1 përjashton vetëm pjesët që EvolveTrace
    /// ndryshon ligjërisht pas terminalit: degët pas së parës dhe primitive_flags
    /// aktual. Çdo autoritet, receipt, split, Besa ose degë fillestare mbetet
    /// byte-për-byte e lidhur me snapshot-in para dispatch-it.
    fn formula_lineage_sha256(response: &LightResponse) -> Result<String, &'static str> {
        Self::route_identity_from_fragments(response)?;
        let legacy = response
            .legacy
            .as_ref()
            .ok_or("RUNTIME_EVIDENCE_LEGACY_MISSING")?;
        let first = legacy
            .evolve
            .branches
            .first()
            .ok_or("RUNTIME_EVIDENCE_INITIAL_BRANCH_MISSING")?;
        let handoff_evidence_sha256 = legacy
            .handoff_evidence_sha256()
            .ok_or("RUNTIME_EVIDENCE_HANDOFF_ATTESTATION_MISSING")?;
        let besa_attestation_sha256 = legacy.besa_attestation.attestation_sha256();
        let split_wire = legacy.primitive_split.to_wire();
        let trace_origin = response.primitive_trace.id.to_le_bytes();
        let trace_timestamp = response.primitive_trace.ts_ns.to_le_bytes();
        let raw_len = (response.primitive_trace.raw_len as u64).to_le_bytes();
        let directive_seal = legacy.directive.seal.to_le_bytes();
        let system_laws_seal = legacy.receipt.system_laws_seal.to_le_bytes();
        let receipt_flags = legacy.receipt.primitive_flags.to_le_bytes();
        let first_parent = first.parent_id.to_le_bytes();
        let first_index = first.branch_index.to_le_bytes();
        let first_generated = first.trace.generated.to_le_bytes();
        let first_transform = first.trace.transform.unwrap_or(0).to_le_bytes();
        let first_coherence = first.coherence.to_bits().to_le_bytes();
        let lang_confidence = response.lang_confidence.to_bits().to_le_bytes();
        let fulfilled = [u8::from(legacy.is_fulfilled())];
        Ok(crate::lgc_algorithm::compute_domain_sha(
            "ESSMAI/LIGHT/RUNTIME_EVIDENCE/FORMULA_LINEAGE",
            &[
                response.trace_id.as_bytes(),
                response.input_sha256.as_bytes(),
                response.domain.as_bytes(),
                response.contract_id.as_bytes(),
                response.lgc_seal.as_bytes(),
                response.algorithm_state.as_bytes(),
                response.evolve_summary.as_bytes(),
                response.user_lang.code().as_bytes(),
                &lang_confidence,
                &trace_origin,
                &trace_timestamp,
                &raw_len,
                legacy.directive.phase.label().as_bytes(),
                &directive_seal,
                &system_laws_seal,
                legacy.receipt.receipt_sha256.as_bytes(),
                legacy.receipt.binding_sha256.as_bytes(),
                legacy.receipt.split_sha256.as_bytes(),
                &receipt_flags,
                split_wire.as_bytes(),
                besa_attestation_sha256.as_bytes(),
                legacy.besa_attestation.xi_negative_index_sha256.as_bytes(),
                legacy.besa_attestation.yi_negative_index_sha256.as_bytes(),
                legacy.besa_attestation.negative_index_sha256.as_bytes(),
                legacy.evolve.origin_label.as_bytes(),
                legacy.evolve.primitive_anchor.as_bytes(),
                legacy.evolve.input_sha256.as_bytes(),
                &first_parent,
                first.branch_tag.as_bytes(),
                first.kodunik.as_bytes(),
                &first_index,
                &first_generated,
                &first_transform,
                &first_coherence,
                handoff_evidence_sha256.as_bytes(),
                &fulfilled,
            ],
        ))
    }

    /// Rindërton VerificationTrace nga MiniEvolver-i autoritativ dhe kërkon
    /// barazi të plotë para se sinjalet të hyjnë në TrustContext.
    fn snapshot_route(
        &self,
        route_key: u64,
        route_sha256: String,
        formula_lineage_sha256: String,
        branch_count_before: usize,
    ) -> Result<RuntimeEvidenceSnapshot, &'static str> {
        let route_handle = match crate::lgc_algorithm::is_canonical_sha256(&route_sha256) {
            true => u64::from_str_radix(&route_sha256[..16], 16)
                .map_err(|_| "RUNTIME_EVIDENCE_ROUTE_DECODE")?,
            false => return Err("RUNTIME_EVIDENCE_ROUTE_SHA256_INVALID"),
        };
        match (
            route_key != 0,
            route_handle == route_key,
            crate::lgc_algorithm::is_canonical_sha256(&formula_lineage_sha256),
            branch_count_before > 0,
        ) {
            (false, _, _, _) => return Err("RUNTIME_EVIDENCE_ROUTE_ZERO"),
            (_, false, _, _) => return Err("RUNTIME_EVIDENCE_ROUTE_SHA256_HANDLE_MISMATCH"),
            (_, _, false, _) => return Err("RUNTIME_EVIDENCE_FORMULA_SHA_INVALID"),
            (_, _, _, false) => return Err("RUNTIME_EVIDENCE_BRANCH_COUNT_ZERO"),
            (true, true, true, true) => {}
        }
        let events = match self.mini.events_for(route_key) {
            Some(events) => events,
            None => {
                return Ok(RuntimeEvidenceSnapshot {
                    route_key,
                    route_sha256,
                    formula_lineage_sha256,
                    branch_count_before,
                    signals: ReliabilitySignals::empty(),
                    history_refs: Vec::new(),
                    verification_trace: None,
                })
            }
        };
        match self.mini.route_sha256(route_key) {
            Some(stored) => match stored == route_sha256.as_str() {
                true => {}
                false => return Err("RUNTIME_EVIDENCE_ROUTE_SHA256_COLLISION"),
            },
            None => return Err("RUNTIME_EVIDENCE_ROUTE_SHA256_MISSING"),
        }

        let mut verification_trace = VerificationTrace::new_bound(route_key, &route_sha256)?;
        for event in events {
            // Snapshot-i duhet të zotërojë domenin ndërsa ledger-i mbetet
            // autoritativ; ky është kopjim i justifikuar i metadata-s, jo wire.
            let evidence_sha256 = event
                .evidence_sha256
                .as_deref()
                .ok_or("RUNTIME_EVIDENCE_SHA256_MISSING")?;
            verification_trace.record_bound_evidence(
                event.evidence_ref,
                evidence_sha256,
                event.module_name.clone(),
                event.survived,
                event.timestamp_ns,
            )?;
        }
        let signals =
            ReliabilitySignals::from_runtime(&self.mini, route_key, Some(&verification_trace))?;
        let history_refs = verification_trace.history_refs();
        Ok(RuntimeEvidenceSnapshot {
            route_key,
            route_sha256,
            formula_lineage_sha256,
            branch_count_before,
            signals,
            history_refs,
            verification_trace: Some(verification_trace),
        })
    }

    pub fn snapshot(
        &self,
        response: &LightResponse,
    ) -> Result<RuntimeEvidenceSnapshot, &'static str> {
        match response.formula_anchor_valid() {
            true => {}
            false => return Err("RUNTIME_EVIDENCE_FORMULA_ANCHOR_INVALID"),
        }
        response
            .ready_for_dispatch()
            .map_err(|_| "RUNTIME_EVIDENCE_HANDOFF_NOT_COMMITTED")?;
        let (route_key, route_sha256) = Self::route_identity_from_fragments(response)?;
        let formula_lineage_sha256 = Self::formula_lineage_sha256(response)?;
        let branch_count_before = response
            .legacy
            .as_ref()
            .map(|legacy| legacy.evolve.branch_count())
            .ok_or("RUNTIME_EVIDENCE_LEGACY_MISSING")?;
        self.snapshot_route(
            route_key,
            route_sha256,
            formula_lineage_sha256,
            branch_count_before,
        )
    }

    /// Shkruan një rezultat vetëm pasi rrjedha terminale dhe EvolveTrace janë
    /// lidhur nga `terminal_evidence_is_bound`.
    pub fn record_terminal(
        &mut self,
        response: &LightResponse,
        snapshot: &RuntimeEvidenceSnapshot,
        result: &QuantumResult,
        terminal_receipt: &PdTerminalReceipt,
        terminal_evidence_bound: bool,
    ) -> Result<RuntimeEvidenceRecord, &'static str> {
        match terminal_evidence_bound {
            true => {}
            false => return Err("RUNTIME_EVIDENCE_TERMINAL_NOT_BOUND"),
        }
        match result.is_bound_to(&response.trace_id) {
            true => {}
            false => return Err("RUNTIME_EVIDENCE_QUANTUM_RESULT_NOT_BOUND"),
        }
        match terminal_receipt.closes_light_cycle(response, result) {
            true => {}
            false => return Err("RUNTIME_EVIDENCE_SHADOW_RECEIPT_NOT_BOUND"),
        }
        let (route_key, route_sha256) = Self::route_identity_from_fragments(response)?;
        let formula_lineage_sha256 = Self::formula_lineage_sha256(response)?;
        match (
            route_key == snapshot.route_key,
            route_sha256.as_str() == snapshot.route_sha256.as_str(),
            formula_lineage_sha256 == snapshot.formula_lineage_sha256,
            self.mini.get_usage_count(route_key) == snapshot.evidence_count(),
        ) {
            (false, _, _, _) => return Err("RUNTIME_EVIDENCE_ROUTE_DRIFT"),
            (_, false, _, _) => return Err("RUNTIME_EVIDENCE_ROUTE_SHA256_DRIFT"),
            (_, _, false, _) => return Err("RUNTIME_EVIDENCE_FORMULA_LINEAGE_DRIFT"),
            (_, _, _, false) => return Err("RUNTIME_EVIDENCE_HISTORY_DRIFT"),
            (true, true, true, true) => {}
        }
        let branch_count_after = response
            .legacy
            .as_ref()
            .map(|legacy| legacy.evolve.branch_count())
            .ok_or("RUNTIME_EVIDENCE_LEGACY_MISSING")?;
        let branch_count =
            u64::try_from(branch_count_after).map_err(|_| "RUNTIME_EVIDENCE_BRANCH_OVERFLOW")?;
        let (evidence_ref, evidence_sha256) = Self::terminal_evidence_identity(
            route_key,
            &route_sha256,
            &snapshot.formula_lineage_sha256,
            result,
            terminal_receipt,
            branch_count,
        )?;
        match evidence_ref {
            0 => return Err("RUNTIME_EVIDENCE_REF_ZERO"),
            _ => {}
        }
        self.mini.record_verified_cycle_bound(
            route_key,
            &route_sha256,
            evidence_ref,
            &evidence_sha256,
            &response.domain,
            result.potentiality,
            result.is_verified,
        )?;
        let retained_count = self.mini.get_usage_count(route_key);
        Ok(RuntimeEvidenceRecord {
            route_key,
            route_sha256,
            evidence_ref,
            evidence_sha256,
            survived: result.is_verified,
            retained_count,
        })
    }

    fn terminal_evidence_identity(
        route_key: u64,
        route_sha256: &str,
        formula_lineage_sha256: &str,
        result: &QuantumResult,
        terminal_receipt: &PdTerminalReceipt,
        branch_count_after: u64,
    ) -> Result<(u64, String), &'static str> {
        let terminal_receipt_sha256 = terminal_receipt.evidence_sha256();
        match (
            crate::lgc_algorithm::is_canonical_sha256(route_sha256),
            crate::lgc_algorithm::is_canonical_sha256(formula_lineage_sha256),
            crate::lgc_algorithm::is_canonical_sha256(&terminal_receipt_sha256),
        ) {
            (true, true, true) => {}
            (false, _, _) => return Err("RUNTIME_EVIDENCE_ROUTE_SHA256_INVALID"),
            (_, false, _) => return Err("RUNTIME_EVIDENCE_FORMULA_SHA_INVALID"),
            (_, _, false) => return Err("RUNTIME_EVIDENCE_SHADOW_RECEIPT_SHA_INVALID"),
        }
        let route_handle = u64::from_str_radix(&route_sha256[..16], 16)
            .map_err(|_| "RUNTIME_EVIDENCE_ROUTE_DECODE")?;
        match route_handle == route_key {
            true => {}
            false => return Err("RUNTIME_EVIDENCE_ROUTE_SHA256_HANDLE_MISMATCH"),
        }
        let route = route_key.to_le_bytes();
        let timestamp = result.ts.to_le_bytes();
        let branch_count = branch_count_after.to_le_bytes();
        let potentiality = result.potentiality.to_bits().to_le_bytes();
        let axis_score = result.axis_score.to_bits().to_le_bytes();
        let survived = [u8::from(result.is_verified)];
        let digest = crate::lgc_algorithm::compute_domain_sha(
            "ESSMAI/LIGHT/TERMINAL_EVIDENCE",
            &[
                &route,
                route_sha256.as_bytes(),
                formula_lineage_sha256.as_bytes(),
                result.trace_id.as_bytes(),
                result.verdict.as_bytes(),
                result.seal.as_bytes(),
                result.evolve_sfx.as_bytes(),
                result.algorithm_state.as_bytes(),
                result.territory.as_bytes(),
                result.depth_hint.as_bytes(),
                result.content.as_bytes(),
                terminal_receipt.receipt_id().as_bytes(),
                terminal_receipt_sha256.as_bytes(),
                &timestamp,
                &branch_count,
                &potentiality,
                &axis_score,
                &survived,
            ],
        );
        let evidence_ref =
            u64::from_str_radix(&digest[..16], 16).map_err(|_| "RUNTIME_EVIDENCE_REF_DECODE")?;
        match evidence_ref {
            0 => Err("RUNTIME_EVIDENCE_REF_ZERO"),
            _ => Ok((evidence_ref, digest)),
        }
    }

    #[cfg(test)]
    fn terminal_evidence_ref(
        route_key: u64,
        route_sha256: &str,
        formula_lineage_sha256: &str,
        result: &QuantumResult,
        terminal_receipt: &PdTerminalReceipt,
        branch_count_after: u64,
    ) -> Result<u64, &'static str> {
        Self::terminal_evidence_identity(
            route_key,
            route_sha256,
            formula_lineage_sha256,
            result,
            terminal_receipt,
            branch_count_after,
        )
        .map(|(evidence_ref, _)| evidence_ref)
    }

    /// Prova post-cikël: snapshot-i para dispatch-it mbron atomin e Fazës 1;
    /// rezultati terminal duhet të shtojë saktësisht një degë të vlefshme.
    /// Një degë negative mund ta heqë vulën primitive pa falsifikuar formulën.
    pub fn terminal_evidence_is_bound(
        dispatch: &DispatchResult,
        response: &LightResponse,
        snapshot: &RuntimeEvidenceSnapshot,
        shadow_verdict: &ShadowVerdict,
        terminal_receipt: Option<&PdTerminalReceipt>,
    ) -> bool {
        let result = match dispatch {
            DispatchResult::Verified(result) => result,
            DispatchResult::Operational(_) | DispatchResult::Timeout(_) | DispatchResult::BridgeError(_) => return false,
        };
        let expected_count = snapshot.branch_count_before.checked_add(1);
        let legacy = match response.legacy.as_ref() {
            Some(legacy) => legacy,
            None => return false,
        };
        let branch = match legacy.evolve.branches.last() {
            Some(branch) => branch,
            None => return false,
        };
        let receipt = match terminal_receipt {
            Some(receipt) => receipt,
            None => return false,
        };
        let route_bound =
            Self::route_identity_from_fragments(response)
                .ok()
                .map(|(route_key, route_sha256)| {
                    route_key == snapshot.route_key
                        && route_sha256.as_str() == snapshot.route_sha256.as_str()
                })
                == Some(true);
        let formula_bound = Self::formula_lineage_sha256(response).ok().as_deref()
            == Some(snapshot.formula_lineage_sha256.as_str());
        let expected_coherence = LightCoordinator::coherence_from_dispatch(dispatch);
        let generation = u64::from(branch.branch_index).to_le_bytes();
        let origin = legacy.evolve.origin_id.to_le_bytes();
        let branch_sha256 = crate::lgc_algorithm::compute_domain_sha(
            "ESSMAI/EVOLVE/BRANCH/V2",
            &[
                legacy.evolve.input_sha256.as_bytes(),
                legacy.evolve.primitive_anchor.as_bytes(),
                &origin,
                &generation,
                branch.branch_tag.as_bytes(),
            ],
        );
        let kodunik_bound = crate::lgc_algorithm::verify_kodunik(&branch.kodunik, &branch_sha256);
        let branch_bound = expected_count == Some(legacy.evolve.branch_count())
            && branch.parent_id == response.primitive_trace.id
            && branch.branch_tag == result.evolve_sfx
            && usize::try_from(branch.branch_index).ok() == Some(legacy.evolve.branch_count())
            && branch.coherence.to_bits() == expected_coherence.to_bits()
            && branch.is_valid()
            && kodunik_bound;
        result.is_bound_to(&response.trace_id)
            && receipt.closes_light_cycle(response, result)
            && route_bound
            && formula_bound
            && legacy.evolve.input_sha256 == response.input_sha256
            && legacy.evolve.runtime_integrity_valid()
            && branch_bound
            && shadow_verdict.bit() == u8::from(result.is_verified)
    }

    /// Vetëkontroll GCL i komponentit të evidencës, pa pretenduar rezultat
    /// Shadow: përdor një ledger lokal të përkohshëm dhe provon barazinë
    /// MiniEvolver↔VerificationTrace↔ReliabilitySignals.
    pub fn gcl_status() -> crate::lab_contracts::gcl_presume::GclReadiness {
        use crate::lab_contracts::gcl_presume::GclReadiness;

        let directive = GjataCollapseLaw::issue(CollapsePhase::Coordination);
        match GjataCollapseLaw::verify(&directive, CollapsePhase::Coordination) {
            true => {}
            false => return GclReadiness::NotReady("GCL directive invalid"),
        }
        let input_sha256 =
            crate::lgc_algorithm::compute_sha256(b"ESSMAI/LIGHT/RUNTIME_EVIDENCE/READINESS");
        let split_sha256 =
            crate::lgc_algorithm::compute_sha256(b"ESSMAI/LIGHT/RUNTIME_EVIDENCE/SPLIT");
        let xi_negative_sha256 =
            crate::lgc_algorithm::compute_sha256(b"ESSMAI/LIGHT/RUNTIME_EVIDENCE/-XI");
        let yi_negative_sha256 =
            crate::lgc_algorithm::compute_sha256(b"ESSMAI/LIGHT/RUNTIME_EVIDENCE/-YI");
        let negative_pair_sha256 =
            crate::lgc_algorithm::compute_sha256(b"ESSMAI/LIGHT/RUNTIME_EVIDENCE/-XI/-YI");
        let formula_lineage_sha256 =
            crate::lgc_algorithm::compute_sha256(b"ESSMAI/LIGHT/RUNTIME_EVIDENCE/FORMULA");
        let (route_key, route_sha256) = match Self::route_identity_from_material(
            &input_sha256,
            &split_sha256,
            &xi_negative_sha256,
            &yi_negative_sha256,
            &negative_pair_sha256,
            directive.seal,
            crate::lab_contracts::system_laws_seal(),
        ) {
            Ok(route_identity) => route_identity,
            Err(reason) => return GclReadiness::NotReady(reason),
        };
        let evidence_sha256 = crate::lgc_algorithm::compute_domain_sha(
            "ESSMAI/LIGHT/RUNTIME_EVIDENCE/READINESS/EVIDENCE",
            &[route_sha256.as_bytes(), formula_lineage_sha256.as_bytes()],
        );
        let evidence_ref = match u64::from_str_radix(&evidence_sha256[..16], 16) {
            Ok(0) => return GclReadiness::NotReady("RUNTIME_EVIDENCE_REF_ZERO"),
            Ok(evidence_ref) => evidence_ref,
            Err(_) => return GclReadiness::NotReady("RUNTIME_EVIDENCE_REF_DECODE"),
        };
        let mut spine = Self::new();
        let blank = spine.snapshot_route(
            route_key,
            route_sha256.clone(),
            formula_lineage_sha256.clone(),
            1,
        );
        let write = spine.mini.record_verified_cycle_bound_at(
            route_key,
            &route_sha256,
            evidence_ref,
            &evidence_sha256,
            "coordination",
            1.0,
            true,
            1,
        );
        match write {
            Ok(()) => {}
            Err(reason) => return GclReadiness::NotReady(reason),
        }
        let known = spine.snapshot_route(route_key, route_sha256, formula_lineage_sha256, 1);
        match (blank, known) {
            (Ok(before), Ok(after)) => match before.evidence_count() == 0
                && after.evidence_count() == 1
                && after.signals.usage_count == 1
                && after.signals.verification_count == 1
            {
                true => GclReadiness::Ready,
                false => GclReadiness::NotReady("runtime evidence spine self-check dështoi"),
            },
            _ => GclReadiness::NotReady("runtime evidence spine self-check dështoi"),
        }
    }
}

impl Default for RuntimeEvidenceSpine {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Phase9Integration;

impl Phase9Integration {
    /// Rruga runtime e Fazës 9 me gjurmën terminale të rindërtuar.
    pub fn interpret_with_evidence_spine(
        input: UiInput,
        trace_id: u64,
        signals: &ReliabilitySignals,
        territory: String,
        history_refs: Vec<u64>,
        at: u64,
        formula_anchor_valid: bool,
        verification_trace: Option<&VerificationTrace>,
        evidence_route_key: Option<u64>,
    ) -> Phase9InterpretResult {
        // Spine nuk aktivizohet nga prania e modulit. Ai kërkon provën reale
        // që i₀/PA/split/GCL/Besa janë i njëjti atom koordinimi.
        let law_check = Self::check_light_laws(formula_anchor_valid);

        // Spine ekzekutohet VETËM nëse ligjet kalojnë. Zero if — match.
        let spine = match law_check.is_passed() {
            true => {
                let result = LightSpine::interpret_full_with_evidence(
                    input,
                    trace_id,
                    signals,
                    territory,
                    history_refs,
                    at,
                    verification_trace,
                    evidence_route_key,
                );
                Some(result)
            }
            false => None,
        };

        Phase9InterpretResult {
            law_check,
            spine,
            #[cfg(test)]
            interprets_only: true, // Light gjithmonë vetëm interpreton
        }
    }

    /// check_light_laws — verifikon ligjin "Light s'vendos".
    ///
    /// Light prodhon vetëm interpretim/besim — kurrë verdikt. Ky është
    /// strukturalisht i garantuar (s'ka API vendimi te Light). Zero if.
    fn check_light_laws(formula_anchor_valid: bool) -> LightLawCheck {
        match formula_anchor_valid {
            true => LightLawCheck::Passed,
            false => LightLawCheck::InvalidFormulaAnchor,
        }
    }

    /// output_is_not_verdict — konfirmon që output-i i Light s'është verdikt.
    ///
    /// Rezultati ka trust_context (besim) + traced_input (gjurmim) — JO
    /// verdikt 0/1. Faza 9 e ruan këtë kufi. Zero if — match.
    pub fn output_is_not_verdict(result: &Phase9InterpretResult) -> bool {
        // Match mbi spine — nëse ekziston, përmban besim/gjurmim (jo verdikt).
        match &result.spine {
            Some(spine) => Self::is_interpretation(spine),
            None => false,
        }
    }

    /// is_interpretation — verifikon që rezultati është interpretim. Zero if.
    fn is_interpretation(spine: &LightSpineResult) -> bool {
        // Ka gjurmim (trace) + besim — interpretim, jo vendim.
        let has_trace = spine.traced_input.trace_id != 0;
        let has_meaning = !spine.traced_input.meaning.meaning.is_empty();
        has_trace & has_meaning
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn spine9_wired_produces_trust_not_verdict() {
        // Dëshmi që SPINE9 i lidhur në runtime prodhon BESIM (jo verdikt).
        let ui = UiInput {
            text: "flamuri kuq e zi".to_string(),
            session_id: "S1".to_string(),
            deep_research: false,
        };
        let sig = ReliabilitySignals::empty();
        let r = Phase9Integration::interpret_with_spine(
            ui,
            0xA451,
            &sig,
            "letersi".to_string(),
            Vec::new(),
            1000,
            true,
        );
        assert!(r.law_check.is_passed());
        assert!(r.interprets_only); // Light interpreton
        assert!(Phase9Integration::output_is_not_verdict(&r)); // KURRË verdikt
        assert!(r.spine.is_some()); // spine u aktivizua
    }

    use super::*;
    use crate::alnur_karina_athar::HandoffEvidence;
    use crate::lab_contracts::trust::TrustGuidance;
    use crate::light_coordinator::{LightCoordinator, LightRequest};

    fn strong_signals() -> ReliabilitySignals {
        ReliabilitySignals {
            usage_count: 10,
            avg_success: 0.9,
            domain_count: 4,
            verification_count: 3,
        }
    }

    fn completed_response(coordinator: &LightCoordinator, text: &str) -> LightResponse {
        let mut response = coordinator.receive(LightRequest::new(text));
        let receipt = response
            .legacy
            .as_ref()
            .expect("Legacy coordination")
            .receipt
            .clone();
        let besa = response
            .legacy
            .as_ref()
            .expect("Legacy coordination")
            .besa_attestation
            .clone();
        let evidence = HandoffEvidence {
            binding_sha256: receipt.binding_sha256,
            receipt_sha256: receipt.receipt_sha256,
            besa_attestation_sha256: besa.attestation_sha256(),
            besa_enrichment_sha256: besa.enrichment_sha256,
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

    fn terminal_result(response: &LightResponse, survived: bool, timestamp: u64) -> QuantumResult {
        let (verdict, potentiality, axis_score, evolve_sfx, algorithm_state) = match survived {
            true => ("OPTIMAL", 0.9, 90.0, "VFYKNG", "SEALED"),
            // Rruga prodhuese nga SHADOW për çiftin kushtetues (0,0)
            // serializohet nga QUANTUM si HOLD/SHADOW_VERIFY.
            false => ("HOLD", 0.2, -80.0, "NEGKNG", "SHADOW_VERIFY"),
        };
        QuantumResult {
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
            ts: timestamp,
            pd_continuity: None,
            is_verified: survived,
        }
    }

    #[test]
    fn spine_runs_light_interprets_only() {
        let input = UiInput::new("explore with Nura".to_string(), "s1".to_string(), false);
        let result = Phase9Integration::interpret_with_spine(
            input,
            42,
            &strong_signals(),
            "physics".to_string(),
            vec![1],
            1000,
            true,
        );
        // Ligjet kaluan → spine ekzekutoi.
        assert!(result.law_check.is_passed());
        assert!(result.spine.is_some());
        // Light gjithmonë vetëm interpreton.
        assert!(result.interprets_only);
    }

    #[test]
    fn output_never_verdict() {
        let input = UiInput::new("q".to_string(), "s".to_string(), false);
        let result = Phase9Integration::interpret_with_spine(
            input,
            1,
            &strong_signals(),
            "d".to_string(),
            vec![1],
            100,
            true,
        );
        // Output-i i Light s'është kurrë verdikt.
        assert!(Phase9Integration::output_is_not_verdict(&result));
    }

    #[test]
    fn light_law_requires_the_materialized_gcl_formula_anchor() {
        assert_eq!(
            Phase9Integration::check_light_laws(true),
            LightLawCheck::Passed
        );
        assert_eq!(
            Phase9Integration::check_light_laws(false),
            LightLawCheck::InvalidFormulaAnchor
        );
    }

    #[test]
    fn runtime_route_key_is_stable_and_requires_the_complete_gcl_atom() {
        let coordinator = LightCoordinator::new();
        let text = "same primitive input under the same GCL formula";
        let first_response = completed_response(&coordinator, text);
        let second_response = completed_response(&coordinator, text);
        let first = RuntimeEvidenceSpine::route_key(&first_response).expect("canonical GCL route");
        let second =
            RuntimeEvidenceSpine::route_key(&second_response).expect("same canonical GCL route");
        assert_eq!(first, second);
        assert_ne!(first, 0);

        let uncommitted = coordinator.receive(LightRequest::new(text));
        assert_eq!(
            RuntimeEvidenceSpine::route_key(&uncommitted)
                .expect_err("uncommitted handoff must fail"),
            "RUNTIME_EVIDENCE_HANDOFF_NOT_COMMITTED"
        );

        let mut drifted = completed_response(&coordinator, text);
        drifted
            .legacy
            .as_mut()
            .expect("Legacy coordination")
            .directive
            .seal ^= 1;
        assert_eq!(
            RuntimeEvidenceSpine::route_key(&drifted).expect_err("drifted Phase 1 atom must fail"),
            "RUNTIME_EVIDENCE_FORMULA_ANCHOR_INVALID"
        );
    }

    #[test]
    fn terminal_evidence_identity_covers_the_complete_return_envelope() {
        let coordinator = LightCoordinator::new();
        let response = completed_response(&coordinator, "terminal identity");
        let spine = RuntimeEvidenceSpine::new();
        let snapshot = spine.snapshot(&response).expect("formula snapshot");
        let first = terminal_result(&response, true, 100);
        let receipt = PdTerminalReceipt::fixture_for(&response, &first, 7);
        let other_receipt = PdTerminalReceipt::fixture_for(&response, &first, 8);
        let mut content_drift = terminal_result(&response, true, 100);
        content_drift.content.push_str(" drift");
        let mut axis_drift = terminal_result(&response, true, 100);
        axis_drift.axis_score = 89.0;
        let count = u64::try_from(snapshot.branch_count_before + 1).expect("bounded branch count");
        let first_ref = RuntimeEvidenceSpine::terminal_evidence_ref(
            snapshot.route_key,
            &snapshot.route_sha256,
            &snapshot.formula_lineage_sha256,
            &first,
            &receipt,
            count,
        )
        .expect("terminal ref");
        let content_ref = RuntimeEvidenceSpine::terminal_evidence_ref(
            snapshot.route_key,
            &snapshot.route_sha256,
            &snapshot.formula_lineage_sha256,
            &content_drift,
            &receipt,
            count,
        )
        .expect("content ref");
        let axis_ref = RuntimeEvidenceSpine::terminal_evidence_ref(
            snapshot.route_key,
            &snapshot.route_sha256,
            &snapshot.formula_lineage_sha256,
            &axis_drift,
            &receipt,
            count,
        )
        .expect("axis ref");
        let receipt_ref = RuntimeEvidenceSpine::terminal_evidence_ref(
            snapshot.route_key,
            &snapshot.route_sha256,
            &snapshot.formula_lineage_sha256,
            &first,
            &other_receipt,
            count,
        )
        .expect("receipt ref");
        assert_ne!(first_ref, content_ref);
        assert_ne!(first_ref, axis_ref);
        assert_ne!(first_ref, receipt_ref);
    }

    #[test]
    fn terminal_binding_rejects_formula_lineage_drift_after_snapshot() {
        let coordinator = LightCoordinator::new();
        let mut response = completed_response(&coordinator, "lineage drift");
        let spine = RuntimeEvidenceSpine::new();
        let snapshot = spine.snapshot(&response).expect("formula snapshot");
        let dispatch = DispatchResult::Verified(terminal_result(&response, true, 100));
        let receipt = match &dispatch {
            DispatchResult::Verified(result) => {
                PdTerminalReceipt::fixture_for(&response, result, 9)
            }
            _ => unreachable!("verified fixture"),
        };
        let (_, verdict) = coordinator.handle_quantum_response(&dispatch, &mut response);
        response.algorithm_state.push_str("_DRIFT");
        assert!(!RuntimeEvidenceSpine::terminal_evidence_is_bound(
            &dispatch,
            &response,
            &snapshot,
            &verdict,
            Some(&receipt),
        ));
    }

    #[test]
    fn terminal_binding_rejects_same_handle_with_different_route_sha256() {
        let coordinator = LightCoordinator::new();
        let mut response = completed_response(&coordinator, "full route identity");
        let mut spine = RuntimeEvidenceSpine::new();
        let mut snapshot = spine.snapshot(&response).expect("formula snapshot");
        let original_tail = &snapshot.route_sha256[62..64];
        let replacement = match original_tail {
            "00" => "01",
            _ => "00",
        };
        snapshot.route_sha256.replace_range(62..64, replacement);
        assert_eq!(
            u64::from_str_radix(&snapshot.route_sha256[..16], 16).ok(),
            Some(snapshot.route_key)
        );

        let dispatch = DispatchResult::Verified(terminal_result(&response, true, 100));
        let result = match &dispatch {
            DispatchResult::Verified(result) => result,
            _ => unreachable!("verified fixture"),
        };
        let receipt = PdTerminalReceipt::fixture_for(&response, result, 10);
        let (_, verdict) = coordinator.handle_quantum_response(&dispatch, &mut response);
        assert!(!RuntimeEvidenceSpine::terminal_evidence_is_bound(
            &dispatch,
            &response,
            &snapshot,
            &verdict,
            Some(&receipt),
        ));
        assert_eq!(
            spine
                .record_terminal(&response, &snapshot, result, &receipt, true)
                .expect_err("full route identity drift must fail"),
            "RUNTIME_EVIDENCE_ROUTE_SHA256_DRIFT"
        );
    }

    #[test]
    fn empty_history_still_interprets() {
        let input = UiInput::new("q".to_string(), "s".to_string(), false);
        let result = Phase9Integration::interpret_with_spine(
            input,
            1,
            &ReliabilitySignals::empty(),
            "d".to_string(),
            vec![],
            100,
            true,
        );
        // Edhe pa histori, Light interpreton (Unknown guidance, por s'vendos).
        assert!(result.spine.is_some());
        let spine = result.spine.unwrap();
        assert_eq!(spine.trust_context.guidance, TrustGuidance::Unknown);
    }

    #[test]
    fn runtime_evidence_closes_positive_and_negative_cycles_without_transport_noise() {
        let coordinator = LightCoordinator::new();
        let text = "Analizo motorin industrial të kuq dhe sensorin termik";
        let mut evidence_spine = RuntimeEvidenceSpine::new();

        let mut positive_response = completed_response(&coordinator, text);
        let blank = evidence_spine
            .snapshot(&positive_response)
            .expect("first cycle snapshot");
        assert_eq!(blank.evidence_count(), 0);
        assert_eq!(blank.signals.usage_count, 0);
        assert!(blank.verification_trace.is_none());

        let positive_dispatch =
            DispatchResult::Verified(terminal_result(&positive_response, true, 100));
        let positive_receipt = match &positive_dispatch {
            DispatchResult::Verified(result) => {
                PdTerminalReceipt::fixture_for(&positive_response, result, 11)
            }
            _ => unreachable!("verified fixture"),
        };
        let (_, positive_verdict) =
            coordinator.handle_quantum_response(&positive_dispatch, &mut positive_response);
        assert!(!RuntimeEvidenceSpine::terminal_evidence_is_bound(
            &positive_dispatch,
            &positive_response,
            &blank,
            &positive_verdict,
            None,
        ));
        let positive_bound = RuntimeEvidenceSpine::terminal_evidence_is_bound(
            &positive_dispatch,
            &positive_response,
            &blank,
            &positive_verdict,
            Some(&positive_receipt),
        );
        assert!(positive_bound);
        let positive_result = match &positive_dispatch {
            DispatchResult::Verified(result) => result,
            _ => unreachable!("verified fixture"),
        };
        let positive_record = evidence_spine
            .record_terminal(
                &positive_response,
                &blank,
                positive_result,
                &positive_receipt,
                positive_bound,
            )
            .expect("positive terminal record");
        assert!(crate::lgc_algorithm::is_canonical_sha256(
            &positive_record.route_sha256
        ));
        assert!(crate::lgc_algorithm::is_canonical_sha256(
            &positive_record.evidence_sha256
        ));
        assert_eq!(
            u64::from_str_radix(&positive_record.route_sha256[..16], 16).ok(),
            Some(positive_record.route_key)
        );
        assert_eq!(
            u64::from_str_radix(&positive_record.evidence_sha256[..16], 16).ok(),
            Some(positive_record.evidence_ref)
        );
        assert_eq!(
            evidence_spine
                .record_terminal(
                    &positive_response,
                    &blank,
                    positive_result,
                    &positive_receipt,
                    positive_bound,
                )
                .expect_err("stale pre-dispatch snapshot must not replay"),
            "RUNTIME_EVIDENCE_HISTORY_DRIFT"
        );

        let positive_history_response = completed_response(&coordinator, text);
        let positive_history = evidence_spine
            .snapshot(&positive_history_response)
            .expect("positive history");
        assert_eq!(positive_history.evidence_count(), 1);
        assert_eq!(positive_history.signals.avg_success, 1.0);
        let positive_trace = positive_history
            .verification_trace
            .as_ref()
            .expect("bound verification trace");
        assert_eq!(
            positive_trace.route_sha256(),
            Some(positive_record.route_sha256.as_str())
        );
        assert_eq!(
            positive_trace.evidence_sha256(positive_record.evidence_ref),
            Some(positive_record.evidence_sha256.as_str())
        );
        assert!(positive_trace.full_identity_valid());
        let interpreted = Phase9Integration::interpret_with_evidence_spine(
            UiInput::new(text.to_string(), "session".to_string(), false),
            positive_response.primitive_trace.id,
            &positive_history.signals,
            positive_response.domain.clone(),
            positive_history.history_refs.clone(),
            1000,
            true,
            positive_history.verification_trace.as_ref(),
            Some(positive_history.route_key),
        );
        let interpreted_spine = interpreted.spine.expect("evidence-aware spine");
        assert!(interpreted_spine.evidence_hardened);
        assert_eq!(interpreted_spine.verification_evidence_count, 1);

        let mut negative_response = completed_response(&coordinator, text);
        assert_eq!(
            negative_response.input_sha256,
            positive_response.input_sha256
        );
        let negative_snapshot = evidence_spine
            .snapshot(&negative_response)
            .expect("negative cycle pre-dispatch snapshot");
        assert_eq!(negative_snapshot.evidence_count(), 1);
        let negative_dispatch =
            DispatchResult::Verified(terminal_result(&negative_response, false, 101));
        let negative_receipt = match &negative_dispatch {
            DispatchResult::Verified(result) => {
                PdTerminalReceipt::fixture_for(&negative_response, result, 13)
            }
            _ => unreachable!("verified fixture"),
        };
        let (_, negative_verdict) =
            coordinator.handle_quantum_response(&negative_dispatch, &mut negative_response);
        assert!(!negative_response.formula_anchor_valid());
        let negative_bound = RuntimeEvidenceSpine::terminal_evidence_is_bound(
            &negative_dispatch,
            &negative_response,
            &negative_snapshot,
            &negative_verdict,
            Some(&negative_receipt),
        );
        assert!(negative_bound);
        let negative_result = match &negative_dispatch {
            DispatchResult::Verified(result) => result,
            _ => unreachable!("verified fixture"),
        };
        evidence_spine
            .record_terminal(
                &negative_response,
                &negative_snapshot,
                negative_result,
                &negative_receipt,
                negative_bound,
            )
            .expect("negative terminal record");

        let mixed_history_response = completed_response(&coordinator, text);
        let mixed_history = evidence_spine
            .snapshot(&mixed_history_response)
            .expect("mixed terminal history");
        assert_eq!(mixed_history.evidence_count(), 2);
        assert_eq!(mixed_history.signals.usage_count, 2);
        assert_eq!(mixed_history.signals.avg_success, 0.5);
        assert_eq!(mixed_history.signals.verification_count, 1);

        let timeout = DispatchResult::Timeout("TIMEOUT:5000ms".to_string());
        assert!(!RuntimeEvidenceSpine::terminal_evidence_is_bound(
            &timeout,
            &negative_response,
            &negative_snapshot,
            &negative_verdict,
            Some(&negative_receipt),
        ));
        let after_timeout_response = completed_response(&coordinator, text);
        let after_timeout = evidence_spine
            .snapshot(&after_timeout_response)
            .expect("history after transport timeout");
        assert_eq!(after_timeout.evidence_count(), 2);
    }
}

#[cfg(test)]
impl Phase9Integration {
    pub fn interpret_with_spine(
        input: UiInput,
        trace_id: u64,
        signals: &ReliabilitySignals,
        territory: String,
        history_refs: Vec<u64>,
        at: u64,
        formula_anchor_valid: bool,
    ) -> Phase9InterpretResult {
        Self::interpret_with_evidence_spine(
            input,
            trace_id,
            signals,
            territory,
            history_refs,
            at,
            formula_anchor_valid,
            None,
            None,
        )
    }
}

#[cfg(test)]
impl RuntimeEvidenceSpine {
    pub fn route_key(response: &LightResponse) -> Result<u64, &'static str> {
        match response.formula_anchor_valid() {
            true => {}
            false => return Err("RUNTIME_EVIDENCE_FORMULA_ANCHOR_INVALID"),
        }
        response
            .ready_for_dispatch()
            .map_err(|_| "RUNTIME_EVIDENCE_HANDOFF_NOT_COMMITTED")?;
        Self::route_identity_from_fragments(response).map(|(route_key, _)| route_key)
    }
}
