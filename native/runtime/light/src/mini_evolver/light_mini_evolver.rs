// src/mini_evolver/light_mini_evolver.rs
// _light_platform (Kumulativ)

// Light Mini Evolver - Complementary Layer (Faza 1 + 2)

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub struct KnowledgeUsageEvent {
    pub knowledge_id: u64,
    /// Referenca e rezultatit terminal që e prodhoi ngjarjen.
    pub evidence_ref: u64,
    /// Identiteti i plotë SHA-256 i evidencës. `None` lejohet vetëm për API-t
    /// historike compatibility; rruga autoritative e trust-it e refuzon.
    pub evidence_sha256: Option<String>,
    pub module_name: String,
    pub timestamp_ns: u64,
    pub linear_mass: f32,
    pub vector_score: f32,
    /// true vetëm në API-n e kontrolluar kur rezultati terminal mbijetoi.
    pub survived: bool,
}

pub struct LightMiniEvolver {
    usage_stats: HashMap<u64, Vec<KnowledgeUsageEvent>>,
    /// `u64` mbetet handle i API-së ekzistuese; ky regjistër ruan identitetin
    /// autoritativ SHA-256 dhe ndalon bashkimin e dy route-ve me të njëjtin
    /// prefiks 64-bit.
    route_sha256: HashMap<u64, String>,
    total_events: u64,
    enabled: bool,
}

impl LightMiniEvolver {
    pub fn new() -> Self {
        Self {
            usage_stats: HashMap::new(),
            route_sha256: HashMap::new(),
            total_events: 0,
            enabled: true,
        }
    }

    pub fn on_algorithm_step(
        &mut self,
        knowledge_id: u64,
        module_name: &str,
        linear_mass: f32,
        vector_score: f32,
    ) {
        match self.enabled {
            false => return,
            true => {}
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let event = KnowledgeUsageEvent {
            knowledge_id,
            evidence_ref: timestamp,
            evidence_sha256: None,
            module_name: module_name.to_string(),
            timestamp_ns: timestamp,
            linear_mass,
            vector_score,
            // API compatibility nuk provon një rezultat terminal.
            survived: false,
        };

        self.usage_stats
            .entry(knowledge_id)
            .or_insert_with(Vec::new)
            .push(event);

        self.total_events += 1;
    }

    /// Rruga autoritative e runtime-it. SHA-256 e plotë e route-it dhe e
    /// evidencës ruhen bashkë me handle-t `u64`; prefiksi nuk lejohet të bëhet
    /// identitet më vete.
    pub fn record_verified_cycle_bound(
        &mut self,
        knowledge_id: u64,
        route_sha256: &str,
        evidence_ref: u64,
        evidence_sha256: &str,
        module_name: &str,
        potentiality: f32,
        survived: bool,
    ) -> Result<(), &'static str> {
        let timestamp_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos().min(u128::from(u64::MAX)) as u64)
            .map_err(|_| "MINI_EVIDENCE_CLOCK_INVALID")?;
        self.record_verified_cycle_bound_at(
            knowledge_id,
            route_sha256,
            evidence_ref,
            evidence_sha256,
            module_name,
            potentiality,
            survived,
            timestamp_ns,
        )
    }

    /// Variant determinist i API-së së lidhur, për self-check dhe fault
    /// stimulation. Të gjitha validimet kryhen para mutacionit të ledger-it.
    #[allow(clippy::too_many_arguments)]
    pub fn record_verified_cycle_bound_at(
        &mut self,
        knowledge_id: u64,
        route_sha256: &str,
        evidence_ref: u64,
        evidence_sha256: &str,
        module_name: &str,
        potentiality: f32,
        survived: bool,
        timestamp_ns: u64,
    ) -> Result<(), &'static str> {
        self.record_cycle_at(
            knowledge_id,
            evidence_ref,
            Some((route_sha256, evidence_sha256)),
            module_name,
            potentiality,
            survived,
            timestamp_ns,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn record_cycle_at(
        &mut self,
        knowledge_id: u64,
        evidence_ref: u64,
        identities: Option<(&str, &str)>,
        module_name: &str,
        potentiality: f32,
        survived: bool,
        timestamp_ns: u64,
    ) -> Result<(), &'static str> {
        use crate::ess_mai_system::limits::{MAX_SIGNALS_ACTIVE, RING_BUFFER_SIZE};

        match self.enabled {
            true => {}
            false => return Err("MINI_EVIDENCE_DISABLED"),
        }
        match (knowledge_id, evidence_ref, timestamp_ns) {
            (0, _, _) => return Err("MINI_EVIDENCE_ROUTE_ZERO"),
            (_, 0, _) => return Err("MINI_EVIDENCE_REF_ZERO"),
            (_, _, 0) => return Err("MINI_EVIDENCE_TIME_ZERO"),
            _ => {}
        }
        match module_name.trim().is_empty() {
            true => return Err("MINI_EVIDENCE_DOMAIN_EMPTY"),
            false => {}
        }
        match potentiality.is_finite() && (0.0..=1.0).contains(&potentiality) {
            true => {}
            false => return Err("MINI_EVIDENCE_POTENTIALITY_INVALID"),
        }

        let route_exists = self.usage_stats.contains_key(&knowledge_id);
        match (route_exists, self.usage_stats.len() < MAX_SIGNALS_ACTIVE) {
            (true, _) | (false, true) => {}
            (false, false) => return Err("MINI_EVIDENCE_ROUTE_CAPACITY"),
        }

        let bound_evidence_sha256 = match identities {
            Some((route_sha256, evidence_sha256)) => {
                match (
                    Self::sha256_handle(route_sha256),
                    Self::sha256_handle(evidence_sha256),
                ) {
                    (Err(reason), _) | (_, Err(reason)) => return Err(reason),
                    (Ok(route_handle), Ok(evidence_handle)) => {
                        match (
                            route_handle == knowledge_id,
                            evidence_handle == evidence_ref,
                        ) {
                            (true, true) => {}
                            (false, _) => return Err("MINI_EVIDENCE_ROUTE_SHA256_HANDLE_MISMATCH"),
                            (_, false) => return Err("MINI_EVIDENCE_SHA256_HANDLE_MISMATCH"),
                        }
                    }
                }

                match self.route_sha256.get(&knowledge_id) {
                    Some(bound) => match bound.as_str() == route_sha256 {
                        true => {}
                        false => return Err("MINI_EVIDENCE_ROUTE_SHA256_COLLISION"),
                    },
                    None => match self.usage_stats.get(&knowledge_id) {
                        Some(events) => match events.is_empty() {
                            true => {}
                            false => return Err("MINI_EVIDENCE_UNBOUND_ROUTE_HISTORY"),
                        },
                        None => {}
                    },
                }

                let same_ref = self.usage_stats.get(&knowledge_id).and_then(|events| {
                    events
                        .iter()
                        .find(|event| event.evidence_ref == evidence_ref)
                });
                match same_ref {
                    Some(event) => match (
                        event.evidence_sha256.as_deref() == Some(evidence_sha256),
                        event.evidence_sha256.is_some(),
                    ) {
                        (true, _) => return Err("MINI_EVIDENCE_REPLAY"),
                        (false, true) => return Err("MINI_EVIDENCE_SHA256_COLLISION"),
                        (false, false) => return Err("MINI_EVIDENCE_UNBOUND_EVIDENCE_HISTORY"),
                    },
                    None => {}
                }
                Some(evidence_sha256.to_string())
            }
            None => {
                match self.route_sha256.contains_key(&knowledge_id) {
                    true => return Err("MINI_EVIDENCE_BOUND_ROUTE_REQUIRES_SHA256"),
                    false => {}
                }
                let replayed = self
                    .usage_stats
                    .get(&knowledge_id)
                    .map(|events| {
                        events
                            .iter()
                            .any(|event| event.evidence_ref == evidence_ref)
                    })
                    .unwrap_or(false);
                match replayed {
                    true => return Err("MINI_EVIDENCE_REPLAY"),
                    false => {}
                }
                None
            }
        };

        match identities {
            Some((route_sha256, _)) => {
                self.route_sha256
                    .entry(knowledge_id)
                    .or_insert_with(|| route_sha256.to_string());
            }
            None => {}
        }
        let events = self
            .usage_stats
            .entry(knowledge_id)
            .or_insert_with(Vec::new);
        match events.len() >= RING_BUFFER_SIZE {
            true => {
                events.remove(0);
            }
            false => {}
        }
        events.push(KnowledgeUsageEvent {
            knowledge_id,
            evidence_ref,
            evidence_sha256: bound_evidence_sha256,
            module_name: module_name.trim().to_string(),
            timestamp_ns,
            linear_mass: potentiality,
            vector_score: u8::from(survived) as f32,
            survived,
        });
        self.total_events = self.total_events.saturating_add(1);
        Ok(())
    }

    fn sha256_handle(value: &str) -> Result<u64, &'static str> {
        match crate::lgc_algorithm::is_canonical_sha256(value) {
            true => {}
            false => return Err("MINI_EVIDENCE_SHA256_INVALID"),
        }
        u64::from_str_radix(&value[..16], 16).map_err(|_| "MINI_EVIDENCE_SHA256_HANDLE_DECODE")
    }

    pub fn get_usage_count(&self, knowledge_id: u64) -> usize {
        self.usage_stats
            .get(&knowledge_id)
            .map(|v| v.len())
            .unwrap_or(0)
    }

    pub fn get_average_vector_score(&self, knowledge_id: u64) -> Option<f32> {
        let events = self.usage_stats.get(&knowledge_id)?;
        match events.is_empty() {
            true => return None,
            false => {}
        }
        Some(events.iter().map(|e| e.vector_score).sum::<f32>() / events.len() as f32)
    }

    pub fn total_events(&self) -> u64 {
        self.total_events
    }

    /// Ngjarjet e ruajtura për një rrugë, në rendin e mbërritjes.
    pub fn events_for(&self, knowledge_id: u64) -> Option<&[KnowledgeUsageEvent]> {
        self.usage_stats.get(&knowledge_id).map(Vec::as_slice)
    }

    /// Identiteti i plotë i route-it që zotëron këtë handle.
    pub fn route_sha256(&self, knowledge_id: u64) -> Option<&str> {
        self.route_sha256.get(&knowledge_id).map(String::as_str)
    }

    /// Identiteti i plotë i evidencës së mbajtur në dritaren aktive.
    pub fn evidence_sha256(&self, knowledge_id: u64, evidence_ref: u64) -> Option<&str> {
        self.usage_stats
            .get(&knowledge_id)?
            .iter()
            .find(|event| event.evidence_ref == evidence_ref)?
            .evidence_sha256
            .as_deref()
    }
}

impl Default for LightMiniEvolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ess_mai_system::limits::RING_BUFFER_SIZE;

    fn full_identity(handle: u64, tail_byte: u8) -> String {
        format!("{handle:016x}{}", format!("{tail_byte:02x}").repeat(24))
    }

    #[test]
    fn runtime_evidence_rejects_replay_and_invalid_mass() {
        let mut mini = LightMiniEvolver::new();
        mini.record_verified_cycle_at(7, 11, "work", 0.8, true, 100)
            .expect("first terminal evidence");
        assert_eq!(
            mini.record_verified_cycle_at(7, 11, "work", 0.8, true, 101),
            Err("MINI_EVIDENCE_REPLAY")
        );
        assert_eq!(
            mini.record_verified_cycle_at(7, 12, "work", f32::NAN, true, 102),
            Err("MINI_EVIDENCE_POTENTIALITY_INVALID")
        );
        assert_eq!(mini.get_usage_count(7), 1);
    }

    #[test]
    fn runtime_evidence_retains_the_bounded_recent_window() {
        let mut mini = LightMiniEvolver::new();
        for index in 0..(RING_BUFFER_SIZE + 1) {
            mini.record_verified_cycle_at(
                7,
                index as u64 + 1,
                "work",
                0.5,
                index % 2 == 0,
                index as u64 + 1,
            )
            .expect("bounded terminal evidence");
        }
        let events = mini.events_for(7).expect("route history");
        assert_eq!(events.len(), RING_BUFFER_SIZE);
        assert_eq!(events.first().map(|event| event.evidence_ref), Some(2));
        assert_eq!(
            events.last().map(|event| event.evidence_ref),
            Some(RING_BUFFER_SIZE as u64 + 1)
        );
    }

    #[test]
    fn bound_runtime_evidence_keeps_full_sha256_behind_u64_handles() {
        let mut mini = LightMiniEvolver::new();
        let route = full_identity(7, 0x22);
        let evidence = full_identity(11, 0x33);
        mini.record_verified_cycle_bound_at(7, &route, 11, &evidence, "work", 0.8, true, 100)
            .expect("full identities must bind");
        assert_eq!(mini.route_sha256(7), Some(route.as_str()));
        assert_eq!(mini.evidence_sha256(7, 11), Some(evidence.as_str()));
        assert_eq!(
            mini.record_verified_cycle_at(7, 12, "work", 0.8, true, 101),
            Err("MINI_EVIDENCE_BOUND_ROUTE_REQUIRES_SHA256")
        );
    }

    #[test]
    fn full_sha256_collisions_and_unbound_history_fail_closed() {
        let mut mini = LightMiniEvolver::new();
        let route = full_identity(7, 0x22);
        let route_collision = full_identity(7, 0x44);
        let evidence = full_identity(11, 0x33);
        let evidence_collision = full_identity(11, 0x55);
        let next_evidence = full_identity(12, 0x66);
        mini.record_verified_cycle_bound_at(7, &route, 11, &evidence, "work", 0.8, true, 100)
            .expect("first full identity");
        assert_eq!(
            mini.record_verified_cycle_bound_at(
                7,
                &route_collision,
                12,
                &next_evidence,
                "work",
                0.8,
                true,
                101,
            ),
            Err("MINI_EVIDENCE_ROUTE_SHA256_COLLISION")
        );
        assert_eq!(
            mini.record_verified_cycle_bound_at(
                7,
                &route,
                11,
                &evidence_collision,
                "work",
                0.8,
                true,
                102,
            ),
            Err("MINI_EVIDENCE_SHA256_COLLISION")
        );

        let mut legacy = LightMiniEvolver::new();
        legacy
            .record_verified_cycle_at(7, 11, "compat", 0.5, false, 100)
            .expect("compatibility event");
        assert_eq!(
            legacy.record_verified_cycle_bound_at(
                7,
                &route,
                12,
                &next_evidence,
                "work",
                0.8,
                true,
                101,
            ),
            Err("MINI_EVIDENCE_UNBOUND_ROUTE_HISTORY")
        );
    }
}

#[cfg(test)]
impl LightMiniEvolver {
    pub fn record_verified_cycle_at(
        &mut self,
        knowledge_id: u64,
        evidence_ref: u64,
        module_name: &str,
        potentiality: f32,
        survived: bool,
        timestamp_ns: u64,
    ) -> Result<(), &'static str> {
        self.record_cycle_at(
            knowledge_id,
            evidence_ref,
            None,
            module_name,
            potentiality,
            survived,
            timestamp_ns,
        )
    }
}
