//! R21 data classification. Hashes bind evidence, never authorize its classification.
//! Shadow must reconstruct every entry from its actual committed source records.
use super::{blob, digest, Hash, KieProof, Reader};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KieHistoryClass {
    LegacyUnscoped = 0,
    ProcessFailure = 1,
    ExactModelSupport = 2,
    ExactModelCounterproof = 3,
    DifferentScope = 4,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KieHistoryProvenance {
    pub transaction_id: Hash,
    pub proof_sha256: Hash,
    pub model_family_sha256: Hash,
}

/// Exact selection-row context; it adds no independent scientific source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KieHistoryContext {
    pub domain: String,
    pub term_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KieHistoryEntry {
    pub polarity: u8,
    pub candidate_id: u64,
    pub source_sha256: String,
    /// Absent in KHA1; present on every KHA2 row when short keys collide.
    pub selection_context: Option<KieHistoryContext>,
    pub class: KieHistoryClass,
    pub provenance: Option<KieHistoryProvenance>,
}

/// An actual native write for the complete scoped computational proposition.
/// It is distinct from lexical Xi/Yi relevance and never a fabricated term.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KieScopedEvidence {
    pub candidate: crate::CandidateStatisticWire,
    pub class: KieHistoryClass,
    pub provenance: KieHistoryProvenance,
}
impl KieScopedEvidence {
    pub fn key(&self)->(u8,u64,&str) {
        (self.candidate.polarity,self.candidate.candidate_id,self.candidate.source_sha256.as_str())
    }
}
impl KieHistoryEntry {
    pub fn key(&self) -> (u8, u64, &str) {
        (self.polarity, self.candidate_id, self.source_sha256.as_str())
    }
    pub fn selection_key(&self) -> ((u8, u64, &str), Option<(&str, &str)>) {
        (self.key(), self.selection_context.as_ref()
            .map(|context| (context.domain.as_str(), context.term_sha256.as_str())))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KieHistoryAssessment {
    pub scope_sha256: Hash,
    pub proposition_sha256: Hash,
    pub entries: Vec<KieHistoryEntry>,
    /// At most one historical observation per polarity of this ONE model family.
    /// Repeating the same computation is not an independent scientific source.
    pub scoped_evidence: Vec<KieScopedEvidence>,
}
impl KieHistoryAssessment {
    fn contextual(&self) -> bool {
        self.entries.first().is_some_and(|entry| entry.selection_context.is_some())
    }
    /// Borrow a row after assessment validation; classification is still data.
    pub fn entry_for(&self, candidate: &crate::CandidateStatisticWire) -> Option<&KieHistoryEntry> {
        let context = self.contextual()
            .then_some((candidate.domain.as_str(), candidate.term_sha256.as_str()));
        let key = ((candidate.polarity, candidate.candidate_id, candidate.source_sha256.as_str()), context);
        self.entries.binary_search_by(|entry| entry.selection_key().cmp(&key))
            .ok().map(|index| &self.entries[index])
    }
    fn validate_shape(&self) -> Result<(), String> {
        let contextual = self.entries.iter().any(|entry| entry.selection_context.is_some());
        if self.scope_sha256 == [0;32] || self.proposition_sha256 == [0;32]
            || self.entries.len() > 128
            || self.entries.windows(2).any(|p| p[0].selection_key() >= p[1].selection_key())
            || contextual && (self.entries.iter().any(|entry| entry.selection_context.is_none())
                || !self.entries.windows(2).any(|p| p[0].key() == p[1].key()))
            || self.scoped_evidence.len()>2
            || self.scoped_evidence.windows(2).any(|p|p[0].key()>=p[1].key()
                || p[0].candidate.polarity==p[1].candidate.polarity
                || p[0].provenance.model_family_sha256!=p[1].provenance.model_family_sha256) {
            return Err("KIE_HISTORY_ASSESSMENT_SHAPE".into());
        }
        for scoped in &self.scoped_evidence {
            let c=&scoped.candidate;let p=&scoped.provenance;
            if !c.shape_is_valid() || !matches!((scoped.class,c.polarity),
                (KieHistoryClass::ExactModelSupport,1)|(KieHistoryClass::ExactModelCounterproof,0))
                || [p.transaction_id,p.proof_sha256,p.model_family_sha256].contains(&[0;32]) {
                return Err("KIE_SCOPED_EVIDENCE_SHAPE".into());
            }
        }
        for entry in &self.entries {
            if entry.polarity > 1 || entry.source_sha256.len()!=64
                || !entry.source_sha256.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
                return Err("KIE_HISTORY_ENTRY_KEY".into());
            }
            if let Some(context) = &entry.selection_context {
                if context.domain.trim().is_empty() || context.domain.len() > super::MAX_PROOF_BYTES
                    || context.term_sha256.len() != 64
                    || !context.term_sha256.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
                    return Err("KIE_HISTORY_ENTRY_CONTEXT".into());
                }
            }
            if matches!(entry.class,KieHistoryClass::ExactModelSupport) && entry.polarity != 1
                || matches!(entry.class,KieHistoryClass::ExactModelCounterproof|KieHistoryClass::ProcessFailure) && entry.polarity != 0 {
                return Err("KIE_HISTORY_ENTRY_POLARITY".into());
            }
            match (&entry.class,&entry.provenance) {
                (KieHistoryClass::LegacyUnscoped,None) => {},
                (KieHistoryClass::LegacyUnscoped,Some(_)) | (_,None) => return Err("KIE_HISTORY_ENTRY_PROVENANCE".into()),
                (_,Some(p)) if [p.transaction_id,p.proof_sha256,p.model_family_sha256].contains(&[0;32]) =>
                    return Err("KIE_HISTORY_ENTRY_PROVENANCE".into()),
                _ => {},
            }
        }
        Ok(())
    }
    pub fn validate_for(&self, proof:&KieProof)->Result<(),String> {
        self.validate_shape()?;
        if self.scope_sha256!=proof.context.identity.key()?
            || self.proposition_sha256!=digest(proof.canonical_proposition.as_bytes()) {
            return Err("KIE_HISTORY_ASSESSMENT_SCOPE".into());
        }
        let q=crate::decode_selection_request(&proof.historical_request).map_err(|e|format!("KIE_HISTORY_REQUEST:{e}"))?;
        let s=crate::decode_selection_response(&proof.historical_response).map_err(|e|format!("KIE_HISTORY_RESPONSE:{e}"))?;
        let projection=crate::pd_memory::failure::relevance::project_available_bound(&q,&s)?;
        let contextual = self.contextual();
        let mut keys=projection.positive.iter().chain(projection.negative.iter()).map(|value| {
            let c = value.candidate;
            ((c.polarity, c.candidate_id, c.source_sha256.as_str()),
                contextual.then_some((c.domain.as_str(), c.term_sha256.as_str())))
        }).collect::<Vec<_>>();
        keys.sort_unstable();
        if keys.len()!=self.entries.len() || keys.iter().copied().ne(self.entries.iter().map(KieHistoryEntry::selection_key)) {
            return Err("KIE_HISTORY_ASSESSMENT_COVERAGE".into());
        }
        for scoped in &self.scoped_evidence {
            let c=&scoped.candidate;
            let domain=match (c.polarity,c.domain.as_str()) {
                (1,"primitive")=>crate::pd_memory::failure::relevance::TERM_PRIMITIVE_DOMAIN,
                (1,"legacy")=>crate::pd_memory::failure::relevance::TERM_LEGACY_DOMAIN,
                (1,_)=>return Err("KIE_SCOPED_EVIDENCE_POSITIVE_DOMAIN".into()),
                (0,_)=>crate::pd_memory::failure::relevance::TERM_NEGATIVE_DOMAIN,
                _=>return Err("KIE_SCOPED_EVIDENCE_POLARITY".into()),
            };
            if c.term_sha256!=crate::sha256_hex(&[domain,proof.canonical_proposition.as_bytes()]) {
                return Err("KIE_SCOPED_EVIDENCE_EXACT_PROPOSITION".into());
            }
        }
        Ok(())
    }
    pub fn encode(&self)->Result<Vec<u8>,String> {
        self.validate_shape()?;
        let mut out=if self.contextual() {b"KHA2".to_vec()} else {b"KHA1".to_vec()};
        out.extend_from_slice(&self.scope_sha256);
        out.extend_from_slice(&self.proposition_sha256);
        out.extend_from_slice(&(self.entries.len() as u32).to_le_bytes());
        for e in &self.entries {
            out.push(e.polarity);
            out.extend_from_slice(&e.candidate_id.to_le_bytes());
            blob(&mut out,e.source_sha256.as_bytes())?;
            if let Some(context) = &e.selection_context {
                blob(&mut out, context.domain.as_bytes())?;
                blob(&mut out, context.term_sha256.as_bytes())?;
            }
            out.push(e.class as u8);
            out.push(u8::from(e.provenance.is_some()));
            if let Some(p)=&e.provenance {
                out.extend_from_slice(&p.transaction_id);
                out.extend_from_slice(&p.proof_sha256);
                out.extend_from_slice(&p.model_family_sha256);
            }
        }
        out.extend_from_slice(&(self.scoped_evidence.len() as u32).to_le_bytes());
        for scoped in &self.scoped_evidence {
            let mut w=crate::Writer::new();
            crate::put_candidate_statistic(&mut w,&scoped.candidate);
            blob(&mut out,&w.bytes)?;
            out.push(scoped.class as u8);
            out.extend_from_slice(&scoped.provenance.transaction_id);
            out.extend_from_slice(&scoped.provenance.proof_sha256);
            out.extend_from_slice(&scoped.provenance.model_family_sha256);
        }
        if out.len() > super::MAX_WIRE_BYTES { return Err("KIE_WIRE_LIMIT".into()); }
        Ok(out)
    }
    pub fn decode(bytes:&[u8])->Result<Self,String> {
        let contextual = bytes.starts_with(b"KHA2");
        let mut r=Reader::new(bytes,if contextual {b"KHA2"} else {b"KHA1"})?;
        let scope_sha256=r.hash()?;
        let proposition_sha256=r.hash()?;
        let n=r.u32()? as usize;
        if n>128 {return Err("KIE_HISTORY_ASSESSMENT_LIMIT".into());}
        let mut entries=Vec::with_capacity(n);
        for _ in 0..n {
            let polarity=r.take(1)?[0];
            let candidate_id=u64::from_le_bytes(r.take(8)?.try_into().map_err(|_|"KIE_HISTORY_ENTRY_ID")?);
            let source_sha256=String::from_utf8(r.bytes(64)?).map_err(|_|"KIE_HISTORY_ENTRY_KEY")?;
            let selection_context = if contextual {Some(KieHistoryContext {
                domain: String::from_utf8(r.bytes(super::MAX_PROOF_BYTES)?)
                    .map_err(|_| "KIE_HISTORY_ENTRY_CONTEXT")?,
                term_sha256: String::from_utf8(r.bytes(64)?)
                    .map_err(|_| "KIE_HISTORY_ENTRY_CONTEXT")?,
            })} else {None};
            let class=match r.take(1)?[0] {
                0=>KieHistoryClass::LegacyUnscoped,1=>KieHistoryClass::ProcessFailure,
                2=>KieHistoryClass::ExactModelSupport,3=>KieHistoryClass::ExactModelCounterproof,
                4=>KieHistoryClass::DifferentScope,_=>return Err("KIE_HISTORY_ENTRY_CLASS".into()),
            };
            let provenance=match r.take(1)?[0] {
                0=>None,
                1=>Some(KieHistoryProvenance{transaction_id:r.hash()?,proof_sha256:r.hash()?,model_family_sha256:r.hash()?}),
                _=>return Err("KIE_HISTORY_ENTRY_PROVENANCE".into()),
            };
            entries.push(KieHistoryEntry{polarity,candidate_id,source_sha256,selection_context,class,provenance});
        }
        let n=r.u32()? as usize;
        if n>2 {return Err("KIE_SCOPED_EVIDENCE_LIMIT".into());}
        let mut scoped_evidence=Vec::with_capacity(n);
        for _ in 0..n {
            let bytes=r.bytes(16*1024)?;
            let mut c=crate::Reader::new(&bytes);
            let candidate=crate::get_candidate_statistic(&mut c).map_err(|e|format!("KIE_SCOPED_CANDIDATE:{e}"))?;
            c.finish().map_err(|e|format!("KIE_SCOPED_CANDIDATE:{e}"))?;
            let class=match r.take(1)?[0] {2=>KieHistoryClass::ExactModelSupport,3=>KieHistoryClass::ExactModelCounterproof,
                _=>return Err("KIE_SCOPED_EVIDENCE_CLASS".into())};
            let provenance=KieHistoryProvenance{transaction_id:r.hash()?,proof_sha256:r.hash()?,model_family_sha256:r.hash()?};
            scoped_evidence.push(KieScopedEvidence{candidate,class,provenance});
        }
        r.end()?;
        let value=Self{scope_sha256,proposition_sha256,entries,scoped_evidence};
        if value.encode()?!=bytes {return Err("KIE_HISTORY_ASSESSMENT_CANONICAL".into());}
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assessment(contextual: bool) -> KieHistoryAssessment {
        let entry = KieHistoryEntry {
            polarity: 0, candidate_id: 7, source_sha256: "a".repeat(64),
            selection_context: contextual.then(|| KieHistoryContext {
                domain: "context-a".into(), term_sha256: "b".repeat(64),
            }),
            class: KieHistoryClass::ExactModelCounterproof,
            provenance: Some(KieHistoryProvenance {
                transaction_id: [3; 32], proof_sha256: [4; 32], model_family_sha256: [5; 32],
            }),
        };
        let mut entries = vec![entry];
        if contextual {
            let mut other = entries[0].clone();
            other.selection_context.as_mut().unwrap().domain = "context-b".into();
            other.class = KieHistoryClass::DifferentScope;
            other.provenance.as_mut().unwrap().transaction_id = [6; 32];
            entries.push(other);
        }
        KieHistoryAssessment {
            scope_sha256: [1; 32], proposition_sha256: [2; 32], entries, scoped_evidence: vec![],
        }
    }

    #[test]
    fn context_codec_preserves_nonempty_kha1_golden_bytes() {
        // Fixed pre-context wire layout, including its original provenance.
        const GOLDEN: &str = "4b48413101010101010101010101010101010101010101010101010101010101010101010202020202020202020202020202020202020202020202020202020202020202010000000007000000000000004000000061616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161616161030103030303030303030303030303030303030303030303030303030303030303030404040404040404040404040404040404040404040404040404040404040404050505050505050505050505050505050505050505050505050505050505050500000000";
        let bytes = (0..GOLDEN.len()).step_by(2)
            .map(|index| u8::from_str_radix(&GOLDEN[index..index + 2], 16).unwrap())
            .collect::<Vec<_>>();
        let expected = assessment(false);
        assert_eq!(expected.encode().unwrap(), bytes);
        let decoded = KieHistoryAssessment::decode(&bytes).unwrap();
        assert_eq!(decoded, expected);
        assert_eq!(decoded.encode().unwrap(), bytes);
        assert!(decoded.entries.iter().all(|entry| entry.selection_context.is_none()));
    }

    #[test]
    fn context_codec_round_trip_and_exact_borrowed_lookup() {
        let expected = assessment(true);
        let bytes = expected.encode().unwrap();
        assert!(bytes.starts_with(b"KHA2"));
        let actual = KieHistoryAssessment::decode(&bytes).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(actual.encode().unwrap(), bytes);
        let mut long_domain = expected.clone();
        long_domain.entries[0].selection_context.as_mut().unwrap().domain =
            "a".repeat(crate::kie::MAX_TEXT_BYTES + 1);
        assert_eq!(KieHistoryAssessment::decode(&long_domain.encode().unwrap()).unwrap(), long_domain);
        let mut candidate = crate::CandidateStatisticWire {
            candidate_id: 7, polarity: 0, domain: "context-b".into(),
            term_sha256: "b".repeat(64), source_sha256: "a".repeat(64),
            security_y: 0, trust_x: 0, confidence_fixed: 5000,
            frequency: 1, domain_count: 1, legacy_score_fixed: 0,
        };
        assert_eq!(actual.entry_for(&candidate), Some(&actual.entries[1]));
        candidate.domain = "context-a".into();
        assert_eq!(actual.entry_for(&candidate), Some(&actual.entries[0]));
        candidate.domain.push('!');
        assert!(actual.entry_for(&candidate).is_none());
        candidate.domain = "context-a".into();
        candidate.term_sha256 = "c".repeat(64);
        assert!(actual.entry_for(&candidate).is_none());
        candidate.term_sha256 = "b".repeat(64);
        candidate.source_sha256 = "d".repeat(64);
        assert!(actual.entry_for(&candidate).is_none());
    }

    #[test]
    fn context_codec_rejects_noncanonical_and_unbounded_rows() {
        let valid = assessment(true);
        for variant in 0..9 {
            let mut invalid = valid.clone();
            match variant {
                0 => for entry in &mut invalid.entries { entry.selection_context = None; },
                1 => invalid.entries[0].selection_context = None,
                2 => { invalid.entries.pop(); },
                3 => invalid.entries[1].selection_context = invalid.entries[0].selection_context.clone(),
                4 => invalid.entries.reverse(),
                5 => invalid.entries[0].selection_context.as_mut().unwrap().term_sha256 = "B".repeat(64),
                6 => invalid.entries[0].selection_context.as_mut().unwrap().domain = " ".into(),
                7 => invalid.entries[0].selection_context.as_mut().unwrap().domain = "x".repeat(crate::kie::MAX_PROOF_BYTES + 1),
                8 => invalid.entries[0].selection_context.as_mut().unwrap().term_sha256.pop().map(|_| ()).unwrap(),
                _ => unreachable!(),
            }
            assert!(invalid.encode().is_err(), "variant {variant}");
        }
        let mut oversized_wire = valid.clone();
        oversized_wire.entries[0].selection_context.as_mut().unwrap().domain = "a".repeat(262_000);
        oversized_wire.entries[1].selection_context.as_mut().unwrap().domain = "b".repeat(261_750);
        assert!(oversized_wire.encode().is_err());
        let bytes = valid.encode().unwrap();
        let mut downgraded = valid.clone();
        for entry in &mut downgraded.entries { entry.selection_context = None; }
        assert_eq!(downgraded.encode().unwrap_err(), "KIE_HISTORY_ASSESSMENT_SHAPE");
        let mut downgraded_wire = bytes.clone();
        downgraded_wire[..4].copy_from_slice(b"KHA1");
        assert!(KieHistoryAssessment::decode(&downgraded_wire).is_err());
        for end in 0..bytes.len() {
            assert!(KieHistoryAssessment::decode(&bytes[..end]).is_err(), "prefix {end}");
        }
        let mut oversized_domain = bytes.clone();
        // KHA2 header, polarity, id and the fixed-length source hash precede the domain.
        oversized_domain[149..153].copy_from_slice(&((crate::kie::MAX_PROOF_BYTES + 1) as u32).to_le_bytes());
        assert!(KieHistoryAssessment::decode(&oversized_domain).is_err());
        let mut invalid_utf8 = bytes.clone();
        invalid_utf8[153] = 0xff;
        assert!(KieHistoryAssessment::decode(&invalid_utf8).is_err());
        let mut trailing = bytes;
        trailing.push(0);
        assert!(KieHistoryAssessment::decode(&trailing).is_err());
        let mut empty = assessment(false);
        empty.entries.clear();
        let mut wrong_version = empty.encode().unwrap();
        wrong_version[..4].copy_from_slice(b"KHA2");
        assert!(KieHistoryAssessment::decode(&wrong_version).is_err());
        assert!(KieHistoryAssessment::decode(&vec![0; crate::kie::MAX_WIRE_BYTES + 1]).is_err());
    }
}
