use crate::lab_contracts::PrimitiveSplit;
use crate::types::ShadowError;
use crate::vault_transaction::sha256_parts;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

static GENERATION: AtomicU64 = AtomicU64::new(1);

type AnchorMap = HashMap<String, (u64, PrimitiveSplit)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorLeaseState {
    Leased,
    Committed,
    Restored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnchorLeaseEvidence {
    pub generation: u64,
    pub session_digest: [u8; 32],
    pub state: AnchorLeaseState,
    pub transaction_id: [u8; 32],
}

/// RAII lease: çdo dalje para commit-it e rikthen PA-në në pritje.
pub struct AnchorLease<'a> {
    store: &'a Mutex<AnchorMap>,
    session_id: String,
    session_digest: [u8; 32],
    material: Option<(u64, PrimitiveSplit)>,
    generation: u64,
    state: AnchorLeaseState,
}

impl<'a> AnchorLease<'a> {
    pub fn acquire(
        store: &'a Mutex<AnchorMap>,
        session_id: &str,
    ) -> Result<Self, ShadowError> {
        let material = store
            .lock()
            .map_err(|_| ShadowError::VaultWriteError("pa_waiting lock poisoned".to_string()))?
            .remove(session_id);
        Ok(Self {
            store,
            session_id: session_id.to_string(),
            session_digest: sha256_parts(&[b"ESSMAI_ANCHOR_LEASE_SESSION_V1", session_id.as_bytes()]),
            material,
            generation: GENERATION.fetch_add(1, Ordering::AcqRel),
            state: AnchorLeaseState::Leased,
        })
    }

    pub fn material(&self) -> Option<&(u64, PrimitiveSplit)> {
        self.material.as_ref()
    }

    pub fn anchor_id(&self) -> Option<u64> {
        self.material.as_ref().map(|(id, _)| *id)
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[cfg(test)]
    pub const fn state(&self) -> AnchorLeaseState {
        self.state
    }

    pub fn commit_consumption(
        mut self,
        transaction_id: [u8; 32],
    ) -> Result<AnchorLeaseEvidence, ShadowError> {
        self.material = None;
        self.state = AnchorLeaseState::Committed;
        Ok(AnchorLeaseEvidence {
            generation: self.generation,
            session_digest: self.session_digest,
            state: self.state,
            transaction_id,
        })
    }

    #[cfg(test)]
    pub fn restore(mut self, _reason: &str) -> Result<AnchorLeaseEvidence, ShadowError> {
        self.restore_inner()?;
        self.state = AnchorLeaseState::Restored;
        Ok(AnchorLeaseEvidence {
            generation: self.generation,
            session_digest: self.session_digest,
            state: self.state,
            transaction_id: [0u8; 32],
        })
    }

    fn restore_inner(&mut self) -> Result<(), ShadowError> {
        match self.material.take() {
            Some(material) => {
                self.store
                    .lock()
                    .map_err(|_| {
                        ShadowError::VaultWriteError(
                            "pa_waiting restore lock poisoned".to_string(),
                        )
                    })?
                    .insert(self.session_id.clone(), material);
                Ok(())
            }
            None => Ok(()),
        }
    }
}

impl Drop for AnchorLease<'_> {
    fn drop(&mut self) {
        match self.state {
            AnchorLeaseState::Leased => match self.restore_inner() {
                Ok(()) => {
                    self.state = AnchorLeaseState::Restored;
                }
                Err(error) => eprintln!("[PA_LEASE] restore failed: {error}"),
            },
            AnchorLeaseState::Committed | AnchorLeaseState::Restored => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split() -> PrimitiveSplit {
        PrimitiveSplit {
            xi: vec!["x".to_string()],
            yi: vec!["y".to_string()],
        }
    }

    #[test]
    fn drop_restores_uncommitted_anchor() {
        let store = Mutex::new(HashMap::from([("s".to_string(), (7, split()))]));
        {
            let lease = AnchorLease::acquire(&store, "s").expect("lease");
            assert_eq!(lease.state(), AnchorLeaseState::Leased);
            assert_eq!(lease.anchor_id(), Some(7));
            assert!(store.lock().expect("store").is_empty());
        }
        assert_eq!(store.lock().expect("store").get("s").map(|v| v.0), Some(7));
    }

    #[test]
    fn commit_consumes_exactly_once() {
        let store = Mutex::new(HashMap::from([("s".to_string(), (7, split()))]));
        let lease = AnchorLease::acquire(&store, "s").expect("lease");
        let evidence = lease
            .commit_consumption([9u8; 32])
            .expect("commit consumption");
        assert_eq!(evidence.state, AnchorLeaseState::Committed);
        assert_eq!(evidence.transaction_id, [9u8; 32]);
        assert!(store.lock().expect("store").is_empty());
    }

    #[test]
    fn explicit_restore_is_auditable() {
        let store = Mutex::new(HashMap::from([("s".to_string(), (7, split()))]));
        let lease = AnchorLease::acquire(&store, "s").expect("lease");
        let evidence = lease.restore("fault before commit").expect("restore");
        assert_eq!(evidence.state, AnchorLeaseState::Restored);
        assert_ne!(evidence.session_digest, [0u8; 32]);
        assert_eq!(store.lock().expect("store").get("s").map(|v| v.0), Some(7));
    }
}
