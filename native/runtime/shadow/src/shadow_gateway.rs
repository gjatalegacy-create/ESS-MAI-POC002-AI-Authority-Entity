// ╔══════════════════════════════════════════════════════════════════════════╗
// ║  SHADOW_GATEWAY.RS — HYRJA E VETME SOVRANE                            ║
// ║  Gjata Legacy™  |  Arkitekt: Bledar Gjata                             ║
// ║                                                                          ║
// ║  `Shadow` — core privat i procesit. Hyrja production: ingest_bridged.  ║
// ║                                                                          ║
// ║  Dy origjinat mbërrijnë VEÇMAS (Quantum → PassPackage, Light →         ║
// ║  LightEnvelope) dhe BASHKOHEN brenda pipeline-it. Shadow është sink-u  ║
// ║  sovran: merr pronësinë (move) dhe asgjë nuk klonohet (LIGJI 0).      ║
// ║                                                                          ║
// ║  Ngrirja → refuzim i fortë (Err), pa punë, pa shkrim.                 ║
// ╚══════════════════════════════════════════════════════════════════════════╝

use crate::anchor_lease::AnchorLease;
use crate::bridge::{LightInbound, QuantumInbound};
use crate::knowledge_lineage::Lineage;
use crate::knowledge_vault::{init_global_vault, KnowledgeVault};
use crate::lab_contracts::{CollapsePhase, GjataCollapseLaw, PrimitiveSplit};
use crate::pending_negative::PendingNegativeBatch;
use crate::shadow_commit::{KnowledgeWrite, SupremeCandidate};
use crate::shadow_gj_legacy::ShadowGjLegacy;
use crate::shadow_pipeline::{now_ns, run_pipeline};
use crate::sovereign_ffi_gate::{seal_verified_output, VerificationReceipt};
use crate::types::{
    LawConfirmedVerdict, LightEnvelope, NegativeContext, PassPackage, ShadowError,
    StructuralRefusal, SupremeVerdict,
};
use crate::vault_transaction::{sha256_parts, DurableCommit, GclReinforcement, VaultTransaction};
use crate::flow_stage::AwaitingAnchor;
use crate::verefied_diary_supremelaw::{
    GclPrimitiveMode, GclPrimitiveVerification, ShadowFlow,
};

/// Arsyeja e ruajtjes negative nga kolapsi (lidhur me vulën sovrane 0xA451).
const REASON_COLLAPSE_ELIM: u32 = 0xA451;

/// R23 private scope for the current ledger projection. The process boundary
/// selects the explicit validated candidate format; this value is created only
/// after the gateway's model/history checks. It supplies no score or authority.
struct VerifiedCurrentLedgerScope {
    input_id: u64,
    final_evidence_digest: u64,
    matrix: Option<crate::kie_history::VerifiedCounterproofProjection>,
    model_case: Option<crate::case_reasoning::VerifiedModelCase>,
}

#[derive(Clone, Copy)]
enum BridgeEvidencePolicy { Legacy, CandidateV1, ScopedMatrixV2 }

impl VerifiedCurrentLedgerScope {
    fn matches(&self, pkg: &PassPackage) -> bool {
        self.input_id == crate::shadow_pipeline::stable_id(&pkg.package_id)
            && self.final_evidence_digest == pkg.final_evidence_digest
            && self.model_case.as_ref().is_none_or(|value| value.matches(pkg))
    }
}

use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

/// Dosja ku shënohet ledger-i i rrjedhave. NUK është autoritet — vetëm
/// dëshmi. Në runtime real boot-i sovran e GARANTON env-in (FATAL pa të);
/// në teste (pa boot) përdoret temp-i ABSOLUT i sistemit — kjo s'është
/// "fallback relativ" (asnjë cwd, asnjë rrugë relative) dhe s'prek kurrë
/// asnjë vendim: ledger-i vetëm shënon. Zero if — vetëm match.
fn rrjedha_dir() -> String {
    match std::env::var("ESSMAI_HANDOFF_DIR") {
        Ok(d) => d,
        Err(_) => std::env::temp_dir().to_string_lossy().into_owned(),
    }
}

/// Përgjigja e Shadow — verdikti suprem + konteksti.
#[derive(Debug, Clone)]
pub struct ShadowResponse {
    /// Sesioni (i zhvendosur nga LightEnvelope — pa klon).
    pub session_id: String,
    /// Vendimi suprem.
    pub verdict: SupremeVerdict,
    /// Nëse kjo rrugë ishte tashmë e njohur si negative (kufi aktiv).
    pub known_negative: Option<NegativeContext>,
    /// Output-i final i Shadow i vulosur nga autoriteti i verifikimit.
    pub verification_receipt: VerificationReceipt,
    /// True only when the validated parent negative asset was included in the
    /// transaction whose durable commit (or idempotent replay) produced this response.
    pub negative_persisted: bool,
}

/// Gjendja e vendosur nga pipeline-i, por ende e pavulosur. Ky tip është privat:
/// as Quantum, as Light dhe as Nura nuk mund ta shohin para mbylljes GCL.
struct UnsealedShadowDecision {
    session_id: String,
    verdict: LawConfirmedVerdict,
    knowledge_writes: Vec<KnowledgeWrite>,
    lineage: Lineage,
    staged_runtime: crate::shadow_spine::ShadowVerificationRuntime,
    runtime_delta_digest: [u8; 32],
    negative_batch_digest: [u8; 32],
    parent_negative_staged: bool,
    known_negative: Option<NegativeContext>,
}

/// R9: proof of passage through the private live decision path. Owns the
/// complete write-set; no Clone, serialization, public constructor or mutable
/// access. It adds no epistemic authority: only the existing pipeline can mint it.
#[derive(Debug)]
pub(crate) struct AuthorizedLiveTransaction<'v> {
    transaction: VaultTransaction,
    originating_vault: &'v KnowledgeVault,
}

impl AuthorizedLiveTransaction<'_> {
    pub(crate) fn into_transaction_for(
        self, vault: &KnowledgeVault,
    ) -> Result<VaultTransaction, ShadowError> {
        if !std::ptr::eq(self.originating_vault, vault) {
            return Err(ShadowError::VaultWriteError("LIVE_COMMIT_WRONG_VAULT".into()));
        }
        Ok(self.transaction)
    }
}

/// Konteksti faktik që lidh seal-in final me i₀ në pritje dhe XY e gjykuar.
/// Nuk del nga Shadow; vetëm përbërësit e tij të vulosur hyjnë në receipt.
struct VerificationContext {
    session_id: String,
    parent_i0: String,
    primitive_anchor: u64,
    xy_digest: u64,
    pd_binding_digest: u64,
    pd_continuum_activation_digest: u64,
    anchor_generation: u64,
    vds_close_digest: [u8; 32],
    law0_digest: [u8; 32],
}

impl VerificationContext {
    #[cfg(test)]
    fn direct(session_id: &str, parent_i0: &str, primitive_anchor: u64) -> Self {
        Self::from_xy(
            session_id,
            parent_i0,
            primitive_anchor,
            "",
            "",
            0,
            "DIRECT",
            0,
            0,
            0,
            0,
            0,
            0,
        )
    }

    fn from_xy(
        session_id: &str,
        parent_i0: &str,
        primitive_anchor: u64,
        x: &str,
        y: &str,
        mass: u32,
        collapse: &str,
        pd_binding_digest: u64,
        pd_continuum_activation_digest: u64,
        final_evidence_digest: u64,
        pd_gcl_process_digest: u64,
        spine_completion_digest: u64,
        anchor_generation: u64,
    ) -> Self {
        let mut material =
            Vec::with_capacity(parent_i0.len() + x.len() + y.len() + collapse.len() + 40);
        material.extend_from_slice(parent_i0.as_bytes());
        material.extend_from_slice(&primitive_anchor.to_le_bytes());
        material.extend_from_slice(x.as_bytes());
        material.push(0x1f);
        material.extend_from_slice(y.as_bytes());
        material.extend_from_slice(&mass.to_le_bytes());
        material.extend_from_slice(collapse.as_bytes());
        material.extend_from_slice(&final_evidence_digest.to_le_bytes());
        material.extend_from_slice(&pd_gcl_process_digest.to_le_bytes());
        material.extend_from_slice(&spine_completion_digest.to_le_bytes());
        let xy_digest = crate::lab_contracts::verification_receipt::fnv1a64(&material);
        Self {
            session_id: session_id.to_string(),
            parent_i0: parent_i0.to_string(),
            primitive_anchor,
            xy_digest,
            pd_binding_digest,
            pd_continuum_activation_digest,
            anchor_generation,
            vds_close_digest: [0u8; 32],
            law0_digest: [0u8; 32],
        }
    }
}

/// Vëzhgueshmëria e Shadow.
#[derive(Debug, Clone, Copy)]
pub struct ShadowStats {
    pub total: u64,
    pub passed: u64,
    pub blocked: u64,
    pub frozen: bool,
    pub primitive_count: usize,
    pub legacy_count: usize,
    pub negative_count: usize,
    pub hypothesis_count: usize,
    pub fact_count: usize,
    /// VULA E AUDITIMIT (HARDENING): ora reale dështoi ≥1 herë — u përdor
    /// fallback monotonik. Lineage i ruajtur; auditimi njoftohet me zë.
    pub time_degraded: bool,
    pub verification_ledger_count: usize,
    pub verification_wisdom_count: usize,
}

/// Autoriteti suprem AIGovern-Cyber — memoria sovrane.
#[derive(Debug)]
pub struct Shadow {
    vault: Arc<KnowledgeVault>,
    frozen: AtomicI32,
    /// PA (Primitive Anchor) i regjistruar HERË nga Light, PARA se Quantum të
    /// ketë mbaruar reasoning-un. session_id → pa_id (TraceInfo.id nga Light).
    /// Hartë e vogël, ephemeral (jo persistent) — vetëm gate-i i hyrjes, jo dije.
    /// PA + (Xi,Yi) NË PRITJE — sinjali i Light: ankora dhe dy hapësirat
    /// primitive presin propozimin XY të Quantum (KOLAPS/3 verifikimi).
    pa_waiting: Mutex<HashMap<String, (u64, PrimitiveSplit)>>,
    /// Multi-verification memory/wisdom; vetëm procesi main.rs e zotëron.
    verification: Mutex<crate::shadow_spine::ShadowVerificationRuntime>,
}

impl Shadow {
    /// Inicializon Shadow: kerneli + vault-i global.
    #[cfg(test)]
    pub fn new() -> Result<Self, ShadowError> {
        ShadowGjLegacy::init()?;
        let vault = Arc::new(KnowledgeVault::new());
        init_global_vault(vault.clone());
        Ok(Self {
            vault,
            frozen: AtomicI32::new(0),
            pa_waiting: Mutex::new(HashMap::new()),
            verification: Mutex::new(crate::shadow_spine::ShadowVerificationRuntime::new()),
        })
    }

    /// FAZA 2 — Krijon Shadow me memorie sovrane DURABLE (WAL në disk).
    /// Ripërsërit gjendjen ekzistuese nga `path` dhe e krijon nëse mungon;
    /// çdo vendim i mëpasshëm shtohet (append) dhe mbijeton restart-in.
    pub fn with_disk<P: AsRef<std::path::Path>>(path: P) -> Result<Self, ShadowError> {
        ShadowGjLegacy::init()?;
        let vault = Arc::new(KnowledgeVault::open_disk(path.as_ref())?);
        init_global_vault(vault.clone());
        Ok(Self {
            vault,
            frozen: AtomicI32::new(0),
            pa_waiting: Mutex::new(HashMap::new()),
            verification: Mutex::new(crate::shadow_spine::ShadowVerificationRuntime::new()),
        })
    }

    /// Private executable composition: preserve once-command verification/PA state
    /// while reusing the single exclusively owned, authenticated Vault projection.
    pub(crate) fn with_resident_vault(vault: Arc<KnowledgeVault>) -> Self {
        Self { vault, frozen: AtomicI32::new(0), pa_waiting: Mutex::new(HashMap::new()),
            verification: Mutex::new(crate::shadow_spine::ShadowVerificationRuntime::new()) }
    }
    pub(crate) fn resident_vault(&self) -> Arc<KnowledgeVault> { self.vault.clone() }

    /// ENTRYPOINT MINIMAL — regjistron Primitive Anchor (PA) HERË, sapo Light
    /// e dërgon, PARA se Quantum të ketë mbaruar reasoning-un. Idempotent
    /// (mbishkruan nëse session_id përsëritet). S'arsyeton, s'verifikon dije —
    /// thjesht e mban PA-në në pritje deri sa Quantum të vijë në `ingest_bridged`.
    pub fn register_primitive_anchor(&self, session_id: &str, pa_id: u64, split: PrimitiveSplit) {
        match self.pa_waiting.lock() {
            Ok(mut map) => {
                map.insert(session_id.to_string(), (pa_id, split));
            }
            Err(poisoned) => {
                // FAIL-LOUD (ULTIMATUM, jo no-op i heshtur): lock i helmuar =
                // një thread paniku. HashMap-i mbetet valid → e RIKUPEROJMË dhe
                // regjistrojmë PA-në (s'e humbim ankorën), me alarm në audit.
                eprintln!("[SHADOW_GATE] ALARM: pa_waiting lock i helmuar — rikuperim + regjistrim PA (session={})", session_id);
                let mut map = poisoned.into_inner();
                map.insert(session_id.to_string(), (pa_id, split));
            }
        }
    }

    /// URA 2 (Light→Shadow) — USHQYESI I PA-GATE-it nga skedari i këmbimit.
    ///
    /// KONTRATA E SKEDARIT (shkruar nga Light, `export_primitive_anchor`):
    ///   e re (5 fusha): "session_id|pa_id_hex|ts_ns|xi:a,b|yi:c\n"
    ///   e vjetër (3):   "session_id|pa_id_hex|ts_ns\n"   — të dyja pranohen.
    ///   pa_id_hex = {:016x}; (Xi,Yi) = kolapsi i koordinimit i Light.
    /// Ky lexues: filtron rreshtat e session-it, merr TË FUNDIT (konsistent me
    /// read_pd_delivery të Light-it), parson hex→u64 dhe e regjistron te gate-i.
    ///
    /// KONTRATË TRANSPORTI (HARDENING — fail-closed):
    ///   • skedari mungon          → Ok(0)  — nisje e parë, normale (i qetë)
    ///   • s'ka rresht për session → Ok(0)  — normale
    ///   • KORRUPSION (fusha të papritura, hex i keq, Xi/Yi wire i keq)
    ///                             → Err(TransportCorrupt) — NDALIM I FORTË.
    /// Korrupsioni i transportit NUK është "asnjë ankorë": është shkelje
    /// kontrate — sesioni refuzohet PARA ingest. Zero if/else — vetëm match.
    pub fn feed_primitive_anchors(
        &self,
        path: &str,
        session_id: &str,
    ) -> Result<usize, ShadowError> {
        // DIAGNOZË, JO HESHTJE (v1.3.2): "skedari mungon" është e VETMJA
        // gjendje normale (nisje e parë). Leje e mohuar / I/O reale = rrjedhë
        // me PSE + shënim + kufi — s'maskohet kurrë si "asnjë ankorë".
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => match e.kind() {
                std::io::ErrorKind::NotFound => return Ok(0), // nisje e parë — normale
                other_kind => {
                    let v = crate::lab_contracts::rrjedha::note(
                        &rrjedha_dir(),
                        "SHADOW",
                        "gateway::feed_primitive_anchors",
                        crate::lab_contracts::rrjedha::RrjedhaClass::DiskDenied,
                        &format!("leximi i {} dështoi: {:?} ({})", path, other_kind, e),
                    );
                    return Err(ShadowError::TransportCorrupt(format!(
                        "disku refuzoi leximin e ankorave ({:?}) — jo 'mungesë', por dështim i deklaruar [{:?}]",
                        other_kind, v)));
                }
            },
        };
        let prefix = format!("{}|", session_id);
        // KUFIRI ANTIDEGRADIM (v1.3.2): sapo sesioni ka NJË rresht të vulosur
        // (c:), rrëshqitja mbrapa në legacy është e dyshimtë — cungim i fushës
        // CRC do të dukej pikërisht kështu. Çelës njëdrejtimësh: i vulosur ⇒
        // përgjithmonë i vulosur për atë sesion.
        let sealed_exists = content
            .lines()
            .filter(|l| l.starts_with(prefix.as_str()))
            .any(|l| l.contains("|c:"));
        let last = content
            .lines()
            .filter(|l| l.starts_with(prefix.as_str()))
            .last();
        match last {
            None => Ok(0), // s'ka rresht për këtë session — normale
            Some(line) => {
                // ══ PACP GATE (kontrata pa_wire) ═══════════════════════════
                // Ankora duhet të mbërrijë BYTE-PËR-BYTE siç u shkrua nga
                // Light: 6 fusha → CRC i detyruar; 3/5 → legacy i deklaruar;
                // çdo korrupsion → TransportCorrupt PARA çdo interpretimi.
                match (
                    crate::lab_contracts::pa_wire::verify_line(line),
                    sealed_exists,
                ) {
                    (crate::lab_contracts::pa_wire::PaWireVerdict::SealedOk, _) => {
                        println!("  [PACP]     ankora e vulosur CRC-OK (session={}) — i₀ i pandryshuar në urë", session_id);
                    }
                    // ÇELËSI NJËDREJTIMËSH: legacy pranohet VETËM kur sesioni
                    // s'ka pasur KURRË rresht të vulosur — përndryshe ky është
                    // degradim (cungim i c:) dhe merr PSE + shënim + refuzim.
                    (crate::lab_contracts::pa_wire::PaWireVerdict::LegacyOk, false) => {
                        println!("  [PACP]     ankora legacy (pa CRC) e pranuar e deklaruar (session={})", session_id);
                        // v1.4.4 (legacy i përforcuar): çdo pranim legacy lë GJURMË
                        // të vulosur + kufi këshillues (Other=5) — trashëgimia
                        // pranohet, por kurrë më në errësirë.
                        let _ = crate::lab_contracts::rrjedha::note(
                            &rrjedha_dir(), "SHADOW", "gateway::feed_primitive_anchors",
                            crate::lab_contracts::rrjedha::RrjedhaClass::Other,
                            &format!("LEGACY_PRANUAR kind=PA bridge=light_pa_export session={session_id} sealed_para=false"));
                        crate::shadow_runtime_pulse::mark_ready(
                            crate::shadow_runtime_pulse::SStage::LegacyWritten,
                        );
                    }
                    (crate::lab_contracts::pa_wire::PaWireVerdict::LegacyOk, true) => {
                        let v = crate::lab_contracts::rrjedha::note(
                            &rrjedha_dir(), "SHADOW", "gateway::feed_primitive_anchors",
                            crate::lab_contracts::rrjedha::RrjedhaClass::TransportDowngrade,
                            &format!("rresht legacy PAS rreshti të vulosur (session={}) — fusha CRC e cunguar?", session_id));
                        return Err(ShadowError::TransportCorrupt(format!(
                            "PACP: DEGRADIM — sesioni {} ka histori të vulosur; legacy s'pranohet më (kufiri {:?})",
                            session_id, v)));
                    }
                    (crate::lab_contracts::pa_wire::PaWireVerdict::Corrupt(why), _) => {
                        // PSE-ja e plotë: dallohet cungimi (fusha mungojnë) nga
                        // kthimi i biteve (CRC s'përputhet) — diagnozë, jo mjegull.
                        let klasa = match why.contains("përputhet") {
                            true => crate::lab_contracts::rrjedha::RrjedhaClass::TransportBitflip,
                            false => {
                                crate::lab_contracts::rrjedha::RrjedhaClass::TransportTruncated
                            }
                        };
                        let _ = crate::lab_contracts::rrjedha::note(
                            &rrjedha_dir(),
                            "SHADOW",
                            "gateway::feed_primitive_anchors",
                            klasa,
                            &format!("{} (session={})", why, session_id),
                        );
                        return Err(ShadowError::TransportCorrupt(format!(
                            "PACP: {} (session={}) — transporti PA refuzohet",
                            why, session_id,
                        )));
                    }
                }
                // Hiq fushën CRC (nëse ekziston) — interpretimi vijues punon
                // mbi trupin kanonik 3/5-fushësh, i provuar tashmë nga gate.
                let body: &str = match line.rsplit_once("|c:") {
                    Some((b, _)) => b,
                    None => line,
                };
                let parts: Vec<&str> = body.split('|').collect();
                // 3 fusha (e vjetër, split bosh) OSE 5 (me Xi/Yi) — të dyja.
                let (sid, hex, split) = match parts.as_slice() {
                    [sid, hex, _ts] => (
                        *sid,
                        *hex,
                        PrimitiveSplit {
                            xi: Vec::new(),
                            yi: Vec::new(),
                        },
                    ),
                    [sid, hex, _ts, xi_seg, yi_seg] => {
                        match PrimitiveSplit::from_wire(xi_seg, yi_seg) {
                            Some(p) => (*sid, *hex, p),
                            None => {
                                return Err(ShadowError::TransportCorrupt(format!(
                                "Xi/Yi wire i keqformuar (session={}) — transporti PA refuzohet",
                                session_id,
                            )))
                            }
                        }
                    }
                    _ => {
                        return Err(ShadowError::TransportCorrupt(format!(
                            "format PA i papritur ({} fusha, session={}) — transporti PA refuzohet",
                            parts.len(),
                            session_id,
                        )))
                    }
                };
                match u64::from_str_radix(hex.trim(), 16) {
                    Ok(pa_id) => {
                        self.register_primitive_anchor(sid, pa_id, split);
                        Ok(1)
                    }
                    Err(_) => Err(ShadowError::TransportCorrupt(format!(
                        "pa_id hex i keqformuar '{}' (session={}) — transporti PA refuzohet",
                        hex.trim(),
                        session_id,
                    ))),
                }
            }
        }
    }

    /// Hyrje direkte VETËM PËR TEST. Production kalon nga main/process_bridge.
    ///
    /// LIGJI 0: asnjë klon i inputit. Në ngrirje → Err pa asnjë veprim.
    #[cfg(test)]
    pub fn ingest(
        &self,
        pkg: PassPackage,
        light: LightEnvelope,
    ) -> Result<ShadowResponse, ShadowError> {
        let parent_i0 = light.session_id.clone();
        let primitive_anchor = self
            .pa_waiting
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&parent_i0)
            .map(|entry| entry.0)
            .unwrap_or(0);
        let context = VerificationContext::direct(&parent_i0, &parent_i0, primitive_anchor);
        let decision = self.ingest_unsealed(
            pkg,
            light,
            GclPrimitiveVerification::direct_test(primitive_anchor),
            false,
            None,
        )?;
        let reinforcement = Self::prepare_gcl_reinforcement(&decision.verdict, primitive_anchor);
        self.commit_decision(decision, context, reinforcement, None)
            .map(|result| result.0)
    }

    /// Pipeline i brendshëm: vendos Y/X por nuk prodhon receipt publik.
    /// Përdoret nga rruga direkte dhe nga rruga bridged, që secila të mbyllë
    /// ankorën e vet para se output-i të vuloset.
    fn ingest_unsealed(
        &self,
        pkg: PassPackage,
        mut light: LightEnvelope,
        primitive_proof: GclPrimitiveVerification,
        witnessed_process_refusal: bool,
        current_ledger: Option<VerifiedCurrentLedgerScope>,
    ) -> Result<UnsealedShadowDecision, ShadowError> {
        match self.is_frozen() {
            true => {
                return Err(ShadowError::Frozen(
                    "Shadow është i ngrirë; asnjë vendim i ri nuk pranohet".to_string(),
                ))
            }
            false => {}
        }

        let ts = now_ns();
        let known_negative = match current_ledger.as_ref().and_then(|scope| scope.matrix.as_ref()) {
            // Diagnostic only; never used as the current NegativeContext write
            // proof below. V2 does not scan or increment unrelated history.
            Some(evidence) => evidence.diagnostic_context(),
            None => self.vault.check_known_negative(pkg.epistemic_mass, &light.territory),
        };
        let input_id = crate::shadow_pipeline::stable_id(&pkg.package_id);
        // Stage memory: vetëm një pipeline i suksesshëm e angazhon historinë.
        let runtime = match self.verification.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                eprintln!("[SHADOW_VERIFY] ALARM: verification runtime lock i helmuar — rikuperim fail-loud");
                poisoned.into_inner()
            }
        };
        let before_ledger = runtime.ledger_size() as u64;
        let before_wisdom = runtime.wisdom_count() as u64;
        let mut staged = runtime.clone();
        let attestation = crate::noi_caller::invoke("SHADOW_SPINE_VERIFY", &pkg.package_id, || Ok(match current_ledger.as_ref() {
            Some(scope) => {
                if !scope.matches(&pkg) {
                    return Err(ShadowError::SealInvalid("CURRENT_LEDGER_SCOPE_MISMATCH".into()));
                }
                match scope.model_case.as_ref() {
                    Some(case) => staged.adjudicate_case_runtime(input_id, &pkg, &light, primitive_proof, ts, case),
                    None => staged.adjudicate_current_runtime(input_id, &pkg, &light, primitive_proof, ts),
                }
            }
            None => crate::shadow_spine::ShadowSpine::adjudicate_runtime(
                &mut staged, input_id, &pkg, &light, primitive_proof, ts,
            ),
        }))?;
        let after_ledger = staged.ledger_size() as u64;
        let after_wisdom = staged.wisdom_count() as u64;
        let runtime_delta_digest = sha256_parts(&[
            b"ESSMAI_SHADOW_RUNTIME_DELTA_V1",
            &input_id.to_le_bytes(),
            &before_ledger.to_le_bytes(),
            &after_ledger.to_le_bytes(),
            &before_wisdom.to_le_bytes(),
            &after_wisdom.to_le_bytes(),
            &[attestation.final_bit, attestation.reason_code],
            &attestation.score.to_bits().to_le_bytes(),
        ]);
        let negative_proof = match primitive_proof.mode {
            GclPrimitiveMode::Refused => Some(NegativeContext {
                // A refused computation is not by itself a falsification of
                // the user proposition. Actual KiC/history evidence stays bound
                // separately in the native snapshot, without changing verdict.
                failure_reason_code: match witnessed_process_refusal {
                    true=>shadow_contracts::negative_asset::PROCESS_OBSERVATION_REASON,
                    false=>REASON_COLLAPSE_ELIM,
                },
                process: light.territory.clone(),
                mass_at_fail: pkg.epistemic_mass,
                frequency: 1,
            }),
            GclPrimitiveMode::Bound | GclPrimitiveMode::DirectTest => None,
            GclPrimitiveMode::LegacyCompatible => {
                return Err(ShadowError::StructuralRefusal(
                    StructuralRefusal::CompatibilityHold,
                ))
            }
        };
        let session_id = std::mem::take(&mut light.session_id);
        let SupremeCandidate {
            verdict,
            knowledge_write,
            lineage,
        } = run_pipeline(pkg, light, &session_id, &self.vault, attestation, negative_proof,
            current_ledger.as_ref().and_then(|scope| scope.matrix.as_ref()),
            current_ledger.as_ref().and_then(|scope| scope.model_case.as_ref()), ts)?;
        crate::shadow_runtime_pulse::mark_ready(crate::shadow_runtime_pulse::SStage::Judge);

        Ok(UnsealedShadowDecision {
            session_id,
            verdict,
            knowledge_writes: vec![knowledge_write],
            lineage,
            staged_runtime: staged,
            runtime_delta_digest,
            negative_batch_digest: [0u8; 32],
            parent_negative_staged: false,
            known_negative,
        })
    }

    /// Përgatit receipt-in, kryen durable commit, pastaj aplikon runtime delta.
    /// Receipt-i nuk publikohet kurrë përpara TX_COMMIT.
    fn commit_decision(
        &self,
        decision: UnsealedShadowDecision,
        context: VerificationContext,
        reinforcement: Option<GclReinforcement>,
        native_binding: Option<&crate::native_cycle::NativeRequestBinding>,
    ) -> Result<(ShadowResponse, DurableCommit), ShadowError> {
        match (
            context.session_id == decision.session_id,
            context.parent_i0.is_empty(),
        ) {
            (true, false) => {}
            (false, false) | (false, true) => {
                return Err(ShadowError::SealInvalid(
                    "verification context nuk i përket session-it të vendimit".to_string(),
                ))
            }
            (true, true) => {
                return Err(ShadowError::SealInvalid(
                    "verification context ka parent i₀ bosh".to_string(),
                ))
            }
        }
        let supreme = decision.verdict.as_supreme();
        let identity_digest = sha256_parts(&[
            b"ESSMAI_SHADOW_IDENTITY_V1",
            decision.session_id.as_bytes(),
            context.parent_i0.as_bytes(),
            &context.primitive_anchor.to_le_bytes(),
        ]);
        if let Some(binding)=native_binding {
            if binding.identity_digest!=identity_digest {
                return Err(ShadowError::SealInvalid("NATIVE_COMMIT_IDENTITY_MISMATCH".into()));
            }
        }
        let mut write_material = Vec::with_capacity(decision.knowledge_writes.len() * 32);
        decision
            .knowledge_writes
            .iter()
            .for_each(|write| write_material.extend_from_slice(&write.digest()));
        let cycle_digest = sha256_parts(&[
            b"ESSMAI_SHADOW_CYCLE_V1",
            &identity_digest,
            &context.xy_digest.to_le_bytes(),
            &context.pd_binding_digest.to_le_bytes(),
            &context.pd_continuum_activation_digest.to_le_bytes(),
            &supreme.living_trust_sha256,
            &decision.lineage.derivation.to_le_bytes(),
            &decision.negative_batch_digest,
            &decision.runtime_delta_digest,
            &context.vds_close_digest,
            &context.law0_digest,
            &write_material,
        ]);
        match self.vault.committed_transaction(&cycle_digest) {
            Some(commit) => {
                let response = ShadowResponse {
                    session_id: decision.session_id,
                    verdict: decision.verdict.into_supreme(),
                    known_negative: decision.known_negative,
                    verification_receipt: commit.receipt.clone(),
                    negative_persisted: decision.parent_negative_staged,
                };
                return Ok((response, commit));
            }
            None => {}
        }
        let receipt = seal_verified_output(
            &decision.session_id,
            &context.parent_i0,
            context.primitive_anchor,
            context.xy_digest,
            context.pd_binding_digest,
            context.pd_continuum_activation_digest,
            &decision.verdict,
        )
        .map_err(|error| {
            ShadowError::SealInvalid(format!("verification output seal dështoi: {error:?}"))
        })?;
        let tx = VaultTransaction::build(
            identity_digest,
            cycle_digest,
            context.anchor_generation,
            decision.negative_batch_digest,
            decision.runtime_delta_digest,
            context.vds_close_digest,
            context.law0_digest,
            &decision.verdict,
            decision.knowledge_writes,
            decision.lineage,
            reinforcement,
            receipt.clone(),
        );
        let tx=match native_binding {
            Some(binding)=>{
                let wire=crate::native_cycle::response_wire(ShadowResponse {
                    session_id:decision.session_id.clone(),
                    verdict:decision.verdict.as_supreme().clone(),
                    known_negative:decision.known_negative.clone(),
                    verification_receipt:receipt.clone(),
                    negative_persisted:decision.parent_negative_staged,
                });
                let snapshot=crate::native_cycle::NativeCycleSnapshot::new(binding,wire)
                    .map_err(ShadowError::VaultWriteError)?;
                tx.with_native_cycle(snapshot).map_err(|e|ShadowError::VaultWriteError(e.to_string()))?
            }
            None=>tx
        };
        let authorized = AuthorizedLiveTransaction {
            transaction: tx,
            originating_vault: &self.vault,
        };
        let commit = self.vault.commit_verified(authorized)?;
        *self
            .verification
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = decision.staged_runtime;
        crate::shadow_runtime_pulse::mark_ready(crate::shadow_runtime_pulse::SStage::Judge);
        let response = ShadowResponse {
            session_id: decision.session_id,
            verdict: decision.verdict.into_supreme(),
            known_negative: decision.known_negative,
            verification_receipt: receipt,
            negative_persisted: decision.parent_negative_staged,
        };
        Ok((response, commit))
    }


    /// A1 data-only staging gate. It converts an already validated parent
    /// asset into the existing KnowledgeWrite::Negative representation and
    /// never performs I/O, adjudication, or persistence.
    fn stage_parent_negative(
        verdict: crate::types::ConstitutionalVerdict,
        validated: Option<crate::negative_asset::ValidatedNegativeAsset>,
    ) -> Result<(Option<KnowledgeWrite>, [u8; 32], [u8; 32]), ShadowError> {
        let required = match (
            verdict,
            validated.as_ref().map(|asset| asset.experimental_failure),
        ) {
            (crate::types::ConstitutionalVerdict::RigorousNegative, Some(true))
            | (crate::types::ConstitutionalVerdict::RigorousNegative, Some(false))
            | (crate::types::ConstitutionalVerdict::RigorousNegative, None)
            | (crate::types::ConstitutionalVerdict::Constructive, Some(true)) => true,
            (crate::types::ConstitutionalVerdict::Constructive, Some(false))
            | (crate::types::ConstitutionalVerdict::Constructive, None) => false,
        };
        match (required, validated) {
            (true, Some(asset)) => {
                let parent = KnowledgeWrite::Negative {
                    input_id: asset.wire.input_id,
                    raw_bytes: asset.wire.bytes,
                    context: NegativeContext {
                        failure_reason_code: asset.wire.reason,
                        process: asset.wire.process,
                        mass_at_fail: asset.wire.mass,
                        frequency: asset.wire.frequency,
                    },
                    ts_ns: asset.wire.ts_ns,
                };
                let digest = parent.digest();
                Ok((Some(parent), digest, asset.token_sha256))
            }
            (true, None) => Err(ShadowError::SealInvalid(String::from(
                "Rigorous Negative verdict requires the validated parent NegativeAssetEnvelope",
            ))),
            (false, Some(_asset)) => Ok((None, [0u8; 32], [0u8; 32])),
            (false, None) => Ok((None, [0u8; 32], [0u8; 32])),
        }
    }

    fn combined_negative_batch_digest(
        parent_digest: [u8; 32],
        target_digest: [u8; 32],
        token_sha256: [u8; 32],
        target_count: u64,
    ) -> [u8; 32] {
        sha256_parts(&[
            b"ESSMAI_NEGATIVE_TRANSACTION_BATCH_V1",
            &parent_digest,
            &target_digest,
            &token_sha256,
            &target_count.to_le_bytes(),
        ])
    }

    fn assemble_negative_writes(
        parent: Option<KnowledgeWrite>,
        targets: Vec<KnowledgeWrite>,
        judge: Vec<KnowledgeWrite>,
    ) -> Vec<KnowledgeWrite> {
        let parent_count = match parent.as_ref() {
            Some(_write) => 1usize,
            None => 0usize,
        };
        let mut ordered = Vec::with_capacity(parent_count + targets.len() + judge.len());
        match parent {
            Some(write) => ordered.push(write),
            None => {}
        }
        ordered.extend(targets);
        ordered.extend(judge);
        ordered
    }

    fn prepare_gcl_reinforcement(
        verdict: &LawConfirmedVerdict,
        primitive_anchor: u64,
    ) -> Option<GclReinforcement> {
        match verdict.kind() {
            crate::types::ConstitutionalVerdict::Constructive => Some(GclReinforcement {
                primitive_anchor,
                hits: 1,
            }),
            crate::types::ConstitutionalVerdict::RigorousNegative => None,
        }
    }

    /// GCL LIVE: Y i verifikuar (D=1) → Trust(X) propagohet te ankora PA e
    /// sesionit në vault (real_hits+1, me kaskadë promovimi hipotezë→fakt).
    /// Thirret VETËM pas verdiktit — kurrë para (rendi Y→X është ligj).
    /// Jashtë zonës së ndaluar; vetëm konsumon verdiktin, s'e ndryshon atë.
    #[cfg(test)]
    fn gcl_reinforce_on_verified(&self, session_id: &str, verdict: &SupremeVerdict) {
        let anchor = match self.pa_waiting.lock() {
            Ok(map) => map.get(session_id).map(|(pa, _)| *pa),
            Err(poisoned) => poisoned.into_inner().get(session_id).map(|(pa, _)| *pa),
        };
        match anchor {
            Some(pa_id) => self.gcl_apply(session_id, pa_id, verdict),
            None => {} // s'ka ankorë NË PRITJE — normale për këtë rrugë;
                       // rruga e urës e mbart ankorën vetë (gcl_anchor).
        }
    }

    /// THELBI i lakut GCL — i vetmi vend ku Trust(X) propagohet realisht.
    /// Thirret nga DY rrugë, me GARANCI kundër numërimit të dyfishtë:
    ///   1. `ingest` (lookup në pa_waiting) — për thirrës direktë me feed;
    ///   2. `ingest_bridged` (ankora e mbartur) — pas verdiktit; aty ankora
    ///      tashmë ËSHTË HEQUR nga pa_waiting, prandaj rruga 1 është e
    ///      garantuar no-op → saktësisht NJË rifortësim për sesion. Zero if.
    #[cfg(test)]
    fn gcl_apply(&self, session_id: &str, pa_id: u64, verdict: &SupremeVerdict) {
        match verdict.is_verified() {
            true => {
                let promoted = self.vault.confirm_primitive(pa_id, 1);
                crate::shadow_runtime_pulse::mark_ready(
                    crate::shadow_runtime_pulse::SStage::GclApply,
                );
                println!(
                    "  [GCL_LIVE] Verified(Y)=1 → Trust(X): ankora i₀ (pa_id={:016x}, session={}) u rifortësua real_hits+1 (promovime={})",
                    pa_id, session_id, promoted
                );
            }
            false => {
                // Y s'u verifikua → X NUK merr besim; ankora mbetet e paprekur
                // (dega negative trajtohet nga judge_supreme/write_negative).
                println!(
                    "  [GCL_LIVE] Verified(Y)=0 → Trust(X) NUK propagohet (pa_id={:016x}, session={})",
                    pa_id, session_id
                );
            }
        }

        // ══ URA 8 — MONTIMI I VEKTORIT TRL (v1.4.6, audit p.2 PRANUAR) ══════
        // Dimensioni VERIFICATION nga SINJALI REAL i verdiktit: `legacy_score`
        // ∈[0,1] (evidence_density·logical_coherence·causal_integrity·
        // convergence_strength·reproducibility) — burimi i njëjtë familjar që
        // ushqen TrlVerdict.trl_score, tashmë i pranishëm në SupremeVerdict.
        // Proxy 0.90/0.30 u zëvendësua: kur Verified(Y)=1 marrim forcën REALE
        // të legacy_score; kur =0, verifikimi s'mbështetet → dysheme 0.30.
        // Zero if — match mbi (verified, a>0). Shadow mbetet i vetmi që sheh fundin.
        let ver_conf = match (verdict.is_verified(), verdict.legacy_score > 0.0) {
            (true, true) => verdict.legacy_score, // forca REALE e verifikimit
            (true, false) => 0.50_f32, // verified por pa sinjal legacy → TRL2 i kujdesshëm
            (false, _) => 0.30_f32,    // s'u verifikua → dysheme (TRL1)
        };
        let ver_lvl = crate::lab_contracts::trl::TrlVector::trl_from_confidence(ver_conf);
        println!(
            "  [TRL_VEKTOR] ver_conf={:.3} (legacy_score={:.3}, verified={}) → ver_lvl={}",
            ver_conf,
            verdict.legacy_score,
            verdict.is_verified(),
            ver_lvl
        );
        Self::shkruaj_trl_dim(session_id, "ver", ver_lvl);
        let prefiksi = format!("{}|", session_id);
        let (mut in_l, mut rsn_l, mut ver_l): (u8, u8, u8) = (0, 0, 0);
        match std::fs::read_to_string(format!(
            "{}/trl_vector.txt",
            rrjedha_dir().trim_end_matches(['/', '\\'])
        )) {
            Err(_) => {}
            Ok(c) => {
                for rreshti in c.lines().filter(|l| l.starts_with(prefiksi.as_str())) {
                    let trupi: &str = match crate::lab_contracts::pa_wire::verify_line_generic(
                        rreshti,
                        &[3],
                        4,
                    ) {
                        crate::lab_contracts::pa_wire::PaWireVerdict::SealedOk => {
                            match rreshti.rsplit_once("|c:") {
                                Some((b, _)) => b,
                                None => rreshti,
                            }
                        }
                        crate::lab_contracts::pa_wire::PaWireVerdict::LegacyOk => rreshti,
                        crate::lab_contracts::pa_wire::PaWireVerdict::Corrupt(_) => continue,
                    };
                    let pjeset: Vec<&str> = trupi.split('|').collect();
                    match pjeset.as_slice() {
                        [_sid, dim, val] => {
                            let lvl = match val.trim().parse::<u8>() {
                                Ok(v) => v,
                                Err(_) => continue,
                            };
                            match *dim {
                                "in" => {
                                    in_l = lvl;
                                }
                                "rsn" => {
                                    rsn_l = lvl;
                                }
                                "ver" => {
                                    ver_l = lvl;
                                }
                                _ => {}
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        let vektori = crate::lab_contracts::trl::TrlVector::new(in_l, rsn_l, ver_l);
        crate::shadow_runtime_pulse::mark_ready(crate::shadow_runtime_pulse::SStage::TrlVerdict);
        match vektori.is_complete() {
            true => println!(
                "  [TRL_VEKTOR] i PLOTË (session={}) — {} (deeptech: zinxhiri foli i tëri)",
                session_id,
                vektori.label()
            ),
            false => println!(
                "  [TRL_VEKTOR] i pjesshëm (session={}) — {} (dimension mungon — rendi i ndezjes)",
                session_id,
                vektori.label()
            ),
        }
    }

    /// URA 8 (v1.4.5): shkruan një dimension TRL të vulosur në trl_vector.txt.
    #[cfg(test)]
    fn shkruaj_trl_dim(session_id: &str, dim: &str, lvl: u8) {
        let body = format!("{}|{}|{}", session_id, dim, lvl);
        let line = crate::lab_contracts::pa_wire::seal_body(&body);
        use std::io::Write as _;
        let path = format!(
            "{}/trl_vector.txt",
            rrjedha_dir().trim_end_matches(['/', '\\'])
        );
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            Ok(mut f) => match f.write_all(line.as_bytes()).and_then(|_| f.flush()) {
                Ok(()) => {}
                Err(e) => eprintln!("  [TRL_VEKTOR] ALARM: dimensioni '{dim}' s'u shkrua ({e})"),
            },
            Err(e) => eprintln!("  [TRL_VEKTOR] ALARM: {} s'u hap ({e})", path),
        }
    }

    /// Provon që parent i₀ i PD-së është identiteti i lindur nga PA-ja që
    /// Shadow kishte në pritje. session_id është vetëm identitet cikli.
    fn parent_i0_matches_anchor(parent_i0: Option<&str>, anchor: Option<u64>) -> bool {
        match (parent_i0, anchor) {
            (Some(parent), Some(pa_id)) => {
                let expected = format!("i0-{pa_id:016x}");
                parent == expected.as_str()
            }
            _ => false,
        }
    }

    /// FAZA 3 — Hyrje me DY KANALE: adapton tipat kufitarë të urave
    /// (Quantum `QuantumInbound` + Light `LightInbound`) → tipat e brendshëm,
    /// pastaj i kalon te `ingest`. Origjinat mbeten të ndara deri brenda passage-it.
    pub fn ingest_bridged(
        &self,
        q: QuantumInbound,
        l: LightInbound,
    ) -> Result<ShadowResponse, ShadowError> {
        self.ingest_bridged_recorded(q,l,None)
    }

    /// Recover an exact committed request BEFORE PA consumption or judgment.
    /// Same identity with changed material is refused; no authority is replayed.
    pub(crate) fn replay_native_cycle(
        &self,binding:&crate::native_cycle::NativeRequestBinding
    )->Result<Option<Vec<u8>>,ShadowError> {
        match self.vault.committed_identity(&binding.identity_digest)? {
            None=>Ok(None),
            Some(commit)=>match commit.native_cycle {
                Some(snapshot) if snapshot.request_sha256==binding.request_sha256 && snapshot.prepared==binding.prepared =>Ok(Some(snapshot.response)),
                Some(_)=>Err(ShadowError::VaultWriteError("NATIVE_REQUEST_CONFLICT".into())),
                None=>Err(ShadowError::VaultWriteError("NATIVE_REPLAY_LEGACY_SNAPSHOT_MISSING".into())),
            }
        }
    }

    /// Read only an exact operational retry through the Shadow owner.
    pub(crate) fn replay_late_native_refusal(
        &self,
        attempt: &[u8],
        binding: &crate::native_cycle::NativeRequestBinding,
    ) -> Result<Option<shadow_contracts::pd_memory::PdOperationRecord>, ShadowError> {
        self.vault.replay_late_native_refusal(attempt, binding)
    }

    /// Preserve the owner admission, typed failure and current-head CAS checks.
    pub(crate) fn record_late_native_refusal(
        &self,
        attempt: &[u8],
        binding: &crate::native_cycle::NativeRequestBinding,
        observed: &ShadowError,
    ) -> Result<shadow_contracts::pd_memory::PdOperationRecord, ShadowError> {
        self.vault.record_late_native_refusal(attempt, binding, observed)
    }

    pub(crate) fn verify_native_kie(&self,request:&shadow_contracts::ShadowCycleRequest,handoff_root:&std::path::Path)->Result<Option<shadow_contracts::kie::KieProof>,String>{
        crate::kie_selection::verify_cycle(&self.vault,request,handoff_root)
    }
    pub(crate) fn validate_native_pd_admission(&self,binding:&crate::native_cycle::NativeRequestBinding)->Result<(),ShadowError>{
        self.vault.validate_native_pd_admission(binding)
    }
    pub(crate) fn ingest_bridged_recorded(
        &self,
        q: QuantumInbound,
        l: LightInbound,
        native_binding:Option<&crate::native_cycle::NativeRequestBinding>,
    ) -> Result<ShadowResponse, ShadowError> {
        self.ingest_bridged_recorded_with_scope(q, l, native_binding, BridgeEvidencePolicy::Legacy)
    }

    /// R23 process-boundary entry after final-evidence validation and exact
    /// CandidateLedger marker decoding. Legacy callers retain the method above.
    pub(crate) fn ingest_bridged_current_recorded(
        &self,
        q: QuantumInbound,
        l: LightInbound,
        native_binding: Option<&crate::native_cycle::NativeRequestBinding>,
    ) -> Result<ShadowResponse, ShadowError> {
        self.ingest_bridged_recorded_with_scope(q, l, native_binding, BridgeEvidencePolicy::CandidateV1)
    }

    /// Explicit V2 process entry. This does not reinterpret existing V1 cycles.
    pub(crate) fn ingest_bridged_scoped_recorded(
        &self, q: QuantumInbound, l: LightInbound,
        native_binding: Option<&crate::native_cycle::NativeRequestBinding>,
    ) -> Result<ShadowResponse, ShadowError> {
        self.ingest_bridged_recorded_with_scope(q, l, native_binding, BridgeEvidencePolicy::ScopedMatrixV2)
    }

    fn ingest_bridged_recorded_with_scope(
        &self,
        mut q: QuantumInbound,
        l: LightInbound,
        native_binding: Option<&crate::native_cycle::NativeRequestBinding>,
        evidence_policy: BridgeEvidencePolicy,
    ) -> Result<ShadowResponse, ShadowError> {
        // Heart-i HCP regjistrohet në HYRJEN AKTIVE të main/process bridge,
        // jo në një helper convenience pa caller production.
        crate::bridge::record_hcp_heart(&q);
        // RUNTIME HARDENING (      , fail-closed): transport Light i gjymtuar
        // REFUZOHET para ingest. Asnjë sukses i heshtur. Zero if — match.
        match l.is_valid() {
            true => {}
            false => {
                return Err(ShadowError::SealInvalid(format!(
                    "light_inbound i pavlefshëm: {}",
                    l.reason_invalid()
                )))
            }
        }

        // ══ KOLAPS/3 — VERIFIKIMI (formula: XY ↔ (Xi,Yi) → D ∈ {0,1}) ══════
        // Nxirren me MOVE (take — zero clone) fushat e kolapsit PARA se q të
        // konsumohet nga into_pass_package. Sjellje e vjetër e paprekur kur
        // fushat janë None/bosh (backward-safe si modeli HCP).
        let xy_x = q.xy_x.take();
        let xy_y = q.xy_y.take();
        let xy_mass = q.xy_mass.take();
        let pd_binding_digest = q.pd_binding_digest.take().unwrap_or(0);
        let pd_continuum_activation_digest = q.pd_continuum_activation_digest.take().unwrap_or(0);
        let pd_parent_i0 = q.pd_parent_i0.take();
        let final_evidence_digest = q.final_evidence_digest;
        let pd_gcl_process_digest = q.pd_gcl_process_digest;
        let spine_completion_digest = q.spine_completion_digest;
        let collapse_refused = q.collapse_refused.take();
        let negative_wire = std::mem::take(&mut q.negative_paths);
        let muscle_evidence = q.muscle_evidence.take();
        let validated_negative_asset = q.validated_negative_asset.take();
        match validated_negative_asset.as_ref() {
            Some(asset) => match asset.target_count == negative_wire.len() {
                true => {}
                false => return Err(ShadowError::SealInvalid(format!(
                    "Negative Knowledge parent/target count mismatch: parent={} paths={}",
                    asset.target_count,
                    negative_wire.len(),
                ))),
            },
            None => {}
        }
        let ingest_mass = q.epistemic_mass;
        let session_for_diary = q.session_id.clone();

        // ── MUSKULI verefied_diary_supremelaw — ditari strukturor i gjykimit.
        // GCL/AIGovern-Cyber aktivizon muskulin; ligjet/gjykatat vetëm i shërbejnë.
        // I njëjti i₀/PA, Xi/Yi dhe transformim Quantum futen si provë e tipizuar
        // në multi-verifikim — jo si kontroll paralel dhe jo si verdict i dytë.
        let directive = GjataCollapseLaw::issue(CollapsePhase::Verification);
        let flow = ShadowFlow::<AwaitingAnchor>::commission(directive)?;

        // PA-GATE: Light regjistroi herët PA + (Xi,Yi); propozimi i Quantum
        // e gjen dhe e konsumon me pronësi të vetme.
        let lease = AnchorLease::acquire(&self.pa_waiting, &q.session_id)?;
        let anchor = lease.material();
        let gcl_anchor: Option<u64> = lease.anchor_id();
        let negative_anchor_split = anchor.map(|material| &material.1);
        let parent_i0 = pd_parent_i0.as_deref().unwrap_or(&q.session_id);
        // Quantum e ndërton parent_i0 nga PA real: `i0-{pa_id:016x}`.
        // session_id identifikon ciklin dhe NUK duhet të shkrihet me i₀.
        let parent_i0_matches = Self::parent_i0_matches_anchor(pd_parent_i0.as_deref(), gcl_anchor);
        let transformation_traced = final_evidence_digest != 0
            && pd_binding_digest != 0
            && pd_continuum_activation_digest != 0
            && spine_completion_digest != 0;
        let gcl_continuity =
            pd_gcl_process_digest != 0 && parent_i0_matches && transformation_traced;
        let mut verification_context = VerificationContext::from_xy(
            &q.session_id,
            parent_i0,
            gcl_anchor.unwrap_or(0),
            xy_x.as_deref().unwrap_or(""),
            xy_y.as_deref().unwrap_or(""),
            xy_mass.unwrap_or(0),
            collapse_refused.as_deref().unwrap_or("ACCEPTED"),
            pd_binding_digest,
            pd_continuum_activation_digest,
            final_evidence_digest,
            pd_gcl_process_digest,
            spine_completion_digest,
            lease.generation(),
        );

        // Multi-verifikimi primitiv i v1.4.6, i përshtatur në kontratat e v1.6.9:
        //   i₀/PA → Xi/Yi → propozimi/refuzimi Quantum → procesi → struktura.
        // Partial XY është korrupsion; refuzimi i mbartur është propozim i ligjshëm
        // por final_bit=0. Rruga legacy ruhet e deklaruar, jo e maskuar si Bound.
        let (proof_mode, xi_verified, yi_verified, proposal_received) = match (
            anchor,
            &xy_x,
            &xy_y,
            &collapse_refused,
        ) {
            (Some((pa_id, split)), Some(x), Some(y), None) => match split.has_material() {
                true => {
                    let x_ok = split.xi.iter().any(|t| t == x);
                    let y_ok = split.yi.iter().any(|t| t == y);
                    match x_ok & y_ok {
                                true => {
                                    println!(
                                        "  [KOLAPS/3] session={} anchor={:016x} XY↔(Xi,Yi) PËRPUTHJE: ({} ↔ {}) mass_fixed={}",
                                        q.session_id, pa_id, x, y, xy_mass.unwrap_or(0));
                                    (GclPrimitiveMode::Bound, true, true, true)
                                }
                                false => return Err(ShadowError::SealInvalid(format!(
                                    "KOLAPS/3: XY=({},{}) JASHTË hapësirave të ankoruara (x∈Xi:{} y∈Yi:{}) — fail-closed",
                                    x, y, x_ok, y_ok))),
                            }
                }
                false => {
                    println!(
                                "  [KOLAPS/3] session={} anchor={:016x} pa material (Xi,Yi) → LEGACY_COMPATIBLE e deklaruar",
                                q.session_id, pa_id);
                    (GclPrimitiveMode::LegacyCompatible, false, false, true)
                }
            },
            (None, Some(x), Some(y), None) => {
                return Err(ShadowError::SealInvalid(format!(
                    "KOLAPS/3: XY=({},{}) mbërriti pa i₀/PA në pritje — fail-closed",
                    x, y
                )))
            }
            (Some((pa_id, _)), None, None, Some(reason)) => {
                println!(
                    "  [KOLAPS/3] session={} anchor={:016x} REFUZIM i mbartur nga Quantum: {}",
                    q.session_id, pa_id, reason
                );
                (GclPrimitiveMode::Refused, false, false, true)
            }
            (_, Some(_), None, _) | (_, None, Some(_), _) => {
                return Err(ShadowError::SealInvalid(
                    "KOLAPS/3: propozim XY i pjesshëm — X dhe Y duhet të vijnë bashkë".to_string(),
                ))
            }
            (None, None, None, Some(reason)) => {
                return Err(ShadowError::SealInvalid(format!(
                    "KOLAPS/3: refuzimi Quantum '{}' nuk ka i₀/PA në pritje",
                    reason
                )))
            }
            (Some((pa_id, _)), None, None, None) => {
                println!(
                        "  [SHADOW PA-GATE] session={} anchor={:016x} pa XY/refuzim → LEGACY_COMPATIBLE",
                        q.session_id, pa_id);
                (GclPrimitiveMode::LegacyCompatible, false, false, true)
            }
            (None, None, None, None) => (GclPrimitiveMode::LegacyCompatible, false, false, true),
            (_, Some(_), Some(_), Some(_)) => {
                return Err(ShadowError::SealInvalid(
                    "KOLAPS/3: Quantum dërgoi njëkohësisht XY dhe collapse_refused".to_string(),
                ))
            }
        };

        // Typestate ngre rendin ekzistues të VDS-së në tipa. Rruga legacy mban
        // vetëm dëshminë compatibility dhe nuk fiton asnjë autoritet të ri.
        let mut nbes_attestation = None;
        let mut nbes_store = None;
        let (process_flow, legacy_proof) = match proof_mode {
            GclPrimitiveMode::Bound | GclPrimitiveMode::Refused => {
                let proposal_flow = flow
                    .trust_anchor(&directive)?
                    .receive_proposal(&directive)?;
                let scope = crate::nbes_shadow::NbesScopeEvidence {
                    session_id: q.session_id.clone(),
                    primitive_anchor: gcl_anchor.unwrap_or(0),
                    muscle_evidence: muscle_evidence.ok_or_else(|| {
                        ShadowError::SealInvalid(
                            "Nbes muscle evidence missing after ProposalReceived".to_string(),
                        )
                    })?,
                };
                let command = proposal_flow.command_nbes(&directive, &scope)?;
                let handoff_root = std::env::var("ESSMAI_HANDOFF_DIR").map_err(|_| {
                    ShadowError::SealInvalid(
                        "ESSMAI_HANDOFF_DIR missing for Nbes hold verification".to_string(),
                    )
                })?;
                let store = crate::selection_hold::SelectionHoldStore::under_handoff(
                    handoff_root,
                );
                let attestation = crate::nbes_shadow::NbesShadow::from_command(command)
                    .commission(&directive)
                    .and_then(|nbes| nbes.verify_holds(&store, &scope, now_ns()))
                    .map_err(|reason| ShadowError::SealInvalid(format!(
                        "Nbes hold verification failed: {reason}",
                    )))?;
                let process = crate::noi_caller::invoke("SHADOW_NBES_PROCESS", &scope.session_id,
                    || Ok(proposal_flow.judge_process(&directive, &attestation, &scope)?))?;
                nbes_attestation = Some(attestation);
                nbes_store = Some(store);
                (Some(process), None)
            }
            GclPrimitiveMode::LegacyCompatible | GclPrimitiveMode::DirectTest => (
                None,
                Some(flow.attest_legacy_compatible()),
            ),
        };

        // DIJA NEGATIVE stage-ohet plotësisht; zero vault mutation para verdict-it.
        let neg_ts = now_ns();
        let kip3_proof=match native_binding.filter(|binding|!binding.kie_proof.is_empty()) {
            Some(binding)=>{
                let proof=shadow_contracts::kie::decode_proof(&binding.kie_proof).map_err(ShadowError::SealInvalid)?;
                crate::noi_caller::invoke("SHADOW_KIE_MODEL", &q.session_id,
                    || crate::kie_selection::validate_proof_model(&proof).map_err(ShadowError::SealInvalid))?;
                crate::noi_caller::invoke("SHADOW_KIE_PRESENTED_HISTORY", &q.session_id,
                    || self.vault.validate_kie_history(&proof).map_err(ShadowError::SealInvalid))?;
                proof.history_assessment.is_some().then_some(proof)
            }
            None=>None,
        };
        // Only a currently verified KIP3 proof can request the V2 projection.
        // V1 keeps the exact R23 interpretation; no historical receipt changes.
        let matrix = match evidence_policy {
            BridgeEvidencePolicy::ScopedMatrixV2 => {
                let proof = kip3_proof.as_ref().ok_or_else(||
                    ShadowError::SealInvalid("CURRENT_MATRIX_KIP3_REQUIRED".into()))?;
                Some(crate::noi_caller::invoke("SHADOW_MATRIX_EVIDENCE", &q.session_id,
                    || self.vault.presented_matrix_counterproofs(proof).map_err(ShadowError::SealInvalid))?)
            },
            _ => None,
        };
        let current_ledger = match evidence_policy {
            BridgeEvidencePolicy::CandidateV1 | BridgeEvidencePolicy::ScopedMatrixV2 => Some(VerifiedCurrentLedgerScope {
                input_id: crate::shadow_pipeline::stable_id(&q.package_id),
                final_evidence_digest: q.final_evidence_digest,
                matrix,
                model_case: native_binding.and_then(|binding| binding.model_case.clone()),
            }),
            BridgeEvidencePolicy::Legacy => None,
        };
        if let Some(case) = current_ledger.as_ref().and_then(|value| value.model_case.as_ref()) {
            let nbes = nbes_attestation.as_ref().ok_or_else(||
                ShadowError::SealInvalid("NBES_CASE_ATTESTATION_REQUIRED".into()))?;
            crate::noi_caller::invoke("SHADOW_NBES_CASE", &q.session_id, ||
                nbes.verify_case_report(case).map_err(ShadowError::SealInvalid))?;
        }
        let pending_negative=match (kip3_proof.as_ref(),validated_negative_asset.as_ref()) {
            (Some(proof),Some(asset)) if asset.project_id==0 && !asset.experimental_failure=>
                PendingNegativeBatch::stage_kie(&negative_wire,negative_anchor_split,
                    xy_x.as_deref(),xy_y.as_deref(),ingest_mass,neg_ts,proof,asset)?,
            _=>PendingNegativeBatch::stage(&negative_wire,negative_anchor_split,
                xy_x.as_deref(),xy_y.as_deref(),ingest_mass,neg_ts)?,
        };
        let negative_assets_held = pending_negative.is_complete_for(negative_wire.len());
        let target_batch_digest = pending_negative.digest();
        match negative_wire.is_empty() {
            false => println!(
                "  [KOLAPS/3] session={} negative_total={} staged={} structure_held={}",
                q.session_id,
                negative_wire.len(),
                pending_negative.count,
                negative_assets_held,
            ),
            true => {}
        }

        let mut held_flow = None;
        let primitive_proof: GclPrimitiveVerification = match proof_mode {
            GclPrimitiveMode::Bound | GclPrimitiveMode::Refused => {
                match negative_assets_held {
                    true => {}
                    false => return Err(ShadowError::SealInvalid(
                        "VDS StructureHeld refuzuar: asetet negative nuk u sistemuan plotësisht"
                            .to_string(),
                    )),
                }
                let process_flow = match process_flow {
                    Some(active) => active,
                    None => {
                        return Err(ShadowError::SealInvalid(String::from(
                            "ShadowFlow humbi gjendjen ProcessJudged",
                        )))
                    }
                };
                let structure_flow = process_flow.hold_structure(&directive)?;
                let proof = structure_flow.attest_structure(
                    proof_mode,
                    gcl_anchor.unwrap_or(0),
                    shadow_contracts::digest_parts(&[parent_i0.as_bytes()]),
                    verification_context.xy_digest,
                    pd_binding_digest,
                    pd_continuum_activation_digest,
                    gcl_anchor.is_some(),
                    parent_i0_matches,
                    xi_verified,
                    yi_verified,
                    proposal_received,
                    transformation_traced,
                    gcl_continuity,
                    nbes_attestation.as_ref().map(|value| value.bit()).unwrap_or(0) == 1,
                )?;
                held_flow = Some(structure_flow);
                proof
            }
            GclPrimitiveMode::LegacyCompatible | GclPrimitiveMode::DirectTest => {
                match legacy_proof {
                    Some(proof) => proof,
                    None => {
                        return Err(ShadowError::SealInvalid(String::from(
                            "ShadowFlow humbi dëshminë legacy-compatible",
                        )))
                    }
                }
            }
        };

        let witnessed_process_refusal = if let Some(binding) = native_binding {
            if binding.prepared.is_empty() { false } else {
                shadow_contracts::pd_prepared::decode_prepared(&binding.prepared)
                    .map_err(ShadowError::SealInvalid)?.failure_witness.is_some()
                    && collapse_refused.is_some()
            }
        } else { false };
        let mut decision = self.ingest_unsealed(
            q.into_pass_package(), l.into_envelope(), primitive_proof, witnessed_process_refusal,
            current_ledger)?;
        let model_write=match kip3_proof.as_ref() {
            Some(proof)=>crate::kie_history::conditional_write(proof,decision.verdict.kind(),
                decision.knowledge_writes.first()).map_err(ShadowError::SealInvalid)?,
            None=>None,
        };
        let (parent_write, parent_digest, parent_token) =
            Self::stage_parent_negative(decision.verdict.kind(), validated_negative_asset)?;
        decision.negative_batch_digest = Self::combined_negative_batch_digest(
            parent_digest,
            target_batch_digest,
            parent_token,
            pending_negative.count as u64,
        );
        decision.parent_negative_staged = parent_write.is_some();
        decision.knowledge_writes = Self::assemble_negative_writes(
            parent_write,
            pending_negative.into_writes(),
            std::mem::take(&mut decision.knowledge_writes),
        );
        match model_write {
            Some(write)=>{
                decision.negative_batch_digest=sha256_parts(&[b"ESSMAI/KIE/SCOPED-WRITE/1",
                    &decision.negative_batch_digest,&write.digest()]);
                decision.knowledge_writes.push(write);
            }
            None=>{},
        }

        let vds_diary_line = match (primitive_proof.mode, held_flow) {
            (
                GclPrimitiveMode::Bound | GclPrimitiveMode::Refused,
                Some(structure_flow),
            ) => structure_flow
                .diary_verdict(&directive)?
                .diary_line(&session_for_diary),
            (GclPrimitiveMode::LegacyCompatible, None) => {
                return Err(ShadowError::StructuralRefusal(
                    StructuralRefusal::CompatibilityHold,
                ))
            }
            (GclPrimitiveMode::DirectTest, None) => {
                return Err(ShadowError::SealInvalid(String::from(
                    "DirectTest nuk lejohet në ingest_bridged production",
                )))
            }
            (GclPrimitiveMode::Bound | GclPrimitiveMode::Refused, None) => {
                return Err(ShadowError::SealInvalid(String::from(
                    "ShadowFlow humbi StructureHeld para SealDiary",
                )))
            }
            (
                GclPrimitiveMode::LegacyCompatible | GclPrimitiveMode::DirectTest,
                Some(_unexpected_flow),
            ) => {
                return Err(ShadowError::SealInvalid(String::from(
                    "ShadowFlow krijoi StructureHeld për një proof mode jo-strukturor",
                )))
            }
        };
        let mut law0 = crate::lab_contracts::UncertaintyLedger::new();
        law0.record(
            crate::lab_contracts::CollapsePhase::Verification,
            "kolaps3_D",
            1.0,
            0.0,
        )
        .map_err(|violation| ShadowError::SealInvalid(violation.report()))?;
        verification_context.vds_close_digest =
            sha256_parts(&[b"ESSMAI_VDS_CLOSE_V1", vds_diary_line.as_bytes()]);
        let law0_report = law0.chain_report();
        verification_context.law0_digest =
            sha256_parts(&[b"ESSMAI_LAW0_CLOSE_V1", law0_report.as_bytes()]);
        let reinforcement =
            Self::prepare_gcl_reinforcement(&decision.verdict, gcl_anchor.unwrap_or(0));
        let (response, commit) =
            self.commit_decision(decision, verification_context, reinforcement, native_binding)?;
        let lease_evidence = lease.commit_consumption(commit.transaction_id)?;
        match (nbes_store.as_ref(), nbes_attestation.as_ref()) {
            (Some(store), Some(attestation)) => {
                let (besa_hold, asht_hold) = attestation.hold_ids();
                match store.cleanup_after_commit(&[besa_hold, asht_hold]) {
                    Ok(()) => {}
                    Err(error) => eprintln!(
                        "[NBES] cleanup defect after durable commit: {error}",
                    ),
                }
            }
            _ => {}
        }
        println!("  {}", vds_diary_line);
        println!(
            "  [PA_LEASE] generation={} state={:?} tx={:02x?}",
            lease_evidence.generation,
            lease_evidence.state,
            &lease_evidence.transaction_id[..4],
        );
        Ok(response)
    }

    /// FAZA 3 — Hyrje nga NJË burim Quantum: ndan paketën në (propozim, transport)
    /// si rrjedha reale Quantum→Light→Shadow, pastaj `ingest`.
    #[cfg(test)]
    pub fn ingest_quantum(&self, q: QuantumInbound) -> Result<ShadowResponse, ShadowError> {
        let (pkg, light) = crate::bridge::split_and_record_heart(q);
        self.ingest(pkg, light)
    }

    /// on_negative — pranon VNK nga Quantum Lab (P3: zinxhiri i plotë Lab→Shadow).
    /// Shadow është i vetmi autoritet që ruan njohuri negative persistente. Zero if.
    #[cfg(any(test, feature = "dev_harness"))]
    pub fn on_negative(
        &self,
        input_id: u64,
        reason: u32,
        process: &str,
        mass: f32,
        frequency: u32,
        bytes: &[u8],
        ts_ns: u64,
    ) -> Result<(), ShadowError> {
        crate::shadow_runtime_pulse::mark_ready(
            crate::shadow_runtime_pulse::SStage::NegativeVault,
        );
        self.vault.write_negative(
            input_id,
            bytes.to_vec(),
            NegativeContext {
                failure_reason_code: reason,
                process: process.to_string(),
                mass_at_fail: mass,
                frequency,
            },
            ts_ns,
        )
    }

    /// export_negative_knowledge — eksporton NK si bytes (P3: për PRO Vault).
    /// Format: count(4) | [len(4) | entry_bytes]*. Thirret nga Quantum pas cikli. Zero if.
    pub fn export_negative_knowledge(&self) -> Vec<u8> {
        let entries = self.vault.export_negative();
        let mut out = Vec::new();
        out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
        for e in &entries {
            let bytes = e.to_bytes();
            out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(&bytes);
        }
        out
    }

    /// Bounded read-only projection used exclusively by the mediated
    /// `--selection-once` port. The muscle never receives the vault handle.
    pub(crate) fn selection_candidates(
        &self,
        request: &shadow_contracts::KnowledgeSelectionRequestWire,
    ) -> Result<
        (
            Vec<shadow_contracts::CandidateStatisticWire>,
            Vec<shadow_contracts::CandidateStatisticWire>,
        ),
        ShadowError,
    > {
        self.vault
            .select_bounded_candidates(request)
    }

    /// KTHIM MIRROR (Shadow → Quantum/Light), DY origjina: kthen
    /// `ShadowLightResponse` (formë identike me pritjet e Quantum/Light).
    /// Varianti fallible; për variantin infallible shih `LightShadowBridge`.
    #[cfg(test)]
    pub fn respond_bridged(
        &self,
        pkg: QuantumInbound,
        light: LightInbound,
    ) -> Result<crate::bridge::ShadowLightResponse, ShadowError> {
        let r = self.ingest_bridged(pkg, light)?;
        Ok(crate::bridge::ShadowLightResponse::from_response(r))
    }

    /// KTHIM MIRROR nga NJË burim Quantum (Light derivohet brenda, si rrjedha reale).
    #[cfg(test)]
    pub fn respond_to_quantum(
        &self,
        pkg: QuantumInbound,
    ) -> Result<crate::bridge::ShadowLightResponse, ShadowError> {
        let r = self.ingest_quantum(pkg)?;
        Ok(crate::bridge::ShadowLightResponse::from_response(r))
    }

    // ── HYRJET NGA LIGHT (Light përgatit; Shadow vendos & ruan) ──────

    /// APUPK — pranon dhe RUAN njohuri projekti personale (`shadow_APUPK_memory`).
    /// Kthen `trace_id`; `Err` nëse paketa s'ka gjurmë/përmbajtje (Shadow vendos).
    pub fn receive_apupk(
        &self,
        pkg: crate::shadow_apupk::ApupkInbound,
    ) -> Result<u64, crate::shadow_apupk::ApupkReject> {
        crate::shadow_apupk::apupk_memory().store(pkg)
    }

    /// APUPK fail-closed për regjistrimin ndër-procesor të projektit.
    /// ProjectContextWitness nuk lejohet pa WAL të sinkronizuar në disk.
    pub fn receive_apupk_durable(
        &self,
        pkg: crate::shadow_apupk::ApupkInbound,
    ) -> Result<u64, crate::shadow_apupk::ApupkReject> {
        crate::shadow_apupk::apupk_memory().store_durable(pkg)
    }

    /// SNB — pranon dhe RUAN një raport bug-u nga Light (`shadow_snb`).
    /// Kthen dëftesën (nivel + frekuencë); `Err` nëse s'ka gjurmë rrjedhe/përshkrim.
    pub fn receive_bug_report(
        &self,
        report: crate::shadow_snb::BugInbound,
    ) -> Result<crate::shadow_snb::SnbReceipt, crate::shadow_snb::SnbReject> {
        crate::shadow_snb::snb_store().report(report)
    }

    /// Lehtësi: kthen vetëm verdiktin suprem.
    #[cfg(test)]
    pub fn ingest_verdict(
        &self,
        pkg: PassPackage,
        light: LightEnvelope,
    ) -> Result<SupremeVerdict, ShadowError> {
        self.ingest(pkg, light).map(|r| r.verdict)
    }

    // ── Ngrirja sovrane ─────────────────────────────────────────────────

    pub fn freeze(&self, stage: i32) {
        self.frozen.store(stage.max(1), Ordering::SeqCst);
        // RUNTIME HARDENING (      ): rezultati i kernelit bëhet eksplicit.
        // Gjendja lokale (frozen) ruhet sidoqoftë; kernel = shtresë e dytë.
        let _kernel_freeze = ShadowGjLegacy::freeze(stage);
    }

    pub fn unfreeze(&self) {
        self.frozen.store(0, Ordering::SeqCst);
        let _kernel_unfreeze = ShadowGjLegacy::unfreeze();
    }

    pub fn is_frozen(&self) -> bool {
        self.frozen.load(Ordering::SeqCst) != 0
    }

    // ── Maturimi & vëzhgueshmëria ───────────────────────────────────────

    /// Provon promovimin Primitive → Legacy (mbijetesa e zgjatur).
    pub fn run_maturation(&self) -> Result<u32, ShadowError> {
        if self.is_frozen() {
            return Err(ShadowError::Frozen("Shadow is frozen; maturation projection refused".into()));
        }
        let promoted = self.vault.run_maturation()?;
        crate::shadow_runtime_pulse::mark_ready(crate::shadow_runtime_pulse::SStage::Maturation);
        Ok(promoted)
    }

    pub fn stats(&self) -> ShadowStats {
        let k = ShadowGjLegacy::get_stats();
        let (verification_ledger_count, verification_wisdom_count) = match self.verification.lock()
        {
            Ok(v) => (v.ledger_size(), v.wisdom_count()),
            Err(poisoned) => {
                let v = poisoned.into_inner();
                (v.ledger_size(), v.wisdom_count())
            }
        };
        ShadowStats {
            total: k.total,
            passed: k.passed,
            blocked: k.blocked,
            frozen: self.is_frozen(),
            primitive_count: self.vault.primitive_count(),
            legacy_count: self.vault.legacy_count(),
            negative_count: self.vault.negative_count(),
            hypothesis_count: self.vault.hypothesis_count(),
            fact_count: self.vault.fact_count(),
            time_degraded: crate::shadow_pipeline::time_degraded(),
            verification_ledger_count,
            verification_wisdom_count,
        }
    }

    /// Akses i lexueshëm te arkivi sovran (për query/konfirmim maturimi).
    #[cfg(test)]
    pub fn vault(&self) -> &Arc<KnowledgeVault> {
        &self.vault
    }
}

// ════════════════════════════════════════════════════════════════════════════
// TESTE — URA 2: transporti i PA-ankorave nga skedari (Light → Shadow)
// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod pa_feed_tests {
    use super::*;

    fn tmp(name: &str) -> String {
        format!("{}/{}", std::env::temp_dir().display(), name)
    }


    #[test]
    fn r15_pa_file_retains_exact_split_and_rejects_changed_bytes_before_replacement() {
        let s = Shadow::new().expect("shadow init");
        let p = tmp("pa_feed_r15_exact.txt");
        let split = PrimitiveSplit {
            xi: vec!["jd_part2=54388.0,".into(), "a|b".into()],
            yi: vec!["provë~e\u{308}videncë".into()],
        };
        let line = crate::lab_contracts::pa_wire::encode_line_verified(
            "R15_EXACT", "00000000000000a4", 7, &split.to_wire(),
        ).expect("sealed exact text transport");
        std::fs::write(&p, &line).unwrap();
        assert_eq!(s.feed_primitive_anchors(&p, "R15_EXACT").unwrap(), 1);
        assert_eq!(s.pa_waiting.lock().unwrap().get("R15_EXACT").unwrap().1, split);
        let index = line.find("xi2:").unwrap() + 4 + 32;
        let mut changed = line.into_bytes();
        changed[index] = b'7';
        std::fs::write(&p, changed).unwrap();
        assert!(matches!(s.feed_primitive_anchors(&p, "R15_EXACT"),
            Err(ShadowError::TransportCorrupt(_))));
        assert_eq!(s.pa_waiting.lock().unwrap().get("R15_EXACT").unwrap().1, split);
        std::fs::remove_file(p).unwrap();
    }
    #[test]
    fn feed_missing_file_is_ok_zero_and_silent() {
        // KONTRATA: skedar që mungon = nisje e parë → Ok(0), pa alarm.
        let s = Shadow::new().expect("shadow init");
        assert_eq!(
            s.feed_primitive_anchors("nuk_ekziston_askund.txt", "S1")
                .expect("skedar që mungon duhet Ok(0), jo Err"),
            0
        );
    }

    #[test]
    fn feed_reads_last_anchor_of_session() {
        let s = Shadow::new().expect("shadow init");
        let p = tmp("pa_feed_last.txt");
        std::fs::write(
            &p,
            "S1|00000000000000aa|1\nS2|00000000000000bb|2\nS1|00000000000000cc|3\n",
        )
        .expect("write");
        // Dy rreshta S1 — regjistrohet i FUNDIT (0xcc); kthimi = Ok(1).
        assert_eq!(
            s.feed_primitive_anchors(&p, "S1").expect("feed i vlefshëm"),
            1
        );
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn feed_corruption_is_fail_closed_err() {
        // HARDENING: korrupsioni s'kapërcehet — Err(TransportCorrupt).
        let s = Shadow::new().expect("shadow init");
        let p = tmp("pa_feed_bad.txt");
        // (a) hex i pavlefshëm → Err me arsye 'hex'.
        std::fs::write(&p, "S1|jo_hex|9\n").expect("write");
        match s.feed_primitive_anchors(&p, "S1") {
            Err(ShadowError::TransportCorrupt(m)) => {
                assert!(m.contains("hex"), "arsyeja duhet të përmendë hex: {m}")
            }
            other => panic!("pritej TransportCorrupt(hex), erdhi {:?}", other),
        }
        // (b) numër fushash i papritur (2) → Err me arsye 'fusha'.
        std::fs::write(&p, "S1|deadbeef\n").expect("write");
        match s.feed_primitive_anchors(&p, "S1") {
            Err(ShadowError::TransportCorrupt(m)) => {
                assert!(m.contains("fusha"), "arsyeja duhet të përmendë fushat: {m}")
            }
            other => panic!("pritej TransportCorrupt(fusha), erdhi {:?}", other),
        }
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn pd_parent_i0_is_bound_to_the_waiting_primitive_anchor_not_session_id() {
        let pa = 0xA451u64;
        assert!(Shadow::parent_i0_matches_anchor(
            Some("i0-000000000000a451"),
            Some(pa),
        ));
        assert!(!Shadow::parent_i0_matches_anchor(
            Some("SESSION-1"),
            Some(pa)
        ));
        assert!(!Shadow::parent_i0_matches_anchor(None, Some(pa)));
        assert!(!Shadow::parent_i0_matches_anchor(
            Some("i0-000000000000a452"),
            Some(pa),
        ));
    }

    #[test]
    fn feed_contract_matches_light_writer_format() {
        // Kontrata e RE (5 fusha): "sid|{:016x}|ts|xi:...|yi:..." — si Light shkruan.
        let s = Shadow::new().expect("shadow init");
        let p = tmp("pa_feed_contract.txt");
        let line = format!(
            "SES_X|{:016x}|123456789|xi:flamur,kuq|yi:shqiponje\n",
            0xA451u64
        );
        std::fs::write(&p, line).expect("write");
        assert_eq!(
            s.feed_primitive_anchors(&p, "SES_X")
                .expect("kontrata 5-fusha e vlefshme"),
            1
        );
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn feed_old_three_field_still_accepted_backward() {
        // Rreshtat e vjetër 3-fushësh pranohen (split bosh) — pas-pajtueshmëri.
        let s = Shadow::new().expect("shadow init");
        let p = tmp("pa_feed_old3.txt");
        std::fs::write(&p, "SES_O|00000000000000ff|9\n").expect("write");
        assert_eq!(
            s.feed_primitive_anchors(&p, "SES_O")
                .expect("formati i vjetër 3-fusha"),
            1
        );
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn gcl_live_reinforce_only_on_verified_y() {
        // GCL LIVE: rendi i ligjit — Verified(Y)=1 → Trust(X) te ankora;
        // Verified(Y)=0 → asnjë propagim. Sinjal real, zero simulim.
        let s = Shadow::new().expect("shadow init");
        let pa: u64 = 0xA451;
        // Ankora e sesionit pritet në gate (siç e sjell Light në runtime).
        s.register_primitive_anchor(
            "SES_GCL",
            pa,
            PrimitiveSplit {
                xi: vec!["flamur".into()],
                yi: vec!["shqiponje".into()],
            },
        );
        // Primitivi ekziston në vault (siç e shkruan judge_supreme në D=1) —
        // firma REALE 9-argumentëshe, si në testet e vetë vault-it.
        s.vault
            .write_primitive(
                pa,
                0.9,
                0.9,
                0.9,
                0xA451,
                crate::types::ShadowPath::Fast,
                "GCL_LIVE_TEST".to_string(),
                b"gcl-live".to_vec(),
                1,
            )
            .expect("write_primitive për testin GCL");
        let hits_para = s.vault.primitive_real_hits(pa);

        // (a) Y JO i verifikuar → Trust(X) NUK propagohet.
        let v0 = SupremeVerdict {
            verified: 0,
            primitive: 0,
            knowledge_band: crate::types::KnowledgeBand::Hypothesis,
            lgc_law: 0,
            legacy_score: 0.0,
            lgc_dsl: String::new(),
            living_trust_sha256: [0u8; 32],
            living_trust_intensity: 0,
            living_trust_kind: 0,
            living_trust_action_state: 0,
            living_trust_action_mask: 0,
            living_trust_required_action_mask: 0,
            living_trust_system_laws_seal: 0,
            living_trust_sovereign_flags: 0,
            living_trust_sovereign_value: 0,

            project_id: 0,
            project_status: shadow_contracts::PROJECT_STATUS_NONE,
            project_context_digest: 0,
            project_evidence_digest: 0,
            project_context_sha256: [0u8; 32],
            project_evidence_sha256: [0u8; 32],
            novel_factualized: false,
            novel_trl_level: 0,
            novel_proof_score: 0,
            novel_rejection: 0,
        };
        s.gcl_reinforce_on_verified("SES_GCL", &v0);
        assert_eq!(
            s.vault.primitive_real_hits(pa),
            hits_para,
            "Y=0 s'duhet të rrisë real_hits"
        );

        // (b) Y i verifikuar → Trust(X) propagohet: real_hits+1.
        let v1 = SupremeVerdict {
            verified: 1,
            primitive: 1,
            knowledge_band: crate::types::KnowledgeBand::Hypothesis,
            lgc_law: 0,
            legacy_score: 0.0,
            lgc_dsl: String::new(),
            living_trust_sha256: [0u8; 32],
            living_trust_intensity: 0,
            living_trust_kind: 0,
            living_trust_action_state: 0,
            living_trust_action_mask: 0,
            living_trust_required_action_mask: 0,
            living_trust_system_laws_seal: 0,
            living_trust_sovereign_flags: 0,
            living_trust_sovereign_value: 0,

            project_id: 0,
            project_status: shadow_contracts::PROJECT_STATUS_NONE,
            project_context_digest: 0,
            project_evidence_digest: 0,
            project_context_sha256: [0u8; 32],
            project_evidence_sha256: [0u8; 32],
            novel_factualized: false,
            novel_trl_level: 0,
            novel_proof_score: 0,
            novel_rejection: 0,
        };
        s.gcl_reinforce_on_verified("SES_GCL", &v1);
        assert_eq!(
            s.vault.primitive_real_hits(pa),
            hits_para + 1,
            "Y=1 duhet të rrisë real_hits me +1"
        );
    }

    #[test]
    fn verification_trl_derives_from_real_legacy_score() {
        // Sjellja (audit v1.4.5 p.2): ver_conf = legacy_score REAL kur verified,
        // jo proxy 0.90. Provohet ligji i mapimit trl_from_confidence mbi vlerat
        // reale që do të mbante SupremeVerdict.legacy_score.
        use crate::lab_contracts::trl::TrlVector;
        // legacy_score i lartë (0.92) → TRL4; i mesëm (0.60) → TRL2.
        assert_eq!(
            TrlVector::trl_from_confidence(0.92),
            4,
            "legacy_score i fortë → verification_trl 4"
        );
        assert_eq!(
            TrlVector::trl_from_confidence(0.60),
            2,
            "legacy_score i mesëm → verification_trl 2"
        );
        // Dysheme kur verified pa sinjal (0.50→TRL2) dhe kur s'verifikohet (0.30→TRL1).
        assert_eq!(TrlVector::trl_from_confidence(0.50), 2);
        assert_eq!(TrlVector::trl_from_confidence(0.30), 1);
        // Ligji i degës: verifikuar me legacy>0 → merr legacy; verifikuar pa
        // legacy → 0.50; i paverifikuar → 0.30 (i njëjti match si në gcl_apply).
        let deg = |verified: bool, ls: f32| -> f32 {
            match (verified, ls > 0.0) {
                (true, true) => ls,
                (true, false) => 0.50,
                (false, _) => 0.30,
            }
        };
        assert_eq!(deg(true, 0.88), 0.88, "verifikuar → forca reale");
        assert_eq!(deg(true, 0.0), 0.50, "verifikuar pa sinjal → kujdes");
        assert_eq!(
            deg(false, 0.99),
            0.30,
            "s'u verifikua → dysheme, injoron legacy"
        );
    }

    #[test]
    fn feed_sealed_crc_line_is_ok_and_tamper_is_err() {
        // PACP end-to-end në urë: encode (Light) → verify+feed (Shadow).
        let s = Shadow::new().expect("shadow init");
        let p = tmp("pa_feed_pacp.txt");
        let line = crate::lab_contracts::pa_wire::encode_line(
            "SES_C",
            "00000000000000a4",
            987654321,
            "xi:flamur,kuq|yi:shqiponje",
        );
        std::fs::write(&p, &line).expect("write");
        assert_eq!(
            s.feed_primitive_anchors(&p, "SES_C")
                .expect("ankora e vulosur CRC-OK"),
            1
        );
        // Tamper: një bit brenda trupit → CRC s'përputhet → TransportCorrupt.
        let tampered = line.replace("kuq", "kur");
        std::fs::write(&p, &tampered).expect("write");
        match s.feed_primitive_anchors(&p, "SES_C") {
            Err(ShadowError::TransportCorrupt(m)) => {
                assert!(m.contains("PACP"), "arsyeja duhet të mbajë vulën PACP: {m}")
            }
            other => panic!("pritej TransportCorrupt(PACP), erdhi {:?}", other),
        }
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn feed_five_field_bad_wire_is_fail_closed_err() {
        // HARDENING: 5 fusha por wire pa prefikset xi:/yi: → Err (jo kapërcim).
        let s = Shadow::new().expect("shadow init");
        let p = tmp("pa_feed_badwire.txt");
        std::fs::write(&p, "SES_B|00000000000000aa|9|flamur|shqiponje\n").expect("write");
        match s.feed_primitive_anchors(&p, "SES_B") {
            Err(ShadowError::TransportCorrupt(m)) => {
                assert!(m.contains("wire"), "arsyeja duhet të përmendë wire: {m}")
            }
            other => panic!("pritej TransportCorrupt(wire), erdhi {:?}", other),
        }
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn gcl_bridged_path_reinforces_after_anchor_removed() {
        // REGRESIONI I GJETJES KURORË (v1.3.2): rruga e urës HEQ ankorën nga
        // pa_waiting PARA verdiktit (KOLAPS/3) — laku GCL duhet të mbyllet
        // NJËSOJ me ankorën e mbartur (gcl_apply), jo me lookup që gjen None.
        let s = Shadow::new().expect("shadow init");
        let pa: u64 = 0xB452;
        s.register_primitive_anchor(
            "SES_BRG",
            pa,
            PrimitiveSplit {
                xi: vec!["flamur".into()],
                yi: vec!["shqiponje".into()],
            },
        );
        s.vault
            .write_primitive(
                pa,
                0.9,
                0.9,
                0.9,
                0xB452,
                crate::types::ShadowPath::Fast,
                "GCL_BRIDGED_TEST".to_string(),
                b"gcl-bridged".to_vec(),
                1,
            )
            .expect("write_primitive për testin e urës");
        let hits_para = s.vault.primitive_real_hits(pa);

        // Simulohet EKZAKTËSISHT rrjedha e urës: (1) ankora HIQET me pronësi…
        let e_hequr = match s.pa_waiting.lock() {
            Ok(mut m) => m.remove("SES_BRG"),
            Err(poisoned) => poisoned.into_inner().remove("SES_BRG"),
        };
        let gcl_anchor = e_hequr.as_ref().map(|(p, _)| *p);
        assert_eq!(gcl_anchor, Some(pa), "ankora duhej hequr me pronësi");

        // (2) …lookup-i i vjetër tani gjen None (prova e vdekjes së lakut të vjetër)…
        let v1 = SupremeVerdict {
            verified: 1,
            primitive: 1,
            knowledge_band: crate::types::KnowledgeBand::Hypothesis,
            lgc_law: 0,
            legacy_score: 0.0,
            lgc_dsl: String::new(),
            living_trust_sha256: [0u8; 32],
            living_trust_intensity: 0,
            living_trust_kind: 0,
            living_trust_action_state: 0,
            living_trust_action_mask: 0,
            living_trust_required_action_mask: 0,
            living_trust_system_laws_seal: 0,
            living_trust_sovereign_flags: 0,
            living_trust_sovereign_value: 0,

            project_id: 0,
            project_status: shadow_contracts::PROJECT_STATUS_NONE,
            project_context_digest: 0,
            project_evidence_digest: 0,
            project_context_sha256: [0u8; 32],
            project_evidence_sha256: [0u8; 32],
            novel_factualized: false,
            novel_trl_level: 0,
            novel_proof_score: 0,
            novel_rejection: 0,
        };
        s.gcl_reinforce_on_verified("SES_BRG", &v1);
        assert_eq!(
            s.vault.primitive_real_hits(pa),
            hits_para,
            "lookup pas heqjes duhet të jetë no-op — kjo ISHTE gjetja"
        );

        // (3) …dhe rruga e re (ankora e mbartur) e mbyll lakun saktësisht një herë.
        match gcl_anchor {
            Some(p) => s.gcl_apply("SES_BRG", p, &v1),
            None => panic!("ankora e mbartur mungon"),
        }
        assert_eq!(
            s.vault.primitive_real_hits(pa),
            hits_para + 1,
            "ankora e mbartur duhet të rifortësojë real_hits+1 në rrugën e urës"
        );
    }

    #[test]
    fn legacy_after_sealed_is_downgrade_refused() {
        // ÇELËSI NJËDREJTIMËSH: i vulosur ⇒ përgjithmonë i vulosur. Rresht
        // legacy PAS një rreshti me CRC për të njëjtin sesion = degradim
        // (cungim i c:) → refuzim i argumentuar, i shënuar në rrjedha.
        let s = Shadow::new().expect("shadow init");
        let p = tmp("pa_feed_downgrade.txt");
        let sealed = crate::lab_contracts::pa_wire::encode_line(
            "SES_D",
            "00000000000000d4",
            111,
            "xi:flamur,kuq|yi:shqiponje",
        );
        let legacy = "SES_D|00000000000000d4|222|xi:flamur,kuq|yi:shqiponje\n";
        std::fs::write(&p, format!("{}{}", sealed, legacy)).expect("write");
        match s.feed_primitive_anchors(&p, "SES_D") {
            Err(ShadowError::TransportCorrupt(m)) => assert!(
                m.contains("DEGRADIM"),
                "arsyeja duhet të deklarojë degradimin: {m}"
            ),
            other => panic!("pritej TransportCorrupt(DEGRADIM), erdhi {:?}", other),
        }
        // Ndërsa sesion PA histori të vulosur pranon legacy të deklaruar (pas-pajtueshmëria e paprekur).
        let p2 = tmp("pa_feed_pure_legacy.txt");
        std::fs::write(&p2, "SES_L|00000000000000e5|333|xi:flamur|yi:shqiponje\n").expect("write");
        assert_eq!(
            s.feed_primitive_anchors(&p2, "SES_L")
                .expect("legacy i pastër pranohet"),
            1
        );
        let _ = std::fs::remove_file(&p);
        let _ = std::fs::remove_file(&p2);
    }
}

#[cfg(test)]
mod a1_single_negative_commit_tests {
    use super::*;
    use crate::negative_asset::ValidatedNegativeAsset;
    use crate::types::ConstitutionalVerdict;
    use shadow_contracts::NegativeKnowledgeWire;

    fn validated_parent(experimental_failure: bool, input_id: u64) -> ValidatedNegativeAsset {
        ValidatedNegativeAsset {
            wire: NegativeKnowledgeWire {
                input_id,
                reason: 0xA451,
                process: "GCL_NEGATIVE_ASSET_SHA256=test".to_string(),
                mass: 0.75,
                frequency: 2,
                suggestion_code: 0,
                bytes: vec![1, 2, 3, 4, 5],
                ts_ns: 77,
            },
            token_sha256: [0x33; 32],
            token_hex: "33".repeat(32),
            experimental_failure,
            target_count: 2,
            project_id: 9,
            trl_level: 3,
        }
    }

    fn negative_write(input_id: u64, marker: u8) -> KnowledgeWrite {
        KnowledgeWrite::Negative {
            input_id,
            raw_bytes: vec![marker],
            context: NegativeContext {
                failure_reason_code: u32::from(marker),
                process: format!("test-{marker}"),
                mass_at_fail: 0.5,
                frequency: 1,
            },
            ts_ns: u64::from(marker),
        }
    }

    fn write_input_id(write: &KnowledgeWrite) -> u64 {
        match write {
            KnowledgeWrite::Primitive {
                input_id,
                vector_points: _,
                temporal_score: _,
                mass: _,
                seal: _,
                path: _,
                domain: _,
                raw_bytes: _,
                ts_ns: _,
            }
            | KnowledgeWrite::Negative {
                input_id,
                raw_bytes: _,
                context: _,
                ts_ns: _,
            } => *input_id,
        }
    }

    #[test]
    fn parent_asset_is_moved_into_existing_negative_write_without_payload_copy() {
        let asset = validated_parent(false, 101);
        let original_ptr = asset.wire.bytes.as_ptr();
        let staged = Shadow::stage_parent_negative(
            ConstitutionalVerdict::RigorousNegative,
            Some(asset),
        )
        .expect("validated parent must stage");
        match staged.0 {
            Some(KnowledgeWrite::Negative {
                input_id,
                raw_bytes,
                context,
                ts_ns,
            }) => {
                assert_eq!(input_id, 101);
                assert_eq!(raw_bytes.as_ptr(), original_ptr);
                assert_eq!(raw_bytes, vec![1, 2, 3, 4, 5]);
                assert_eq!(context.failure_reason_code, 0xA451);
                assert_eq!(context.frequency, 2);
                assert_eq!(ts_ns, 77);
            }
            Some(KnowledgeWrite::Primitive {
                input_id,
                vector_points,
                temporal_score,
                mass,
                seal,
                path,
                domain,
                raw_bytes,
                ts_ns,
            }) => {
                let unexpected = (
                    input_id,
                    vector_points,
                    temporal_score,
                    mass,
                    seal,
                    path,
                    domain,
                    raw_bytes,
                    ts_ns,
                );
                panic!("parent staged as primitive: {unexpected:?}");
            }
            None => panic!("required parent was not staged"),
        }
    }

    #[test]
    fn rigorous_negative_without_parent_is_rejected_before_transaction_build() {
        let result = Shadow::stage_parent_negative(
            ConstitutionalVerdict::RigorousNegative,
            None,
        );
        match result {
            Err(ShadowError::SealInvalid(message)) => {
                assert!(message.contains("requires the validated parent"));
            }
            Err(other) => panic!("unexpected error: {other:?}"),
            Ok(staged) => panic!("missing parent was accepted: {staged:?}"),
        }
    }

    #[test]
    fn experimental_parent_is_required_even_for_constructive_verdict() {
        let result = Shadow::stage_parent_negative(
            ConstitutionalVerdict::Constructive,
            Some(validated_parent(true, 202)),
        )
        .expect("experimental parent must stage");
        assert!(result.0.is_some());
    }

    #[test]
    fn non_experimental_constructive_parent_is_not_persisted() {
        let result = Shadow::stage_parent_negative(
            ConstitutionalVerdict::Constructive,
            Some(validated_parent(false, 303)),
        )
        .expect("constructive classification");
        assert!(result.0.is_none());
    }

    #[test]
    fn combined_negative_batch_digest_is_deterministic_and_binds_all_material() {
        let baseline = Shadow::combined_negative_batch_digest(
            [1; 32],
            [2; 32],
            [3; 32],
            4,
        );
        assert_eq!(
            baseline,
            Shadow::combined_negative_batch_digest([1; 32], [2; 32], [3; 32], 4),
        );
        assert_ne!(
            baseline,
            Shadow::combined_negative_batch_digest([9; 32], [2; 32], [3; 32], 4),
        );
        assert_ne!(
            baseline,
            Shadow::combined_negative_batch_digest([1; 32], [9; 32], [3; 32], 4),
        );
        assert_ne!(
            baseline,
            Shadow::combined_negative_batch_digest([1; 32], [2; 32], [9; 32], 4),
        );
        assert_ne!(
            baseline,
            Shadow::combined_negative_batch_digest([1; 32], [2; 32], [3; 32], 9),
        );
    }

    #[test]
    fn write_set_order_is_parent_then_targets_then_judge() {
        let ordered = Shadow::assemble_negative_writes(
            Some(negative_write(1, 1)),
            vec![negative_write(2, 2), negative_write(3, 3)],
            vec![negative_write(4, 4)],
        );
        let ids: Vec<u64> = ordered.iter().map(write_input_id).collect();
        assert_eq!(ids, vec![1, 2, 3, 4]);
    }
}

#[cfg(test)]
mod live_write_boundary_tests {
    use super::*;

    #[test]
    fn r9_authorization_cannot_move_to_a_different_vault() {
        let origin = KnowledgeVault::new();
        let other = KnowledgeVault::new();
        // A harmless storage fixture tests ownership only; it is not evidence
        // of a computed verdict. Production cannot construct this wrapper here.
        let authorized = AuthorizedLiveTransaction {
            transaction: crate::vault_transaction::test_support::sample_transaction(),
            originating_vault: &origin,
        };
        let error = other.commit_verified(authorized).expect_err("foreign Vault must refuse");
        assert!(error.to_string().contains("LIVE_COMMIT_WRONG_VAULT"));
        assert_eq!(origin.store_counts_for_test(), [0; 7]);
        assert_eq!(other.store_counts_for_test(), [0; 7]);
    }

    #[test]
    fn r9_authorization_does_not_skip_transaction_validation() {
        let origin = KnowledgeVault::new();
        let mut transaction = crate::vault_transaction::test_support::sample_transaction();
        transaction.receipt.parent_i0.push_str("-changed");
        let authorized = AuthorizedLiveTransaction { transaction, originating_vault: &origin };
        assert!(origin.commit_verified(authorized).is_err());
        assert_eq!(origin.store_counts_for_test(), [0; 7]);
    }
}

#[cfg(test)]
mod r36_maturation_gateway_tests {
    use super::*;
    #[test]
    fn r36_frozen_gateway_refuses_manual_maturation() {
        let shadow=Shadow {
            vault:Arc::new(KnowledgeVault::new()), frozen:AtomicI32::new(1),
            pa_waiting:Mutex::new(HashMap::new()),
            verification:Mutex::new(crate::shadow_spine::ShadowVerificationRuntime::new()),
        };
        assert!(matches!(shadow.run_maturation(),Err(ShadowError::Frozen(_))));
        assert_eq!(shadow.vault.store_counts_for_test(),[0;7]);
        assert_eq!(shadow.vault.observer.birth_count(),0);
    }
}

#[cfg(test)]
mod resident_request_tests {
    use super::*;
    #[test]
    fn r37_resident_wrappers_share_only_vault_and_keep_request_state_private() {
        let vault=Arc::new(KnowledgeVault::new());
        let first=Shadow::with_resident_vault(vault.clone());
        first.register_primitive_anchor("prior-request",17,PrimitiveSplit {xi:vec!["a".into()],yi:vec!["b".into()]});
        first.frozen.store(1,Ordering::SeqCst); // local fixture; do not change the global C kernel.
        let second=Shadow::with_resident_vault(vault.clone());
        assert!(Arc::ptr_eq(&first.resident_vault(),&second.resident_vault()));
        assert!(first.is_frozen());assert!(!second.is_frozen());
        assert_eq!(first.pa_waiting.lock().unwrap().len(),1);
        assert!(second.pa_waiting.lock().unwrap().is_empty());
        assert!(!std::ptr::eq(&first.verification,&second.verification));
        assert_eq!((second.stats().verification_ledger_count,second.stats().verification_wisdom_count),(0,0));
        // Storage fixture, not an admission shortcut: both wrappers observe
        // exactly the same subsequent projection mutation.
        vault.write_negative(17,b"resident exact row".to_vec(),crate::types::NegativeContext {
            failure_reason_code:7,process:"resident test".into(),mass_at_fail:0.2,frequency:1},1).unwrap();
        assert_eq!((first.stats().negative_count,second.stats().negative_count),(1,1));
    }
}
