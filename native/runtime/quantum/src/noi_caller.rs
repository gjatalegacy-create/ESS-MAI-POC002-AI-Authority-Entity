// Shared caller-owned delivery; the neutral primitive remains stateless.
#[path = "../../zero-noi/caller.rs"]
mod delivery;
pub use delivery::{invoke, InvocationError};
