//! bounded pd_mp references, never a second material store.
//! PHQ1/PHP1 are NEW read-only map codecs; existing WAL records are unchanged.
use super::*;
pub const MAX_HISTORY_ENTRIES: u16 = 64;
pub const MAX_HISTORY_QUERY_BYTES: usize = 8192;
pub const MAX_HISTORY_PAGE_BYTES: usize = 96 * 1024;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdHistoryLookup {
    pub identity: PdIdentity,
    pub from_revision: u64,
    pub limit: u16,
}
impl PdHistoryLookup {
    pub fn validate(&self) -> Result<(), String> {
        self.identity.validate()?;
        if self.from_revision == 0 || self.limit == 0 || self.limit > MAX_HISTORY_ENTRIES {
            return Err("PD_HISTORY_QUERY_RANGE".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdSourceKind {
    ScientificDialogue = 1,
    MissingPairDialogue = 2,
    LinkedScientificDialogue = 3,
    CanonicalIzDialogue = 4,
    LateStructuralRefusal = 5,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PdMapStatus {
    OperationalNoVerdict = 1,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdMapEntry {
    pub revision: u64,
    pub record_id: Hash,
    pub previous_record: Hash,
    pub operation_id: String,
    pub source_kind: PdSourceKind,
    pub status: PdMapStatus,
    pub question_reference: Hash,
    pub has_candidate: bool,
    pub report_class: ReportClass,
    pub permitted_next: NextAction,
}
impl PdMapEntry {
    /// Caller supplies a record already validated by authoritative projection/replay.
    /// Only reference metadata is copied; no source, answer, question or checkpoint.
    pub fn from_validated_record(record: &PdOperationRecord) -> Result<Self, String> {
        let source_kind = match record.request.source_input.get(..4) {
            Some(b"PDC1") => PdSourceKind::ScientificDialogue,
            Some(b"PDF1") => PdSourceKind::MissingPairDialogue,
            Some(b"PDS1") | Some(b"PDS2") | Some(b"PDS3") | Some(b"PDS4") | Some(b"PDS5") => PdSourceKind::LinkedScientificDialogue,
            Some(b"PIZ1") | Some(b"PIZ2") | Some(b"PIZ3") => PdSourceKind::CanonicalIzDialogue,
            Some(b"PLF1") => PdSourceKind::LateStructuralRefusal,
            _ => return Err("PD_HISTORY_SOURCE_KIND".into()),
        };
        let entry = Self {
            revision: record.revision,
            record_id: record.record_id,
            previous_record: record.previous_record,
            operation_id: record.request.operation_id.clone(),
            source_kind,
            status: PdMapStatus::OperationalNoVerdict,
            question_reference: Sha256::digest(record.request.scoped_question.as_bytes()).into(),
            has_candidate: !record.request.candidate.is_empty(),
            report_class: record.request.witness.report_class,
            permitted_next: record.request.witness.permitted_next,
        };
        entry.validate()?;
        Ok(entry)
    }
    fn validate(&self) -> Result<(), String> {
        text_valid(&self.operation_id, 1024)?;
        if self.revision == 0
            || self.record_id == [0; 32]
            || self.question_reference == [0; 32]
            || (self.revision == 1) != (self.previous_record == [0; 32])
        {
            return Err("PD_HISTORY_ENTRY_BINDING".into());
        }
        let allowed = matches!(
            (self.report_class, self.permitted_next),
            (
                ReportClass::MissingConditions,
                NextAction::AcquireScopedEvidence | NextAction::ExplainConditions
            ) | (ReportClass::AuthorityRefused, NextAction::ExplainConditions)
                | (
                    ReportClass::TransportUncertain,
                    NextAction::ResolveCommitStatus
                )
        );
        if !allowed {
            return Err("PD_HISTORY_WITNESS_SCOPE".into());
        }
        if matches!(self.source_kind,PdSourceKind::MissingPairDialogue | PdSourceKind::LateStructuralRefusal)
            && (self.has_candidate
                || self.report_class != ReportClass::MissingConditions
                || self.permitted_next != NextAction::AcquireScopedEvidence)
        {
            return Err("PD_HISTORY_FAILURE_POLICY".into());
        }
        if self.source_kind == PdSourceKind::LinkedScientificDialogue
            && (!self.has_candidate
                || self.report_class != ReportClass::MissingConditions
                || self.permitted_next != NextAction::AcquireScopedEvidence)
        {
            return Err("PD_HISTORY_LINKED_SCIENCE_POLICY".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdHistoryPage {
    pub query: PdHistoryLookup,
    pub head_revision: u64,
    pub entries: Vec<PdMapEntry>,
    pub next_revision: Option<u64>,
}
impl PdHistoryPage {
    pub fn validate(&self) -> Result<(), String> {
        self.query.validate()?;
        let expected = if self.query.from_revision > self.head_revision {
            0
        } else {
            (self.head_revision - self.query.from_revision + 1).min(u64::from(self.query.limit))
                as usize
        };
        if self.entries.len() != expected {
            return Err("PD_HISTORY_INCOMPLETE_PAGE".into());
        }
        let mut previous = None;
        for (index, entry) in self.entries.iter().enumerate() {
            entry.validate()?;
            let revision = self
                .query
                .from_revision
                .checked_add(index as u64)
                .ok_or("PD_HISTORY_REVISION_OVERFLOW")?;
            if entry.revision != revision {
                return Err("PD_HISTORY_ORDER".into());
            }
            if previous
                .map(|id| id != entry.previous_record)
                .unwrap_or(false)
            {
                return Err("PD_HISTORY_CHAIN".into());
            }
            previous = Some(entry.record_id);
        }
        let next = match self.entries.last() {
            Some(last) if last.revision < self.head_revision => Some(
                last.revision
                    .checked_add(1)
                    .ok_or("PD_HISTORY_REVISION_OVERFLOW")?,
            ),
            _ => None,
        };
        if self.next_revision != next {
            return Err("PD_HISTORY_CURSOR".into());
        }
        Ok(())
    }
}
pub fn encode_history_lookup(query: &PdHistoryLookup) -> Result<Vec<u8>, String> {
    query.validate()?;
    let mut out = b"PHQ1".to_vec();
    identity_out(&mut out, &query.identity)?;
    out.extend_from_slice(&query.from_revision.to_le_bytes());
    out.extend_from_slice(&query.limit.to_le_bytes());
    if out.len() > MAX_HISTORY_QUERY_BYTES {
        return Err("PD_HISTORY_QUERY_LIMIT".into());
    }
    Ok(out)
}
pub fn decode_history_lookup(bytes: &[u8]) -> Result<PdHistoryLookup, String> {
    if bytes.len() > MAX_HISTORY_QUERY_BYTES {
        return Err("PD_HISTORY_QUERY_LIMIT".into());
    }
    let mut r = Reader { bytes, pos: 0 };
    if r.take(4)? != b"PHQ1" {
        return Err("PD_HISTORY_QUERY_MAGIC".into());
    }
    let identity = identity_in(&mut r)?;
    let from_revision = r.u64()?;
    let limit = u16::from_le_bytes(r.take(2)?.try_into().map_err(|_| "PD_HISTORY_LIMIT")?);
    r.end()?;
    let query = PdHistoryLookup {
        identity,
        from_revision,
        limit,
    };
    if encode_history_lookup(&query)? != bytes {
        return Err("PD_HISTORY_QUERY_NONCANONICAL".into());
    }
    Ok(query)
}
pub fn encode_history_page(page: &PdHistoryPage) -> Result<Vec<u8>, String> {
    page.validate()?;
    let mut out = b"PHP1".to_vec();
    put_bytes(
        &mut out,
        &encode_history_lookup(&page.query)?,
        MAX_HISTORY_QUERY_BYTES,
    )?;
    out.extend_from_slice(&page.head_revision.to_le_bytes());
    out.extend_from_slice(&page.next_revision.unwrap_or(0).to_le_bytes());
    out.extend_from_slice(&(page.entries.len() as u16).to_le_bytes());
    for e in &page.entries {
        out.extend_from_slice(&e.revision.to_le_bytes());
        out.extend_from_slice(&e.record_id);
        out.extend_from_slice(&e.previous_record);
        put_text(&mut out, &e.operation_id)?;
        out.push(e.source_kind as u8);
        out.push(e.status as u8);
        out.extend_from_slice(&e.question_reference);
        out.push(u8::from(e.has_candidate));
        out.push(e.report_class as u8);
        out.push(e.permitted_next as u8);
    }
    if out.len() > MAX_HISTORY_PAGE_BYTES {
        return Err("PD_HISTORY_PAGE_LIMIT".into());
    }
    Ok(out)
}
pub fn decode_history_page(bytes: &[u8]) -> Result<PdHistoryPage, String> {
    if bytes.len() > MAX_HISTORY_PAGE_BYTES {
        return Err("PD_HISTORY_PAGE_LIMIT".into());
    }
    let mut r = Reader { bytes, pos: 0 };
    if r.take(4)? != b"PHP1" {
        return Err("PD_HISTORY_PAGE_MAGIC".into());
    }
    let query = decode_history_lookup(&r.bytes(MAX_HISTORY_QUERY_BYTES)?)?;
    let head_revision = r.u64()?;
    let next = r.u64()?;
    let count = u16::from_le_bytes(r.take(2)?.try_into().map_err(|_| "PD_HISTORY_COUNT")?);
    if count > MAX_HISTORY_ENTRIES || count > query.limit {
        return Err("PD_HISTORY_COUNT_LIMIT".into());
    }
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(count as usize)
        .map_err(|_| "PD_HISTORY_ALLOCATION")?;
    for _ in 0..count {
        let revision = r.u64()?;
        let record_id = r.hash()?;
        let previous_record = r.hash()?;
        let operation_id = r.text()?;
        let source_kind = match r.u8()? {
            1 => PdSourceKind::ScientificDialogue,
            2 => PdSourceKind::MissingPairDialogue,
            3 => PdSourceKind::LinkedScientificDialogue,
            4 => PdSourceKind::CanonicalIzDialogue,
            5 => PdSourceKind::LateStructuralRefusal,
            _ => return Err("PD_HISTORY_SOURCE_KIND".into()),
        };
        let status = match r.u8()? {
            1 => PdMapStatus::OperationalNoVerdict,
            _ => return Err("PD_HISTORY_STATUS_NO_VERDICT_ONLY".into()),
        };
        let question_reference = r.hash()?;
        let has_candidate = match r.u8()? {
            0 => false,
            1 => true,
            _ => return Err("PD_HISTORY_CANDIDATE_BOOL".into()),
        };
        let report_class = match r.u8()? {
            1 => ReportClass::MissingConditions,
            2 => ReportClass::AuthorityRefused,
            3 => ReportClass::TransportUncertain,
            _ => return Err("PD_HISTORY_REPORT_CLASS".into()),
        };
        let permitted_next = match r.u8()? {
            1 => NextAction::AcquireScopedEvidence,
            2 => NextAction::ExplainConditions,
            3 => NextAction::ResolveCommitStatus,
            _ => return Err("PD_HISTORY_NEXT_ACTION".into()),
        };
        entries.push(PdMapEntry {
            revision,
            record_id,
            previous_record,
            operation_id,
            source_kind,
            status,
            question_reference,
            has_candidate,
            report_class,
            permitted_next,
        });
    }
    r.end()?;
    let page = PdHistoryPage {
        query,
        head_revision,
        entries,
        next_revision: if next == 0 { None } else { Some(next) },
    };
    if encode_history_page(&page)? != bytes {
        return Err("PD_HISTORY_PAGE_NONCANONICAL".into());
    }
    Ok(page)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn page() -> PdHistoryPage {
        let record = PdOperationRecord::construct(super::super::tests::request(), [0; 32]).unwrap();
        PdHistoryPage {
            query: PdHistoryLookup {
                identity: record.request.identity.clone(),
                from_revision: 1,
                limit: 64,
            },
            head_revision: 1,
            entries: vec![PdMapEntry::from_validated_record(&record).unwrap()],
            next_revision: None,
        }
    }
    #[test]
    fn codec_is_bounded_canonical_and_never_a_knowledge_verdict() {
        let p = page();
        let bytes = encode_history_page(&p).unwrap();
        assert_eq!(decode_history_page(&bytes).unwrap(), p);
        assert_eq!(p.entries[0].status, PdMapStatus::OperationalNoVerdict);
        assert!(bytes.len() < 2048);
        for limit in [0, 65, u16::MAX] {
            let mut q = p.query.clone();
            q.limit = limit;
            assert!(encode_history_lookup(&q).is_err());
        }
        let mut b = bytes;
        b.push(0);
        assert!(decode_history_page(&b).is_err());
        assert!(decode_history_page(&vec![0; MAX_HISTORY_PAGE_BYTES + 1]).is_err());
    }
    #[test]
    fn missing_revision_wrong_cursor_and_forged_status_are_rejected() {
        let mut p = page();
        p.head_revision = 2;
        assert!(p.validate().is_err());
        let mut p = page();
        p.next_revision = Some(2);
        assert!(p.validate().is_err());
        let mut bytes = encode_history_page(&page()).unwrap();
        let idx = bytes.len() - 36;
        assert_eq!(bytes[idx], 1);
        bytes[idx] = 2;
        assert!(decode_history_page(&bytes)
            .unwrap_err()
            .contains("STATUS_NO_VERDICT_ONLY"));
    }
}
