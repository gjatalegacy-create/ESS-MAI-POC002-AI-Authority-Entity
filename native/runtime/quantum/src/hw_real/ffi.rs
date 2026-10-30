// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  ffi.rs — AIGovern-Cyber Hardware FFI Boundary                               ║
// ║  GJATA LEGACY™ — Quantum Platform                                     ║
// ║                                                                          ║
// ║  RREGULL ABSOLUT: I VETËM FILE me `unsafe` në hardware layer.         ║
// ║  Çdo extern "C" deklaratë është këtu.                                ║
// ║  Safe wrappers validojnë output C para kthimit në Rust.               ║
// ║                                                                          ║
// ║  LGC SEALS: const fn compile-time për të 9 modulet hardware.          ║
// ║  Njësoj si modulet ekzistuese      — zero runtime computation.        ║
// ╚══════════════════════════════════════════════════════════════════════════╝

use std::marker::PhantomData;

// ════════════════════════════════════════════════════════════════════════════
// §1  FNV-1a + LGC SEAL — compile-time (identikë me      ekzistues)
// ════════════════════════════════════════════════════════════════════════════

#[inline(always)]
pub const fn fnv1a_64(s: &[u8]) -> u64 {
    let mut h: u64 = 14695981039346656037;
    let mut i = 0usize;
    while i < s.len() {
        h ^= s[i] as u64;
        h = h.wrapping_mul(1099511628211);
        i += 1;
    }
    h
}

pub const fn hw_seal(m: &[u8]) -> u64 {
    fnv1a_64(m) ^ fnv1a_64(b"GJATA_LEGACY")
}

/// LGC Seals compile-time për të 9 modulet hardware
pub const S_RESOURCE_TERRITORY: u64 = hw_seal(b"RESOURCE_TERRITORY");
pub const S_RESERVE_TERRITORY: u64 = hw_seal(b"RESERVE_TERRITORY");
pub const S_PRESSURE_TERRITORY: u64 = hw_seal(b"PRESSURE_TERRITORY");
pub const S_SCALE_HARDUER: u64 = hw_seal(b"SCALE_HARDUER");
pub const S_ACT_PARALLEL: u64 = hw_seal(b"ACT_PARALLEL_ARGS");
pub const S_QUANTUM_SOFTHARD: u64 = hw_seal(b"QUANTUM_SOFTHARD");
pub const S_SENSOR_MATH: u64 = hw_seal(b"SENSOR_MATH_HARDUER");
pub const S_COLD_HEAT: u64 = hw_seal(b"COLD_HEAT_MANAGER");
pub const S_COLD_DOWN: u64 = hw_seal(b"COLD_DOWN");

// ════════════════════════════════════════════════════════════════════════════
// §2  LGC TOKEN — identik me      (PhantomData !Send !Clone !Copy)
// ════════════════════════════════════════════════════════════════════════════

pub struct HwLgcToken(PhantomData<*const ()>);

#[derive(Debug)]
pub enum HwLgcError {
    SealMismatch(u64),
    CKernelNull,
    CKernelInvalid(i32),
    InvariantViolation(&'static str),
}

/// Hap LGC gate për modul hardware
/// Branchless delta check — identik me lgc_open()     
pub fn hw_lgc_open(expected: u64, runtime: u64) -> Result<HwLgcToken, HwLgcError> {
    let delta = expected ^ runtime;
    let invalid = (delta | delta.wrapping_neg()) >> 63;
    match invalid {
        0 => Ok(HwLgcToken(PhantomData)),
        _ => Err(HwLgcError::SealMismatch(delta)),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// §3  C STRUCTS — repr(C) për FFI
// ════════════════════════════════════════════════════════════════════════════

/// Pasqyron HwSnapshot nga hw_resource.h
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CHwSnapshot {
    pub ram_free_bytes: u64,
    pub ram_total_bytes: u64,
    pub cpu_load_pct: f32,
    pub cores_active: u32,
    pub cores_total: u32,
    pub energy_margin_pct: f32,
    pub on_battery: bool,
    pub _pad: [u8; 7],
    pub ts_ns: u64,
}

/// Pasqyron ThermalSnapshot nga hw_thermal.h
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CThermalSnapshot {
    pub t_estimated_c: f32,
    pub t_limit_c: f32,
    pub t_warn_c: f32,
    pub t_junction_max_c: f32,
    pub p_cpu_w: f32,
    pub p_mem_w: f32,
    pub p_total_w: f32,
    pub sensor_used: bool,
    pub overheat: bool,
    pub warning: bool,
    pub _pad: [u8; 5],
    pub ts_ns: u64,
}

/// Pasqyron ColdDownAction nga hw_colddown.h
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CColdDownAction {
    Normal = 0,
    Warn = 1,
    Cool = 2,
    Throttle = 3,
    Emergency = 4,
}

/// Pasqyron ColdDownResult nga hw_colddown.h
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct CColdDownResult {
    pub rc: i32,
    pub fan_set: bool,
    pub cooling_set: bool,
    pub throttle_set: bool,
    pub fan_pct_applied: u8,
}

// ════════════════════════════════════════════════════════════════════════════
// §4  EXTERN "C" — TË GJITHA DEKLARATAT UNSAFE JANË KËTU
//     Asgjë unsafe jashtë këtij seksioni.
// ════════════════════════════════════════════════════════════════════════════

#[cfg(feature = "hw_kernel")]
extern "C" {
    // hw_resource.h
    fn hw_resource_snapshot(out: *mut CHwSnapshot) -> i32;

    // hw_thermal.h
    fn hw_thermal_read(
        hw: *const CHwSnapshot,
        cfg: *const u8, // NULL = use default ThermalConfig
        out: *mut CThermalSnapshot,
    ) -> i32;

    // hw_colddown.h
    fn hw_colddown_execute(action: CColdDownAction, result: *mut CColdDownResult) -> i32;
    fn hw_colddown_restore_normal() -> i32;
    fn hw_colddown_sysfs_available() -> i32;
}

// ════════════════════════════════════════════════════════════════════════════
// §5  SAFE WRAPPERS — validojnë output C para kthimit
//     Jashtë këtyre, kodi është 100% safe Rust.
// ════════════════════════════════════════════════════════════════════════════

/// Merr snapshot hardware nga C kernel
/// Validon të gjitha invariantet para kthimit
pub fn kernel_hw_snapshot(_tok: &HwLgcToken) -> Result<CHwSnapshot, HwLgcError> {
    #[cfg(feature = "hw_kernel")]
    {
        let mut snap = CHwSnapshot::default();
        let rc = unsafe { hw_resource_snapshot(&mut snap) };
        match rc != 0 {
            true => return Err(HwLgcError::CKernelInvalid(rc)),
            false => {}
        }
        // Validim invariantesh
        match snap.ram_total_bytes == 0 {
            true => return Err(HwLgcError::InvariantViolation("ram_total=0")),
            false => {}
        }
        match snap.ram_free_bytes > snap.ram_total_bytes {
            true => return Err(HwLgcError::InvariantViolation("ram_free > ram_total")),
            false => {}
        }
        match snap.cpu_load_pct < 0.0 || snap.cpu_load_pct > 100.0 {
            true => return Err(HwLgcError::InvariantViolation("cpu_load_pct ∉ [0,100]")),
            false => {}
        }
        match snap.energy_margin_pct < 0.0 || snap.energy_margin_pct > 1.0 {
            true => return Err(HwLgcError::InvariantViolation("energy_margin ∉ [0,1]")),
            false => {}
        }
        match snap.cores_active == 0 || snap.cores_active > snap.cores_total {
            true => return Err(HwLgcError::InvariantViolation("cores_active invalid")),
            false => {}
        }
        Ok(snap)
    }

    // ── FALLBACK NOMINAL (hw_kernel OFF — hardware NUK matet) ──────────────
    // Kompilohet VETËM pa feature-n hw_kernel. Prodhimi (full/hw_kernel)
    // përdor kernelin C REAL më lart. Këto s'janë "simulim që pretendon":
    // janë vlera nominale të DEKLARUARA si të pamatura. Sinjali i vërtetë
    // është kernel_hw_available()=false → hw_real=false rrjedh në gate dhe
    // asnjë fazë s'i trajton këto si matje reale. ts_ns=0 = "pa stampë reale".
    #[cfg(not(feature = "hw_kernel"))]
    {
        let _ = _tok;
        Ok(CHwSnapshot {
            ram_free_bytes: 2u64 * 1024 * 1024 * 1024, // nominal (i pamatur)
            ram_total_bytes: 8u64 * 1024 * 1024 * 1024, // nominal (i pamatur)
            cpu_load_pct: 30.0,                        // nominal (i pamatur)
            cores_active: 4,
            cores_total: 4,
            energy_margin_pct: 0.85,
            on_battery: false,
            _pad: [0u8; 7],
            ts_ns: 0, // 0 = pa stampë reale
        })
    }
}

/// Lexon gjendjen termike
pub fn kernel_thermal_read(
    _tok: &HwLgcToken,
    hw: &CHwSnapshot,
) -> Result<CThermalSnapshot, HwLgcError> {
    #[cfg(feature = "hw_kernel")]
    {
        let mut snap = CThermalSnapshot::default();
        let rc = unsafe { hw_thermal_read(hw, core::ptr::null(), &mut snap) };
        match rc != 0 {
            true => return Err(HwLgcError::CKernelInvalid(rc)),
            false => {}
        }
        // Validim
        match snap.t_limit_c <= 0.0 {
            true => return Err(HwLgcError::InvariantViolation("t_limit_c ≤ 0")),
            false => {}
        }
        match snap.p_total_w < 0.0 {
            true => return Err(HwLgcError::InvariantViolation("p_total_w < 0")),
            false => {}
        }
        Ok(snap)
    }

    #[cfg(not(feature = "hw_kernel"))]
    {
        let _ = (_tok, hw);
        // FALLBACK NOMINAL (hw_kernel OFF): termika nga formula matematike mbi
        // snapshot-in nominal. sensor_used=false = DEKLARIM se s'ka sensor real.
        let p_cpu = 65.0f32 * (hw.cpu_load_pct / 100.0);
        let ram_used_gb = (hw.ram_total_bytes.saturating_sub(hw.ram_free_bytes)) as f32
            / (1024.0 * 1024.0 * 1024.0);
        let p_mem = ram_used_gb * 3.0;
        let p_total = p_cpu + p_mem;
        let t_est = 25.0 + p_total * 0.5;
        let t_limit = 100.0 * 0.85;
        Ok(CThermalSnapshot {
            t_estimated_c: t_est,
            t_limit_c: t_limit,
            t_warn_c: 100.0 * 0.75,
            t_junction_max_c: 100.0,
            p_cpu_w: p_cpu,
            p_mem_w: p_mem,
            p_total_w: p_total,
            sensor_used: false,
            overheat: t_est >= t_limit,
            warning: t_est >= (100.0 * 0.75) && t_est < t_limit,
            _pad: [0u8; 5],
            ts_ns: 0,
        })
    }
}

/// Ekzekuton veprimin e ftohjes
pub fn kernel_colddown_execute(
    _tok: &HwLgcToken,
    action: CColdDownAction,
) -> Result<CColdDownResult, HwLgcError> {
    #[cfg(feature = "hw_kernel")]
    {
        let mut result = CColdDownResult::default();
        let rc = unsafe { hw_colddown_execute(action, &mut result) };
        result.rc = rc;
        Ok(result)
    }

    #[cfg(not(feature = "hw_kernel"))]
    {
        let _ = _tok;
        // FALLBACK NOMINAL (hw_kernel OFF): asnjë veprim fizik nuk u aplikua.
        // Mos prodho boolean-e/fan percentage që duken si kontroll real.
        let _ = action;
        Ok(CColdDownResult {
            rc: -2, // provider fizik i padisponueshëm
            fan_set: false,
            cooling_set: false,
            throttle_set: false,
            fan_pct_applied: 0,
        })
    }
}

/// Kthen sistemin në gjendje normale
pub fn kernel_restore_normal(_tok: &HwLgcToken) -> Result<(), HwLgcError> {
    #[cfg(feature = "hw_kernel")]
    {
        let rc = unsafe { hw_colddown_restore_normal() };
        match rc != 0 {
            true => return Err(HwLgcError::CKernelInvalid(rc)),
            false => {}
        }
        Ok(())
    }
    #[cfg(not(feature = "hw_kernel"))]
    {
        let _ = _tok;
        Ok(())
    }
}

/// Tregon nëse build-i përdor provider-in REAL të snapshot-it hardware.
/// Kjo nuk është e njëjtë me disponueshmërinë e sysfs/fan control: në Windows
/// snapshot-i është real edhe kur sysfs cooling nuk ekziston.
pub fn kernel_hw_available() -> bool {
    #[cfg(feature = "hw_kernel")]
    {
        true
    }
    #[cfg(not(feature = "hw_kernel"))]
    {
        false
    }
}

/// Aftësi e veçantë: a mund të aplikohet cooling/throttle përmes kernelit C.
/// `false` nuk e zhvlerëson snapshot-in real (p.sh. Windows pa sysfs).
pub fn kernel_cooling_available() -> bool {
    #[cfg(feature = "hw_kernel")]
    {
        unsafe { hw_colddown_sysfs_available() > 0 }
    }
    #[cfg(not(feature = "hw_kernel"))]
    {
        false
    }
}

#[cfg(all(test, not(feature = "hw_kernel")))]
mod fallback_tests {
    use super::*;

    #[test]
    fn nominal_fallback_never_claims_physical_cooling() {
        let token = hw_lgc_open(S_COLD_DOWN, S_COLD_DOWN).expect("token");
        let result = kernel_colddown_execute(&token, CColdDownAction::Emergency)
            .expect("typed fallback result");
        assert_ne!(result.rc, 0);
        assert!(!result.fan_set);
        assert!(!result.cooling_set);
        assert!(!result.throttle_set);
        assert_eq!(result.fan_pct_applied, 0);
        assert!(!kernel_hw_available());
        assert!(!kernel_cooling_available());
    }
}

#[cfg(all(test, feature = "hw_kernel"))]
mod r38_resource_tests {
    use super::CHwSnapshot;

    extern "C" {
        fn hw_resource_cpu_load_from_counters(
            idle1: u64,
            kern1: u64,
            user1: u64,
            idle2: u64,
            kern2: u64,
            user2: u64,
            load_pct: *mut f32,
        ) -> i32;
        fn hw_resource_validate(snapshot: *const CHwSnapshot) -> i32;
    }

    fn calculate(before: [u64; 3], after: [u64; 3]) -> (i32, f32) {
        let mut load = -17.0;
        let rc = unsafe {
            hw_resource_cpu_load_from_counters(
                before[0], before[1], before[2],
                after[0], after[1], after[2], &mut load,
            )
        };
        (rc, load)
    }

    fn assert_invalid(before: [u64; 3], after: [u64; 3]) {
        let (rc, load) = calculate(before, after);
        assert_ne!(rc, 0, "invalid counters became valid {load}% CPU");
        assert_eq!(load, -17.0, "an invalid sample must not publish a load");
    }

    fn finite_snapshot() -> CHwSnapshot {
        CHwSnapshot {
            ram_free_bytes: 4 * 1024 * 1024 * 1024,
            ram_total_bytes: 8 * 1024 * 1024 * 1024,
            cpu_load_pct: 25.0,
            cores_active: 4,
            cores_total: 4,
            energy_margin_pct: 1.0,
            on_battery: false,
            _pad: [0; 7],
            ts_ns: 1,
        }
    }

    #[test]
    fn r38_cpu_valid_windows_keep_existing_results() {
        for (before, after, expected) in [
            ([10, 20, 30], [110, 120, 30], 0.0),
            ([10, 20, 30], [10, 120, 130], 100.0),
            ([10, 20, 30], [110, 120, 130], 50.0),
            ([0, 0, 0], [100, 200, 200], 75.0),
            ([u64::MAX - 1000, u64::MAX - 500, u64::MAX - 800],
             [u64::MAX - 900, u64::MAX - 300, u64::MAX - 600], 75.0),
        ] {
            let (rc, load) = calculate(before, after);
            assert_eq!(rc, 0);
            assert_eq!(load, expected);
        }
    }

    #[test]
    fn r38_cpu_each_regressing_counter_is_invalid() {
        for after in [[9, 120, 130], [10, 19, 130], [10, 120, 29]] {
            assert_invalid([10, 20, 30], after);
        }
    }

    #[test]
    fn r38_cpu_idle_delta_cannot_exceed_total_delta() {
        assert_invalid([0, 0, 0], [101, 50, 50]);
    }

    #[test]
    fn r38_cpu_no_elapsed_measurement_is_not_zero_load() {
        assert_invalid([10, 20, 30], [10, 20, 30]);
        assert_invalid([10, 20, 30], [11, 20, 30]);
    }

    #[test]
    fn r38_cpu_overflowing_delta_sum_is_invalid() {
        assert_invalid([0, 0, 0], [0, u64::MAX, 1]);
    }

    #[test]
    fn r38_cpu_null_output_is_rejected() {
        let rc = unsafe {
            hw_resource_cpu_load_from_counters(0, 0, 0, 0, 10, 10, core::ptr::null_mut())
        };
        assert_ne!(rc, 0);
    }

    #[test]
    fn r38_resource_finite_boundary_values_remain_valid() {
        for cpu in [0.0, 25.0, 100.0] {
            for energy in [0.0, 0.2, 1.0] {
                let mut snapshot = finite_snapshot();
                snapshot.cpu_load_pct = cpu;
                snapshot.energy_margin_pct = energy;
                assert_ne!(unsafe { hw_resource_validate(&snapshot) }, 0);
            }
        }
    }

    #[test]
    fn r38_resource_nonfinite_cpu_is_invalid() {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut snapshot = finite_snapshot();
            snapshot.cpu_load_pct = value;
            assert_eq!(unsafe { hw_resource_validate(&snapshot) }, 0,
                "nonfinite CPU must not cross the C resource boundary");
        }
    }

    #[test]
    fn r38_resource_nonfinite_energy_is_invalid() {
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut snapshot = finite_snapshot();
            snapshot.energy_margin_pct = value;
            assert_eq!(unsafe { hw_resource_validate(&snapshot) }, 0,
                "nonfinite energy must not cross the C resource boundary");
        }
    }
}

#[cfg(all(test, feature = "hw_kernel"))]
mod r39_physical_domain_tests {
    use super::{CHwSnapshot, CThermalSnapshot};

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct PhysicalConfig {
        tdp_base_w: f32,
        mem_power_per_gb_w: f32,
        r_thermal_c_per_w: f32,
        t_ambient_c: f32,
        t_junction_max_c: f32,
        safety_ratio: f32,
    }

    extern "C" {
        fn hw_thermal_validate(snapshot: *const CThermalSnapshot) -> i32;
        fn hw_resource_cpu_load_from_counters(
            idle1: u64, kern1: u64, user1: u64,
            idle2: u64, kern2: u64, user2: u64,
            load_pct: *mut f32,
        ) -> i32;
    }

    fn default_config() -> PhysicalConfig {
        PhysicalConfig {
            tdp_base_w: 65.0,
            mem_power_per_gb_w: 3.0,
            r_thermal_c_per_w: 0.5,
            t_ambient_c: 25.0,
            t_junction_max_c: 100.0,
            safety_ratio: 0.85,
        }
    }

    fn resource() -> CHwSnapshot {
        CHwSnapshot {
            ram_free_bytes: 2 * 1024 * 1024 * 1024,
            ram_total_bytes: 4 * 1024 * 1024 * 1024,
            cpu_load_pct: 50.0,
            cores_active: 4,
            cores_total: 4,
            energy_margin_pct: 1.0,
            on_battery: false,
            _pad: [0; 7],
            ts_ns: 1,
        }
    }

    fn output() -> CThermalSnapshot {
        CThermalSnapshot {
            t_estimated_c: 44.25,
            t_limit_c: 85.0,
            t_warn_c: 75.0,
            t_junction_max_c: 100.0,
            p_cpu_w: 32.5,
            p_mem_w: 6.0,
            p_total_w: 38.5,
            sensor_used: false,
            overheat: false,
            warning: false,
            _pad: [0; 5],
            ts_ns: 1,
        }
    }

    fn read(hw: &CHwSnapshot, config: Option<&PhysicalConfig>) -> (i32, CThermalSnapshot) {
        let config = config.map_or(core::ptr::null(), |value| {
            (value as *const PhysicalConfig).cast::<u8>()
        });
        let mut snapshot = CThermalSnapshot::default();
        let rc = unsafe { super::hw_thermal_read(hw, config, &mut snapshot) };
        (rc, snapshot)
    }

    fn set_config(config: &mut PhysicalConfig, field: usize, value: f32) {
        match field {
            0 => config.tdp_base_w = value,
            1 => config.mem_power_per_gb_w = value,
            2 => config.r_thermal_c_per_w = value,
            3 => config.t_ambient_c = value,
            4 => config.t_junction_max_c = value,
            5 => config.safety_ratio = value,
            _ => unreachable!(),
        }
    }

    fn set_output(snapshot: &mut CThermalSnapshot, field: usize, value: f32) {
        match field {
            0 => snapshot.t_estimated_c = value,
            1 => snapshot.t_limit_c = value,
            2 => snapshot.t_warn_c = value,
            3 => snapshot.t_junction_max_c = value,
            4 => snapshot.p_cpu_w = value,
            5 => snapshot.p_mem_w = value,
            6 => snapshot.p_total_w = value,
            _ => unreachable!(),
        }
    }

    fn assert_bits(actual: f32, expected: f32) {
        assert_eq!(actual.to_bits(), expected.to_bits(), "{actual} != {expected}");
    }

    fn assert_selected_temperature(snapshot: &CThermalSnapshot, expected: f32) {
        // Valid live sensors retain priority over the physical estimate.
        #[cfg(target_os = "windows")]
        assert!(!snapshot.sensor_used, "Windows fixture must use the existing mathematical path");
        if !snapshot.sensor_used {
            assert_bits(snapshot.t_estimated_c, expected);
        }
        assert_ne!(unsafe { hw_thermal_validate(snapshot) }, 0);
    }

    #[test]
    fn r39_thermal_nonfinite_config_is_rejected() {
        for field in 0..6 {
            for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                let mut config = default_config();
                set_config(&mut config, field, value);
                let (rc, _) = read(&resource(), Some(&config));
                assert_ne!(rc, 0, "config field={field} value={value} was accepted");
            }
        }
    }

    #[test]
    fn r39_thermal_invalid_physical_config_is_rejected() {
        for (field, value) in [
            (0, -1.0), (1, -1.0), (2, -0.5),
            (4, 0.0), (4, -1.0), (5, 0.0), (5, -0.1), (5, 1.01),
        ] {
            let mut config = default_config();
            set_config(&mut config, field, value);
            let (rc, _) = read(&resource(), Some(&config));
            assert_ne!(rc, 0, "config field={field} value={value} was accepted");
        }
    }

    #[test]
    fn r39_thermal_nonfinite_output_is_rejected() {
        for field in 0..7 {
            for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                let mut snapshot = output();
                set_output(&mut snapshot, field, value);
                assert_eq!(unsafe { hw_thermal_validate(&snapshot) }, 0,
                    "output field={field} value={value} was accepted");
            }
        }
    }

    #[test]
    fn r39_thermal_invalid_output_composition_and_flags_are_rejected() {
        for (field, value) in [
            (0, -50.01), (0, 300.01), (1, 0.0), (1, 101.0),
            (2, 0.0), (2, -1.0), (2, 101.0), (3, 0.0),
            (4, -1.0), (5, -1.0), (6, -1.0),
            (6, f32::from_bits(38.5_f32.to_bits() + 1)),
        ] {
            let mut snapshot = output();
            set_output(&mut snapshot, field, value);
            assert_eq!(unsafe { hw_thermal_validate(&snapshot) }, 0,
                "inconsistent output field={field} value={value} was accepted");
        }
        for (temperature, overheat, warning) in [
            (44.25, true, false), (44.25, false, true),
            (75.0, false, false), (85.0, false, true), (85.0, true, true),
        ] {
            let mut snapshot = output();
            snapshot.t_estimated_c = temperature;
            snapshot.overheat = overheat;
            snapshot.warning = warning;
            assert_eq!(unsafe { hw_thermal_validate(&snapshot) }, 0,
                "inconsistent flags at temperature={temperature} were accepted");
        }
    }

    #[test]
    fn r39_thermal_finite_overflow_is_rejected() {
        let mut hw = resource();
        hw.cpu_load_pct = 100.0;
        for config in [
            PhysicalConfig {
                tdp_base_w: f32::MAX,
                mem_power_per_gb_w: f32::MAX,
                r_thermal_c_per_w: 0.0,
                ..default_config()
            },
            PhysicalConfig {
                tdp_base_w: f32::MAX,
                mem_power_per_gb_w: 0.0,
                r_thermal_c_per_w: 2.0,
                ..default_config()
            },
        ] {
            let (rc, _) = read(&hw, Some(&config));
            assert_ne!(rc, 0, "finite inputs overflowing the physical equation were accepted");
        }
    }

    #[test]
    fn r39_thermal_default_equation_is_exact() {
        let (rc, snapshot) = read(&resource(), None);
        assert_eq!(rc, 0);
        assert_bits(snapshot.p_cpu_w, 32.5);
        assert_bits(snapshot.p_mem_w, 6.0);
        assert_bits(snapshot.p_total_w, 38.5);
        assert_bits(snapshot.t_junction_max_c, 100.0);
        assert_bits(snapshot.t_limit_c, 85.0);
        assert_bits(snapshot.t_warn_c, 75.0);
        assert_selected_temperature(&snapshot, 44.25);
    }

    #[test]
    fn r39_thermal_valid_custom_coefficients_are_honored() {
        let mut hw = resource();
        hw.cpu_load_pct = 25.0;
        let config = PhysicalConfig {
            tdp_base_w: 40.0,
            mem_power_per_gb_w: 2.0,
            r_thermal_c_per_w: 0.25,
            t_ambient_c: 10.0,
            t_junction_max_c: 120.0,
            safety_ratio: 0.5,
        };
        let (rc, snapshot) = read(&hw, Some(&config));
        assert_eq!(rc, 0);
        assert_bits(snapshot.p_cpu_w, 10.0);
        assert_bits(snapshot.p_mem_w, 4.0);
        assert_bits(snapshot.p_total_w, 14.0);
        assert_bits(snapshot.t_limit_c, 60.0);
        assert_bits(snapshot.t_warn_c, 90.0);
        assert_selected_temperature(&snapshot, 13.5);

        let zero = PhysicalConfig {
            tdp_base_w: 0.0,
            mem_power_per_gb_w: 0.0,
            r_thermal_c_per_w: 0.0,
            t_ambient_c: -20.0,
            safety_ratio: 1.0,
            ..default_config()
        };
        let (rc, snapshot) = read(&hw, Some(&zero));
        assert_eq!(rc, 0);
        assert_bits(snapshot.p_total_w, 0.0);
        assert_bits(snapshot.t_limit_c, 100.0);
        assert_selected_temperature(&snapshot, -20.0);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn r39_thermal_warning_limit_boundaries_and_hot_are_preserved() {
        for (temperature, overheat, warning) in [
            (f32::from_bits(75.0_f32.to_bits() - 1), false, false),
            (75.0, false, true),
            (f32::from_bits(85.0_f32.to_bits() - 1), false, true),
            (85.0, true, false), (100.0, true, false), (300.0, true, false),
        ] {
            let config = PhysicalConfig {
                tdp_base_w: 0.0,
                mem_power_per_gb_w: 0.0,
                r_thermal_c_per_w: 0.0,
                t_ambient_c: temperature,
                ..default_config()
            };
            let (rc, snapshot) = read(&resource(), Some(&config));
            assert_eq!(rc, 0, "valid temperature={temperature} was rejected");
            assert_selected_temperature(&snapshot, temperature);
            assert_eq!((snapshot.overheat, snapshot.warning), (overheat, warning));
        }
    }

    #[test]
    fn r39_thermal_output_boundaries_and_valid_hot_are_accepted() {
        for (temperature, overheat, warning) in [
            (-50.0, false, false),
            (f32::from_bits(75.0_f32.to_bits() - 1), false, false),
            (75.0, false, true),
            (f32::from_bits(85.0_f32.to_bits() - 1), false, true),
            (85.0, true, false), (300.0, true, false),
        ] {
            let mut snapshot = output();
            snapshot.t_estimated_c = temperature;
            snapshot.overheat = overheat;
            snapshot.warning = warning;
            assert_ne!(unsafe { hw_thermal_validate(&snapshot) }, 0,
                "valid temperature={temperature} was rejected");
        }
        let mut low_limit = output();
        low_limit.t_limit_c = 40.0;
        low_limit.overheat = true;
        assert_ne!(unsafe { hw_thermal_validate(&low_limit) }, 0,
            "warning above limit is valid when overheat has precedence");
    }

    #[test]
    fn r39_windows_idle_delta_is_subset_of_kernel_delta() {
        let mut load = -17.0;
        let rc = unsafe {
            hw_resource_cpu_load_from_counters(100, 200, 0, 200, 250, 100, &mut load)
        };
        assert_ne!(rc, 0, "idle delta larger than kernel delta became valid CPU load");
        assert_bits(load, -17.0);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn r39_c_physical_temperature_controls_existing_hpro_bound() {
        use crate::lab_contracts_v11::hardware::DeviceSnapshot;
        use crate::layer3::hpro::ProdFormula;

        let healthy = resource();
        let mut hot = healthy;
        hot.cpu_load_pct = 100.0;
        hot.ram_total_bytes = 64 * 1024 * 1024 * 1024;
        for (raw, expected_temperature, expected_overheat, expected_budget) in [
            (healthy, 44.25, false, (u128::from(healthy.ram_free_bytes) * 4 / 5) as u64),
            (hot, 150.5, true, 0),
        ] {
            let (rc, thermal) = read(&raw, None);
            assert_eq!(rc, 0);
            assert_selected_temperature(&thermal, expected_temperature);
            assert_eq!(thermal.overheat, expected_overheat);
            // Same resource/thermal field conversion as HardwareRuntimeContext::capture.
            let device = DeviceSnapshot {
                ram_free_bytes: raw.ram_free_bytes,
                ram_total_bytes: raw.ram_total_bytes,
                cpu_load: (raw.cpu_load_pct / 100.0).clamp(0.0, 1.0),
                cores_active: raw.cores_active.max(1),
                cores_total: raw.cores_total.max(raw.cores_active).max(1),
                energy_margin: raw.energy_margin_pct.clamp(0.0, 1.0),
                on_battery: raw.on_battery,
                temp_c: thermal.t_estimated_c,
                temp_limit_c: thermal.t_limit_c.max(1.0),
                timestamp: raw.ts_ns,
            };
            let bound = ProdFormula::compute_bounds(&device);
            assert_eq!(bound.ram_budget_bytes, expected_budget);
            assert_eq!(bound.allows_deepening(), !expected_overheat);
            if expected_overheat {
                assert_bits(bound.pressure_push, 0.0);
                assert!(!bound.deep_allowed);
            }
        }
    }
}


#[cfg(all(test, feature = "hw_kernel"))]
mod audit_hw_boundary_tests {
    extern "C" {
        fn hw_resource_cpu_load_from_linux_counters(
            first: *const u64, second: *const u64, load_pct: *mut f32,
        ) -> i32;
    }

    fn calculate(first: [u64; 8], second: [u64; 8]) -> (i32, f32) {
        let mut load = -123.0;
        let rc = unsafe {
            hw_resource_cpu_load_from_linux_counters(first.as_ptr(), second.as_ptr(), &mut load)
        };
        (rc, load)
    }

    #[test]
    fn linux_cpu_no_interval_or_regression_is_not_measured_idle() {
        let first = [100; 8];
        assert_eq!(calculate(first, first), (-2, -123.0));
        for field in 0..8 {
            let mut second = [110; 8];
            second[field] = 99;
            assert_eq!(calculate(first, second), (-2, -123.0));
        }
        assert_eq!(calculate([100,0,0,1000,10,0,0,0], [110,0,0,1000,0,0,0,0]), (-2, -123.0));
    }

    #[test]
    fn linux_cpu_overflow_and_null_do_not_modify_measurement() {
        assert_eq!(calculate([0; 8], [u64::MAX,1,0,0,0,0,0,0]), (-2, -123.0));
        let mut load = -123.0;
        assert_eq!(unsafe { hw_resource_cpu_load_from_linux_counters(core::ptr::null(), [0;8].as_ptr(), &mut load) }, -1);
        assert_eq!(load, -123.0);
        assert_eq!(unsafe { hw_resource_cpu_load_from_linux_counters([0;8].as_ptr(), [1;8].as_ptr(), core::ptr::null_mut()) }, -1);
    }

    #[test]
    fn linux_cpu_large_valid_interval_stays_in_physical_domain() {
        for (busy, idle) in [(671_091_u64, 0_u64), (617_404, 53_687), (u64::MAX, 0)] {
            let (rc, load) = calculate([0;8], [busy,0,0,idle,0,0,0,0]);
            let expected = (100.0_f64 * (busy as f64 / (busy + idle) as f64)) as f32;
            assert_eq!(rc, 0);
            assert!((0.0..=100.0).contains(&load), "busy={busy} idle={idle} load={load}");
            assert!((load - expected).abs() <= f32::EPSILON * 100.0);
        }
    }

    #[test]
    fn linux_cpu_valid_intervals_preserve_load() {
        for (second, expected) in [
            ([0,0,0,100,0,0,0,0], 0.0),
            ([100,0,0,0,0,0,0,0], 100.0),
            ([25,0,25,40,10,0,0,0], 50.0),
        ] {
            assert_eq!(calculate([0;8], second), (0, expected));
        }
        assert_eq!(calculate([u64::MAX - 1;8], [u64::MAX;8]), (0, 75.0));
    }
}
