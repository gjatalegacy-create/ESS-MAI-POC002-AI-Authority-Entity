//! V189_DERIVATIVE: schema-1 PD checkpoint DATA verifier.
//! Pure historical data decoding and source signal/trace rederivation only.
//! No ProgressiveDebatic engine, ingest, response decision, seal, token or I/O.
//! Source: quantum/progressive_debatic/checkpoint.rs:14-517; types.rs:28-231;
//! core.rs:24-39,171-290; genius_detector.rs:14-21,180-189,227-237,250-257;
//! pd_spine_contract.rs:15-35. The codec policy remains schema 1 / tie policy 1.
//! No provenance/adjudication claim follows merely from a valid SHA or snapshot.
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
const MASS_SCALE: u32 = 10_000;
const GENIUS_TIE_POLICY_VERSION: u32 = 1;
// Source SHA256 progressive_debatic/checkpoint.rs: 5cbabc0af24c095f0c63b6219fe8eea6c2033fe0f0314644b625bb8690e3345c
// Source SHA256 progressive_debatic/types.rs: 2283946be071de8f47211515835e3be93250879634d153a03418e62cff1f4296
// Source SHA256 progressive_debatic/core.rs: 0c80ff2e7628ec746c9e618d497916b94606971f33d1e3bd600393ea7680ffa1
// Source SHA256 progressive_debatic/genius_detector.rs: 6d49345667f0b350e92a2e597ef0b4766c6437957974382a4331e46bf396b508
// Source SHA256 pd_spine_contract.rs: 96b15648c1a73b5852cbf676a2a344665229ae19bd16460fadb2f7ee014fbc0e

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FixedMass(pub u32);
impl FixedMass {
    pub const ZERO: Self = Self(0);
    pub fn new(raw: u32) -> Self {
        Self(raw.min(MASS_SCALE))
    }
    pub fn from_ratio(positive: u32, total: u32) -> Self {
        match total {
            0 => Self::ZERO,
            _ => Self::new(positive.saturating_mul(MASS_SCALE) / total),
        }
    }
    pub fn raw(self) -> u32 {
        self.0
    }
}

/// id 12-shenjësh heksadecimal nga FNV-1a (deterministik, hash-i real i sistemit).
pub fn pd_id(parts: &[&[u8]]) -> String {
    // Përziej pjesët në mënyrë branchless (fold mbi FNV).
    let h = parts.iter().fold(0xcbf29ce484222325u64, |acc, p| {
        crate::fnv1a64(p) ^ acc.wrapping_mul(1099511628211)
    });
    format!("{:012x}", h & 0xFFFF_FFFF_FFFF)
}

// ─────────────────────────────────────────────────────────────────────────────
// DebateMode — 4 mënyrat e angazhimit
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebateMode {
    Intellect,  // nevoja praktike — shtyn lehtë
    Philosophy, // intuitë pa strukturë — PD ndërton scaffold
    Scientific, // koncepti lidhet me matje — kërkon kauzalitet/invariantë
    Novel,      // zona e frontierit — kundërshtim shumë i butë
}


// ─────────────────────────────────────────────────────────────────────────────
// CognitiveSignal — çdo mesazh i shfaqur, i procesuar gjatë ingest
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct CognitiveSignal {
    pub signal_id: String,
    pub turn_index: u32,
    pub raw_content: String,
    pub content_length: u32,
    pub idea_density: u32, // fixed mass [0..10_000]
    pub energy_level: u32, // fixed mass [0..10_000]
    pub concept_refs: Vec<String>,
    pub contradicts: Vec<String>,
    pub returns_to: Vec<String>,
    pub timestamp: u64,
}

// ─────────────────────────────────────────────────────────────────────────────
// EpistemicTrace — gjurma e akumuluar gjatë sesionit
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct EpistemicTrace {
    pub trace_id: String,
    pub session_id: String,
    pub signals: Vec<CognitiveSignal>,
    pub total_turns: u32,
    pub dominant_concept: Option<String>,
    pub concept_map: std::collections::HashMap<String, u32>,
    pub energy_trajectory: Vec<u32>,
    pub idea_evolution: Vec<u32>,
    pub accumulated_mass: u32,
    pub structural_coherence: u32,
    pub implicit_primitives: Vec<String>,
    pub contradiction_pairs: Vec<(String, String)>,
    pub territory: String,
    pub started_at: u64,
    pub last_updated_at: u64,
}

impl EpistemicTrace {
    pub fn new(session_id: &str, territory: &str, started_at: u64) -> Self {
        EpistemicTrace {
            trace_id: pd_id(&[session_id.as_bytes(), b"trace"]),
            session_id: session_id.to_string(),
            signals: Vec::new(),
            total_turns: 0,
            dominant_concept: None,
            concept_map: std::collections::HashMap::new(),
            energy_trajectory: Vec::new(),
            idea_evolution: Vec::new(),
            accumulated_mass: 0,
            structural_coherence: 0,
            implicit_primitives: Vec::new(),
            contradiction_pairs: Vec::new(),
            territory: territory.to_string(),
            started_at,
            last_updated_at: started_at,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GeniusSignal — momenti kur sistemi "ndjen" diçka reale (3 sinjale)
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct GeniusSignal {
    pub signal_id: String,
    pub session_id: String,
    pub returns_to_idea: bool,
    pub energy_increasing: bool,
    pub concept_gaining_shape: bool,
    pub returns_score: u32,
    pub energy_score: u32,
    pub shape_score: u32,
    pub genius_score: u32,
    pub detected_concept: String,
    pub detected_at_turn: u32,
    pub trace_ref: String,
    pub timestamp: u64,
}

// ─────────────────────────────────────────────────────────────────────────────
// PressureBudget — sa mund të shtyjë PD para se të tërhiqet
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct PressureBudget {
    pub challenges_issued: u32,
    pub current_mode: DebateMode,
    pub budget_exhausted: bool,
    pub user_resistance_detected: bool,
}

impl PressureBudget {
    // Limitet absolute sipas mode-it (nga pseudo).
    pub const MAX_INTELLECT: u32 = 2;
    pub const MAX_PHILOSOPHY: u32 = 3;
    pub const MAX_SCIENTIFIC: u32 = 4;
    pub const MAX_NOVEL: u32 = 2;


    /// max_for — limiti i kundërshtimeve për mode-in aktual. Zero if — match.
    pub fn max_for(mode: DebateMode) -> u32 {
        match mode {
            DebateMode::Intellect => Self::MAX_INTELLECT,
            DebateMode::Philosophy => Self::MAX_PHILOSOPHY,
            DebateMode::Scientific => Self::MAX_SCIENTIFIC,
            DebateMode::Novel => Self::MAX_NOVEL,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PdResponseType — çfarë vendos PD të bëjë në çdo kthesë
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdResponseType {
    Help,
    Orient,
    Clarify,
    ChallengeGentle,
    ChallengeWithFacts,
    ScaffoldBullet,
    ScaffoldMeasure,
    ScaffoldStructure,
    Listen,
}

impl PdResponseType {

    /// is_challenge — a është kundërshtim (konsumon buxhet). Zero if — match.
    pub fn is_challenge(self) -> bool {
        match self {
            PdResponseType::ChallengeGentle | PdResponseType::ChallengeWithFacts => true,
            _ => false,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PdTurn — çdo kthesë (sinjali + vendimi i PD)
// ─────────────────────────────────────────────────────────────────────────────
#[derive(Debug, Clone)]
pub struct PdTurn {
    pub turn_id: String,
    pub turn_index: u32,
    pub user_signal: CognitiveSignal,
    pub pd_response_type: PdResponseType,
    pub pd_response_hint: String,
    pub challenge_issued: bool,
    pub mode_at_turn: DebateMode,
    pub genius_check: Option<u32>,
}

/// PdSession — gjendje e plotë e një sesioni gërmimi.
#[derive(Debug, Clone)]
pub struct PdSession {
    pub session_id: String,
    pub active: bool,
    pub current_mode: DebateMode,
    pub pressure_budget: PressureBudget,
    pub turns: Vec<PdTurn>,
    pub trace: EpistemicTrace,
    pub genius_signal: Option<GeniusSignal>,
    pub genius_sent_to_pim: bool,
    pub territory: String,
    pub started_at: u64,
    pub last_activity_at: u64,
    pub total_turns: u32,
}

#[derive(Debug, Clone)]
pub struct GeniusDetector {
    pub min_turns_before_check: u32,
    pub weight_returns: u32,
    pub weight_energy: u32,
    pub weight_shape: u32,
    pub genius_threshold: u32,
}

pub fn most_frequent_concept(map: &HashMap<String, u32>) -> Option<String> {
    map.iter()
        .max_by(|left, right| {
            left.1
                .cmp(right.1)
                .then_with(|| right.0.as_bytes().cmp(left.0.as_bytes()))
        })
        .map(|entry| entry.0.clone())
}

pub fn mean(values: &[u32]) -> u32 {
    match values.is_empty() {
        true => 0,
        false => {
            let sum = values
                .iter()
                .fold(0u64, |acc, value| acc.saturating_add(u64::from(*value)));
            (sum / values.len() as u64).min(u64::from(u32::MAX)) as u32
        }
    }
}

pub fn tokenize(content: &str) -> Vec<String> {
    content
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| word.to_lowercase())
        .collect()
}

pub fn extract_signal(
    content: &str,
    turn_index: u32,
    ts: u64,
    prior_concepts: &HashMap<String, u32>,
) -> CognitiveSignal {
    let words = tokenize(content);
    let content_length = content.chars().count() as u32;

    // Konceptet: fjalë domethënëse (≥4 shkronja), unike, të renditura.
    let mut concept_refs: Vec<String> = words
        .iter()
        .filter(|w| w.chars().count() >= 4)
        .cloned()
        .collect();
    concept_refs.sort();
    concept_refs.dedup();

    // idea_density: unik/total i fjalëve.
    let total = words.len().max(1);
    let unique: HashSet<&String> = words.iter().collect();
    let idea_density = FixedMass::from_ratio(unique.len() as u32, total as u32).raw();

    // energy_level: 80% gjatësi + 20% theks, fixed u32 [0..10_000].
    let emphasis = content.chars().filter(|&c| c == '!' || c == '?').count() as u32;
    let length_mass = FixedMass::from_ratio(content_length.min(160), 160).raw();
    let emphasis_mass = FixedMass::from_ratio(emphasis.min(3), 3).raw();
    let energy_level = (u64::from(length_mass)
        .saturating_mul(8_000)
        .saturating_add(u64::from(emphasis_mass).saturating_mul(2_000))
        / u64::from(MASS_SCALE))
    .min(u64::from(MASS_SCALE)) as u32;

    // returns_to: koncepte që KANË dalë më parë (kthim te e njëjta ide).
    let returns_to: Vec<String> = concept_refs
        .iter()
        .filter(|c| prior_concepts.contains_key(*c))
        .cloned()
        .collect();

    // contradicts: nëse sinjali ka fjalë mohuese, konceptet që mohon.
    let has_negation = words.iter().any(|w| {
        matches!(
            w.as_str(),
            "not" | "never" | "no" | "nuk" | "jo" | "kurre" | "pa"
        )
    });
    let contradicts: Vec<String> = match has_negation {
        true => concept_refs.clone(),
        false => Vec::new(),
    };

    CognitiveSignal {
        signal_id: pd_id(&[content.as_bytes(), &ts.to_le_bytes()]),
        turn_index,
        raw_content: content.to_string(),
        content_length,
        idea_density,
        energy_level,
        concept_refs,
        contradicts,
        returns_to,
        timestamp: ts,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// update_trace — akumulon sinjalin te trace-i (dominanti = kandidati mbijetues).
// ─────────────────────────────────────────────────────────────────────────────
pub fn update_trace(trace: &mut EpistemicTrace, signal: &CognitiveSignal) {
    trace.signals.push(signal.clone());
    trace.total_turns += 1;

    // concept_map: numëro shfaqjet e çdo koncepti.
    for c in &signal.concept_refs {
        *trace.concept_map.entry(c.clone()).or_insert(0) += 1;
    }
    trace.energy_trajectory.push(signal.energy_level);
    trace.idea_evolution.push(signal.idea_density);

    // dominant_concept: më i shpeshti (kandidati që i mbijetoi eliminimit).
    trace.dominant_concept = most_frequent_concept(&trace.concept_map);

    // accumulated_mass: mesatarja e dendësisë së ideve deri tani.
    trace.accumulated_mass = mean(&trace.idea_evolution).min(MASS_SCALE);

    // structural_coherence: fokusi = sa herë del dominanti / total kthesa.
    let dom_count = trace
        .dominant_concept
        .as_ref()
        .and_then(|d| trace.concept_map.get(d))
        .copied()
        .unwrap_or(0);
    trace.structural_coherence = FixedMass::from_ratio(dom_count, trace.total_turns.max(1)).raw();

    // implicit_primitives: koncepte që rishfaqen (count ≥ 2) — strukturë në lindje.
    let mut prims: Vec<String> = trace
        .concept_map
        .iter()
        .filter(|(_, &c)| c >= 2)
        .map(|(k, _)| k.clone())
        .collect();
    prims.sort();
    trace.implicit_primitives = prims;

    // contradiction_pairs: çift (koncept i mohuar, dominant) — tension produktiv.
    let dom = trace.dominant_concept.clone().unwrap_or_default();
    for c in &signal.contradicts {
        let pair = (c.clone(), dom.clone());
        // Shto vetëm nëse i ri DHE jo vetë-çift. Zero if — match mbi (a,b).
        let novel = !trace.contradiction_pairs.contains(&pair);
        let distinct = c != &dom;
        match novel && distinct {
            true => trace.contradiction_pairs.push(pair),
            false => {}
        }
    }

    trace.last_updated_at = signal.timestamp;
}

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

struct CheckpointData {
    session: PdSession,
}
fn parse_checkpoint(bytes: &[u8], expected_session_id: &str) -> Result<CheckpointData, String> {
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
    // Canonical re-encoding checks every field; no engine/state transition exists.
    let mut out = Encoder::new();
    out.raw(MAGIC)?;
    PD_CHECKPOINT_SCHEMA.put(&mut out)?;
    GENIUS_TIE_POLICY_VERSION.put(&mut out)?;
    active.put(&mut out)?;
    detector.put(&mut out)?;
    session.put(&mut out)?;
    require(out.bytes == payload, "non-canonical snapshot")?;
    Ok(CheckpointData { session })
}
/// Same data invariants as the original schema-1 import, without constructing a PD
/// engine or making a response decision. Integrity does not establish provenance.
pub fn validate_checkpoint(bytes: &[u8], expected_session_id: &str) -> Result<(), String> {
    parse_checkpoint(bytes, expected_session_id).map(|_| ())
}
/// NEW append-only user checkpoint boundary for PDF1 native admission.
/// Both snapshots pass the original data invariants. This checks immutable
/// history and the one actual user turn, not question authority or provenance.
pub fn validate_user_checkpoint_transition(
    current_raw: &[u8],
    session_id: &str,
    actual_user_text: &str,
    expected_revision: u64,
    previous_raw: Option<&[u8]>,
) -> Result<(), String> {
    let current = parse_checkpoint(current_raw, session_id)?.session;
    let next_revision = expected_revision
        .checked_add(1)
        .ok_or("PD checkpoint: revision overflow")?;
    require(
        u64::from(current.total_turns) == next_revision,
        "user transition turn/revision mismatch",
    )?;
    let last = current
        .turns
        .last()
        .ok_or("PD checkpoint: user turn missing")?;
    require(
        last.user_signal.raw_content == actual_user_text,
        "user transition actual text mismatch",
    )?;
    match previous_raw {
        None => require(
            expected_revision == 0 && current.total_turns == 1,
            "user transition previous checkpoint missing",
        ),
        Some(bytes) => {
            require(
                expected_revision > 0,
                "user transition unexpected previous checkpoint",
            )?;
            let previous = parse_checkpoint(bytes, session_id)?.session;
            require(
                u64::from(previous.total_turns) == expected_revision,
                "user transition previous revision mismatch",
            )?;
            require(
                current.session_id == previous.session_id
                    && current.territory == previous.territory
                    && current.started_at == previous.started_at
                    && current.trace.trace_id == previous.trace.trace_id,
                "user transition lineage changed",
            )?;
            require(
                current.last_activity_at >= previous.last_activity_at,
                "user transition clock regressed",
            )?;
            for (old, retained) in previous.turns.iter().zip(&current.turns) {
                require(
                    same_canonical_data(old, retained)?,
                    "user transition turn history changed",
                )?;
            }
            for (old, retained) in previous.trace.signals.iter().zip(&current.trace.signals) {
                require(
                    same_canonical_data(old, retained)?,
                    "user transition signal history changed",
                )?;
            }
            Ok(())
        }
    }
}
fn same_canonical_data<T: Wire>(left: &T, right: &T) -> Result<bool, String> {
    let mut lhs = Encoder::new();
    let mut rhs = Encoder::new();
    left.put(&mut lhs)?;
    right.put(&mut rhs)?;
    Ok(lhs.bytes == rhs.bytes)
}

fn response_tag(value: PdResponseType) -> u8 {
    match value {
        PdResponseType::Help => 0,
        PdResponseType::Orient => 1,
        PdResponseType::Clarify => 2,
        PdResponseType::ChallengeGentle => 3,
        PdResponseType::ChallengeWithFacts => 4,
        PdResponseType::ScaffoldBullet => 5,
        PdResponseType::ScaffoldMeasure => 6,
        PdResponseType::ScaffoldStructure => 7,
        PdResponseType::Listen => 8,
    }
}
fn mode_tag(value: DebateMode) -> u8 {
    match value {
        DebateMode::Intellect => 0,
        DebateMode::Philosophy => 1,
        DebateMode::Scientific => 2,
        DebateMode::Novel => 3,
    }
}
/// Bind a fully validated historical snapshot to the exact native pending turn.
/// Source runtime.rs:280-387 supplies the normal/GENIUS question and references.
/// R3 data-only binding: stable cognitive history is distinct from native attempt.
pub fn validate_prepared_checkpoint_for_session(
    bytes: &[u8],
    pending: &crate::pd_prepared::PdPendingData,
    cognitive_session_id: &str,
) -> Result<(), String> {
    let data = parse_checkpoint(bytes, cognitive_session_id)?;
    let s = &data.session;
    let last = s
        .turns
        .last()
        .ok_or("PD checkpoint: prepared turn missing")?;
    let spine = &pending.spine;
    require(
        spine.created_at == last.user_signal.timestamp
            && spine.continuum.user_input_digest
                == crate::digest_parts(&[last.user_signal.raw_content.as_bytes()]),
        "prepared raw input/timestamp mismatch",
    )?;
    match spine.activation.origin_code {
        0 => {
            require(
                pending.turn_id == last.turn_id
                    && pending.prepared_question == last.pd_response_hint
                    && pending.response_type == response_tag(last.pd_response_type),
                "prepared normal turn mismatch",
            )?;
            require(
                spine.trace_digest == crate::digest_parts(&[last.user_signal.signal_id.as_bytes()])
                    && spine.turn_digest == crate::digest_parts(&[last.turn_id.as_bytes()]),
                "prepared normal references mismatch",
            )?;
            require(
                spine.activation.mode_code == mode_tag(last.mode_at_turn)
                    && spine.activation.genius_signal_digest
                        == last.genius_check.map(u64::from).unwrap_or(0)
                    && spine.activation.trace_mass == last.user_signal.idea_density
                    && spine.activation.structural_coherence == last.user_signal.energy_level,
                "prepared normal activation mismatch",
            )?;
        }
        1 => {
            let g = s
                .genius_signal
                .as_ref()
                .ok_or("PD checkpoint: prepared genius missing")?;
            require(
                g.detected_at_turn == s.total_turns && g.timestamp == last.user_signal.timestamp,
                "prepared stale genius",
            )?;
            let question = format!(
                "Cila provë konkrete e ngre konceptin '{}' në hapin tjetër?",
                g.detected_concept
            );
            let package_id = pd_id(&[s.session_id.as_bytes(), &g.timestamp.to_le_bytes(), b"pkg"]);
            require(
                pending.turn_id == g.signal_id
                    && pending.prepared_question == question
                    && pending.response_type == 7,
                "prepared genius turn mismatch",
            )?;
            require(
                spine.trace_digest == crate::digest_parts(&[g.trace_ref.as_bytes()])
                    && spine.turn_digest == crate::digest_parts(&[package_id.as_bytes()]),
                "prepared genius references mismatch",
            )?;
            require(
                spine.activation.mode_code == 3
                    && spine.activation.genius_signal_digest
                        == crate::digest_parts(&[
                            g.signal_id.as_bytes(),
                            g.trace_ref.as_bytes(),
                            &g.genius_score.to_le_bytes(),
                        ])
                    && spine.activation.trace_mass == s.trace.accumulated_mass
                    && spine.activation.structural_coherence == s.trace.structural_coherence,
                "prepared genius activation mismatch",
            )?;
        }
        _ => return Err("PD checkpoint: terminal output has no native pending".into()),
    }
    Ok(())
}

/// Validated data projection, never an engine or authority capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointMetadata {
    pub session_id: String,
    pub territory: String,
    pub total_turns: u32,
    pub last_user_text: String,
    pub last_activity_at: u64,
}
pub fn checkpoint_metadata(bytes: &[u8], session_id: &str) -> Result<CheckpointMetadata, String> {
    let session = parse_checkpoint(bytes, session_id)?.session;
    let last = session
        .turns
        .last()
        .ok_or("PD checkpoint: metadata user turn missing")?;
    Ok(CheckpointMetadata {
        session_id: session.session_id.clone(),
        territory: session.territory.clone(),
        total_turns: session.total_turns,
        last_user_text: last.user_signal.raw_content.clone(),
        last_activity_at: session.last_activity_at,
    })
}
