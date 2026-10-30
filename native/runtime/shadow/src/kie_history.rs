//! R21: immutable transaction provenance projected into scoped KiE history.
//! This index contains references and classifications, never a second authority.
//! KIP2 keeps its original interpretation; only new KIP3 consumes this projection.
use crate::{shadow_commit::KnowledgeWrite, vault_transaction::VaultTransaction};
use shadow_contracts::{kie::*, negative_asset::*, pd_memory, CandidateStatisticWire};
use std::collections::BTreeMap;

type Key = (u8, u64, String, String, String);
const MODEL_DOMAIN: &str="KIE/MODEL-CONDITIONAL/";

fn model_domain(proof:&KieProof,polarity:u8)->Result<String,String> {
    let model=science_domains::decode_candidate(&proof.candidate_bytes).map_err(|e|format!("KIE_MODEL_DOMAIN:{e}"))?;
    match polarity {
        // Context, parameters and model family are provenance, not independent
        // scientific domains. Do not manufacture canonical legacy maturation.
        1=>{
            let adapter=match model.query.adapter {
                science_domains::AdapterId::Physics=>"physics",
                science_domains::AdapterId::Astronomy=>"astronomy",
                science_domains::AdapterId::History=>"history",
                science_domains::AdapterId::SocialScience=>"social-science",
                science_domains::AdapterId::Psychology=>"psychology",
            };
            Ok(format!("{MODEL_DOMAIN}1/{adapter}"))
        }
        0=>Ok(format!("{MODEL_DOMAIN}0/{}/{}",pd_memory::hex(&proof.context.identity.key()?),
            pd_memory::hex(&crate::native_cycle::kie_family(&model)))),
        _=>Err("KIE_MODEL_POLARITY".into()),
    }
}

/// The source judge payload is a serialized Quantum state, not the proposition.
/// Retain it unchanged and derive a separate scoped conditional object only
/// AFTER the existing sovereign pipeline has actually produced this verdict.
pub(crate) fn conditional_write(proof:&KieProof,verdict:crate::types::ConstitutionalVerdict,
    judge:Option<&KnowledgeWrite>)->Result<Option<KnowledgeWrite>,String> {
    crate::kie_selection::validate_proof_model(proof)?;
    match proof.history_assessment.is_some() {true=>{},false=>return Ok(None)}
    let raw_bytes=proof.canonical_proposition.as_bytes().to_vec();
    let result=match (proof.relation,verdict,judge) {
        (0,crate::types::ConstitutionalVerdict::Constructive,Some(KnowledgeWrite::Primitive {
            input_id,vector_points,temporal_score,mass,seal,path,ts_ns,..}))=>Some(KnowledgeWrite::Primitive {
                input_id:*input_id,vector_points:*vector_points,temporal_score:*temporal_score,mass:*mass,
                seal:*seal,path:*path,domain:model_domain(proof,1)?,raw_bytes,ts_ns:*ts_ns}),
        (1,crate::types::ConstitutionalVerdict::RigorousNegative,Some(KnowledgeWrite::Negative {
            input_id,context,ts_ns,..}))=>Some(KnowledgeWrite::Negative {input_id:*input_id,raw_bytes,
                context:crate::types::NegativeContext {failure_reason_code:MODEL_COUNTERPROOF_REASON,
                    process:model_domain(proof,0)?,mass_at_fail:context.mass_at_fail,frequency:1},ts_ns:*ts_ns}),
        _=>None,
    };
    Ok(result)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ImmutableNegativeMetric {
    key: Key,
    write_sha256: [u8; 32],
    mass_bits: u32,
    frequency: u32,
}

impl ImmutableNegativeMetric {
    fn from_write(write: &KnowledgeWrite) -> Result<Option<Self>, String> {
        let KnowledgeWrite::Negative { input_id, raw_bytes, context, .. } = write else {
            return Ok(None);
        };
        if !context.mass_at_fail.is_finite() {
            return Err("KIE_CURRENT_SOURCE_NONFINITE_MASS".into());
        }
        Ok(Some(Self {
            key: key_for(0, *input_id, raw_bytes, &context.process, context.failure_reason_code),
            write_sha256: write.digest(),
            mass_bits: context.mass_at_fail.to_bits(),
            frequency: context.frequency,
        }))
    }

    fn same_measurement(&self, other: &Self) -> bool {
        self.key == other.key && self.mass_bits == other.mass_bits
            && self.frequency == other.frequency
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Origin {
    scope: [u8; 32],
    proposition: [u8; 32],
    family: [u8; 32],
    class: KieHistoryClass,
    provenance: KieHistoryProvenance,
    scoped_candidate: Option<CandidateStatisticWire>,
    // Captured from the immutable T1 write, before NegativeStore aggregation.
    // No float is reconstructed from quantized selection confidence.
    negative_metric: Option<ImmutableNegativeMetric>,
}

type ScopeKey = ([u8; 32], [u8; 32], [u8; 32]);
type OriginKey = ([u8; 32], String);

#[derive(Debug)]
struct IndexedNegativeOrigin {
    origin: Origin,
    conflicted: bool,
}

/// A Shadow-owned, current-presentation projection. Private fields prevent a
/// caller from supplying arbitrary masses or promoting NPRO conditions to
/// semantic counterproof. This is a derived index view, never a new authority.
#[derive(Debug)]
pub(crate) struct VerifiedCounterproofProjection {
    scope_sha256: [u8; 32],
    proposition_sha256: [u8; 32],
    model_family_sha256: [u8; 32],
    proof_sha256: [u8; 32],
    metrics: Vec<ImmutableNegativeMetric>,
}

impl VerifiedCounterproofProjection {
    pub(crate) fn proposition_sha256(&self) -> [u8; 32] {
        self.proposition_sha256
    }

    pub(crate) fn matches_proof(&self, proof: &KieProof) -> bool {
        proof.validate().is_ok()
            && proof.context.identity.key().ok() == Some(self.scope_sha256)
            && digest(proof.canonical_proposition.as_bytes()) == self.proposition_sha256
            && proof.proof_sha256 == self.proof_sha256
            && science_domains::decode_candidate(&proof.candidate_bytes).ok()
                .is_some_and(|model| crate::native_cycle::kie_family(&model) == self.model_family_sha256)
    }

    /// Response-only diagnostic from an already verified current semantic
    /// source. This is not the current verdict's negative_proof or a write
    /// capability; callers must not feed it into materialization authority.
    pub(crate) fn diagnostic_context(&self) -> Option<crate::types::NegativeContext> {
        self.metrics.first().map(|metric| crate::types::NegativeContext {
            failure_reason_code: MODEL_COUNTERPROOF_REASON,
            process: metric.key.3.clone(),
            mass_at_fail: f32::from_bits(metric.mass_bits),
            frequency: metric.frequency,
        })
    }

    /// v189 query_negative_pattern's exact source formula, applied only to the
    /// immutable semantic sources presented and verified for this request.
    /// Empty is the source formula's empty-set result, not invented evidence.
    pub(crate) fn negative_pattern(&self, epistemic_mass: f32) -> Result<f32, String> {
        if !epistemic_mass.is_finite() {
            return Err("KIE_CURRENT_NONFINITE_MASS".into());
        }
        const SOURCE_NEG_MASS_TOLERANCE: f32 = 0.05;
        let similar = self.metrics.iter().fold(0.0f32, |sum, metric| {
            let mass = f32::from_bits(metric.mass_bits);
            let close = (SOURCE_NEG_MASS_TOLERANCE - (mass - epistemic_mass).abs()).max(0.0)
                / SOURCE_NEG_MASS_TOLERANCE;
            sum + close * (metric.frequency as f32).min(10.0) / 10.0
        });
        Ok(match self.metrics.is_empty() {
            true => 0.0,
            false => (similar / self.metrics.len() as f32).clamp(0.0, 1.0),
        })
    }
}

#[derive(Debug, Default)]
pub(crate) struct HistoryIndex {
    entries: BTreeMap<Key, Vec<Option<Origin>>>,
    scoped: BTreeMap<ScopeKey, Vec<Origin>>,
    // Rebuilt from the same validated T1 replay; no second persistence store.
    negative_origins: BTreeMap<OriginKey, IndexedNegativeOrigin>,
}

#[derive(Debug)]
pub(crate) struct PendingOrigin {
    raw_sha: [u8; 32],
    origin: Option<Origin>,
}

fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b|
        (h ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3))
}

fn key_for(polarity: u8, id: u64, raw: &[u8], domain: &str, reason: u32) -> Key {
    let (term_domain, source_domain) = match (polarity, domain) {
        (1, "legacy") => (b"ESSMAI/SELECTION/TERM/LEGACY".as_slice(), b"ESSMAI/SELECTION/SOURCE/LEGACY".as_slice()),
        (1, _) => (b"ESSMAI/SELECTION/TERM/PRIMITIVE".as_slice(), b"ESSMAI/SELECTION/SOURCE/PRIMITIVE".as_slice()),
        _ => (b"ESSMAI/SELECTION/TERM/NEGATIVE".as_slice(), b"ESSMAI/SELECTION/SOURCE/NEGATIVE".as_slice()),
    };
    let source = match polarity {
        1 => shadow_contracts::sha256_hex(&[source_domain, &id.to_le_bytes(), raw]),
        _ => shadow_contracts::sha256_hex(&[source_domain, &id.to_le_bytes(), &reason.to_le_bytes(), raw]),
    };
    (polarity, id, source, domain.into(), shadow_contracts::sha256_hex(&[term_domain, raw]))
}

fn candidate_key(candidate: &CandidateStatisticWire) -> Key {
    (candidate.polarity, candidate.candidate_id, candidate.source_sha256.clone(),
        candidate.domain.clone(), candidate.term_sha256.clone())
}

fn scoped_candidate(write:&KnowledgeWrite,tx:&VaultTransaction)->Result<CandidateStatisticWire,String> {
    let fixed=|mass|shadow_contracts::fixed_mass(mass).ok_or("KIE_SCOPED_NONFINITE_METRIC".to_string());
    let (key,confidence,frequency,legacy)=match write {
        KnowledgeWrite::Primitive {input_id,raw_bytes,vector_points,temporal_score,..}=>
            (key_for(1,*input_id,raw_bytes,"primitive",0),fixed(*vector_points)?,0,
                fixed((*vector_points*0.40+*temporal_score*0.25).clamp(0.0,1.0))?),
        KnowledgeWrite::Negative {input_id,raw_bytes,context,..}=>
            (key_for(0,*input_id,raw_bytes,&context.process,context.failure_reason_code),
                shadow_contracts::MASS_SCALE.saturating_sub(fixed(context.mass_at_fail)?),context.frequency,0),
    };
    Ok(CandidateStatisticWire {polarity:key.0,candidate_id:key.1,source_sha256:key.2,domain:key.3,term_sha256:key.4,
        security_y:tx.receipt.y_verdict,trust_x:tx.receipt.x_verdict,confidence_fixed:confidence,frequency,
        domain_count:1,legacy_score_fixed:legacy})
}

/// Only an authenticated, nonexperimental operator-observation parent qualifies.
/// Its lexical target is not an empirical counterexample merely because NPRO
/// reported weakness. Unknown or experimental evidence is never downgraded here.
fn observation_parent(tx: &VaultTransaction, proof: &KieProof) -> Option<NegativeAssetEnvelope> {
    tx.knowledge_writes.iter().find_map(|write| {
        let KnowledgeWrite::Negative { raw_bytes, context, .. } = write else { return None };
        let parent = NegativeAssetEnvelope::decode(raw_bytes).ok()?;
        let expected = crate::vault_transaction::sha256_parts(&[
            NEGATIVE_ASSET_TOKEN_DOMAIN, &parent.canonical_body()]);
        let bound = format!("GCL_NEGATIVE_ASSET_SHA256={}", pd_memory::hex(&expected));
        (parent.shape_is_valid(shadow_contracts::MASS_SCALE)
            && parent.encode() == *raw_bytes
            && parent.token_sha256 == expected
            && context.process.split('|').any(|part| part == bound)
            && parent.input_sha256 == digest(proof.canonical_proposition.as_bytes())
            && parent.project_id == 0 && !parent.experimental_failure
            && parent.asset_class == NEGATIVE_ASSET_CLASS_KNOWLEDGE)
            .then_some(parent)
    })
}

fn target_observation(write: &KnowledgeWrite, parent: &NegativeAssetEnvelope) -> bool {
    let KnowledgeWrite::Negative { input_id, raw_bytes, context, .. } = write else { return false };
    parent.targets.iter().any(|target| {
        let space = match target.space { NEGATIVE_SPACE_XI => "Xi", NEGATIVE_SPACE_YI => "Yi", _ => return false };
        let process = format!("KOLAPS/2:-{space}(term={};{})", target.term, target.path_reason());
        *input_id == fnv(target.term.as_bytes()) && raw_bytes.as_slice() == target.term.as_bytes()
            && context.process == process && context.failure_reason_code == 0xA451
    })
}

/// Prepared only from an already validated transaction. The original immutable
/// transaction remains the source; no caller-supplied classification is stored.
pub(crate) fn prepare(tx: &VaultTransaction) -> Result<Vec<PendingOrigin>, String> {
    let proof = tx.native_cycle.as_ref().filter(|s| !s.kie_proof.is_empty())
        .map(|s| decode_proof(&s.kie_proof)).transpose()?;
    let source = proof.as_ref().map(|proof| -> Result<_, String> {
        crate::kie_selection::validate_proof_model(proof)?;
        let model = science_domains::decode_candidate(&proof.candidate_bytes)
            .map_err(|e| format!("KIE_HISTORY_SOURCE_MODEL:{e}"))?;
        let family = crate::native_cycle::kie_family(&model);
        Ok((proof.context.identity.key()?, digest(proof.canonical_proposition.as_bytes()), family))
    }).transpose()?;
    let witnessed = tx.native_cycle.as_ref().filter(|s| !s.prepared.is_empty())
        .map(|s| shadow_contracts::pd_prepared::decode_prepared(&s.prepared))
        .transpose()?.is_some_and(|p| p.failure_witness.is_some());
    let parent = proof.as_ref().and_then(|p| observation_parent(tx, p));
    tx.knowledge_writes.iter().map(|write| {
        let raw = match write {
            KnowledgeWrite::Primitive { raw_bytes, .. }
            | KnowledgeWrite::Negative { raw_bytes, .. } => raw_bytes,
        };
        let origin = match (&proof, source) {
            (Some(proof), Some((scope, proposition, family))) => {
                let class = match write {
                    KnowledgeWrite::Primitive { .. } if tx.y == 1 && tx.x == 1 && proof.relation == 0
                        && raw.as_slice() == proof.canonical_proposition.as_bytes() => KieHistoryClass::ExactModelSupport,
                    KnowledgeWrite::Negative { context, .. } if context.failure_reason_code == MODEL_COUNTERPROOF_REASON
                        && proof.history_assessment.is_some() && proof.relation == 1 && tx.y == 0 && tx.x == 0
                        && raw.as_slice() == proof.canonical_proposition.as_bytes() => KieHistoryClass::ExactModelCounterproof,
                    KnowledgeWrite::Negative { context, .. } if witnessed && (is_non_refuting_process(context.failure_reason_code, &context.process, raw)
                        || parent.as_ref().is_some_and(|p| target_observation(write, p))) => KieHistoryClass::ProcessFailure,
                    _ => KieHistoryClass::LegacyUnscoped,
                };
                Some(Origin { scope, proposition, family, class, provenance: KieHistoryProvenance {
                    transaction_id: tx.transaction_id, proof_sha256: proof.proof_sha256, model_family_sha256: family },
                    scoped_candidate:match class {
                        KieHistoryClass::ExactModelSupport|KieHistoryClass::ExactModelCounterproof=>Some(scoped_candidate(write,tx)?),
                        _=>None,
                    }, negative_metric: match class {
                        KieHistoryClass::ExactModelCounterproof => ImmutableNegativeMetric::from_write(write)?,
                        _ => None,
                    } })
            }
            _ => None,
        };
        Ok(PendingOrigin { raw_sha: digest(raw), origin })
    }).collect()
}

impl HistoryIndex {
    fn publish_scoped(&mut self,origin:Option<&Origin>) {
        match origin.filter(|origin|origin.scoped_candidate.is_some()) {
            Some(origin)=> {
                let group = self.scoped.entry((origin.scope, origin.proposition, origin.family)).or_default();
                if !group.iter().any(|prior| prior.class == origin.class) {
                    group.push(origin.clone());
                }
            }
            None=>{},
        }
    }

    fn publish_negative_origin(&mut self, origin: Option<&Origin>) {
        let Some(origin) = origin.filter(|origin| origin.class == KieHistoryClass::ExactModelCounterproof) else { return };
        let Some(metric) = &origin.negative_metric else { return };
        let key = (origin.provenance.transaction_id, metric.key.2.clone());
        match self.negative_origins.entry(key) {
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert(IndexedNegativeOrigin { origin: origin.clone(), conflicted: false });
            }
            std::collections::btree_map::Entry::Occupied(mut slot) => {
                // Preserve the first source. Never overwrite a conflict with a
                // later apparently valid value; resolution must fail closed.
                if slot.get().origin != *origin { slot.get_mut().conflicted = true; }
            }
        }
    }
    /// Bind to the actual storage projection after its canonical deduplication.
    /// A colliding/merged different byte object taints the reference as unknown.
    pub(crate) fn publish_negative(&mut self, pending: PendingOrigin, entry: &crate::knowledge_vault::NegativeEntry) {
        let origin = (pending.raw_sha == digest(&entry.raw_bytes)).then_some(pending.origin).flatten();
        self.publish_scoped(origin.as_ref());
        self.publish_negative_origin(origin.as_ref());
        let key = key_for(0, entry.input_id, &entry.raw_bytes, &entry.process, entry.failure_reason_code);
        self.entries.entry(key).or_default().push(origin);
    }
    pub(crate) fn publish_positive(&mut self, pending: PendingOrigin, entry: &crate::knowledge_vault::PrimitiveEntry) {
        let origin = (pending.raw_sha == digest(&entry.raw_bytes)).then_some(pending.origin).flatten();
        self.publish_scoped(origin.as_ref());
        for domain in ["primitive", "legacy"] {
            self.entries.entry(key_for(1, entry.input_id, &entry.raw_bytes, domain, 0))
                .or_default().push(origin.clone());
        }
    }

    fn classify(&self, candidate: &CandidateStatisticWire, scope: [u8; 32], proposition: [u8; 32], family: [u8; 32]) -> KieHistoryEntry {
        let mut entry = KieHistoryEntry { polarity: candidate.polarity, candidate_id: candidate.candidate_id,
            source_sha256: candidate.source_sha256.clone(),
            selection_context: Some(KieHistoryContext {
                domain: candidate.domain.clone(), term_sha256: candidate.term_sha256.clone(),
            }), class: KieHistoryClass::LegacyUnscoped, provenance: None };
        let Some(origins) = self.entries.get(&candidate_key(candidate)) else { return entry };
        let classified = origins.iter().map(|origin| origin.as_ref().map(|origin| {
            let class = match (origin.scope == scope && origin.proposition == proposition, origin.family == family) {
                (false, _) => KieHistoryClass::DifferentScope,
                (true, false) => KieHistoryClass::LegacyUnscoped,
                (true, true) => origin.class,
            };
            (class, &origin.provenance)
        })).collect::<Vec<_>>();
        // A real scoped counterproof remains decisive even alongside observations.
        let selected = classified.iter().flatten().find(|(class, _)| *class == KieHistoryClass::ExactModelCounterproof)
            .or_else(|| {
                match classified.iter().any(|v| v.is_none() || v.is_some_and(|(c, _)| c == KieHistoryClass::LegacyUnscoped)) {
                    true => None,
                    false => classified.iter().flatten().find(|(class, _)| *class != KieHistoryClass::DifferentScope)
                        .or_else(|| classified.iter().flatten().next()),
                }
            });
        match selected {
            Some((class, provenance)) => { entry.class = *class; entry.provenance = Some((*provenance).clone()); }
            None => {}
        }
        entry
    }

    pub(crate) fn assess(&self, proof: &KieProof) -> Result<KieHistoryAssessment, String> {
        let request = shadow_contracts::decode_selection_request(&proof.historical_request).map_err(|e| format!("KIE_HISTORY_REQUEST:{e}"))?;
        let response = shadow_contracts::decode_selection_response(&proof.historical_response).map_err(|e| format!("KIE_HISTORY_RESPONSE:{e}"))?;
        let projection = pd_memory::failure::relevance::project_available_bound(&request, &response)?;
        let model = science_domains::decode_candidate(&proof.candidate_bytes).map_err(|e| format!("KIE_HISTORY_CURRENT_MODEL:{e}"))?;
        let scope = proof.context.identity.key()?;
        let proposition = digest(proof.canonical_proposition.as_bytes());
        let family = crate::native_cycle::kie_family(&model);
        let mut entries = projection.positive.iter().chain(projection.negative.iter())
            .map(|value| self.classify(value.candidate, scope, proposition, family)).collect::<Vec<_>>();
        entries.sort_by(|a, b| a.selection_key().cmp(&b.selection_key()));
        // Every previously valid selection retains its exact KHA1 encoding.
        // Only colliding short keys require the full row context in KHA2.
        if !entries.windows(2).any(|pair| pair[0].key() == pair[1].key()) {
            for entry in &mut entries { entry.selection_context = None; }
        }
        let mut scoped_evidence=self.scoped.get(&(scope, proposition, family)).into_iter().flatten().filter_map(|origin|
                origin.scoped_candidate.as_ref().map(|candidate|KieScopedEvidence {
                    candidate:candidate.clone(),class:origin.class,provenance:origin.provenance.clone()})).collect::<Vec<_>>();
        scoped_evidence.sort_by(|a,b|(a.candidate.polarity,a.candidate.candidate_id,&a.candidate.source_sha256)
            .cmp(&(b.candidate.polarity,b.candidate.candidate_id,&b.candidate.source_sha256)));
        let assessment = KieHistoryAssessment { scope_sha256: scope, proposition_sha256: proposition, entries,scoped_evidence };
        assessment.validate_for(proof)?;
        Ok(assessment)
    }

    pub(crate) fn project_presented_counterproofs(&self, proof: &KieProof)
        -> Result<VerifiedCounterproofProjection, String> {
        proof.validate()?;
        crate::kie_selection::validate_proof_model(proof)?;
        let assessment = proof.history_assessment.as_ref().ok_or("KIE_CURRENT_REQUIRES_KIP3")?;
        if self.assess(proof)? != *assessment {
            return Err("KIE_CURRENT_ACTUAL_SOURCE_MISMATCH".into());
        }
        let scope = proof.context.identity.key()?;
        let proposition = digest(proof.canonical_proposition.as_bytes());
        let model = science_domains::decode_candidate(&proof.candidate_bytes)
            .map_err(|e| format!("KIE_CURRENT_MODEL:{e}"))?;
        let family = crate::native_cycle::kie_family(&model);
        let request = crate_selection_request(proof)?;
        let response = shadow_contracts::decode_selection_response(&proof.historical_response)
            .map_err(|e| format!("KIE_CURRENT_RESPONSE:{e}"))?;
        let available = pd_memory::failure::relevance::project_available_bound(&request, &response)?;
        let mut metrics: BTreeMap<String, ImmutableNegativeMetric> = BTreeMap::new();
        let mut include = |candidate: &CandidateStatisticWire, provenance: &KieHistoryProvenance| -> Result<(), String> {
            let indexed = self.negative_origins.get(&(provenance.transaction_id, candidate.source_sha256.clone()))
                .ok_or("KIE_CURRENT_ORIGIN_MISSING")?;
            let origin = &indexed.origin;
            let metric = origin.negative_metric.as_ref().ok_or("KIE_CURRENT_METRIC_MISSING")?;
            if indexed.conflicted || origin.scope != scope || origin.proposition != proposition
                || origin.family != family || origin.class != KieHistoryClass::ExactModelCounterproof
                || origin.provenance != *provenance || metric.key != candidate_key(candidate)
                || metric.write_sha256 == [0; 32] || !f32::from_bits(metric.mass_bits).is_finite() {
                return Err("KIE_CURRENT_ORIGIN_BINDING".into());
            }
            match metrics.entry(candidate.source_sha256.clone()) {
                std::collections::btree_map::Entry::Vacant(slot) => { slot.insert(metric.clone()); }
                std::collections::btree_map::Entry::Occupied(slot) => {
                    if !slot.get().same_measurement(metric) {
                        return Err("KIE_CURRENT_DUPLICATE_SOURCE_CONFLICT".into());
                    }
                }
            }
            Ok(())
        };
        for value in &available.negative {
            let candidate = value.candidate;
            let entry = assessment.entry_for(candidate).ok_or("KIE_CURRENT_SELECTED_SOURCE_MISSING")?;
            if entry.class == KieHistoryClass::ExactModelCounterproof {
                include(candidate, entry.provenance.as_ref().ok_or("KIE_CURRENT_PROVENANCE_MISSING")?)?;
            }
        }
        for scoped in &assessment.scoped_evidence {
            if scoped.class == KieHistoryClass::ExactModelCounterproof {
                include(&scoped.candidate, &scoped.provenance)?;
            }
        }
        Ok(VerifiedCounterproofProjection {
            scope_sha256: scope, proposition_sha256: proposition, model_family_sha256: family,
            proof_sha256: proof.proof_sha256, metrics: metrics.into_values().collect(),
        })
    }
}

fn crate_selection_request(proof: &KieProof) -> Result<shadow_contracts::KnowledgeSelectionRequestWire, String> {
    shadow_contracts::decode_selection_request(&proof.historical_request)
        .map_err(|e| format!("KIE_CURRENT_REQUEST:{e}"))
}

/// A new reason marker is never sufficient to manufacture a semantic negative.
/// It must name this exact proposition in a verified KIP3 model-counter cycle.
pub(crate) fn validate_new_markers(tx: &VaultTransaction) -> Result<(), String> {
    let is_marked=|write:&KnowledgeWrite| match write {
        KnowledgeWrite::Negative { context, .. }=>context.failure_reason_code == MODEL_COUNTERPROOF_REASON,
        KnowledgeWrite::Primitive {domain,..}=>domain.starts_with(MODEL_DOMAIN),
    };
    let marked = tx.knowledge_writes.iter().filter(|write|is_marked(write)).collect::<Vec<_>>();
    match marked.is_empty() { true => return Ok(()), false => {} }
    let snapshot = tx.native_cycle.as_ref().filter(|snapshot|!snapshot.kie_proof.is_empty())
        .ok_or("KIE_COUNTER_SOURCE_MISSING")?;
    let proof = decode_proof(&snapshot.kie_proof)?;
    crate::kie_selection::validate_proof_model(&proof)?;
    let judge=tx.knowledge_writes.iter().rev().find(|write|!is_marked(write));
    let expected=conditional_write(&proof,tx.verdict_kind,judge)?;
    match (&expected,marked.as_slice()) {
        (Some(expected),[actual]) if expected.digest()==actual.digest()=>{},
        _=>return Err("KIE_CONDITIONAL_WRITE_METRICS_OR_SOURCE".into()),
    }
    let expected_domain=model_domain(&proof,u8::from(proof.relation==0))?;
    match proof.history_assessment.is_some() && marked.len() == 1 && marked.iter().all(|write|match write {
        KnowledgeWrite::Negative {raw_bytes,context,..}=>proof.relation==1 && (tx.y,tx.x)==(0,0)
            && raw_bytes.as_slice()==proof.canonical_proposition.as_bytes() && context.process==expected_domain,
        KnowledgeWrite::Primitive {raw_bytes,domain,..}=>proof.relation==0 && (tx.y,tx.x)==(1,1)
            && raw_bytes.as_slice()==proof.canonical_proposition.as_bytes() && *domain==expected_domain,
    }) {
        true => Ok(()), false => Err("KIE_COUNTER_EXACT_MODEL_BINDING".into()),
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    // Actual pinned numerical model, synthetic selection/record identifiers.
    // This fixture does not assert durable PDS, GCL admission or native success.
    pub(crate) fn empty_current_projection() -> (KieProof, VerifiedCounterproofProjection) {
        let identity = pd_memory::PdIdentity { owner:"r24-fixture".into(), obligation:"model".into(),
            case_id:"case".into(), scope:"astronomy".into(), domain:"astronomy".into(), lineage_root:"root".into() };
        let query = science_domains::ScienceQuery {
            adapter:science_domains::AdapterId::Astronomy, session_id:pd_memory::hex(&identity.key().unwrap()),
            case_id:identity.case_id.clone(), scope_id:identity.scope.clone(), obligation_id:identity.obligation.clone(),
            query_id:"query".into(), input_source_id:"fixture".into(),
            task:science_domains::ScienceTask::EarthRotationAngle { jd_part1:2451545.0, jd_part2:0.0,
                time_scale:science_domains::TimeScale::Ut1, claim_radians:0.0, tolerance_radians:0.1 },
        };
        let query_bytes = science_domains::encode_query(&query).unwrap();
        let candidate_bytes = science_domains::encode_candidate(&science_domains::evaluate(&query).unwrap()).unwrap();
        let result = science_domains::kie::evaluate_kie(&query_bytes, &candidate_bytes).unwrap();
        let request = shadow_contracts::KnowledgeSelectionRequestWire {
            version:shadow_contracts::PROTOCOL_VERSION, requester:shadow_contracts::SelectionRequester::AshtQuantum,
            purpose:shadow_contracts::SelectionPurpose::EvidenceCandidates, session_id:"attempt".into(),
            parent_i0:pd_memory::hex(&digest(result.canonical_proposition.as_bytes())), primitive_anchor:1,
            trace_id:"attempt".into(), split_sha256:"a".repeat(64), xi:vec![result.canonical_proposition.clone()],
            yi:vec![], upstream_hold_sha256:Some("b".repeat(64)), evidence_sha256:"c".repeat(64),
            max_positive:shadow_contracts::SELECTION_MAX_CANDIDATES, max_negative:shadow_contracts::SELECTION_MAX_CANDIDATES,
        };
        let historical_request = shadow_contracts::encode_selection_request(&request);
        let mut response = shadow_contracts::KnowledgeSelectionResponseWire {
            version:shadow_contracts::PROTOCOL_VERSION, requester:request.requester, session_id:request.session_id.clone(),
            parent_i0:request.parent_i0.clone(), primitive_anchor:request.primitive_anchor, split_sha256:request.split_sha256.clone(),
            upstream_hold_sha256:request.upstream_hold_sha256.clone(),
            request_sha256:shadow_contracts::sha256_hex(&[b"ESSMAI/SELECTION/REQUEST/V178", &historical_request]),
            selection_sha256:String::new(), hold_id:"fixture".into(), hold_sha256:"d".repeat(64),
            positive:vec![], negative:vec![], law_seal:1, system_laws_seal:1, expires_at_ns:1,
        };
        response.selection_sha256 = pd_memory::failure::relevance::selection_digest(&response);
        let mut proof = KieProof {
            context:KieContext { identity, operation_id:"fixture-op".into(), record_id:[3;32] },
            query_bytes, candidate_bytes, canonical_proposition:result.canonical_proposition,
            historical_request, historical_response:shadow_contracts::encode_selection_response(&response),
            kis_mass:result.kis_mass, kic_mass:result.kic_mass, relation:result.relation,
            proof_sha256:[0;32], prior_adjudication:None, history_assessment:None,
        };
        let index = HistoryIndex::default();
        proof.history_assessment = Some(index.assess(&proof).unwrap());
        let proof = proof.seal_data().unwrap();
        let projection = index.project_presented_counterproofs(&proof).unwrap();
        (proof, projection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn candidate()->CandidateStatisticWire {
        let key=key_for(0,7,b"scoped term","operator weakness",0xA451);
        CandidateStatisticWire { polarity:0,candidate_id:7,source_sha256:key.2,domain:key.3,term_sha256:key.4,
            security_y:0,trust_x:0,confidence_fixed:9000,frequency:1,domain_count:1,legacy_score_fixed:0 }
    }
    fn origin(class:KieHistoryClass)->Origin {
        Origin {scope:[1;32],proposition:[2;32],family:[3;32],class,
            provenance:KieHistoryProvenance {transaction_id:[4;32],proof_sha256:[5;32],model_family_sha256:[3;32]},
            scoped_candidate:None,negative_metric:None}
    }
    #[test]
    fn r21_unknown_and_mixed_unknown_sources_hold_but_real_counterproof_survives() {
        let c=candidate(); let key=candidate_key(&c); let mut index=HistoryIndex::default();
        assert_eq!(index.classify(&c,[1;32],[2;32],[3;32]).class,KieHistoryClass::LegacyUnscoped);
        index.entries.insert(key.clone(),vec![Some(origin(KieHistoryClass::ProcessFailure))]);
        assert_eq!(index.classify(&c,[1;32],[2;32],[3;32]).class,KieHistoryClass::ProcessFailure);
        index.entries.get_mut(&key).unwrap().push(None);
        assert_eq!(index.classify(&c,[1;32],[2;32],[3;32]).class,KieHistoryClass::LegacyUnscoped);
        index.entries.get_mut(&key).unwrap().push(Some(origin(KieHistoryClass::ExactModelCounterproof)));
        let actual=index.classify(&c,[1;32],[2;32],[3;32]);
        assert_eq!(actual.class,KieHistoryClass::ExactModelCounterproof);
        assert_eq!(actual.provenance.unwrap().transaction_id,[4;32]);
    }
    #[test]
    fn r21_scope_is_exact_and_family_change_is_unknown_not_counterproof() {
        let c=candidate(); let mut index=HistoryIndex::default();
        index.entries.insert(candidate_key(&c),vec![Some(origin(KieHistoryClass::ExactModelCounterproof))]);
        assert_eq!(index.classify(&c,[9;32],[2;32],[3;32]).class,KieHistoryClass::DifferentScope);
        assert_eq!(index.classify(&c,[1;32],[9;32],[3;32]).class,KieHistoryClass::DifferentScope);
        assert_eq!(index.classify(&c,[1;32],[2;32],[9;32]).class,KieHistoryClass::LegacyUnscoped);
        let mut altered=c.clone(); altered.domain.push('!');
        assert_eq!(index.classify(&altered,[1;32],[2;32],[3;32]).class,KieHistoryClass::LegacyUnscoped);
        altered=c; altered.source_sha256="f".repeat(64);
        assert_eq!(index.classify(&altered,[1;32],[2;32],[3;32]).class,KieHistoryClass::LegacyUnscoped);
    }
    #[test]
    fn r21_storage_merge_cannot_relabel_different_bytes_with_an_observation_origin() {
        let mut index=HistoryIndex::default();
        let pending=PendingOrigin {
            raw_sha:digest(b"different"),origin:Some(origin(KieHistoryClass::ProcessFailure))};
        let entry=crate::knowledge_vault::NegativeEntry {input_id:7,failure_reason_code:0xA451,
            process:"operator weakness".into(),mass_at_fail:0.1,frequency:1,raw_bytes:b"scoped term".to_vec(),
            access_count:0,added_at:0};
        index.publish_negative(pending,&entry);
        assert_eq!(index.classify(&candidate(),[1;32],[2;32],[3;32]).class,KieHistoryClass::LegacyUnscoped);
    }

    fn presented_fixture(class: KieHistoryClass) -> (HistoryIndex, KieProof, Origin) {
        let (mut proof, _) = test_support::empty_current_projection();
        let mass = 0.500_037f32;
        let reason = if class == KieHistoryClass::ExactModelCounterproof {
            MODEL_COUNTERPROOF_REASON
        } else { PROCESS_OBSERVATION_REASON };
        let write = KnowledgeWrite::Negative { input_id:7, raw_bytes:proof.canonical_proposition.as_bytes().to_vec(),
            context:crate::types::NegativeContext { failure_reason_code:reason, process:"r24-source-fixture".into(),
                mass_at_fail:mass, frequency:3 }, ts_ns:1 };
        let metric = ImmutableNegativeMetric::from_write(&write).unwrap().unwrap();
        let candidate = CandidateStatisticWire { candidate_id:7, polarity:0, domain:metric.key.3.clone(),
            term_sha256:metric.key.4.clone(), source_sha256:metric.key.2.clone(), security_y:0, trust_x:0,
            confidence_fixed:shadow_contracts::MASS_SCALE-shadow_contracts::fixed_mass(mass).unwrap(),
            frequency:3, domain_count:1, legacy_score_fixed:0 };
        let model = science_domains::decode_candidate(&proof.candidate_bytes).unwrap();
        let family = crate::native_cycle::kie_family(&model);
        let origin = Origin { scope:proof.context.identity.key().unwrap(), proposition:digest(proof.canonical_proposition.as_bytes()),
            family, class, provenance:KieHistoryProvenance { transaction_id:[4;32], proof_sha256:proof.proof_sha256,
                model_family_sha256:family }, scoped_candidate:None, negative_metric:Some(metric) };
        let mut index = HistoryIndex::default();
        index.entries.insert(candidate_key(&candidate), vec![Some(origin.clone())]);
        index.publish_negative_origin(Some(&origin));
        let mut response = shadow_contracts::decode_selection_response(&proof.historical_response).unwrap();
        response.negative.push(candidate);
        response.selection_sha256 = pd_memory::failure::relevance::selection_digest(&response);
        proof.historical_response = shadow_contracts::encode_selection_response(&response);
        proof.history_assessment = Some(index.assess(&proof).unwrap());
        (index, proof.seal_data().unwrap(), origin)
    }

    #[test]
    fn r24_current_counterproof_uses_exact_original_mass_and_frequency() {
        let (index, proof, origin) = presented_fixture(KieHistoryClass::ExactModelCounterproof);
        let projection = index.project_presented_counterproofs(&proof).unwrap();
        let metric = origin.negative_metric.unwrap();
        assert_eq!(projection.metrics.len(), 1);
        assert_eq!(projection.metrics[0].mass_bits, 0.500_037f32.to_bits());
        assert_eq!(projection.metrics[0].frequency, 3);
        let actual_mass = f32::from_bits(metric.mass_bits);
        assert_eq!(projection.negative_pattern(actual_mass).unwrap(), 0.3);
        let reference = ((0.05f32-(actual_mass-0.52).abs()).max(0.0)/0.05*3.0/10.0).clamp(0.0,1.0);
        assert_eq!(projection.negative_pattern(0.52).unwrap().to_bits(), reference.to_bits());
        assert!(projection.matches_proof(&proof));
        assert_eq!(projection.proposition_sha256(), digest(proof.canonical_proposition.as_bytes()));
        assert!(projection.negative_pattern(f32::NAN).is_err());
    }

    #[test]
    fn r24_process_observation_is_not_a_current_semantic_counterproof() {
        let (index, proof, _) = presented_fixture(KieHistoryClass::ProcessFailure);
        assert!(index.negative_origins.is_empty());
        let projection = index.project_presented_counterproofs(&proof).unwrap();
        assert!(projection.metrics.is_empty());
        assert_eq!(projection.negative_pattern(0.500_037).unwrap(), 0.0);
        assert!(projection.diagnostic_context().is_none());
    }

    #[test]
    fn r24_current_projection_rejects_rehashed_forgery_and_context_substitution() {
        let (index, proof, _) = presented_fixture(KieHistoryClass::ExactModelCounterproof);
        let projection = index.project_presented_counterproofs(&proof).unwrap();
        let mut forged = proof.clone();
        forged.history_assessment.as_mut().unwrap().entries[0].provenance.as_mut().unwrap().transaction_id = [9;32];
        let forged = forged.seal_data().unwrap();
        assert!(index.project_presented_counterproofs(&forged).is_err());
        assert!(!projection.matches_proof(&forged));
        let mut changed = proof.clone();
        changed.context.identity.scope = "different-context".into();
        assert!(index.project_presented_counterproofs(&changed).is_err());
        assert!(!projection.matches_proof(&changed));
        let mut legacy = proof;
        legacy.history_assessment = None;
        assert!(index.project_presented_counterproofs(&legacy.seal_data().unwrap()).is_err());
    }

    #[test]
    fn r24_same_source_presented_lexically_and_scoped_is_counted_once() {
        let (mut index, mut proof, mut origin) = presented_fixture(KieHistoryClass::ExactModelCounterproof);
        let candidate = shadow_contracts::decode_selection_response(&proof.historical_response).unwrap().negative.remove(0);
        origin.scoped_candidate = Some(candidate);
        // Production prepare records the same full origin before either index.
        index.negative_origins.clear();
        index.publish_negative_origin(Some(&origin));
        index.publish_scoped(Some(&origin));
        proof.history_assessment = Some(index.assess(&proof).unwrap());
        let proof = proof.seal_data().unwrap();
        assert_eq!(proof.history_assessment.as_ref().unwrap().entries.len(),1);
        assert_eq!(proof.history_assessment.as_ref().unwrap().scoped_evidence.len(),1);
        let projection = index.project_presented_counterproofs(&proof).unwrap();
        assert_eq!(projection.metrics.len(),1);
        assert_eq!(projection.negative_pattern(0.500_037).unwrap(),0.3);
    }

    #[test]
    fn r24_conflicting_source_descriptor_never_overwrites_original() {
        let (mut index, proof, origin) = presented_fixture(KieHistoryClass::ExactModelCounterproof);
        let mut changed = origin.clone();
        changed.negative_metric.as_mut().unwrap().frequency = 10;
        index.publish_negative_origin(Some(&changed));
        index.publish_negative_origin(Some(&origin));
        assert!(index.project_presented_counterproofs(&proof).is_err());
        let stored = index.negative_origins.values().next().unwrap();
        assert!(stored.conflicted);
        assert_eq!(stored.origin.negative_metric.as_ref().unwrap().frequency,3);
    }

    #[test]
    fn r24_indexed_scope_preserves_legacy_assessment_and_ignores_unrelated_origins() {
        let (mut index, mut proof, mut origin) = presented_fixture(KieHistoryClass::ExactModelCounterproof);
        origin.scoped_candidate = Some(shadow_contracts::decode_selection_response(&proof.historical_response).unwrap().negative.remove(0));
        index.negative_origins.clear();
        index.publish_negative_origin(Some(&origin));
        let mut old_order = Vec::new();
        for n in 1..=32 {
            let mut unrelated = origin.clone();
            unrelated.scope = [n;32];
            unrelated.provenance.transaction_id = [0x80+n;32];
            index.publish_scoped(Some(&unrelated));
            index.publish_negative_origin(Some(&unrelated));
            old_order.push(unrelated);
        }
        index.publish_scoped(Some(&origin));
        old_order.push(origin.clone());
        let actual = index.assess(&proof).unwrap();
        let mut expected = actual.clone();
        // Exact old Vec selection/order, independently retained as reference.
        expected.scoped_evidence = old_order.iter().filter(|o| o.scope==origin.scope
            && o.proposition==origin.proposition && o.family==origin.family)
            .map(|o| KieScopedEvidence { candidate:o.scoped_candidate.clone().unwrap(),
                class:o.class, provenance:o.provenance.clone() }).collect();
        expected.scoped_evidence.sort_by(|a,b|a.key().cmp(&b.key()));
        assert_eq!(actual.encode().unwrap(),expected.encode().unwrap());
        proof.history_assessment = Some(actual);
        let proof = proof.seal_data().unwrap();
        let projection = index.project_presented_counterproofs(&proof).unwrap();
        assert_eq!(projection.metrics.len(),1);
        assert_eq!(projection.negative_pattern(0.500_037).unwrap(),0.3);
    }

    #[test]
    fn r24_prepare_publish_preserves_t1_metrics_after_storage_aggregation() {
        // A data-only index fixture exercises the actual prepare/publish chain.
        // It does not claim native admission, a validated receipt or a WAL write.
        let (mut proof, _) = test_support::empty_current_projection();
        assert_eq!(proof.relation, 1);
        let original_context = crate::types::NegativeContext {
            failure_reason_code: MODEL_COUNTERPROOF_REASON,
            process: model_domain(&proof, 0).unwrap(),
            mass_at_fail: 0.500_037,
            frequency: 3,
        };
        let mut tx = crate::vault_transaction::test_support::sample_transaction();
        tx.y = 0;
        tx.x = 0;
        tx.receipt.y_verdict = 0;
        tx.receipt.x_verdict = 0;
        tx.verdict_kind = crate::types::ConstitutionalVerdict::RigorousNegative;
        tx.native_cycle = Some(crate::native_cycle::NativeCycleSnapshot {
            request_sha256: [8;32], response:vec![], prepared:vec![],
            kie_proof:encode_proof(&proof).unwrap(),
            model_judgment: None,
        });
        tx.knowledge_writes = vec![KnowledgeWrite::Negative {
            input_id:7, raw_bytes:proof.canonical_proposition.as_bytes().to_vec(),
            context:original_context.clone(), ts_ns:1,
        }];
        let pending = prepare(&tx).unwrap().remove(0);
        assert_eq!(pending.origin.as_ref().unwrap().class, KieHistoryClass::ExactModelCounterproof);
        let mut index = HistoryIndex::default();
        let aggregated = crate::knowledge_vault::NegativeEntry {
            input_id:7, failure_reason_code:MODEL_COUNTERPROOF_REASON,
            process:original_context.process.clone(), mass_at_fail:0.500_01, frequency:99,
            raw_bytes:proof.canonical_proposition.as_bytes().to_vec(), access_count:99, added_at:99,
        };
        index.publish_negative(pending, &aggregated);
        proof.history_assessment = Some(index.assess(&proof).unwrap());
        let proof = proof.seal_data().unwrap();
        let projection = index.project_presented_counterproofs(&proof).unwrap();
        assert_eq!(projection.metrics.len(),1);
        assert_eq!(projection.metrics[0].write_sha256,tx.knowledge_writes[0].digest());
        assert_eq!(projection.negative_pattern(original_context.mass_at_fail).unwrap(),0.3);
        assert_eq!(projection.diagnostic_context(),Some(original_context));

        let KnowledgeWrite::Negative { context, .. } = &mut tx.knowledge_writes[0] else { unreachable!() };
        context.failure_reason_code = PROCESS_OBSERVATION_REASON;
        let process_origin = prepare(&tx).unwrap().remove(0).origin.unwrap();
        assert!(process_origin.negative_metric.is_none());
    }

    // Like the R24 prepare/publish fixture, these are data-only index tests.
    // Synthetic T1 identifiers do not claim native admission or a WAL commit.
    fn r35_counterproof_transaction(
        proof: &KieProof, transaction_id: [u8; 32], frequency: u32,
    ) -> (VaultTransaction, crate::knowledge_vault::NegativeEntry) {
        let context = crate::types::NegativeContext {
            failure_reason_code: MODEL_COUNTERPROOF_REASON,
            process: model_domain(proof, 0).unwrap(),
            mass_at_fail: 0.500_037,
            frequency,
        };
        let mut tx = crate::vault_transaction::test_support::sample_transaction();
        tx.transaction_id = transaction_id;
        tx.y = 0;
        tx.x = 0;
        tx.receipt.y_verdict = 0;
        tx.receipt.x_verdict = 0;
        tx.verdict_kind = crate::types::ConstitutionalVerdict::RigorousNegative;
        tx.native_cycle = Some(crate::native_cycle::NativeCycleSnapshot {
            request_sha256: [8; 32], response: vec![], prepared: vec![],
            kie_proof: encode_proof(proof).unwrap(), model_judgment: None,
        });
        tx.knowledge_writes = vec![KnowledgeWrite::Negative {
            input_id: 7, raw_bytes: proof.canonical_proposition.as_bytes().to_vec(),
            context: context.clone(), ts_ns: 1,
        }];
        let entry = crate::knowledge_vault::NegativeEntry {
            input_id: 7, failure_reason_code: context.failure_reason_code,
            process: context.process, mass_at_fail: context.mass_at_fail,
            frequency: context.frequency,
            raw_bytes: proof.canonical_proposition.as_bytes().to_vec(),
            access_count: 0, added_at: 1,
        };
        (tx, entry)
    }

    #[test]
    fn r35_same_content_keeps_context_provenance_without_repeated_scoped_evidence() {
        let (proof_a, _) = test_support::empty_current_projection();
        let mut proof_b = proof_a.clone();
        proof_b.context.identity.case_id = "case-b".into();
        proof_b.context.operation_id = "fixture-op-b".into();
        proof_b.context.record_id = [12; 32];
        let mut query_b = science_domains::decode_query(&proof_b.query_bytes).unwrap();
        query_b.session_id = pd_memory::hex(&proof_b.context.identity.key().unwrap());
        query_b.case_id = proof_b.context.identity.case_id.clone();
        proof_b.query_bytes = science_domains::encode_query(&query_b).unwrap();
        proof_b.candidate_bytes =
            science_domains::encode_candidate(&science_domains::evaluate(&query_b).unwrap()).unwrap();
        let result_b =
            science_domains::kie::evaluate_kie(&proof_b.query_bytes, &proof_b.candidate_bytes).unwrap();
        assert_eq!(result_b.canonical_proposition, proof_a.canonical_proposition);
        proof_b.history_assessment = Some(HistoryIndex::default().assess(&proof_b).unwrap());
        let proof_b = proof_b.seal_data().unwrap();
        assert_ne!(proof_a.context.identity.key().unwrap(), proof_b.context.identity.key().unwrap());
        assert_ne!(proof_a.proof_sha256, proof_b.proof_sha256);

        let (tx_a, entry_a) = r35_counterproof_transaction(&proof_a, [21; 32], 2);
        let (tx_b, entry_b) = r35_counterproof_transaction(&proof_b, [22; 32], 3);
        assert_eq!(entry_a.raw_bytes, entry_b.raw_bytes);
        let pending_a = prepare(&tx_a).unwrap().remove(0);
        let pending_b = prepare(&tx_b).unwrap().remove(0);
        let origin_a = pending_a.origin.as_ref().unwrap().clone();
        let origin_b = pending_b.origin.as_ref().unwrap().clone();
        assert_eq!(origin_a.class, KieHistoryClass::ExactModelCounterproof);
        assert_eq!(origin_b.class, KieHistoryClass::ExactModelCounterproof);
        assert_eq!(origin_a.family, origin_b.family);
        let mut index = HistoryIndex::default();
        index.publish_negative(pending_a, &entry_a);
        let assessment_a = index.assess(&proof_a).unwrap();
        index.publish_negative(pending_b, &entry_b);
        let assessment_b = index.assess(&proof_b).unwrap();

        let mut repeated = proof_b.clone();
        repeated.context.operation_id = "fixture-op-b-repeat".into();
        repeated.context.record_id = [13; 32];
        repeated.history_assessment = Some(assessment_b.clone());
        let repeated = repeated.seal_data().unwrap();
        let (repeated_tx, _) = r35_counterproof_transaction(&repeated, [23; 32], 9);
        index.publish_negative(prepare(&repeated_tx).unwrap().remove(0), &entry_b);

        assert_eq!(index.assess(&proof_a).unwrap(), assessment_a);
        assert_eq!(index.assess(&proof_b).unwrap(), assessment_b);
        assert_eq!(assessment_a.scoped_evidence.len(), 1);
        assert_eq!(assessment_b.scoped_evidence.len(), 1);
        assert_eq!(assessment_a.scoped_evidence[0].provenance, origin_a.provenance);
        assert_eq!(assessment_b.scoped_evidence[0].provenance, origin_b.provenance);
        assert_eq!(index.classify(
            origin_a.scoped_candidate.as_ref().unwrap(),
            origin_b.scope, origin_b.proposition, origin_b.family,
        ).class, KieHistoryClass::DifferentScope);

        for (mut proof, tx, frequency) in [(proof_a, &tx_a, 2), (proof_b, &tx_b, 3)] {
            let assessment = index.assess(&proof).unwrap();
            assert_eq!(assessment.scoped_evidence[0].provenance.transaction_id, tx.transaction_id);
            assert_eq!(assessment.scoped_evidence[0].provenance.proof_sha256, proof.proof_sha256);
            proof.history_assessment = Some(assessment);
            let projection = index.project_presented_counterproofs(&proof.seal_data().unwrap()).unwrap();
            assert_eq!(projection.metrics.len(), 1);
            assert_eq!(projection.metrics[0].frequency, frequency);
            assert_eq!(projection.metrics[0].mass_bits, 0.500_037f32.to_bits());
            assert_eq!(projection.metrics[0].write_sha256, tx.knowledge_writes[0].digest());
        }
    }

    #[test]
    fn r35_wrong_row_bytes_cannot_publish_prepared_counterproof_provenance() {
        let (mut proof, _) = test_support::empty_current_projection();
        let (tx, mut wrong_entry) = r35_counterproof_transaction(&proof, [31; 32], 3);
        let pending = prepare(&tx).unwrap().remove(0);
        let origin = pending.origin.as_ref().unwrap().clone();
        assert_eq!(origin.class, KieHistoryClass::ExactModelCounterproof);
        assert!(origin.scoped_candidate.is_some());
        assert!(origin.negative_metric.is_some());
        wrong_entry.raw_bytes = b"different canonical object".to_vec();
        assert_ne!(pending.raw_sha, digest(&wrong_entry.raw_bytes));

        let mut index = HistoryIndex::default();
        index.publish_negative(pending, &wrong_entry);
        assert!(index.scoped.is_empty());
        assert!(index.negative_origins.is_empty());
        let key = key_for(0, wrong_entry.input_id, &wrong_entry.raw_bytes,
            &wrong_entry.process, wrong_entry.failure_reason_code);
        let mut wrong_candidate = origin.scoped_candidate.unwrap();
        wrong_candidate.source_sha256 = key.2;
        wrong_candidate.term_sha256 = key.4;
        let classified = index.classify(
            &wrong_candidate, origin.scope, origin.proposition, origin.family,
        );
        assert_eq!(classified.class, KieHistoryClass::LegacyUnscoped);
        assert!(classified.provenance.is_none());
        proof.history_assessment = Some(index.assess(&proof).unwrap());
        let proof = proof.seal_data().unwrap();
        let projection = index.project_presented_counterproofs(&proof).unwrap();
        assert!(projection.metrics.is_empty());
        assert_eq!(projection.negative_pattern(0.500_037).unwrap(), 0.0);
        assert!(projection.diagnostic_context().is_none());
    }

    #[test]
    fn r42_distinct_negative_contexts_survive_history_assessment() {
        let (mut proof, _) = test_support::empty_current_projection();
        let request = shadow_contracts::decode_selection_request(&proof.historical_request).unwrap();
        let vault = crate::knowledge_vault::KnowledgeVault::new();
        for (process, ts_ns) in [("r42-context-a", 1), ("r42-context-b", 2)] {
            vault.write_negative(7, proof.canonical_proposition.as_bytes().to_vec(),
                crate::types::NegativeContext {
                    failure_reason_code: 0x101, process: process.into(),
                    mass_at_fail: 0.5, frequency: 1,
                }, ts_ns).unwrap();
        }
        assert_eq!(vault.negative_count(), 2);
        let (positive, negative) = vault.select_bounded_candidates(&request).unwrap();
        assert!(positive.is_empty());
        assert_eq!(negative.len(), 2);
        assert_eq!(negative[0].candidate_id, negative[1].candidate_id);
        assert_eq!(negative[0].source_sha256, negative[1].source_sha256);
        assert_eq!(negative[0].term_sha256, negative[1].term_sha256);
        assert_ne!(negative[0].domain, negative[1].domain);
        let mut response = shadow_contracts::decode_selection_response(&proof.historical_response).unwrap();
        response.negative = negative;
        response.selection_sha256 = pd_memory::failure::relevance::selection_digest(&response);
        proof.historical_response = shadow_contracts::encode_selection_response(&response);
        proof.history_assessment = None;

        let assessment = vault.kie_history_assessment(&proof)
            .expect("distinct negative contexts remain separately classified");
        assert_eq!(assessment.entries.len(), 2);
        assert!(assessment.entries.iter().all(|entry|
            entry.class == KieHistoryClass::LegacyUnscoped && entry.provenance.is_none()));
        assert!(assessment.scoped_evidence.is_empty());
        proof.history_assessment = Some(assessment);
        let proof = proof.seal_data().unwrap();
        let combined = shadow_contracts::kie::combined_evaluation(&proof).unwrap();
        assert_eq!(combined.historical_negative_count, 0);
        assert_eq!(combined.unresolved_history_count, 2);
    }

    #[test]
    fn r42_duplicate_source_contexts_keep_exact_current_provenance() {
        let (proof_a, _) = test_support::empty_current_projection();
        let mut proof_b = proof_a.clone();
        proof_b.context.identity.case_id = "r42-case-b".into();
        proof_b.context.operation_id = "r42-op-b".into();
        proof_b.context.record_id = [42; 32];
        let mut query_b = science_domains::decode_query(&proof_b.query_bytes).unwrap();
        query_b.session_id = pd_memory::hex(&proof_b.context.identity.key().unwrap());
        query_b.case_id = proof_b.context.identity.case_id.clone();
        proof_b.query_bytes = science_domains::encode_query(&query_b).unwrap();
        proof_b.candidate_bytes =
            science_domains::encode_candidate(&science_domains::evaluate(&query_b).unwrap()).unwrap();
        let result_b =
            science_domains::kie::evaluate_kie(&proof_b.query_bytes, &proof_b.candidate_bytes).unwrap();
        assert_eq!(result_b.canonical_proposition, proof_a.canonical_proposition);
        proof_b.history_assessment = Some(HistoryIndex::default().assess(&proof_b).unwrap());
        let proof_b = proof_b.seal_data().unwrap();
        let (tx_a, entry_a) = r35_counterproof_transaction(&proof_a, [41; 32], 2);
        let (tx_b, entry_b) = r35_counterproof_transaction(&proof_b, [42; 32], 3);
        let pending_a = prepare(&tx_a).unwrap().remove(0);
        let pending_b = prepare(&tx_b).unwrap().remove(0);
        let origin_a = pending_a.origin.as_ref().unwrap().clone();
        let origin_b = pending_b.origin.as_ref().unwrap().clone();
        let candidate_a = origin_a.scoped_candidate.as_ref().unwrap().clone();
        let candidate_b = origin_b.scoped_candidate.as_ref().unwrap().clone();
        assert_ne!(origin_a.scope, origin_b.scope);
        assert_eq!(origin_a.family, origin_b.family);
        assert_eq!(candidate_a.source_sha256, candidate_b.source_sha256);
        assert_eq!(candidate_a.term_sha256, candidate_b.term_sha256);
        assert_eq!(candidate_a.candidate_id, candidate_b.candidate_id);
        assert_ne!(candidate_a.domain, candidate_b.domain);
        let mut index = HistoryIndex::default();
        index.publish_negative(pending_a, &entry_a);
        index.publish_negative(pending_b, &entry_b);

        for (mut proof, tx, current, foreign, frequency) in [
            (proof_a, &tx_a, &origin_a, &origin_b, 2),
            (proof_b, &tx_b, &origin_b, &origin_a, 3),
        ] {
            let mut response = shadow_contracts::decode_selection_response(&proof.historical_response).unwrap();
            response.negative = vec![candidate_a.clone(), candidate_b.clone()];
            response.selection_sha256 = pd_memory::failure::relevance::selection_digest(&response);
            proof.historical_response = shadow_contracts::encode_selection_response(&response);
            proof.history_assessment = None;
            let assessment = index.assess(&proof)
                .expect("one source may retain separately scoped historical rows");
            assert_eq!(assessment.entries.len(), 2);
            let exact = assessment.entries.iter()
                .filter(|entry| entry.class == KieHistoryClass::ExactModelCounterproof)
                .collect::<Vec<_>>();
            let different = assessment.entries.iter()
                .filter(|entry| entry.class == KieHistoryClass::DifferentScope)
                .collect::<Vec<_>>();
            assert_eq!(exact.len(), 1);
            assert_eq!(different.len(), 1);
            assert_eq!(exact[0].provenance.as_ref(), Some(&current.provenance));
            assert_eq!(different[0].provenance.as_ref(), Some(&foreign.provenance));
            assert_eq!(assessment.scoped_evidence.len(), 1);
            assert_eq!(assessment.scoped_evidence[0].provenance, current.provenance);
            proof.history_assessment = Some(assessment);
            let proof = proof.seal_data().unwrap();
            shadow_contracts::kie::with_scoped_semantic_candidates(&proof, |positive, negative| {
                assert!(positive.is_empty());
                assert_eq!(negative.len(), 1);
                assert_eq!(negative[0].domain, current.scoped_candidate.as_ref().unwrap().domain);
                assert_eq!(negative[0].frequency, frequency);
            }).unwrap();
            let projection = index.project_presented_counterproofs(&proof).unwrap();
            assert_eq!(projection.metrics.len(), 1);
            assert_eq!(projection.metrics[0].frequency, frequency);
            assert_eq!(projection.metrics[0].mass_bits, 0.500_037f32.to_bits());
            assert_eq!(projection.metrics[0].write_sha256, tx.knowledge_writes[0].digest());
            let combined = shadow_contracts::kie::combined_evaluation(&proof).unwrap();
            assert_eq!(combined.historical_negative_count, 1);
            assert_eq!(combined.unresolved_history_count, 0);
        }
    }

    fn contextual_counterproof_fixture() -> (HistoryIndex, KieProof) {
        let (proof_a, _) = test_support::empty_current_projection();
        let mut proof_b = proof_a.clone();
        proof_b.context.identity.case_id = "context-codec-b".into();
        proof_b.context.operation_id = "context-codec-op-b".into();
        proof_b.context.record_id = [52; 32];
        let mut query = science_domains::decode_query(&proof_b.query_bytes).unwrap();
        query.session_id = pd_memory::hex(&proof_b.context.identity.key().unwrap());
        query.case_id = proof_b.context.identity.case_id.clone();
        proof_b.query_bytes = science_domains::encode_query(&query).unwrap();
        proof_b.candidate_bytes =
            science_domains::encode_candidate(&science_domains::evaluate(&query).unwrap()).unwrap();
        proof_b.history_assessment = Some(HistoryIndex::default().assess(&proof_b).unwrap());
        let proof_b = proof_b.seal_data().unwrap();
        let (tx_a, entry_a) = r35_counterproof_transaction(&proof_a, [51; 32], 2);
        let (tx_b, entry_b) = r35_counterproof_transaction(&proof_b, [52; 32], 3);
        let pending_a = prepare(&tx_a).unwrap().remove(0);
        let pending_b = prepare(&tx_b).unwrap().remove(0);
        let candidates = vec![
            pending_a.origin.as_ref().unwrap().scoped_candidate.as_ref().unwrap().clone(),
            pending_b.origin.as_ref().unwrap().scoped_candidate.as_ref().unwrap().clone(),
        ];
        let mut index = HistoryIndex::default();
        index.publish_negative(pending_a, &entry_a);
        index.publish_negative(pending_b, &entry_b);
        let mut proof = proof_a;
        let mut response = shadow_contracts::decode_selection_response(&proof.historical_response).unwrap();
        response.negative = candidates;
        response.selection_sha256 = pd_memory::failure::relevance::selection_digest(&response);
        proof.historical_response = shadow_contracts::encode_selection_response(&response);
        proof.history_assessment = Some(index.assess(&proof).unwrap());
        (index, proof.seal_data().unwrap())
    }

    #[test]
    fn context_history_recomputed_legacy_assessment_preserves_proof_bytes() {
        let (index, mut proof, _) = presented_fixture(KieHistoryClass::ExactModelCounterproof);
        let original = encode_proof(&proof).unwrap();
        let assessment = index.assess(&proof).unwrap();
        assert_eq!(Some(&assessment), proof.history_assessment.as_ref());
        assert!(assessment.encode().unwrap().starts_with(b"KHA1"));
        assert!(assessment.entries.iter().all(|entry| entry.selection_context.is_none()));
        proof.history_assessment = Some(assessment);
        let recomputed = proof.seal_data().unwrap();
        assert_eq!(encode_proof(&recomputed).unwrap(), original);
        assert_eq!(decode_proof(&original).unwrap(), recomputed);
        assert_eq!(index.project_presented_counterproofs(&recomputed).unwrap().metrics.len(), 1);
    }

    #[test]
    fn context_history_rejects_rehashed_class_and_context_substitution() {
        let (index, proof) = contextual_counterproof_fixture();
        let original = proof.history_assessment.as_ref().unwrap();
        assert!(original.encode().unwrap().starts_with(b"KHA2"));
        assert_eq!(decode_proof(&encode_proof(&proof).unwrap()).unwrap(), proof);
        for field in 0..2 {
            let mut uncovered = proof.clone();
            let context = uncovered.history_assessment.as_mut().unwrap().entries[0]
                .selection_context.as_mut().unwrap();
            if field == 0 { context.domain.push('!'); }
            else { context.term_sha256 = "f".repeat(64); }
            assert!(uncovered.seal_data().unwrap_err().contains("KIE_HISTORY_ASSESSMENT_COVERAGE"));
        }

        let mut relabeled = proof.clone();
        let entries = &mut relabeled.history_assessment.as_mut().unwrap().entries;
        let a = entries[0].clone();
        let b = entries[1].clone();
        entries[0].class = b.class;
        entries[0].provenance = b.provenance;
        entries[1].class = a.class;
        entries[1].provenance = a.provenance;
        let relabeled = relabeled.seal_data().unwrap();
        assert_eq!(index.project_presented_counterproofs(&relabeled).unwrap_err(),
            "KIE_CURRENT_ACTUAL_SOURCE_MISMATCH");

        let mut substituted = proof.clone();
        let mut response = shadow_contracts::decode_selection_response(&substituted.historical_response).unwrap();
        let original_domain = response.negative[0].domain.clone();
        response.negative[0].domain.push('!');
        let changed_domain = response.negative[0].domain.clone();
        response.selection_sha256 = pd_memory::failure::relevance::selection_digest(&response);
        substituted.historical_response = shadow_contracts::encode_selection_response(&response);
        let entries = &mut substituted.history_assessment.as_mut().unwrap().entries;
        entries.iter_mut().find(|entry|
            entry.selection_context.as_ref().unwrap().domain == original_domain).unwrap()
            .selection_context.as_mut().unwrap().domain = changed_domain;
        entries.sort_by(|a, b| a.selection_key().cmp(&b.selection_key()));
        let substituted = substituted.seal_data().unwrap();
        assert_eq!(index.project_presented_counterproofs(&substituted).unwrap_err(),
            "KIE_CURRENT_ACTUAL_SOURCE_MISMATCH");
        assert_eq!(index.assess(&proof).unwrap(), *original);
        assert_eq!(index.project_presented_counterproofs(&proof).unwrap().metrics.len(), 1);
    }

    #[test]
    fn context_history_order_preserves_one_original_scientific_source() {
        let (index, mut proof) = contextual_counterproof_fixture();
        let original_assessment = proof.history_assessment.clone().unwrap();
        let original_projection = index.project_presented_counterproofs(&proof).unwrap();
        let mut response = shadow_contracts::decode_selection_response(&proof.historical_response).unwrap();
        response.negative.reverse();
        response.selection_sha256 = pd_memory::failure::relevance::selection_digest(&response);
        proof.historical_response = shadow_contracts::encode_selection_response(&response);
        let actual = index.assess(&proof).unwrap();
        assert_eq!(actual.encode().unwrap(), original_assessment.encode().unwrap());
        proof.history_assessment = Some(actual);
        let proof = proof.seal_data().unwrap();
        let projection = index.project_presented_counterproofs(&proof).unwrap();
        assert_eq!(projection.metrics, original_projection.metrics);
        assert_eq!(projection.metrics.len(), 1);
        assert_eq!(projection.metrics[0].frequency, 2);
        assert_eq!(projection.metrics[0].mass_bits, 0.500_037f32.to_bits());
        assert_eq!(shadow_contracts::kie::combined_evaluation(&proof).unwrap().historical_negative_count, 1);
    }
}
