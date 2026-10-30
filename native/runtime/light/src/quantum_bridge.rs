// quantum_bridge.rs — Light Platform → Quantum Communication
// GJATA LEGACY™ — Light Platform
//
// Entry point i Light-it për komunikim me Quantum Platform.
// Pasqyron formatin e quantum_platform::light_bridge dhe përdor procesin real.
//
// FLUKSI:
//   1. full_quantum_cycle(response, text)
//      → serializon QuantumInput
//      → nis quantum-platform me stdin/stdout të kufizuar
//      → pret një [Q→LIGHT] envelope me timeout
//      → riverifikon trace-in e të njëjtit i₀
//
// send_to_quantum/receive_from_quantum mbeten API compatibility për bus-in C
// proces-lokal; rrjedha prodhuese nuk pretendon se ai është IPC.
//
// FORMAT payload dërguar (saktë si quantum::types::QuantumInput::deserialize):
//   "trace_id={};domain={};contract_id={};lgc_seal={};lang_code={};evolve={};input_sha256={};ts={};text={}"
//
// FORMAT payload marrë (saktë si quantum::types::QuantumOutput::serialize):
//   "trace_id={};verdict={};pot={:.4};axis={:.1};seal={};sfx={};state={};territory={};depth={};ts={};content={}"

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ============================================================
// §1  KONSTANTET — identike me light_buss.h
// ============================================================

const LB_MAX_PAYLOAD: usize = 2048;

#[cfg(feature = "c_kernel")]
const LB_OK: i32 = 0;
#[cfg(feature = "c_kernel")]
const LB_ERR_NOT_INIT: i32 = -1;

// Finite native-process budget: one Quantum cycle includes multiple real Shadow
// exchanges. This transport allowance grants no authority and extends no hold.
// Explicit per-call overrides remain available for fault/deadline tests.
const QUANTUM_TIMEOUT_SECS: u64 = 90;
// Stdout/stderr drenohen plotësisht për të shmangur deadlock, por memoria e
// mbajtur për secilin kanal kufizohet fail-closed.
const QUANTUM_PROCESS_OUTPUT_MAX: usize = 1024 * 1024;

// Emrat e moduleve në bus — saktë si Quantum i pret

static QUANTUM_BUS_READY: AtomicBool = AtomicBool::new(false);

// ============================================================
// §3  extern "C" — FFI declarations (light_buss.c)
//     Aktiv vetëm me feature = "c_kernel"
// ============================================================

#[cfg(feature = "c_kernel")]
extern "C" {
    fn light_buss_init() -> i32;

    fn light_buss_destroy();
}

// ============================================================
// §4  QBridgeError
// ============================================================

#[derive(Debug, Clone)]
pub enum QBridgeError {
    /// Bus jo i inicializuar
    #[cfg(feature = "c_kernel")]
    BussNotInit,
    /// Timeout — Quantum nuk u përgjigj
    Timeout(u64),
    /// Payload shumë i madh (> 2048 bytes)
    PayloadTooLarge(usize),
    /// Deserializim i QuantumOutput dështoi
    DeserializeFail(String),
    /// CString konvertim dështoi
    StringConvert,
    /// Gabim tjetër i bus
    #[cfg(feature = "c_kernel")]
    BussError(i32),
    /// Përgjigjja erdhi nga bus-i Quantum, por i përket një i₀ tjetër.
    ResponseTraceMismatch { expected: String, actual: String },
    /// Envelope-i Quantum→Light nuk ruan formën ose lidhjen e deklaruar.
    ResponseEnvelopeInvalid,
    /// FAIL-CLOSED (#8): bus real mungon dhe dev_simulation s'është aktiv.
    /// Në prodhim, mungesa e c_kernel = gabim kritik, JO no-op i heshtur.
    #[cfg(not(feature = "c_kernel"))]
    BusUnavailable,
    /// Binari real Quantum nuk u gjet ose nuk është executable.
    ProcessUnavailable(String),
    /// Procesi Quantum nuk mund të nisej.
    ProcessSpawn(String),
    /// Kufiri stdin/stdout/stderr i procesit dështoi.
    ProcessIo(String),
    /// Quantum përfundoi pa sukses.
    ProcessExit(String),
    /// Quantum nuk prodhoi saktësisht një envelope `[Q→LIGHT]`.
    ProcessOutput(String),
}

impl std::fmt::Display for QBridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(feature = "c_kernel")]
            Self::BussNotInit => write!(f, "QBRIDGE:NOT_INIT"),
            Self::Timeout(ms) => write!(f, "QBRIDGE:TIMEOUT:{}ms", ms),
            Self::PayloadTooLarge(n) => write!(f, "QBRIDGE:PAYLOAD_TOO_LARGE:{}", n),
            Self::DeserializeFail(s) => write!(f, "QBRIDGE:DESER:{}", s),
            Self::StringConvert => write!(f, "QBRIDGE:CSTRING_FAIL"),
            #[cfg(feature = "c_kernel")]
            Self::BussError(rc) => write!(f, "QBRIDGE:BUSS_RC:{}", rc),
            Self::ResponseTraceMismatch { expected, actual } => {
                write!(f, "QBRIDGE:TRACE_MISMATCH:{expected}:{actual}")
            }
            Self::ResponseEnvelopeInvalid => write!(f, "QBRIDGE:RESPONSE_ENVELOPE_INVALID"),
            #[cfg(not(feature = "c_kernel"))]
            Self::BusUnavailable => write!(f, "QBRIDGE:BUS_UNAVAILABLE"),
            Self::ProcessUnavailable(reason) => {
                write!(f, "QBRIDGE:PROCESS_UNAVAILABLE:{reason}")
            }
            Self::ProcessSpawn(reason) => write!(f, "QBRIDGE:PROCESS_SPAWN:{reason}"),
            Self::ProcessIo(reason) => write!(f, "QBRIDGE:PROCESS_IO:{reason}"),
            Self::ProcessExit(reason) => write!(f, "QBRIDGE:PROCESS_EXIT:{reason}"),
            Self::ProcessOutput(reason) => write!(f, "QBRIDGE:PROCESS_OUTPUT:{reason}"),
        }
    }
}

// ============================================================
// §5  QuantumResult — output nga Quantum Platform
//
// Deserialized from Quantum's QuantumOutput::serialize() format:
//   "trace_id={};verdict={};pot={:.4};axis={:.1};seal={};
//    sfx={};state={};territory={};depth={};ts={};content={}"
// ============================================================

// ─────────────────────────────────────────────────────────────────────────────
// PATCH 7.2 — AlgorithmStateTag: tipizim i 'algorithm_state' (shablloni 7.3)
// ─────────────────────────────────────────────────────────────────────────────

/// Gjendja e algoritmit, E TIPIZUAR (jo String i papërpunuar).
///
/// Para patch-it, 'algorithm_state' pranohej si tekst i lirë nga Quantum.
/// Tani tipizohet në enum — i njëjti standard si bridge_shadow (7.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlgorithmStateTag {
    /// Primitivi u vulos (SEALED).
    Sealed,
    /// U refuzua (REJECTED).
    Rejected,
    /// Kaloi te Shadow për verifikim (SHADOW_VERIFY).
    ShadowVerify,
    /// Gjendje e panjohur (fallback).
    Unknown,
}

impl AlgorithmStateTag {
    /// from_wire — parson string-un në enum të tipizuar. Zero if — match.
    pub fn from_wire(s: &str) -> Self {
        match s {
            "SEALED" => AlgorithmStateTag::Sealed,
            "REJECTED" => AlgorithmStateTag::Rejected,
            "SHADOW_VERIFY" => AlgorithmStateTag::ShadowVerify,
            _ => AlgorithmStateTag::Unknown,
        }
    }
}

#[derive(Debug)]
pub struct QuantumResult {
    /// Trace ID — i njëjtë me input (chain continuity)
    pub trace_id: String,
    /// OPTIMAL | NEGATIVE | HOLD | DISMISSED
    pub verdict: String,
    /// Potentiality [0,1] from PIM
    pub potentiality: f32,
    /// Axis score [-100,+100] from PRO
    pub axis_score: f32,
    /// KODUNIK seal nga Quantum (QNT:...)
    pub seal: String,
    /// DSL suffix: VFYKNG (verified) ose NEGKNG (negative)
    pub evolve_sfx: String,
    /// AlgorithmState: SEALED | REJECTED | SHADOW_VERIFY
    pub algorithm_state: String,
    /// Territory: INDUSTRIAL | SEMANTIC | PHILOSOPHICAL | ...
    pub territory: String,
    /// Reasoning depth: SHALLOW | MEDIUM | DEEP | VERY_DEEP
    pub depth_hint: String,
    /// Content — reasoning summary
    pub content: String,
    /// Timestamp i envelope-it që Quantum ndërtoi pas përgjigjes së Shadow.
    pub ts: u64,
    /// true = shadow bit 1 (Optimal)
    pub is_verified: bool,
    pub pd_continuity: Option<shadow_contracts::pd_recovery::RecoveryLookup>,
}

// FIX #1 — unescape për fusha free-text (simetrik me quantum::escape_field).
/// escape_field — zëvendëson delimiterët e protokollit. Simetrik me unescape. Zero if.
fn escape_field(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(';', "\\s")
        .replace('=', "\\e")
        .replace('|', "\\p")
        .replace(':', "\\c")
}

/// Kthen sekuencat \s \e \p \c në ; = | : . Zero if — fold state machine.
fn unescape_field(s: &str) -> String {
    let (out, _) = s
        .chars()
        .fold((String::new(), false), |(mut acc, esc), ch| match esc {
            true => {
                let decoded = match ch {
                    's' => ';',
                    'e' => '=',
                    'p' => '|',
                    'c' => ':',
                    '\\' => '\\',
                    other => other,
                };
                acc.push(decoded);
                (acc, false)
            }
            false => match ch {
                '\\' => (acc, true),
                _ => {
                    acc.push(ch);
                    (acc, false)
                }
            },
        });
    out
}

impl QuantumResult {
    pub fn from_payload(s: &str) -> Result<Self, QBridgeError> {
        let mut trace_id = String::new();
        let mut verdict = String::new();
        let mut potentiality = f32::NAN;
        let mut axis_score = f32::NAN;
        let mut seal = String::new();
        let mut evolve_sfx = String::new();
        let mut algorithm_state = String::new();
        let mut territory = String::new();
        let mut depth_hint = String::new();
        let mut ts = 0u64;
        let mut content = String::new();

        for part in s.split(';') {
            match part.split_once('=') {
                Some((k, v)) => {
                    match k {
                        "trace_id" => trace_id = unescape_field(v),
                        "verdict" => verdict = unescape_field(v),
                        // FAIL-CLOSED: vlerë numerike e prishur = payload i refuzuar
                        // me reason — kurrë 0.0 e butë (korrupsioni s'maskohet).
                        "pot" => {
                            potentiality = match v.parse() {
                                Ok(x) => x,
                                Err(_) => {
                                    return Err(QBridgeError::DeserializeFail(format!(
                                        "LB_ERR_NUM: pot i prishur '{}'",
                                        v
                                    )))
                                }
                            }
                        }
                        "axis" => {
                            axis_score = match v.parse() {
                                Ok(x) => x,
                                Err(_) => {
                                    return Err(QBridgeError::DeserializeFail(format!(
                                        "LB_ERR_NUM: axis i prishur '{}'",
                                        v
                                    )))
                                }
                            }
                        }
                        "seal" => seal = unescape_field(v),
                        "sfx" => evolve_sfx = unescape_field(v),
                        "state" => algorithm_state = unescape_field(v),
                        "territory" => territory = unescape_field(v),
                        "depth" => depth_hint = unescape_field(v),
                        "ts" => {
                            ts = match v.parse() {
                                Ok(value) => value,
                                Err(_) => {
                                    return Err(QBridgeError::DeserializeFail(format!(
                                        "LB_ERR_NUM: ts i prishur '{}'",
                                        v
                                    )))
                                }
                            }
                        }
                        "content" => content = unescape_field(v),
                        _ => {}
                    }
                }
                None => {}
            }
        }

        // FAIL-CLOSED (#3): fushat kritike duhet të jenë të pranishme.
        // Para: vetëm trace_id; payload i prishur kthente Ok me DISMISSED/0.0 (sukses i rremë).
        // Tani: trace_id, verdict, seal, state, content të detyrueshme. Zero if — match.
        let missing = match trace_id.is_empty() {
            true => Some("trace_id"),
            false => match verdict.is_empty() {
                true => Some("verdict"),
                false => match seal.is_empty() {
                    true => Some("seal"),
                    false => match algorithm_state.is_empty() {
                        true => Some("state"),
                        false => match evolve_sfx.is_empty() {
                            true => Some("sfx"),
                            false => match territory.is_empty() {
                                true => Some("territory"),
                                false => match depth_hint.is_empty() {
                                    true => Some("depth"),
                                    false => match ts == 0 {
                                        true => Some("ts"),
                                        false => match content.is_empty() {
                                            true => Some("content"),
                                            false => None,
                                        },
                                    },
                                },
                            },
                        },
                    },
                },
            },
        };

        match missing {
            Some(field) => {
                return Err(QBridgeError::DeserializeFail(format!(
                    "missing critical field: {field}"
                )))
            }
            None => {}
        }

        // RUNTIME HARDENING (runtime_patch): validim i vlerave.
        // verdict duhet të jetë një nga vlerat e njohura — fail-closed. Zero if — match.
        let verdict_ok = match verdict.as_str() {
            "OPTIMAL" | "NEGATIVE" | "HOLD" | "DISMISSED" => true,
            _ => false,
        };
        match verdict_ok {
            true => {}
            false => {
                return Err(QBridgeError::DeserializeFail(format!(
                    "invalid verdict: {verdict}"
                )))
            }
        }

        // algorithm_state duhet të parsohet në tag të njohur (jo Unknown). Zero if — match.
        let state_tag = AlgorithmStateTag::from_wire(&algorithm_state);
        match state_tag == AlgorithmStateTag::Unknown {
            true => {
                return Err(QBridgeError::DeserializeFail(format!(
                    "invalid algorithm_state: {algorithm_state}"
                )))
            }
            false => {}
        }

        let is_verified = verdict == "OPTIMAL";

        let result = QuantumResult {
            trace_id,
            verdict,
            potentiality,
            axis_score,
            seal,
            evolve_sfx,
            algorithm_state,
            territory,
            depth_hint,
            content,
            ts,
            is_verified,
            pd_continuity: None,
        };
        match result.wire_shape_is_valid() {
            true => Ok(result),
            false => Err(QBridgeError::ResponseEnvelopeInvalid),
        }
    }

    /// Forma e envelope-it që Light mund ta provojë pa marrë autoritet:
    /// vlerat numerike janë të fundme/kufizuara, verdict/state/sfx janë
    /// konsistente dhe seal-i deklaron të njëjtin trace me 16 hex kanonike.
    pub fn wire_shape_is_valid(&self) -> bool {
        let state_bound = matches!(
            (self.verdict.as_str(), self.algorithm_state.as_str()),
            ("OPTIMAL", "SEALED")
                | ("HOLD", "SHADOW_VERIFY")
                | ("NEGATIVE" | "DISMISSED", "REJECTED")
        );
        let sfx_bound = matches!(
            (self.verdict.as_str(), self.evolve_sfx.as_str()),
            ("OPTIMAL", "VFYKNG") | ("HOLD" | "NEGATIVE" | "DISMISSED", "NEGKNG")
        );
        let seal_bound = match self
            .seal
            .strip_prefix("QNT:")
            .and_then(|value| value.rsplit_once(':'))
        {
            Some((trace, digest)) => {
                trace == self.trace_id
                    && digest.len() == 16
                    && digest
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            }
            None => false,
        };
        !self.trace_id.is_empty()
            && !self.content.is_empty()
            && !self.territory.is_empty()
            && !self.depth_hint.is_empty()
            && self.ts != 0
            && self.potentiality.is_finite()
            && (0.0..=1.0).contains(&self.potentiality)
            && self.axis_score.is_finite()
            && (-100.0..=100.0).contains(&self.axis_score)
            && self.is_verified == (self.verdict == "OPTIMAL")
            && state_bound
            && sfx_bound
            && seal_bound
    }

    /// Lidhja e përgjigjes me i₀ që Light dërgoi. Ky kontroll kryhet pas
    /// deserializimit dhe përsëri në koordinator para konsumit.
    pub fn is_bound_to(&self, expected_trace_id: &str) -> bool {
        self.wire_shape_is_valid() && self.trace_id == expected_trace_id
    }
}

// ============================================================
// §6  Bus safe wrappers — thin layer mbi extern "C"
// ============================================================

/// Initialize light_buss — thirret 1 herë në startup
pub fn buss_init() -> Result<(), QBridgeError> {
    #[cfg(feature = "c_kernel")]
    {
        let rc = unsafe { light_buss_init() };
        let result = match rc {
            LB_OK => Ok(()),
            LB_ERR_NOT_INIT => Err(QBridgeError::BussNotInit),
            other => Err(QBridgeError::BussError(other)),
        };
        QUANTUM_BUS_READY.store(result.is_ok(), Ordering::Release);
        return result;
    }
    // RUNTIME DEFAULT = EKZEKUTIV: prodhimi ndërtohet me `c_kernel` → përdoret
    // ura REALE C më sipër (light_buss_*). Simulimi no-op kompilohet VETËM me
    // `dev_simulation` eksplicit (testim lokal) dhe NUK ekziston në binarin e
    // prodhimit. Pa c_kernel dhe pa dev_simulation → fail-closed (Err kritik).
    #[cfg(all(not(feature = "c_kernel"), feature = "dev_simulation"))]
    {
        QUANTUM_BUS_READY.store(true, Ordering::Release);
        Ok(()) // dev_simulation — no-op i lejuar VETËM në testim lokal
    }
    #[cfg(all(not(feature = "c_kernel"), not(feature = "dev_simulation")))]
    {
        QUANTUM_BUS_READY.store(false, Ordering::Release);
        Err(QBridgeError::BusUnavailable) // RUNTIME/PRODHIM: bus real mungon → kritik
    }
}

/// Statusi GCL i urës prodhuese. Bus-i C ruhet për compatibility lokale, por
/// READY kërkon binarin real Quantum që konsumon stdin dhe kthen stdout.
#[cfg(not(test))]
pub fn gcl_status() -> crate::lab_contracts::gcl_presume::GclReadiness {
    match crate::project_process_bridge::resolve_quantum_binary() {
        Ok(_) => crate::lab_contracts::gcl_presume::GclReadiness::Ready,
        Err(_) => crate::lab_contracts::gcl_presume::GclReadiness::NotReady(
            "Quantum process binary mungon ose nuk është executable",
        ),
    }
}

#[cfg(test)]
pub fn gcl_status() -> crate::lab_contracts::gcl_presume::GclReadiness {
    crate::lab_contracts::gcl_presume::GclReadiness::Ready
}

pub fn production_process_ready() -> Result<(), QBridgeError> {
    crate::project_process_bridge::resolve_quantum_binary()
        .map(|_| ())
        .map_err(QBridgeError::ProcessUnavailable)
}

/// Shutdown light_buss
pub fn buss_destroy() {
    #[cfg(feature = "c_kernel")]
    unsafe {
        light_buss_destroy();
    }
    QUANTUM_BUS_READY.store(false, Ordering::Release);
}

// ============================================================
// §7  send_to_quantum() — ENTRY POINT 1
//
// Builds QuantumInput payload and sends via light_buss HIGH.
// Called from LightCoordinator::dispatch_to_quantum().
//
// Payload format (saktë si Quantum QuantumInput::deserialize):
//   trace_id=...;domain=...;contract_id=...;lgc_seal=...;
//   lang_code=...;evolve=...;input_sha256=...;ts=...;text=...
// ============================================================

fn serialize_quantum_input(
    trace_id: &str,
    domain: &str,
    contract_id: &str,
    lgc_seal: &str,
    lang_code: &str,
    evolve_summary: &str,
    input_sha256: &str,
    text: &str,
) -> Result<String, QBridgeError> {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Saktë si Quantum::QuantumInput::deserialize() pret.
    // RUNTIME HARDENING (      ): escape i të gjitha fushave (delimiter injection).
    let payload = format!(
        "trace_id={};domain={};contract_id={};lgc_seal={};lang_code={};evolve={};input_sha256={};ts={};text={}",
        escape_field(trace_id),
        escape_field(domain),
        escape_field(contract_id),
        escape_field(lgc_seal),
        escape_field(lang_code),
        escape_field(evolve_summary),
        escape_field(input_sha256),
        ts,
        escape_field(text),
    );
    match payload.len() <= LB_MAX_PAYLOAD {
        true => Ok(payload),
        false => Err(QBridgeError::PayloadTooLarge(payload.len())),
    }
}

/// send_project_to_quantum — e njëjta rrjedhë Light→Quantum, por me
/// ProjectContextWitness të lëshuar nga Shadow main. Evidenca shkencore nuk
/// krijon kanal paralel: ajo udhëton si pjesë opsionale e inputit që më pas
/// paketohet në FinalEvidenceWire nën GCL.
pub fn build_project_payload(
    trace_id: &str,
    domain: &str,
    contract_id: &str,
    lgc_seal: &str,
    lang_code: &str,
    evolve_summary: &str,
    input_sha256: &str,
    text: &str,
    witness: &shadow_contracts::ProjectContextWitnessWire,
    title: &str,
    hypothesis: &str,
    assumptions: &[String],
    description: &str,
    files: &[(u8, Vec<u8>)],
) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let assumptions_wire = assumptions.join("\n");
    let files_wire = crate::gcl_project_contract::encode_files_hex(files);
    format!(
        concat!(
            "trace_id={};domain={};contract_id={};lgc_seal={};lang_code={};",
            "evolve={};input_sha256={};ts={};text={};",
            "project_id={};project_user_id={};project_trace_id={};",
            "project_revision={};project_content_sha256={};",
            "project_sovereign_flags={};project_context_sha256={};",
            "project_title={};project_hypothesis={};project_assumptions={};",
            "project_description={};project_files={}"
        ),
        escape_field(trace_id),
        escape_field(domain),
        escape_field(contract_id),
        escape_field(lgc_seal),
        escape_field(lang_code),
        escape_field(evolve_summary),
        escape_field(input_sha256),
        ts,
        escape_field(text),
        witness.project_id,
        witness.user_id,
        witness.trace_id,
        witness.revision,
        escape_field(&witness.content_sha256),
        witness.light_sovereign_flags,
        escape_field(&witness.context_sha256),
        escape_field(title),
        escape_field(hypothesis),
        escape_field(&assumptions_wire),
        escape_field(description),
        escape_field(&files_wire),
    )
}

// ============================================================
// §8  receive_from_quantum() — ENTRY POINT 2 (callback)
//
// Polls light_buss for response from quantum_platform.
// Non-blocking per tentativë, timeout me retry loop.
// Called from LightCoordinator::wait_quantum_response().
// ============================================================

// ============================================================
// §9  full_quantum_cycle() — cikli i plotë Light→Quantum→Light
//
// Ekzekuton procesin real Quantum dhe lidh stdin/stdout në 1 thirrje.
// Native timeout default = QUANTUM_TIMEOUT_SECS (90s); hold expiry is unchanged.
// Called from LightCoordinator.
// ============================================================

// Diagnostics are observations only. Losing a diagnostic sink must not turn a
// completed Quantum cycle into a new refusal or imply a fresh retry.
fn forward_success_diagnostics(
    bytes: &[u8],
    writer: &mut impl std::io::Write,
) -> std::io::Result<()> {
    writer.write_all(bytes)?;
    writer.flush()
}

fn parse_successful_process_output(
    stdout_bytes: Vec<u8>,
    stderr_bytes: &[u8],
    diagnostics: &mut impl std::io::Write,
    fallback: &mut impl std::io::Write,
) -> Result<QuantumCycleResult, QBridgeError> {
    if let Err(error) = forward_success_diagnostics(stderr_bytes, diagnostics) {
        // Best effort only: the original protocol result remains authoritative
        // even if both output sinks fail. This marker is not a response envelope.
        let _ = writeln!(
            fallback,
            "[QUANTUM_DIAGNOSTIC_WRITE_FAILED] retained_bytes={} error={error:?} commit_status=unchanged no_retry=true",
            stderr_bytes.len(),
        );
    }
    let stdout_text = String::from_utf8(stdout_bytes).map_err(|_| {
        QBridgeError::ProcessOutput("Quantum stdout is not valid UTF-8".to_string())
    })?;
    parse_process_output(&stdout_text)
}

fn bounded_stderr(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .replace('\r', " ")
        .replace('\n', " ")
        .chars()
        .take(512)
        .collect()
}

/// Operational output is a stored inquiry, never a native terminal capability.
#[derive(Debug)]
pub enum QuantumCycleResult {
    Native(QuantumResult),
    Operational(shadow_contracts::pd_memory::PdOperationRecord),
}

fn invoke_quantum_process(payload: &str, timeout_secs: u64) -> Result<QuantumCycleResult, QBridgeError> {
    let quantum_bin = crate::project_process_bridge::resolve_quantum_binary()
        .map_err(QBridgeError::ProcessUnavailable)?;
    let mut input = payload.as_bytes().to_vec();
    input.push(b'\n');
    let output = match crate::process_deadline::run(
        &mut Command::new(&quantum_bin), input, Duration::from_secs(timeout_secs), QUANTUM_PROCESS_OUTPUT_MAX,
    ) {
        Ok(output) => output,
        Err(error) => {
            if error.spawn_failed {
                return Err(QBridgeError::ProcessSpawn(format!("{}: {}", quantum_bin.display(), error.detail)));
            }
            let marker = if error.timed_out { "QUANTUM_TIMEOUT_COMMIT_STATUS_UNCERTAIN" }
                         else { "QUANTUM_IO_COMMIT_STATUS_UNCERTAIN" };
            eprintln!("[{marker}] deadline_secs={timeout_secs}; {}; resolve the exact retained recovery lookup before retry; no rollback or fresh retry inferred", error.detail);
            for (label, bytes) in [("STDOUT", &error.stdout), ("STDERR", &error.stderr)] {
                let retained = bytes.len().min(QUANTUM_PROCESS_OUTPUT_MAX);
                eprintln!("[QUANTUM_PROCESS_{label}]\n{}", String::from_utf8_lossy(&bytes[..retained]));
                if bytes.len() > retained {
                    eprintln!("[QUANTUM_PROCESS_{label}_TRUNCATED] retained={retained}");
                }
            }
            return if error.timed_out {
                Err(QBridgeError::Timeout(error.elapsed.as_millis().min(u128::from(u64::MAX)) as u64))
            } else { Err(QBridgeError::ProcessIo(error.detail)) };
        }
    };
    let status = output.status;
    let stdout_bytes = output.stdout;
    let stderr_bytes = output.stderr;
    match (
        stdout_bytes.len() <= QUANTUM_PROCESS_OUTPUT_MAX,
        stderr_bytes.len() <= QUANTUM_PROCESS_OUTPUT_MAX,
    ) {
        (true, true) => {}
        (false, _) => {
            return Err(QBridgeError::ProcessOutput(
                "Quantum stdout exceeded the bounded process limit".to_string(),
            ))
        }
        (_, false) => {
            return Err(QBridgeError::ProcessOutput(
                "Quantum stderr exceeded the bounded process limit".to_string(),
            ))
        }
    }
    match status.success() {
        true => {}
        false => {
            // Preserve the already-bounded native failure transcript for LGC/audit.
            // The UI summary alone can truncate the actual refusing condition.
            eprintln!(
                "[QUANTUM_NATIVE_FAILURE_STDOUT]\n{}",
                String::from_utf8_lossy(&stdout_bytes)
            );
            eprintln!(
                "[QUANTUM_NATIVE_FAILURE_STDERR]\n{}",
                String::from_utf8_lossy(&stderr_bytes)
            );
            return Err(QBridgeError::ProcessExit(format!(
                "status={status}; stderr={}",
                bounded_stderr(&stderr_bytes),
            )));
        }
    }

    // Both captures already passed the existing bounds, and the child exited
    // successfully. Preserve stderr even when the native outcome is a hold.
    parse_successful_process_output(
        stdout_bytes,
        &stderr_bytes,
        &mut std::io::stderr(),
        &mut std::io::stdout(),
    )
}

fn parse_process_output(stdout_text: &str) -> Result<QuantumCycleResult, QBridgeError> {
    let responses = stdout_text
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("[Q→LIGHT]  "))
        .collect::<Vec<_>>();
    let late = stdout_text.lines().filter_map(|line| line.strip_prefix("[PD_LATE_REFUSAL] ")).collect::<Vec<_>>();
    let references = stdout_text.lines().filter_map(|line| line.strip_prefix("[PD_NATIVE_REFERENCE] ")).collect::<Vec<_>>();
    match late.as_slice() {
        [] => {},
        [value] if responses.is_empty() && references.is_empty() => {
            use shadow_contracts::pd_memory as pd;
            if value.len() > 2 * (pd::MAX_REQUEST_BYTES + 256) || value.len() % 2 != 0
                || !value.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
                return Err(QBridgeError::ProcessOutput("PD_LATE_HEX".into()));
            }
            let bytes = value.as_bytes().chunks_exact(2).map(|pair| {
                let digit = |b:u8| match b { b'0'..=b'9'=>b-b'0', _=>b-b'a'+10 };
                digit(pair[0]) * 16 + digit(pair[1])
            }).collect::<Vec<_>>();
            let record = pd::decode_record(&bytes).map_err(QBridgeError::ProcessOutput)?;
            pd::late_refusal::validate_operation(&record.request).map_err(QBridgeError::ProcessOutput)?;
            return Ok(QuantumCycleResult::Operational(record));
        },
        _ => return Err(QBridgeError::ProcessOutput("PD_LATE_AMBIGUOUS_TERMINAL".into())),
    }
    let payload = match responses.as_slice() {
        [single] => *single,
        [] => {
            return Err(QBridgeError::ProcessOutput(
                "Quantum returned no [Q→LIGHT] envelope".to_string(),
            ))
        }
        _ => {
            return Err(QBridgeError::ProcessOutput(
                "Quantum returned multiple [Q→LIGHT] envelopes".to_string(),
            ))
        }
    };
    let mut result = QuantumResult::from_payload(payload)?;
    result.pd_continuity = match references.as_slice() {
        [] => None,
        [value] => Some(
            shadow_contracts::pd_followup::decode_reference(value)
                .map_err(QBridgeError::ProcessOutput)?,
        ),
        _ => {
            return Err(QBridgeError::ProcessOutput(
                "multiple PD native references".into(),
            ))
        }
    };
    Ok(QuantumCycleResult::Native(result))
}

fn bind_late_record(
    record: &shadow_contracts::pd_memory::PdOperationRecord,
    trace_id: &str, domain: &str, input_sha256: &str, text: &str,
    predecessor: Option<&shadow_contracts::pd_recovery::RecoveryLookup>,
    kie_context: Option<&shadow_contracts::kie::KieContext>,
    iz_context: Option<&shadow_contracts::pd_memory::iz::IzContext>,
) -> Result<(), String> {
    use shadow_contracts::pd_memory as pd;
    record.validate()?;
    let source = pd::late_refusal::validate_operation(&record.request)?;
    let request = shadow_contracts::decode_cycle_request(&source.attempt).map_err(|e|e.to_string())?;
    let prepared = shadow_contracts::pd_prepared::decode_prepared(&source.prepared)?;
    let expected_context = kie_context.ok_or("LIGHT_PD_LATE_KIE_CONTEXT_REQUIRED")?;
    if iz_context.is_some() || &source.context != expected_context
        || request.quantum.package_id != trace_id || request.quantum.session_id != trace_id
        || request.light.session_id != trace_id || prepared.pending.session_id != trace_id
        || request.quantum.territory != domain || request.light.territory != domain
        || source.context.identity.scope != domain
        || request.quantum.final_evidence.light_input_bytes != text.as_bytes()
        || request.quantum.final_evidence.light_input_sha256 != input_sha256
        || prepared.continuity.as_ref().map(|link|&link.predecessor) != predecessor
        || record.previous_record != expected_context.record_id {
        return Err("LIGHT_PD_LATE_CURRENT_INPUT_BINDING".into());
    }
    Ok(())
}

fn bind_late_owner(
    record: &shadow_contracts::pd_memory::PdOperationRecord,
    mut fetch: impl FnMut(&[u8]) -> Result<Vec<u8>,String>,
) -> Result<(),String> {
    use shadow_contracts::pd_memory as pd;
    let lookup = pd::PdLookup { identity:record.request.identity.clone(), operation_id:Some(record.request.operation_id.clone()) };
    let bytes = fetch(&pd::encode_lookup(&lookup)?)?;
    let actual = pd::decode_optional_record(&bytes)?.ok_or("LIGHT_PD_LATE_DURABLE_OPERATION_MISSING")?;
    if pd::encode_record(&actual)? != pd::encode_record(record)? {
        return Err("LIGHT_PD_LATE_DURABLE_OPERATION_MISMATCH".into());
    }
    Ok(())
}

pub fn full_quantum_cycle_with_iz(
    trace_id: &str,
    domain: &str,
    contract_id: &str,
    lgc_seal: &str,
    lang_code: &str,
    evolve_summary: &str,
    input_sha256: &str,
    text: &str,
    timeout_secs: Option<u64>,
    predecessor: Option<&shadow_contracts::pd_recovery::RecoveryLookup>,
    kie_context: Option<&shadow_contracts::kie::KieContext>,
    iz_context: Option<&shadow_contracts::pd_memory::iz::IzContext>,
) -> Result<QuantumCycleResult, QBridgeError> {
    validate_iz_native_route(iz_context, predecessor, kie_context)?;
    let timeout = timeout_secs.unwrap_or(QUANTUM_TIMEOUT_SECS);

    // HAPI 1: Ndërto të njëjtin wire të kufizuar që QuantumInput deserializon.
    let mut payload = serialize_quantum_input(
        trace_id,
        domain,
        contract_id,
        lgc_seal,
        lang_code,
        evolve_summary,
        input_sha256,
        text,
    )?;

    append_native_context(&mut payload, predecessor, kie_context)?;
    append_iz_context(&mut payload, iz_context)?;
    // Explicit additional clock unit for declared PDS1-derived cognitive state.
    // The historical seconds ts field remains unchanged.
    append_native_clock_ms(&mut payload, native_clock_millis(SystemTime::now())?)?;
    // HAPI 2: Nise procesin real, mbyll stdin pas një inputi, prit me timeout.
    let result = match invoke_quantum_process(&payload, timeout)? {
        QuantumCycleResult::Native(value) => value,
        QuantumCycleResult::Operational(record) => {
            bind_late_record(&record,trace_id,domain,input_sha256,text,predecessor,kie_context,iz_context)
                .map_err(QBridgeError::ProcessOutput)?;
            bind_late_owner(&record,|bytes| crate::pd_operational_bridge::native_exchange(
                crate::pd_operational_bridge::NativePeer::Shadow,"--pd-head-once",bytes,
                shadow_contracts::pd_memory::MAX_REQUEST_BYTES + 256))
                .map_err(QBridgeError::ProcessOutput)?;
            return Ok(QuantumCycleResult::Operational(record));
        },
    };

    // HAPI 3: Lidhe përgjigjen me të njëjtin i₀ para kthimit.
    match result.is_bound_to(trace_id) {
        true => Ok(QuantumCycleResult::Native(result)),
        false => match result.trace_id == trace_id {
            true => Err(QBridgeError::ResponseEnvelopeInvalid),
            false => Err(QBridgeError::ResponseTraceMismatch {
                expected: trace_id.to_string(),
                actual: result.trace_id,
            }),
        },
    }
}

fn validate_iz_native_route(
    context: Option<&shadow_contracts::pd_memory::iz::IzContext>,
    predecessor: Option<&shadow_contracts::pd_recovery::RecoveryLookup>,
    kie: Option<&shadow_contracts::kie::KieContext>,
) -> Result<(), QBridgeError> {
    if let Some(context) = context {
        context.validate().map_err(QBridgeError::ProcessOutput)?;
        let expected = context.native_anchor.as_ref().map(|anchor| &anchor.predecessor);
        if kie.is_some() || predecessor != expected {
            return Err(QBridgeError::ProcessOutput("IZ_NATIVE_CONTEXT_ROUTE_CONFLICT".into()));
        }
    }
    Ok(())
}

fn append_iz_context(
    payload: &mut String,
    context: Option<&shadow_contracts::pd_memory::iz::IzContext>,
) -> Result<(), QBridgeError> {
    let Some(context) = context else {
        return Ok(());
    };
    let bytes = shadow_contracts::pd_memory::iz::encode_context(context)
        .map_err(QBridgeError::ProcessOutput)?;
    let size = bytes
        .len()
        .checked_mul(2)
        .and_then(|n| n.checked_add(payload.len() + 12))
        .ok_or(QBridgeError::PayloadTooLarge(usize::MAX))?;
    if size > 64 * 1024 {
        return Err(QBridgeError::PayloadTooLarge(size));
    }
    payload.push_str(";iz_context=");
    use std::fmt::Write as _;
    for byte in bytes {
        write!(payload, "{byte:02x}").map_err(|_| QBridgeError::StringConvert)?;
    }
    Ok(())
}
fn native_clock_millis(now: SystemTime) -> Result<u64, QBridgeError> {
    let elapsed = now
        .duration_since(UNIX_EPOCH)
        .map_err(|_| QBridgeError::ProcessOutput("LIGHT_PD_CLOCK_BEFORE_UNIX_EPOCH".into()))?;
    u64::try_from(elapsed.as_millis())
        .map_err(|_| QBridgeError::ProcessOutput("LIGHT_PD_CLOCK_MILLIS_OVERFLOW".into()))
}
fn append_native_clock_ms(payload: &mut String, millis: u64) -> Result<(), QBridgeError> {
    use std::fmt::Write as _;
    write!(payload, ";pd_clock_ms={millis}").map_err(|_| QBridgeError::StringConvert)?;
    if payload.len() > 64 * 1024 {
        return Err(QBridgeError::PayloadTooLarge(payload.len()));
    }
    Ok(())
}
// This is the native-process bound; the existing 2048-byte C bus/base-input
// contract above is unchanged. Hex context contains no wire delimiters.
fn append_native_context(
    payload: &mut String,
    predecessor: Option<&shadow_contracts::pd_recovery::RecoveryLookup>,
    kie_context: Option<&shadow_contracts::kie::KieContext>,
) -> Result<(), QBridgeError> {
    const NATIVE_INPUT_MAX: usize = 64 * 1024;
    if let Some(parent) = predecessor {
        let reference = shadow_contracts::pd_followup::encode_reference(parent)
            .map_err(QBridgeError::ProcessOutput)?;
        payload.push_str(";pd_predecessor=");
        payload.push_str(&reference);
    }
    if let Some(context) = kie_context {
        let bytes =
            shadow_contracts::kie::encode_context(context).map_err(QBridgeError::ProcessOutput)?;
        let size = bytes
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(payload.len() + 13))
            .ok_or(QBridgeError::PayloadTooLarge(usize::MAX))?;
        if size > NATIVE_INPUT_MAX {
            return Err(QBridgeError::PayloadTooLarge(size));
        }
        payload.push_str(";kie_context=");
        use std::fmt::Write as _;
        for byte in bytes {
            write!(payload, "{byte:02x}").map_err(|_| QBridgeError::StringConvert)?;
        }
    }
    if payload.len() > NATIVE_INPUT_MAX {
        return Err(QBridgeError::PayloadTooLarge(payload.len()));
    }
    Ok(())
}

#[cfg(test)]
mod quantum_response_contract_tests {
    use super::*;
    #[cfg(unix)]
    use std::sync::Mutex;

    #[cfg(unix)]
    static PROCESS_ENV_LOCK: Mutex<()> = Mutex::new(());

    fn payload(trace_id: &str, state: &str, ts: u64) -> String {
        format!(
            "trace_id={trace_id};verdict=OPTIMAL;pot=0.9000;axis=12.0;\
             seal=QNT:{trace_id}:0123456789abcdef;sfx=VFYKNG;state={state};\
             territory=SEMANTIC;depth=DEEP;ts={ts};content=shadow verified"
        )
    }

    // Envelope-only storage fixture. This is deliberately not a late refusal
    // or a semantic admission; production must reject it under that marker.
    fn operational_data_fixture() -> shadow_contracts::pd_memory::PdOperationRecord {
        use sha2::{Digest,Sha256};
        use shadow_contracts::pd_memory as pd;
        let identity=pd::PdIdentity { owner:"test".into(),obligation:"binding".into(),
            case_id:"case".into(),scope:"scope".into(),domain:"scope".into(),lineage_root:"root".into() };
        let source=pd::encode_continuity_input(&pd::ContinuityInput {identity:identity.clone(),
            operation_id:"operation".into(),user_text:"actual harmless input".into(),science_query:vec![1]}).unwrap();
        let digest=Sha256::digest(&source).into();
        let step=pd::LgcStep::new(0,digest,"fixture","observation",[2;32]).unwrap();
        pd::PdOperationRecord::construct(pd::PdOperationRequest {
            identity,operation_id:"operation".into(),source_input:source,expected_revision:0,previous_state:[0;32],
            witness:pd::ConditionWitness {predecessor:"fixture".into(),parent_authority:"none".into(),
                failed_action:"unavailable".into(),condition_code:"FIXTURE".into(),reason:"binding only".into(),
                source_component:"test".into(),source_sha256:[1;32],attempt_trace:"trace".into(),
                input_sha256:digest,lgc_parent:step.step_digest,report_class:pd::ReportClass::MissingConditions,
                permitted_next:pd::NextAction::AcquireScopedEvidence},
            checkpoint:vec![1],scoped_question:"Which scoped observation?".into(),candidate:vec![],candidate_key:[0;32],lgc_steps:vec![step],
        },[0;32]).unwrap()
    }

    #[test]
    fn r25_late_marker_cannot_mix_native_authority_or_untyped_operational_data() {
        let native=payload("T3_anchor","SEALED",42);
        for text in [
            "[PD_LATE_REFUSAL] 00\n[PD_LATE_REFUSAL] 00".to_string(),
            format!("[PD_LATE_REFUSAL] 00\n[Q→LIGHT]  {native}"),
            "[PD_LATE_REFUSAL] 00\n[PD_NATIVE_REFERENCE] anything".to_string(),
        ] {
            assert!(matches!(parse_process_output(&text),Err(QBridgeError::ProcessOutput(reason)) if reason=="PD_LATE_AMBIGUOUS_TERMINAL"));
        }
        use shadow_contracts::pd_memory as pd;
        let record=operational_data_fixture();
        let encoded=pd::encode_record(&record).unwrap().iter().map(|b|format!("{b:02x}")).collect::<String>();
        assert!(parse_process_output(&format!("[PD_LATE_REFUSAL] {encoded}")).is_err(),"an ordinary PDR1 is not a late owner-witnessed refusal");
        for malformed in ["0","GG","AB","","00"] {
            assert!(parse_process_output(&format!("[PD_LATE_REFUSAL] {malformed}")).is_err());
        }
        assert!(matches!(parse_process_output(&format!("[Q→LIGHT]  {native}")),Ok(QuantumCycleResult::Native(_))));
    }

    #[test]
    fn r25_owner_resolution_requires_exact_operation_and_full_record() {
        use shadow_contracts::pd_memory as pd;
        let record=operational_data_fixture();
        bind_late_owner(&record,|bytes| {
            let lookup=pd::decode_lookup(bytes)?;
            assert_eq!(lookup.identity,record.request.identity);
            assert_eq!(lookup.operation_id.as_deref(),Some("operation"));
            pd::encode_optional_record(Some(&record))
        }).unwrap();
        assert_eq!(bind_late_owner(&record,|_|pd::encode_optional_record(None)).unwrap_err(),"LIGHT_PD_LATE_DURABLE_OPERATION_MISSING");
        let mut changed_request=record.request.clone();
        changed_request.scoped_question="Different stored question?".into();
        let changed=pd::PdOperationRecord::construct(changed_request,[0;32]).unwrap();
        assert_eq!(bind_late_owner(&record,|_|pd::encode_optional_record(Some(&changed))).unwrap_err(),"LIGHT_PD_LATE_DURABLE_OPERATION_MISMATCH");
    }

    #[test]
    fn iz_context_is_delimiter_safe_exact_and_resource_bounded() {
        use sha2::{Digest, Sha256};
        use shadow_contracts::pd_memory::{self as pd, iz};
        let origin = "motor;injected_key=not_a_wire_key".to_string();
        let context = iz::IzContext {
            version: 1,
            identity: pd::PdIdentity {
                owner: "owner".into(),
                obligation: "question".into(),
                case_id: "case".into(),
                scope: "WORK→NURA".into(),
                domain: "WORK→NURA".into(),
                lineage_root: pd::hex(&Sha256::digest(origin.as_bytes()).into()),
            },
            operation_id: "initial".into(),
            origin_i0: origin.clone(),
            user_text: origin,
            parent: None,
            answer: iz::IzAnswer::default(),
            map: None,
            native_anchor: None,
            native_map: None,
        };
        let mut base = "text=motor".to_string();
        append_iz_context(&mut base, None).unwrap();
        assert_eq!(base, "text=motor");
        append_iz_context(&mut base, Some(&context)).unwrap();
        let encoded = base.strip_prefix("text=motor;iz_context=").unwrap();
        assert!(encoded
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
        let decoded = encoded
            .as_bytes()
            .chunks_exact(2)
            .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(iz::decode_context(&decoded).unwrap(), context);
        let mut full = "x".repeat(64 * 1024);
        let original = full.clone();
        assert!(append_iz_context(&mut full, Some(&context)).is_err());
        assert_eq!(full, original);
        validate_iz_native_route(Some(&context), None, None).unwrap();
        let predecessor = shadow_contracts::pd_recovery::RecoveryLookup { identity: [4;32], request_sha256: [5;32] };
        assert!(validate_iz_native_route(Some(&context), Some(&predecessor), None).is_err());
        let mut native = context.clone();
        native.version = iz::IZ_NATIVE_CONTINUITY_VERSION;
        native.native_anchor = Some(shadow_contracts::pd_prepared::PdContinuationLink {
            cognitive_session_id: pd::hex(&native.identity.key().unwrap()), territory: native.identity.scope.clone(),
            predecessor: predecessor.clone(), predecessor_transaction_id: [6;32], predecessor_continuation_sha256: [7;32],
            predecessor_next_i0: "next-source-id".into(), clock_millis: true,
        });
        validate_iz_native_route(Some(&native), Some(&predecessor), None).unwrap();
        assert!(validate_iz_native_route(Some(&native), None, None).is_err());
        let mut changed = predecessor.clone(); changed.request_sha256 = [8;32];
        assert!(validate_iz_native_route(Some(&native), Some(&changed), None).is_err());
        let kie = shadow_contracts::kie::KieContext { identity: native.identity.clone(), operation_id: "science".into(), record_id: [9;32] };
        assert!(validate_iz_native_route(Some(&native), Some(&predecessor), Some(&kie)).is_err());
        let mut wire = String::from("text=motor");
        append_native_context(&mut wire, Some(&predecessor), None).unwrap();
        append_iz_context(&mut wire, Some(&native)).unwrap();
        assert!(wire.contains(";pd_predecessor="));
        assert!(wire.contains(";iz_context=49435833"));
    }
    struct DiagnosticSink {
        bytes: Vec<u8>,
        chunk: usize,
        fail_after: Option<usize>,
        fail_flush: bool,
        flushes: usize,
    }

    impl std::io::Write for DiagnosticSink {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            let remaining = self.fail_after
                .map(|limit| limit.saturating_sub(self.bytes.len()))
                .unwrap_or(usize::MAX);
            if remaining == 0 {
                return Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "diagnostic sink"));
            }
            let take = bytes.len().min(self.chunk).min(remaining);
            self.bytes.extend_from_slice(&bytes[..take]);
            Ok(take)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.flushes += 1;
            if self.fail_flush {
                Err(std::io::Error::new(std::io::ErrorKind::Other, "diagnostic flush"))
            } else {
                Ok(())
            }
        }
    }

    fn diagnostic_sink(fail_after: Option<usize>, fail_flush: bool) -> DiagnosticSink {
        DiagnosticSink { bytes: Vec::new(), chunk: 3, fail_after, fail_flush, flushes: 0 }
    }

    #[test]
    fn r38_success_diagnostics_preserve_exact_bytes_and_native_result() {
        let response = payload("T3_anchor", "SEALED", 42);
        let stdout = format!("[Q→LIGHT]  {response}\n").into_bytes();
        let stderr = b"[HCP_GATE]\r\n\0utf8=\xc3\xab invalid=\xff\xfe\n[Q\xe2\x86\x92LIGHT]  not-an-envelope\n";
        let mut diagnostics = Vec::new();
        let mut fallback = Vec::new();
        let result = parse_successful_process_output(stdout, stderr, &mut diagnostics, &mut fallback)
            .expect("one original native response");
        assert_eq!(diagnostics, stderr);
        assert!(fallback.is_empty());
        assert!(matches!(result, QuantumCycleResult::Native(value) if value.is_bound_to("T3_anchor") && value.ts == 42));
    }

    #[test]
    fn r38_success_diagnostics_handle_partial_writes_and_flush() {
        let bytes = b"\0\xffpartial\r\n\xc3\xab";
        let mut sink = diagnostic_sink(None, false);
        forward_success_diagnostics(bytes, &mut sink).unwrap();
        assert_eq!(sink.bytes, bytes);
        assert_eq!(sink.flushes, 1);

        let mut failed = diagnostic_sink(Some(7), false);
        let error = forward_success_diagnostics(bytes, &mut failed).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);
        assert_eq!(failed.bytes, bytes[..7]);
        assert_eq!(failed.flushes, 0);

        let mut flush_failed = diagnostic_sink(None, true);
        assert!(forward_success_diagnostics(bytes, &mut flush_failed).is_err());
        assert_eq!(flush_failed.bytes, bytes);
        assert_eq!(flush_failed.flushes, 1);
    }

    #[test]
    fn r38_diagnostic_failure_preserves_native_result_and_protocol_error() {
        let response = payload("T3_anchor", "SEALED", 42);
        let stderr = b"actual diagnostic";
        let mut sink = diagnostic_sink(Some(4), false);
        let mut fallback = Vec::new();
        let result = parse_successful_process_output(
            format!("[Q→LIGHT]  {response}\n").into_bytes(),
            stderr,
            &mut sink,
            &mut fallback,
        ).expect("log failure does not veto native result");
        assert!(matches!(result, QuantumCycleResult::Native(value) if value.is_bound_to("T3_anchor")));
        assert_eq!(sink.bytes, stderr[..4]);
        let marker = String::from_utf8(fallback).unwrap();
        assert!(marker.starts_with("[QUANTUM_DIAGNOSTIC_WRITE_FAILED] "));
        assert!(marker.contains("commit_status=unchanged no_retry=true"));
        assert!(!marker.contains("[Q→LIGHT]") && !marker.contains("[PD_NATIVE_REFERENCE]"));

        let mut sink = diagnostic_sink(Some(0), false);
        let mut fallback = diagnostic_sink(Some(0), false);
        let result = parse_successful_process_output(
            b"missing protocol response".to_vec(), stderr, &mut sink, &mut fallback,
        );
        assert!(matches!(result, Err(QBridgeError::ProcessOutput(_))));
        let result = parse_successful_process_output(
            format!("[Q→LIGHT]  {response}\n").into_bytes(), stderr, &mut sink, &mut fallback,
        );
        assert!(matches!(result, Ok(QuantumCycleResult::Native(value)) if value.is_bound_to("T3_anchor")));
    }

    #[test]
    fn native_clock_unit_is_explicit_and_failure_never_becomes_zero() {
        assert_eq!(
            native_clock_millis(UNIX_EPOCH + std::time::Duration::from_millis(1234)).unwrap(),
            1234
        );
        assert!(native_clock_millis(UNIX_EPOCH - std::time::Duration::from_secs(1)).is_err());
        let mut payload = "ts=1;text=scope".to_string();
        append_native_clock_ms(&mut payload, 1234).unwrap();
        assert_eq!(payload, "ts=1;text=scope;pd_clock_ms=1234");
        assert!(append_native_clock_ms(&mut "x".repeat(64 * 1024), 1).is_err());
    }
    #[test]
    fn native_context_wire_is_exact_lowerhex_and_default_is_unchanged() {
        let context = shadow_contracts::kie::KieContext {
            identity: shadow_contracts::pd_memory::PdIdentity {
                owner: "operator".into(),
                obligation: "model predicate".into(),
                case_id: "case-a".into(),
                scope: "WORK→NURA".into(),
                domain: "WORK→NURA".into(),
                lineage_root: "source-a".into(),
            },
            operation_id: "science-op".into(),
            record_id: [7; 32],
        };
        let mut payload = "text=canonical-model".to_string();
        append_native_context(&mut payload, None, None).unwrap();
        assert_eq!(payload, "text=canonical-model");
        let parent = shadow_contracts::pd_recovery::RecoveryLookup {
            identity: [1; 32],
            request_sha256: [2; 32],
        };
        append_native_context(&mut payload, Some(&parent), Some(&context)).unwrap();
        let expected_parent = shadow_contracts::pd_followup::encode_reference(&parent).unwrap();
        assert!(payload.contains(&format!(";pd_predecessor={expected_parent};kie_context=")));
        let encoded = payload.split(";kie_context=").nth(1).unwrap();
        assert!(encoded
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
        let decoded = encoded
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            shadow_contracts::kie::decode_context(&decoded).unwrap(),
            context
        );
        let mut invalid = context.clone();
        invalid.record_id = [0; 32];
        assert!(append_native_context(&mut String::new(), None, Some(&invalid)).is_err());
        assert!(append_native_context(&mut "x".repeat(64 * 1024 + 1), None, None).is_err());
    }
    #[test]
    fn response_is_bound_to_the_requested_i0() {
        let result = QuantumResult::from_payload(&payload("T3_anchor", "SEALED", 42))
            .expect("canonical Quantum response");
        assert!(result.is_bound_to("T3_anchor"));
        assert!(!result.is_bound_to("T3_other"));
        assert_eq!(result.ts, 42);
    }

    #[test]
    fn response_rejects_state_or_timestamp_defaults() {
        assert!(QuantumResult::from_payload(&payload("T3_anchor", "REJECTED", 42)).is_err());
        assert!(QuantumResult::from_payload(&payload("T3_anchor", "SEALED", 0)).is_err());
    }

    #[test]
    fn response_rejects_a_seal_for_another_trace() {
        let tampered =
            payload("T3_anchor", "SEALED", 42).replace("QNT:T3_anchor:", "QNT:T3_other:");
        assert!(QuantumResult::from_payload(&tampered).is_err());
    }

    #[cfg(unix)]
    struct QuantumScript {
        path: std::path::PathBuf,
        previous: Option<std::ffi::OsString>,
    }

    #[cfg(unix)]
    impl QuantumScript {
        fn install(body: &str) -> Self {
            use std::os::unix::fs::PermissionsExt as _;
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "essmai-qbridge-test-{}-{unique}.sh",
                std::process::id(),
            ));
            std::fs::write(&path, body).expect("write Quantum test process");
            let mut permissions = std::fs::metadata(&path)
                .expect("Quantum test process metadata")
                .permissions();
            permissions.set_mode(0o700);
            std::fs::set_permissions(&path, permissions)
                .expect("mark Quantum test process executable");
            let previous = std::env::var_os("ESSMAI_QUANTUM_BIN");
            std::env::set_var("ESSMAI_QUANTUM_BIN", &path);
            Self { path, previous }
        }
    }

    #[cfg(unix)]
    impl Drop for QuantumScript {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(value) => std::env::set_var("ESSMAI_QUANTUM_BIN", value),
                None => std::env::remove_var("ESSMAI_QUANTUM_BIN"),
            }
            let _ = std::fs::remove_file(&self.path);
        }
    }

    #[cfg(unix)]
    #[test]
    fn production_process_bridge_accepts_one_envelope_and_fails_closed() {
        let _lock = PROCESS_ENV_LOCK.lock().expect("Quantum process env lock");
        let response = payload("T3_anchor", "SEALED", 42);

        {
            let script = format!(
                "#!/bin/sh\nIFS= read -r _line\nprintf '%s\\n' '  [Q→LIGHT]  {response}'\n"
            );
            let _installed = QuantumScript::install(&script);
            let result =
                invoke_quantum_process("trace_id=T3_anchor", 2).expect("one process envelope");
            assert!(matches!(result,QuantumCycleResult::Native(value) if value.is_bound_to("T3_anchor")));
        }

        {
            let script = format!(
                "#!/bin/sh\nIFS= read -r _line\nprintf '%s\\n' '  [Q→LIGHT]  {response}'\nprintf '%s\\n' '  [Q→LIGHT]  {response}'\n"
            );
            let _installed = QuantumScript::install(&script);
            assert!(matches!(
                invoke_quantum_process("trace_id=T3_anchor", 2),
                Err(QBridgeError::ProcessOutput(_))
            ));
        }

        {
            let _installed = QuantumScript::install("#!/bin/sh\nexit 7\n");
            assert!(matches!(
                invoke_quantum_process("trace_id=T3_anchor", 2),
                Err(QBridgeError::ProcessExit(_))
            ));
        }

        {
            let _installed = QuantumScript::install("#!/bin/sh\nsleep 2\n");
            assert!(matches!(
                invoke_quantum_process("trace_id=T3_anchor", 0),
                Err(QBridgeError::Timeout(_))
            ));
        }
    }
}
