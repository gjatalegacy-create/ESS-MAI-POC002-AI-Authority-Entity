// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  TRACE_INFO.RS — AIGovern-Cyber LIGHT PLATFORM                               ║
// ║  Gjata Legacy™  |  Arkitekt: Bledar Gjata                             ║
// ║                                                                          ║
// ║  Gjurma e parë e inputit — stamp fillestar sapo hyn.                  ║
// ║  Këtu: i₀ i plotë + discriminator instance; zero identitet i cunguar. ║
// ║  Dështimi i orës/sequence-it refuzohet, nuk shndërrohet në zero.       ║
// ║                                                                          ║
// ║  Roli: TRACER — vë vulë, nuk vendos, nuk verifikon.                  ║
// ╚══════════════════════════════════════════════════════════════════════════╝

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

// ════════════════════════════════════════════════════════════════════════════
// §1  FNV-1a 64-bit — i njëjtë me quantum_v14_core.rs
//     Inline: zero dipendencë, zero allokacion
// ════════════════════════════════════════════════════════════════════════════

#[inline(always)]
pub fn fnv64(data: &[u8]) -> u64 {
    let mut h: u64 = 14695981039346656037;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}

// ════════════════════════════════════════════════════════════════════════════
// §2  DOMAIN HINT — indikacion i lehtë domeni
//     Koordinohet me Domain në nura_core.rs
//     light_coordinator bën konversionin midis të dyjave
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub enum DomainHint {
    Work,    // NURA: biznes, industri, governance
    Home,    // ALISAH: familje, shtëpi, IoT/MATTER
    Outside, // AMMY: mobile, ephemeral web
    Unknown, // Pa detekim — light_coordinator do vendosë
}

// ════════════════════════════════════════════════════════════════════════════
// §3  TRACE INFO — struct kryesor
// ════════════════════════════════════════════════════════════════════════════

/// Counter sekuencial global — atomik, thread-safe, zero lock
static TRACE_SEQ: AtomicU64 = AtomicU64::new(1);

/// Identiteti i instancës fiksohet një herë për proces. PID-i dallon proceset
/// bashkëkohore; koha e nisjes së instancës dallon ripërdorimin e PID-it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProcessInstance {
    process_id: u32,
    started_ns: u64,
}

static PROCESS_INSTANCE: OnceLock<ProcessInstance> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TraceIdentityError {
    EmptyInput,
    ClockBeforeUnixEpoch,
    ClockOutOfRange,
    SequenceExhausted,
    ProcessIdentityUnavailable,
    DegenerateIdentity,
}

impl fmt::Display for TraceIdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "TRACE_IDENTITY:EMPTY_INPUT"),
            Self::ClockBeforeUnixEpoch => write!(f, "TRACE_IDENTITY:CLOCK_BEFORE_UNIX_EPOCH"),
            Self::ClockOutOfRange => write!(f, "TRACE_IDENTITY:CLOCK_OUT_OF_RANGE"),
            Self::SequenceExhausted => write!(f, "TRACE_IDENTITY:SEQUENCE_EXHAUSTED"),
            Self::ProcessIdentityUnavailable => {
                write!(f, "TRACE_IDENTITY:PROCESS_INSTANCE_UNAVAILABLE")
            }
            Self::DegenerateIdentity => write!(f, "TRACE_IDENTITY:DEGENERATE_I0"),
        }
    }
}

fn system_time_ns_at(now: SystemTime) -> Result<u64, TraceIdentityError> {
    let duration = now
        .duration_since(UNIX_EPOCH)
        .map_err(|_| TraceIdentityError::ClockBeforeUnixEpoch)?;
    u64::try_from(duration.as_nanos()).map_err(|_| TraceIdentityError::ClockOutOfRange)
}

fn system_time_ns() -> Result<u64, TraceIdentityError> {
    system_time_ns_at(SystemTime::now())
}

fn process_instance() -> Result<ProcessInstance, TraceIdentityError> {
    match PROCESS_INSTANCE.get() {
        Some(instance) => Ok(*instance),
        None => {
            let candidate = ProcessInstance {
                process_id: std::process::id(),
                started_ns: system_time_ns()?,
            };
            match (candidate.process_id, candidate.started_ns) {
                (0, _) | (_, 0) => Err(TraceIdentityError::ProcessIdentityUnavailable),
                _ => match PROCESS_INSTANCE.set(candidate) {
                    Ok(()) => Ok(candidate),
                    Err(_) => PROCESS_INSTANCE
                        .get()
                        .copied()
                        .ok_or(TraceIdentityError::ProcessIdentityUnavailable),
                },
            }
        }
    }
}

fn next_trace_sequence() -> Result<u64, TraceIdentityError> {
    TRACE_SEQ
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |current| {
            current.checked_add(1)
        })
        .map_err(|_| TraceIdentityError::SequenceExhausted)
}

#[derive(Debug, Clone)]
pub struct TraceInfo {
    /// ID unik: fnv64(input) XOR timestamp_ns
    /// SHA-256 real i inputit lind në Light (evolve_trace::input_sha256)
    pub id: u64,

    /// Timestamp nanoseconds since UNIX_EPOCH
    pub ts_ns: u64,

    /// Numëruesi sekuencial i sesionit — rrit me 1 për çdo input
    pub seq: u64,

    /// PID-i i procesit LIGHT që lindi i₀.
    pub process_id: u32,

    /// Fillimi i instancës LIGHT në nanosekonda; fiksohet një herë për proces.
    pub process_started_ns: u64,

    /// Gjatësia e inputit të papërpunuar
    pub raw_len: usize,

    /// Hint i domenit — Unknown deri sa light_coordinator vendos
    pub domain_hint: DomainHint,
}

impl TraceInfo {
    /// Krijon i₀ autoritativ. Ora, instance dhe sequence janë fallible:
    /// dështimi mbyll rrjedhën para GCL/LGC dhe nuk prodhon identitet zero.
    pub fn try_new(raw_input: &[u8]) -> Result<Self, TraceIdentityError> {
        let instance = process_instance()?;
        let ts = system_time_ns()?;
        let seq = next_trace_sequence()?;
        Self::from_parts(raw_input, ts, seq, instance)
    }

    fn from_parts(
        raw_input: &[u8],
        ts_ns: u64,
        seq: u64,
        instance: ProcessInstance,
    ) -> Result<Self, TraceIdentityError> {
        match raw_input.is_empty() {
            true => return Err(TraceIdentityError::EmptyInput),
            false => {}
        }
        match (ts_ns, seq, instance.process_id, instance.started_ns) {
            (0, _, _, _) => return Err(TraceIdentityError::ClockOutOfRange),
            (_, 0, _, _) => return Err(TraceIdentityError::SequenceExhausted),
            (_, _, 0, _) | (_, _, _, 0) => {
                return Err(TraceIdentityError::ProcessIdentityUnavailable)
            }
            _ => {}
        }

        let id = fnv64(raw_input) ^ ts_ns;
        match id {
            0 => return Err(TraceIdentityError::DegenerateIdentity),
            _ => {}
        }

        Ok(Self {
            id,
            ts_ns,
            seq,
            process_id: instance.process_id,
            process_started_ns: instance.started_ns,
            raw_len: raw_input.len(),
            domain_hint: DomainHint::Unknown,
        })
    }

    /// Konstruktor convenience vetëm për testet ekzistuese. Rruga prodhuese
    /// përdor gjithmonë `try_new` dhe propagandon dështimin e tipizuar.
    #[cfg(test)]
    pub fn new(raw_input: &[u8]) -> Self {
        Self::try_new(raw_input).expect("test trace identity")
    }

    /// Vendos domain hint — merret nga light_coordinator
    pub fn with_domain(mut self, hint: DomainHint) -> Self {
        self.domain_hint = hint;
        self
    }

    /// ID si hex 16 karaktere — për logging dhe evolve_trace
    pub fn id_hex(&self) -> String {
        format!("{:016x}", self.id)
    }

    /// Session-id autoritativ: sequence + i₀ i plotë + instance e verifikueshme.
    /// Asnjë pjesë e autoritetit nuk shkurtohet në 32 bit.
    pub fn label(&self) -> String {
        format!(
            "T{}_{id}_P{pid:08x}_{started:016x}",
            self.seq,
            id = self.id_hex(),
            pid = self.process_id,
            started = self.process_started_ns,
        )
    }

    /// Verifikim i bazë: ID nuk mund të jetë zero
    pub fn is_valid(&self) -> bool {
        self.id != 0
            && self.ts_ns != 0
            && self.seq != 0
            && self.process_id != 0
            && self.process_started_ns != 0
            && self.raw_len > 0
    }
}

// ════════════════════════════════════════════════════════════════════════════
// §4  TRACE SESSION — mbledh trace-t e një sesioni
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn trace_for_process(process_id: u32, started_ns: u64) -> TraceInfo {
        TraceInfo::from_parts(
            b"same input",
            1_770_000_000_000_000_000,
            1,
            ProcessInstance {
                process_id,
                started_ns,
            },
        )
        .expect("synthetic process trace")
    }

    #[test]
    fn authoritative_label_contains_full_i0_and_process_instance() {
        let trace = trace_for_process(41, 1_769_999_999_000_000_000);
        assert!(trace.label().contains(&trace.id_hex()));
        assert!(trace.label().contains("P00000029"));
        assert!(trace
            .label()
            .contains(&format!("{:016x}", trace.process_started_ns)));
        assert!(trace.is_valid());
    }

    #[test]
    fn separate_process_instances_cannot_share_a_session_label() {
        let first = trace_for_process(41, 1_769_999_999_000_000_000);
        let second = trace_for_process(42, 1_769_999_999_000_000_000);
        assert_eq!(first.id, second.id);
        assert_eq!(first.ts_ns, second.ts_ns);
        assert_eq!(first.seq, second.seq);
        assert_ne!(first.label(), second.label());
    }

    #[test]
    fn clock_failure_is_typed_and_never_becomes_zero() {
        // Windows SystemTime uses 100 ns ticks; 1 ns truncates to zero ticks.
        // A full second makes this fixture strictly before the epoch.
        let before_epoch = UNIX_EPOCH
            .checked_sub(Duration::from_secs(1))
            .expect("representable test time");
        assert!(before_epoch < UNIX_EPOCH, "fixture must precede UNIX_EPOCH");
        assert_eq!(
            system_time_ns_at(before_epoch),
            Err(TraceIdentityError::ClockBeforeUnixEpoch)
        );
        assert!(matches!(
            TraceInfo::from_parts(
                b"clock",
                0,
                1,
                ProcessInstance {
                    process_id: 41,
                    started_ns: 1,
                },
            ),
            Err(TraceIdentityError::ClockOutOfRange)
        ));
    }
}
