//! Explicit recovery of a durable native PD cycle.
//! All returned state is historical data, not a live authority capability.
use shadow_contracts::{pd_cycle, pd_recovery as recovery};
use std::{
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};

use ess_mai_zero_noi::trigger;
use sha2::{Digest, Sha256};
use shadow_contracts::pd_memory::{hex, Hash, LgcStep};

fn digest(bytes: &[u8]) -> Hash {
    Sha256::digest(bytes).into()
}
/// Caller-owned, append-only delivery observations. This is diagnostic LGC
/// material, not a new persisted T1/T2 attestation or an NBES verification.
struct NativeTrace {
    root: Hash,
    steps: Vec<LgcStep>,
}
impl NativeTrace {
    fn new(predecessor_input: &[u8]) -> Self {
        Self {
            root: digest(predecessor_input),
            steps: Vec::new(),
        }
    }
    fn append(&mut self, site: &str, action: &str, payload: Hash) -> Result<(), String> {
        let predecessor = self
            .steps
            .last()
            .map(|v| v.step_digest)
            .unwrap_or(self.root);
        let sequence = u32::try_from(self.steps.len()).map_err(|_| "PD_NATIVE_LGC_SEQUENCE")?;
        let step = LgcStep::new(sequence, predecessor, site, action, payload)?;
        eprintln!(
            "[LGC/NATIVE] process={} sequence={} predecessor={} site={:?} action={:?} payload_sha256={} step_sha256={} authority=0 new_attestation=false",
            std::process::id(), step.sequence, hex(&step.predecessor),
            step.site, step.action, hex(&step.payload_digest), hex(&step.step_digest)
        );
        self.steps.push(step);
        Ok(())
    }
    /// The predecessor supplies the exact action. The neutral trigger only borrows
    /// those bytes; it neither selects a route nor grants a missing permission.
    fn invoke<T>(
        &mut self,
        site: &str,
        input: &[u8],
        effect: impl FnOnce(&[u8]) -> Result<T, String>,
        output_digest: impl FnOnce(&T) -> Hash,
    ) -> Result<T, String> {
        self.append(site, "ENTER_PREDECESSOR_REQUESTED_ACTION", digest(input))?;
        let pulse = trigger(input);
        let result = effect(pulse.request());
        match &result {
            Ok(value) => self.append(site, "RETURNED_OK", output_digest(value))?,
            Err(reason) => self.append(site, "RETURNED_ERROR", digest(reason.as_bytes()))?,
        }
        result
    }
}

fn read_bounded(path: &Path, max: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take((max + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > max {
        return Err("PD_NATIVE_READ_LIMIT".into());
    }
    Ok(bytes)
}
fn immutable(path: &Path, bytes: &[u8]) -> Result<(), String> {
    match crate::pd_operational::publish_new(path, bytes) {
        Ok(()) => Ok(()),
        Err(original) => match read_bounded(path, bytes.len()) {
            Ok(existing) if existing == bytes => Ok(()),
            _ => Err(format!("PD_NATIVE_INTENT_CONFLICT:{original}")),
        },
    }
}
/// Transport outbox only. Durable epistemic history remains solely in Shadow.
/// Retained even after an uncertain process exit so exact retry is possible.
pub fn retain_intent(
    request: &shadow_contracts::ShadowCycleRequest,
    prepared: &shadow_contracts::pd_prepared::PdPreparedEnvelope,
) -> Result<PathBuf, String> {
    let raw = shadow_contracts::encode_cycle_request(request);
    let packet = pd_cycle::encode_cycle(&raw, prepared)?;
    let lookup = recovery::RecoveryLookup {
        identity: pd_cycle::identity(request)?,
        request_sha256: prepared.source_request_sha256,
    };
    let root = std::env::var_os("ESSMAI_HANDOFF_DIR")
        .map(PathBuf::from)
        .ok_or("PD_HANDOFF_MISSING")?;
    if !root.is_absolute() {
        return Err("PD_NATIVE_OUTBOX_RELATIVE".into());
    }
    let directory = root.join("native_pd_intents");
    std::fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let name = format!(
        "{}-{}",
        shadow_contracts::pd_memory::hex(&lookup.identity),
        shadow_contracts::pd_memory::hex(&lookup.request_sha256)
    );
    let mut trace = NativeTrace::new(&raw);
    let intent_path = directory.join(format!("{name}.pdt1"));
    trace.invoke(
        "RETAIN_EXACT_PDT1_INTENT",
        &packet,
        |borrowed| immutable(&intent_path, borrowed),
        |_| digest(&packet),
    )?;
    let reference = directory.join(format!("{name}.lookup"));
    let lookup_bytes = recovery::encode_lookup(&lookup)?;
    trace.invoke(
        "RETAIN_EXPLICIT_RECOVERY_LOOKUP",
        &lookup_bytes,
        |borrowed| immutable(&reference, borrowed),
        |_| digest(&lookup_bytes),
    )?;
    Ok(reference)
}
pub fn recover(lookup: &recovery::RecoveryLookup) -> Result<recovery::RecoveryBundle, String> {
    let encoded = recovery::encode_lookup(lookup)?;
    let mut trace = NativeTrace::new(&encoded);
    let bytes = trace.invoke(
        "SHADOW_EXACT_STORED_RECOVERY",
        &encoded,
        |borrowed| {
            crate::pd_operational::native_transport(
                "--pd-recover-once",
                borrowed,
                Duration::from_secs(90),
            )
        },
        |returned| digest(returned),
    )?;
    let bundle = trace.invoke(
        "BIND_RETURNED_HISTORICAL_BUNDLE",
        &bytes,
        |borrowed| {
            let bundle = recovery::decode_bundle(borrowed)?;
            bundle.validate_for(lookup)?;
            Ok(bundle)
        },
        |_| digest(&bytes),
    )?;
    // Actual source finalization only. No NBES rerun or new Shadow adjudication.
    trace.invoke(
        "PD_ORIGINAL_FINALIZER_ON_STORED_T1_T2",
        &bytes,
        |_| crate::pd_prepared::finalize_recovered(&bundle),
        |closure| digest(format!("{closure:?}").as_bytes()),
    )?;
    Ok(bundle)
}
pub fn dispatch(args: &[String]) -> Option<i32> {
    if args.get(1).map(String::as_str) != Some("--pd-native-recover-once") {
        return None;
    }
    if args.len() != 4 {
        return Some(64);
    }
    let result = (|| {
        let lookup = recovery::decode_lookup(&read_bounded(Path::new(&args[2]), 68)?)?;
        let bundle = recover(&lookup)?;
        crate::pd_operational::publish_new(Path::new(&args[3]), &recovery::encode_bundle(&bundle)?)
    })();
    Some(match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("[PD/NATIVE/RECOVERY] {e}");
            70
        }
    })
}

#[cfg(test)]
mod trigger_tests {
    use super::*;
    #[test]
    fn neutral_trigger_delivers_exact_borrowed_bytes_once_and_binds_both_observations() {
        let input = b"explicit stable context and exact stored request".to_vec();
        let expected = input.clone();
        let mut trace = NativeTrace::new(&input);
        let mut called = 0;
        let output = trace
            .invoke(
                "UNIT_EXACT_ACTION",
                &input,
                |borrowed| {
                    called += 1;
                    assert!(std::ptr::eq(borrowed.as_ptr(), input.as_ptr()));
                    Ok(borrowed.to_vec())
                },
                |value| digest(value),
            )
            .unwrap();
        assert_eq!(called, 1);
        assert_eq!(output, expected);
        assert_eq!(input, expected);
        assert_eq!(trace.steps.len(), 2);
        assert_eq!(trace.steps[0].predecessor, digest(&input));
        assert_eq!(trace.steps[0].payload_digest, digest(&input));
        assert_eq!(trace.steps[1].predecessor, trace.steps[0].step_digest);
        assert_eq!(trace.steps[1].payload_digest, digest(&output));
        assert_eq!(trace.steps[1].action, "RETURNED_OK");
    }
    #[test]
    fn source_refusal_is_logged_and_returned_without_retry_fallback_or_success_digest() {
        let input = b"exact lookup".to_vec();
        let mut trace = NativeTrace::new(&input);
        let mut calls = 0;
        let result: Result<(), String> = trace.invoke(
            "UNIT_CLOSED_ACTION",
            &input,
            |_| {
                calls += 1;
                Err("SOURCE_AUTHORITY_REFUSED".into())
            },
            |_| panic!("no success result exists"),
        );
        assert_eq!(calls, 1);
        assert_eq!(result.unwrap_err(), "SOURCE_AUTHORITY_REFUSED");
        assert_eq!(trace.steps[1].action, "RETURNED_ERROR");
        assert_eq!(
            trace.steps[1].payload_digest,
            digest(b"SOURCE_AUTHORITY_REFUSED")
        );
    }
    #[test]
    fn a_new_predecessor_action_extends_but_never_rewrites_prior_observations() {
        let input = b"one";
        let mut trace = NativeTrace::new(input);
        trace
            .invoke("FIRST", input, |_| Ok(()), |_| digest(b"done"))
            .unwrap();
        let previous = trace.steps.clone();
        trace
            .invoke("SECOND", b"two", |_| Ok(()), |_| digest(b"done again"))
            .unwrap();
        assert_eq!(&trace.steps[..2], previous.as_slice());
        assert_eq!(trace.steps[2].predecessor, previous[1].step_digest);
        assert_eq!(trace.steps[2].sequence, 2);
    }
}
