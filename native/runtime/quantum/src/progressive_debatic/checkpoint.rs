//! V189_DERIVATIVE: bounded, canonical per-session PD state checkpoint.
//! This is operational data, not a Shadow receipt, authority token or knowledge.
//! The caller must bind these bytes to the same-WAL committed mp_pd record.
//! Schema 1 uses deterministic genius tie policy 1; no legacy snapshot migration.
//! Import is atomic and only accepts a fresh default engine (no session overwrite).

use super::core::{extract_signal, update_trace, PdSession, ProgressiveDebatic};
use super::genius_detector::{GeniusDetector, GENIUS_TIE_POLICY_VERSION};
use super::types::*;
use crate::pd_spine_contract::MASS_SCALE;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub const PD_CHECKPOINT_SCHEMA: u32 = 1;
pub const PD_CHECKPOINT_MAX_BYTES: usize = 8 * 1024 * 1024;
pub const PD_CHECKPOINT_MAX_TURNS: usize = 4096;
const MAGIC: &[u8; 8] = b"ESSPDCK1";
const MAX_STRING: usize = 64 * 1024;
const MAX_ITEMS: usize = 16 * 1024;
const MAX_TOTAL_ITEMS: usize = 128 * 1024;

fn require(ok: bool, reason: &str) -> Result<(), String> {
    match ok {
        true => Ok(()),
        false => Err(format!("PD checkpoint: {reason}")),
    }
}

struct Encoder {
    bytes: Vec<u8>,
    items: usize,
}
impl Encoder {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            items: 0,
        }
    }
    fn raw(&mut self, bytes: &[u8]) -> Result<(), String> {
        let next = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or_else(|| "PD checkpoint: size overflow".to_string())?;
        require(next <= PD_CHECKPOINT_MAX_BYTES - 32, "encoded size limit")?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn count(&mut self, len: usize) -> Result<(), String> {
        self.items = self
            .items
            .checked_add(len)
            .ok_or_else(|| "PD checkpoint: item count overflow".to_string())?;
        require(
            len <= MAX_ITEMS && self.items <= MAX_TOTAL_ITEMS,
            "collection limit",
        )?;
        (len as u32).put(self)
    }
}
struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
    items: usize,
}
impl<'a> Decoder<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            offset: 0,
            items: 0,
        }
    }
    fn take(&mut self, len: usize) -> Result<&'a [u8], String> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| "PD checkpoint: offset overflow".to_string())?;
        let out = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| "PD checkpoint: truncated input".to_string())?;
        self.offset = end;
        Ok(out)
    }
    fn count(&mut self) -> Result<usize, String> {
        let len = u32::get(self)? as usize;
        self.items = self
            .items
            .checked_add(len)
            .ok_or_else(|| "PD checkpoint: item count overflow".to_string())?;
        require(
            len <= MAX_ITEMS && self.items <= MAX_TOTAL_ITEMS,
            "collection limit",
        )?;
        require(
            len <= self.bytes.len().saturating_sub(self.offset),
            "unbacked collection length",
        )?;
        Ok(len)
    }
}
trait Wire: Sized {
    fn put(&self, out: &mut Encoder) -> Result<(), String>;
    fn get(input: &mut Decoder<'_>) -> Result<Self, String>;
}
macro_rules! integer_wire {
    ($ty:ty, $n:expr) => {
        impl Wire for $ty {
            fn put(&self, out: &mut Encoder) -> Result<(), String> {
                out.raw(&self.to_le_bytes())
            }
            fn get(input: &mut Decoder<'_>) -> Result<Self, String> {
                let mut bytes = [0u8; $n];
                bytes.copy_from_slice(input.take($n)?);
                Ok(<$ty>::from_le_bytes(bytes))
            }
        }
    };
}
integer_wire!(u8, 1);
integer_wire!(u32, 4);
integer_wire!(u64, 8);
impl Wire for bool {
    fn put(&self, out: &mut Encoder) -> Result<(), String> {
        u8::from(*self).put(out)
    }
    fn get(input: &mut Decoder<'_>) -> Result<Self, String> {
        match u8::get(input)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err("PD checkpoint: invalid boolean".to_string()),
        }
    }
}
impl Wire for String {
    fn put(&self, out: &mut Encoder) -> Result<(), String> {
        require(self.len() <= MAX_STRING, "string limit")?;
        (self.len() as u32).put(out)?;
        out.raw(self.as_bytes())
    }
    fn get(input: &mut Decoder<'_>) -> Result<Self, String> {
        let len = u32::get(input)? as usize;
        require(len <= MAX_STRING, "string limit")?;
        String::from_utf8(input.take(len)?.to_vec())
            .map_err(|_| "PD checkpoint: invalid UTF-8".to_string())
    }
}
impl<T: Wire> Wire for Vec<T> {
    fn put(&self, out: &mut Encoder) -> Result<(), String> {
        out.count(self.len())?;
        for value in self {
            value.put(out)?;
        }
        Ok(())
    }
    fn get(input: &mut Decoder<'_>) -> Result<Self, String> {
        let len = input.count()?;
        let mut out = Vec::with_capacity(len);
        for _ in 0..len {
            out.push(T::get(input)?);
        }
        Ok(out)
    }
}
impl<T: Wire> Wire for Option<T> {
    fn put(&self, out: &mut Encoder) -> Result<(), String> {
        match self {
            None => 0u8.put(out),
            Some(value) => {
                1u8.put(out)?;
                value.put(out)
            }
        }
    }
    fn get(input: &mut Decoder<'_>) -> Result<Self, String> {
        match u8::get(input)? {
            0 => Ok(None),
            1 => Ok(Some(T::get(input)?)),
            _ => Err("PD checkpoint: invalid option tag".to_string()),
        }
    }
}
impl Wire for (String, String) {
    fn put(&self, out: &mut Encoder) -> Result<(), String> {
        self.0.put(out)?;
        self.1.put(out)
    }
    fn get(input: &mut Decoder<'_>) -> Result<Self, String> {
        Ok((String::get(input)?, String::get(input)?))
    }
}
impl Wire for HashMap<String, u32> {
    fn put(&self, out: &mut Encoder) -> Result<(), String> {
        let mut entries: Vec<_> = self.iter().collect();
        entries.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        out.count(entries.len())?;
        for (key, count) in entries {
            key.put(out)?;
            count.put(out)?;
        }
        Ok(())
    }
    fn get(input: &mut Decoder<'_>) -> Result<Self, String> {
        let len = input.count()?;
        let mut out = HashMap::with_capacity(len);
        let mut previous: Option<String> = None;
        for _ in 0..len {
            let key = String::get(input)?;
            let count = u32::get(input)?;
            require(
                previous
                    .as_ref()
                    .map(|p| p.as_bytes() < key.as_bytes())
                    .unwrap_or(true),
                "duplicate or unordered map key",
            )?;
            require(
                !key.is_empty() && count != 0,
                "empty map key or zero frequency",
            )?;
            previous = Some(key.clone());
            out.insert(key, count);
        }
        Ok(out)
    }
}
macro_rules! enum_wire {
    ($ty:ty, $($tag:literal => $variant:path),+ $(,)?) => {
        impl Wire for $ty {
            fn put(&self, out: &mut Encoder) -> Result<(), String> {
                let tag: u8 = match self { $($variant => $tag),+ };
                tag.put(out)
            }
            fn get(input: &mut Decoder<'_>) -> Result<Self, String> {
                match u8::get(input)? { $($tag => Ok($variant)),+, _ => Err("PD checkpoint: unknown enum tag".to_string()) }
            }
        }
    };
}
enum_wire!(DebateMode, 0 => DebateMode::Intellect, 1 => DebateMode::Philosophy,
    2 => DebateMode::Scientific, 3 => DebateMode::Novel);
enum_wire!(PdResponseType, 0 => PdResponseType::Help, 1 => PdResponseType::Orient,
    2 => PdResponseType::Clarify, 3 => PdResponseType::ChallengeGentle,
    4 => PdResponseType::ChallengeWithFacts, 5 => PdResponseType::ScaffoldBullet,
    6 => PdResponseType::ScaffoldMeasure, 7 => PdResponseType::ScaffoldStructure,
    8 => PdResponseType::Listen);
macro_rules! struct_wire {
    ($ty:ty { $($field:ident),+ $(,)? }) => {
        impl Wire for $ty {
            fn put(&self, out: &mut Encoder) -> Result<(), String> { $(self.$field.put(out)?;)+ Ok(()) }
            fn get(input: &mut Decoder<'_>) -> Result<Self, String> { Ok(Self { $($field: Wire::get(input)?),+ }) }
        }
    };
}
struct_wire!(GeniusDetector {
    min_turns_before_check,
    weight_returns,
    weight_energy,
    weight_shape,
    genius_threshold
});
struct_wire!(CognitiveSignal {
    signal_id,
    turn_index,
    raw_content,
    content_length,
    idea_density,
    energy_level,
    concept_refs,
    contradicts,
    returns_to,
    timestamp
});
struct_wire!(EpistemicTrace {
    trace_id,
    session_id,
    signals,
    total_turns,
    dominant_concept,
    concept_map,
    energy_trajectory,
    idea_evolution,
    accumulated_mass,
    structural_coherence,
    implicit_primitives,
    contradiction_pairs,
    territory,
    started_at,
    last_updated_at
});
struct_wire!(GeniusSignal {
    signal_id,
    session_id,
    returns_to_idea,
    energy_increasing,
    concept_gaining_shape,
    returns_score,
    energy_score,
    shape_score,
    genius_score,
    detected_concept,
    detected_at_turn,
    trace_ref,
    timestamp
});
struct_wire!(PressureBudget {
    challenges_issued,
    current_mode,
    budget_exhausted,
    user_resistance_detected
});
struct_wire!(PdTurn {
    turn_id,
    turn_index,
    user_signal,
    pd_response_type,
    pd_response_hint,
    challenge_issued,
    mode_at_turn,
    genius_check
});
struct_wire!(PdSession {
    session_id,
    active,
    current_mode,
    pressure_budget,
    turns,
    trace,
    genius_signal,
    genius_sent_to_pim,
    territory,
    started_at,
    last_activity_at,
    total_turns
});

fn wire_bytes<T: Wire>(value: &T) -> Result<Vec<u8>, String> {
    let mut out = Encoder::new();
    value.put(&mut out)?;
    Ok(out.bytes)
}
fn validate_detector(value: &GeniusDetector) -> Result<(), String> {
    require(
        value.min_turns_before_check <= PD_CHECKPOINT_MAX_TURNS as u32,
        "detector turn limit",
    )?;
    require(
        [
            value.weight_returns,
            value.weight_energy,
            value.weight_shape,
            value.genius_threshold,
        ]
        .iter()
        .all(|v| *v <= MASS_SCALE),
        "detector fixed mass range",
    )?;
    require(
        value.weight_returns != 0 || value.weight_energy != 0 || value.weight_shape != 0,
        "zero detector weights",
    )
}
fn validate_session(session: &PdSession) -> Result<(), String> {
    require(
        !session.session_id.is_empty() && session.session_id.len() <= 256,
        "session identity",
    )?;
    require(
        !session.territory.is_empty() && session.territory.len() <= 256,
        "territory scope",
    )?;
    require(session.turns.len() <= PD_CHECKPOINT_MAX_TURNS, "turn limit")?;
    let n = session.turns.len();
    require(
        session.total_turns as usize == n
            && session.trace.total_turns as usize == n
            && session.trace.signals.len() == n,
        "turn/signal counts disagree",
    )?;
    require(
        session.trace.session_id == session.session_id
            && session.trace.territory == session.territory
            && session.trace.started_at == session.started_at,
        "trace scope mismatch",
    )?;
    require(
        session.trace.trace_id == pd_id(&[session.session_id.as_bytes(), b"trace"]),
        "trace identity mismatch",
    )?;
    require(
        session.pressure_budget.current_mode == session.current_mode,
        "budget mode mismatch",
    )?;
    let challenges = session.turns.iter().filter(|t| t.challenge_issued).count() as u32;
    require(
        session.pressure_budget.challenges_issued == challenges,
        "challenge count mismatch",
    )?;
    require(
        session.pressure_budget.budget_exhausted
            == (challenges >= PressureBudget::max_for(session.current_mode)),
        "exhaustion flag mismatch",
    )?;
    let mut rebuilt =
        EpistemicTrace::new(&session.session_id, &session.territory, session.started_at);
    for (index, (turn, signal)) in session.turns.iter().zip(&session.trace.signals).enumerate() {
        require(
            turn.turn_index as usize == index && signal.turn_index as usize == index,
            "non-contiguous turn index",
        )?;
        require(
            turn.turn_id
                == pd_id(&[
                    session.session_id.as_bytes(),
                    &(index as u32).to_le_bytes(),
                    b"turn",
                ]),
            "turn identity mismatch",
        )?;
        require(
            turn.challenge_issued == turn.pd_response_type.is_challenge(),
            "challenge response mismatch",
        )?;
        require(
            turn.genius_check.map(|v| v <= MASS_SCALE).unwrap_or(true),
            "genius turn score range",
        )?;
        require(
            wire_bytes(&turn.user_signal)? == wire_bytes(signal)?,
            "turn/signal material mismatch",
        )?;
        let expected = extract_signal(
            &signal.raw_content,
            index as u32,
            signal.timestamp,
            &rebuilt.concept_map,
        );
        require(
            wire_bytes(signal)? == wire_bytes(&expected)?,
            "signal does not derive from raw material",
        )?;
        update_trace(&mut rebuilt, signal);
    }
    require(
        wire_bytes(&rebuilt)? == wire_bytes(&session.trace)?,
        "derived trace mismatch",
    )?;
    require(
        session.last_activity_at == session.trace.last_updated_at,
        "last activity mismatch",
    )?;
    let last_genius_turn = session
        .turns
        .iter()
        .rev()
        .find(|turn| turn.genius_check.is_some());
    match (&session.genius_signal, last_genius_turn) {
        (None, None) => require(!session.genius_sent_to_pim, "genius sent without a signal")?,
        (Some(genius), Some(turn)) => {
            require(
                genius.session_id == session.session_id
                    && genius.trace_ref == session.trace.trace_id,
                "genius scope mismatch",
            )?;
            require(
                genius.detected_at_turn == turn.turn_index + 1
                    && genius.timestamp == turn.user_signal.timestamp
                    && Some(genius.genius_score) == turn.genius_check,
                "genius latest-turn mismatch",
            )?;
            require(
                genius.signal_id
                    == pd_id(&[
                        session.session_id.as_bytes(),
                        &genius.timestamp.to_le_bytes(),
                        b"genius",
                    ]),
                "genius identity mismatch",
            )?;
            require(
                [
                    genius.returns_score,
                    genius.energy_score,
                    genius.shape_score,
                    genius.genius_score,
                ]
                .iter()
                .all(|v| *v <= MASS_SCALE),
                "genius score range",
            )?;
            require(
                session
                    .trace
                    .concept_map
                    .contains_key(&genius.detected_concept),
                "unknown genius concept",
            )?;
            require(
                [
                    genius.returns_to_idea,
                    genius.energy_increasing,
                    genius.concept_gaining_shape,
                ]
                .iter()
                .filter(|v| **v)
                .count()
                    >= 2,
                "insufficient genius signals",
            )?;
        }
        _ => return Err("PD checkpoint: missing or unbacked genius signal".to_string()),
    }
    Ok(())
}

impl ProgressiveDebatic {
    /// Export one complete session plus shared engine configuration/active flag.
    /// Other sessions are not part of this explicitly per-session snapshot.
    /// SHA256 is corruption detection, not authentication or knowledge authority.
    pub fn export_checkpoint(&self, session_id: &str) -> Result<Vec<u8>, String> {
        let session = self
            .sessions
            .get(session_id)
            .ok_or_else(|| "PD checkpoint: session not found".to_string())?;
        require(session.session_id == session_id, "session map key mismatch")?;
        validate_detector(&self.detector)?;
        validate_session(session)?;
        let mut out = Encoder::new();
        out.raw(MAGIC)?;
        PD_CHECKPOINT_SCHEMA.put(&mut out)?;
        GENIUS_TIE_POLICY_VERSION.put(&mut out)?;
        self.active.put(&mut out)?;
        self.detector.put(&mut out)?;
        session.put(&mut out)?;
        let digest = Sha256::digest(&out.bytes);
        out.bytes.extend_from_slice(&digest);
        Ok(out.bytes)
    }

    /// Restore only into a fresh default engine; never overwrite live sessions,
    /// narrow/reset the budget, call open_session or recreate an authority token.
    /// All parsing/validation completes before the single assignment to self.
    pub fn import_checkpoint(
        &mut self,
        expected_session_id: &str,
        bytes: &[u8],
    ) -> Result<(), String> {
        require(
            self.sessions.is_empty()
                && self.active
                && wire_bytes(&self.detector)? == wire_bytes(&GeniusDetector::default())?,
            "target is not a fresh default engine",
        )?;
        require(
            !expected_session_id.is_empty() && expected_session_id.len() <= 256,
            "expected session identity",
        )?;
        require(
            bytes.len() >= 48 && bytes.len() <= PD_CHECKPOINT_MAX_BYTES,
            "checkpoint size limit",
        )?;
        let (payload, supplied_digest) = bytes.split_at(bytes.len() - 32);
        require(
            Sha256::digest(payload).as_slice() == supplied_digest,
            "checksum mismatch",
        )?;
        let mut input = Decoder::new(payload);
        require(input.take(MAGIC.len())? == MAGIC, "magic mismatch")?;
        require(
            u32::get(&mut input)? == PD_CHECKPOINT_SCHEMA,
            "unsupported schema",
        )?;
        require(
            u32::get(&mut input)? == GENIUS_TIE_POLICY_VERSION,
            "unsupported genius tie policy",
        )?;
        let active = bool::get(&mut input)?;
        let detector = GeniusDetector::get(&mut input)?;
        let session = PdSession::get(&mut input)?;
        require(input.offset == payload.len(), "trailing input")?;
        require(
            session.session_id == expected_session_id,
            "expected session mismatch",
        )?;
        validate_detector(&detector)?;
        validate_session(&session)?;
        let mut sessions = HashMap::new();
        sessions.insert(session.session_id.clone(), session);
        *self = Self {
            sessions,
            detector,
            active,
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> ProgressiveDebatic {
        let mut pd = ProgressiveDebatic::new();
        pd.detector.genius_threshold = 6000;
        pd.open_session("session-A", "science", DebateMode::Scientific, 100);
        for (offset, text) in [
            "not matter matter matter matter",
            "not matter matter matter matter",
            "matter energy relation is measurable and reproducible",
        ]
        .iter()
        .enumerate()
        {
            pd.ingest("session-A", text, 101 + offset as u64)
                .expect("ingest");
        }
        pd
    }
    fn seal(mut payload: Vec<u8>) -> Vec<u8> {
        let digest = Sha256::digest(&payload);
        payload.extend_from_slice(&digest);
        payload
    }
    fn unchecked_checkpoint(pd: &ProgressiveDebatic) -> Vec<u8> {
        // Test-only adversarial writer: bypass export validation but retain the
        // canonical framing/checksum to exercise semantic import validation.
        let mut out = Encoder::new();
        out.raw(MAGIC).expect("magic");
        PD_CHECKPOINT_SCHEMA.put(&mut out).expect("schema");
        GENIUS_TIE_POLICY_VERSION.put(&mut out).expect("policy");
        pd.active.put(&mut out).expect("active");
        pd.detector.put(&mut out).expect("detector");
        pd.session("session-A")
            .expect("session")
            .put(&mut out)
            .expect("session");
        seal(out.bytes)
    }
    fn assert_semantic_import_rejected(pd: &ProgressiveDebatic) {
        let mut target = ProgressiveDebatic::new();
        assert!(target
            .import_checkpoint("session-A", &unchecked_checkpoint(pd))
            .is_err());
        assert!(target.sessions.is_empty());
        assert!(target.active);
        assert_eq!(
            wire_bytes(&target.detector).expect("target"),
            wire_bytes(&GeniusDetector::default()).expect("default")
        );
    }

    fn mutate(bytes: &[u8], offset: usize, value: u8) -> Vec<u8> {
        let mut payload = bytes[..bytes.len() - 32].to_vec();
        payload[offset] = value;
        seal(payload)
    }
    fn outcome(output: PdEngineOutput) -> String {
        format!("{output:?}")
    }

    #[test]
    fn checkpoint_roundtrip_preserves_state_and_next_real_ingest() {
        let mut original = engine();
        let bytes = original.export_checkpoint("session-A").expect("export");
        let mut restored = ProgressiveDebatic::new();
        restored
            .import_checkpoint("session-A", &bytes)
            .expect("import");
        assert_eq!(
            bytes,
            restored
                .export_checkpoint("session-A")
                .expect("export restored")
        );
        let before = restored.session("session-A").expect("session");
        assert_eq!(before.pressure_budget.challenges_issued, 2u32);
        assert_eq!(before.started_at, 100);
        assert_eq!(before.last_activity_at, 103);
        let text = "not energy energy energy energy";
        assert_eq!(
            outcome(
                original
                    .ingest("session-A", text, 104)
                    .expect("ingest live")
            ),
            outcome(
                restored
                    .ingest("session-A", text, 104)
                    .expect("ingest restored")
            )
        );
        assert_eq!(
            original.export_checkpoint("session-A").expect("live"),
            restored.export_checkpoint("session-A").expect("restored")
        );
    }

    #[test]
    fn inactive_flags_resistance_and_real_genius_state_survive() {
        let mut pd = engine();
        pd.detector.genius_threshold = 0;
        pd.ingest("session-A",
            "again matter energy relation measurable invariant physical conservation observable repeatable phase oscillation through a laboratory apparatus",
            104).expect("ingest");
        assert!(pd
            .session("session-A")
            .expect("session")
            .genius_signal
            .is_some());
        pd.active = false;
        let session = pd.sessions.get_mut("session-A").expect("session");
        session.active = false;
        session.pressure_budget.user_resistance_detected = true;
        session.genius_sent_to_pim = true;
        let bytes = pd.export_checkpoint("session-A").expect("export");
        let mut restored = ProgressiveDebatic::new();
        restored
            .import_checkpoint("session-A", &bytes)
            .expect("import");
        assert!(!restored.active);
        let session = restored.session("session-A").expect("restored");
        assert!(!session.active);
        assert!(session.pressure_budget.user_resistance_detected);
        assert!(session.genius_sent_to_pim);
        assert_eq!(restored.detector.genius_threshold, 0);
        assert_eq!(
            bytes,
            restored.export_checkpoint("session-A").expect("re-export")
        );
    }

    #[test]
    fn canonical_map_order_and_tie_policy_are_independent_of_insertion_order() {
        let mut left = ProgressiveDebatic::new();
        left.open_session("session-A", "science", DebateMode::Novel, 1);
        left.ingest("session-A", "zeta alpha", 2).expect("ingest");
        let mut right = left.clone();
        let map = &mut right
            .sessions
            .get_mut("session-A")
            .expect("session")
            .trace
            .concept_map;
        map.clear();
        map.insert("zeta".to_string(), 1);
        map.insert("alpha".to_string(), 1);
        assert_eq!(
            super::super::genius_detector::most_frequent_concept(map),
            Some("alpha".to_string())
        );
        assert_eq!(
            left.export_checkpoint("session-A").expect("left"),
            right.export_checkpoint("session-A").expect("right")
        );
    }

    #[test]
    fn exact_u32_budget_width_and_exhaustion_are_not_reset() {
        // A historical over-exhausted but internally consistent record remains
        // exhausted. This fixture checks width; it is not claimed as live science.
        let mut pd = ProgressiveDebatic::new();
        pd.open_session("session-A", "science", DebateMode::Scientific, 1);
        for index in 0..300u32 {
            pd.ingest(
                "session-A",
                "matter matter matter matter",
                2 + u64::from(index),
            )
            .expect("ingest");
        }
        let session = pd.sessions.get_mut("session-A").expect("session");
        for turn in &mut session.turns {
            turn.pd_response_type = PdResponseType::ChallengeWithFacts;
            turn.challenge_issued = true;
            turn.pd_response_hint = "historical challenge".to_string();
        }
        session.pressure_budget.challenges_issued = 300;
        session.pressure_budget.budget_exhausted = true;
        let bytes = pd.export_checkpoint("session-A").expect("export");
        let mut restored = ProgressiveDebatic::new();
        restored
            .import_checkpoint("session-A", &bytes)
            .expect("import");
        assert_eq!(
            restored
                .session("session-A")
                .expect("session")
                .pressure_budget
                .challenges_issued,
            300
        );
        assert!(
            restored
                .session("session-A")
                .expect("session")
                .pressure_budget
                .budget_exhausted
        );
        restored
            .ingest("session-A", "not matter matter matter", 400)
            .expect("ingest");
        assert_eq!(
            restored
                .session("session-A")
                .expect("session")
                .pressure_budget
                .challenges_issued,
            300
        );
        assert_eq!(
            restored
                .session("session-A")
                .expect("session")
                .turns
                .last()
                .expect("turn")
                .pd_response_type,
            PdResponseType::Listen
        );
    }

    #[test]
    fn invalid_inputs_never_mutate_target() {
        let bytes = engine().export_checkpoint("session-A").expect("export");
        let mut corrupt = bytes.clone();
        corrupt[20] ^= 1;
        let cases = vec![
            bytes[..bytes.len() - 1].to_vec(),
            corrupt,
            mutate(&bytes, 8, 2),
            mutate(&bytes, 12, 2),
            mutate(&bytes, 16, 3),
            // Header (37 bytes), session string length (4), first session byte.
            mutate(&bytes, 41, b'x'),
            vec![0; PD_CHECKPOINT_MAX_BYTES + 1],
            seal([bytes[..bytes.len() - 32].to_vec(), vec![0]].concat()),
        ];
        for value in cases {
            let mut target = ProgressiveDebatic::new();
            assert!(target.import_checkpoint("session-A", &value).is_err());
            assert!(target.sessions.is_empty());
            assert!(target.active);
            assert_eq!(
                wire_bytes(&target.detector).expect("detector"),
                wire_bytes(&GeniusDetector::default()).expect("default")
            );
        }
        let mut target = ProgressiveDebatic::new();
        assert!(target.import_checkpoint("other-session", &bytes).is_err());
        assert!(target.sessions.is_empty());
        target
            .import_checkpoint("session-A", &bytes)
            .expect("restore");
        let before = target.export_checkpoint("session-A").expect("before");
        assert!(target.import_checkpoint("session-A", &bytes).is_err());
        assert_eq!(
            before,
            target.export_checkpoint("session-A").expect("after")
        );
    }

    #[test]
    fn inconsistent_counts_scopes_and_derived_material_fail_export() {
        let mut pd = engine();
        pd.sessions
            .get_mut("session-A")
            .expect("session")
            .pressure_budget
            .challenges_issued = 0;
        assert!(pd.export_checkpoint("session-A").is_err());
        assert_semantic_import_rejected(&pd);
        let mut pd = engine();
        pd.sessions
            .get_mut("session-A")
            .expect("session")
            .trace
            .session_id = "B".into();
        assert!(pd.export_checkpoint("session-A").is_err());
        assert_semantic_import_rejected(&pd);
        let mut pd = engine();
        pd.sessions
            .get_mut("session-A")
            .expect("session")
            .trace
            .accumulated_mass ^= 1;
        assert!(pd.export_checkpoint("session-A").is_err());
        assert_semantic_import_rejected(&pd);
    }

    #[test]
    fn decoder_rejects_duplicate_unordered_and_unbacked_collections() {
        for keys in [["same", "same"], ["zeta", "alpha"]] {
            let mut encoded = Encoder::new();
            2u32.put(&mut encoded).expect("count");
            for key in keys {
                key.to_string().put(&mut encoded).expect("key");
                1u32.put(&mut encoded).expect("value");
            }
            assert!(
                <HashMap<String, u32> as Wire>::get(&mut Decoder::new(&encoded.bytes)).is_err()
            );
        }
        assert!(<Vec<String> as Wire>::get(&mut Decoder::new(&u32::MAX.to_le_bytes())).is_err());
    }
}
