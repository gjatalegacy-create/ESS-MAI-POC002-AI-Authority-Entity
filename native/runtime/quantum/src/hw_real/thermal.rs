// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  sensor_math_harduer.rs — Hapi 13                                      ║
// ║  cold_heat_manager.rs   — Hapi 14                                      ║
// ║  cold_down.rs           — Hapi 15                                      ║
// ║  GJATA LEGACY™ — Quantum Platform                                     ║
// ║                                                                          ║
// ║  Rrjedha termike (PARALEL — e pavarur nga pipeline kryesore):          ║
// ║  Sensor_Math_Harduer → snapshot termik                                 ║
// ║       ↓ (nëse overheat)                                                ║
// ║  Cold_Heat_Manager   → vendos ThermalAction                            ║
// ║       ↓                                                                 ║
// ║  Cold_Down           → urdhëron hardware fizik                         ║
// ╚══════════════════════════════════════════════════════════════════════════╝

use crate::hw_real::ffi::{
    hw_lgc_open, kernel_colddown_execute, kernel_restore_normal, kernel_thermal_read,
    CColdDownAction, CHwSnapshot, CThermalSnapshot, HwLgcError, S_COLD_DOWN, S_COLD_HEAT,
    S_SENSOR_MATH,
};
use std::sync::atomic::{AtomicBool, Ordering};

// ════════════════════════════════════════════════════════════════════════════
// SENSOR MATH HARDUER — Hapi 13
//
// Dyja modalitete:
//   1. IoT sensor (sysfs) → lexim direkt nëse ekziston
//   2. Formula matematike → nëse nuk ka sensor
//
// Funksionon si monitor i vazhdueshëm PARALEL me pipeline.
// Kur overheating → sinjalizon Cold_Heat_Manager me snapshot.
// ════════════════════════════════════════════════════════════════════════════

/// Modaliteti i sensorit
#[derive(Debug, Clone, PartialEq)]
pub enum SensorMode {
    /// Sensor fizik IoT — lexon nga sysfs
    IotDirect,
    /// Pa sensor — llogarit me formula matematike
    Mathematical,
}

/// Snapshot termik i prodhuar nga Sensor_Math_Harduer
#[derive(Debug, Clone)]
pub struct ThermalReading {
    pub inner: CThermalSnapshot,
    pub mode: SensorMode,
    pub hw_snapshot: CHwSnapshot, // burimi për llogaritje
}

impl ThermalReading {
    pub fn overheat(&self) -> bool {
        self.inner.overheat
    }
    pub fn warning(&self) -> bool {
        self.inner.warning
    }
    pub fn temp_c(&self) -> f32 {
        self.inner.t_estimated_c
    }
    pub fn t_limit(&self) -> f32 {
        self.inner.t_limit_c
    }
    pub fn p_total_w(&self) -> f32 {
        self.inner.p_total_w
    }

    /// Raporti termik ∈ [0.0, 1.0]: 0=ftohtë, 1=limit
    pub fn thermal_ratio(&self) -> f32 {
        match self.inner.t_junction_max_c <= 0.0 {
            true => return 0.0,
            false => {}
        }
        (self.inner.t_estimated_c / self.inner.t_junction_max_c).clamp(0.0, 1.0)
    }

    /// for_test — ndërton ThermalReading për teste (      ). hot=true → warning=true.
    /// Zero if — boolean direkt te fushat.
    #[cfg(test)]
    pub fn for_test(hot: bool) -> Self {
        let mut inner = CThermalSnapshot::default();
        inner.warning = hot;
        inner.overheat = false; // warning para emergency (graduale)
        inner.t_estimated_c = 50.0;
        inner.t_junction_max_c = 100.0;
        ThermalReading {
            inner,
            mode: SensorMode::Mathematical,
            hw_snapshot: CHwSnapshot::default(),
        }
    }
}

pub struct SensorMathHarduer;

// ════════════════════════════════════════════════════════════════════════════
// THERMAL STATE (v1.4.3) — TRE GJENDJE, JO DY
//
// GJETJA (audit p.6, PRANUAR): kur sensori dështonte, sistemi supozonte
// thermal_hot=false — "i verbër = i ftohtë". E rrezikshme për ligjin
// "pa mbinxehje". Tani: Unknown është gjendje MË VETE, dhe për thellësitë
// Deep/VeryDeep trajtohet KONSERVATIVISHT si e nxehtë (tërheqje) — harduerin
// e mbron dyshimi, jo optimizmi. Quantum NUK bëhet autoritet: kjo është
// vetëm mbrojtje fizike, verdikti mbetet te Shadow. Zero if — match.
// ════════════════════════════════════════════════════════════════════════════

/// Gjendja termike e lexuar — e vërteta, jo supozimi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThermalState {
    /// Sensori foli: temperatura brenda kufijve.
    Normal,
    /// Sensori foli: overheat OSE warning — tërheqje e detyruar.
    Hot,
    /// Sensori NUK foli (s'u hap / snapshot mungoi / leximi dështoi) —
    /// e panjohura NUK është e ftohtë.
    Unknown,
}

impl ThermalState {
    pub fn label(&self) -> &'static str {
        match self {
            ThermalState::Normal => "NORMAL",
            ThermalState::Hot => "I NXEHTË",
            ThermalState::Unknown => "I PANJOHUR",
        }
    }

    /// Nxehtësia EFEKTIVE për vendimin e harduerit:
    ///   Hot     → true gjithmonë (tërheqje).
    ///   Normal  → false.
    ///   Unknown → KONSERVATIV: true VETËM për Deep/VeryDeep (puna e rëndë
    ///             s'nis mbi harduer të verbër); Shallow/Medium vazhdojnë —
    ///             runtime i papenguar, mbinxehja e pamundur nga thellësia.
    pub fn effective_hot(&self, depth: crate::hw_real::territories::DepthHint) -> bool {
        match (self, depth.requires_hw_gate()) {
            (ThermalState::Hot, _) => true,
            (ThermalState::Unknown, true) => true, // konservativ: dyshimi mbron
            (ThermalState::Unknown, false) => false,
            (ThermalState::Normal, _) => false,
        }
    }
}

#[cfg(test)]
mod thermal_state_tests {
    use super::ThermalState;
    use crate::hw_real::territories::DepthHint;

    #[test]
    fn unknown_is_conservative_for_deep_work() {
        // E panjohura ≠ e ftohtë: puna e thellë tërhiqet, e cekëta vazhdon.
        assert!(ThermalState::Unknown.effective_hot(DepthHint::Deep));
        assert!(ThermalState::Unknown.effective_hot(DepthHint::VeryDeep));
        assert!(!ThermalState::Unknown.effective_hot(DepthHint::Shallow));
        assert!(!ThermalState::Unknown.effective_hot(DepthHint::Medium));
    }

    #[test]
    fn hot_always_pulls_back_regardless_of_depth() {
        assert!(ThermalState::Hot.effective_hot(DepthHint::Shallow));
        assert!(ThermalState::Hot.effective_hot(DepthHint::VeryDeep));
    }

    #[test]
    fn normal_never_fakes_heat() {
        assert!(!ThermalState::Normal.effective_hot(DepthHint::VeryDeep));
        assert!(!ThermalState::Normal.effective_hot(DepthHint::Shallow));
    }
}

impl SensorMathHarduer {
    /// Lexon/llogarit gjendjen termike aktuale.
    /// Provon sensor fizik fillimisht — nëse nuk ka → formula matematike.
    pub fn read(hw: &CHwSnapshot) -> Result<ThermalReading, HwLgcError> {
        let tok = hw_lgc_open(S_SENSOR_MATH, S_SENSOR_MATH)?;

        let thermal = kernel_thermal_read(&tok, hw)?;

        let mode = match thermal.sensor_used {
            true => SensorMode::IotDirect,
            false => SensorMode::Mathematical,
        };

        Ok(ThermalReading {
            inner: thermal,
            mode,
            hw_snapshot: *hw,
        })
    }

    /// Kontrollon nëse gjendja termike kërkon veprim nga Cold_Heat_Manager.
    /// Kthen true nëse: overheat OSE warning
    pub fn needs_action(reading: &ThermalReading) -> bool {
        reading.overheat() || reading.warning()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// COLD HEAT MANAGER — Hapi 14
//
// Merr ThermalReading nga Sensor_Math_Harduer.
// Analizon situatën dhe vendos ThermalAction.
// Sinjalizon Cold_Down me veprimin e duhur.
//
// Strategjia:
//   Normal:    T < T_warn                → CD_ACTION_NORMAL
//   Warning:   T_warn ≤ T < T_limit     → CD_ACTION_WARN
//   Overheat:  T ≥ T_limit (< max×0.95) → CD_ACTION_COOL
//   Critical:  T ≥ T_max×0.95           → CD_ACTION_THROTTLE
//   Emergency: T ≥ T_max                → CD_ACTION_EMERGENCY
// ════════════════════════════════════════════════════════════════════════════

/// Veprimi termik i vendosur nga Cold_Heat_Manager
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThermalAction {
    /// Sistem normal — asgjë nuk bëhet
    Normal,
    /// Paralajmërim — fan 70%, cooling +1 level
    WarnCool,
    /// Overheat — fan 100%, cooling max
    ActivateCooling,
    /// Kritik — fan 100% + CPU throttle
    Throttle,
    /// Emergjencë — çdo gjë në maksimum
    Emergency,
}

impl ThermalAction {
    /// Konverto në CColdDownAction për FFI
    pub fn to_c_action(self) -> CColdDownAction {
        match self {
            ThermalAction::Normal => CColdDownAction::Normal,
            ThermalAction::WarnCool => CColdDownAction::Warn,
            ThermalAction::ActivateCooling => CColdDownAction::Cool,
            ThermalAction::Throttle => CColdDownAction::Throttle,
            ThermalAction::Emergency => CColdDownAction::Emergency,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ThermalAction::Normal => "NORMAL",
            ThermalAction::WarnCool => "WARN_COOL",
            ThermalAction::ActivateCooling => "ACTIVATE_COOLING",
            ThermalAction::Throttle => "THROTTLE",
            ThermalAction::Emergency => "EMERGENCY",
        }
    }

    pub fn severity(self) -> u8 {
        match self {
            ThermalAction::Normal => 0,
            ThermalAction::WarnCool => 1,
            ThermalAction::ActivateCooling => 2,
            ThermalAction::Throttle => 3,
            ThermalAction::Emergency => 4,
        }
    }
}

/// Vendimi i Cold_Heat_Manager
#[derive(Debug, Clone)]
pub struct ThermalDecision {
    pub action: ThermalAction,
    pub reading: ThermalReading,
    pub throttle_hint: bool, // sinjal tek Pressure_Territory: redukto reasoning
    pub restore_after: bool, // kthen normal pas veprimit
}

pub struct ColdHeatManager;

impl ColdHeatManager {
    /// Analizon ThermalReading dhe vendos ThermalAction.
    pub fn evaluate(reading: ThermalReading) -> Result<ThermalDecision, HwLgcError> {
        let tok = hw_lgc_open(S_COLD_HEAT, S_COLD_HEAT)?;
        let _ = tok;

        let t = reading.temp_c();
        let limit = reading.t_limit();
        let warn = reading.inner.t_warn_c;
        let max = reading.inner.t_junction_max_c;

        // Prioritet kritik i ruajtur: i pari true fiton (më i ngutshmi → më i buti).
        // Zero if — match mbi tuple boolean.
        let action = match (t >= max, t >= max * 0.95, t >= limit, t >= warn) {
            (true, _, _, _) => ThermalAction::Emergency,
            (_, true, _, _) => ThermalAction::Throttle,
            (_, _, true, _) => ThermalAction::ActivateCooling,
            (_, _, _, true) => ThermalAction::WarnCool,
            _ => ThermalAction::Normal,
        };

        // throttle_hint: sinjalizon Pressure_Territory të lëshojë hapësirë
        let throttle_hint = action.severity() >= 2;
        // restore_after: kthej normal vetëm për warning — jo për overheating kritik
        let restore_after = action == ThermalAction::WarnCool;

        Ok(ThermalDecision {
            action,
            reading,
            throttle_hint,
            restore_after,
        })
    }
}

// ════════════════════════════════════════════════════════════════════════════
// COLD DOWN — Hapi 15
//
// Ekzekutuesi final: urdhëron hardware fizik.
// Merr ThermalDecision nga Cold_Heat_Manager.
// Thirr kernel_colddown_execute() via FFI.
// ════════════════════════════════════════════════════════════════════════════

/// Rezultati i veprimit Cold_Down
#[derive(Debug, Clone)]
pub struct ColdDownResult {
    pub action_taken: ThermalAction,
    pub fan_set: bool,
    pub cooling_set: bool,
    pub throttle_set: bool,
    pub fan_pct: u8,
    /// All requested provider acknowledgments completed; not physical-effect proof.
    pub success: bool,
    pub restored: bool, // nëse u kthye normal pas warmdown
}

fn colddown_required_mask(action: ThermalAction) -> u8 {
    match action {
        ThermalAction::Normal => 0,
        ThermalAction::WarnCool | ThermalAction::ActivateCooling => 3,
        ThermalAction::Throttle | ThermalAction::Emergency => 7,
    }
}

fn colddown_ack_mask(result: &crate::hw_real::ffi::CColdDownResult) -> u8 {
    u8::from(result.fan_set)
        | (u8::from(result.cooling_set) << 1)
        | (u8::from(result.throttle_set) << 2)
}

fn colddown_acknowledged(
    action: ThermalAction,
    result: &crate::hw_real::ffi::CColdDownResult,
) -> bool {
    let expected_fan = match action {
        ThermalAction::Normal => 0,
        ThermalAction::WarnCool => 70,
        _ => 100,
    };
    result.rc == 0
        && colddown_ack_mask(result) == colddown_required_mask(action)
        && result.fan_pct_applied == expected_fan
}

pub struct ColdDown;

impl ColdDown {
    /// Ekzekuton veprimin e ftohjes bazuar në ThermalDecision.
    /// Urdhëron hardware fizik: fan, cooling device, CPU throttle.
    pub fn execute(decision: ThermalDecision) -> Result<ColdDownResult, HwLgcError> {
        let tok = hw_lgc_open(S_COLD_DOWN, S_COLD_DOWN)?;

        let c_action = decision.action.to_c_action();
        let c_result = kernel_colddown_execute(&tok, c_action)?;

        // Completion acknowledges the requested writes; physical effect remains unproven.
        let success = colddown_acknowledged(decision.action, &c_result);
        let requested = colddown_required_mask(decision.action);
        let acknowledged = colddown_ack_mask(&c_result);
        let _ = std::io::Write::write_fmt(&mut std::io::stderr(), format_args!(
            "[HW_ACTUATION] hw_ts_ns={} thermal_ts_ns={} action={} requested={} ack={} missing={} rc={} complete_ack={} physical_effect=UNPROVEN\n",
            decision.reading.hw_snapshot.ts_ns, decision.reading.inner.ts_ns,
            decision.action.label(), requested, acknowledged,
            requested & !acknowledged, c_result.rc, success));

        // Nëse restore_after → ktheje në normal pas pauzës
        // restore vetëm kur restore_after AND success. Zero if — match mbi tuple.
        let restored = match (decision.restore_after, success) {
            (true, true) => kernel_restore_normal(&tok).is_ok(),
            _ => false,
        };

        Ok(ColdDownResult {
            action_taken: decision.action,
            fan_set: c_result.fan_set,
            cooling_set: c_result.cooling_set,
            throttle_set: c_result.throttle_set,
            fan_pct: c_result.fan_pct_applied,
            success,
            restored,
        })
    }

    /// Rrjedha e plotë termike: Sensor → Heat → Down
    /// Thirret nga thread-i paralel i monitorimit termik.
    /// Nëse nuk ka nevojë për veprim → kthehet menjëherë (zero overhead).
    pub fn run_thermal_cycle(hw: &CHwSnapshot) -> Result<Option<ColdDownResult>, HwLgcError> {
        // 1. Sensor_Math_Harduer: lexo/llogarit temperaturën
        let reading = SensorMathHarduer::read(hw)?;

        // 2. Nëse nuk ka nevojë për veprim → kthehu menjëherë
        match SensorMathHarduer::needs_action(&reading) {
            false => return Ok(None),
            true => {}
        }

        // 3. Cold_Heat_Manager: vendos veprimin
        let decision = ColdHeatManager::evaluate(reading)?;

        // 4. Cold_Down: ekzekuto veprimin fizik
        let result = ColdDown::execute(decision)?;

        Ok(Some(result))
    }
}

// ════════════════════════════════════════════════════════════════════════════
// THERMAL GUARD — mbikëqyrës i pavarur (thread paralel)
//
// Punon PARALEL me pipeline kryesore — nuk bllokon asnjë fazë.
// Përdor AtomicBool për komunikim zero-lock me pipeline.
// ════════════════════════════════════════════════════════════════════════════

/// Shared state ndërmjet thermal thread dhe pipeline
pub struct ThermalGuardState {
    /// true = thermal thread ka vendosur STOP reasoning
    pub thermal_stop: AtomicBool,
    /// true = sistemi është nën paralajmërim termik
    pub thermal_warning: AtomicBool,
}

impl ThermalGuardState {
    pub const fn new() -> Self {
        Self {
            thermal_stop: AtomicBool::new(false),
            thermal_warning: AtomicBool::new(false),
        }
    }

    pub fn is_stop(&self) -> bool {
        self.thermal_stop.load(Ordering::Acquire)
    }

    pub fn is_warning(&self) -> bool {
        self.thermal_warning.load(Ordering::Acquire)
    }

    pub fn set_stop(&self, val: bool) {
        self.thermal_stop.store(val, Ordering::Release);
    }

    pub fn set_warning(&self, val: bool) {
        self.thermal_warning.store(val, Ordering::Release);
    }

    /// DEGRADIM I KONTROLLUAR (jo panik): monitori termik s'u nis dot.
    /// Postuim i sigurt — pa monitor s'ka dëshmi ftohtësie, ndaj vendoset
    /// PARALAJMËRIM (pipeline-i mund të ngadalësojë), por procesi NUK rrëzohet.
    pub fn mark_monitor_unavailable(&self) {
        self.thermal_warning.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod r39_hardware_contract_tests {
    use super::{colddown_acknowledged, ThermalAction};
    use crate::hw_real::ffi::CColdDownResult;

    fn receipt(mask: u8, rc: i32, fan_pct: u8) -> CColdDownResult {
        CColdDownResult {
            rc,
            fan_set: mask & 1 != 0,
            cooling_set: mask & 2 != 0,
            throttle_set: mask & 4 != 0,
            fan_pct_applied: fan_pct,
        }
    }

    #[test]
    fn r39_contract_ack_requires_exact_requested_components() {
        for (action, required, fan_pct) in [
            (ThermalAction::Normal, 0, 0),
            (ThermalAction::WarnCool, 3, 70),
            (ThermalAction::ActivateCooling, 3, 100),
            (ThermalAction::Throttle, 7, 100),
            (ThermalAction::Emergency, 7, 100),
        ] {
            for actual in 0..8 {
                let result = receipt(actual, 0, fan_pct);
                assert_eq!(colddown_acknowledged(action, &result), actual == required,
                    "request={action:?} required={required} ack={actual}");
            }
            let wrong_setting = receipt(required, 0, fan_pct.wrapping_add(1));
            assert!(!colddown_acknowledged(action, &wrong_setting));
        }
    }

    #[test]
    fn r39_contract_error_rc_never_becomes_completed_ack() {
        for action in [
            ThermalAction::Normal, ThermalAction::WarnCool,
            ThermalAction::ActivateCooling, ThermalAction::Throttle,
            ThermalAction::Emergency,
        ] {
            for mask in 0..8 {
                for rc in [-1, -2, -3, -4] {
                    assert!(!colddown_acknowledged(action, &receipt(mask, rc, 100)));
                }
            }
        }
    }

    #[cfg(all(feature = "hw_kernel", target_os = "windows"))]
    fn require_absent_sysfs() {
        // These real-provider regressions must never reach a writable device path.
        for path in [
            "/sys",
            "/sys/class/hwmon/hwmon0",
            "/sys/class/thermal/cooling_device0",
            "/sys/devices/system/cpu/cpu0/cpufreq",
            "/sys/class/hwmon/hwmon0/pwm1",
            "/sys/class/hwmon/hwmon0/pwm1_enable",
            "/sys/class/thermal/cooling_device0/cur_state",
            "/sys/class/thermal/cooling_device0/max_state",
            "/sys/devices/system/cpu/cpu0/cpufreq/scaling_max_freq",
            "/sys/devices/system/cpu/cpu0/cpufreq/scaling_available_frequencies",
        ] {
            match std::fs::symlink_metadata(path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                result => panic!("unsafe physical test precondition for {path}: {result:?}"),
            }
        }
    }

    #[cfg(all(feature = "hw_kernel", target_os = "windows"))]
    #[test]
    fn r39_contract_unavailable_c_provider_does_not_ack() {
        use crate::hw_real::ffi::{
            hw_lgc_open, kernel_colddown_execute, CColdDownAction, S_COLD_DOWN,
        };
        require_absent_sysfs();
        let token = hw_lgc_open(S_COLD_DOWN, S_COLD_DOWN).expect("module binding");
        for action in [
            CColdDownAction::Warn, CColdDownAction::Cool,
            CColdDownAction::Throttle, CColdDownAction::Emergency,
        ] {
            let result = kernel_colddown_execute(&token, action).expect("typed receipt");
            assert_eq!(result.rc, -4, "unavailable provider action={action:?}");
            assert!(!result.fan_set && !result.cooling_set && !result.throttle_set);
            assert_eq!(result.fan_pct_applied, 0);
        }
    }

    #[cfg(all(feature = "hw_kernel", target_os = "windows"))]
    #[test]
    fn r39_contract_unavailable_execute_is_not_success() {
        use super::{ColdDown, ThermalDecision, ThermalReading};
        require_absent_sysfs();
        for action in [
            ThermalAction::WarnCool, ThermalAction::ActivateCooling,
            ThermalAction::Throttle, ThermalAction::Emergency,
        ] {
            let result = ColdDown::execute(ThermalDecision {
                action,
                reading: ThermalReading::for_test(true),
                throttle_hint: false,
                restore_after: false,
            }).expect("typed acknowledgment");
            assert!(!result.success, "unavailable provider claimed success for {action:?}");
            assert!(!result.fan_set && !result.cooling_set && !result.throttle_set);
            assert_eq!(result.fan_pct, 0);
            assert!(!result.restored);
        }
    }

    #[cfg(all(feature = "hw_kernel", target_os = "windows"))]
    #[test]
    fn r39_contract_normal_has_no_applied_fan_percentage() {
        use crate::hw_real::ffi::{
            hw_lgc_open, kernel_colddown_execute, CColdDownAction, S_COLD_DOWN,
        };
        require_absent_sysfs();
        let token = hw_lgc_open(S_COLD_DOWN, S_COLD_DOWN).expect("module binding");
        let result = kernel_colddown_execute(&token, CColdDownAction::Normal)
            .expect("no-action receipt");
        assert_eq!(result.rc, 0);
        assert!(!result.fan_set && !result.cooling_set && !result.throttle_set);
        assert_eq!(result.fan_pct_applied, 0, "no write means no applied fan percentage");
    }
}
