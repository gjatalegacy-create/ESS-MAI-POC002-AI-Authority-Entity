//! Caller-owned delivery accounting. It does NOT belong to 0-NOI's primitive
//! and never authorises an operation. The invoked native function must perform
//! its own existing law/evidence/authority checks. Failure is not swallowed.
#[cfg(test)]
thread_local! { static EVENTS: std::cell::RefCell<Vec<(String, String)>> = const { std::cell::RefCell::new(Vec::new()) }; }
#[cfg(test)]
fn event(site: &str, phase: &str) { EVENTS.with(|events| events.borrow_mut().push((site.into(), phase.into()))); }
#[cfg(test)]
pub fn test_events() -> Vec<(String, String)> { EVENTS.with(|events| std::mem::take(&mut *events.borrow_mut())) }
use std::sync::atomic::{AtomicU64, Ordering};
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Occurrence { pub site: String, pub scope: String, pub sequence: u64, pub process: u32 }
#[derive(Default)]
pub struct Delivery { delivered: std::collections::BTreeSet<(u32, u64)> }
impl Delivery {
    pub fn accept(&mut self, o: &Occurrence, site: &str, scope: &str) -> Result<(), &'static str> {
        match o.site == site && o.scope == scope && o.sequence != 0 && o.process == std::process::id() {
            true => {}, false => return Err("NOI_CALLER_SCOPE_MISMATCH"),
        }
        match self.delivered.insert((o.process, o.sequence)) { true => Ok(()), false => Err("NOI_CALLER_REDELIVERY") }
    }
}
fn next(sequence: &AtomicU64) -> Result<u64, &'static str> {
    sequence.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
        .map(|v| v + 1).map_err(|_| "NOI_OCCURRENCE_EXHAUSTED")
}
fn try_observe(site: &str, scope: &str) -> Result<Occurrence, &'static str> {
    match !site.is_empty() && !scope.is_empty() && site.len() <= 1024 && scope.len() <= 65536
        && !site.chars().chain(scope.chars()).any(char::is_control) {
        true => {}, false => return Err("NOI_CALLER_INVALID_SCOPE"),
    }
    let request = (site, scope);
    let pulse = ess_mai_zero_noi::trigger(&request);
    let _borrow = pulse.request();
    let sequence = next(&SEQUENCE)?;
    let o = Occurrence { site: site.into(), scope: scope.into(), sequence, process: std::process::id() };
    let v = pulse.vector();
    eprintln!("[0-NOI] site={} scope={} pid={} occurrence={} trigger={} authority={} evidence=0 verdict={} persistence={}",
        site, scope, o.process, sequence, v.trigger, v.authority, v.verdict, v.persistence);
    Ok(o)
}
/// Compatibility diagnostic only: unsuccessful observation returns sequence 0,
/// which Delivery rejects. Use invoke() for an actual delivered callback.
#[cfg(test)]
pub fn observe(site: &str, scope: &str) -> Occurrence {
    match try_observe(site, scope) {
        Ok(o) => o,
        Err(reason) => {
            eprintln!("[0-NOI] diagnostic_not_delivered reason={reason} authority=0");
            Occurrence { site: site.into(), scope: scope.into(), sequence: 0, process: std::process::id() }
        }
    }
}
#[derive(Debug)]
pub enum InvocationError<E> { Delivery(&'static str), Operation(E) }
impl<E: std::fmt::Display> std::fmt::Display for InvocationError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self { Self::Delivery(e) => write!(f, "{e}"), Self::Operation(e) => write!(f, "{e}") }
    }
}
/// Delivery causally invokes the existing operation exactly once in this call.
/// FnOnce is caller-owned; 0-NOI neither inspects the payload nor decides success.
pub fn invoke<T, E>(site: &str, scope: &str, operation: impl FnOnce() -> Result<T, E>) -> Result<T, InvocationError<E>> {
    let occurrence = try_observe(site, scope).map_err(InvocationError::Delivery)?;
    let mut delivery = Delivery::default();
    delivery.accept(&occurrence, site, scope).map_err(InvocationError::Delivery)?;
    eprintln!("[NOI_DELIVERY] site={} occurrence={} callback=ENTER authority=0 pid={}", site, occurrence.sequence, occurrence.process);
    #[cfg(test)] event(site, "ENTER");
    let result = operation().map_err(InvocationError::Operation);
    #[cfg(test)] event(site, match result.is_ok() { true => "RETURNED_OK", false => "RETURNED_ERROR" });
    eprintln!("[NOI_DELIVERY] site={} occurrence={} callback={} authority=0 pid={}", site, occurrence.sequence,
        match result.is_ok() { true => "RETURNED_OK", false => "RETURNED_ERROR" }, occurrence.process);
    result
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn completion_is_not_claimed_on_error_or_panic() {
        test_events();
        let r: Result<(), InvocationError<&str>> = invoke("error", "scope", || Err("missing evidence"));
        assert!(matches!(r, Err(InvocationError::Operation("missing evidence"))));
        assert_eq!(test_events(), vec![("error".into(), "ENTER".into()), ("error".into(), "RETURNED_ERROR".into())]);
        let r = std::panic::catch_unwind(|| {
            let _: Result<(), InvocationError<&str>> = invoke("panic", "scope", || panic!("local regression"));
        });
        assert!(r.is_err());
        assert_eq!(test_events(), vec![("panic".into(), "ENTER".into())]);
    }

    #[test] fn repeated_invocations_are_not_redeliveries() {
        let mut d = Delivery::default(); let first = observe("D-BAB", "s"); d.accept(&first, "D-BAB", "s").unwrap();
        for _ in 0..100 { let o = observe("D-BAB", "s"); d.accept(&o, "D-BAB", "s").unwrap(); }
        assert!(d.accept(&first, "D-BAB", "s").is_err());
    }
    #[test] fn wrong_scope_does_not_consume_occurrence() {
        let o = observe("BESA", "a"); let mut d = Delivery::default();
        assert!(d.accept(&o, "BESA", "b").is_err()); assert!(d.accept(&o, "BESA", "a").is_ok());
    }
    #[test] fn zero_sequence_is_never_delivered() { let o = Occurrence { site:"s".into(), scope:"a".into(), sequence:0, process:std::process::id() }; assert!(Delivery::default().accept(&o,"s","a").is_err()); }
    #[test] fn exhausted_sequence_never_wraps() { let sequence=AtomicU64::new(u64::MAX);assert!(next(&sequence).is_err());assert_eq!(sequence.load(Ordering::Relaxed),u64::MAX); }
    #[test] fn last_sequence_delivered_once() { let sequence=AtomicU64::new(u64::MAX-1);assert_eq!(next(&sequence).unwrap(),u64::MAX);assert!(next(&sequence).is_err()); }
    #[test] fn real_callback_is_consumed_once() { let mut n=0;let r:Result<u8,InvocationError<&str>>=invoke("D-BAB","s",||{n+=1;Ok(7)});assert_eq!(r.unwrap(),7);assert_eq!(n,1); }
    #[test] fn callback_failure_stays_failure() { let r:Result<(),InvocationError<&str>>=invoke("D-BAB","s",||Err("SOURCE_GAP"));assert!(matches!(r,Err(InvocationError::Operation("SOURCE_GAP")))); }
    #[test] fn invalid_scope_never_runs_callback() { let mut n=0;let r:Result<(),InvocationError<&str>>=invoke("D-BAB","",||{n+=1;Ok(())});assert!(r.is_err());assert_eq!(n,0); }
}
