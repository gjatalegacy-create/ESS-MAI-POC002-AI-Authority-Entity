//! NEW operational projection over the existing KnowledgeVault WAL.
//! No epistemic verdict, knowledge write, authority token or separate persistence.
//! Only KnowledgeVault commits this projection after the same backend sync.
use shadow_contracts::pd_memory::{Hash, PdIdentity, PdOperationRecord, PdOperationRequest};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Default)]
pub(crate) struct PdProjection {
    heads: HashMap<Hash, Arc<PdOperationRecord>>,
    // Scoped revision references to the same Arc-owned Vault records. No payload copy.
    revisions: HashMap<(Hash, u64), Arc<PdOperationRecord>>,
    operations: HashMap<(Hash, String), Arc<PdOperationRecord>>,
    candidates: HashMap<(Hash, Hash), Arc<PdOperationRecord>>,
}

pub(crate) enum PdCommitPlan {
    Existing(Arc<PdOperationRecord>),
    Append(PdAppend),
}

pub(crate) struct PdAppend {
    key: Hash,
    operation_key: (Hash, String),
    revision_key: (Hash, u64),
    candidate_key: Option<(Hash, Hash)>,
    record: Arc<PdOperationRecord>,
}

impl PdAppend {
    pub(crate) fn record(&self) -> &PdOperationRecord {
        &self.record
    }
}

fn operation_valid(operation_id: &str) -> Result<(), String> {
    match operation_id.trim().is_empty()
        || operation_id.len() > 1024
        || operation_id.chars().any(|value| value == '\0')
    {
        true => Err("PD_OPERATION_ID_INVALID".into()),
        false => Ok(()),
    }
}

impl PdProjection {
    pub(crate) fn head(&self, identity: &PdIdentity) -> Result<Option<PdOperationRecord>, String> {
        self.clone_scoped(identity, self.heads.get(&identity.key()?))
    }

    /// Stable cognitive sessions for operational PD are exact identity hashes.
    /// Non-operational legacy native sessions have no matching projection key.
    pub(crate) fn head_for_cognitive_session(&self, session: &str, territory: &str)
        -> Result<Option<PdOperationRecord>, String> {
        let Ok(key) = shadow_contracts::pd_memory::parse_hash(session) else { return Ok(None) };
        if shadow_contracts::pd_memory::hex(&key) != session { return Ok(None); }
        match self.heads.get(&key) {
            None => Ok(None),
            Some(head) if head.request.identity.key()? == key && head.request.identity.scope == territory =>
                Ok(Some(head.as_ref().clone())),
            Some(_) => Err("PD_COGNITIVE_SCOPE_COLLISION".into()),
        }
    }

    /// Exact historical reference; current-head CAS remains the admission caller's duty.
    /// Resolves the existing revision Arc, never guesses a topic or copies into a new store.
    pub(crate) fn lgc_record(&self, reference:&shadow_contracts::pd_memory::lgc::PdLgcRef)
        ->Result<Option<PdOperationRecord>,String> {
        reference.validate()?;
        match self.revisions.get(&(reference.identity_key,reference.revision)) {
            None=>Ok(None),
            Some(record)=>{reference.validate_record(record)?;Ok(Some(record.as_ref().clone()))},
        }
    }

    pub(crate) fn history(&self, query:&shadow_contracts::pd_memory::history::PdHistoryLookup)->Result<shadow_contracts::pd_memory::history::PdHistoryPage,String> {
        use shadow_contracts::pd_memory::history::{PdHistoryPage,PdMapEntry};
        query.validate()?;
        let key=query.identity.key()?;
        let head_revision=match self.heads.get(&key){
            Some(head) if head.request.identity==query.identity=>head.revision,
            Some(_)=>return Err("PD_IDENTITY_COLLISION".into()),None=>0,
        };
        let count=if query.from_revision>head_revision{0}else{(head_revision-query.from_revision+1).min(u64::from(query.limit)) as usize};
        let mut entries=Vec::new();entries.try_reserve_exact(count).map_err(|_|"PD_HISTORY_ALLOCATION")?;
        for offset in 0..count {
            let revision=query.from_revision.checked_add(offset as u64).ok_or("PD_HISTORY_REVISION_OVERFLOW")?;
            let record=self.revisions.get(&(key,revision)).ok_or("PD_HISTORY_MAP_REFERENCE_MISSING")?;
            if record.request.identity!=query.identity || record.revision!=revision {return Err("PD_HISTORY_MAP_REFERENCE_MISMATCH".into());}
            entries.push(PdMapEntry::from_validated_record(record)?);
        }
        let next_revision=match entries.last(){Some(entry) if entry.revision<head_revision=>Some(entry.revision.checked_add(1).ok_or("PD_HISTORY_REVISION_OVERFLOW")?),_=>None};
        let page=PdHistoryPage{query:query.clone(),head_revision,entries,next_revision};page.validate()?;Ok(page)
    }

    pub(crate) fn operation(
        &self,
        identity: &PdIdentity,
        operation_id: &str,
    ) -> Result<Option<PdOperationRecord>, String> {
        operation_valid(operation_id)?;
        self.clone_scoped(
            identity,
            self.operations
                .get(&(identity.key()?, operation_id.to_string())),
        )
    }

    pub(crate) fn candidate(
        &self,
        identity: &PdIdentity,
        query_digest: &Hash,
    ) -> Result<Option<PdOperationRecord>, String> {
        match query_digest == &[0; 32] {
            true => return Err("PD_CANDIDATE_QUERY_UNBOUND".into()),
            false => {}
        }
        self.clone_scoped(
            identity,
            self.candidates.get(&(identity.key()?, *query_digest)),
        )
    }

    fn clone_scoped(
        &self,
        identity: &PdIdentity,
        record: Option<&Arc<PdOperationRecord>>,
    ) -> Result<Option<PdOperationRecord>, String> {
        match record {
            Some(record) if &record.request.identity == identity => {
                Ok(Some(record.as_ref().clone()))
            }
            Some(_) => Err("PD_IDENTITY_COLLISION".into()),
            None => Ok(None),
        }
    }

    /// Caller holds the shared KnowledgeVault commit lock and this projection's
    /// write lock until durable append and publication finish.
    pub(crate) fn prepare(&mut self, request: PdOperationRequest) -> Result<PdCommitPlan, String> {
        request.validate()?;
        let key = request.identity.key()?;
        let operation_key = (key, request.operation_id.clone());
        let request_digest = request.request_digest()?;
        // Deliberately precedes CAS: an exact retry remains historical success
        // after another operation has advanced the same head.
        match self.operations.get(&operation_key) {
            Some(prior) => {
                return match prior.request_digest == request_digest && prior.request == request {
                    true => Ok(PdCommitPlan::Existing(Arc::clone(prior))),
                    false => Err("PD_OPERATION_CONFLICT".into()),
                };
            }
            None => {}
        }
        let previous_record = match self.heads.get(&key) {
            Some(head) => {
                match head.request.identity == request.identity
                    && request.expected_revision == head.revision
                    && request.previous_state == head.state_digest
                {
                    true => head.record_id,
                    false => return Err("PD_SCOPED_CAS_CONFLICT".into()),
                }
            }
            None => match request.expected_revision == 0 && request.previous_state == [0; 32] {
                true => [0; 32],
                false => return Err("PD_SCOPED_CAS_MISSING_HEAD".into()),
            },
        };
        // pd_mp validates references to Vault-owned dialogue material.
        // A direct storage request cannot skip the parent-question rule.
        if shadow_contracts::pd_memory::late_refusal::source_is_late(&request.source_input) {
            let parent=self.heads.get(&key).ok_or("PD_LATE_PARENT_MISSING")?;
            shadow_contracts::pd_memory::late_refusal::validate_transition(&request,parent)?;
        }
        if request.source_input.starts_with(b"PDF1") || request.source_input.starts_with(b"PDS1") {
            use shadow_contracts::pd_memory::failure::validate_failure_operation;
            use shadow_contracts::pd_memory::linked_science::{
                decode_dialogue_source, decode_linked_input, validate_linked_parent,
            };
            let input = validate_failure_operation(&request)?;
            match self.heads.get(&key) {
                None if input.in_reply_to == [0; 32] => {}
                Some(head) if input.in_reply_to == head.record_id => {
                    let original = decode_dialogue_source(&head.request.source_input)?;
                    if request.source_input.starts_with(b"PDS1") {
                        validate_linked_parent(&decode_linked_input(&request.source_input)?, head)?;
                    }
                    if input.selection_request != original.selection_request
                        || input.selection_response != original.selection_response
                        || input.refusal != original.refusal
                    {
                        return Err("PD_FAILURE_ORIGINAL_MATERIAL_CHANGED".into());
                    }
                }
                _ => return Err("PD_FAILURE_PARENT_QUESTION_MISMATCH".into()),
            }
        }
        if shadow_contracts::pd_memory::iz_science::source_is_science(&request.source_input) {
            let parent=self.heads.get(&key).ok_or("PDS2_PARENT_MISSING")?;
            shadow_contracts::pd_memory::iz_science::validate_transition(&request,parent)?;
            crate::pd_failure_admission::validate_pds2_candidate(&request)?;
        }
        if shadow_contracts::pd_memory::iz::source_is_iz(&request.source_input) {
            let source = shadow_contracts::pd_memory::iz::decode_source(&request.source_input)?;
            let head = self.heads.get(&key).map(|v| v.as_ref());
            if source.context.version != shadow_contracts::pd_memory::iz::IZ_NATIVE_CONTINUITY_VERSION {
                shadow_contracts::pd_memory::iz::validate_parent(&source, head)?;
                crate::pd_iz_admission::validate_checkpoint_transition(
                    &source.context,&request.checkpoint,request.expected_revision,head)
                    .map_err(|e| format!("IZ_REPLAY_CHECKPOINT_TRANSITION:{e}"))?;
            }
            // V3 crosses both reference domains. KnowledgeVault validates its
            // actual native T1/T2 + operational checkpoint under commit_lock,
            // on live append AND replay before publishing this prepared record.
        }
        let record = Arc::new(PdOperationRecord::construct(request, previous_record)?);
        let candidate_key = match record.request.candidate.is_empty() {
            true => None,
            false => Some((key, record.request.candidate_key)),
        };
        let revision_key = (key, record.revision);
        if self.revisions.contains_key(&revision_key) { return Err("PD_HISTORY_DUPLICATE_REVISION".into()); }
        self.revisions.try_reserve(1).map_err(|_| "PD_HISTORY_INDEX_ALLOCATION")?;
        // Reserve map growth before append. Publication only inserts already
        // allocated keys and Arc references; it does not clone checkpoint bytes.
        self.heads
            .try_reserve(1)
            .map_err(|_| "PD_HEAD_ALLOCATION")?;
        self.operations
            .try_reserve(1)
            .map_err(|_| "PD_OPERATION_ALLOCATION")?;
        match candidate_key {
            Some(_) => self
                .candidates
                .try_reserve(1)
                .map_err(|_| "PD_CANDIDATE_ALLOCATION")?,
            None => {}
        }
        Ok(PdCommitPlan::Append(PdAppend {
            key,
            operation_key,
            revision_key,
            candidate_key,
            record,
        }))
    }

    pub(crate) fn publish(&mut self, append: PdAppend) -> Arc<PdOperationRecord> {
        let PdAppend {
            key,
            operation_key,
            revision_key,
            candidate_key,
            record,
        } = append;
        self.heads.insert(key, Arc::clone(&record));
        self.operations.insert(operation_key, Arc::clone(&record));
        self.revisions.insert(revision_key, Arc::clone(&record));
        match candidate_key {
            Some(key) => {
                self.candidates.insert(key, Arc::clone(&record));
            }
            None => {}
        }
        record
    }

    /// The record's internal hash is necessary but not sufficient: replay also
    /// rechecks scope, sequence, previous state and exact previous-record chain.
    #[cfg(test)]
    pub(crate) fn replay(&mut self, record: PdOperationRecord) -> Result<(), String> {
        record.validate()?;
        match self.prepare(record.request.clone())? {
            PdCommitPlan::Existing(prior) => match prior.as_ref() == &record {
                true => Ok(()),
                false => Err("PD_REPLAY_OPERATION_CONFLICT".into()),
            },
            PdCommitPlan::Append(append) => match append.record() == &record {
                true => {
                    self.publish(append);
                    Ok(())
                }
                false => Err("PD_REPLAY_CHAIN_CONFLICT".into()),
            },
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::knowledge_vault::KnowledgeVault;
    use shadow_contracts::pd_memory::{ConditionWitness, LgcStep, NextAction, ReportClass};
    use std::path::PathBuf;

    #[test]
    fn lgc_resolution_never_guesses_a_head_or_returns_another_source_policy(){
        use shadow_contracts::pd_memory::lgc::PdLgcRef;
        let mut projection=PdProjection::default();
        let record=Arc::new(PdOperationRecord::construct(request("exact-data-only"),[0;32]).unwrap());
        let key=record.request.identity.key().unwrap();
        let reference=PdLgcRef{identity_key:key,record_id:record.record_id,revision:1,
            state_digest:record.state_digest,question_sha256:[4;32],lgc_tip:[5;32],
            user_input_sha256:[6;32],question_binding:[7;32]};
        // A current head alone is never a substitute for the exact revision index.
        projection.heads.insert(key,Arc::clone(&record));
        assert!(projection.lgc_record(&reference).unwrap().is_none());
        projection.revisions.insert((key,1),record);
        // Matching key/revision is insufficient: PDC1 must not masquerade as PIZ.
        assert!(projection.lgc_record(&reference).is_err());
        let mut foreign=reference.clone();foreign.identity_key=[88;32];
        assert!(projection.lgc_record(&foreign).unwrap().is_none());
        let mut wrong_revision=reference;wrong_revision.revision=2;
        assert!(projection.lgc_record(&wrong_revision).unwrap().is_none());
        assert_eq!(projection.revisions.len(),1);
    }

    pub(crate) fn request(operation_id: &str) -> PdOperationRequest {
        let step = LgcStep::new(0, [2; 32], "storage fixture", "witness conditions", [4; 32])
            .expect("fixture LGC");
        let mut request = PdOperationRequest {
            identity: PdIdentity {
                owner: "storage-owner".into(),
                obligation: "obligation-A".into(),
                case_id: "case-A".into(),
                scope: "scope-A".into(),
                domain: "astronomy".into(),
                lineage_root: "root-A".into(),
            },
            operation_id: operation_id.into(),
            source_input: Vec::new(),
            expected_revision: 0,
            previous_state: [0; 32],
            witness: ConditionWitness {
                predecessor: "ASHT".into(),
                parent_authority: "GCL".into(),
                failed_action: "prior confrontation".into(),
                condition_code: "MISSING_PRIOR".into(),
                reason: "no exact prior pair".into(),
                source_component: "quantum".into(),
                source_sha256: [1; 32],
                attempt_trace: "attempt-A".into(),
                input_sha256: [2; 32],
                lgc_parent: step.step_digest,
                report_class: ReportClass::MissingConditions,
                permitted_next: NextAction::AcquireScopedEvidence,
            },
            checkpoint: vec![1, 2, 3],
            scoped_question: "Which scoped source is still missing?".into(),
            candidate: Vec::new(),
            candidate_key: [0; 32],
            lgc_steps: vec![step],
        };
        bind_source(&mut request);
        request
    }

    fn bind_source(request: &mut PdOperationRequest) {
        use sha2::{Digest, Sha256};
        request.source_input = shadow_contracts::pd_memory::encode_continuity_input(
            &shadow_contracts::pd_memory::ContinuityInput {
                identity: request.identity.clone(),
                operation_id: request.operation_id.clone(),
                user_text: "Explicit operational storage test".into(),
                science_query: vec![1],
            },
        )
        .expect("fixture source input");
        request.witness.input_sha256 = Sha256::digest(&request.source_input).into();
        let step = LgcStep::new(
            0,
            request.witness.input_sha256,
            "storage fixture",
            "witness conditions",
            [4; 32],
        )
        .expect("fixture bound LGC");
        request.witness.lgc_parent = step.step_digest;
        request.lgc_steps = vec![step];
    }

    fn path(label: &str) -> PathBuf {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let occurrence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("test clock")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "essmai_pd_storage_{label}_{}_{time}_{occurrence}.wal",
            std::process::id()
        ))
    }

    fn after(prior: &PdOperationRecord, op: &str) -> PdOperationRequest {
        let mut next = prior.request.clone();
        next.operation_id = op.into();
        next.expected_revision = prior.revision;
        next.previous_state = prior.state_digest;
        next.checkpoint.push(9);
        bind_source(&mut next);
        next
    }

    fn counts(vault: &KnowledgeVault) -> [usize; 7] {
        vault.store_counts_for_test()
    }

    #[test]
    fn pd_reopen_exact_retry_after_advanced_head_and_no_knowledge_pollution() {
        let file = path("reopen");
        let first = request("operation-1");
        let identity = first.identity.clone();
        let (saved_first, saved_second);
        {
            let vault = KnowledgeVault::open_disk(&file).expect("open");
            saved_first = vault.commit_pd_operation(first.clone()).expect("first");
            saved_second = vault
                .commit_pd_operation(after(&saved_first, "operation-2"))
                .expect("second");
            let before = std::fs::metadata(&file).expect("length").len();
            assert_eq!(
                vault
                    .commit_pd_operation(first.clone())
                    .expect("historical retry"),
                saved_first
            );
            assert_eq!(std::fs::metadata(&file).expect("length").len(), before);
            assert_eq!(
                vault.pd_head(&identity).expect("head"),
                Some(saved_second.clone())
            );
            assert_eq!(counts(&vault), [0; 7]);
        }
        {
            let vault = KnowledgeVault::open_disk(&file).expect("reopen");
            assert_eq!(vault.pd_head(&identity).expect("head"), Some(saved_second));
            assert_eq!(
                vault
                    .pd_operation(&identity, "operation-1")
                    .expect("history"),
                Some(saved_first.clone())
            );
            let before = std::fs::metadata(&file).expect("length").len();
            assert_eq!(
                vault.commit_pd_operation(first).expect("restart retry"),
                saved_first
            );
            assert_eq!(std::fs::metadata(&file).expect("length").len(), before);
            assert_eq!(counts(&vault), [0; 7]);
        }
        std::fs::remove_file(file).expect("remove isolated WAL");
    }

    #[test]
    fn changed_operation_payload_and_stale_new_operation_are_rejected() {
        let file = path("conflict");
        {
            let vault = KnowledgeVault::open_disk(&file).expect("open");
            let q = request("one");
            let record = vault.commit_pd_operation(q.clone()).expect("first");
            let before = std::fs::metadata(&file).expect("length").len();
            let mut changed = q.clone();
            changed.scoped_question.push('?');
            assert!(vault.commit_pd_operation(changed).is_err());
            let mut stale = q;
            stale.operation_id = "two".into();
            bind_source(&mut stale);
            assert!(vault.commit_pd_operation(stale).is_err());
            assert_eq!(
                vault.pd_head(&record.request.identity).expect("head"),
                Some(record)
            );
            assert_eq!(std::fs::metadata(&file).expect("length").len(), before);
        }
        std::fs::remove_file(file).expect("remove isolated WAL");
    }

    #[test]
    fn candidate_lookup_is_exact_scope_query_and_newest_committed_match() {
        let file = path("candidate");
        let identity;
        let latest;
        {
            let vault = KnowledgeVault::open_disk(&file).expect("open");
            let mut q = request("candidate-1");
            q.candidate = b"unadmitted fixture candidate".to_vec();
            q.candidate_key = [5; 32];
            identity = q.identity.clone();
            let first = vault.commit_pd_operation(q).expect("first candidate");
            let mut second = after(&first, "candidate-2");
            second.candidate = b"newer unadmitted fixture candidate".to_vec();
            latest = vault.commit_pd_operation(second).expect("next candidate");
            let mut no_candidate = after(&latest, "checkpoint-only");
            no_candidate.candidate.clear();
            no_candidate.candidate_key = [0; 32];
            vault
                .commit_pd_operation(no_candidate)
                .expect("later checkpoint");
            assert_eq!(
                vault.pd_candidate(&identity, &[5; 32]).expect("candidate"),
                Some(latest.clone())
            );
            assert!(vault
                .pd_candidate(&identity, &[6; 32])
                .expect("other query")
                .is_none());
            let mut other = identity.clone();
            other.case_id = "case-B".into();
            assert!(vault
                .pd_candidate(&other, &[5; 32])
                .expect("other case")
                .is_none());
            assert!(vault.pd_head(&other).expect("other head").is_none());
            assert_eq!(counts(&vault), [0; 7]);
        }
        {
            let vault = KnowledgeVault::open_disk(&file).expect("reopen");
            assert_eq!(
                vault.pd_candidate(&identity, &[5; 32]).expect("candidate"),
                Some(latest)
            );
        }
        std::fs::remove_file(file).expect("remove isolated WAL");
    }

    #[test]
    fn same_operation_name_is_scoped_by_full_identity() {
        let file = path("scope");
        {
            let vault = KnowledgeVault::open_disk(&file).expect("open");
            let a = request("shared-name");
            let mut b = a.clone();
            b.identity.case_id = "case-B".into();
            bind_source(&mut b);
            let ra = vault.commit_pd_operation(a).expect("case A");
            let rb = vault.commit_pd_operation(b).expect("case B");
            assert_ne!(ra.record_id, rb.record_id);
            assert_eq!((ra.revision, rb.revision), (1, 1));
            assert_eq!(
                vault.pd_head(&ra.request.identity).expect("head A"),
                Some(ra)
            );
            assert_eq!(
                vault.pd_head(&rb.request.identity).expect("head B"),
                Some(rb)
            );
        }
        std::fs::remove_file(file).expect("remove isolated WAL");
    }

    #[test]
    fn in_memory_backend_refuses_pd_success_and_publishes_nothing() {
        let vault = KnowledgeVault::new();
        let q = request("non-durable");
        let identity = q.identity.clone();
        assert!(vault.commit_pd_operation(q).is_err());
        assert!(vault.pd_head(&identity).expect("head").is_none());
        assert_eq!(counts(&vault), [0; 7]);
    }

    #[test]
    fn replay_rejects_tamper_and_broken_chain_and_deduplicates_exact_records() {
        let first = PdOperationRecord::construct(request("one"), [0; 32]).expect("first");
        let second =
            PdOperationRecord::construct(after(&first, "two"), first.record_id).expect("second");
        let mut projection = PdProjection::default();
        projection.replay(first.clone()).expect("first replay");
        projection.replay(second.clone()).expect("second replay");
        projection
            .replay(first.clone())
            .expect("historical exact duplicate");
        assert_eq!(
            projection.head(&first.request.identity).expect("head"),
            Some(second.clone())
        );
        let mut changed = second.clone();
        changed.request.checkpoint.push(1);
        assert!(projection.replay(changed).is_err());
        let mut next = after(&second, "three");
        next.previous_state = first.state_digest;
        let wrong_state =
            PdOperationRecord::construct(next, second.record_id).expect("internally consistent");
        assert!(projection.replay(wrong_state).is_err());
        let wrong_parent = PdOperationRecord::construct(after(&second, "three"), first.record_id)
            .expect("internally consistent");
        assert!(projection.replay(wrong_parent).is_err());
        assert_eq!(
            projection.head(&first.request.identity).expect("head"),
            Some(second)
        );
    }

    #[test]
    fn knowledge_and_operational_records_share_one_wal_without_cross_writes() {
        let file = path("mixed");
        let identity = request("operational").identity;
        {
            let vault = KnowledgeVault::open_disk(&file).expect("open");
            vault
                .commit_test_transaction(crate::vault_transaction::test_support::sample_transaction())
                .expect("fixture knowledge transaction");
            let before = counts(&vault);
            vault
                .commit_pd_operation(request("operational"))
                .expect("PD record");
            assert_eq!(counts(&vault), before);
        }
        {
            let vault = KnowledgeVault::open_disk(&file).expect("reopen mixed WAL");
            assert_eq!(vault.primitive_count(), 1);
            assert_eq!(vault.negative_count(), 0);
            assert!(vault.pd_head(&identity).expect("head").is_some());
        }
        std::fs::remove_file(file).expect("remove isolated WAL");
    }
    #[test]
    fn history_is_bounded_paginated_scoped_and_rebuilt_from_same_wal() {
        use shadow_contracts::pd_memory::history::*;
        let file=path("history");let identity;let other_identity;let expected_first;let expected_last;
        {
            let vault=KnowledgeVault::open_disk(&file).unwrap();
            let mut initial=request("shared-operation");initial.candidate=b"unadmitted model fixture".to_vec();initial.candidate_key=[6;32];
            identity=initial.identity.clone();
            let mut other=initial.clone();other.identity.case_id="another-case".into();bind_source(&mut other);other_identity=other.identity.clone();
            let mut head=vault.commit_pd_operation(initial).unwrap();expected_first=head.record_id;
            vault.commit_pd_operation(other).unwrap();
            for revision in 2..=66 {head=vault.commit_pd_operation(after(&head,&format!("history-{revision}"))).unwrap();}
            expected_last=head.record_id;
            // The real exclusive WAL lock also excludes external reads on Windows.
            drop(vault);
            let before=std::fs::read(&file).unwrap();
            let vault=KnowledgeVault::open_disk(&file).unwrap();
            let query=PdHistoryLookup{identity:identity.clone(),from_revision:1,limit:64};
            let page=vault.pd_history(&query).unwrap();
            assert_eq!((page.entries.len(),page.head_revision,page.next_revision),(64,66,Some(65)));
            assert_eq!(page.entries[0].record_id,expected_first);
            assert!(page.entries.iter().all(|entry|entry.status==PdMapStatus::OperationalNoVerdict&&entry.has_candidate&&entry.source_kind==PdSourceKind::ScientificDialogue));
            let mut invalid=query;invalid.limit=65;assert!(vault.pd_history(&invalid).is_err());
            assert_eq!(counts(&vault),[0;7]);
            drop(vault);
            assert_eq!(std::fs::read(&file).unwrap(),before);
        }
        {
            let vault=KnowledgeVault::open_disk(&file).unwrap();
            let page=vault.pd_history(&PdHistoryLookup{identity:identity.clone(),from_revision:65,limit:64}).unwrap();
            assert_eq!((page.entries.len(),page.next_revision),(2,None));assert_eq!(page.entries[1].record_id,expected_last);
            let first=vault.pd_history(&PdHistoryLookup{identity:identity.clone(),from_revision:1,limit:1}).unwrap();
            assert_eq!(first.entries[0].record_id,expected_first);assert_eq!(first.next_revision,Some(2));
            let other=vault.pd_history(&PdHistoryLookup{identity:other_identity,from_revision:1,limit:64}).unwrap();
            assert_eq!((other.head_revision,other.entries.len()),(1,1));assert_ne!(other.entries[0].record_id,expected_first);
            let exhausted=vault.pd_history(&PdHistoryLookup{identity,from_revision:67,limit:64}).unwrap();assert!(exhausted.entries.is_empty());assert_eq!(exhausted.next_revision,None);
            assert_eq!(counts(&vault),[0;7]);
        }
        std::fs::remove_file(file).unwrap();
    }
    #[test]
    fn history_index_shares_arc_records_and_never_copies_vault_material() {
        use sha2::Digest;
        let mut projection=PdProjection::default();let request=request("reference-only");let identity=request.identity.clone();
        let append=match projection.prepare(request).unwrap(){PdCommitPlan::Append(value)=>value,_=>panic!("new operation")};
        let record=projection.publish(append);let key=identity.key().unwrap();
        assert!(Arc::ptr_eq(projection.revisions.get(&(key,1)).unwrap(),&record));
        assert!(Arc::ptr_eq(projection.operations.get(&(key,"reference-only".into())).unwrap(),&record));
        let page=projection.history(&shadow_contracts::pd_memory::history::PdHistoryLookup{identity,from_revision:1,limit:1}).unwrap();
        let question_hash:Hash=sha2::Sha256::digest(record.request.scoped_question.as_bytes()).into();
        assert_eq!(page.entries[0].question_reference,question_hash);
        let encoded=shadow_contracts::pd_memory::history::encode_history_page(&page).unwrap();
        assert!(!encoded.windows(record.request.scoped_question.len()).any(|w|w==record.request.scoped_question.as_bytes()));
    }

}

#[cfg(test)]
mod failure_consistency_tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use shadow_contracts::pd_memory::{failure::*, *};
    use shadow_contracts::*;
    fn fixture() -> FailureDialogueInput {
        let request = KnowledgeSelectionRequestWire {
            version: PROTOCOL_VERSION,
            requester: SelectionRequester::AshtQuantum,
            purpose: SelectionPurpose::EvidenceCandidates,
            session_id: "case-A".into(),
            parent_i0: "i0-0000000000000007".into(),
            primitive_anchor: 7,
            trace_id: "trace-A".into(),
            split_sha256: "11".repeat(32),
            xi: vec!["alpha".into()],
            yi: vec!["beta".into()],
            upstream_hold_sha256: Some("22".repeat(32)),
            evidence_sha256: "33".repeat(32),
            max_positive: SELECTION_MAX_CANDIDATES,
            max_negative: SELECTION_MAX_CANDIDATES,
        };
        let request_bytes = encode_selection_request(&request);
        let response = KnowledgeSelectionResponseWire {
            version: PROTOCOL_VERSION,
            requester: request.requester,
            session_id: request.session_id.clone(),
            parent_i0: request.parent_i0.clone(),
            primitive_anchor: 7,
            split_sha256: request.split_sha256.clone(),
            upstream_hold_sha256: request.upstream_hold_sha256.clone(),
            request_sha256: shadow_contracts::sha256_hex(&[
                b"ESSMAI/SELECTION/REQUEST/V178",
                &request_bytes,
            ]),
            selection_sha256: shadow_contracts::sha256_hex(&[
                b"ESSMAI/SELECTION/CANDIDATES/V178",
                &[],
            ]),
            hold_id: "hold-A".into(),
            hold_sha256: "44".repeat(32),
            positive: vec![],
            negative: vec![],
            law_seal: 1,
            system_laws_seal: 2,
            expires_at_ns: 3,
        };
        FailureDialogueInput {
            identity: PdIdentity {
                owner: "owner".into(),
                obligation: "obligation".into(),
                case_id: request.session_id.clone(),
                scope: "scope".into(),
                domain: "physics".into(),
                lineage_root: request.parent_i0.clone(),
            },
            operation_id: "failed-initial".into(),
            user_text: "Si dallohet alpha nga beta?".into(),
            selection_request: request_bytes,
            selection_response: encode_selection_response(&response),
            refusal: MISSING_POSITIVE.into(),
            in_reply_to: [0; 32],
            positive: String::new(),
            negative: String::new(),
            distinction: String::new(),
            source_reference: String::new(),
        }
    }

    fn request_for(input: &FailureDialogueInput) -> PdOperationRequest {
        let mut request = super::tests::request(&input.operation_id);
        request.identity = input.identity.clone();
        request.source_input = encode_failure_input(input).unwrap();
        request.witness.input_sha256 = Sha256::digest(&request.source_input).into();
        request.witness.reason = input.refusal.clone();
        request.witness.condition_code = "MISSING_EXACT_POSITIVE".into();
        request.witness.attempt_trace = input.operation_id.clone();
        let step = LgcStep::new(
            0,
            request.witness.input_sha256,
            "D-BAB",
            "fixture observed missing pair",
            [9; 32],
        )
        .unwrap();
        request.witness.lgc_parent = step.step_digest;
        request.lgc_steps = vec![step];
        request.checkpoint = [FAILURE_CHECKPOINT_PREFIX, b"fixture opaque checkpoint"].concat();
        validate_failure_operation(&request).unwrap();
        request
    }
    #[test]
    fn witness_relabelling_is_rejected_by_projection_and_semantic_replay() {
        let initial = request_for(&fixture());
        for field in 0..4 {
            let mut altered = initial.clone();
            match field {
                0 => altered.witness.reason = "unrelated refusal".into(),
                1 => altered.witness.condition_code = "MISSING_EXACT_NEGATIVE".into(),
                2 => {
                    altered.witness.report_class = ReportClass::TransportUncertain;
                    altered.witness.permitted_next = NextAction::ResolveCommitStatus;
                }
                _ => altered.witness.attempt_trace = "different operation".into(),
            }
            altered.validate().unwrap(); // Generic shape/hash validation alone is insufficient.
            let internally_valid = PdOperationRecord::construct(altered.clone(), [0; 32]).unwrap();
            let mut projection = PdProjection::default();
            assert!(
                matches!(projection.prepare(altered),Err(e) if e=="PD_FAILURE_WITNESS_INCONSISTENT")
            );
            assert!(projection
                .replay(internally_valid)
                .unwrap_err()
                .contains("WITNESS_INCONSISTENT"));
            assert!(projection.head(&initial.identity).unwrap().is_none());
        }
    }
    #[test]
    fn direct_commit_cannot_write_inconsistent_witness_and_reopen_keeps_valid_record() {
        let path = std::env::temp_dir().join(format!(
            "pd-failure-witness-{}-{}.wal",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let request = request_for(&fixture());
        let committed;
        {
            let vault = crate::knowledge_vault::KnowledgeVault::open_disk(&path).unwrap();
            // Inspect only after releasing the lifetime-held exclusive owner.
            drop(vault);
            let before = std::fs::read(&path).unwrap();
            let vault = crate::knowledge_vault::KnowledgeVault::open_disk(&path).unwrap();
            let mut bad = request.clone();
            bad.witness.reason = "invented transport failure".into();
            assert!(vault.commit_pd_operation(bad).is_err());
            drop(vault);
            assert_eq!(std::fs::read(&path).unwrap(), before);
            let vault = crate::knowledge_vault::KnowledgeVault::open_disk(&path).unwrap();
            committed = vault.commit_pd_operation(request).unwrap();
            assert_eq!((vault.primitive_count(), vault.negative_count()), (0, 0));
        }
        let vault = crate::knowledge_vault::KnowledgeVault::open_disk(&path).unwrap();
        assert_eq!(
            vault.pd_head(&committed.request.identity).unwrap(),
            Some(committed)
        );
        assert_eq!((vault.primitive_count(), vault.negative_count()), (0, 0));
        drop(vault);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn direct_storage_answer_requires_exact_parent_even_with_valid_current_cas() {
        let input = fixture();
        let first = PdOperationRecord::construct(request_for(&input), [0; 32]).unwrap();
        let mut projection = PdProjection::default();
        projection.replay(first.clone()).unwrap();
        let mut answer = input.clone();
        answer.operation_id = "answer-2".into();
        answer.in_reply_to = [7; 32];
        answer.positive = "reported support".into();
        answer.negative = "reported counter".into();
        answer.distinction = "reported distinction".into();
        answer.source_reference = "reported source".into();
        let mut request = request_for(&answer);
        request.expected_revision = first.revision;
        request.previous_state = first.state_digest;
        assert!(
            matches!(projection.prepare(request),Err(e) if e=="PD_FAILURE_PARENT_QUESTION_MISMATCH")
        );
        assert_eq!(projection.head(&input.identity).unwrap(), Some(first));
    }
    #[test]
    fn linked_science_keeps_exact_parent_and_returns_to_pdf1_without_admission() {
        use shadow_contracts::pd_memory::linked_science::*;
        let input=fixture();
        let first=PdOperationRecord::construct(request_for(&input),[0;32]).unwrap();
        let mut projection=PdProjection::default();projection.replay(first.clone()).unwrap();
        let mut answer=input.clone();answer.operation_id="linked-science-answer".into();
        answer.in_reply_to=first.record_id;answer.positive="reported support".into();
        answer.negative="reported counter".into();answer.distinction="reported distinction".into();
        answer.source_reference="offline user reference".into();
        answer.user_text=[&answer.positive,&answer.negative,&answer.distinction,&answer.source_reference]
            .map(String::as_str).join("\n");
        let linked=LinkedScienceInput { dialogue:answer.clone(),
            parent_question_sha256:Sha256::digest(first.request.scoped_question.as_bytes()).into(),
            science_query:vec![1], relevance_statement:"Storage-only opaque fixture; no scientific admission".into() };
        let mut request=request_for(&answer);
        request.source_input=encode_linked_input(&linked).unwrap();
        request.expected_revision=first.revision;request.previous_state=first.state_digest;
        request.candidate=b"opaque candidate fixture; native admission separately verifies model".to_vec();
        request.candidate_key=[3;32];
        fn bind(request:&mut PdOperationRequest) {
            request.witness.input_sha256=Sha256::digest(&request.source_input).into();
            let step=LgcStep::new(0,request.witness.input_sha256,"test","bind source",[9;32]).unwrap();
            request.witness.lgc_parent=step.step_digest;request.lgc_steps=vec![step];
        }
        bind(&mut request);
        let mut forged=linked.clone();forged.parent_question_sha256[0]^=1;
        let mut wrong=request.clone();wrong.source_input=encode_linked_input(&forged).unwrap();bind(&mut wrong);
        assert!(matches!(projection.prepare(wrong),Err(e) if e=="PD_SCIENCE_PARENT_BINDING"));
        let second=match projection.prepare(request).unwrap(){PdCommitPlan::Append(a)=>projection.publish(a),_=>panic!("new record")};
        let mut plain=answer;plain.operation_id="plain-answer-after-model".into();plain.in_reply_to=second.record_id;
        let mut request=request_for(&plain);request.expected_revision=second.revision;request.previous_state=second.state_digest;
        let third=match projection.prepare(request).unwrap(){PdCommitPlan::Append(a)=>projection.publish(a),_=>panic!("new record")};
        assert_eq!(third.previous_record,second.record_id);assert!(third.request.candidate.is_empty());
        assert_eq!(third.request.witness.report_class,ReportClass::MissingConditions);
        let mut replay=PdProjection::default();replay.replay(first).unwrap();replay.replay(second.as_ref().clone()).unwrap();
        replay.replay(third.as_ref().clone()).unwrap();assert_eq!(replay.head(&input.identity).unwrap(),Some(third.as_ref().clone()));
    }

    #[test]
    fn failure_history_is_operational_reference_not_positive_or_negative_admission() {
        use shadow_contracts::pd_memory::history::*;
        let input=fixture();let record=PdOperationRecord::construct(request_for(&input),[0;32]).unwrap();
        let mut projection=PdProjection::default();projection.replay(record.clone()).unwrap();
        let query=PdHistoryLookup{identity:input.identity,from_revision:1,limit:64};
        let page=projection.history(&query).unwrap();let entry=&page.entries[0];
        assert_eq!(entry.source_kind,PdSourceKind::MissingPairDialogue);
        assert_eq!(entry.status,PdMapStatus::OperationalNoVerdict);
        assert_eq!(entry.record_id,record.record_id);assert!(!entry.has_candidate);
        assert_eq!(entry.report_class,ReportClass::MissingConditions);
        assert_eq!(entry.permitted_next,NextAction::AcquireScopedEvidence);
        assert_eq!(decode_history_page(&encode_history_page(&page).unwrap()).unwrap(),page);
    }

}

#[cfg(test)]
mod pds2_storage_tests{
    use super::*;
    use shadow_contracts::pd_memory::{self as pd,iz,iz_science as bridge};
    use sha2::{Digest,Sha256};
    fn sha(b:&[u8])->pd::Hash{Sha256::digest(b).into()}
    // Reuse already pinned Q4 SYNTHETIC v5 storage fixtures only for real
    // source-produced PD checkpoints. This does not execute native admission.
    fn fixture()->(PdOperationRecord,PdOperationRequest){
        let parent=pd::decode_record(include_bytes!("test_fixtures/pd_v5/parent-positive/parent.record")).unwrap();
        let env=shadow_contracts::pd_prepared::decode_prepared(include_bytes!("test_fixtures/pd_v5/parent-positive/prepared.bin")).unwrap();
        let mut c=env.source_iz.unwrap().context;c.operation_id="pds2-storage-test".into();c.answer=iz::IzAnswer::default();
        let query=science_domains::ScienceQuery{adapter:science_domains::AdapterId::Astronomy,
            session_id:pd::hex(&c.identity.key().unwrap()),case_id:c.identity.case_id.clone(),scope_id:c.identity.scope.clone(),
            obligation_id:c.identity.obligation.clone(),query_id:"storage-regression".into(),input_source_id:"IAU regression, not observation".into(),
            task:science_domains::ScienceTask::EarthRotationAngle{jd_part1:2400000.5,jd_part2:54388.0,
                time_scale:science_domains::TimeScale::Ut1,claim_radians:0.4022837240028158,tolerance_radians:1e-12}};
        let value=bridge::from_parent(c,&parent,science_domains::encode_query(&query).unwrap(),"unadjudicated fixture relevance".into()).unwrap();
        let computed=science_domains::evaluate(&query).unwrap();let candidate=science_domains::encode_candidate(&computed).unwrap();
        let checkpoint=[pd::failure::FAILURE_CHECKPOINT_PREFIX,env.checkpoint.as_slice()].concat();
        let steps=bridge::build_steps(&value,&checkpoint,&candidate).unwrap();
        let request=PdOperationRequest{identity:value.context.identity.clone(),operation_id:value.context.operation_id.clone(),
            source_input:bridge::encode_input(&value).unwrap(),expected_revision:parent.revision,previous_state:parent.state_digest,
            witness:bridge::build_witness(&value,&steps,[8;32]).unwrap(),checkpoint,scoped_question:bridge::question(&value).unwrap(),
            candidate,candidate_key:computed.query_sha256,lgc_steps:steps};
        bridge::validate_transition(&request,&parent).unwrap();(parent,request)
    }
    fn rebind(request:&mut PdOperationRequest){
        let input=bridge::decode_input(&request.source_input).unwrap();
        request.lgc_steps=bridge::build_steps_with_question(&input,&request.checkpoint,&request.candidate,&request.scoped_question).unwrap();
        request.witness=bridge::build_witness(&input,&request.lgc_steps,[8;32]).unwrap();
        request.validate().unwrap();
    }
    // The parent/checkpoint is the explicitly SYNTHETIC storage fixture above.
    // The ERA candidate is genuinely recomputed by the installed provider, but
    // neither this fixture nor its persistence asserts native GCL admission.
    fn computed_fixture(kind:u8)->(PdOperationRecord,PdOperationRequest){
        use pd::iz_science_math::{computed_question,EraComparison};
        let(parent,mut request)=fixture();
        let mut input=bridge::decode_input(&request.source_input).unwrap();
        input.question_policy=bridge::QuestionPolicy::ComputedV1;
        let mut query=science_domains::decode_query(&input.science_query).unwrap();
        let base=science_domains::evaluate(&query).unwrap();
        let science_domains::ScienceTask::EarthRotationAngle{ref mut claim_radians,ref mut tolerance_radians,..}=query.task;
        *claim_radians=if kind==0 {base.computed_radians}else{0.0};
        *tolerance_radians=1e-12;
        if kind==2 {
            let residual=science_domains::evaluate(&query).unwrap().circular_residual_radians;
            assert!(residual>0.0);
            let science_domains::ScienceTask::EarthRotationAngle{ref mut tolerance_radians,..}=query.task;
            *tolerance_radians=residual;
        }
        let actual=science_domains::evaluate(&query).unwrap();
        assert_eq!(actual.relation,match kind{
            0=>science_domains::ModelRelation::SupportsClaim,
            1=>science_domains::ModelRelation::CountersClaim,
            2=>science_domains::ModelRelation::Unresolved,
            _=>panic!("unknown arithmetic fixture"),
        });
        input.science_query=science_domains::encode_query(&query).unwrap();
        request.source_input=bridge::encode_input(&input).unwrap();
        assert!(request.source_input.starts_with(b"PDS4"));
        request.candidate=science_domains::encode_candidate(&actual).unwrap();
        request.candidate_key=actual.query_sha256;
        let science_domains::ScienceTask::EarthRotationAngle{claim_radians,tolerance_radians,..}=actual.query.task;
        request.scoped_question=computed_question(&input,&EraComparison{
            query_sha256:actual.query_sha256,result_sha256:actual.result_sha256,
            model_sha256:actual.provenance.compiled_model_sha256,
            claim_radians,computed_radians:actual.computed_radians,
            residual_radians:actual.circular_residual_radians,tolerance_radians,
        }).unwrap();
        rebind(&mut request);
        bridge::validate_transition(&request,&parent).unwrap();
        crate::pd_failure_admission::validate_pds2_candidate(&request).unwrap();
        (parent,request)
    }
    fn computed_path(label:&str,kind:u8)->std::path::PathBuf{
        std::env::temp_dir().join(format!("pds4-math-{label}-{kind}-{}-{}.wal",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()))
    }
    #[test]
    fn computed_result_questions_survive_real_disk_reopen_and_exact_retry_without_knowledge(){
        for kind in 0..3 {
            let(parent,request)=computed_fixture(kind);
            let question=request.scoped_question.clone();
            let expected=match kind {0=>"KiS=1 KiC=0 U=0",1=>"KiS=0 KiC=1 U=0",_=>"KiS=0 KiC=0 U=1"};
            assert!(question.contains(expected));
            assert!(question.contains("OperationalNoVerdict"));
            assert!(question.contains("pa pranim K+/K-"));
            assert!(question.contains("Mungesa historike e dëshmuar në prind"));
            if kind==2 {assert!(question.contains("jo kundërprovë dhe jo pranim"));}
            let path=computed_path("reopen",kind);
            let saved;
            {
                let vault=crate::KnowledgeVault::open_disk(&path).unwrap();
                assert_eq!(vault.commit_pd_operation(parent.request.clone()).unwrap(),parent);
                saved=vault.commit_pd_operation(request.clone()).unwrap();
                assert_eq!(saved.request.scoped_question,question);
                assert_eq!(saved.previous_record,parent.record_id);
                assert_eq!(vault.store_counts_for_test(),[0;7]);
            }
            // Windows intentionally denies raw reads while the Vault owns its
            // exclusive file handle. Compare bytes only between owner lifetimes.
            let before=std::fs::read(&path).unwrap();
            {
                let vault=crate::KnowledgeVault::open_disk(&path).unwrap();
                assert_eq!(vault.pd_head(&request.identity).unwrap(),Some(saved.clone()));
                assert_eq!(vault.pd_operation(&request.identity,&request.operation_id).unwrap(),Some(saved.clone()));
                let retry=vault.commit_pd_operation(request.clone()).unwrap();
                assert_eq!(retry,saved);
                assert_eq!(retry.request.scoped_question,question);
                assert_eq!(vault.commit_pd_operation(request.clone()).unwrap(),saved);
                assert_eq!(vault.store_counts_for_test(),[0;7]);
            }
            assert_eq!(std::fs::read(&path).unwrap(),before);
            std::fs::remove_file(path).unwrap();
        }
    }
    #[test]
    fn computed_question_rebinding_cannot_hide_wrong_relation_or_omitted_output_live_or_replay(){
        for kind in 0..3 {
            let(parent,good)=computed_fixture(kind);
            let original_marker=match kind {0=>"KiS=1 KiC=0 U=0",1=>"KiS=0 KiC=1 U=0",_=>"KiS=0 KiC=0 U=1"};
            let wrong_marker=if kind==1 {"KiS=1 KiC=0 U=0"}else{"KiS=0 KiC=1 U=0"};
            let path=computed_path("refusal",kind);
            {
                let vault=crate::KnowledgeVault::open_disk(&path).unwrap();
                vault.commit_pd_operation(parent.request.clone()).unwrap();
            }
            let before=std::fs::read(&path).unwrap();
            for omit_output in [false,true] {
                {
                    let vault=crate::KnowledgeVault::open_disk(&path).unwrap();
                    let mut bad=good.clone();
                    bad.scoped_question=if omit_output {
                        "Operational_iZ: mungon një provë; jep evidencën. OperationalNoVerdict.".into()
                    }else{
                        good.scoped_question.replacen(original_marker,wrong_marker,1)
                    };
                    assert_ne!(bad.scoped_question,good.scoped_question);
                    // Rebind ordinary data digests deliberately: rejection must
                    // come from independently recomputed meaning, not stale hashes.
                    rebind(&mut bad);
                    assert_eq!(bad.candidate,good.candidate);
                    assert_eq!(bad.source_input,good.source_input);
                    let internally_coherent=PdOperationRecord::construct(bad.clone(),parent.record_id).unwrap();
                    internally_coherent.validate().unwrap();
                    let mut projection=PdProjection::default();
                    projection.replay(parent.clone()).unwrap();
                    let live=projection.prepare(bad.clone()).err().expect("computed output must be checked");
                    assert_eq!(live,"IZ_SCIENCE_COMPUTED_OUTPUT_MISMATCH");
                    assert_eq!(projection.replay(internally_coherent).unwrap_err(),"IZ_SCIENCE_COMPUTED_OUTPUT_MISMATCH");
                    assert_eq!(projection.head(&parent.request.identity).unwrap(),Some(parent.clone()));
                    let error=vault.commit_pd_operation(bad).unwrap_err();
                    assert!(format!("{error:?}").contains("IZ_SCIENCE_COMPUTED_OUTPUT_MISMATCH"));
                    assert_eq!(vault.pd_head(&parent.request.identity).unwrap(),Some(parent.clone()));
                    assert_eq!(vault.store_counts_for_test(),[0;7]);
                }
                assert_eq!(std::fs::read(&path).unwrap(),before);
            }
            {
                let vault=crate::KnowledgeVault::open_disk(&path).unwrap();
                vault.commit_pd_operation(good).unwrap();
            }
            std::fs::remove_file(path).unwrap();
        }
    }
    #[test]
    fn live_and_replay_reject_self_consistent_candidate_and_history_substitution(){
        let(parent,good)=fixture();
        for kind in 0..4{
            let mut projection=PdProjection::default();projection.replay(parent.clone()).unwrap();
            let mut bad=good.clone();
            match kind{
                0=>{let last=bad.candidate.len()-1;bad.candidate[last]^=1;},
                1=>{let mut input=bridge::decode_input(&bad.source_input).unwrap();
                    let mut q=science_domains::decode_query(&input.science_query).unwrap();q.query_id.push_str("-substituted");
                    input.science_query=science_domains::encode_query(&q).unwrap();bad.source_input=bridge::encode_input(&input).unwrap();},
                2=>{let prior=iz::validate_operation(&parent.request).unwrap();bad.checkpoint=[pd::failure::FAILURE_CHECKPOINT_PREFIX,
                    parent.request.checkpoint.strip_prefix(prior.context.checkpoint_prefix()).unwrap()].concat();},
                _=>{let mut input=bridge::decode_input(&bad.source_input).unwrap();input.prior_lgc_head[0]^=1;
                    bad.source_input=bridge::encode_input(&input).unwrap();},
            }
            rebind(&mut bad);
            let forged=PdOperationRecord::construct(bad.clone(),parent.record_id).unwrap();
            assert!(projection.prepare(bad).is_err());assert!(projection.replay(forged).is_err());
            assert_eq!(projection.head(&parent.request.identity).unwrap(),Some(parent.clone()));
        }
    }
    #[test]
    fn actual_disk_append_reopen_and_exact_retry_preserve_one_map_and_no_knowledge(){
        let(parent,request)=fixture();
        let path=std::env::temp_dir().join(format!("pds2-storage-{}-{}.wal",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let saved;
        {
            let vault=crate::KnowledgeVault::open_disk(&path).unwrap();
            vault.commit_pd_operation(parent.request.clone()).unwrap();
            saved=vault.commit_pd_operation(request.clone()).unwrap();
            assert_eq!(vault.commit_pd_operation(request.clone()).unwrap(),saved);
            assert_eq!(saved.previous_record,parent.record_id);
        }
        let before=std::fs::read(&path).unwrap();
        {
            let vault=crate::KnowledgeVault::open_disk(&path).unwrap();
            assert_eq!(vault.pd_head(&request.identity).unwrap(),Some(saved.clone()));
            assert_eq!(vault.commit_pd_operation(request.clone()).unwrap(),saved);
            let mut bad=request;bad.candidate[0]^=1;rebind(&mut bad);
            assert!(vault.commit_pd_operation(bad).is_err());
            assert_eq!(vault.pd_head(&saved.request.identity).unwrap(),Some(saved));
        }
        assert_eq!(sha(&std::fs::read(&path).unwrap()),sha(&before));
        std::fs::remove_file(path).unwrap();
    }
}
