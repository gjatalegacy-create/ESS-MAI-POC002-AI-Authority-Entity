// thermal_thread.rs - AIGovern-Cyber Parallel Thermal Monitor
// GJATA LEGACY - Quantum Platform
//
// Spawns a background thread that runs continuously,
// independent from the main reasoning pipeline.
// Uses ThermalGuardState (AtomicBool) for zero-lock
// communication with the pipeline.
//
// Thread lifecycle:
//   start_thermal_monitor() -> Arc<ThermalGuardState>
//       |
//       +-> thread loop:
//             hw_snapshot() -> run_thermal_cycle()
//               -> if overheat: set thermal_stop=true
//               -> if warning:  set thermal_warning=true
//             sleep(THERMAL_POLL_MS)
//   stop_thermal_monitor(state) -> signals thread to exit

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::hw_real::ffi::{hw_lgc_open, kernel_hw_snapshot, S_RESOURCE_TERRITORY};
use crate::hw_real::thermal::{ColdDown, ColdDownResult, ThermalAction, ThermalGuardState};
use crate::hw_real::ffi::HwLgcError;

/// Poll interval for thermal monitoring (milliseconds)
const THERMAL_POLL_MS: u64 = 2000;

/// Starts the parallel thermal monitor thread.
/// Returns Arc<ThermalGuardState> shared with the pipeline.
/// Thread runs until thermal_stop is set externally OR overheat.
pub fn start_thermal_monitor() -> Arc<ThermalGuardState> {
    let state = Arc::new(ThermalGuardState::new());
    let state_thread = Arc::clone(&state);

    // FAIL-CLOSED, JO PANIK (audit): spawn i thread-it mund të dështojë (OS
    // limit). Para: .expect → rrëzonte të gjithë procesin Quantum. Tani:
    // degradim i kontrolluar — monitori shënohet i padisponueshëm dhe gjendja
    // vendoset THROTTLE si postuim i sigurt (asnjë supozim ftohtësie pa monitor).
    match thread::Builder::new()
        .name("hw_thermal_monitor".to_string())
        .spawn(move || {
            thermal_monitor_loop(state_thread);
        }) {
        Ok(_handle) => {}
        Err(e) => {
            eprintln!(
                "[HW_THERMAL] DEGRADIM: spawn i monitorit dështoi ({e}) → \
                       gjendja THROTTLE (postuim i sigurt, s'ka panik)"
            );
            state.mark_monitor_unavailable();
        }
    }

    state
}

/// The thermal monitor loop — runs until stop signal.
fn thermal_monitor_loop(state: Arc<ThermalGuardState>) {
    loop {
        // Check if pipeline requested stop. Zero if — match me break.
        match state.is_stop() {
            true => break,
            false => {}
        }

        // Get hardware snapshot for thermal calculation
        let hw_result = hw_lgc_open(S_RESOURCE_TERRITORY, S_RESOURCE_TERRITORY)
            .and_then(|token| kernel_hw_snapshot(&token));
        let hw = match hw_result {
            Ok(snapshot) => snapshot,
            Err(error) => {
                record_thermal_cycle(&state, Err(error));
                thread::sleep(Duration::from_millis(THERMAL_POLL_MS));
                continue;
            }
        };

        // Run thermal cycle: Sensor -> Heat -> Down
        record_thermal_cycle(&state, ColdDown::run_thermal_cycle(&hw));

        thread::sleep(Duration::from_millis(THERMAL_POLL_MS));
    }
}

fn record_thermal_cycle(
    state: &ThermalGuardState,
    outcome: Result<Option<ColdDownResult>, HwLgcError>,
) {
    match outcome {
        Ok(Some(result)) => {
            // Hazard severity comes from the requested thermal action. Actuator
            // acknowledgments never prove that the thermal condition disappeared.
            match result.action_taken {
                ThermalAction::Throttle | ThermalAction::Emergency => {
                    state.set_stop(true);
                    state.set_warning(true);
                }
                ThermalAction::WarnCool | ThermalAction::ActivateCooling => state.set_warning(true),
                ThermalAction::Normal => state.set_warning(false),
            }
        }
        Ok(None) => state.set_warning(false),
        Err(error) => {
            eprintln!("[HW_THERMAL] measurement unavailable: {error:?}; policy=REDUCE_LOAD");
            state.mark_monitor_unavailable();
        }
    }
}

/// Stops the thermal monitor gracefully.
/// Sets thermal_stop = true, thread exits on next poll.
pub fn stop_thermal_monitor(state: &Arc<ThermalGuardState>) {
    state.set_stop(true);
}

/// Checks thermal state from pipeline side (zero-lock).
/// Called from Orchestrator before each reasoning cycle.
pub fn thermal_pipeline_check(state: &Arc<ThermalGuardState>) -> ThermalPipelineStatus {
    match state.is_stop() {
        true => return ThermalPipelineStatus::EmergencyStop,
        false => {}
    }
    match state.is_warning() {
        true => return ThermalPipelineStatus::ReduceLoad,
        false => {}
    }
    ThermalPipelineStatus::Normal
}

/// Status returned to pipeline from thermal check
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ThermalPipelineStatus {
    /// Normal temperature - proceed
    Normal,
    /// Warning temperature - reduce reasoning depth
    ReduceLoad,
    /// Emergency - stop reasoning immediately
    EmergencyStop,
}

impl ThermalPipelineStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "THERMAL_NORMAL",
            Self::ReduceLoad => "THERMAL_WARN_REDUCE",
            Self::EmergencyStop => "THERMAL_EMERGENCY_STOP",
        }
    }

    pub fn blocks_pipeline(self) -> bool {
        self == Self::EmergencyStop
    }
}


#[cfg(test)]
mod audit_hw_boundary_tests {
    use super::*;
    use crate::hw_real::thermal::ThermalAction;

    fn receipt(action: ThermalAction, actuator_ack: bool) -> ColdDownResult {
        ColdDownResult {
            action_taken: action, fan_set: actuator_ack, cooling_set: actuator_ack,
            throttle_set: actuator_ack, fan_pct: 0, success: actuator_ack, restored: false,
        }
    }

    #[test]
    fn thermal_emergency_without_actuator_access_still_stops() {
        for action in [ThermalAction::Throttle, ThermalAction::Emergency] {
            let state = Arc::new(ThermalGuardState::new());
            record_thermal_cycle(&state, Ok(Some(receipt(action, false))));
            assert_eq!(thermal_pipeline_check(&state), ThermalPipelineStatus::EmergencyStop);
        }
    }

    #[test]
    fn thermal_warning_depends_on_hazard_not_actuator_ack() {
        for action in [ThermalAction::WarnCool, ThermalAction::ActivateCooling] {
            for ack in [false, true] {
                let state = Arc::new(ThermalGuardState::new());
                record_thermal_cycle(&state, Ok(Some(receipt(action, ack))));
                assert_eq!(thermal_pipeline_check(&state), ThermalPipelineStatus::ReduceLoad);
            }
        }
    }

    #[test]
    fn thermal_missing_measurement_uses_existing_degraded_policy() {
        let state = Arc::new(ThermalGuardState::new());
        record_thermal_cycle(&state, Err(HwLgcError::CKernelInvalid(-2)));
        assert_eq!(thermal_pipeline_check(&state), ThermalPipelineStatus::ReduceLoad);
        record_thermal_cycle(&state, Ok(None));
        assert_eq!(thermal_pipeline_check(&state), ThermalPipelineStatus::Normal);
    }

    #[test]
    fn thermal_stop_cannot_be_cleared_by_later_normal_sample() {
        let state = Arc::new(ThermalGuardState::new());
        state.set_stop(true);
        record_thermal_cycle(&state, Ok(None));
        assert_eq!(thermal_pipeline_check(&state), ThermalPipelineStatus::EmergencyStop);
        record_thermal_cycle(&state, Err(HwLgcError::CKernelInvalid(-2)));
        assert_eq!(thermal_pipeline_check(&state), ThermalPipelineStatus::EmergencyStop);
    }
}
