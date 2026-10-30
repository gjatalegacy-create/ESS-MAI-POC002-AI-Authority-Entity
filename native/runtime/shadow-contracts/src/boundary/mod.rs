//! R15 / F2. Contracts whose bytes cross a process boundary, held once.
//! Light, Quantum and Shadow re-export these modules instead of keeping copies,
//! so the encoders and decoders on both sides of a boundary are one definition.
pub mod gcl_project;
pub mod living_trust;
pub mod pa_wire;
pub mod pd_continuum;
pub mod pd_spine;
pub mod split_wire;
pub mod verification_receipt;
