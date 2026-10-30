// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  KNOWLEDGE_VAULT.RS — AIGovern-Cyber SHADOW PLATFORM                    ║
// ║  Gjata Legacy™  |  Arkitekt: Bledar Gjata                             ║
// ║                                                                          ║
// ║  7 magazinat e dijes — ARKIVI SOVRAN.                                 ║
// ║                                                                          ║
// ║  RREGULLA ABSOLUTE (LAW_3 — pastërti arsyetimi):                     ║
// ║    LEXON:   çdo modul Shadow + Quantum                                 ║
// ║    SHKRUAN: shadow_gj_legacy.lgc VETËM (write API = pub(crate))       ║
// ║                                                                          ║
// ║  — I PASURUAR:                                                    ║
// ║    • Negative Knowledge me kontekst (shkak/proces/masë/frekuencë)     ║
// ║    • access_count gjurmohet për primitive + negative                  ║
// ║    • Maturimi Legacy ushqehet realisht (vector_points/temporal_score) ║
// ║    • check_known_negative() — Negative si KUFI aktiv                   ║
// ║    • VaultBackend — abstraksion për persistencë në disk (RAM tani)    ║
// ╚══════════════════════════════════════════════════════════════════════════╝

use crate::knowledge_lineage::{cross_domain_legacy_ready, Lineage};
use crate::shadow_commit::KnowledgeWrite;
use crate::types::{KnowledgeBand, NegativeContext, PrimitiveTrace, ShadowError, ShadowPath};
use crate::vault_transaction::{CommitStatus, DurableCommit, VaultError, VaultTransaction};
use std::collections::HashMap;
#[path = "candidate_store.rs"]
mod candidate_store;
use candidate_store::CandidateStore;
#[path = "vault_maturation.rs"]
mod maturation;
pub use maturation::{MaturationAppend, MaturationRecord};
use std::sync::{Arc, Mutex, RwLock};

// ════════════════════════════════════════════════════════════════════════════
// §0  VAULT BACKEND — abstraksion për persistencë (RAM tani, disk më vonë)
// ════════════════════════════════════════════════════════════════════════════

/// Cila magazinë po shkruhet (për backend-in e persistencës).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StoreKind {
    Scientific,
    Negative,
    Hypothesis,
    Fact,
    Primitive,
    Sovereign,
    Legacy,
}

/// Backend i persistencës (grepi i durabilitetit). Burimi i LEXIMIT mbetet
/// gjithmonë RAM (Vec); backend-i regjistron NGJARJET logjike që rikrijojnë
/// gjendjen në startup. Metodat kanë trup bosh si parazgjedhje → backend-et
/// jo-persistente (RAM) s'implementojnë asgjë.
///
/// Regjistrohen VETËM ngjarjet thelbësore të dijes:
///   `on_primitive` / `on_verified` / `on_negative` / `on_confirm`.
/// `access_count` (popullariteti runtime) është metadata e butë dhe NUK
/// persiston veçmas — rindërtohet nga ripërsëritja e `Confirm`.
///
/// LIGJI 0: të gjitha metodat marrin payload-in me referencë (`&[u8]`) —
/// asnjë klon; serializimi në disk është kufiri i vetëm i durabilitetit.
/// Borrowed append permission for the already validated live transaction.
/// Only this module creates it, after commit checks. It is bound to the actual
/// backend instance, cannot be cloned, and exposes no mutable transaction.
#[derive(Debug)]
pub struct KnowledgeAppend<'a> {
    transaction: &'a VaultTransaction,
    backend: &'a dyn VaultBackend,
}

impl<'a> KnowledgeAppend<'a> {
    pub(crate) fn transaction_for<B: VaultBackend + ?Sized>(
        self, backend: &B,
    ) -> Result<&'a VaultTransaction, VaultError> {
        if !same_backend(self.backend, backend) {
            return Err(VaultError::CorruptTransaction("LIVE_APPEND_WRONG_BACKEND".into()));
        }
        Ok(self.transaction)
    }
}

// Concrete Any identity excludes an enclosing backend and its offset-zero field
// sharing an erased address. Owned DiskBackend instances have nonzero state.
// Same-type zero-sized in-memory backends are stateless, not durable owners.
fn same_backend<B: VaultBackend + ?Sized>(expected: &dyn VaultBackend, actual: &B) -> bool {
    std::any::Any::type_id(expected) == std::any::Any::type_id(actual)
        && expected as *const dyn VaultBackend as *const () == actual as *const B as *const ()
}

pub trait VaultBackend: std::fmt::Debug + Send + Sync + std::any::Any {
    fn commit_native_continuation(&self,_record:&crate::native_continuation::NativeContinuationRecord)->Result<(),VaultError>{
        Err(VaultError::Io("PD_T2_DURABLE_BACKEND_REQUIRED".into()))
    }

    /// Operational success requires an explicitly durable backend. The in-memory
    /// default must not manufacture persistence for PD.
    fn commit_pd_operation(
        &self,
        _record: &shadow_contracts::pd_memory::PdOperationRecord,
    ) -> Result<(), VaultError> {
        Err(VaultError::Io("PD_DURABLE_BACKEND_REQUIRED".to_string()))
    }

    /// Një transaction i plotë është njësia e vetme authoritative e durability.
    fn append_verified(&self, permit: KnowledgeAppend<'_>) -> Result<(), VaultError> {
        permit.transaction_for(self)?;
        Ok(())
    }

    /// An existing Vault projection may move admitted rows, never admit new knowledge.
    fn append_maturation(&self, permit: MaturationAppend<'_>) -> Result<(), VaultError> {
        permit.record_for(self)?;
        Err(VaultError::Io("MATURATION_BACKEND_UNSUPPORTED".into()))
    }

    /// Storage/recovery fixtures only; never present in the production binary.
    #[cfg(test)]
    fn commit_test_transaction(&self, tx: &VaultTransaction) -> Result<(), VaultError>
    where Self: Sized {
        self.append_verified(KnowledgeAppend { transaction: tx, backend: self })
    }

    /// API-të më poshtë ruhen vetëm për kompatibilitet legacy/test. Edhe ato
    /// janë Result-first: disk failure nuk mund të raportohet si sukses.
    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
    fn on_primitive(
        &self,
        _input_id: u64,
        _vector_points: f32,
        _temporal_score: f32,
        _mass: f32,
        _seal: u32,
        _path: u8,
        _domain: &str,
        _bytes: &[u8],
        _ts: u64,
    ) -> Result<(), VaultError> {
        Ok(())
    }
    #[cfg(test)]
    fn on_verified(
        &self,
        _input_id: u64,
        _band: u8,
        _mass: f32,
        _bytes: &[u8],
        _ts: u64,
    ) -> Result<(), VaultError> {
        Ok(())
    }
    #[cfg(any(test, feature = "dev_harness"))]
    fn on_negative(
        &self,
        _input_id: u64,
        _reason: u32,
        _process: &str,
        _mass: f32,
        _frequency: u32,
        _bytes: &[u8],
        _ts: u64,
    ) -> Result<(), VaultError> {
        Ok(())
    }
    #[cfg(test)]
    fn on_confirm(&self, _input_id: u64, _hits: u32) -> Result<(), VaultError> {
        Ok(())
    }
}

/// Backend në RAM — pa durabilitet (default i; përdoret nga Shadow::new).
#[derive(Debug, Default)]
pub struct InMemoryBackend;

impl VaultBackend for InMemoryBackend {
    fn append_maturation(&self, permit: MaturationAppend<'_>) -> Result<(), VaultError> {
        permit.record_for(self)?;
        Ok(())
    }
}

// ════════════════════════════════════════════════════════════════════════════
// §1  ENTRY TYPES PER STORE
// ════════════════════════════════════════════════════════════════════════════

/// Tolerancë e masës për të njohur të njëjtin model negativ.
const NEG_MASS_TOLERANCE: f32 = 0.05;

/// FNV-1a 64-bit — identifikues përmbajtjeje deterministik (content_hash).
fn fnv64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 14695981039346656037;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}

#[derive(Debug, Clone)]
pub struct ScientificEntry {
    pub domain: String,
    pub content: Vec<u8>,
    pub source: String,
    pub added_at: u64,
}

#[derive(Debug, Clone)]
pub struct NegativeEntry {
    pub input_id: u64,
    /// Kodi i arsyes së dështimit — bitmask (qeverisje | vula | etj.)
    pub failure_reason_code: u32,
    /// Procesi / territori ku ndodhi
    pub process: String,
    /// Epistemic mass në çastin e dështimit
    pub mass_at_fail: f32,
    /// Sa herë u përsërit ky model negativ
    pub frequency: u32,
    pub raw_bytes: Vec<u8>,
    /// Sa herë u konsultua si kufi për procese të reja
    pub access_count: u32,
    pub added_at: u64,
}

// Content identity is independent of a contextual use or a write capability.
fn candidate_content_sha256(bytes: &[u8]) -> [u8; 32] {
    crate::vault_transaction::sha256_parts(&[
        b"OPL2S_CANDIDATE_CONTENT_V1", &(bytes.len() as u64).to_le_bytes(), bytes,
    ])
}

impl NegativeEntry {
    pub fn context(&self) -> NegativeContext {
        NegativeContext {
            failure_reason_code: self.failure_reason_code,
            process: self.process.clone(),
            mass_at_fail: self.mass_at_fail,
            frequency: self.frequency,
        }
    }

    /// to_bytes — serializon entry-n si bytes (P3: eksport sinkron për PRO Vault).
    /// Format: input_id(8) | reason(4) | mass(4) | freq(4) | access(4) | added(8)
    ///         | proc_len(4) | proc_bytes | raw_len(4) | raw_bytes. Zero if.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&self.input_id.to_le_bytes());
        out.extend_from_slice(&self.failure_reason_code.to_le_bytes());
        out.extend_from_slice(&self.mass_at_fail.to_le_bytes());
        out.extend_from_slice(&self.frequency.to_le_bytes());
        out.extend_from_slice(&self.access_count.to_le_bytes());
        out.extend_from_slice(&self.added_at.to_le_bytes());
        let proc_bytes = self.process.as_bytes();
        out.extend_from_slice(&(proc_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(proc_bytes);
        out.extend_from_slice(&(self.raw_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&self.raw_bytes);
        out
    }

    /// from_bytes — deserializon entry-n nga bytes. Zero if — match mbi gjatësi.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        // Minimumi: 8+4+4+4+4+8+4 = 36 byte header. Zero if — match.
        let enough_header = bytes.len() >= 36;
        match enough_header {
            false => None,
            true => {
                let input_id = u64::from_le_bytes(bytes[0..8].try_into().ok()?);
                let failure_reason_code = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
                let mass_at_fail = f32::from_le_bytes(bytes[12..16].try_into().ok()?);
                let frequency = u32::from_le_bytes(bytes[16..20].try_into().ok()?);
                let access_count = u32::from_le_bytes(bytes[20..24].try_into().ok()?);
                let added_at = u64::from_le_bytes(bytes[24..32].try_into().ok()?);
                let proc_len = u32::from_le_bytes(bytes[32..36].try_into().ok()?) as usize;
                // Lexo process (fail-closed: truncation = korrupsion → refuzo).
                let proc_end = 36 + proc_len;
                let process = match bytes.len() >= proc_end {
                    true => String::from_utf8_lossy(&bytes[36..proc_end]).to_string(),
                    false => {
                        eprintln!("[VAULT_DECODE] ALARM: rekord i cunguar (process: pritej {proc_end}B, ka {}B) — REFUZUAR (korrupsion)", bytes.len());
                        return None;
                    }
                };
                // Lexo raw_bytes (fail-closed: truncation = korrupsion → refuzo).
                let raw_bytes = match bytes.len() >= proc_end + 4 {
                    true => {
                        let raw_len =
                            u32::from_le_bytes(bytes[proc_end..proc_end + 4].try_into().ok()?)
                                as usize;
                        let raw_start = proc_end + 4;
                        let raw_end = raw_start + raw_len;
                        match bytes.len() >= raw_end {
                            true => bytes[raw_start..raw_end].to_vec(),
                            false => {
                                eprintln!("[VAULT_DECODE] ALARM: rekord i cunguar (raw: pritej {raw_end}B, ka {}B) — REFUZUAR (korrupsion)", bytes.len());
                                return None;
                            }
                        }
                    }
                    false => {
                        eprintln!("[VAULT_DECODE] ALARM: rekord i cunguar (raw_len header mungon) — REFUZUAR (korrupsion)");
                        return None;
                    }
                };
                Some(NegativeEntry {
                    input_id,
                    failure_reason_code,
                    process,
                    mass_at_fail,
                    frequency,
                    raw_bytes,
                    access_count,
                    added_at,
                })
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct HypothesisEntry {
    pub input_id: u64,
    pub mass: f32,
    pub raw_bytes: Vec<u8>,
    pub added_at: u64,
}

#[derive(Debug, Clone)]
pub struct FactEntry {
    pub input_id: u64,
    pub mass: f32,
    pub raw_bytes: Vec<u8>,
    pub added_at: u64,
}

#[derive(Debug, Clone)]
pub struct PrimitiveEntry {
    pub input_id: u64,
    pub content_hash: u64, /* identifikon inputin (FNV i raw_bytes)  */
    pub raw_bytes: Vec<u8>,
    pub vector_points: f32,  /* pikë të akumuluara drejt Legacy       */
    pub real_hits: u32,      /* sa herë u konfirmua nga bota reale    */
    pub temporal_score: f32, /* qëndrueshmëria kohore                 */
    pub access_count: u32,   /* sa herë u konsultua si bazë vendimi   */
    pub added_at: u64,
    /// GJURMUESHMËRIA: historiku i përforcimit ndër-domain (ku/si/kur/masë/vulë).
    pub primitive_chain: Vec<PrimitiveTrace>,
    /// I gatshëm për Legacy kur primitive_chain ka ≥5 domain-e unike e konsistente.
    pub legacy_ready: bool,
}

impl PrimitiveEntry {
    /// Skor cilësie i kristalizimit (zero if/else) — ruhet te LegacyEntry kur
    /// primitivi promovohet. Promovimi VENDOSET nga rregulla ndër-domain (≥5),
    /// jo nga ky skor; ky skor është thjesht metrikë e cilësisë së dijes.
    pub fn legacy_score(&self) -> f32 {
        (self.vector_points * 0.40
            + self.real_hits as f32 * 0.35 / 100.0
            + self.temporal_score * 0.25)
            .clamp(0.0, 1.0)
    }

    /// LIGJI Primitive→Legacy (i PËRFORCUAR, FAZA 3): gatishmëria kërkon
    /// BOTH përhapje ndër-domain (`legacy_ready`) DHE maturim real nga bota
    /// (real_hits + temporal_score + access_count mbi prag).
    ///
    /// Një legacy lind me KOHË: jo mjafton të prekë 5 domaine — duhet të ketë
    /// "mbijetuar" përdorimin real. `legacy_ready` mbahet nga `apply_primitive`;
    /// maturimi vlerësohet nga MaturationGate mbi sinjalet e këtij entry.
    /// Zero if/else — produkt boolean.
    pub fn ready_for_legacy(&self) -> bool {
        let signals = crate::legacy_maturation::MaturationSignals {
            real_hits: self.real_hits,
            temporal_score: self.temporal_score,
            access_count: self.access_count,
            domain_count: crate::knowledge_lineage::unique_domains(&self.primitive_chain),
            chain_strength: crate::primitive_chain::ChainStrength::compute(&self.primitive_chain)
                .strength,
        };
        // BOTH: rregulla ndër-domain (legacy_ready) AND maturim real.
        crate::legacy_maturation::MaturationGate::legacy_qualified(&signals, self.legacy_ready)
    }

    /// legacy_reason — prejardhja e plotë: PSE/SI ky primitiv (do të) bëhet
    /// legacy. Hook për vëzhguesin e primitivëve (plani #7) — çdo legacy ka
    /// arsyen e regjistruar se si erdhi.
    pub fn legacy_reason(&self) -> crate::legacy_maturation::LegacyReason {
        let signals = crate::legacy_maturation::MaturationSignals {
            real_hits: self.real_hits,
            temporal_score: self.temporal_score,
            access_count: self.access_count,
            domain_count: crate::knowledge_lineage::unique_domains(&self.primitive_chain),
            chain_strength: crate::primitive_chain::ChainStrength::compute(&self.primitive_chain)
                .strength,
        };
        crate::legacy_maturation::LegacyReason::derive(&signals, self.legacy_ready)
    }
}

#[derive(Debug, Clone)]
pub struct SovereignEntry {
    pub law_id: u8,   /* 1-5                                   */
    pub decision: u8, /* 0/1                                   */
    pub input_id: u64,
    pub raw_bytes: Vec<u8>,
    pub added_at: u64,
}

#[derive(Debug, Clone)]
pub struct LegacyEntry {
    pub origin_primitive_id: u64,
    pub raw_bytes: Vec<u8>,
    pub crystallized_at: u64,
    pub legacy_score: f32,
    /// I paprekshëm — Shadow nuk e modifikon kurrë (LAW_4)
    pub immutable_seal: u32,
}

// ════════════════════════════════════════════════════════════════════════════
// §2  KNOWLEDGE VAULT — 7 stores + backend
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug)]
pub struct KnowledgeVault {
    scientific: Arc<RwLock<Vec<ScientificEntry>>>,
    negative: Arc<RwLock<CandidateStore<NegativeEntry>>>,
    hypothesis: Arc<RwLock<Vec<HypothesisEntry>>>,
    fact: Arc<RwLock<Vec<FactEntry>>>,
    primitive: Arc<RwLock<CandidateStore<PrimitiveEntry>>>,
    sovereign: Arc<RwLock<Vec<SovereignEntry>>>,
    legacy: Arc<RwLock<CandidateStore<LegacyEntry>>>,
    /// FAZA 7 — vëzhguesi i përhershëm: regjistron prejardhjen e çdo legacy.
    pub observer: Arc<crate::legacy_observer::LegacyObserver>,
    backend: Box<dyn VaultBackend>,
    commit_lock: Mutex<()>,
    pd_projection: RwLock<crate::pd_memory_store::PdProjection>,
    /// pd_mp index: references the single Vault-owned T2 record; no second warehouse.
    continuation_refs:RwLock<HashMap<[u8;32],Arc<crate::native_continuation::NativeContinuationRecord>>>,

    native_pd_heads: RwLock<HashMap<[u8; 32], crate::native_cycle::NativePdHead>>,
    // Exact operation -> existing T1 identity only. Rebuilt from the same WAL.
    native_iz_operations: RwLock<HashMap<([u8;32],String),[u8;32]>>,
    // KiE origin survives common-filter adjudication. Values reference existing
    // T1 identities only; no raw PDS1 entry populates this map.
    kie_adjudicated_refs: RwLock<HashMap<crate::native_cycle::KieHistoryKey,[u8;32]>>,
    kie_history: RwLock<crate::kie_history::HistoryIndex>,
    committed_by_cycle: RwLock<HashMap<[u8; 32], DurableCommit>>,
    cycle_by_identity: RwLock<HashMap<[u8; 32], [u8; 32]>>,
}

impl Default for KnowledgeVault {
    fn default() -> Self {
        Self::with_backend(Box::new(InMemoryBackend))
    }
}

impl KnowledgeVault {
    /// Test-only observability for the no-knowledge-pollution assertions.
    /// Counts do not expose a lock, store, write guard, or admission capability.
    #[cfg(test)]
    pub(crate) fn store_counts_for_test(&self) -> [usize; 7] {
        [
            self.scientific.read().expect("scientific").len(),
            self.negative.read().expect("negative").len(),
            self.hypothesis.read().expect("hypothesis").len(),
            self.fact.read().expect("fact").len(),
            self.primitive.read().expect("primitive").len(),
            self.sovereign.read().expect("sovereign").len(),
            self.legacy.read().expect("legacy").len(),
        ]
    }

    pub fn new() -> Self {
        Self::default()
    }

    /// Krijon vault me një backend persistence të zgjedhur (RAM ose disk).
    pub fn with_backend(backend: Box<dyn VaultBackend>) -> Self {
        Self {
            scientific: Arc::new(RwLock::new(Vec::new())),
            negative: Arc::new(RwLock::new(CandidateStore::new())),
            hypothesis: Arc::new(RwLock::new(Vec::new())),
            fact: Arc::new(RwLock::new(Vec::new())),
            primitive: Arc::new(RwLock::new(CandidateStore::new())),
            sovereign: Arc::new(RwLock::new(Vec::new())),
            legacy: Arc::new(RwLock::new(CandidateStore::new())),
            observer: Arc::new(crate::legacy_observer::LegacyObserver::new()),
            backend,
            commit_lock: Mutex::new(()),
            pd_projection: RwLock::new(crate::pd_memory_store::PdProjection::default()),
            continuation_refs:RwLock::new(HashMap::new()),
            native_pd_heads: RwLock::new(HashMap::new()),
            native_iz_operations: RwLock::new(HashMap::new()),
            kie_adjudicated_refs: RwLock::new(HashMap::new()),
            kie_history: RwLock::new(crate::kie_history::HistoryIndex::default()),
            committed_by_cycle: RwLock::new(HashMap::new()),
            cycle_by_identity: RwLock::new(HashMap::new()),
        }
    }

    /// FAZA 2 — Hap vault me DURABILITET në disk; ripërsërit log-un ekzistues.
    /// Pas restart-it, gjendja sovrane rikrijohet EKZAKT nga WAL-i.
    pub fn open_disk(path: &std::path::Path) -> Result<Self, ShadowError> {
        // This projection is private until every complete frame has passed
        // decoding AND semantic replay. The same locked backend repairs a torn
        // suffix only after the final callback succeeds; no ReplayOp Vec lives
        // alongside the rebuilt stores. Global lineage is deferred as well.
        let mut vault = Self::new();
        let mut replay_lineages = HashMap::new();
        let backend = crate::vault_disk::DiskBackend::open_replaying(path, |op| {
            vault.replay_op(op, &mut replay_lineages).map_err(|error| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("semantic replay: {error:?}"),
                )
            })
        })
        .map_err(|e| ShadowError::VaultWriteError(format!("disk open: {e}")))?;
        for lineage in replay_lineages.into_values() {
            crate::knowledge_lineage::ledger().record_result(lineage)?;
        }
        vault.backend = Box::new(backend);
        Ok(vault)
    }

    /// Ripërsërit ngjarjet e regjistruara përmes TË NJËJTËS logjikë aplikimi
    /// (pa rilogim). Dedup-i negativ dhe promovimi Legacy riprodhohen besnikërisht.
    fn replay_op(
        &self,
        op: crate::vault_disk::ReplayOp,
        replay_lineages: &mut HashMap<u64, Lineage>,
    ) -> Result<(), ShadowError> {
        use crate::vault_disk::ReplayOp;
        match op {
            ReplayOp::Primitive {
                input_id,
                vector_points,
                temporal_score,
                mass,
                seal,
                path,
                ts,
                domain,
                bytes,
            } => {
                let trace = PrimitiveTrace {
                    domain,
                    timestamp: ts,
                    mass,
                    path_taken: ShadowPath::from_u8(path),
                    lgc_seal: seal,
                };
                // FAIL-LOUD (ULTIMATUM): dështim semantik gjatë replay = gjendje
                // e persistuar problematike → raportohet, s'kalon në heshtje.
                self.apply_primitive(input_id, bytes, vector_points, temporal_score, trace)?;
                // Riprodho promovimin ndër-domain (5-të domain → Legacy). Count, jo Result.
                let _ = self.try_promote_to_legacy_policy(false);
            }
            ReplayOp::Verified {
                input_id,
                band,
                mass,
                ts,
                bytes,
            } => {
                let kb = match band {
                    1 => KnowledgeBand::Hypothesis,
                    2 => KnowledgeBand::Fact,
                    3 => KnowledgeBand::Scientific,
                    _ => KnowledgeBand::Negative,
                };
                self.apply_verified(input_id, bytes, kb, mass, ts)?;
            }
            ReplayOp::Negative {
                input_id,
                reason,
                process,
                mass,
                frequency,
                ts,
                bytes,
            } => {
                self.apply_negative(input_id, reason, process, mass, frequency, bytes, ts)?;
            }
            ReplayOp::Maturation(record) => {
                self.run_maturation_action(Some(&record))?;
            }
            ReplayOp::Confirm { input_id, hits } => {
                let _ = self.apply_confirm(input_id, hits); // count, jo Result
            }
            ReplayOp::Transaction(tx) => {
                self.apply_committed_transaction_with_lineage(
                    tx,
                    CommitStatus::AlreadyCommitted,
                    Some(replay_lineages),
                )?;
            }
            ReplayOp::NativeContinuation(record) => self.apply_native_continuation(record)?,
            ReplayOp::PdOperation(record) => {
                record.validate().map_err(ShadowError::VaultWriteError)?;
                let mut projection = self.pd_projection.write()
                    .map_err(|_| ShadowError::VaultWriteError("PD replay lock poisoned".into()))?;
                match projection.prepare(record.request.clone()).map_err(ShadowError::VaultWriteError)? {
                    crate::pd_memory_store::PdCommitPlan::Existing(prior) => {
                        if prior.as_ref() != &record {
                            return Err(ShadowError::VaultWriteError("PD_REPLAY_OPERATION_CONFLICT".into()));
                        }
                    }
                    crate::pd_memory_store::PdCommitPlan::Append(append) => {
                        if append.record() != &record {
                            return Err(ShadowError::VaultWriteError("PD_REPLAY_CHAIN_CONFLICT".into()));
                        }
                        let previous = projection.head(&record.request.identity).map_err(ShadowError::VaultWriteError)?;
                        self.validate_pd_progression(&record.request, previous.as_ref())?;
                        projection.publish(append);
                    }
                }
            }
        };
        Ok(())
    }

    /// Operational PD is a typed projection over this same Vault/WAL. These
    /// APIs never create an epistemic verdict or mutate the seven knowledge stores.
    pub(crate) fn pd_head(
        &self,
        identity: &shadow_contracts::pd_memory::PdIdentity,
    ) -> Result<Option<shadow_contracts::pd_memory::PdOperationRecord>, ShadowError> {
        let delivery_scope = shadow_contracts::pd_memory::hex(&identity.key().map_err(ShadowError::VaultWriteError)?);
        crate::noi_caller::invoke("SHADOW_PD_HEAD", &delivery_scope, || {
        self.pd_projection
            .read()
            .map_err(|_| ShadowError::VaultWriteError("PD projection lock poisoned".into()))?
            .head(identity)
            .map_err(ShadowError::VaultWriteError)
    
        })
    }

    /// Read-only pd_mp references, derived atomically from the same Vault WAL.
    pub(crate) fn pd_history(&self, query:&shadow_contracts::pd_memory::history::PdHistoryLookup)->Result<shadow_contracts::pd_memory::history::PdHistoryPage,ShadowError> {
        let delivery_scope = shadow_contracts::pd_memory::hex(&query.identity.key().map_err(ShadowError::VaultWriteError)?);
        crate::noi_caller::invoke("SHADOW_PD_HISTORY", &delivery_scope, || {
        self.pd_projection.read().map_err(|_|ShadowError::VaultWriteError("PD projection lock poisoned".into()))?
            .history(query).map_err(ShadowError::VaultWriteError)
    
        })
    }

    /// Read-only exact LGC reference to the same Vault-owned operational record.
    pub(crate) fn pd_lgc_record(&self, reference:&shadow_contracts::pd_memory::lgc::PdLgcRef)
        ->Result<Option<shadow_contracts::pd_memory::PdOperationRecord>,ShadowError> {
        let delivery_scope = shadow_contracts::pd_memory::hex(&reference.record_id);
        crate::noi_caller::invoke("SHADOW_PD_LGC_READ", &delivery_scope, || {
        self.pd_projection.read().map_err(|_|ShadowError::VaultWriteError("PD projection lock poisoned".into()))?
            .lgc_record(reference).map_err(ShadowError::VaultWriteError)
    
        })
    }

    pub(crate) fn pd_operation(
        &self,
        identity: &shadow_contracts::pd_memory::PdIdentity,
        operation_id: &str,
    ) -> Result<Option<shadow_contracts::pd_memory::PdOperationRecord>, ShadowError> {
        let delivery_scope = shadow_contracts::pd_memory::hex(&identity.key().map_err(ShadowError::VaultWriteError)?);
        crate::noi_caller::invoke("SHADOW_PD_OPERATION", &delivery_scope, || {
        self.pd_projection
            .read()
            .map_err(|_| ShadowError::VaultWriteError("PD projection lock poisoned".into()))?
            .operation(identity, operation_id)
            .map_err(ShadowError::VaultWriteError)
    
        })
    }

    pub(crate) fn pd_candidate(
        &self,
        identity: &shadow_contracts::pd_memory::PdIdentity,
        query_digest: &shadow_contracts::pd_memory::Hash,
    ) -> Result<Option<shadow_contracts::pd_memory::PdOperationRecord>, ShadowError> {
        let delivery_scope = shadow_contracts::pd_memory::hex(&identity.key().map_err(ShadowError::VaultWriteError)?);
        crate::noi_caller::invoke("SHADOW_PD_CANDIDATE", &delivery_scope, || {
        self.pd_projection
            .read()
            .map_err(|_| ShadowError::VaultWriteError("PD projection lock poisoned".into()))?
            .candidate(identity, query_digest)
            .map_err(ShadowError::VaultWriteError)
    
        })
    }

    pub(crate) fn commit_pd_operation(
        &self,
        request: shadow_contracts::pd_memory::PdOperationRequest,
    ) -> Result<shadow_contracts::pd_memory::PdOperationRecord, ShadowError> {
        if shadow_contracts::pd_memory::late_refusal::source_is_late(&request.source_input) {
            return Err(ShadowError::VaultWriteError("PD_LATE_REFUSAL_SHADOW_OWNER_REQUIRED".into()));
        }
        self.commit_pd_operation_inner(request)
    }

    /// Only the owner handling the actual typed ingest result calls this path.
    /// The public operational endpoint cannot supply or re-seal this event.
    pub(crate) fn record_late_native_refusal(
        &self, attempt:&[u8], binding:&crate::native_cycle::NativeRequestBinding,
        observed:&ShadowError,
    ) -> Result<shadow_contracts::pd_memory::PdOperationRecord,ShadowError> {
        use shadow_contracts::pd_memory::late_refusal;
        let err=ShadowError::VaultWriteError;
        if !matches!(observed,ShadowError::StructuralRefusal(crate::types::StructuralRefusal::MissingNegativeProof)) {
            return Err(err("PD_LATE_NOT_OBSERVED_STRUCTURAL_REFUSAL".into()));
        }
        let wire=shadow_contracts::decode_cycle_request(attempt).map_err(|e|err(e.to_string()))?;
        let mut expected=crate::native_cycle::NativeRequestBinding::from_request(&wire,attempt).map_err(err)?;
        expected.prepared=binding.prepared.clone();
        let proof=shadow_contracts::kie::decode_proof(&binding.kie_proof).map_err(err)?;
        expected.attach_kie(&proof,&wire).map_err(err)?;
        if expected!=*binding {return Err(err("PD_LATE_NATIVE_BINDING".into()));}
        let parent=self.pd_operation(&proof.context.identity,&proof.context.operation_id)?
            .ok_or_else(||err("PD_LATE_ORIGINAL_RECORD_MISSING".into()))?;
        let request=late_refusal::request_from_observation(&parent,attempt.to_vec(),
            binding.prepared.clone(),binding.kie_proof.clone()).map_err(err)?;
        self.commit_pd_operation_inner(request)
    }

    /// Exact operational retry precedes current-head CAS and fresh judgment.
    /// A saved NoVerdict cannot be reinterpreted as a native response/receipt.
    pub(crate) fn replay_late_native_refusal(
        &self, attempt:&[u8], binding:&crate::native_cycle::NativeRequestBinding,
    ) -> Result<Option<shadow_contracts::pd_memory::PdOperationRecord>,ShadowError> {
        use shadow_contracts::pd_memory::late_refusal;
        let err=ShadowError::VaultWriteError;
        if binding.prepared.is_empty(){return Ok(None);}
        let prepared=shadow_contracts::pd_prepared::decode_prepared(&binding.prepared).map_err(err)?;
        let Some(link)=&prepared.source_dialogue else{return Ok(None)};
        let id=late_refusal::operation_id(&binding.request_sha256);
        let Some(record)=self.pd_operation(&link.context.identity,&id)? else{return Ok(None)};
        let wire=shadow_contracts::decode_cycle_request(attempt).map_err(|e|err(e.to_string()))?;
        late_refusal::validate_response(&record,&wire,&prepared).map_err(err)?;
        Ok(Some(record))
    }

    fn commit_pd_operation_inner(
        &self, request:shadow_contracts::pd_memory::PdOperationRequest,
    ) -> Result<shadow_contracts::pd_memory::PdOperationRecord,ShadowError> {
        // Same commit lock as epistemic transactions; one writer and one WAL.
        let _commit = self
            .commit_lock
            .lock()
            .map_err(|_| ShadowError::VaultWriteError("PD commit lock poisoned".into()))?;
        // Readers cannot see a partially published head/op/candidate index.
        // Holding this guard across sync also avoids post-sync lock acquisition.
        let mut projection = self
            .pd_projection
            .write()
            .map_err(|_| ShadowError::VaultWriteError("PD projection lock poisoned".into()))?;
        match projection
            .prepare(request)
            .map_err(ShadowError::VaultWriteError)?
        {
            crate::pd_memory_store::PdCommitPlan::Existing(record) => Ok(record.as_ref().clone()),
            crate::pd_memory_store::PdCommitPlan::Append(append) => {
                // Both reference domains are checked under the same commit lock.
                // An anchored inquiry cannot roll cognition back to the old PD head.
                let previous = projection.head(&append.record().request.identity)
                    .map_err(ShadowError::VaultWriteError)?;
                self.validate_pd_progression(&append.record().request, previous.as_ref())?;
                crate::noi_caller::invoke("SHADOW_PERSIST_PD", &shadow_contracts::pd_memory::hex(&append.record().record_id), || {
                    self.backend.commit_pd_operation(append.record())
                        .map_err(|error| ShadowError::VaultWriteError(error.to_string()))
                })?;
                Ok(projection.publish(append).as_ref().clone())
            }
        }
    }

    // Called under commit_lock for live appends, or before the replaying Vault
    // is exposed. Only map references are inspected; no material is copied.
    fn require_operational_pd_scope_unpromoted(
        &self, identity: &shadow_contracts::pd_memory::PdIdentity,
    ) -> Result<(), ShadowError> {
        let key = identity.key().map_err(ShadowError::VaultWriteError)?;
        let session = shadow_contracts::pd_memory::hex(&key);
        let scope = crate::native_cycle::cognitive_scope_key(&session, &identity.scope);
        if self.native_pd_heads.read()
            .map_err(|_| ShadowError::VaultWriteError("PD_NATIVE_HEAD_LOCK".into()))?
            .contains_key(&scope) {
            return Err(ShadowError::VaultWriteError(
                "PD_NATIVE_CONTINUATION_REQUIRED_AFTER_PROMOTION".into()));
        }
        Ok(())
    }

    // Called under commit_lock, or while rebuilding an unexposed Vault. This
    // resolves existing T1/T2 bytes only; it never finalizes an incomplete cycle.
    fn native_inquiry_bundle(
        &self, anchor: &shadow_contracts::pd_prepared::PdContinuationLink,
    ) -> Result<shadow_contracts::pd_recovery::RecoveryBundle, ShadowError> {
        let err = ShadowError::VaultWriteError;
        anchor.validate().map_err(err)?;
        let scope = crate::native_cycle::cognitive_scope_key(&anchor.cognitive_session_id, &anchor.territory);
        let head = self.native_pd_heads.read().map_err(|_| err("PD_NATIVE_HEAD_LOCK".into()))?
            .get(&scope).copied().ok_or_else(|| err("PD_NATIVE_PREDECESSOR_HEAD_MISSING".into()))?;
        if head.identity != anchor.predecessor.identity
            || head.request_sha256 != anchor.predecessor.request_sha256
            || head.transaction_id != anchor.predecessor_transaction_id {
            return Err(err("PD_NATIVE_PREDECESSOR_NOT_CURRENT_HEAD".into()));
        }
        let old = self.committed_identity(&head.identity)?
            .ok_or_else(|| err("PD_NATIVE_PREDECESSOR_T1_MISSING".into()))?;
        let snapshot = old.native_cycle.as_ref()
            .ok_or_else(|| err("PD_NATIVE_PREDECESSOR_SNAPSHOT_MISSING".into()))?;
        let continuation = self.continuation_refs.read()
            .map_err(|_| err("PD_NATIVE_PREDECESSOR_T2_LOCK".into()))?
            .get(&head.identity).cloned()
            .ok_or_else(|| err("PD_NATIVE_PREDECESSOR_T2_NOT_DURABLE".into()))?;
        continuation.validate_against(&old).map_err(|e| err(e.to_string()))?;
        let bundle = shadow_contracts::pd_recovery::RecoveryBundle {
            version: shadow_contracts::pd_recovery::RECOVERY_VERSION,
            identity: head.identity, request_sha256: snapshot.request_sha256,
            transaction_id: old.transaction_id, prepared: snapshot.prepared.clone(),
            response: snapshot.response.clone(), continuation: Some(continuation.material.clone()),
        };
        if shadow_contracts::pd_prepared::PdContinuationLink::from_bundle(&bundle).map_err(err)? != *anchor {
            return Err(err("PD_NATIVE_INQUIRY_ANCHOR_MISMATCH".into()));
        }
        Ok(bundle)
    }

    fn validate_pd_progression(
        &self, request: &shadow_contracts::pd_memory::PdOperationRequest,
        previous: Option<&shadow_contracts::pd_memory::PdOperationRecord>,
    ) -> Result<(), ShadowError> {
        use shadow_contracts::pd_memory::{iz, iz_science};
        if shadow_contracts::pd_memory::late_refusal::source_is_late(&request.source_input) {
            let err=ShadowError::VaultWriteError;
            let source=shadow_contracts::pd_memory::late_refusal::validate_operation(request).map_err(err)?;
            let parent=previous.ok_or_else(||err("PD_LATE_PARENT_MISSING".into()))?;
            shadow_contracts::pd_memory::late_refusal::validate_transition(request,parent).map_err(err)?;
            let wire=shadow_contracts::decode_cycle_request(&source.attempt).map_err(|e|err(e.to_string()))?;
            let mut binding=crate::native_cycle::NativeRequestBinding::from_request(&wire,&source.attempt).map_err(err)?;
            binding.prepared=source.prepared.clone();
            let proof=shadow_contracts::kie::decode_proof(&source.kie_proof).map_err(err)?;
            binding.attach_kie(&proof,&wire).map_err(err)?;
            if self.committed_identity(&binding.identity_digest)?.is_some() {
                return Err(err("PD_LATE_ATTEMPT_ALREADY_NATIVE".into()));
            }
            crate::kie_selection::validate_proof_model(&proof).map_err(err)?;
            crate::kie_selection::validate_prior_adjudication(self,&proof).map_err(err)?;
            self.validate_kie_history(&proof).map_err(err)?;
            let prepared=shadow_contracts::pd_prepared::decode_prepared(&source.prepared).map_err(err)?;
            match &prepared.continuity {
                Some(anchor)=>{
                    let bundle=self.native_inquiry_bundle(anchor)?;
                    prepared.validate_dialogue_continuation_source(&bundle,parent).map_err(err)
                }
                None=>self.require_operational_pd_scope_unpromoted(&request.identity),
            }?;
            return Ok(());
        }
        // Exact operational retries return before progression validation. A new
        // operational result cannot reuse an iZ operation already owned by T1.
        // Read this separate index while the caller holds commit_lock (or during
        // unexposed replay); never reacquire the operational projection lock.
        let operation = (request.identity.key().map_err(ShadowError::VaultWriteError)?,
            request.operation_id.clone());
        if self.native_iz_operations.read()
            .map_err(|_| ShadowError::VaultWriteError("PD_IZ_NATIVE_OPERATION_LOCK".into()))?
            .contains_key(&operation) {
            return Err(ShadowError::VaultWriteError("PD_IZ_OPERATION_ALREADY_NATIVE".into()));
        }
        let context = match request.source_input.get(..4) {
            Some(b"PIZ3") => iz::decode_source(&request.source_input).map_err(ShadowError::VaultWriteError)?.context,
            Some(b"PDS3") | Some(b"PDS5") => iz_science::decode_input(&request.source_input).map_err(ShadowError::VaultWriteError)?.context,
            _ => return self.require_operational_pd_scope_unpromoted(&request.identity),
        };
        let anchor = context.native_anchor.as_ref()
            .ok_or_else(|| ShadowError::VaultWriteError("PD_NATIVE_INQUIRY_ANCHOR_MISSING".into()))?;
        let bundle = self.native_inquiry_bundle(anchor)?;
        iz::validate_native_transition(request, &bundle, previous).map_err(ShadowError::VaultWriteError)
    }

    /// Pre-admission repeats at commit; this observation cannot waive either CAS.
    pub(crate) fn validate_pd_native_inquiry(
        &self, request: &shadow_contracts::pd_memory::PdOperationRequest,
    ) -> Result<(), ShadowError> {
        let _guard = self.commit_lock.lock()
            .map_err(|_| ShadowError::VaultWriteError("PD_NATIVE_MAP_COMMIT_LOCK".into()))?;
        if let Some(prior) = self.pd_operation(&request.identity, &request.operation_id)? {
            return if prior.request == *request { Ok(()) }
                else { Err(ShadowError::VaultWriteError("PD_OPERATION_CONFLICT".into())) };
        }
        let previous = self.pd_head(&request.identity)?;
        self.validate_pd_progression(request, previous.as_ref())
    }

    /// An atomic reference-only map over the existing native and operational indexes.
    pub(crate) fn pd_iz_native_map(
        &self, context: &shadow_contracts::pd_memory::iz::IzContext,
    ) -> Result<shadow_contracts::pd_memory::iz::IzNativeMapSnapshot, ShadowError> {
        let delivery_scope = shadow_contracts::pd_memory::hex(&context.identity.key().map_err(ShadowError::VaultWriteError)?);
        crate::noi_caller::invoke("SHADOW_NATIVE_MAP", &delivery_scope, || {
        use shadow_contracts::pd_memory::iz;
        let err = ShadowError::VaultWriteError;
        context.validate().map_err(err)?;
        let _guard = self.commit_lock.lock().map_err(|_| err("PD_NATIVE_MAP_COMMIT_LOCK".into()))?;
        let anchor = context.native_anchor.as_ref().ok_or_else(|| err("PD_NATIVE_INQUIRY_ANCHOR_MISSING".into()))?;
        self.native_inquiry_bundle(anchor)?;
        let head = self.pd_head(&context.identity)?;
        if head.as_ref().map(iz::IzParent::from_record).transpose().map_err(err)? != context.parent {
            return Err(err("PD_NATIVE_INQUIRY_OPERATIONAL_HEAD_MISMATCH".into()));
        }
        iz::native_map_for(&context.identity, anchor, head.as_ref()).map_err(err)
    
        })
    }

    /// Exact material read after the map. Rechecks both references atomically.
    /// No durable T2 is created by this read and no receipt is newly issued.
    pub(crate) fn pd_iz_native_read(
        &self, context: &shadow_contracts::pd_memory::iz::IzContext,
    ) -> Result<shadow_contracts::pd_recovery::RecoveryBundle, ShadowError> {
        let delivery_scope = shadow_contracts::pd_memory::hex(&context.identity.key().map_err(ShadowError::VaultWriteError)?);
        crate::noi_caller::invoke("SHADOW_NATIVE_READ", &delivery_scope, || {
        let err = ShadowError::VaultWriteError;
        context.validate().map_err(err)?;
        let _guard = self.commit_lock.lock().map_err(|_| err("PD_NATIVE_MAP_COMMIT_LOCK".into()))?;
        let anchor = context.native_anchor.as_ref().ok_or_else(|| err("PD_NATIVE_INQUIRY_ANCHOR_MISSING".into()))?;
        let bundle = self.native_inquiry_bundle(anchor)?;
        let head = self.pd_head(&context.identity)?;
        shadow_contracts::pd_memory::iz::validate_native_binding(context, &bundle, head.as_ref()).map_err(err)?;
        Ok(bundle)
    
        })
    }

    pub(crate) fn pd_iz_native_operational_read(
        &self, context: &shadow_contracts::pd_memory::iz::IzContext,
    ) -> Result<Option<shadow_contracts::pd_memory::PdOperationRecord>, ShadowError> {
        let delivery_scope = shadow_contracts::pd_memory::hex(&context.identity.key().map_err(ShadowError::VaultWriteError)?);
        crate::noi_caller::invoke("SHADOW_NATIVE_OPERATIONAL_READ", &delivery_scope, || {
        let err = ShadowError::VaultWriteError;
        context.validate().map_err(err)?;
        let _guard = self.commit_lock.lock().map_err(|_| err("PD_NATIVE_MAP_COMMIT_LOCK".into()))?;
        let anchor = context.native_anchor.as_ref().ok_or_else(|| err("PD_NATIVE_INQUIRY_ANCHOR_MISSING".into()))?;
        let bundle = self.native_inquiry_bundle(anchor)?;
        let head = self.pd_head(&context.identity)?;
        shadow_contracts::pd_memory::iz::validate_native_binding(context, &bundle, head.as_ref()).map_err(err)?;
        Ok(head)
    
        })
    }

    /// Read-only exact v5 operation lookup, including historical retries after
    /// another native turn advanced the current scope. Never finalizes T2.
    pub(crate) fn pd_iz_native_lookup(&self, context:&shadow_contracts::pd_memory::iz::IzContext)
        ->Result<Option<shadow_contracts::pd_recovery::RecoveryLookup>,ShadowError> {
        let delivery_scope = shadow_contracts::pd_memory::hex(&context.identity.key().map_err(ShadowError::VaultWriteError)?);
        crate::noi_caller::invoke("SHADOW_NATIVE_LOOKUP", &delivery_scope, || {
        let err=|reason:String|ShadowError::VaultWriteError(reason);
        context.validate().map_err(err)?;
        let _guard=self.commit_lock.lock().map_err(|_|err("PD_NATIVE_MAP_COMMIT_LOCK".into()))?;
        let key=(context.identity.key().map_err(err)?,context.operation_id.clone());
        let identity=self.native_iz_operations.read().map_err(|_|err("PD_IZ_NATIVE_OPERATION_LOCK".into()))?
            .get(&key).copied();
        let Some(identity)=identity else {return Ok(None)};
        let commit=self.committed_identity(&identity)?.ok_or_else(||err("PD_IZ_NATIVE_T1_REFERENCE_MISSING".into()))?;
        let snapshot=commit.native_cycle.ok_or_else(||err("PD_IZ_NATIVE_SNAPSHOT_MISSING".into()))?;
        let envelope=shadow_contracts::pd_prepared::decode_prepared(&snapshot.prepared).map_err(err)?;
        let link=envelope.source_iz.ok_or_else(||err("PD_IZ_NATIVE_SOURCE_MISSING".into()))?;
        if !shadow_contracts::pd_memory::iz::same_intent(context,&link.context) {
            return Err(err("PD_IZ_NATIVE_OPERATION_CONFLICT".into()));
        }
        Ok(Some(shadow_contracts::pd_recovery::RecoveryLookup{identity,request_sha256:snapshot.request_sha256}))
    
        })
    }

    fn native_iz_operation_key(tx:&VaultTransaction)->Result<Option<([u8;32],String)>,ShadowError>{
        let Some(snapshot)=&tx.native_cycle else {return Ok(None)};
        if snapshot.prepared.is_empty(){return Ok(None);}
        let envelope=shadow_contracts::pd_prepared::decode_prepared(&snapshot.prepared).map_err(ShadowError::VaultWriteError)?;
        envelope.source_iz.map(|link|Ok((link.context.identity.key().map_err(ShadowError::VaultWriteError)?,link.context.operation_id))).transpose()
    }
    fn reserve_native_iz_operation(&self,needed:bool)->Result<(),ShadowError>{
        if needed {self.native_iz_operations.write().map_err(|_|ShadowError::VaultWriteError("PD_IZ_NATIVE_OPERATION_LOCK".into()))?
            .try_reserve(1).map_err(|_|ShadowError::VaultWriteError("PD_IZ_NATIVE_OPERATION_ALLOCATION".into()))?;}
        Ok(())
    }

    /// R3 pre-judgment check. The final T1 repeats the same check under the
    /// commit lock, so an intervening successful turn cannot fork the topic.
    pub(crate) fn validate_native_pd_admission(
        &self, binding: &crate::native_cycle::NativeRequestBinding,
    ) -> Result<(), ShadowError> {
        let _guard = self.commit_lock.lock()
            .map_err(|_| ShadowError::VaultWriteError("PD_NATIVE_MAP_COMMIT_LOCK".into()))?;
        if let Some(old) = self.committed_identity(&binding.identity_digest)? {
            return match old.native_cycle {
                Some(snapshot) if snapshot.request_sha256 == binding.request_sha256
                    && snapshot.prepared == binding.prepared => Ok(()),
                _ => Err(ShadowError::VaultWriteError("PD_NATIVE_EXACT_RETRY_CONFLICT".into())),
            };
        }
        self.prepare_native_pd_head(binding.identity_digest, binding.request_sha256,
            [0; 32], &binding.prepared, false).map(|_| ())
    }

    /// Resolve actual stored T1 and already durable T2; this deliberately never
    /// calls recover/finalize while holding commit_lock or manufactures T2.
    fn prepare_native_pd_head(
        &self, identity: [u8; 32], request_sha256: [u8; 32], transaction_id: [u8; 32],
        prepared: &[u8], historical_replay: bool,
    ) -> Result<Option<([u8; 32], crate::native_cycle::NativePdHead)>, ShadowError> {
        if prepared.is_empty() { return Ok(None); }
        let err = |reason: String| ShadowError::VaultWriteError(reason);
        let envelope = shadow_contracts::pd_prepared::decode_prepared(prepared).map_err(err)?;
        if envelope.source_request_sha256 != request_sha256 {
            return Err(err("PD_NATIVE_MAP_SOURCE_REQUEST".into()));
        }
        if let Some(link)=&envelope.source_iz {
            // One logical iZ operation cannot acquire a second result through a
            // different route. Exact native T1 retries returned before this gate.
            // source_dialogue intentionally references a stored PDS operation.
            if self.pd_operation(&link.context.identity, &link.context.operation_id)?.is_some() {
                return Err(err("PD_IZ_OPERATION_ALREADY_OPERATIONAL".into()));
            }
            let key=(link.context.identity.key().map_err(err)?,link.context.operation_id.clone());
            if self.native_iz_operations.read().map_err(|_|err("PD_IZ_NATIVE_OPERATION_LOCK".into()))?.contains_key(&key) {
                // Exact T1 retry returns before this check. A new physical T1
                // cannot reuse the same logical operation with a new verdict.
                return Err(err("PD_IZ_NATIVE_OPERATION_CONFLICT".into()));
            }
        }
        let metadata = shadow_contracts::pd_checkpoint_data::checkpoint_metadata(
            &envelope.checkpoint, envelope.cognitive_session_id()).map_err(err)?;
        let scope = crate::native_cycle::cognitive_scope_key(&metadata.session_id, &metadata.territory);
        let head = self.native_pd_heads.read()
            .map_err(|_| err("PD_NATIVE_HEAD_LOCK".into()))?.get(&scope).copied();
        let root_identity = match &envelope.continuity {
            None => {
                if let Some(link)=&envelope.source_iz {
                    if head.is_some(){return Err(err("PD_NATIVE_IZ_ALREADY_PROMOTED".into()));}
                    let latest=self.pd_head(&link.context.identity)?;
                    match (&link.context.parent,latest.as_ref()) {
                        (None,None)=>envelope.validate_iz_initial().map_err(err)?,
                        (Some(parent),Some(source)) if source.record_id==parent.record_id=>{
                            envelope.validate_iz_source(source).map_err(err)?;
                        },
                        (None,Some(_))=>return Err(err("PD_NATIVE_IZ_INITIAL_HAS_OPERATIONAL_HISTORY".into())),
                        _=>return Err(err("PD_NATIVE_IZ_NOT_CURRENT_HEAD".into())),
                    }
                }
                if let Some(link)=&envelope.source_dialogue {
                    if head.is_some() {return Err(err("PD_NATIVE_DIALOGUE_ALREADY_PROMOTED".into()));}
                    let source=self.pd_operation(&link.context.identity,&link.context.operation_id)?
                        .ok_or_else(||err("PD_NATIVE_DIALOGUE_SOURCE_MISSING".into()))?;
                    let latest=self.pd_head(&link.context.identity)?.ok_or_else(||err("PD_NATIVE_DIALOGUE_HEAD_MISSING".into()))?;
                    if latest.record_id!=source.record_id {return Err(err("PD_NATIVE_DIALOGUE_NOT_CURRENT_HEAD".into()));}
                    envelope.validate_dialogue_source(&source).map_err(err)?;
                }
                // Earlier v1 WAL records remain readable without retroactive
                // migration. A new v1 cannot silently reset an existing scope.
                if head.is_some() && !historical_replay {
                    return Err(err("PD_NATIVE_COGNITIVE_SCOPE_ALREADY_EXISTS".into()));
                }
                identity
            }
            Some(link) => {
                let head = head.ok_or_else(|| err("PD_NATIVE_PREDECESSOR_HEAD_MISSING".into()))?;
                if head.identity != link.predecessor.identity
                    || head.request_sha256 != link.predecessor.request_sha256
                    || head.transaction_id != link.predecessor_transaction_id
                    || identity == head.identity {
                    return Err(err("PD_NATIVE_PREDECESSOR_NOT_CURRENT_HEAD".into()));
                }
                let bundle = self.native_inquiry_bundle(link)?;
                match envelope.version {
                    shadow_contracts::pd_prepared::PREPARED_IZ_CONTINUITY_VERSION => {
                        let origin = envelope.source_iz.as_ref().ok_or_else(|| err("PD_NATIVE_IZ_SOURCE_MISSING".into()))?;
                        let latest = self.pd_head(&origin.context.identity)?;
                        envelope.validate_iz_continuation_source(&bundle, latest.as_ref()).map_err(err)?;
                    }
                    shadow_contracts::pd_prepared::PREPARED_DIALOGUE_CONTINUITY_VERSION => {
                        let origin = envelope.source_dialogue.as_ref().ok_or_else(|| err("PD_NATIVE_DIALOGUE_SOURCE_MISSING".into()))?;
                        let source = self.pd_operation(&origin.context.identity, &origin.context.operation_id)?
                            .ok_or_else(|| err("PD_NATIVE_DIALOGUE_SOURCE_MISSING".into()))?;
                        let latest = self.pd_head(&origin.context.identity)?
                            .ok_or_else(|| err("PD_NATIVE_DIALOGUE_HEAD_MISSING".into()))?;
                        if latest.record_id != source.record_id {
                            return Err(err("PD_NATIVE_DIALOGUE_NOT_CURRENT_HEAD".into()));
                        }
                        envelope.validate_dialogue_continuation_source(&bundle, &source).map_err(err)?;
                    }
                    _ => {
                        envelope.validate_predecessor(&bundle).map_err(err)?;
                        // A native-only successor cannot skip actual user turns
                        // appended under its current durable native predecessor.
                        let latest = self.pd_projection.read().map_err(|_| err("PD projection lock poisoned".into()))?
                            .head_for_cognitive_session(&link.cognitive_session_id, &link.territory).map_err(err)?;
                        if let Some(record) = latest {
                            if matches!(record.request.source_input.get(..4), Some(b"PIZ3") | Some(b"PDS3") | Some(b"PDS5")) {
                                let context = shadow_contracts::pd_memory::iz::context_from_record(&record).map_err(err)?;
                                if context.native_anchor.as_ref() == Some(link) {
                                    return Err(err("PD_NATIVE_PENDING_INQUIRY_REQUIRES_EXACT_SOURCE".into()));
                                }
                            }
                        }
                    }
                }
                head.root_identity
            }
        };
        Ok(Some((scope, crate::native_cycle::NativePdHead {
            identity, request_sha256, transaction_id, root_identity,
        })))
    }
    fn prepare_native_pd_transaction(
        &self, tx: &VaultTransaction, historical_replay: bool,
    ) -> Result<Option<([u8; 32], crate::native_cycle::NativePdHead)>, ShadowError> {
        match &tx.native_cycle {
            None => Ok(None),
            Some(snapshot) => self.prepare_native_pd_head(tx.identity_digest,
                snapshot.request_sha256, tx.transaction_id, &snapshot.prepared, historical_replay),
        }
    }
    fn reserve_native_pd_head(&self, needed: bool) -> Result<(), ShadowError> {
        if needed {
            self.native_pd_heads.write()
                .map_err(|_| ShadowError::VaultWriteError("PD_NATIVE_HEAD_LOCK".into()))?
                .try_reserve(1)
                .map_err(|_| ShadowError::VaultWriteError("PD_NATIVE_HEAD_ALLOCATION".into()))?;
        }
        Ok(())
    }

    /// Exact scoped historical lookup, preserving MODEL_CONDITIONAL origin and
    /// the actual common-filter receipt separately. It is not a P/N-statistics API.
    pub(crate) fn kie_adjudicated(&self, identity:&shadow_contracts::pd_memory::PdIdentity,
        proposition:&str, query_bytes:&[u8]) -> Result<Option<crate::native_cycle::KieAdjudication>,ShadowError> {
        let err=|reason:String|ShadowError::VaultWriteError(reason);
        if proposition.is_empty() || proposition.len()>shadow_contracts::kie::MAX_TEXT_BYTES {
            return Err(err("KIE_PROPOSITION_LIMIT".into()));
        }
        let query=science_domains::decode_query(query_bytes).map_err(|e|err(e.to_string()))?;
        if query.session_id!=shadow_contracts::pd_memory::hex(&identity.key().map_err(err)?)
            || query.case_id!=identity.case_id || query.scope_id!=identity.scope || query.obligation_id!=identity.obligation {
            return Err(err("KIE_HISTORY_QUERY_SCOPE".into()));
        }
        let candidate=science_domains::evaluate(&query).map_err(|e|err(e.to_string()))?;
        let key=crate::native_cycle::KieHistoryKey {scope:identity.key().map_err(err)?,
            proposition:crate::vault_transaction::sha256_parts(&[b"ESSMAI/KIE/PROPOSITION/1",proposition.as_bytes()]),
            family:crate::native_cycle::kie_family(&candidate)};
        let reference=self.kie_adjudicated_refs.read().map_err(|_|err("KIE_HISTORY_LOCK".into()))?
            .get(&key).copied();
        let Some(reference)=reference else {return Ok(None)};
        let commit=self.committed_identity(&reference)?.ok_or_else(||err("KIE_HISTORY_T1_MISSING".into()))?;
        let snapshot=commit.native_cycle.as_ref().ok_or_else(||err("KIE_HISTORY_SNAPSHOT_MISSING".into()))?;
        let proof=shadow_contracts::kie::decode_proof(&snapshot.kie_proof).map_err(err)?;
        if proof.context.identity!=*identity || proof.canonical_proposition!=proposition {
            return Err(err("KIE_HISTORY_REFERENCE_MISMATCH".into()));
        }
        Ok(Some(crate::native_cycle::KieAdjudication {commit,evidence_family_digest:key.family}))
    }
    fn prepare_kie_adjudication(&self,tx:&VaultTransaction)
        ->Result<Option<crate::native_cycle::KieHistoryKey>,ShadowError> {
        crate::kie_history::validate_new_markers(tx).map_err(ShadowError::VaultWriteError)?;
        let Some(snapshot)=&tx.native_cycle else {return Ok(None)};
        if snapshot.kie_proof.is_empty() {return Ok(None)}
        let err=|reason:String|ShadowError::VaultWriteError(reason);
        let proof=shadow_contracts::kie::decode_proof(&snapshot.kie_proof).map_err(err)?;
        crate::kie_selection::validate_proof_model(&proof).map_err(err)?;
        // Hashes/source labels alone are insufficient: the PDS1 must already be
        // in this same authoritative Vault, before this T1 in WAL order.
        let source=self.pd_operation(&proof.context.identity,&proof.context.operation_id)?
            .ok_or_else(||err("KIE_SOURCE_RECORD_NOT_DURABLE".into()))?;
        if source.record_id!=proof.context.record_id {return Err(err("KIE_SOURCE_RECORD_MISMATCH".into()));}
        shadow_contracts::pd_memory::linked_science::validate_science_record(&source).map_err(err)?;
        let source_query=shadow_contracts::pd_memory::linked_science::science_query_from_source(&source.request.source_input).map_err(err)?;
        if source_query!=proof.query_bytes || source.request.candidate!=proof.candidate_bytes {
            return Err(err("KIE_SOURCE_MATERIAL_CHANGED".into()));
        }
        let candidate=science_domains::decode_candidate(&proof.candidate_bytes).map_err(|e|err(e.to_string()))?;
        if source.request.candidate_key!=candidate.query_sha256 {return Err(err("KIE_SOURCE_QUERY_KEY".into()));}
        // Historical proof references resolve against immutable preceding T1,
        // not the current map head, so later records cannot invalidate replay.
        crate::kie_selection::validate_prior_adjudication(self,&proof).map_err(err)?;
        self.validate_kie_history(&proof).map_err(err)?;
        Ok(Some(crate::native_cycle::KieHistoryKey {
            scope:proof.context.identity.key().map_err(err)?,
            proposition:crate::vault_transaction::sha256_parts(&[b"ESSMAI/KIE/PROPOSITION/1",proof.canonical_proposition.as_bytes()]),
            family:crate::native_cycle::kie_family(&candidate),
        }))
    }
    pub(crate) fn kie_history_assessment(&self,proof:&shadow_contracts::kie::KieProof)
        ->Result<shadow_contracts::kie::KieHistoryAssessment,String> {
        self.kie_history.read().map_err(|_|"KIE_HISTORY_PROJECTION_LOCK".to_string())?.assess(proof)
    }
    pub(crate) fn validate_kie_history(&self,proof:&shadow_contracts::kie::KieProof)->Result<(),String> {
        match &proof.history_assessment {
            None => Ok(()), // KIP2 sealed records keep their exact old policy.
            Some(expected) => match self.kie_history_assessment(proof)? == *expected {
                true => Ok(()), false => Err("KIE_HISTORY_ACTUAL_SOURCE_MISMATCH".into()),
            },
        }
    }

    /// Owner-only projection over immutable, validated T1 source descriptors.
    /// It reads the exact scoped index, never the aggregated NegativeStore.
    pub(crate) fn presented_matrix_counterproofs(
        &self, proof: &shadow_contracts::kie::KieProof,
    ) -> Result<crate::kie_history::VerifiedCounterproofProjection, String> {
        crate::kie_selection::validate_proof_model(proof)?;
        let projection = self.kie_history.read()
            .map_err(|_| "KIE_HISTORY_PROJECTION_LOCK".to_string())?
            .project_presented_counterproofs(proof)?;
        if !projection.matches_proof(proof)
            || projection.proposition_sha256() != shadow_contracts::kie::digest(proof.canonical_proposition.as_bytes()) {
            return Err("CURRENT_MATRIX_PROOF_BINDING".into());
        }
        Ok(projection)
    }
    fn reserve_kie_adjudication(&self,needed:bool)->Result<(),ShadowError> {
        if needed {
            self.kie_adjudicated_refs.write().map_err(|_|ShadowError::VaultWriteError("KIE_HISTORY_LOCK".into()))?
                .try_reserve(1).map_err(|_|ShadowError::VaultWriteError("KIE_HISTORY_ALLOCATION".into()))?;
        }
        Ok(())
    }

    /// T2 lookup/derivation takes the receipt and prepared state from T1 in this Vault.
    pub(crate) fn finalize_native_continuation(&self,identity:[u8;32],request_sha256:[u8;32])->Result<crate::native_continuation::NativeContinuationRecord,ShadowError>{
        let _guard=self.commit_lock.lock().map_err(|_|ShadowError::VaultWriteError("PD_T2_COMMIT_LOCK".into()))?;
        let commit=self.committed_identity(&identity)?.ok_or_else(||ShadowError::VaultWriteError("PD_T1_NOT_FOUND".into()))?;
        let record=crate::noi_caller::invoke("SHADOW_FORM_T2", &shadow_contracts::pd_memory::hex(&identity),
            || crate::native_continuation::NativeContinuationRecord::derive(identity,request_sha256,&commit).map_err(ShadowError::VaultWriteError))?;
        let mut refs=self.continuation_refs.write().map_err(|_|ShadowError::VaultWriteError("PD_T2_MAP_LOCK".into()))?;
        if let Some(prior)=refs.get(&identity) {
            return if prior.as_ref()==&record {Ok(prior.as_ref().clone())} else {Err(ShadowError::VaultWriteError("PD_T2_CONFLICT".into()))};
        }
        refs.try_reserve(1).map_err(|_|ShadowError::VaultWriteError("PD_T2_ALLOCATION".into()))?;
        let owned=Arc::new(record);
        crate::noi_caller::invoke("SHADOW_PERSIST_T2", &shadow_contracts::pd_memory::hex(&identity),
            || self.backend.commit_native_continuation(&owned).map_err(|e|ShadowError::VaultWriteError(e.to_string())))?;
        refs.insert(identity,Arc::clone(&owned));
        Ok(owned.as_ref().clone())
    }
    /// Explicit historic lookup; no "last conversation" fallback and no rejudgment.
    pub(crate) fn recover_native(&self, lookup:&shadow_contracts::pd_recovery::RecoveryLookup)
        ->Result<shadow_contracts::pd_recovery::RecoveryBundle,ShadowError> {
        let delivery_scope = shadow_contracts::pd_memory::hex(&lookup.identity);
        crate::noi_caller::invoke("SHADOW_NATIVE_RECOVERY", &delivery_scope, || {
        let continuation=self.finalize_native_continuation(lookup.identity,lookup.request_sha256)?;
        let commit=self.committed_identity(&lookup.identity)?
            .ok_or_else(||ShadowError::VaultWriteError("PD_RECOVERY_T1_MISSING".into()))?;
        let snapshot=commit.native_cycle
            .ok_or_else(||ShadowError::VaultWriteError("PD_RECOVERY_SNAPSHOT_MISSING".into()))?;
        let bundle=shadow_contracts::pd_recovery::RecoveryBundle {
            version:shadow_contracts::pd_recovery::RECOVERY_VERSION,
            identity:lookup.identity, request_sha256:lookup.request_sha256,
            transaction_id:commit.transaction_id, prepared:snapshot.prepared,
            response:snapshot.response, continuation:Some(continuation.material),
        };
        bundle.validate_for(lookup).map_err(ShadowError::VaultWriteError)?;
        Ok(bundle)
    
        })
    }
    fn apply_native_continuation(&self,record:crate::native_continuation::NativeContinuationRecord)->Result<(),ShadowError>{
        let commit=self.committed_identity(&record.identity)?.ok_or_else(||ShadowError::VaultWriteError("PD_T2_WITHOUT_T1".into()))?;
        record.validate_against(&commit).map_err(|e|ShadowError::VaultWriteError(e.to_string()))?;
        let mut refs=self.continuation_refs.write().map_err(|_|ShadowError::VaultWriteError("PD_T2_MAP_LOCK".into()))?;
        if let Some(prior)=refs.get(&record.identity) {
            if prior.as_ref()!=&record {return Err(ShadowError::VaultWriteError("PD_T2_REPLAY_CONFLICT".into()));}
            return Ok(());
        }
        refs.try_reserve(1).map_err(|_|ShadowError::VaultWriteError("PD_T2_ALLOCATION".into()))?;
        refs.insert(record.identity,Arc::new(record));Ok(())
    }

    /// Kthen commit-in e një cikli të komituar; retry merr transaction ID dhe
    /// receipt-in ekzakt pa rigjykim ose side-effect të dytë.
    pub fn committed_transaction(&self, cycle_digest: &[u8; 32]) -> Option<DurableCommit> {
        self.committed_by_cycle
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(cycle_digest)
            .cloned()
            .map(|commit| DurableCommit {
                transaction_id: commit.transaction_id,
                cycle_digest: commit.cycle_digest,
                status: CommitStatus::AlreadyCommitted,
                receipt: commit.receipt,
                native_cycle: commit.native_cycle,
            })
    }

    /// Exact original identity lookup for native replay BEFORE admission side
    /// effects/judgment. Missing back-reference is corruption, never a miss.
    pub(crate) fn committed_identity(
        &self,
        identity: &[u8; 32],
    ) -> Result<Option<DurableCommit>, ShadowError> {
        let cycle = self
            .cycle_by_identity
            .read()
            .map_err(|_| ShadowError::VaultWriteError("identity index lock poisoned".into()))?
            .get(identity)
            .copied();
        match cycle {
            None => Ok(None),
            Some(cycle) => {
                let mut commit = self
                    .committed_by_cycle
                    .read()
                    .map_err(|_| ShadowError::VaultWriteError("commit index lock poisoned".into()))?
                    .get(&cycle)
                    .cloned()
                    .ok_or_else(|| {
                        ShadowError::VaultWriteError(
                            "identity map without committed receipt".into(),
                        )
                    })?;
                commit.status = CommitStatus::AlreadyCommitted;
                Ok(Some(commit))
            }
        }
    }

    pub fn committed_receipt(
        &self,
        cycle_digest: &[u8; 32],
    ) -> Option<crate::sovereign_ffi_gate::VerificationReceipt> {
        self.committed_transaction(cycle_digest)
            .map(|commit| commit.receipt)
    }

    /// Njësia e vetme authoritative e materializimit Shadow.
    /// WAL/fsync përfundon përpara çdo mutacioni RAM/lineage/reinforcement.
    pub(crate) fn commit_verified(
        &self,
        authorized: crate::shadow_gateway::AuthorizedLiveTransaction<'_>,
    ) -> Result<DurableCommit, ShadowError> {
        let tx = authorized.into_transaction_for(self)?;
        tx.validate_for_live().map_err(|error| ShadowError::VaultWriteError(error.to_string()))?;
        self.commit_transaction_inner(tx)
    }

    /// Existing synthetic storage fixtures have no compute authority. This
    /// entry exists only in test binaries, explicitly separate from live writes.
    #[cfg(test)]
    pub(crate) fn commit_test_transaction(
        &self, tx: VaultTransaction,
    ) -> Result<DurableCommit, ShadowError> {
        self.commit_transaction_inner(tx)
    }

    fn commit_transaction_inner(
        &self, tx: VaultTransaction,
    ) -> Result<DurableCommit, ShadowError> {
        let _guard = self.commit_lock.lock().map_err(|error| {
            ShadowError::VaultWriteError(format!("commit lock poisoned: {error}"))
        })?;
        tx.validate()
            .map_err(|error| ShadowError::VaultWriteError(error.to_string()))?;
        let prior_cycle = self
            .cycle_by_identity
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&tx.identity_digest)
            .copied();
        match prior_cycle {
            Some(cycle) => match cycle == tx.cycle_digest {
                true => {
                    let commit = self.committed_transaction(&cycle).ok_or_else(|| {
                        ShadowError::VaultWriteError(
                            "identity map pa committed receipt".to_string(),
                        )
                    })?;
                    return match commit.transaction_id == tx.transaction_id {
                        true => Ok(commit),
                        false => Err(ShadowError::VaultWriteError(
                            VaultError::ConflictingReplay.to_string(),
                        )),
                    };
                }
                false => {
                    return Err(ShadowError::VaultWriteError(
                        VaultError::ConflictingReplay.to_string(),
                    ))
                }
            },
            None => {}
        }
        let native_head = self.prepare_native_pd_transaction(&tx, false)?;
        self.reserve_native_pd_head(native_head.is_some())?;
        let native_iz_key=Self::native_iz_operation_key(&tx)?;
        self.reserve_native_iz_operation(native_iz_key.is_some())?;
        let kie_key=self.prepare_kie_adjudication(&tx)?;
        self.reserve_kie_adjudication(kie_key.is_some())?;
        // All new fallible source classification and lock checks happen before
        // durable append. commit_lock excludes another index writer; readers
        // cannot poison this RwLock between this check and publication.
        let _history_preflight=crate::kie_history::prepare(&tx).map_err(ShadowError::VaultWriteError)?;
        drop(self.kie_history.write().map_err(|_|ShadowError::VaultWriteError("KIE_HISTORY_PROJECTION_LOCK".into()))?);
        crate::noi_caller::invoke("SHADOW_PERSIST_T1", &shadow_contracts::pd_memory::hex(&tx.identity_digest), || {
            self.backend.append_verified(KnowledgeAppend {
                transaction: &tx,
                backend: self.backend.as_ref(),
            }).map_err(|error| ShadowError::VaultWriteError(error.to_string()))
        })?;
        self.apply_committed_transaction(tx, CommitStatus::Committed)
    }

    fn apply_committed_transaction(
        &self,
        tx: VaultTransaction,
        status: CommitStatus,
    ) -> Result<DurableCommit, ShadowError> {
        self.apply_committed_transaction_with_lineage(tx, status, None)
    }

    fn apply_committed_transaction_with_lineage(
        &self,
        tx: VaultTransaction,
        status: CommitStatus,
        replay_lineages: Option<&mut HashMap<u64, Lineage>>,
    ) -> Result<DurableCommit, ShadowError> {
        tx.validate()
            .map_err(|error| ShadowError::VaultWriteError(error.to_string()))?;
        let prior_cycle = self
            .cycle_by_identity
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&tx.identity_digest)
            .copied();
        match prior_cycle {
            Some(cycle) => match cycle == tx.cycle_digest {
                true => {
                    let commit = self.committed_transaction(&cycle).ok_or_else(|| {
                        ShadowError::VaultWriteError(
                            "replay identity map pa committed receipt".to_string(),
                        )
                    })?;
                    return match commit.transaction_id == tx.transaction_id {
                        true => Ok(commit),
                        false => Err(ShadowError::VaultWriteError(
                            VaultError::ConflictingReplay.to_string(),
                        )),
                    };
                }
                false => {
                    return Err(ShadowError::VaultWriteError(
                        VaultError::ConflictingReplay.to_string(),
                    ))
                }
            },
            None => {}
        }
        // Semantic predecessor validation precedes every knowledge/projection
        // mutation, including streaming WAL replay and torn-tail repair.
        let native_head = self.prepare_native_pd_transaction(&tx, replay_lineages.is_some())?;
        self.reserve_native_pd_head(native_head.is_some())?;
        let native_iz_key=Self::native_iz_operation_key(&tx)?;
        self.reserve_native_iz_operation(native_iz_key.is_some())?;
        let kie_key=self.prepare_kie_adjudication(&tx)?;
        self.reserve_kie_adjudication(kie_key.is_some())?;
        let history_origins=crate::kie_history::prepare(&tx).map_err(ShadowError::VaultWriteError)?;
        let mut history=self.kie_history.write().map_err(|_|ShadowError::VaultWriteError("KIE_HISTORY_PROJECTION_LOCK".into()))?;
        let VaultTransaction {
            exact_candidate_identity,
            transaction_id,
            identity_digest,
            cycle_digest,
            anchor_generation: _anchor_generation,
            negative_batch_digest: _negative_batch_digest,
            runtime_delta_digest: _runtime_delta_digest,
            vds_close_digest: _vds_close_digest,
            law0_digest: _law0_digest,
            verdict_kind: _verdict_kind,
            y: _y,
            x: _x,
            living_trust_sha256: _living_trust_sha256,
            knowledge_writes,
            lineage,
            gcl_reinforcement,
            receipt_digest: _receipt_digest,
            receipt,
            native_cycle,
        } = tx;
        for (write, pending) in knowledge_writes.into_iter().zip(history_origins) {
            match write {
                KnowledgeWrite::Primitive {
                    input_id,
                    vector_points,
                    temporal_score,
                    mass,
                    seal,
                    path,
                    domain,
                    raw_bytes,
                    ts_ns,
                } => {
                    let trace = PrimitiveTrace {
                        domain,
                        timestamp: ts_ns,
                        mass,
                        path_taken: path,
                        lgc_seal: seal,
                    };
                    self.apply_primitive_policy(
                        input_id, raw_bytes, vector_points, temporal_score, trace,
                        exact_candidate_identity, |entry| history.publish_positive(pending, entry),
                    )?;
                }
                KnowledgeWrite::Negative {
                    input_id,
                    raw_bytes,
                    context,
                    ts_ns,
                } => {
                    self.apply_negative_policy(
                        input_id, context.failure_reason_code, context.process,
                        context.mass_at_fail, context.frequency, raw_bytes, ts_ns,
                        exact_candidate_identity, |entry| history.publish_negative(pending, entry),
                    )?;
                }
            }
        }
        // Each origin is bound to the exact row returned by its application.
        // No second lookup can substitute an FNV slot or a similar-mass row.
        drop(history);
        match replay_lineages {
            Some(staged) => {
                staged.try_reserve(1).map_err(|error| {
                    ShadowError::VaultWriteError(format!("replay lineage allocation: {error}"))
                })?;
                staged.insert(lineage.input_id, lineage);
            }
            None => crate::knowledge_lineage::ledger().record_result(lineage)?,
        }
        match gcl_reinforcement {
            Some(reinforcement) => {
                self.apply_confirm_policy(reinforcement.primitive_anchor, reinforcement.hits, exact_candidate_identity);
            }
            None => {}
        }
        let commit = DurableCommit {
            transaction_id,
            cycle_digest,
            status,
            receipt,
            native_cycle,
        };
        // Publish the full record before its identity becomes discoverable.
        // The commit lock serializes writers; identity readers then see either
        // absence or a complete exact-replay record, never a dangling reference.
        self.committed_by_cycle
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(cycle_digest, commit.clone());
        self.cycle_by_identity
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(identity_digest, cycle_digest);
        if let Some(key)=kie_key {
            // Updating a family reference does not overwrite historical T1 or
            // turn one repeated model calculation into independent evidence.
            self.kie_adjudicated_refs.write()
                .map_err(|_|ShadowError::VaultWriteError("KIE_HISTORY_LOCK".into()))?
                .insert(key,identity_digest);
        }
        if let Some(key)=native_iz_key {
            self.native_iz_operations.write()
                .map_err(|_|ShadowError::VaultWriteError("PD_IZ_NATIVE_OPERATION_LOCK".into()))?
                .insert(key,identity_digest);
        }
        if let Some((scope, head)) = native_head {
            self.native_pd_heads.write()
                .map_err(|_| ShadowError::VaultWriteError("PD_NATIVE_HEAD_LOCK".into()))?
                .insert(scope, head);
        }
        Ok(commit)
    }

    // ── READ API — çdo modul mund të lexojë ─────────────────────────────

    pub fn query_scientific_similarity(&self, candidate_scores: &[f32]) -> f32 {
        let store = self.scientific.read().unwrap_or_else(|p| p.into_inner());
        match store.is_empty() {
            true => return 0.0,
            false => {}
        }
        let sum: f32 = candidate_scores.iter().copied().sum();
        (sum / candidate_scores.len().max(1) as f32).clamp(0.0, 1.0)
    }

    pub fn check_sovereign_conflict(&self, reasoning_strength: f32) -> f32 {
        // PJESA C: store-i SOVRAN (vendimet supreme) — nëse lock-u helmohet, njofto
        // PARA rikuperimit. Rikuperimi mbetet fail-safe, por gjendja duhet audituar.
        let store = self.sovereign.read().unwrap_or_else(|p| {
            eprintln!("[SOVEREIGN] ANOMALY: sovereign store lock poisoned — recovering, vendimet sovrane duhet audituar");
            p.into_inner()
        });
        match store.is_empty() {
            true => return 0.0,
            false => {}
        }
        // Vendimi sovran është binar (0/1): numëro refuzimet si sasi diskrete,
        // pastaj normalizo vetëm në kufirin e score-it. Kjo shmang aritmetikën
        // float mbi një counter dhe nuk lejon vlera negative për decision jo-kanonik.
        let rejected: usize = store.iter().filter(|e| e.decision == 0).count();
        let conflict = rejected as f32 / store.len() as f32;
        (conflict * (1.0 - reasoning_strength * 0.5)).clamp(0.0, 1.0)
    }

    pub fn query_negative_pattern(&self, epistemic_mass: f32) -> f32 {
        let store = self.negative.read().unwrap_or_else(|p| p.into_inner());
        match store.is_empty() {
            true => return 0.0,
            false => {}
        }
        // Process observations remain in storage, but neither contribute to nor
        // dilute the source negative-density formula for actual counter-evidence.
        let (similar, count) = store.iter()
            .filter(|e| !shadow_contracts::negative_asset::is_non_refuting_process(
                e.failure_reason_code, &e.process, &e.raw_bytes))
            .fold((0.0f32, 0usize), |(sum, count), e| {
                let close = (NEG_MASS_TOLERANCE - (e.mass_at_fail - epistemic_mass).abs()).max(0.0)
                    / NEG_MASS_TOLERANCE;
                (sum + close * (e.frequency as f32).min(10.0) / 10.0, count + 1)
            });
        match count {
            0 => 0.0,
            _ => (similar / count as f32).clamp(0.0, 1.0),
        }
    }

    /// NEGATIVE SI KUFI AKTIV — a është kjo rrugë tashmë e njohur si negative?
    /// Kthen kontekstin (shkak/proces/masë/frekuencë) dhe rrit access_count.
    pub fn check_known_negative(
        &self,
        epistemic_mass: f32,
        territory: &str,
    ) -> Option<NegativeContext> {
        let mut store = self.negative.write().unwrap_or_else(|p| p.into_inner());
        let row = store.iter().position(|e|
            !shadow_contracts::negative_asset::is_non_refuting_process(
                e.failure_reason_code, &e.process, &e.raw_bytes)
            && e.process == territory
            && (e.mass_at_fail - epistemic_mass).abs() <= NEG_MASS_TOLERANCE)?;
        store.update_at(row, |e| {
            e.access_count = e.access_count.saturating_add(1);
            e.context()
        })
    }

    pub fn primitive_count(&self) -> usize {
        self.primitive
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .len()
    }

    /// GCL LIVE (vetëm lexim): real_hits e një ankore PA sipas input_id —
    /// 0 nëse primitivi s'ekziston. Ushqen auditimin e lakut Y→X në runtime.
    pub fn primitive_real_hits(&self, input_id: u64) -> u32 {
        self.primitive
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .iter()
            .filter(|e| e.input_id == input_id)
            .map(|e| e.real_hits)
            .next()
            .unwrap_or(0)
    }
    pub fn legacy_count(&self) -> usize {
        self.legacy.read().unwrap_or_else(|p| p.into_inner()).len()
    }
    pub fn negative_count(&self) -> usize {
        self.negative
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .len()
    }
    pub fn hypothesis_count(&self) -> usize {
        self.hypothesis
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .len()
    }

    /// export_negative — kthen klon të të gjitha NegativeEntry-ve (P3: për PRO Vault).
    /// Vetëm lexim — Shadow mban autoritetin; Quantum merr kopje sinkronizimi. Zero if.
    pub fn export_negative(&self) -> Vec<NegativeEntry> {
        self.negative
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .to_vec()
    }

    /// Read-only, bounded projection for the mediated Besa/Asht selection
    /// port. No vault entry, access counter, trust score, or verdict is
    /// mutated. Only digests and bounded statistics cross the process border.
    pub(crate) fn select_bounded_candidates(
        &self,
        request: &shadow_contracts::KnowledgeSelectionRequestWire,
    ) -> Result<
        (
            Vec<shadow_contracts::CandidateStatisticWire>,
            Vec<shadow_contracts::CandidateStatisticWire>,
        ),
        ShadowError,
    > {
        let delivery_scope = shadow_contracts::sha256_hex(&[b"ESSMAI/NOI/SELECTION",request.session_id.as_bytes(),request.trace_id.as_bytes(),request.evidence_sha256.as_bytes()]);
        crate::noi_caller::invoke("SHADOW_CANDIDATE_SELECTION", &delivery_scope, || {
        if !request.shape_is_valid() {
            return Err(ShadowError::SealInvalid("selection request shape rejected".into()));
        }
        let max_positive = request.max_positive;
        let max_negative = request.max_negative;
        // R5: select the exact requested bytes BEFORE the capacity bound.
        // Source request_bound_relevance hashes each whole Xi/Yi term. Comparing
        // the borrowed stored bytes here preserves that exact predicate without
        // promoting subwords of an admitted proposition to separate facts.
        let requested = request.xi.iter().chain(&request.yi)
            .map(|term| term.as_bytes()).collect::<Vec<_>>();
        let primitives = self.primitive.read().unwrap_or_else(|p| p.into_inner());
        let legacies = self.legacy.read().unwrap_or_else(|p| p.into_inner());
        let negatives = self.negative.read().unwrap_or_else(|p| p.into_inner());

        let mut positive = Vec::with_capacity(usize::from(max_positive));
        for entry in primitives.matching(&requested).take(usize::from(max_positive)) {
            let confidence_fixed = match shadow_contracts::fixed_mass(entry.vector_points) {
                Some(value) => value,
                None => {
                    return Err(ShadowError::SealInvalid(
                        "selection primitive vector_points is non-finite".to_string(),
                    ))
                }
            };
            let legacy_score_fixed = match shadow_contracts::fixed_mass(entry.legacy_score()) {
                Some(value) => value,
                None => {
                    return Err(ShadowError::SealInvalid(
                        "selection primitive legacy_score is non-finite".to_string(),
                    ))
                }
            };
            positive.push(shadow_contracts::CandidateStatisticWire {
                candidate_id: entry.input_id,
                polarity: 1,
                domain: "primitive".to_string(),
                term_sha256: shadow_contracts::sha256_hex(&[
                    b"ESSMAI/SELECTION/TERM/PRIMITIVE",
                    entry.raw_bytes.as_slice(),
                ]),
                source_sha256: shadow_contracts::sha256_hex(&[
                    b"ESSMAI/SELECTION/SOURCE/PRIMITIVE",
                    &entry.input_id.to_le_bytes(),
                    entry.raw_bytes.as_slice(),
                ]),
                security_y: 1,
                trust_x: u8::from(entry.legacy_ready),
                confidence_fixed,
                frequency: entry.real_hits,
                domain_count: crate::knowledge_lineage::unique_domains(&entry.primitive_chain)
                    .min(usize::from(u16::MAX)) as u16,
                legacy_score_fixed,
            });
        }
        let remaining = usize::from(max_positive).saturating_sub(positive.len());
        for entry in legacies.matching(&requested).take(remaining) {
            let legacy_score_fixed = match shadow_contracts::fixed_mass(entry.legacy_score) {
                Some(value) => value,
                None => {
                    return Err(ShadowError::SealInvalid(
                        "selection legacy score is non-finite".to_string(),
                    ))
                }
            };
            positive.push(shadow_contracts::CandidateStatisticWire {
                candidate_id: entry.origin_primitive_id,
                polarity: 1,
                domain: "legacy".to_string(),
                term_sha256: shadow_contracts::sha256_hex(&[
                    b"ESSMAI/SELECTION/TERM/LEGACY",
                    entry.raw_bytes.as_slice(),
                ]),
                source_sha256: shadow_contracts::sha256_hex(&[
                    b"ESSMAI/SELECTION/SOURCE/LEGACY",
                    &entry.origin_primitive_id.to_le_bytes(),
                    entry.raw_bytes.as_slice(),
                ]),
                security_y: 1,
                trust_x: 1,
                confidence_fixed: legacy_score_fixed,
                frequency: 1,
                domain_count: 1,
                legacy_score_fixed,
            });
        }

        let mut negative = Vec::with_capacity(usize::from(max_negative));
        for entry in negatives.matching(&requested).filter(|entry|
            !shadow_contracts::negative_asset::is_non_refuting_process(
                entry.failure_reason_code, &entry.process, &entry.raw_bytes)).take(usize::from(max_negative)) {
            let mass_fixed = match shadow_contracts::fixed_mass(entry.mass_at_fail) {
                Some(value) => value,
                None => {
                    return Err(ShadowError::SealInvalid(
                        "selection negative mass is non-finite".to_string(),
                    ))
                }
            };
            negative.push(shadow_contracts::CandidateStatisticWire {
                candidate_id: entry.input_id,
                polarity: 0,
                domain: entry.process.clone(),
                term_sha256: shadow_contracts::sha256_hex(&[
                    b"ESSMAI/SELECTION/TERM/NEGATIVE",
                    entry.raw_bytes.as_slice(),
                ]),
                source_sha256: shadow_contracts::sha256_hex(&[
                    b"ESSMAI/SELECTION/SOURCE/NEGATIVE",
                    &entry.input_id.to_le_bytes(),
                    &entry.failure_reason_code.to_le_bytes(),
                    entry.raw_bytes.as_slice(),
                ]),
                security_y: 0,
                trust_x: 0,
                confidence_fixed: shadow_contracts::MASS_SCALE.saturating_sub(mass_fixed),
                frequency: entry.frequency,
                domain_count: 1,
                legacy_score_fixed: 0,
            });
        }
        Ok((positive, negative))
    
        })
    }
    pub fn fact_count(&self) -> usize {
        self.fact.read().unwrap_or_else(|p| p.into_inner()).len()
    }

    // ── WRITE API — VETËM shadow_gj_legacy.lgc (pub(crate)) ─────────────

    /// Shkruan në PrimitiveStore — 500 seal i konfirmuar.
    /// Çdo shkrim regjistron një GJURMË (domain/kohë/masë/path/vulë). Përsëritja
    /// e të njëjtës përmbajtje në domain të ndryshëm e afron drejt Legacy.
    /// FAZA 2: WAL-first — regjistron ngjarjen (borrow), pastaj apliko (move).
    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
    pub(crate) fn write_primitive(
        &self,
        input_id: u64,
        vector_points: f32,
        temporal_score: f32,
        mass: f32,
        seal: u32,
        path: ShadowPath,
        domain: String,
        raw_bytes: Vec<u8>,
        ts_ns: u64,
    ) -> Result<(), ShadowError> {
        self.backend
            .on_primitive(
                input_id,
                vector_points,
                temporal_score,
                mass,
                seal,
                path.as_u8(),
                &domain,
                &raw_bytes,
                ts_ns,
            )
            .map_err(|error| ShadowError::VaultWriteError(error.to_string()))?;
        let trace = PrimitiveTrace {
            domain,
            timestamp: ts_ns,
            mass,
            path_taken: path,
            lgc_seal: seal,
        };
        self.apply_primitive(input_id, raw_bytes, vector_points, temporal_score, trace)
    }

    /// Aplikim në RAM (pa rilogim) — i përdorur edhe nga ripërsëritja.
    /// Dedupe ndër-domain me `content_hash`: i njëjti input → shton gjurmë;
    /// i ri → entry e re. `legacy_ready` rillogaritet sa herë (ZERO if/else mbi vulën).
    fn apply_primitive(
        &self, input_id: u64, raw_bytes: Vec<u8>, vector_points: f32,
        temporal_score: f32, trace: PrimitiveTrace,
    ) -> Result<(), ShadowError> {
        self.apply_primitive_policy(input_id, raw_bytes, vector_points, temporal_score, trace, false, |_| {})
            .map(|_| ())
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_primitive_policy(
        &self,
        input_id: u64,
        raw_bytes: Vec<u8>,
        vector_points: f32,
        temporal_score: f32,
        trace: PrimitiveTrace,
        exact: bool,
        publish: impl FnOnce(&PrimitiveEntry),
    ) -> Result<usize, ShadowError> {
        let content_hash = fnv64(&raw_bytes);
        let content_sha = candidate_content_sha256(&raw_bytes);
        let ts = trace.timestamp;

        let mut store = self.primitive.write().unwrap_or_else(|poisoned| {
            eprintln!("[VAULT] primitive RAM lock poisoned pas TX_COMMIT — recovery");
            poisoned.into_inner()
        });

        // Old WAL tags retain their original projection; TX5 requires exact identity.
        let existing = store.iter().position(|e| match exact {
            true => e.raw_bytes == raw_bytes && candidate_content_sha256(&e.raw_bytes) == content_sha,
            false => e.content_hash == content_hash,
        });
        if let Some(row) = existing {
            store.update_at(row, |e| {
                e.primitive_chain.push(trace);
                e.legacy_ready = cross_domain_legacy_ready(&e.primitive_chain);
                e.access_count = e.access_count.saturating_add(1);
                publish(e); // Lock still held: maturation cannot substitute the row.
            }).expect("located primitive row remains present");
            return Ok(row); // Preserve the existing trace-accumulation and maturity law.
        }

        // Shfaqje e parë → entry e re me gjurmën e parë.
        let chain = vec![trace];
        let legacy_ready = cross_domain_legacy_ready(&chain); // 1 domain < 5 → false
        store.push_unindexed(PrimitiveEntry {
            input_id,
            content_hash,
            raw_bytes,
            vector_points,
            real_hits: 0,
            temporal_score,
            access_count: 0,
            added_at: ts,
            primitive_chain: chain,
            legacy_ready,
        });
        publish(store.last().expect("just appended primitive"));
        Ok(store.len() - 1)
    }

    /// Shkruan sipas knowledge_band — verified por jo primitive.
    /// FAZA 2: regjistron NJË ngjarje `on_verified`; ripërsëritja riprodhon
    /// të njëjtin rrugëzim (përfshirë rastin Negative).
    #[cfg(test)]
    pub(crate) fn write_verified(
        &self,
        input_id: u64,
        raw_bytes: Vec<u8>,
        band: KnowledgeBand,
        mass: f32,
        ts_ns: u64,
    ) -> Result<(), ShadowError> {
        self.backend
            .on_verified(input_id, band.as_u8(), mass, &raw_bytes, ts_ns)
            .map_err(|error| ShadowError::VaultWriteError(error.to_string()))?;
        self.apply_verified(input_id, raw_bytes, band, mass, ts_ns)
    }

    fn apply_verified(
        &self,
        input_id: u64,
        raw_bytes: Vec<u8>,
        band: KnowledgeBand,
        mass: f32,
        ts_ns: u64,
    ) -> Result<(), ShadowError> {
        match band {
            KnowledgeBand::Hypothesis => {
                let mut s = self
                    .hypothesis
                    .write()
                    .map_err(|e| ShadowError::VaultWriteError(e.to_string()))?;
                s.push(HypothesisEntry {
                    input_id,
                    mass,
                    raw_bytes,
                    added_at: ts_ns,
                });
            }
            KnowledgeBand::Fact | KnowledgeBand::Scientific => {
                let mut s = self
                    .fact
                    .write()
                    .map_err(|e| ShadowError::VaultWriteError(e.to_string()))?;
                s.push(FactEntry {
                    input_id,
                    mass,
                    raw_bytes,
                    added_at: ts_ns,
                });
            }
            KnowledgeBand::Negative => {
                // verified me bandë negative → trajto si negative pa kontekst të pasur.
                self.apply_negative(input_id, 0, String::new(), mass, 1, raw_bytes, ts_ns)?;
            }
        }
        Ok(())
    }

    /// Shkruan në NegativeStore me KONTEKST të plotë (LAW_5).
    /// FAZA 2: regjistron INPUT-in; dedup-i (frekuenca) riprodhohet besnikërisht
    /// nga ripërsëritja e të njëjtave ngjarje në renditje.
    #[cfg(any(test, feature = "dev_harness"))]
    pub(crate) fn write_negative(
        &self,
        input_id: u64,
        raw_bytes: Vec<u8>,
        ctx: NegativeContext,
        ts_ns: u64,
    ) -> Result<(), ShadowError> {
        self.backend
            .on_negative(
                input_id,
                ctx.failure_reason_code,
                &ctx.process,
                ctx.mass_at_fail,
                ctx.frequency,
                &raw_bytes,
                ts_ns,
            )
            .map_err(|error| ShadowError::VaultWriteError(error.to_string()))?;
        self.apply_negative(
            input_id,
            ctx.failure_reason_code,
            ctx.process,
            ctx.mass_at_fail,
            ctx.frequency,
            raw_bytes,
            ts_ns,
        )
    }

    /// Aplikim në RAM me dedupe (pa rilogim) — i përdorur edhe nga ripërsëritja.
    /// Nëse modeli ekziston (arsye + proces + masë e ngjashme) → frekuencë++.
    #[allow(clippy::too_many_arguments)]
    fn apply_negative(
        &self, input_id: u64, reason: u32, process: String, mass: f32,
        frequency: u32, raw_bytes: Vec<u8>, ts_ns: u64,
    ) -> Result<(), ShadowError> {
        self.apply_negative_policy(input_id, reason, process, mass, frequency, raw_bytes, ts_ns, false, |_| {})
            .map(|_| ())
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_negative_policy(
        &self,
        input_id: u64,
        reason: u32,
        process: String,
        mass: f32,
        frequency: u32,
        raw_bytes: Vec<u8>,
        ts_ns: u64,
        exact: bool,
        publish: impl FnOnce(&NegativeEntry),
    ) -> Result<usize, ShadowError> {
        let content_sha = candidate_content_sha256(&raw_bytes);
        let mut store = self.negative.write().unwrap_or_else(|poisoned| {
            eprintln!("[VAULT] negative RAM lock poisoned pas TX_COMMIT — recovery");
            poisoned.into_inner()
        });

        let existing = store.iter().position(|e| {
            let same_reason = e.failure_reason_code == reason;
            let same_proc = e.process == process;
            let close_mass = (e.mass_at_fail - mass).abs() <= NEG_MASS_TOLERANCE;
            let same_content = !exact || (e.raw_bytes == raw_bytes
                && candidate_content_sha256(&e.raw_bytes) == content_sha);
            same_reason && same_proc && close_mass && same_content
        });
        if let Some(row) = existing {
            store.update_at(row, |e| {
                e.frequency = e.frequency.saturating_add(1);
                e.access_count = e.access_count.saturating_add(1);
                e.added_at = ts_ns;
                publish(e);
            }).expect("located negative row remains present");
            return Ok(row);
        }

        store.push_unindexed(NegativeEntry {
            input_id,
            failure_reason_code: reason,
            process,
            mass_at_fail: mass,
            frequency: frequency.max(1),
            raw_bytes,
            access_count: 0,
            added_at: ts_ns,
        });
        publish(store.last().expect("just appended negative"));
        Ok(store.len() - 1)
    }

    // ── MATURIMI — Primitive → Legacy (mbijetesa e zgjatur) ─────────────

    /// Konfirmim nga bota reale: rrit real_hits dhe access_count, pastaj
    /// provon promovimin në Legacy. Kjo zbaton "mbijetesën e zgjatur".
    /// Konfirmim nga bota reale: rrit real_hits dhe access_count, pastaj
    /// provon promovimin në Legacy. Kjo zbaton "mbijetesën e zgjatur".
    /// FAZA 2: regjistron `on_confirm`; ripërsëritja riprodhon EKZAKT
    /// rritjen e real_hits dhe promovimin (kaskadë RAM, pa rilogim).
    #[cfg(test)]
    pub fn confirm_primitive(&self, input_id: u64, hits: u32) -> u32 {
        match self.backend.on_confirm(input_id, hits) {
            Ok(()) => self.apply_confirm(input_id, hits),
            Err(error) => {
                eprintln!("[VAULT] confirm legacy refuzuar: {error}");
                0
            }
        }
    }

    fn apply_confirm(&self, input_id: u64, hits: u32) -> u32 {
        self.apply_confirm_policy(input_id, hits, false)
    }

    fn apply_confirm_policy(&self, input_id: u64, hits: u32, exact: bool) -> u32 {
        {
            let mut store = self.primitive.write().unwrap_or_else(|p| p.into_inner());
            for row in 0..store.len() {
                if store[row].input_id == input_id {
                    store.update_at(row, |e| {
                        e.real_hits = e.real_hits.saturating_add(hits);
                        e.access_count = e.access_count.saturating_add(1);
                    }).expect("located reinforcement row remains present");
                }
            }
        }
        self.try_promote_to_legacy_policy(exact)
    }

    /// Rrit access_count për një primitiv kur konsultohet si bazë vendimi.
    pub fn note_primitive_access(&self, input_id: u64) {
        let mut store = self.primitive.write().unwrap_or_else(|p| p.into_inner());
        for row in 0..store.len() {
            if store[row].input_id == input_id {
                store.update_at(row, |e| e.access_count = e.access_count.saturating_add(1))
                    .expect("located access row remains present");
            }
        }
    }

    /// Promovon Primitive → Legacy kur përmbushet rregulla NDËR-DOMAIN
    /// (≥5 domain-e unike me vulë konsistente). Hook publik për maturim manual.
    pub fn run_maturation(&self) -> Result<u32, ShadowError> {
        self.run_maturation_action(None)
    }

    #[cfg(test)]
    pub(crate) fn try_promote_to_legacy(&self) -> u32 {
        self.try_promote_to_legacy_policy(true)
    }

    fn try_promote_to_legacy_policy(&self, exact: bool) -> u32 {
        let mut promoted = 0u32;

        // Identifiko kandidatët e gatshëm (lexim).
        let ready_ids: Vec<u64> = {
            let store = self.primitive.read().unwrap_or_else(|p| p.into_inner());
            store
                .iter()
                .filter(|e| e.ready_for_legacy())
                .map(|e| e.input_id)
                .collect()
        };

        for id in ready_ids {
            // Hiq nga primitive (kristalizohet — nuk qëndron i dyfishtë).
            let entry = {
                let mut store = self.primitive.write().unwrap_or_else(|p| p.into_inner());
                // A package id can name several objects; only a currently ready
                // row may mature under TX5/current operation. Historical replay
                // retains its original selection to preserve old receipts.
                match store.iter().position(|e| e.input_id == id && (!exact || e.ready_for_legacy())) {
                    Some(pos) => Some(store.remove(pos)),
                    None => None,
                }
            };

            match entry {
                Some(c) => {
                    let immutable = (c.input_id as u32).wrapping_mul(0xA4B1);
                    // FAZA 7 — kap prejardhjen PARA zhvendosjes së raw_bytes.
                    let birth_id = c.input_id;
                    let birth_at = c.added_at;
                    let birth_score = c.legacy_score();
                    let birth_reason = c.legacy_reason();
                    let mut legacy = self.legacy.write().unwrap_or_else(|p| p.into_inner());
                    legacy.push(LegacyEntry {
                        origin_primitive_id: c.input_id,
                        raw_bytes: c.raw_bytes,
                        crystallized_at: c.added_at,
                        legacy_score: birth_score,
                        immutable_seal: immutable,
                    });
                    promoted = promoted.wrapping_add(1);
                    // Regjistro lindjen me arsyen e plotë (vëzhguesi i përhershëm).
                    self.observer
                        .observe_birth(birth_id, birth_at, birth_score, birth_reason);
                }
                None => {}
            }
        }
        promoted
    }
}

// ════════════════════════════════════════════════════════════════════════════
// §3  CALLBACKS nga C kernel → Rust (vetëm ABI; kerneli dublikatë jashtë build)
//     shadow_gj_legacy_kernel.c NUK është në build.rs; këto simbole mbeten
//     për pajtueshmëri ABI. Shkrimi real bëhet nga shadow_gj_legacy.rs.
// ════════════════════════════════════════════════════════════════════════════

use std::sync::OnceLock;

static GLOBAL_VAULT: OnceLock<Arc<KnowledgeVault>> = OnceLock::new();

pub fn init_global_vault(vault: Arc<KnowledgeVault>) {
    // FAIL-LOUD: set() dështon vetëm nëse vault-i global është VENDOSUR TASHMË
    // (dyfishim inicializimi = anomali). Raportohet, s'fshihet me `let _`.
    match GLOBAL_VAULT.set(vault) {
        Ok(())  => {}
        Err(_)  => eprintln!("[GLOBAL_VAULT] ALARM: init i dyfishtë — vault-i global u vendos tashmë (injorohet i dyti)"),
    }
}

pub fn get_global_vault() -> Option<Arc<KnowledgeVault>> {
    GLOBAL_VAULT.get().cloned()
}

/// Callback ABI legacy nga kerneli C. Simboli ruhet vetëm me feature të
/// shprehur, por write-i refuzohet pa committed verdict. View-i është zero-copy.
///
/// # Safety
/// `pkg_bytes` duhet të mbetet i lexueshëm për `pkg_len` gjatë thirrjes.
#[cfg(any(test, feature = "legacy_ffi_compat"))]
#[no_mangle]
pub unsafe extern "C" fn vault_write_primitive(pkg_bytes: *const u8, pkg_len: u32, _ts_ns: u64) {
    match unsafe { legacy_ffi_view(pkg_bytes, pkg_len) } {
        Some(view) => eprintln!(
            "[VAULT_FFI] CompatibilityHold: primitive direct write ({} byte)",
            view.len(),
        ),
        None => eprintln!("[VAULT_FFI] CompatibilityHold: primitive payload bosh"),
    }
}

/// Callback ABI legacy për dije të verifikuar; zero-copy dhe zero authority.
///
/// # Safety
/// `pkg_bytes` duhet të mbetet i lexueshëm për `pkg_len` gjatë thirrjes.
#[cfg(any(test, feature = "legacy_ffi_compat"))]
#[no_mangle]
pub unsafe extern "C" fn vault_write_verified(
    pkg_bytes: *const u8,
    pkg_len: u32,
    _knowledge_band: u8,
    _ts_ns: u64,
) {
    match unsafe { legacy_ffi_view(pkg_bytes, pkg_len) } {
        Some(view) => eprintln!(
            "[VAULT_FFI] CompatibilityHold: verified direct write ({} byte)",
            view.len(),
        ),
        None => eprintln!("[VAULT_FFI] CompatibilityHold: verified payload bosh"),
    }
}

/// Callback ABI legacy për dije negative; technical/FFI failure nuk bëhet dije.
///
/// # Safety
/// `pkg_bytes` duhet të mbetet i lexueshëm për `pkg_len` gjatë thirrjes.
#[cfg(any(test, feature = "legacy_ffi_compat"))]
#[no_mangle]
pub unsafe extern "C" fn vault_write_negative(pkg_bytes: *const u8, pkg_len: u32, _ts_ns: u64) {
    match unsafe { legacy_ffi_view(pkg_bytes, pkg_len) } {
        Some(view) => eprintln!(
            "[VAULT_FFI] CompatibilityHold: negative direct write ({} byte)",
            view.len(),
        ),
        None => eprintln!("[VAULT_FFI] CompatibilityHold: negative payload bosh"),
    }
}

/// Borrowed view mbi buffer-in e caller-it. Asnjë alokim, clone ose transfer
/// pronësie; caller-i mbetet pronari dhe e liron memorien pas kthimit.
#[cfg(any(test, feature = "legacy_ffi_compat"))]
unsafe fn legacy_ffi_view<'a>(pkg_bytes: *const u8, pkg_len: u32) -> Option<&'a [u8]> {
    match pkg_bytes.is_null() || pkg_len == 0 {
        true => None,
        false => {
            // SAFETY: kontrata e ABI garanton pointer të lexueshëm dhe lifetime.
            Some(unsafe { std::slice::from_raw_parts(pkg_bytes, pkg_len as usize) })
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// §4  TESTE NË-KREJT (akses te write API pub(crate))
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: shkruan një primitiv në një domain të dhënë (nënshkrimi i ri).
    fn wp(v: &KnowledgeVault, id: u64, domain: &str, bytes: Vec<u8>, ts: u64) {
        v.write_primitive(
            id,
            0.9,
            0.9,
            0.9,
            0xA451,
            ShadowPath::Fast,
            domain.to_string(),
            bytes,
            ts,
        )
        .expect("wp: rezultat i papritur në test");
    }

    /// 5 domain-e standarde për të kristalizuar një primitiv → Legacy.
    const FIVE_DOMAINS: [&str; 5] = ["science", "industrial", "medical", "security", "research"];

    #[test]
    fn native_iz_lookup_uses_exact_operation_reference_without_latest_head_fallback(){
        use shadow_contracts::pd_memory::{self as pd,iz};
        let vault=KnowledgeVault::new();
        let origin="Analizo motorin industrial".to_string();
        let context=iz::IzContext{version:iz::IZ_ACTUAL_INPUT_VERSION,
            identity:pd::PdIdentity{owner:"lookup-data-only".into(),obligation:"distinction".into(),
                case_id:"case".into(),scope:"WORK→NURA".into(),domain:"WORK→NURA".into(),
                lineage_root:pd::hex(&{use sha2::Digest;sha2::Sha256::digest(origin.as_bytes()).into()})},
            operation_id:"exact-operation".into(),origin_i0:origin.clone(),user_text:origin,
            parent:None,answer:iz::IzAnswer::default(),map:None,native_anchor:None,native_map:None};
        context.validate().unwrap();
        let key=context.identity.key().unwrap();
        let scope=crate::native_cycle::cognitive_scope_key(&pd::hex(&key),&context.identity.scope);
        vault.native_pd_heads.write().unwrap().insert(scope,crate::native_cycle::NativePdHead{
            identity:[1;32],request_sha256:[2;32],transaction_id:[3;32],root_identity:[1;32]});
        assert!(vault.pd_iz_native_lookup(&context).unwrap().is_none());
        vault.native_iz_operations.write().unwrap().insert((key,"other-operation".into()),[1;32]);
        assert!(vault.pd_iz_native_lookup(&context).unwrap().is_none());
        // An exact dangling reference is corruption, not permission to pick head.
        vault.native_iz_operations.write().unwrap().insert((key,context.operation_id.clone()),[4;32]);
        assert!(format!("{:?}",vault.pd_iz_native_lookup(&context).unwrap_err())
            .contains("PD_IZ_NATIVE_T1_REFERENCE_MISSING"));
        assert!(vault.continuation_refs.read().unwrap().is_empty());
    }

    #[test]
    fn maturation_primitive_to_legacy() {
        let v = KnowledgeVault::new();
        // I njëjti input, i përforcuar në 5 DOMAIN të ndryshme → kristalizim.
        let content = b"reinforced-fact".to_vec();
        for (i, dom) in FIVE_DOMAINS.iter().enumerate() {
            wp(&v, 100 + i as u64, dom, content.clone(), 1000 + i as u64);
        }
        // Dedupe me content_hash → NJË entry me zinxhir 5-gjurmësh (input_id=100).
        assert_eq!(v.primitive_count(), 1);
        assert_eq!(v.legacy_count(), 0);

        // LIGJI I PËRFORCUAR (FAZA 3): 5 domaine janë të nevojshme por JO të
        // mjaftueshme — legacy lind me KOHË. real_hits=0 → ende s'maturohet.
        let promoted_before = v.try_promote_to_legacy();
        assert_eq!(promoted_before, 0);
        assert_eq!(v.legacy_count(), 0);

        // Konfirmim nga bota reale (≥3 hits) → maturim → promovohet tani.
        // (input_id=100 mbahet nga shfaqja e parë pas dedupe.)
        let promoted = v.confirm_primitive(100, 3);
        assert_eq!(promoted, 1);
        assert_eq!(v.primitive_count(), 0);
        assert_eq!(v.legacy_count(), 1);

        // FAZA 7 — vëzhguesi regjistroi lindjen me arsyen e plotë.
        assert_eq!(v.observer.birth_count(), 1);
        assert!(v.observer.was_observed(100));
        let reason = v.observer.reason_for(100);
        assert!(reason.is_some());
        let reason = reason.expect("maturation_primitive_to_legacy: rezultat i papritur në test");
        assert!(reason.cross_domain_ok);
        assert!(reason.maturation_ok);
        assert_eq!(reason.domain_count, 5);
    }

    #[test]
    fn four_domains_do_not_promote() {
        let v = KnowledgeVault::new();
        // Vetëm 4 domain-e unike (< 5) → NUK kristalizohet.
        let content = b"not-yet".to_vec();
        for (i, dom) in FIVE_DOMAINS.iter().take(4).enumerate() {
            wp(&v, 200 + i as u64, dom, content.clone(), 1 + i as u64);
        }
        assert_eq!(v.primitive_count(), 1);
        assert_eq!(v.try_promote_to_legacy(), 0);
        assert_eq!(v.primitive_count(), 1);
        assert_eq!(v.legacy_count(), 0);
    }

    #[test]
    fn same_domain_five_times_does_not_promote() {
        let v = KnowledgeVault::new();
        // 5 përforcime por NË TË NJËJTIN domain → 1 domain unik → s'promovon.
        let content = b"echo".to_vec();
        for i in 0..5u64 {
            wp(&v, 300 + i, "science", content.clone(), 1 + i);
        }
        assert_eq!(v.primitive_count(), 1);
        assert_eq!(v.try_promote_to_legacy(), 0); // unique_domains == 1
        assert_eq!(v.legacy_count(), 0);
    }

    #[test]
    fn fresh_primitive_does_not_promote() {
        let v = KnowledgeVault::new();
        // Një domain i vetëm → unique_domains = 1 < 5 → s'promovon.
        wp(&v, 7, "science", vec![9], 1);
        let promoted = v.run_maturation().unwrap();
        assert_eq!(promoted, 0);
        assert_eq!(v.primitive_count(), 1);
        assert_eq!(v.legacy_count(), 0);
    }

    fn r42_selection_request() -> shadow_contracts::KnowledgeSelectionRequestWire {
        use shadow_contracts::{KnowledgeSelectionRequestWire, SelectionPurpose, SelectionRequester};
        KnowledgeSelectionRequestWire {
            version: shadow_contracts::PROTOCOL_VERSION,
            requester: SelectionRequester::AshtQuantum,
            purpose: SelectionPurpose::EvidenceCandidates,
            session_id: "r42-index-retention".into(), parent_i0: "11".repeat(32),
            primitive_anchor: 41, trace_id: "r42-index-retention".into(),
            split_sha256: "22".repeat(32), xi: vec!["alpha".into()], yi: vec!["beta".into()],
            upstream_hold_sha256: Some("33".repeat(32)), evidence_sha256: "44".repeat(32),
            max_positive: shadow_contracts::SELECTION_MAX_CANDIDATES,
            max_negative: shadow_contracts::SELECTION_MAX_CANDIDATES,
        }
    }

    #[test]
    fn r42_duplicate_primitive_trace_keeps_warm_index() {
        let v = KnowledgeVault::new();
        let tx = |id| crate::vault_transaction::test_support::sample_transaction_with(
            [id; 32], [id + 1; 32], b"alpha");
        v.commit_test_transaction(tx(81)).unwrap();
        let before = v.select_bounded_candidates(&r42_selection_request()).unwrap();
        assert_eq!(before.0.len(), 1);
        assert!(v.primitive.read().unwrap().r42_index_is_warm());
        v.commit_test_transaction(tx(83)).unwrap();
        {
            let rows = v.primitive.read().unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].primitive_chain.len(), 2);
            assert_eq!(rows[0].raw_bytes, b"alpha");
            assert!(rows.r42_index_is_warm(), "trace-only TX5 update discarded the content index");
        }
        assert_eq!(v.select_bounded_candidates(&r42_selection_request()).unwrap(), before);
    }

    #[test]
    fn r42_duplicate_negative_frequency_keeps_warm_index() {
        let v = KnowledgeVault::new();
        v.commit_test_transaction(r35_negative_transaction(85, b"beta")).unwrap();
        let mut expected = v.select_bounded_candidates(&r42_selection_request()).unwrap();
        assert_eq!(expected.1.len(), 1);
        assert!(v.negative.read().unwrap().r42_index_is_warm());
        v.commit_test_transaction(r35_negative_transaction(87, b"beta")).unwrap();
        {
            let rows = v.negative.read().unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!((rows[0].frequency, rows[0].access_count), (2, 1));
            assert_eq!(rows[0].raw_bytes, b"beta");
            assert!(rows.r42_index_is_warm(), "frequency-only TX5 update discarded the content index");
        }
        expected.1[0].frequency = 2;
        assert_eq!(v.select_bounded_candidates(&r42_selection_request()).unwrap(), expected);
    }

    #[test]
    fn r42_reinforcement_without_promotion_keeps_warm_index() {
        let v = KnowledgeVault::new();
        v.commit_test_transaction(crate::vault_transaction::test_support::sample_transaction_with(
            [89; 32], [90; 32], b"alpha")).unwrap();
        let before = v.select_bounded_candidates(&r42_selection_request()).unwrap();
        assert!(v.primitive.read().unwrap().r42_index_is_warm());
        assert_eq!(v.apply_confirm_policy(41, 1, true), 0);
        {
            let rows = v.primitive.read().unwrap();
            assert_eq!((rows[0].real_hits, rows[0].access_count), (1, 1));
            assert_eq!(rows[0].raw_bytes, b"alpha");
            assert!(rows.r42_index_is_warm(), "counter-only reinforcement discarded the content index");
        }
        let after = v.select_bounded_candidates(&r42_selection_request()).unwrap();
        assert_eq!(after.0.len(), 1);
        assert_eq!(after.0[0].candidate_id, before.0[0].candidate_id);
        assert_eq!(after.0[0].source_sha256, before.0[0].source_sha256);
        assert_eq!(after.0[0].frequency, 1);
        assert!(after.1.is_empty());
    }

    #[test]
    fn r42_known_negative_read_keeps_warm_index_on_match_and_miss() {
        let v = KnowledgeVault::new();
        v.commit_test_transaction(r35_negative_transaction(91, b"beta")).unwrap();
        let before = v.select_bounded_candidates(&r42_selection_request()).unwrap();
        assert!(v.negative.read().unwrap().r42_index_is_warm());
        assert!(v.check_known_negative(0.5, "same-scientific-method").is_some());
        {
            let rows = v.negative.read().unwrap();
            assert_eq!(rows[0].access_count, 1);
            assert!(rows.r42_index_is_warm(), "access-only lookup discarded the content index");
        }
        assert!(v.check_known_negative(0.5, "unrelated-method").is_none());
        assert!(v.negative.read().unwrap().r42_index_is_warm(), "missing lookup discarded the content index");
        assert_eq!(v.select_bounded_candidates(&r42_selection_request()).unwrap(), before);
    }
    #[test]
    fn metadata_updates_keep_all_shared_anchor_rows_and_missing_anchor_unchanged() {
        let v = KnowledgeVault::new();
        for (id, raw) in [(101, b"alpha".as_slice()), (103, b"beta".as_slice())] {
            v.commit_test_transaction(crate::vault_transaction::test_support::sample_transaction_with(
                [id; 32], [id + 1; 32], raw)).unwrap();
        }
        wp(&v, 42, "other", b"gamma".to_vec(), 1);
        assert_eq!(v.select_bounded_candidates(&r42_selection_request()).unwrap().0.len(), 2);
        assert!(v.primitive.read().unwrap().r42_index_is_warm());
        assert_eq!(v.apply_confirm_policy(41, 1, true), 0);
        v.note_primitive_access(41);
        assert_eq!(v.apply_confirm_policy(999, 1, true), 0);
        v.note_primitive_access(999);
        let rows = v.primitive.read().unwrap();
        assert!(rows.r42_index_is_warm());
        assert_eq!(rows.iter().map(|row| (row.input_id, row.real_hits, row.access_count)).collect::<Vec<_>>(),
            [(41, 1, 2), (41, 1, 2), (42, 0, 0)]);
        assert_eq!(rows.iter().map(|row| row.raw_bytes.as_slice()).collect::<Vec<_>>(),
            [b"alpha".as_slice(), b"beta".as_slice(), b"gamma".as_slice()]);
    }

    // Storage fixtures, not native admission or scientific facts.
    fn r35_negative_transaction(id: u8, bytes: &[u8]) -> VaultTransaction {
        let base = crate::vault_transaction::test_support::sample_transaction_with([id;32], [id+1;32], bytes);
        let verdict = crate::types::LawConfirmedVerdict::confirm(
            crate::vault_transaction::test_support::sample_supreme(0,0), true).unwrap();
        let mut receipt = base.receipt;
        receipt.y_verdict=0; receipt.x_verdict=0;
        VaultTransaction::build(base.identity_digest,base.cycle_digest,base.anchor_generation,
            base.negative_batch_digest,base.runtime_delta_digest,base.vds_close_digest,base.law0_digest,
            &verdict,vec![KnowledgeWrite::Negative{input_id:41,raw_bytes:bytes.to_vec(),
                context:NegativeContext{failure_reason_code:0x101,process:"same-scientific-method".into(),
                    mass_at_fail:0.50,frequency:1},ts_ns:u64::from(id)}],base.lineage,None,receipt)
    }

    #[test]
    fn r35_negative_distinct_content_is_not_one_candidate() {
        let v = KnowledgeVault::new();
        v.commit_test_transaction(r35_negative_transaction(11,b"proposition-alpha")).unwrap();
        v.commit_test_transaction(r35_negative_transaction(13,b"proposition-beta")).unwrap();
        let rows = v.export_negative();
        assert_eq!(rows.len(), 2, "different bytes must retain their distinct candidate origins");
        assert_eq!(rows[0].raw_bytes, b"proposition-alpha");
        assert_eq!(rows[1].raw_bytes, b"proposition-beta");
        assert_eq!((rows[0].frequency, rows[1].frequency), (1, 1));
    }

    #[test]
    fn r35_primitive_index_collision_cannot_equate_different_bytes() {
        let v = KnowledgeVault::new();
        let first=crate::vault_transaction::test_support::sample_transaction_with([11;32],[12;32],b"alpha");
        let second=crate::vault_transaction::test_support::sample_transaction_with([13;32],[14;32],b"beta");
        v.commit_test_transaction(first).unwrap();
        // Harmless in-memory auxiliary index fault; no cryptographic collision is claimed.
        v.primitive.write().unwrap()[0].content_hash = fnv64(b"beta");
        v.commit_test_transaction(second).unwrap();
        let rows = v.primitive.read().unwrap();
        assert_eq!(rows.len(), 2, "an auxiliary index is not exact content identity");
        assert_eq!(rows[0].raw_bytes, b"alpha");
        assert_eq!(rows[1].raw_bytes, b"beta");
        assert_eq!(rows[0].primitive_chain.len(), 1);
        // input_id names the originating package, not unique candidate content.
        assert_eq!(rows[0].input_id, rows[1].input_id);
    }

    #[test]
    fn r35_mixed_wal_preserves_legacy_projection_and_new_exact_identity() {
        let path = temp_wal("r35_mixed_identity");
        let new = r35_negative_transaction(25, b"beta");
        {
            let v = KnowledgeVault::open_disk(&path).unwrap();
            v.commit_test_transaction(r35_negative_transaction(21,b"alpha").legacy_projection_fixture()).unwrap();
            v.commit_test_transaction(r35_negative_transaction(23,b"beta").legacy_projection_fixture()).unwrap();
            // Historical coarse projection is preserved, not silently relabelled.
            assert_eq!(v.negative_count(), 1);
            assert_eq!(v.export_negative()[0].frequency, 2);
            v.commit_test_transaction(new.clone()).unwrap();
            assert_eq!(v.negative_count(), 2);
            let size = std::fs::metadata(&path).unwrap().len();
            v.commit_test_transaction(new.clone()).unwrap();
            assert_eq!(std::fs::metadata(&path).unwrap().len(), size);
        }
        let before = std::fs::read(&path).unwrap();
        {
            let v = KnowledgeVault::open_disk(&path).unwrap();
            let rows = v.export_negative();
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0].raw_bytes, b"alpha");
            assert_eq!(rows[1].raw_bytes, b"beta");
            assert_eq!((rows[0].frequency, rows[1].frequency), (2, 1));
            v.commit_test_transaction(new).unwrap();
        } // Release the exclusive Windows WAL lock before byte comparison.
        assert_eq!(std::fs::read(&path).unwrap(), before);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn r35_exact_content_retains_legitimate_trace_accumulation() {
        let v = KnowledgeVault::new();
        let trace = |domain: &str, timestamp| PrimitiveTrace {
            domain: domain.into(), timestamp, mass: 1.0,
            path_taken: ShadowPath::Fast, lgc_seal: 0xA451,
        };
        for (i, domain) in FIVE_DOMAINS.iter().enumerate() {
            assert_eq!(v.apply_primitive_policy(900+i as u64, b"shared-content".to_vec(),
                1.0, 1.0, trace(domain,i as u64), true, |_| {}).unwrap(), 0);
        }
        let rows = v.primitive.read().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].primitive_chain.len(), 5);
        assert!(rows[0].legacy_ready);
        drop(rows);
        assert_eq!(v.try_promote_to_legacy(), 0); // Real confirmation remains necessary.
        assert_eq!(v.confirm_primitive(900, 3), 1);
        assert_ne!(candidate_content_sha256(b"a,b"), candidate_content_sha256(b"ab"));
    }

    #[test]
    fn r35_maturation_does_not_promote_an_unready_sibling_with_same_input_id() {
        let v = KnowledgeVault::new();
        let trace = |domain: &str| PrimitiveTrace { domain: domain.into(), timestamp: 1,
            mass: 1.0, path_taken: ShadowPath::Fast, lgc_seal: 0xA451 };
        v.apply_primitive_policy(41,b"unready".to_vec(),1.0,1.0,trace("one"),true, |_| {}).unwrap();
        for domain in FIVE_DOMAINS {
            v.apply_primitive_policy(41,b"ready".to_vec(),1.0,1.0,trace(domain),true, |_| {}).unwrap();
        }
        {
            let mut entries=v.primitive.write().unwrap();
            for entry in entries.iter_mut() {entry.real_hits=3;}
            assert!(!entries[0].ready_for_legacy());
            assert!(entries[1].ready_for_legacy());
        }
        assert_eq!(v.run_maturation().unwrap(),1);
        assert_eq!(v.legacy.read().unwrap()[0].raw_bytes,b"ready");
        assert_eq!(v.primitive.read().unwrap()[0].raw_bytes,b"unready");
    }

    #[test]
    fn r35_provenance_publication_holds_row_lock_for_new_and_reused_content() {
        let v=KnowledgeVault::new();
        for timestamp in [1,2] {
            let trace=PrimitiveTrace {domain:"science".into(),timestamp,mass:1.0,
                path_taken:ShadowPath::Fast,lgc_seal:0xA451};
            let row=v.apply_primitive_policy(41,b"exact-content".to_vec(),1.0,1.0,trace,true,|entry| {
                assert_eq!(entry.raw_bytes,b"exact-content");
                assert_eq!(entry.primitive_chain.len(),timestamp as usize);
                assert!(matches!(v.primitive.try_write(),Err(std::sync::TryLockError::WouldBlock)));
            }).unwrap();
            assert_eq!(row,0);
            assert!(v.primitive.try_write().is_ok());
        }
    }

    #[test]
    fn negative_dedup_increments_frequency() {
        let v = KnowledgeVault::new();
        let ctx = NegativeContext {
            failure_reason_code: 0x101,
            process: "INDUSTRIAL".to_string(),
            mass_at_fail: 0.50,
            frequency: 1,
        };
        v.write_negative(1, vec![0], ctx.clone(), 10)
            .expect("negative_dedup_increments_frequency: rezultat i papritur në test");
        v.write_negative(2, vec![0], ctx.clone(), 20)
            .expect("negative_dedup_increments_frequency: rezultat i papritur në test");
        assert_eq!(v.negative_count(), 1); // dedupe — i njëjti model
        let store = v
            .negative
            .read()
            .expect("negative_dedup_increments_frequency: rezultat i papritur në test");
        assert_eq!(store[0].frequency, 2);
    }

    #[test]
    fn known_negative_boundary_lookup() {
        let v = KnowledgeVault::new();
        let ctx = NegativeContext {
            failure_reason_code: 0x1,
            process: "TERR".to_string(),
            mass_at_fail: 0.50,
            frequency: 1,
        };
        v.write_negative(1, vec![0], ctx, 10)
            .expect("known_negative_boundary_lookup: rezultat i papritur në test");
        assert!(v.check_known_negative(0.50, "TERR").is_some());
        assert!(v.check_known_negative(0.50, "OTHER").is_none());
        assert!(v.check_known_negative(0.90, "TERR").is_none()); // jashtë tolerancës
    }

    #[test]
    fn verified_routing_by_band() {
        let v = KnowledgeVault::new();
        v.write_verified(1, vec![0], KnowledgeBand::Hypothesis, 0.80, 1)
            .expect("verified_routing_by_band: rezultat i papritur në test");
        v.write_verified(2, vec![0], KnowledgeBand::Fact, 1.00, 2)
            .expect("verified_routing_by_band: rezultat i papritur në test");
        v.write_verified(3, vec![0], KnowledgeBand::Scientific, 1.00, 3)
            .expect("verified_routing_by_band: rezultat i papritur në test");
        assert_eq!(v.hypothesis_count(), 1);
        assert_eq!(v.fact_count(), 2); // Fact + Scientific → fact store
    }

    // ── FAZA 2 — durabiliteti (round-trip në disk) ──────────────────────

    fn temp_wal(tag: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("temp_wal: rezultat i papritur në test")
            .as_nanos();
        let mut p = std::env::temp_dir();
        p.push(format!(
            "shadow_wal_{}_{}_{}.log",
            tag,
            std::process::id(),
            nanos
        ));
        p
    }

    #[test]
    fn durability_roundtrip() {
        let path = temp_wal("roundtrip");

        // Sesioni 1 — shkruaj dije, pastaj "rrëzo" (drop).
        {
            let v = KnowledgeVault::open_disk(&path)
                .expect("durability_roundtrip: rezultat i papritur në test");
            wp(&v, 100, "science", b"prim".to_vec(), 1);
            v.write_verified(200, b"hyp".to_vec(), KnowledgeBand::Hypothesis, 0.80, 2)
                .expect("durability_roundtrip: rezultat i papritur në test");
            v.write_verified(300, b"fact".to_vec(), KnowledgeBand::Fact, 1.00, 3)
                .expect("durability_roundtrip: rezultat i papritur në test");
            let ctx = NegativeContext {
                failure_reason_code: 0x1,
                process: "TERR".to_string(),
                mass_at_fail: 0.5,
                frequency: 1,
            };
            v.write_negative(400, b"neg".to_vec(), ctx.clone(), 4)
                .expect("durability_roundtrip: rezultat i papritur në test");
            v.write_negative(401, b"neg2".to_vec(), ctx, 5)
                .expect("durability_roundtrip: rezultat i papritur në test"); // dedupe → frekuencë 2
            assert_eq!(v.primitive_count(), 1);
            assert_eq!(v.negative_count(), 1);
        }

        // Sesioni 2 — rihap; gjendja rikrijohet EKZAKT nga WAL-i.
        {
            let v = KnowledgeVault::open_disk(&path)
                .expect("durability_roundtrip: rezultat i papritur në test");
            assert_eq!(v.primitive_count(), 1);
            assert_eq!(v.hypothesis_count(), 1);
            assert_eq!(v.fact_count(), 1);
            assert_eq!(v.negative_count(), 1);
            let neg = v
                .check_known_negative(0.5, "TERR")
                .expect("durability_roundtrip: rezultat i papritur në test");
            assert_eq!(neg.frequency, 2); // dedup-i mbijetoi restart-in
        }

        drop(std::fs::remove_file(&path));
    }

    #[test]
    fn promotion_survives_restart() {
        let path = temp_wal("promote");
        let content = b"p".to_vec();

        {
            let v = KnowledgeVault::open_disk(&path)
                .expect("promotion_survives_restart: rezultat i papritur në test");
            // I njëjti input në 5 domain → kristalizim Legacy.
            for (i, dom) in FIVE_DOMAINS.iter().enumerate() {
                wp(&v, 500 + i as u64, dom, content.clone(), 1 + i as u64);
            }
            assert_eq!(v.primitive_count(), 1);
            let promoted = v.confirm_primitive(500, 3);
            assert_eq!(promoted, 1);
            assert_eq!(v.primitive_count(), 0);
            assert_eq!(v.legacy_count(), 1);
        }

        {
            // Promovimi riprodhohet nga 5 ngjarje Primitive + Confirm;
            // domain-et pa real_hits nuk fitojnë vetë autoritet Legacy.
            let v = KnowledgeVault::open_disk(&path)
                .expect("promotion_survives_restart: rezultat i papritur në test");
            assert_eq!(v.primitive_count(), 0);
            assert_eq!(v.legacy_count(), 1);
        }

        drop(std::fs::remove_file(&path));
    }

    #[test]
    fn corrupt_tail_is_rejected_without_truncation() {
        use std::io::Write;
        let path = temp_wal("corrupt_reject");
        {
            let vault = KnowledgeVault::open_disk(&path).expect("initial vault");
            wp(&vault, 1, "science", b"ok".to_vec(), 1);
        }
        {
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .expect("append isolated corruption fixture");
            // Original fixture: a COMPLETE LEN=u32::MAX, not an unverifiable
            // partial LEN. FILE3 must reject this out-of-bounds header rather
            // than treating detected corruption as a recoverable torn write.
            file.write_all(&[0xFF, 0xFF, 0xFF, 0xFF, 0x00])
                .expect("write corrupted length");
            file.sync_all().expect("persist isolated fixture");
        }
        let before = std::fs::read(&path).expect("snapshot corrupted WAL");
        match KnowledgeVault::open_disk(&path) {
            Err(ShadowError::VaultWriteError(message)) => {
                assert!(message.contains("WAL_RECORD_LIMIT"), "{message}");
            }
            Err(error) => panic!("expected corrupt-length refusal, got {error:?}"),
            Ok(_) => panic!("corrupt complete LEN must not be repaired"),
        }
        assert_eq!(std::fs::read(&path).expect("read rejected WAL"), before);
        std::fs::remove_file(path).expect("remove isolated corruption fixture");
    }

    #[test]
    fn consistent_torn_tail_recovers_and_remains_appendable() {
        use std::io::Write;
        let path = temp_wal("torn_recover");
        {
            let vault = KnowledgeVault::open_disk(&path).expect("initial vault");
            wp(&vault, 1, "science", b"ok".to_vec(), 1);
        }
        let committed = std::fs::read(&path).expect("committed fixture");
        // Copy a real encoded frame (after the 12-byte FILE header), then omit
        // its final payload byte. LEN, !LEN and CRC are a correct header; only
        // the interrupted payload is incomplete. No invented garbage header.
        let frame = &committed[12..];
        assert!(frame.len() > 12);
        {
            let mut file = std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .expect("append isolated torn fixture");
            file.write_all(&frame[..frame.len() - 1])
                .expect("partial payload");
            file.sync_all().expect("persist torn fixture");
        }
        {
            let vault = KnowledgeVault::open_disk(&path).expect("recover consistent torn tail");
            assert_eq!(vault.primitive_count(), 1);
            assert_eq!(
                std::fs::metadata(&path).expect("repaired length").len(),
                committed.len() as u64
            );
            wp(&vault, 2, "science", b"next".to_vec(), 2);
            assert_eq!(vault.primitive_count(), 2);
        }
        {
            let vault = KnowledgeVault::open_disk(&path).expect("reopen following clean append");
            assert_eq!(vault.primitive_count(), 2);
        }
        std::fs::remove_file(path).expect("remove isolated torn fixture");
    }

    fn replay_fixture_transaction(
        input_id: u64,
        identity: [u8; 32],
        cycle: [u8; 32],
    ) -> VaultTransaction {
        let mut tx = crate::vault_transaction::test_support::sample_transaction_with(
            identity,
            cycle,
            b"replay-fixture",
        );
        tx.lineage.input_id = input_id;
        for write in &mut tx.knowledge_writes {
            match write {
                KnowledgeWrite::Primitive { input_id: id, .. }
                | KnowledgeWrite::Negative { input_id: id, .. } => *id = input_id,
            }
        }
        let verdict = crate::types::LawConfirmedVerdict::confirm(
            crate::vault_transaction::test_support::sample_supreme(1, 1),
            false,
        )
        .expect("fixture verdict");
        VaultTransaction::build(
            tx.identity_digest,
            tx.cycle_digest,
            tx.anchor_generation,
            tx.negative_batch_digest,
            tx.runtime_delta_digest,
            tx.vds_close_digest,
            tx.law0_digest,
            &verdict,
            tx.knowledge_writes,
            tx.lineage,
            tx.gcl_reinforcement,
            tx.receipt,
        )
    }

    fn append_consistent_incomplete_header(path: &std::path::Path) -> Vec<u8> {
        use std::io::Write;
        let complete = std::fs::read(path).expect("complete fixture bytes");
        // Real LEN + !LEN from an existing correctly encoded frame. Only CRC
        // and payload are absent: this suffix alone is safe to repair.
        let prefix = &complete[12..20];
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(path)
            .expect("fixture append");
        file.write_all(prefix)
            .expect("consistent torn frame header");
        file.sync_all().expect("persist fixture");
        drop(file);
        std::fs::read(path).expect("complete bad fixture snapshot")
    }

    #[test]
    fn promoted_pd_scope_preserves_exact_retry_but_rejects_new_append_and_replay() {
        use crate::vault_disk::ReplayOp;
        use shadow_contracts::pd_memory::PdOperationRecord;
        let path = temp_wal("promoted_pd_no_fork");
        let initial = crate::pd_memory_store::tests::request("promotion-first");
        let first;
        {
            let vault = KnowledgeVault::open_disk(&path).unwrap();
            first = vault.commit_pd_operation(initial.clone()).unwrap();
        }
        let before = std::fs::read(&path).unwrap();
        {
            let vault = KnowledgeVault::open_disk(&path).unwrap();
            // Explicit data-only gate fixture. Actual native promotion is tested
            // by the native runner; no constitutional authority is simulated here.
            let session = shadow_contracts::pd_memory::hex(&initial.identity.key().unwrap());
            let scope = crate::native_cycle::cognitive_scope_key(&session, &initial.identity.scope);
            let native_head = crate::native_cycle::NativePdHead {
                identity:[31;32], request_sha256:[32;32], transaction_id:[33;32], root_identity:[31;32],
            };
            vault.native_pd_heads.write().unwrap().insert(scope,native_head);
            assert_eq!(vault.commit_pd_operation(initial).unwrap(), first);
            let mut replay_lineages = HashMap::new();
            vault.replay_op(ReplayOp::PdOperation(first.clone()), &mut replay_lineages).unwrap();
            let mut next = crate::pd_memory_store::tests::request("promotion-new-answer");
            next.expected_revision = first.revision;
            next.previous_state = first.state_digest;
            let record = PdOperationRecord::construct(next.clone(),first.record_id).unwrap();
            for error in [vault.commit_pd_operation(next).unwrap_err(),
                vault.replay_op(ReplayOp::PdOperation(record),&mut replay_lineages).unwrap_err()] {
                assert!(error.to_string().contains("PD_NATIVE_CONTINUATION_REQUIRED_AFTER_PROMOTION"));
            }
            assert_eq!(vault.pd_head(&first.request.identity).unwrap(),Some(first.clone()));
            assert!(vault.pd_operation(&first.request.identity,"promotion-new-answer").unwrap().is_none());
            assert_eq!(vault.native_pd_heads.read().unwrap().get(&scope),Some(&native_head));
            assert_eq!(std::fs::metadata(&path).unwrap().len(),before.len() as u64);
        }
        assert_eq!(std::fs::read(&path).unwrap(),before);
        {
            let vault = KnowledgeVault::open_disk(&path).unwrap();
            assert_eq!(vault.pd_head(&first.request.identity).unwrap(),Some(first));
        }
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn semantic_pd_chain_error_prevents_tail_repair_and_global_lineage_publication() {
        use shadow_contracts::pd_memory::PdOperationRecord;
        let path = temp_wal("semantic_pd_torn");
        let input_id = 0xF0E1_D2C3_B4A5_0711;
        let before_lineage = crate::knowledge_lineage::ledger().lookup(input_id);
        let tx = replay_fixture_transaction(input_id, [0x71; 32], [0x72; 32]);
        let request = crate::pd_memory_store::tests::request("semantic-first");
        let first = PdOperationRecord::construct(request, [0; 32]).expect("first PD");
        let mut request = crate::pd_memory_store::tests::request("semantic-wrong-parent");
        request.expected_revision = 1;
        request.previous_state = first.state_digest;
        // Internally canonical/hash-valid, but not the actual previous record.
        let wrong = PdOperationRecord::construct(request, [0x55; 32])
            .expect("self-consistent wrong parent");
        {
            let (backend, _) = crate::vault_disk::DiskBackend::open(&path).expect("fixture WAL");
            backend.commit_test_transaction(&tx).expect("raw valid TX");
            backend.commit_pd_operation(&first).expect("raw first PD");
            backend
                .commit_pd_operation(&wrong)
                .expect("raw self-consistent invalid-chain PD");
        }
        let before = append_consistent_incomplete_header(&path);
        assert!(
            KnowledgeVault::open_disk(&path).is_err(),
            "invalid semantic PD prefix must reject"
        );
        assert_eq!(std::fs::read(&path).expect("unchanged WAL"), before);
        assert_eq!(
            crate::knowledge_lineage::ledger().lookup(input_id),
            before_lineage
        );
        std::fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn semantic_transaction_conflict_prevents_tail_repair_and_partial_publication() {
        let path = temp_wal("semantic_tx_torn");
        let input_id = 0xF0E1_D2C3_B4A5_0712;
        let before_lineage = crate::knowledge_lineage::ledger().lookup(input_id);
        let first = replay_fixture_transaction(input_id, [0x73; 32], [0x74; 32]);
        let conflict = replay_fixture_transaction(input_id, [0x73; 32], [0x75; 32]);
        {
            let (backend, _) = crate::vault_disk::DiskBackend::open(&path).expect("fixture WAL");
            backend
                .commit_test_transaction(&first)
                .expect("first canonical TX");
            backend
                .commit_test_transaction(&conflict)
                .expect("second canonical TX with conflicting identity");
        }
        let before = append_consistent_incomplete_header(&path);
        assert!(KnowledgeVault::open_disk(&path).is_err());
        assert_eq!(std::fs::read(&path).expect("unchanged WAL"), before);
        assert_eq!(
            crate::knowledge_lineage::ledger().lookup(input_id),
            before_lineage
        );
        std::fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn valid_mixed_stream_replays_then_repairs_then_exposes_both_projections() {
        use shadow_contracts::pd_memory::PdOperationRecord;
        let path = temp_wal("semantic_valid_torn");
        let input_id = 0xF0E1_D2C3_B4A5_0713;
        let tx = replay_fixture_transaction(input_id, [0x76; 32], [0x77; 32]);
        let request = crate::pd_memory_store::tests::request("semantic-valid");
        let record = PdOperationRecord::construct(request, [0; 32]).expect("PD fixture");
        {
            let (backend, _) = crate::vault_disk::DiskBackend::open(&path).expect("fixture WAL");
            backend.commit_test_transaction(&tx).expect("raw canonical TX");
            backend
                .commit_pd_operation(&record)
                .expect("raw canonical PD");
        }
        let complete_len = std::fs::metadata(&path).expect("length").len();
        append_consistent_incomplete_header(&path);
        {
            let vault =
                KnowledgeVault::open_disk(&path).expect("semantically valid stream repairs");
            assert_eq!(
                std::fs::metadata(&path).expect("repaired length").len(),
                complete_len
            );
            assert_eq!(vault.primitive_count(), 1);
            assert_eq!(
                vault.pd_head(&record.request.identity).expect("PD head"),
                Some(record.clone())
            );
            assert_eq!(
                crate::knowledge_lineage::ledger().lookup(input_id),
                Some(tx.lineage)
            );
            assert!(vault
                .committed_identity(&tx.identity_digest)
                .expect("identity lookup")
                .is_some());
            assert!(vault
                .committed_identity(&[0x78; 32])
                .expect("missing identity")
                .is_none());
        }
        {
            let vault = KnowledgeVault::open_disk(&path).expect("second reopen");
            assert_eq!(vault.primitive_count(), 1);
            assert_eq!(
                vault.pd_head(&record.request.identity).expect("PD head"),
                Some(record)
            );
        }
        std::fs::remove_file(path).expect("remove fixture");
    }

    #[derive(Debug)]
    struct FailingBackend;

    impl VaultBackend for FailingBackend {
        fn append_verified(&self, _permit: KnowledgeAppend<'_>) -> Result<(), VaultError> {
            Err(VaultError::Io(String::from("fault injection")))
        }
    }

    #[test]
    fn transactional_backend_failure_leaves_ram_unchanged() {
        let vault = KnowledgeVault::with_backend(Box::new(FailingBackend));
        let tx = crate::vault_transaction::test_support::sample_transaction();
        let cycle = tx.cycle_digest;
        assert!(vault.commit_test_transaction(tx).is_err());
        assert_eq!(vault.primitive_count(), 0);
        assert_eq!(vault.negative_count(), 0);
        assert!(vault.committed_transaction(&cycle).is_none());
    }

    // Synthetic transactions exercise publication, not independent P/N proof.
    #[test]
    fn concurrent_identity_lookup_never_observes_a_dangling_commit() {
        use std::sync::{Arc, Barrier};
        let vault = Arc::new(KnowledgeVault::new());
        let transactions: Vec<_> = (1u64..=256)
            .map(|n| {
                let mut identity = [0x91; 32];
                identity[..8].copy_from_slice(&n.to_le_bytes());
                let mut cycle = [0x92; 32];
                cycle[..8].copy_from_slice(&n.to_le_bytes());
                crate::vault_transaction::test_support::sample_transaction_with(
                    identity, cycle, &n.to_le_bytes(),
                )
            })
            .collect();
        let expected: Vec<_> = transactions.iter()
            .map(|tx| (tx.identity_digest, tx.transaction_id, tx.receipt.clone()))
            .collect();
        let start = Arc::new(Barrier::new(3));
        std::thread::scope(|scope| {
            let writer_vault = Arc::clone(&vault);
            let writer_start = Arc::clone(&start);
            scope.spawn(move || {
                writer_start.wait();
                for tx in transactions {
                    writer_vault.commit_test_transaction(tx).expect("publish transaction");
                    std::thread::yield_now();
                }
            });
            for _ in 0..2 {
                let reader_vault = Arc::clone(&vault);
                let reader_start = Arc::clone(&start);
                let records = &expected;
                scope.spawn(move || {
                    reader_start.wait();
                    for _ in 0..64 {
                        for (identity, transaction_id, receipt) in records {
                            if let Some(found) = reader_vault.committed_identity(identity)
                                .expect("absence or complete record, never dangling identity") {
                                assert_eq!(&found.transaction_id, transaction_id);
                                assert_eq!(&found.receipt, receipt);
                                assert_eq!(found.status, CommitStatus::AlreadyCommitted);
                            }
                        }
                        std::thread::yield_now();
                    }
                });
            }
        });
        for (identity, transaction_id, receipt) in expected {
            let found = vault.committed_identity(&identity).unwrap().expect("published");
            assert_eq!(found.transaction_id, transaction_id);
            assert_eq!(found.receipt, receipt);
        }
    }

    #[test]
    fn same_transaction_is_exactly_once() {
        let vault = KnowledgeVault::new();
        let tx = crate::vault_transaction::test_support::sample_transaction();
        let cycle = tx.cycle_digest;
        let first = vault.commit_test_transaction(tx.clone()).expect("first commit");
        let second = vault.commit_test_transaction(tx).expect("retry commit");
        assert_eq!(first.status, CommitStatus::Committed);
        assert_eq!(second.status, CommitStatus::AlreadyCommitted);
        assert_eq!(first.transaction_id, second.transaction_id);
        assert_eq!(first.receipt, second.receipt);
        assert_eq!(vault.primitive_count(), 1);
        assert!(vault.committed_receipt(&cycle).is_some());
    }

    #[test]
    fn same_identity_with_different_cycle_is_rejected() {
        let vault = KnowledgeVault::new();
        let first = crate::vault_transaction::test_support::sample_transaction_with(
            [0x11u8; 32],
            [0x22u8; 32],
            b"first-cycle",
        );
        let conflict = crate::vault_transaction::test_support::sample_transaction_with(
            [0x11u8; 32],
            [0x33u8; 32],
            b"conflicting-cycle",
        );
        vault
            .commit_test_transaction(first)
            .expect("first identity commit");
        assert!(vault.commit_test_transaction(conflict).is_err());
        assert_eq!(vault.primitive_count(), 1);
    }
}

#[cfg(test)]
#[path = "scoped_selection_tests.rs"]
mod scoped_selection_tests;

#[cfg(test)]
#[path = "native_inquiry_tests.rs"]
mod native_inquiry_tests;

#[cfg(test)]
#[path = "late_refusal_tests.rs"]
mod late_refusal_tests;

#[cfg(test)]
mod live_append_boundary_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug, Default)]
    struct CountingBackend(AtomicUsize);

    impl VaultBackend for CountingBackend {
        fn append_verified(&self, permit: KnowledgeAppend<'_>) -> Result<(), VaultError> {
            permit.transaction_for(self)?;
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[test]
    fn r9_append_permission_cannot_target_a_different_backend() {
        let origin = CountingBackend::default();
        let other = CountingBackend::default();
        let tx = crate::vault_transaction::test_support::sample_transaction();
        let permission = KnowledgeAppend { transaction: &tx, backend: &origin };
        let error = other.append_verified(permission).expect_err("different backend");
        assert!(error.to_string().contains("LIVE_APPEND_WRONG_BACKEND"));
        assert_eq!(origin.0.load(Ordering::SeqCst), 0);
        assert_eq!(other.0.load(Ordering::SeqCst), 0);
        // Positive control: the identical borrowed transaction reaches its owner.
        origin.append_verified(KnowledgeAppend { transaction: &tx, backend: &origin }).unwrap();
        assert_eq!(origin.0.load(Ordering::SeqCst), 1);
        assert_eq!(other.0.load(Ordering::SeqCst), 0);
    }
}


#[cfg(test)]
mod process_observation_consumer_tests {
    use super::*;
    use shadow_contracts::negative_asset::PROCESS_OBSERVATION_REASON;

    // Storage-only fixtures exercise projection and actual disk replay, not GCL admission.
    fn record(v:&KnowledgeVault,id:u64,reason:u32,process:&str) {
        v.write_negative(id,b"era00".to_vec(),NegativeContext{
            failure_reason_code:reason,process:process.into(),mass_at_fail:0.5,frequency:3,
        },id+1).unwrap();
    }
    fn request()->shadow_contracts::KnowledgeSelectionRequestWire {
        shadow_contracts::KnowledgeSelectionRequestWire {
            version:shadow_contracts::PROTOCOL_VERSION,
            requester:shadow_contracts::SelectionRequester::AshtQuantum,
            purpose:shadow_contracts::SelectionPurpose::EvidenceCandidates,
            session_id:"process-observation".into(),parent_i0:"11".repeat(32),primitive_anchor:7,
            trace_id:"process-observation".into(),split_sha256:"22".repeat(32),
            xi:vec!["era00".into()],yi:vec![],upstream_hold_sha256:Some("33".repeat(32)),
            evidence_sha256:"44".repeat(32),max_positive:1,max_negative:1,
        }
    }
    #[test]
    fn r10_process_observations_are_retained_without_refuting_or_diluting_real_evidence() {
        let v=KnowledgeVault::new();
        record(&v,1,PROCESS_OBSERVATION_REASON,"WORK");
        assert_eq!(v.negative_count(),1);
        assert_eq!(v.export_negative()[0].raw_bytes,b"era00");
        assert_eq!(v.query_negative_pattern(0.5),0.0);
        assert!(v.check_known_negative(0.5,"WORK").is_none());
        assert!(v.select_bounded_candidates(&request()).unwrap().1.is_empty());
        assert_eq!(v.export_negative()[0].access_count,0);
        record(&v,2,0x777,"WORK");
        let only_real=KnowledgeVault::new();record(&only_real,2,0x777,"WORK");
        assert_eq!(v.query_negative_pattern(0.5),only_real.query_negative_pattern(0.5));
        assert_eq!(v.select_bounded_candidates(&request()).unwrap(),
            only_real.select_bounded_candidates(&request()).unwrap());
        assert_eq!(v.select_bounded_candidates(&request()).unwrap().1[0].candidate_id,2,
            "excluded process observations cannot consume the one-candidate capacity");
        assert_eq!(v.check_known_negative(0.5,"WORK").unwrap().failure_reason_code,0x777);
        assert_eq!(v.negative_count(),2);
        assert_eq!(v.export_negative()[0].access_count,0);
    }
    #[test]
    fn r10_legacy_ambiguous_full_proposition_is_not_silently_reclassified() {
        let v=KnowledgeVault::new();record(&v,3,0xA451,"WORK");
        assert_eq!(v.select_bounded_candidates(&request()).unwrap().1.len(),1);
        assert!(v.check_known_negative(0.5,"WORK").is_some());
        assert!(v.query_negative_pattern(0.5)>0.0);
    }
    #[test]
    fn r10_observation_projection_survives_disk_replay_without_erasure_or_read_writes() {
        let path=std::env::temp_dir().join(format!("r10-observation-{}-{}.svlt",
            std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        {
            let v=KnowledgeVault::open_disk(&path).unwrap();
            record(&v,1,PROCESS_OBSERVATION_REASON,"WORK");record(&v,2,0x777,"WORK");
        }
        let before=std::fs::read(&path).unwrap();
        {
            let v=KnowledgeVault::open_disk(&path).unwrap();
            assert_eq!(v.negative_count(),2);assert_eq!(v.export_negative().len(),2);
            assert_eq!(v.select_bounded_candidates(&request()).unwrap().1[0].candidate_id,2);
            assert_eq!(v.check_known_negative(0.5,"WORK").unwrap().failure_reason_code,0x777);
        }
        assert_eq!(std::fs::read(&path).unwrap(),before);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn r10_source_staged_negative_axes_remain_active_for_the_next_cycle() {
        // Actual source staging checks anchored/excluded targets; storage below
        // is an explicit test fixture, not a fresh constitutional judgement.
        // Activity as a negative path is not universal scientific refutation.
        for axis in ["Xi","Yi"] {
            let split=crate::lab_contracts::PrimitiveSplit {
                xi:if axis=="Xi" {vec!["era00".into()]} else {vec![]},
                yi:if axis=="Yi" {vec!["era00".into()]} else {vec![]},
            };
            let wire=format!("era00~{axis}~Reverse^-1:gcl_target:weakness=10000");
            let staged=crate::pending_negative::PendingNegativeBatch::stage(
                &[wire],Some(&split),None,None,0.5,1).unwrap();
            assert_eq!(staged.count,1);
            let v=KnowledgeVault::new();
            for write in staged.into_writes() {
                let crate::shadow_commit::KnowledgeWrite::Negative {input_id,raw_bytes,context,ts_ns}=write
                    else {panic!("source negative staging changed kind")};
                assert_eq!(context.failure_reason_code,0xA451);
                v.write_negative(input_id,raw_bytes,context,ts_ns).unwrap();
            }
            let stored=v.export_negative();assert_eq!(stored.len(),1);
            assert_eq!(stored[0].raw_bytes,b"era00");
            assert!(stored[0].process.starts_with(&format!("KOLAPS/2:-{axis}(term=era00;")));
            assert!(v.query_negative_pattern(0.5)>0.0);
            let selected=v.select_bounded_candidates(&request()).unwrap().1;
            assert_eq!(selected.len(),1);assert_eq!(selected[0].polarity,0);
            assert_eq!(selected[0].candidate_id,stored[0].input_id);
            assert!(v.check_known_negative(0.5,&stored[0].process).is_some());
        }
    }
}

#[cfg(test)]
mod r31_vault_delivery {
    use super::*;
    #[test]
    fn r31_exact_map_and_candidate_reads_are_triggered_without_creating_material() {
        let vault=KnowledgeVault::new();
        let identity=shadow_contracts::pd_memory::PdIdentity {
            owner:"r31-owner".into(), obligation:"r31-obligation".into(),
            case_id:"r31-case".into(), scope:"r31-scope".into(),
            domain:"r31-domain".into(), lineage_root:"r31-root".into(),
        };
        let before=vault.store_counts_for_test();
        crate::noi_caller::test_events();
        assert!(vault.pd_head(&identity).unwrap().is_none());
        assert!(vault.pd_candidate(&identity, &[7;32]).unwrap().is_none());
        assert_eq!(vault.store_counts_for_test(),before);
        let events=crate::noi_caller::test_events();
        for site in ["SHADOW_PD_HEAD","SHADOW_PD_CANDIDATE"] {
            assert!(events.iter().any(|(s,p)|s==site && p=="RETURNED_OK"),
                "exact Vault read bypassed 0-NOI: {site}; {events:?}");
        }
        let mut invalid=identity;
        invalid.owner.clear();
        assert!(vault.pd_head(&invalid).is_err());
        assert_eq!(vault.store_counts_for_test(),before);
        assert!(!crate::noi_caller::test_events().iter().any(|(s,_)|s.starts_with("SHADOW_PERSIST")));
    }
}

#[cfg(test)]
#[path = "maturation_tests.rs"]
mod maturation_tests;
