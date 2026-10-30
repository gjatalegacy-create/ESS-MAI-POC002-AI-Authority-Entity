//! One neutral primitive. No state, hashes, callbacks, I/O or authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NeutralVector {
    pub state: u8, pub knowledge_creation: u8, pub verdict: u8,
    pub authority: u8, pub persistence: u8, pub trigger: u8,
}
impl NeutralVector {
    pub const fn canonical() -> Self {
        Self { state: 0, knowledge_creation: 0, verdict: 0, authority: 0, persistence: 0, trigger: 1 }
    }
}
#[derive(Debug)]
pub struct Pulse<'a, T: ?Sized> { request: &'a T }
impl<'a, T: ?Sized> Pulse<'a,T> {
    pub fn request(&self) -> &'a T { self.request }
    pub const fn vector(&self) -> NeutralVector { NeutralVector::canonical() }
}
pub fn trigger<T: ?Sized>(request: &T) -> Pulse<'_, T> { Pulse { request } }
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn no_mutation_no_authority() {
        let request=String::from("MISSING_EVIDENCE");
        for _ in 0..1000 { let p=trigger(&request); assert!(core::ptr::eq(p.request(), &request));
            assert_eq!(p.vector(), NeutralVector::canonical()); }
        assert_eq!(request,"MISSING_EVIDENCE");
    }
    #[test] fn no_payload_interpretation() { let p=trigger(&[255u8;7]); assert_eq!(p.vector().authority,0); }
}
