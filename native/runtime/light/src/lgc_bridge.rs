// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  LGC_BRIDGE.RS — URA RUST → C KERNEL                                  ║
// ║  GJATA LEGACY™  — AIGovern-Cyber                                              ║
// ║                                                                          ║
// ║  Rust zëvendëson Python/ctypes plotësisht.                            ║
// ║  extern "C" = zero-cost, zero-marshal, zero-overhead.                  ║
// ║                                                                          ║
// ║  HIERARKIA:                                                              ║
// ║  ALLAH → BLEDAR GJATA → GJATA LEGACY →                                ║
// ║  SHADOW_GJ_LEGACY → NURA → ALISAH → AMMY                             ║
// ╚══════════════════════════════════════════════════════════════════════════╝

#[cfg(any(feature = "dev_simulation", test))]
use std::sync::atomic::{AtomicBool, Ordering};

// ─────────────────────────────────────────────────────────────────────────────
// FFI — Pasqyrë e shadow_gj_legacy.h (zero-cost, direkt)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(any(feature = "c_kernel", test))]
const LGC_ID_LEN: usize = 48;
const LGC_MOD_NAME: usize = 64;
#[cfg(any(feature = "c_kernel", test))]
const LGC_REASON_LEN: usize = 128;

#[cfg(any(feature = "c_kernel", test))]
const LGC_DECISION_BLOCK: i32 = 0;
#[cfg(any(feature = "c_kernel", test))]
const LGC_DECISION_PASS: i32 = 1;
#[cfg(any(feature = "c_kernel", test))]
const LGC_STATE_LIGHT: i32 = 0;
#[cfg(any(feature = "c_kernel", test))]
const LGC_STATE_PRESSURED: i32 = 1;
#[cfg(any(feature = "c_kernel", test))]
const LGC_STATE_MAX: i32 = 4;

/// Vula e vetme e njohur nga `shadow_gj_legacy.c`.
/// `(500 & 0xFFFF) ^ 0xA5A5 == 0xA451`.
pub const LGC_SEAL_PRIMITIVE_FLAGS: u32 = 0xA451;
#[cfg(any(feature = "dev_simulation", test))]
static RUST_SIMULATION_FROZEN: AtomicBool = AtomicBool::new(false);
#[cfg(test)]
static PHASE1_BRIDGE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
pub(crate) fn phase1_test_lock() -> std::sync::MutexGuard<'static, ()> {
    PHASE1_BRIDGE_TEST_LOCK
        .lock()
        .expect("phase-1 LGC test lock")
}

#[cfg(any(feature = "c_kernel", test))]
#[repr(C)]
pub struct LgcRequest {
    pub req_id: [u8; LGC_ID_LEN],
    pub caller: [u8; LGC_MOD_NAME],
    pub sha256: [u8; 65],
    pub ram_usage: f32,
    pub flags: u32,
    pub ts: u64,
}

#[cfg(any(feature = "c_kernel", test))]
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LgcResult {
    pub req_id: [u8; LGC_ID_LEN],
    // FFI shkruan enum C me madhësi `int`. Fusha pranohet fillimisht si i32
    // që një discriminant i korruptuar të mos krijojë enum Rust të pavlefshëm.
    pub decision: i32,
    pub state: i32,
    pub sha256: [u8; 65],
    pub reason: [u8; LGC_REASON_LEN],
    pub ts: u64,
}



// ─────────────────────────────────────────────────────────────────────────────
// EXTERN "C" — Lidhja direkte me shadow_gj_legacy.c
// Kompilo: gcc -shared -fPIC -o libnura_kernel.so shadow_gj_legacy.c buss_legacy.c
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "c_kernel")]
#[link(name = "light_kernel")]
extern "C" {
    pub fn lgc_init() -> i32;
    pub fn lgc_check(req: *const LgcRequest, res: *mut LgcResult) -> i32;


    pub fn lgc_freeze(stage: i32) -> i32;
    pub fn lgc_unfreeze() -> i32;
}

// ─────────────────────────────────────────────────────────────────────────────
// SHA-256 autoritativ në Rust.
// Kerneli C nuk ekspozon SHA-256 prodhues; porta nuk pretendon barazi me stub-in.
// ─────────────────────────────────────────────────────────────────────────────





// ─────────────────────────────────────────────────────────────────────────────
// LGC BRIDGE — Wrapper Rust i sigurt
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BridgeDecision {
    pub pass: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BridgeBackend {
    #[cfg(feature = "c_kernel")]
    CKernel,
    #[cfg(any(feature = "dev_simulation", test))]
    RustSimulation,
    Unavailable,
}

pub struct LgcBridge {
    backend: BridgeBackend,
}

impl LgcBridge {
    /// Inicializimi i vetëm i portës LGC.
    ///
    /// Prodhimi (`runtime_mode`) nuk fiton kurrë një fallback simbolik:
    /// pa `c_kernel`, backend-i mbetet `Unavailable` dhe boot-i ndalet.
    /// Mirror-i Rust ekziston vetëm nën `dev_simulation` eksplicit.
    pub fn new() -> Self {
        #[cfg(feature = "c_kernel")]
        {
            let return_code = unsafe { lgc_init() };
            match return_code == 0 {
                true => {
                    tracing::info!("LGC_BRIDGE: C kernel AKTIV — libnura_kernel.so");
                    return Self {
                        backend: BridgeBackend::CKernel,
                    };
                }
                false => {
                    tracing::error!(
                        "LGC_BRIDGE: lgc_init dështoi (rc={}) — FAIL-CLOSED",
                        return_code
                    );
                    return Self {
                        backend: BridgeBackend::Unavailable,
                    };
                }
            }
        }

        #[cfg(all(not(feature = "c_kernel"), feature = "dev_simulation"))]
        {
            tracing::warn!("LGC_BRIDGE: mirror Rust vetëm për dev_simulation");
            RUST_SIMULATION_FROZEN.store(false, Ordering::Release);
            return Self {
                backend: BridgeBackend::RustSimulation,
            };
        }

        #[cfg(all(not(feature = "c_kernel"), not(feature = "dev_simulation")))]
        {
            tracing::error!(
                "LGC_BRIDGE: runtime prodhues pa c_kernel — FAIL-CLOSED"
            );
            Self {
                backend: BridgeBackend::Unavailable,
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn rust_simulation_for_test() -> Self {
        RUST_SIMULATION_FROZEN.store(false, Ordering::Release);
        Self {
            backend: BridgeBackend::RustSimulation,
        }
    }

    #[cfg(test)]
    pub(crate) fn unavailable_for_test() -> Self {
        Self {
            backend: BridgeBackend::Unavailable,
        }
    }

    pub fn backend_label(&self) -> &'static str {
        match self.backend {
            #[cfg(feature = "c_kernel")]
            BridgeBackend::CKernel => "C_KERNEL",
            #[cfg(any(feature = "dev_simulation", test))]
            BridgeBackend::RustSimulation => "RUST_DEV_SIMULATION",
            BridgeBackend::Unavailable => "UNAVAILABLE",
        }
    }





    /// Kontrata e vetme C/Rust: preflight-i është identik; vetëm autoriteti
    /// i ekzekutimit ndryshon sipas feature-it të deklaruar.
    pub fn check_with_flags(
        &self,
        caller: &str,
        sha256: &str,
        ram_usage: f32,
        flags: u32, // seal i fshehur brenda flags
    ) -> BridgeDecision {
        match preflight(caller, sha256, ram_usage) {
            Ok(()) => {}
            Err(reason) => return blocked("PREFLIGHT_BLOCK", reason),
        }

        match self.backend {
            #[cfg(feature = "c_kernel")]
            BridgeBackend::CKernel => self.check_c_with_flags(caller, sha256, ram_usage, flags),
            #[cfg(any(feature = "dev_simulation", test))]
            BridgeBackend::RustSimulation => {
                self.check_rust_with_flags(flags)
            }
            BridgeBackend::Unavailable => blocked(
                "BACKEND_UNAVAILABLE",
                &format!("runtime_mode kërkon c_kernel të inicializuar:check_flags={flags:#010x}"),
            ),
        }
    }

    #[cfg(any(feature = "dev_simulation", test))]
    fn check_rust_with_flags(&self, flags: u32) -> BridgeDecision {
        let frozen = RUST_SIMULATION_FROZEN.load(Ordering::Acquire);
        let sealed = flags != 0 && crate::legalgocrypt::unseal_from_flags(flags) == 500;
        let pass = !frozen && sealed;
        let reason = match (frozen, sealed) {
            (true, _) => "LGC_FROZEN",
            (false, false) => "SEAL_STRIPPED",
            (false, true) => "SEAL_VERIFIED",
        };
        BridgeDecision {
            pass,
            reason: reason.to_string(),
        }
    }

    #[cfg(feature = "c_kernel")]
    fn check_c_with_flags(
        &self,
        caller: &str,
        sha256: &str,
        ram_usage: f32,
        flags: u32,
    ) -> BridgeDecision {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut req = LgcRequest {
            req_id: [0u8; 48],
            caller: [0u8; 64],
            sha256: [0u8; 65],
            ram_usage,
            flags, // ← 500 fshehur brenda, bus kalon të paprekur
            ts,
        };

        let cal = caller.as_bytes();
        req.caller[..cal.len().min(63)].copy_from_slice(&cal[..cal.len().min(63)]);
        let sha = sha256.as_bytes();
        req.sha256[..sha.len().min(64)].copy_from_slice(&sha[..sha.len().min(64)]);
        req.req_id.copy_from_slice(&sha[..LGC_ID_LEN]);

        let mut res = LgcResult {
            req_id: [0u8; 48],
            decision: LGC_DECISION_BLOCK,
            state: LGC_STATE_LIGHT,
            sha256: [0u8; 65],
            reason: [0u8; 128],
            ts: 0,
        };

        // Rezultati lexohet vetëm pasi return-code dhe çdo fushë ABI kalojnë.
        let return_code = unsafe { lgc_check(&req, &mut res) };
        decode_c_result(return_code, &res, sha256, ram_usage)
    }

    pub fn freeze(&self, stage: i32) -> Result<(), String> {
        match self.backend {
            #[cfg(feature = "c_kernel")]
            BridgeBackend::CKernel => ffi_status("lgc_freeze", unsafe { lgc_freeze(stage) }),
            #[cfg(any(feature = "dev_simulation", test))]
            BridgeBackend::RustSimulation => {
                RUST_SIMULATION_FROZEN.store(true, Ordering::Release);
                Ok(())
            }
            BridgeBackend::Unavailable => Err(format!("LGC_BACKEND_UNAVAILABLE:freeze_stage={stage}")),
        }
    }

    pub fn unfreeze(&self) -> Result<(), String> {
        match self.backend {
            #[cfg(feature = "c_kernel")]
            BridgeBackend::CKernel => ffi_status("lgc_unfreeze", unsafe { lgc_unfreeze() }),
            #[cfg(any(feature = "dev_simulation", test))]
            BridgeBackend::RustSimulation => {
                RUST_SIMULATION_FROZEN.store(false, Ordering::Release);
                Ok(())
            }
            BridgeBackend::Unavailable => Err("LGC_BACKEND_UNAVAILABLE".to_string()),
        }
    }
}

fn preflight(caller: &str, sha256: &str, ram_usage: f32) -> Result<(), &'static str> {
    match (
        caller.is_empty(),
        caller.len() > LGC_MOD_NAME - 1,
        caller.as_bytes().contains(&0),
        sha256.len() == 64
            && sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        ram_usage.is_finite() && (0.0..=0.90).contains(&ram_usage),
    ) {
        (true, _, _, _, _) => Err("CALLER_EMPTY"),
        (_, true, _, _, _) => Err("CALLER_TOO_LONG"),
        (_, _, true, _, _) => Err("CALLER_CONTAINS_NUL"),
        (_, _, _, false, _) => Err("SHA256_NOT_CANONICAL"),
        (_, _, _, _, false) => Err("RAM_USAGE_OUT_OF_RANGE"),
        _ => Ok(()),
    }
}

fn blocked(category: &str, reason: &str) -> BridgeDecision {
    BridgeDecision {
        pass: false,
        reason: format!("{category}:{reason}"),
    }
}

#[cfg(feature = "c_kernel")]
fn ffi_status(operation: &str, return_code: i32) -> Result<(), String> {
    match return_code {
        0 => Ok(()),
        code => Err(format!("{operation}_FFI_ERROR:{code}")),
    }
}

#[cfg(any(feature = "c_kernel", test))]
fn decode_c_result(
    return_code: i32,
    result: &LgcResult,
    expected_sha256: &str,
    expected_ram_usage: f32,
) -> BridgeDecision {
    match return_code {
        0 => {}
        code => {
            return blocked(
                "FFI_ERROR",
                &format!("lgc_check_RETURN_CODE:{code}"),
            )
        }
    }

    match matches!(result.decision, LGC_DECISION_BLOCK | LGC_DECISION_PASS) {
        true => {}
        false => {
            return blocked(
                "FFI_INVALID_RESULT",
                "LGC_DECISION_OUT_OF_DOMAIN",
            )
        }
    }
    match (LGC_STATE_LIGHT..=LGC_STATE_MAX).contains(&result.state) {
        true => {}
        false => {
            return blocked(
                "FFI_INVALID_RESULT",
                "LGC_STATE_OUT_OF_DOMAIN",
            )
        }
    }
    let expected_state = match expected_ram_usage > 0.50 {
        true => LGC_STATE_PRESSURED,
        false => LGC_STATE_LIGHT,
    };
    match result.state == expected_state {
        true => {}
        false => {
            return blocked(
                "FFI_INVALID_RESULT",
                "LGC_STATE_SEMANTIC_DRIFT",
            )
        }
    }
    match result.req_id.as_slice() == &expected_sha256.as_bytes()[..LGC_ID_LEN] {
        true => {}
        false => {
            return blocked(
                "FFI_INVALID_RESULT",
                "LGC_REQUEST_ID_ECHO_DRIFT",
            )
        }
    }
    match &result.sha256[..64] == expected_sha256.as_bytes() && result.sha256[64] == 0 {
        true => {}
        false => {
            return blocked(
                "FFI_INVALID_RESULT",
                "LGC_SHA256_ECHO_DRIFT",
            )
        }
    }
    match result.ts > 0 {
        true => {}
        false => {
            return blocked(
                "FFI_INVALID_RESULT",
                "LGC_TIMESTAMP_MISSING",
            )
        }
    }

    let reason_end = match result
        .reason
        .iter()
        .position(|byte| *byte == 0)
    {
        Some(index) => index,
        None => {
            return blocked(
                "FFI_INVALID_RESULT",
                "LGC_REASON_NOT_TERMINATED",
            )
        }
    };
    let reason = match std::str::from_utf8(&result.reason[..reason_end]) {
        Ok(value) => value.to_string(),
        Err(_) => {
            return blocked(
                "FFI_INVALID_RESULT",
                "LGC_REASON_INVALID_UTF8",
            )
        }
    };

    BridgeDecision {
        pass: result.decision == LGC_DECISION_PASS,
        reason,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// LIGHT BUSS — FFI structs
// Layout identik me light_buss.h (padding explicit — ABI e saktë)
// ─────────────────────────────────────────────────────────────────────────────























// ─────────────────────────────────────────────────────────────────────────────
// LIGHT BUSS — extern "C" FFI
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "c_kernel")]
#[link(name = "light_kernel")]
extern "C" {











}

// ─────────────────────────────────────────────────────────────────────────────
// LIGHT BUSS — wrapper Rust i sigurt
// Roli: BUS — kalon mesazhet, nuk interpreton, nuk vendos
// ─────────────────────────────────────────────────────────────────────────────











#[cfg(test)]
mod phase1_boot_contract_tests {
    use super::*;

    const SHA: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn raw_result(decision: i32, state: i32) -> LgcResult {
        let mut result = LgcResult {
            req_id: [0u8; LGC_ID_LEN],
            decision,
            state,
            sha256: [0u8; 65],
            reason: [0u8; LGC_REASON_LEN],
            ts: 1,
        };
        result.sha256[..64].copy_from_slice(SHA.as_bytes());
        result.req_id.copy_from_slice(&SHA.as_bytes()[..LGC_ID_LEN]);
        result
    }

    #[test]
    fn rust_mirror_has_the_same_pass_block_and_freeze_vectors() {
        let _guard = phase1_test_lock();
        let bridge = LgcBridge::rust_simulation_for_test();
        assert!(
            bridge
                .check_with_flags("BOOT", SHA, 0.1, LGC_SEAL_PRIMITIVE_FLAGS)
                .pass
        );
        assert!(!bridge.check_with_flags("BOOT", SHA, 0.1, 0).pass);

        bridge.freeze(7).expect("simulation freeze");
        assert!(
            !bridge
                .check_with_flags("BOOT", SHA, 0.1, LGC_SEAL_PRIMITIVE_FLAGS)
                .pass
        );
        bridge.unfreeze().expect("simulation unfreeze");
        assert!(
            bridge
                .check_with_flags("BOOT", SHA, 0.1, LGC_SEAL_PRIMITIVE_FLAGS)
                .pass
        );
    }

    #[test]
    fn common_preflight_is_fail_closed_for_both_backends() {
        let _guard = phase1_test_lock();
        let bridge = LgcBridge::rust_simulation_for_test();
        for (caller, sha, ram) in [
            ("", SHA, 0.1),
            (
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                SHA,
                0.1,
            ),
            ("BO\0OT", SHA, 0.1),
            ("BOOT", "ABCDEF", 0.1),
            (
                "BOOT",
                "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
                0.1,
            ),
            ("BOOT", SHA, -0.1),
            ("BOOT", SHA, 0.91),
            ("BOOT", SHA, f32::NAN),
        ] {
            let decision =
                bridge.check_with_flags(caller, sha, ram, LGC_SEAL_PRIMITIVE_FLAGS);
            assert!(!decision.pass, "{caller:?} {sha:?} {ram:?}");
            assert!(decision.reason.starts_with("PREFLIGHT_BLOCK:"));
        }
    }

    #[test]
    fn ffi_return_code_and_raw_result_are_validated_before_acceptance() {
        let valid = raw_result(LGC_DECISION_PASS, LGC_STATE_LIGHT);
        assert!(decode_c_result(0, &valid, SHA, 0.1).pass);
        let pressured = raw_result(LGC_DECISION_PASS, LGC_STATE_PRESSURED);
        assert!(decode_c_result(0, &pressured, SHA, 0.6).pass);

        let ffi_error = decode_c_result(-7, &valid, SHA, 0.1);
        assert!(!ffi_error.pass);
        assert_eq!(ffi_error.reason, "FFI_ERROR:lgc_check_RETURN_CODE:-7");

        let invalid_decision = raw_result(99, LGC_STATE_LIGHT);
        assert!(!decode_c_result(0, &invalid_decision, SHA, 0.1).pass);

        let invalid_state = raw_result(LGC_DECISION_PASS, 99);
        assert!(!decode_c_result(0, &invalid_state, SHA, 0.1).pass);

        let semantic_state_drift = raw_result(LGC_DECISION_PASS, LGC_STATE_PRESSURED);
        assert!(!decode_c_result(0, &semantic_state_drift, SHA, 0.1).pass);

        let mut drifted_request_id = raw_result(LGC_DECISION_PASS, LGC_STATE_LIGHT);
        drifted_request_id.req_id[0] ^= 1;
        assert!(!decode_c_result(0, &drifted_request_id, SHA, 0.1).pass);

        let mut drifted_echo = raw_result(LGC_DECISION_PASS, LGC_STATE_LIGHT);
        drifted_echo.sha256[0] ^= 1;
        let echo_error = decode_c_result(0, &drifted_echo, SHA, 0.1);
        assert!(!echo_error.pass);
        assert_eq!(echo_error.reason, "FFI_INVALID_RESULT:LGC_SHA256_ECHO_DRIFT");

        let mut missing_timestamp = raw_result(LGC_DECISION_PASS, LGC_STATE_LIGHT);
        missing_timestamp.ts = 0;
        assert!(!decode_c_result(0, &missing_timestamp, SHA, 0.1).pass);

        let mut unterminated_reason = raw_result(LGC_DECISION_PASS, LGC_STATE_LIGHT);
        unterminated_reason.reason.fill(b'x');
        assert!(!decode_c_result(0, &unterminated_reason, SHA, 0.1).pass);

        let mut invalid_utf8_reason = raw_result(LGC_DECISION_PASS, LGC_STATE_LIGHT);
        invalid_utf8_reason.reason[0] = 0xff;
        invalid_utf8_reason.reason[1] = 0;
        assert!(!decode_c_result(0, &invalid_utf8_reason, SHA, 0.1).pass);
    }

    #[test]
    fn ffi_layout_matches_the_c_contract() {
        assert_eq!(std::mem::size_of::<LgcRequest>(), 200);
        assert_eq!(std::mem::size_of::<LgcResult>(), 264);
    }

    #[cfg(all(not(feature = "c_kernel"), not(feature = "dev_simulation")))]
    #[test]
    fn runtime_build_without_c_kernel_is_unavailable() {
        let _guard = phase1_test_lock();
        let bridge = LgcBridge::new();
        assert_eq!(bridge.backend_label(), "UNAVAILABLE");
        assert!(
            !bridge
                .check_with_flags("BOOT", SHA, 0.1, LGC_SEAL_PRIMITIVE_FLAGS)
                .pass
        );
        assert_eq!(bridge.freeze(1).unwrap_err(), "LGC_BACKEND_UNAVAILABLE:freeze_stage=1");
        assert!(bridge.unfreeze().is_err());
    }
}
