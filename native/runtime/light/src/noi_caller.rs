//! Host-owned activation; no delivery event grants authority or truth.
#[path = "../../zero-noi/caller.rs"]
mod delivery;
pub(crate) fn invoke<T>(site: &str, scope: &str, operation: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    delivery::invoke(site, scope, operation).map_err(|error| match error {
        delivery::InvocationError::Delivery(reason) => reason.into(),
        delivery::InvocationError::Operation(reason) => reason,
    })
}
#[cfg(test)]
pub(crate) use delivery::test_events;

#[cfg(test)]
mod tests {
    #[test] fn successful_dispatch_is_observed_without_changing_result() {
        super::test_events();
        let result=super::invoke("local", "scope", || Ok(7u8)).unwrap();
        assert_eq!(result,7);
        assert_eq!(super::test_events(),vec![("local".into(),"ENTER".into()),("local".into(),"RETURNED_OK".into())]);
    }
}
