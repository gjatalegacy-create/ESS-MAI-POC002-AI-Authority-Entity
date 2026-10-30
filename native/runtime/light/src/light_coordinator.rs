// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  LIGHT_COORDINATOR.RS — AIGovern-Cyber LIGHT PLATFORM                        ║
// ║  Gjata Legacy™  |  Arkitekt: Bledar Gjata                             ║
// ║                                                                          ║
// ║  Porta e hyrjes dhe daljes. Koordinues i pastër.                      ║
// ║  KURRË nuk vendos. Shadow vendos.                                     ║
// ║                                                                          ║
// ║  FLUKSI:                                                                ║
// ║    receive(input)                                                        ║
// ║      → lingua::parse_input()              [gjuhë → SystemQuery]       ║
// ║      → trace_info::TraceInfo::try_new()   [i₀ + instance fallible]    ║
// ║      → nura_core::Domain::detect()         [domain routing]           ║
// ║      → software_contract::create()         [kontratë + seal]         ║
// ║      → evolve_trace::EvolveTrace::branch() [dega INPKNG]             ║
// ║      → LightResponse (gati për Quantum)                               ║
// ║    format_output(content, lang)                                         ║
// ║      → lingua::format_output()            [akademik → user]           ║
// ║                                                                          ║
// ║  Roli: COORDINATOR — orchestron, rout-on. Zero logjikë vendimi.      ║
// ╚══════════════════════════════════════════════════════════════════════════╝

use crate::alnur_karina_athar::HandoffEvidence;
use crate::ess_mai_system::dsl;
use crate::lab_contracts::{CollapsePhase, GjataCollapseLaw, PrimitiveSplit};
use crate::lgc_algorithm::{compute_sha256, LegacyCoordination, LegacyLgcAlgorithm};
use crate::lingua::{DetectedLang, LinguaCore};
use crate::nura_core::{Domain, NuraCore};
use crate::software_contract::{Contract, SoftwareContract};
use crate::trace_info::{DomainHint, TraceIdentityError, TraceInfo};
use std::fmt;
use std::io;

// ════════════════════════════════════════════════════════════════════════════
// §1  REQUEST — input nga user ose sistem
// ════════════════════════════════════════════════════════════════════════════

/// Ligji byte-sh i hyrjes LIGHT. Ura ekzistuese LIGHT→QUANTUM ka envelope
/// 2,048-byte dhe escaping-u mund të dyfishojë çdo byte inputi. Buxheti
/// konservativ 512-byte lë hapësirë të provuar për trace/GCL/contract/evolve
/// metadata dhe zbatohet përpara kopjes `String → Vec<u8>`.
pub const MAX_LIGHT_INPUT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LightInputError {
    Empty,
    TooLarge {
        limit: usize,
        observed_at_least: usize,
    },
    InvalidUtf8 {
        valid_up_to: usize,
    },
    IncompleteFrame {
        observed_bytes: usize,
    },
    EmbeddedLineBreak {
        byte_offset: usize,
    },
    Io(io::ErrorKind),
}

impl fmt::Display for LightInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "LIGHT_INPUT:EMPTY"),
            Self::TooLarge {
                limit,
                observed_at_least,
            } => write!(
                f,
                "LIGHT_INPUT:TOO_LARGE limit={limit} observed_at_least={observed_at_least}"
            ),
            Self::InvalidUtf8 { valid_up_to } => {
                write!(f, "LIGHT_INPUT:INVALID_UTF8 valid_up_to={valid_up_to}")
            }
            Self::IncompleteFrame { observed_bytes } => {
                write!(
                    f,
                    "LIGHT_INPUT:INCOMPLETE_FRAME observed_bytes={observed_bytes}"
                )
            }
            Self::EmbeddedLineBreak { byte_offset } => {
                write!(
                    f,
                    "LIGHT_INPUT:EMBEDDED_LINE_BREAK byte_offset={byte_offset}"
                )
            }
            Self::Io(kind) => write!(f, "LIGHT_INPUT:IO kind={kind:?}"),
        }
    }
}

#[derive(Debug)]
pub struct LightRequest {
    /// Teksti i papërpunuar
    text: String,

    /// Bytes raw — për trace_info::new()
    raw: Vec<u8>,
}

impl LightRequest {
    /// Konstrukton kërkesën nga një `String` tashmë i kufizuar nga lexuesi.
    /// Madhësia dhe boshllëku gjykohen përpara kopjes së vetme në `raw`.
    pub fn try_from_string(mut text: String) -> Result<Self, LightInputError> {
        let observed = text.len();
        match observed > MAX_LIGHT_INPUT_BYTES {
            true => {
                return Err(LightInputError::TooLarge {
                    limit: MAX_LIGHT_INPUT_BYTES,
                    observed_at_least: observed,
                })
            }
            false => {}
        }

        match text
            .as_bytes()
            .iter()
            .position(|byte| matches!(*byte, b'\n' | b'\r'))
        {
            Some(byte_offset) => return Err(LightInputError::EmbeddedLineBreak { byte_offset }),
            None => {}
        }

        let trimmed_len = text.trim().len();
        match trimmed_len {
            0 => return Err(LightInputError::Empty),
            _ => {}
        }
        let trim_start = text.len().saturating_sub(text.trim_start().len());
        let trim_end = trim_start.saturating_add(trimmed_len);
        text.truncate(trim_end);
        match trim_start {
            0 => {}
            count => {
                text.drain(..count);
            }
        }
        let raw = text.as_bytes().to_vec();

        Ok(Self { raw, text })
    }

    /// Konstruktor i sigurt për caller-at që kanë vetëm `&str`: asnjë
    /// allokim nuk ndodh për input mbi kufirin.
    pub fn try_new(text: &str) -> Result<Self, LightInputError> {
        match text.len() > MAX_LIGHT_INPUT_BYTES {
            true => Err(LightInputError::TooLarge {
                limit: MAX_LIGHT_INPUT_BYTES,
                observed_at_least: text.len(),
            }),
            false => Self::try_from_string(text.to_owned()),
        }
    }

    /// Convenience vetëm për testet historike. Runtime-i përdor API-në
    /// fallible dhe nuk mund të kapërcejë ligjin byte-sh.
    #[cfg(test)]
    pub fn new(text: &str) -> Self {
        Self::try_new(text).expect("bounded LIGHT test request")
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

// ════════════════════════════════════════════════════════════════════════════
// §2  STATUS — çfarë ka ndodhur
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, PartialEq)]
pub enum LightStatus {
    /// Input traçed, contract created, gati për Quantum
    Routed,
    /// Kontrata dështoi — gabim autorizimi ose modul i panjohur
    ContractFailed(String),
    /// GCL/LGC/PA/LAW0/receipt dështoi para handoff-it.
    LegacyFailed(String),
}

impl LightStatus {
    pub fn label(&self) -> &str {
        match self {
            Self::Routed => "ROUTED",
            Self::ContractFailed(_) => "CONTRACT_FAILED",
            Self::LegacyFailed(_) => "LEGACY_LGC_FAILED",
        }
    }

    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Routed)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// §3  RESPONSE — rezultati i koordinimit
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug)]
pub struct LightResponse {
    /// Session-id autoritativ: sequence + i₀ i plotë + instance LIGHT.
    pub trace_id: String,

    /// Primitive Trace autoritative e krijuar NJË herë në hyrje. Ky objekt
    /// bartet deri te eksporti PA; main nuk lejohet të rikrijojë TraceInfo.
    pub primitive_trace: TraceInfo,

    /// WORK→NURA / HOME→ALISAH / OUTSIDE→AMMY
    pub domain: String,

    /// KODUNIK seal i kontratës
    pub contract_id: String,

    /// Kontrata e plotë, e vulosur dhe e riverifikueshme. Fushat publike
    /// `contract_id`/`lgc_seal` ruhen për wire compatibility, por handoff-i
    /// hapet vetëm kur përputhen me këtë objekt dhe me receipt-in GCL.
    contract: Option<Contract>,

    /// LGC lightweight seal — dërguar te Quantum për verifikim
    pub lgc_seal: String,

    /// Gjendja DSL aktuale
    pub algorithm_state: String,

    /// Rezyme evolve_trace e lidhur me session-id autoritativ.
    pub evolve_summary: String,

    /// Gjuha e detektuar — për format_output pas Shadow
    pub user_lang: DetectedLang,

    /// Confidence i detektimit gjuhësor [0,1]
    pub lang_confidence: f32,

    /// KOLAPS KOORDINIMI — (Xi, Yi): dy hapësirat primitive të lindura nga PA.
    /// Formula e gdhendur: i0 → PA → (Xi, Yi). Light koordinon, s'arsyeton.
    pub primitive_split: PrimitiveSplit,

    /// SHA-256 REAL i inputit — lind në Light (evolve_trace), Shadow vetëm vulos.
    pub input_sha256: String,

    /// Autoriteti unik LGC: receipt + EvolveTrace + muskuli evidence-bound.
    pub legacy: Option<LegacyCoordination>,

    /// Statusi
    pub status: LightStatus,
}

impl LightResponse {
    /// Invarianti kushtetues i Primitive Trace: trace_id që udhëton drejt
    /// Quantum-it duhet të jetë etiketa e të njëjtit objekt që zbret si PA/i₀
    /// në Shadow. Një TraceInfo i rindërtuar do ta bëjë këtë `false`.
    pub fn primitive_continuity_valid(&self) -> bool {
        let trace_ok = self.trace_id == self.primitive_trace.label();
        match &self.legacy {
            Some(legacy) => {
                trace_ok
                    && legacy.primitive_fragment_valid().is_ok()
                    && legacy.receipt.trace_id == self.trace_id
                    && legacy.receipt.trace_origin == self.primitive_trace.id
                    && legacy.receipt.primitive_anchor == self.primitive_trace.id_hex()
                    && legacy.receipt.input_sha256 == self.input_sha256
                    && legacy.primitive_split == self.primitive_split
            }
            None => false,
        }
    }

    /// Një kontratë Light→Quantum është pjesë e së njëjtës formulë vetëm kur
    /// mund të riverifikohet dhe kur autoriteti/receipt-i i saj janë pikërisht
    /// ata që prodhoi LegacyLgcAlgorithm nën GCL.
    pub fn authority_continuity_valid(&self) -> bool {
        match (&self.legacy, &self.contract) {
            (Some(legacy), Some(contract)) => {
                SoftwareContract::enforce(contract).is_ok()
                    && contract.id == self.contract_id
                    && contract.lgc_seal == self.lgc_seal
                    && contract.caller == "light_coordinator"
                    && contract.target == "quantum_platform"
                    && contract.action == "route_input"
                    && contract.platform == "LIGHT"
                    && contract.authority_sha256 == legacy.receipt.binding_sha256
                    && contract.legacy_receipt_sha256.as_deref()
                        == Some(legacy.receipt.receipt_sha256.as_str())
            }
            _ => false,
        }
    }

    /// Besa nuk shton autoritet dhe nuk arsyeton. Ajo është fragmenti GCL që
    /// provon se kërkesa statistikore, hold-i dhe adresat −Xi/−Yi burojnë nga
    /// i njëjti split dhe nga të njëjtat vula të koordinimit.
    pub fn besa_continuity_valid(&self) -> bool {
        match &self.legacy {
            Some(legacy) => {
                let besa = &legacy.besa_attestation;
                legacy.besa_fragment_valid().is_ok()
                    && besa.trace_id == self.trace_id
                    && besa.split_sha256 == legacy.receipt.split_sha256
            }
            None => false,
        }
    }

    /// Ankora e vetme logjike e formulës në Light:
    /// i₀ + PA/(Xi,Yi) + −Xi/−Yi + GCL receipt + kontrata e autorizuar.
    pub fn formula_anchor_valid(&self) -> bool {
        let coordination_valid = match &self.legacy {
            Some(legacy) => legacy.coordination_fragment_valid().is_ok(),
            None => false,
        };
        coordination_valid
            && self.primitive_continuity_valid()
            && self.authority_continuity_valid()
            && self.besa_continuity_valid()
    }

    pub fn ready_for_handoff(&self) -> Result<(), String> {
        match self.status.is_ok() {
            true => {}
            false => return Err(format!("Light status: {}", self.status.label())),
        }
        match (
            self.primitive_continuity_valid(),
            self.authority_continuity_valid(),
        ) {
            (true, true) => {}
            (false, _) => return Err(String::from("primitive continuity invalid")),
            (_, false) => return Err(String::from("GCL authority continuity invalid")),
        }
        match self.formula_anchor_valid() {
            true => {}
            false => return Err(String::from("GCL formula anchor invalid")),
        }
        self.legacy
            .as_ref()
            .ok_or_else(|| "legacy coordination missing".to_string())?
            .ready_for_handoff()
            .map_err(|e| e.to_string())
    }

    pub fn complete_legacy_handoff(&mut self, evidence: &HandoffEvidence) -> Result<(), String> {
        match self.formula_anchor_valid() {
            true => {}
            false => return Err(String::from("GCL formula anchor invalid before commit")),
        }
        self.legacy
            .as_mut()
            .ok_or_else(|| "legacy coordination missing".to_string())?
            .complete_handoff(evidence)
            .map_err(|e| e.to_string())
    }

    pub fn ready_for_dispatch(&self) -> Result<(), String> {
        match self.status.is_ok() {
            true => {}
            false => return Err(format!("Light status: {}", self.status.label())),
        }
        match self.formula_anchor_valid() {
            true => {}
            false => return Err(String::from("GCL formula anchor invalid")),
        }
        self.legacy
            .as_ref()
            .ok_or_else(|| "legacy coordination missing".to_string())?
            .ready_for_dispatch()
            .map_err(|e| e.to_string())
    }
}

// ════════════════════════════════════════════════════════════════════════════
// §4  LIGHT COORDINATOR
// ════════════════════════════════════════════════════════════════════════════

pub struct LightCoordinator {
    /// NURA unified entity (NURA+ALISAH+AMMY)
    pub core: NuraCore,
    /// Lingua — adaptive language engine
    pub lingua: LinguaCore,
}

impl LightCoordinator {
    pub fn new() -> Self {
        Self {
            core: NuraCore::new(),
            lingua: LinguaCore::new(),
        }
    }

    pub fn try_receive_with_iz(
        &self,
        req: &LightRequest,
        iz_context: Option<&mut shadow_contracts::pd_memory::iz::IzContext>,
    ) -> Result<LightResponse, TraceIdentityError> {
        // HAPI 0 — gjuha natyrore hyn vetëm një herë në Lingua.
        let query = self.lingua.parse_input(&req.text);
        let user_lang = query.lang.clone();
        let lang_conf = query.confidence;
        let domain_hint0 = query.domain_hint();

        // HAPI 1 — i₀ stampohet vetëm një herë; domain-i ndryshon vetëm hint-in,
        // jo identitetin, timestamp-in ose sequence-in e Primitive Trace.
        let raw_trace = TraceInfo::try_new(&req.raw)?;
        let domain = self.core.route(&query.normalized);
        let domain_hint = match &domain {
            Domain::Work => DomainHint::Work,
            Domain::Home => DomainHint::Home,
            Domain::Outside => DomainHint::Outside,
        };
        let final_hint = match domain_hint == DomainHint::Unknown {
            true => domain_hint0,
            false => domain_hint,
        };
        let trace = raw_trace.with_domain(final_hint);
        let trace_id = trace.label();
        let domain_label = domain.label().to_string();

        // HAPI 2 — GCL është autoriteti; LGC është algoritmi Legacy që zbaton
        // koordinimin e autorizuar. Këtu ndodh thirrja e vetme production për
        // PA → Xi/Yi, LegalGoCrypt V2, KODUNIK dhe receipt-in autoritativ.
        let directive = GjataCollapseLaw::issue(CollapsePhase::Coordination);
        let legacy = match LegacyLgcAlgorithm::coordinate_under_gcl_with_iz(
            directive, &req.raw, &trace, &query, iz_context,
        ) {
            Ok(value) => value,
            Err(error) => {
                return Ok(LightResponse {
                    trace_id,
                    primitive_trace: trace,
                    domain: domain_label,
                    contract_id: String::from("NO_CONTRACT"),
                    contract: None,
                    lgc_seal: String::new(),
                    algorithm_state: dsl::AlgorithmState::Failed(error.to_string())
                        .label()
                        .to_string(),
                    evolve_summary: String::from("LEGACY_LGC_NOT_ROUTED"),
                    user_lang,
                    lang_confidence: lang_conf,
                    primitive_split: PrimitiveSplit {
                        xi: Vec::new(),
                        yi: Vec::new(),
                    },
                    input_sha256: compute_sha256(&req.raw),
                    legacy: None,
                    status: LightStatus::LegacyFailed(error.to_string()),
                });
            }
        };

        let primitive_split = legacy.primitive_split.clone();
        let input_sha256 = legacy.receipt.input_sha256.clone();
        let evolve_summary = legacy.evolve.summary();
        let algorithm_state = legacy.evolve.state.label().to_string();

        // HAPI 3 — kontrata nuk gjeneron KODUNIK të dytë. Ajo lidhet me të
        // njëjtin KODUNIK dhe receipt që prodhoi algoritmi Legacy nën GCL.
        let contract = match SoftwareContract::create_bound(
            "light_coordinator",
            "quantum_platform",
            "route_input",
            "LIGHT",
            &legacy.receipt,
        ) {
            Ok(value) => value,
            Err(error) => {
                return Ok(LightResponse {
                    trace_id,
                    primitive_trace: trace,
                    domain: domain_label,
                    contract_id: String::from("NO_CONTRACT"),
                    contract: None,
                    lgc_seal: String::new(),
                    algorithm_state: dsl::AlgorithmState::Failed(error.to_string())
                        .label()
                        .to_string(),
                    evolve_summary,
                    user_lang,
                    lang_confidence: lang_conf,
                    primitive_split,
                    input_sha256,
                    legacy: Some(legacy),
                    status: LightStatus::ContractFailed(error.to_string()),
                });
            }
        };

        Ok(match SoftwareContract::enforce(&contract) {
            Ok(()) => {
                // Dy String-et janë pamja wire-compatible; objekti `contract`
                // mbetet prova autoritative që i riverifikon para çdo daljeje.
                let contract_id = contract.id.clone();
                let lgc_seal = contract.lgc_seal.clone();
                LightResponse {
                    trace_id,
                    primitive_trace: trace,
                    domain: domain_label,
                    contract_id,
                    contract: Some(contract),
                    lgc_seal,
                    algorithm_state,
                    evolve_summary,
                    user_lang,
                    lang_confidence: lang_conf,
                    primitive_split,
                    input_sha256,
                    legacy: Some(legacy),
                    status: LightStatus::Routed,
                }
            }
            Err(error) => {
                let contract_id = contract.id.clone();
                let lgc_seal = contract.lgc_seal.clone();
                LightResponse {
                    trace_id,
                    primitive_trace: trace,
                    domain: domain_label,
                    contract_id,
                    contract: Some(contract),
                    lgc_seal,
                    algorithm_state: dsl::AlgorithmState::Failed(error.to_string())
                        .label()
                        .to_string(),
                    evolve_summary,
                    user_lang,
                    lang_confidence: lang_conf,
                    primitive_split,
                    input_sha256,
                    legacy: Some(legacy),
                    status: LightStatus::ContractFailed(error.to_string()),
                }
            }
        })
    }

    /// Wrapper vetëm për testet ekzistuese; prodhimi propagandon
    /// `TraceIdentityError` përmes `try_receive`.
    #[cfg(test)]
    pub fn receive(&self, req: LightRequest) -> LightResponse {
        self.try_receive(&req)
            .expect("bounded test request must obtain a trace identity")
    }

    /// FORMAT OUTPUT — pas Shadow verdict + SHA256 + KODUNIK
    /// Konverton output të sistemit në gjuhë akademike tek user
    pub fn format_output(&self, content: &str, response: &LightResponse, verified: bool) -> String {
        self.lingua.format_output(
            content,
            &response.user_lang,
            &crate::lingua::Intent::Statement, // default — Shadow do specifikon
            verified,
        )
    }
}

impl Default for LightCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// §5  COORDINATOR STATUS
// ════════════════════════════════════════════════════════════════════════════

// ════════════════════════════════════════════════════════════════════════════
// §6  QUANTUM DISPATCH —  entry points për Quantum Platform
//     Shtuara mbi §4 ekzistues. Zero fshirje.
// ════════════════════════════════════════════════════════════════════════════

use crate::quantum_bridge::{QBridgeError, QuantumCycleResult, QuantumResult};

/// Rezultati i ciklit të plotë Light → Quantum → Light
#[derive(Debug)]
pub enum DispatchResult {
    /// Quantum u përgjigj me sukses
    Verified(QuantumResult),
    /// Owner-resolved inquiry; neither a terminal verdict nor knowledge write.
    Operational(shadow_contracts::pd_memory::PdOperationRecord),
    /// Quantum nuk u përgjigj brenda timeout
    Timeout(String),
    /// Gabim i bus ose kontratës
    BridgeError(String),
}

impl DispatchResult {
    pub fn is_verified(&self) -> bool {
        matches!(self, Self::Verified(r) if r.is_verified)
    }

    pub fn content(&self) -> &str {
        match self {
            Self::Verified(r) => &r.content,
            Self::Operational(record) => &record.request.scoped_question,
            Self::Timeout(s) => s.as_str(),
            Self::BridgeError(s) => s.as_str(),
        }
    }

    pub fn evolve_sfx(&self) -> &str {
        match self {
            Self::Verified(r) => &r.evolve_sfx,
            _ => dsl::SFX_NEGATIVE,
        }
    }

    pub fn algorithm_state(&self) -> &str {
        match self {
            Self::Verified(r) => &r.algorithm_state,
            Self::Operational(_) => "OPERATIONAL_NO_VERDICT",
            Self::Timeout(_) => "TIMEOUT",
            Self::BridgeError(_) => "BRIDGE_ERROR",
        }
    }
}

impl LightCoordinator {
    pub fn dispatch_to_quantum_with_iz(
        &self,
        response: &LightResponse,
        text: &str,
        predecessor: Option<&shadow_contracts::pd_recovery::RecoveryLookup>,
        kie_context: Option<&shadow_contracts::kie::KieContext>,
        iz_context: Option<&shadow_contracts::pd_memory::iz::IzContext>,
    ) -> DispatchResult {
        // Porta e vetme e daljes: status + lineage + receipt + handoff durable.
        match response.ready_for_dispatch() {
            Ok(()) => {}
            Err(reason) => return DispatchResult::BridgeError(reason),
        }

        let lang_code = response.user_lang.code().to_string();

        match crate::quantum_bridge::full_quantum_cycle_with_iz(
            &response.trace_id,
            &response.domain,
            &response.contract_id,
            &response.lgc_seal, // lgc_seal shtohet në §3 më poshtë
            &lang_code,
            &response.evolve_summary,
            &response.input_sha256,
            text,
            None, // finite native transport budget (90s); no authority/hold extension
            predecessor,
            kie_context,
            iz_context,
        ) {
            Ok(QuantumCycleResult::Operational(record)) => DispatchResult::Operational(record),
            Ok(QuantumCycleResult::Native(result)) => match result.is_bound_to(&response.trace_id) {
                true => DispatchResult::Verified(result),
                false => DispatchResult::BridgeError(
                    "QBRIDGE:RESPONSE_NOT_BOUND_TO_GCL_ANCHOR".to_string(),
                ),
            },
            Err(QBridgeError::Timeout(ms)) => DispatchResult::Timeout(format!("TIMEOUT:{}ms", ms)),
            Err(e) => DispatchResult::BridgeError(e.to_string()),
        }
    }

    /// HANDLE_QUANTUM_RESPONSE — përpunon QuantumResult.
    ///
    /// Thirret pas dispatch_to_quantum() me sukses.
    /// Hapat:
    ///   1. Shton degën sipas verdict, me koherencë nga Quantum (potentiality).
    ///      → kjo rillogarit vulën 500 te EvolveTrace.
    ///   2. Lexon SupremeOutcome-in që Quantum e ka marrë nga Shadow main.
    ///      Light nuk ekzekuton një verdikt të dytë lokal.
    ///   3. Formaton output-in për përdoruesin.
    ///
    /// Kthen: (output_string, shadow_verdict).
    pub fn handle_quantum_response(
        &self,
        dispatch: &DispatchResult,
        response: &mut LightResponse,
    ) -> (String, crate::shadow_seal_bridge::ShadowVerdict) {
        use crate::shadow_seal_bridge::ShadowVerdict;

        match response.ready_for_dispatch() {
            Ok(()) => {}
            Err(reason) => {
                let output =
                    self.format_output(&format!("LIGHT_HANDOFF_REFUSED:{reason}"), response, false);
                return (output, ShadowVerdict::VerifiableNotPrimitive);
            }
        }
        match dispatch {
            DispatchResult::Verified(result) => match result.is_bound_to(&response.trace_id) {
                true => {}
                false => {
                    let output = self.format_output(
                        "QUANTUM_RESPONSE_NOT_BOUND_TO_LIGHT_I0",
                        response,
                        false,
                    );
                    return (output, ShadowVerdict::VerifiableNotPrimitive);
                }
            },
            _ => {}
        }
        let suffix = dispatch.evolve_sfx();
        let coherence = Self::coherence_from_dispatch(dispatch);

        // EvolveTrace autoritative është ajo që lindi brenda receipt-it LGC.
        // Nuk ndërtohet trace e dytë pas Quantum-it.
        let evolve_bound = match response.legacy.as_mut() {
            Some(legacy) => match legacy.evolve.branch_scored(suffix, coherence) {
                Ok(_) => true,
                Err(error) => {
                    eprintln!("  [EVOLVE] FAIL-CLOSED: {error}");
                    false
                }
            },
            None => {
                eprintln!("  [LEGACY] FAIL-CLOSED: receipt/EvolveTrace mungon pas dispatch-it");
                false
            }
        };

        // QuantumOutput ndërtohet vetëm pas ciklit real Quantum→Shadow main
        // dhe bart SupremeOutcome-in e Shadow. Light vetëm e përkthen në tipin
        // e vet historik; nuk ekzekuton më një verdikt të dytë lokal.
        let verified = dispatch.is_verified() && evolve_bound;
        let verdict = match verified {
            true => ShadowVerdict::PrimitiveKnowledge,
            false => ShadowVerdict::VerifiableNotPrimitive,
        };
        let content = dispatch.content();
        let output = self.format_output(content, response, verified);
        (output, verdict)
    }

    /// Koherenca [0,1] nga rezultati i Quantum-it.
    /// OPTIMAL → e lartë (mbetet primitiv); NEGATIVE/HOLD/DISMISSED → e ulët.
    pub(crate) fn coherence_from_dispatch(dispatch: &DispatchResult) -> f32 {
        match dispatch {
            DispatchResult::Verified(r) => {
                // potentiality është tashmë [0,1]; OPTIMAL e afron me 1.0
                let base = r.potentiality.clamp(0.0, 1.0);
                // verdikti rrit ose ul: OPTIMAL → +, ndryshe → toward 0
                let verdict_factor = match r.verdict.as_str() {
                    "OPTIMAL" => 1.0,
                    "HOLD" => 0.85,
                    _ => 0.50, // NEGATIVE / DISMISSED → fragmentim
                };
                (base * verdict_factor).clamp(0.0, 1.0)
            }
            // Timeout/BridgeError → s'ka propozim → fragmentim total
            _ => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_contract_is_bounded_before_copy_and_rejects_frame_breaks() {
        // 555 = maksimumi byte-sh i field keys + trace/domain/contract/LGC/
        // lang/evolve/SHA/timestamp pas escaping-ut ekzistues.
        let worst_case_wire = MAX_LIGHT_INPUT_BYTES.saturating_mul(2).saturating_add(555);
        assert_eq!(worst_case_wire, 1_579);
        assert_eq!(2_048usize.saturating_sub(worst_case_wire), 469);

        let exact = "a".repeat(MAX_LIGHT_INPUT_BYTES);
        let request = LightRequest::try_new(&exact).expect("exact byte limit");
        assert_eq!(request.raw_len(), MAX_LIGHT_INPUT_BYTES);
        assert_eq!(request.text(), exact);

        let oversized = "a".repeat(MAX_LIGHT_INPUT_BYTES + 1);
        match LightRequest::try_new(&oversized) {
            Err(LightInputError::TooLarge {
                limit: MAX_LIGHT_INPUT_BYTES,
                observed_at_least,
            }) => assert_eq!(observed_at_least, MAX_LIGHT_INPUT_BYTES + 1),
            other => panic!("expected bounded rejection, received {other:?}"),
        }
        assert!(matches!(
            LightRequest::try_new(" \t "),
            Err(LightInputError::Empty)
        ));
        assert!(matches!(
            LightRequest::try_new("first\nsecond"),
            Err(LightInputError::EmbeddedLineBreak { byte_offset: 5 })
        ));
    }

    #[test]
    fn receive_uses_one_legacy_authority_and_one_kodunik() {
        let coordinator = LightCoordinator::new();
        let response = coordinator.receive(LightRequest::new(
            "Analizo motorin industrial të kuq dhe sensorin termik",
        ));
        assert!(response.status.is_ok(), "{}", response.status.label());
        assert!(response.primitive_continuity_valid());
        assert!(response.authority_continuity_valid());
        assert!(response.formula_anchor_valid());
        let legacy = response.legacy.as_ref().expect("Legacy coordination");
        assert!(legacy.coordination_fragment_valid().is_ok());
        assert_eq!(response.contract_id, legacy.receipt.kodunik.code);
        assert_eq!(response.input_sha256, legacy.receipt.input_sha256);
        assert_eq!(response.primitive_split, legacy.primitive_split);
        response
            .ready_for_handoff()
            .expect("coordinate collapse must be complete before export");
        assert!(response.ready_for_dispatch().is_err());
    }

    #[test]
    fn durable_evidence_is_required_before_dispatch() {
        let coordinator = LightCoordinator::new();
        let mut response =
            coordinator.receive(LightRequest::new("Krijo raportin final të laboratorit"));
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
            .ready_for_dispatch()
            .expect("dispatch must open only after durable evidence");
    }

    #[test]
    fn input_without_primitive_material_fails_closed() {
        let coordinator = LightCoordinator::new();
        let response = coordinator.receive(LightRequest::new("dhe ose por"));
        assert!(matches!(&response.status, LightStatus::LegacyFailed(_)));
        assert!(response.legacy.is_none());
        assert!(response.ready_for_dispatch().is_err());
    }

    #[test]
    fn handoff_refuses_contract_or_gcl_anchor_tampering() {
        let coordinator = LightCoordinator::new();
        let mut response = coordinator.receive(LightRequest::new(
            "Analizo lidhjen e ankorës primitive me kontratën",
        ));
        assert!(response.formula_anchor_valid());
        response.contract_id.push_str("-tampered");
        assert!(!response.authority_continuity_valid());
        assert!(response.ready_for_handoff().is_err());
        assert!(response.ready_for_dispatch().is_err());
    }

    #[test]
    fn formula_anchor_refuses_negative_axis_index_tampering() {
        let coordinator = LightCoordinator::new();
        let mut response = coordinator.receive(LightRequest::new(
            "Analizo motorin industrial dhe sensorin termik",
        ));
        assert!(response.besa_continuity_valid());
        response
            .legacy
            .as_mut()
            .expect("Legacy coordination")
            .besa_attestation
            .xi_negative_index_sha256 = "tampered".to_string();
        assert!(!response.besa_continuity_valid());
        assert!(!response.formula_anchor_valid());
        assert!(response.ready_for_handoff().is_err());

        let mut canonical_drift = coordinator.receive(LightRequest::new(
            "Analizo motorin industrial dhe sensorin termik përsëri",
        ));
        canonical_drift
            .legacy
            .as_mut()
            .expect("Legacy coordination")
            .besa_attestation
            .enrichment_sha256 = "00".repeat(32);
        assert!(!canonical_drift.besa_continuity_valid());
        assert!(canonical_drift.ready_for_handoff().is_err());
    }

    #[test]
    fn handoff_commit_rechecks_the_complete_gcl_fragment() {
        let coordinator = LightCoordinator::new();
        let mut response = coordinator.receive(LightRequest::new(
            "Analizo vazhdimësinë e direktivës me primitive anchor",
        ));
        response
            .legacy
            .as_mut()
            .expect("Legacy coordination")
            .directive = GjataCollapseLaw::issue(CollapsePhase::Reasoning);
        let unusable_evidence = HandoffEvidence {
            binding_sha256: String::new(),
            receipt_sha256: String::new(),
            besa_attestation_sha256: String::new(),
            besa_enrichment_sha256: String::new(),
            commit_sha256: String::new(),
            primitive_anchor_sha256: String::new(),
            receipt_written: false,
            besa_written: false,
            primitive_anchor_written: false,
            commit_written: false,
            flushed: false,
            kodunik_verified: false,
        };
        assert!(!response.formula_anchor_valid());
        assert!(response
            .complete_legacy_handoff(&unusable_evidence)
            .is_err());
    }
}

#[cfg(test)]
impl LightCoordinator {
    pub fn try_receive(&self, req: &LightRequest) -> Result<LightResponse, TraceIdentityError> {
        self.try_receive_with_iz(req, None)
    }
}

#[cfg(test)]
impl LightRequest {
    pub fn raw_len(&self) -> usize {
        self.raw.len()
    }
}
