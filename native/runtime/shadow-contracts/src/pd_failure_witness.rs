//! R10: data-only witness of the exact completed Quantum proposal/refusal.
//! Shadow must compare it with the actual request and independently checked KiE.
//! No count of turns, witness, hash, or model predicate grants an authority.
use crate::{kie, Reader, ShadowCycleRequest, Writer};
use sha2::{Digest, Sha256};

pub type Hash = [u8; 32];
pub const MAX_WITNESS_BYTES: usize = 64 * 1024;
const MAGIC: &[u8; 8] = b"ESSPFW01";

fn sha(bytes: &[u8]) -> Hash { Sha256::digest(bytes).into() }

/// Display-only escape for the legacy line transport. The witness retains the
/// exact refusal bytes; the rendered question is escaped before its digest is
/// derived. Backslash is escaped too, so these visible sequences are reversible.
fn display_reason(reason: &str) -> String {
    let mut out = String::with_capacity(reason.len());
    for ch in reason.chars() {
        match ch {
            '\\' => out.push_str(r"\\"),
            '|' => out.push_str(r"\u{7c}"),
            '\n' => out.push_str(r"\n"),
            '\r' => out.push_str(r"\r"),
            '\t' => out.push_str(r"\t"),
            other if other.is_control() => out.extend(other.escape_unicode()),
            other => out.push(other),
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KieModelAssessment {
    pub proof_sha256: Hash,
    pub relation: u8,
    pub kis: u32,
    pub kic: u32,
    pub historical_positive_count: u32,
    pub historical_negative_count: u32,
    pub historical_positive_mass: u64,
    pub historical_negative_mass: u64,
    pub historical_veto: bool,
    /// R21: exact classified-history data consumed by the KiE gate. The
    /// witness records that computation; it neither judges nor issues a veto.
    /// None preserves the original V1 witness wire and replay semantics.
    pub history_assessment_sha256: Option<Hash>,
    pub unresolved_history_count: u32,
}
impl KieModelAssessment {
    pub fn from_proof(proof: &kie::KieProof) -> Result<Self, String> {
        proof.validate()?;
        let c = kie::combined_evaluation(proof)?;
        Ok(Self {
            proof_sha256: proof.proof_sha256, relation: proof.relation,
            kis: proof.kis_mass, kic: proof.kic_mass,
            historical_positive_count: u32::try_from(c.historical_positive_count).map_err(|_| "PD_WITNESS_COUNT")?,
            historical_negative_count: u32::try_from(c.historical_negative_count).map_err(|_| "PD_WITNESS_COUNT")?,
            historical_positive_mass: c.historical_positive_mass,
            historical_negative_mass: c.historical_negative_mass,
            historical_veto: c.historical_veto,
            history_assessment_sha256: proof.history_assessment_digest()?,
            unresolved_history_count: u32::try_from(c.unresolved_history_count).map_err(|_| "PD_WITNESS_COUNT")?,
        })
    }
    fn validate(&self) -> Result<(), String> {
        let masses = match self.relation {
            0 => (kie::PREDICATE_SCALE, 0), 1 => (0, kie::PREDICATE_SCALE),
            2 => (0, 0), _ => return Err("PD_WITNESS_MODEL_RELATION".into()),
        };
        // V1 is replayed under its original arithmetic. V2 only validates the
        // recorded data shape here; validate_model binds it to the separately
        // computed and source-checked proof at admission/replay.
        let policy_shape = match self.history_assessment_sha256 {
            None => self.unresolved_history_count == 0 && kie::historical_veto_v1(
                u64::from(self.historical_positive_count),
                u64::from(self.historical_negative_count),
                self.historical_positive_mass,
                self.historical_negative_mass,
            ) == self.historical_veto,
            Some(hash) => hash != [0; 32],
        };

        if self.proof_sha256 == [0;32] || (self.kis,self.kic) != masses
            || !policy_shape
            || self.historical_positive_count > u32::from(crate::SELECTION_MAX_CANDIDATES)
            || self.historical_negative_count > u32::from(crate::SELECTION_MAX_CANDIDATES)
            || self.unresolved_history_count > u32::from(crate::SELECTION_MAX_CANDIDATES) {
            return Err("PD_WITNESS_MODEL_SHAPE".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdFailureWitness {
    pub request_sha256: Hash,
    pub input_sha256: Hash,
    /// Conditions reported by the actual BESA/ASHT/split request material.
    /// This does not pretend to be a journal of a crash before that request.
    pub preconditions_sha256: Hash,
    pub source_trace_digest: u64,
    pub negative_path_count: u32,
    pub refusal: Option<String>,
    pub model: Option<KieModelAssessment>,
    /// V3 only. Old witnesses retain their original question and exact bytes.
    pub inquiry: Option<crate::iz_frontier::IzFrontier>,
    /// Digest binding the exact request, recorded conditions and observed outcome.
    /// This is a data witness, not an authority token or a complete event journal.
    pub lgc_tip: Hash,
}
impl PdFailureWitness {
    pub fn from_request(request: &ShadowCycleRequest, proof: Option<&kie::KieProof>) -> Result<Self, String> {

        let q = &request.quantum;
        let m = &q.final_evidence.muscle_evidence;
        if m.asht_hold_id.starts_with("kie-") != proof.is_some()
            || proof.is_some_and(|p| p.canonical_proposition.as_bytes() != q.final_evidence.light_input_bytes) {
            return Err("PD_WITNESS_KIE_ROUTE".into());
        }
        let mut conditions = Writer::new();
        for value in [&m.besa_hold_sha256, &m.asht_hold_sha256, &m.split_sha256] { conditions.string(value); }
        conditions.string(q.pd_parent_i0.as_deref().ok_or("PD_WITNESS_PARENT")?);
        let mut value = Self {
            request_sha256: sha(&crate::encode_cycle_request(request)),
            input_sha256: sha(&q.final_evidence.light_input_bytes),
            preconditions_sha256: sha(&conditions.bytes),
            source_trace_digest: q.final_evidence.pd_spine.trace_digest,
            negative_path_count: u32::try_from(q.negative_paths.len()).map_err(|_| "PD_WITNESS_PATH_COUNT")?,
            refusal: q.collapse_refused.clone(),
            model: proof.map(KieModelAssessment::from_proof).transpose()?,
            inquiry: None,
            lgc_tip: [0;32],
        };
        value.lgc_tip = sha(&value.body());
        value.validate()?;
        Ok(value)
    }
    pub fn from_request_current(request: &ShadowCycleRequest, proof: Option<&kie::KieProof>) -> Result<Self, String> {
        let mut value=Self::from_request(request,proof)?;
        let mut inquiry=crate::iz_frontier::IzFrontier::from_current_request(value.refusal.as_deref(),request,proof)?;
        if inquiry.uses_current_processing() {
            if let Some(proof)=proof { inquiry=inquiry.with_historical_branch(proof)?; }
        }
        value.inquiry=Some(inquiry);
        value.lgc_tip=sha(&value.body());
        value.validate()?;
        value.validate_model(proof)?;
        Ok(value)
    }
    /// Attach the current provider's numerical result as data. Shadow still
    /// recomputes that provider result before admitting this witness.
    pub fn with_model_measurement(mut self,proof:&kie::KieProof,
        measurement:crate::kie_measurement::KieMeasurement)->Result<Self,String> {
        self.inquiry=Some(self.inquiry.take().ok_or("PD_WITNESS_CURRENT_INQUIRY_MISSING")?
            .with_model_measurement(proof,measurement)?);
        self.lgc_tip=sha(&self.body());
        self.validate()?;self.validate_model(Some(proof))?;
        Ok(self)
    }
    pub fn validate(&self) -> Result<(), String> {
        if [self.request_sha256,self.input_sha256,self.preconditions_sha256,self.lgc_tip].contains(&[0;32])
            || self.source_trace_digest == 0
            || self.refusal.as_ref().is_some_and(|s| s.trim().is_empty() || s.len() > 8192 || s.contains('\0'))
            || sha(&self.body()) != self.lgc_tip {
            return Err("PD_WITNESS_BINDING".into());
        }
        if let Some(model) = &self.model { model.validate()?; }
        if let Some(inquiry)=&self.inquiry {
            inquiry.validate()?;
            inquiry.validate_refusal(self.refusal.as_deref())?;
            inquiry.validate_request_hash(&self.request_sha256)?;
            // Bound the final transport composition arithmetically. Native
            // question text is still formed only after the receipt checks.
            let reason=self.refusal.as_deref().unwrap_or("shkaku i autoritetit pasues ende nuk eshte deklaruar");
            let escaped=reason.chars().map(|ch|match ch {
                '\\' | '\n' | '\r' | '\t' => 2, '|' => 6,
                other if other.is_control() => other.escape_unicode().count(),
                other => other.len_utf8(),
            }).sum::<usize>();
            if 512 + escaped + inquiry.rendered_byte_bound()? > crate::iz_frontier::MAX_RENDER_BYTES {
                return Err("PD_WITNESS_QUESTION_LIMIT".into());
            }
        }
        Ok(())
    }
    pub fn validate_for_request(&self, request: &ShadowCycleRequest) -> Result<(), String> {
        self.validate()?;
        let q = &request.quantum;
        let m = &q.final_evidence.muscle_evidence;
        let mut conditions = Writer::new();
        for value in [&m.besa_hold_sha256, &m.asht_hold_sha256, &m.split_sha256] { conditions.string(value); }
        conditions.string(q.pd_parent_i0.as_deref().ok_or("PD_WITNESS_PARENT")?);
        if self.request_sha256 != sha(&crate::encode_cycle_request(request))
            || self.input_sha256 != sha(&q.final_evidence.light_input_bytes)
            || self.preconditions_sha256 != sha(&conditions.bytes)
            || self.source_trace_digest != q.final_evidence.pd_spine.trace_digest
            || self.negative_path_count as usize != q.negative_paths.len()
            || self.refusal != q.collapse_refused
            || m.asht_hold_id.starts_with("kie-") != self.model.is_some() {
            return Err("PD_WITNESS_ACTUAL_REQUEST".into());
        }
        if let Some(inquiry)=&self.inquiry { inquiry.validate_current_request(request)?; }
        Ok(())
    }
    pub fn validate_model(&self, proof: Option<&kie::KieProof>) -> Result<(), String> {
        self.validate()?;
        if self.model != proof.map(KieModelAssessment::from_proof).transpose()? {
            return Err("PD_WITNESS_ACTUAL_MODEL".into());
        }
        if let Some(inquiry)=&self.inquiry { inquiry.validate_model(proof)?; }
        Ok(())
    }
    /// Called only after the existing receipt/negative-persistence checks.
    /// This question describes the recorded failure; it is never a future i0.
    pub fn verified_model_question(&self) -> Result<String, String> {
        self.validate()?;
        let inquiry=self.inquiry.as_ref().ok_or("PD_WITNESS_MODEL_OUTPUT_MISSING")?;
        if inquiry.model_operators().is_none() || inquiry.model_measurement().is_none() {
            return Err("PD_WITNESS_MODEL_OUTPUT_MISSING".into());
        }
        let question=format!("iZ — SHADOW konfirmoi (1,1) vetem per pohimin nen modelin e deklaruar; kjo nuk verteton nje pretendim me te gjere. {} LGC={} request_sha256={}. Pyetjet ruajne mungesat e rrjedhes origjinale; nuk parashikojne i0 dhe nuk japin autoritet te ri.",
            inquiry.render_verified_model()?,crate::pd_memory::hex(&self.lgc_tip),crate::pd_memory::hex(&self.request_sha256));
        if question.len()>crate::iz_frontier::MAX_RENDER_BYTES {
            return Err("PD_WITNESS_QUESTION_LIMIT".into());
        }
        Ok(question)
    }
    pub fn negative_question(&self) -> Result<String, String> {
        self.validate()?;
        if let Some(inquiry)=&self.inquiry {
            let reason=display_reason(self.refusal.as_deref().unwrap_or("shkaku i autoritetit pasues ende nuk eshte deklaruar"));
            let question=format!("iZ — D-BAB deshmon kushtin aktual: {reason}. {} LGC={} request_sha256={}. Pyetja nuk parashikon i0 dhe nuk jep autoritet apo (1,1).",
                inquiry.render()?,crate::pd_memory::hex(&self.lgc_tip),crate::pd_memory::hex(&self.request_sha256));
            if question.len()>crate::iz_frontier::MAX_RENDER_BYTES {
                return Err("PD_WITNESS_QUESTION_LIMIT".into());
            }
            return Ok(question);
        }
        let mut details = String::new();
        if let Some(m) = &self.model {
            let label = match m.relation {
                0 => "KiS mbeshtet pohimin nen modelin e deklaruar; kjo nuk provon zbatueshmerine e tij ndaj rastit",
                1 => "KiC kundershton pohimin nen modelin e deklaruar",
                _ => "KiE mbetet i papercaktuar; barazia nuk eshte kunderprove",
            };
            details = if let Some(hash) = m.history_assessment_sha256 {
                format!("{label}. Historiku i klasifikuar per kete pohim: P={} me mase {}, N={} me mase {}; veto_historike={}; prejardhje_e_pazgjidhur={}; classification_sha256={}. Masat historike nuk jane pesha te KiS/KiC. ",
                    m.historical_positive_count,m.historical_positive_mass,
                    m.historical_negative_count,m.historical_negative_mass,m.historical_veto,
                    m.unresolved_history_count,
                    crate::pd_memory::hex(&hash))
            } else {
                format!("{label}. Historiku relevant: P={} me mase {}, N={} me mase {}, veto={}. ",
                    m.historical_positive_count,m.historical_positive_mass,
                    m.historical_negative_count,m.historical_negative_mass,m.historical_veto)
            };
        }
        let reason = display_reason(self.refusal.as_deref().unwrap_or("propozimi Quantum u dergua, por verifikimi final Shadow nuk dha pranim pozitiv; shkaku i nje autoriteti specifik nuk deklarohet ketu"));
        let ask = if self.model.as_ref().is_some_and(|m| m.historical_veto) {
            "Cila prove e kontrollueshme adreson kundershtimin historik per te njejtin Xi/Yi, dhe si krahasohet me proven mbeshtetese?"
        } else if self.model.as_ref().is_some_and(|m| m.relation == 1) {
            "Cilin input, kusht ose prove te kontrollueshme propozon per ta testuar kundershtimin KiC, bashke me proven mbeshtetese?"
        } else if self.model.as_ref().is_some_and(|m| m.unresolved_history_count > 0) {
            "Cila evidence lidh rekordet me prejardhje te pazgjidhur me pohimin dhe kontekstin ekzakt? Pa kete lidhje ato mbeten kushte per sqarim, jo kunderprove e deklaruar dhe jo leje per pranim."
        } else if self.model.as_ref().is_some_and(|m| m.relation == 2) {
            "Cila matje, saktesi ose prove mbeshtetese dhe kundershtuese e zgjidh barazine pa ndryshuar arbitrarisht tolerancen?"
        } else if self.refusal.as_ref().is_some_and(|r| r.starts_with("OPERATOR_EVIDENCE_UNRESOLVED:")) {
            "Xi/Yi ruhen. Cila prove me burim dhe cili dallim konkret mbeshtet lidhjen shkakore te propozuar, dhe cila prove e kundershton? Dobesia e operatorit nuk verteton qe pohimi eshte i gabuar."
        } else if self.refusal.as_ref().is_some_and(|r| r.starts_with("hapësirë e njëanshme:") || r.starts_with("vetem nje ane primitive:")) {
            "Ana primitive e pranishme ruhet. Cili element i anes Xi ose Yi qe mungon, me burim dhe prove mbeshtetese e kundershtuese, ploteson dallimin e ketij rasti? Mungesa e njeres ane nuk verteton se pohimi eshte i gabuar."
        } else if self.refusal.as_ref().is_some_and(|r| r.starts_with("XiYi=∅: asnjë bashkëprezencë")) {
            "Kandidatet Xi/Yi ruhen. Cili fragment me burim dokumenton bashkeprezencen e tyre ose sqaron lidhjen e propozuar, dhe cila prove e kundershton? Mungesa e bashkeprezences ne fragmentet aktuale nuk eshte vete kunderprove ndaj pohimit."
        } else if self.refusal.as_ref().is_some_and(|r| r.starts_with("pa material primitiv:")) {
            "Cili dallim konkret Xi/Yi, me burim dhe prove mbeshtetese e kundershtuese, e ben propozimin te shqyrtueshem? Mungesa e materialit per procesim nuk eshte vete kunderprove ndaj pohimit."
        } else {
            "Cila prove dhe cili kusht i kontrollueshem adreson kete refuzim, duke ruajtur edhe kundershtimin?"
        };
        Ok(format!("iZ — {details}D-BAB deshmon rezultatin e procesit: {reason}. {ask} LGC={} request_sha256={}. Ky eshte kerkim prove per vazhdimin; nuk parashikon i0 dhe nuk jep (1,1) nga numri i cikleve.",
            crate::pd_memory::hex(&self.lgc_tip),crate::pd_memory::hex(&self.request_sha256)))
    }
    fn body(&self) -> Vec<u8> {
        let version = if self.inquiry.is_some() {3} else if self.model.as_ref().is_some_and(|m| m.history_assessment_sha256.is_some()) { 2 } else { 1 };
        let mut w=Writer::new();w.bytes.extend_from_slice(MAGIC);w.u16(version);
        for h in [self.request_sha256,self.input_sha256,self.preconditions_sha256] {w.bytes.extend_from_slice(&h);}
        w.u64(self.source_trace_digest);w.u32(self.negative_path_count);
        match &self.refusal {None=>w.u8(0),Some(s)=>{w.u8(1);w.string(s);}}
        match &self.model {None=>w.u8(0),Some(m)=>{
            w.u8(1);w.bytes.extend_from_slice(&m.proof_sha256);w.u8(m.relation);w.u32(m.kis);w.u32(m.kic);
            w.u32(m.historical_positive_count);w.u32(m.historical_negative_count);
            w.u64(m.historical_positive_mass);w.u64(m.historical_negative_mass);w.u8(u8::from(m.historical_veto));
            if version==3 { w.u8(u8::from(m.history_assessment_sha256.is_some())); }
            if let Some(hash) = m.history_assessment_sha256 {
                w.bytes.extend_from_slice(&hash);w.u32(m.unresolved_history_count);
            }
        }}
        if let Some(inquiry)=&self.inquiry {
            // Validated callers reject an invalid frontier before accepting the
            // witness; keep hashing total so malformed data cannot cause panic.
            match inquiry.encode() { Ok(bytes)=>w.bytes(&bytes), Err(_)=>w.bytes(&[]) }
        }
        w.bytes
    }
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        self.validate()?;let mut bytes=self.body();bytes.extend_from_slice(&self.lgc_tip);
        if bytes.len()>MAX_WITNESS_BYTES{return Err("PD_WITNESS_LIMIT".into());} Ok(bytes)
    }
    pub fn decode(bytes:&[u8])->Result<Self,String>{
        if bytes.len()>MAX_WITNESS_BYTES{return Err("PD_WITNESS_LIMIT".into());}
        let parsed=(||->Result<Self,crate::WireError>{
            let mut r=Reader::new(bytes);
            if r.take(8)?!=MAGIC{return Err(crate::WireError::InvalidMagic);}
            let version=r.u16()?;
            if !matches!(version,1|2|3){return Err(crate::WireError::InvalidMagic);}
            fn hash(r:&mut Reader<'_>)->Result<Hash,crate::WireError>{let mut h=[0;32];h.copy_from_slice(r.take(32)?);Ok(h)}
            fn marker(r:&mut Reader<'_>)->Result<bool,crate::WireError>{match r.u8()?{0=>Ok(false),1=>Ok(true),v=>Err(crate::WireError::InvalidBool(v))}}
            let request_sha256=hash(&mut r)?;let input_sha256=hash(&mut r)?;let preconditions_sha256=hash(&mut r)?;
            let source_trace_digest=r.u64()?;let negative_path_count=r.u32()?;
            let refusal=if marker(&mut r)?{Some(r.string()?)}else{None};
            let model=if marker(&mut r)?{Some(KieModelAssessment{
                proof_sha256:hash(&mut r)?,relation:r.u8()?,kis:r.u32()?,kic:r.u32()?,
                historical_positive_count:r.u32()?,historical_negative_count:r.u32()?,
                historical_positive_mass:r.u64()?,historical_negative_mass:r.u64()?,historical_veto:marker(&mut r)?,
                history_assessment_sha256: None,
                unresolved_history_count: 0,
            })}else{None};
            let mut model=model;
            if let Some(m)=&mut model {
                let classified=version==2 || (version==3 && marker(&mut r)?);
                if classified {m.history_assessment_sha256=Some(hash(&mut r)?);m.unresolved_history_count=r.u32()?;}
            }
            if version==2 && model.is_none(){return Err(crate::WireError::InvalidMagic);}
            let inquiry=if version==3 {Some(crate::iz_frontier::IzFrontier::decode(&r.bytes()?)
                .map_err(|_|crate::WireError::InvalidMagic)?)}else{None};
            let lgc_tip=hash(&mut r)?;r.finish()?;
            Ok(Self{request_sha256,input_sha256,preconditions_sha256,source_trace_digest,negative_path_count,refusal,model,inquiry,lgc_tip})
        })().map_err(|e|format!("PD_WITNESS_CODEC:{e:?}"))?;
        parsed.validate()?;if parsed.encode()?!=bytes{return Err("PD_WITNESS_NONCANONICAL".into());}Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(relation:u8,reason:&str)->PdFailureWitness {
        let mut v=PdFailureWitness{request_sha256:[1;32],input_sha256:[2;32],preconditions_sha256:[3;32],
            source_trace_digest:4,negative_path_count:2,refusal:Some(reason.into()),model:Some(KieModelAssessment{
                proof_sha256:[5;32],relation,kis:if relation==0{10000}else{0},kic:if relation==1{10000}else{0},
                historical_positive_count:0,historical_negative_count:0,historical_positive_mass:0,historical_negative_mass:0,historical_veto:false,
                history_assessment_sha256:None,
                unresolved_history_count:0,
            }),inquiry:None,lgc_tip:[0;32]};v.lgc_tip=sha(&v.body());v
    }
    fn full_scoped_proof() -> kie::KieProof {
        // Synthetic shape fixture, not provider execution or native admission.
        let mut proof=crate::iz_frontier::test_proof(1);
        let mut response=crate::decode_selection_response(&proof.historical_response).unwrap();
        let provenance=kie::KieHistoryProvenance { transaction_id:[6;32],proof_sha256:[7;32],model_family_sha256:[8;32] };
        let candidate=|id:u8|crate::CandidateStatisticWire {
            candidate_id:u64::from(id),polarity:0,domain:"negative".into(),
            term_sha256:crate::sha256_hex(&[crate::pd_memory::failure::relevance::TERM_NEGATIVE_DOMAIN,b"typed-model"]),
            source_sha256:crate::pd_memory::hex(&[id;32]),security_y:1,trust_x:1,
            confidence_fixed:3500,frequency:1,domain_count:1,legacy_score_fixed:3500,
        };
        response.negative=(1..=127).map(candidate).collect();
        let assessment=proof.history_assessment.as_mut().unwrap();
        assessment.entries=response.negative.iter().map(|c|kie::KieHistoryEntry {
            selection_context:None,
            polarity:0,candidate_id:c.candidate_id,source_sha256:c.source_sha256.clone(),
            class:kie::KieHistoryClass::ExactModelCounterproof,provenance:Some(provenance.clone()),
        }).collect();
        assessment.scoped_evidence=vec![kie::KieScopedEvidence { candidate:candidate(128),
            class:kie::KieHistoryClass::ExactModelCounterproof,provenance }];
        response.selection_sha256=crate::pd_memory::failure::relevance::selection_digest(&response);
        proof.historical_response=crate::encode_selection_response(&response);
        proof.seal_data().unwrap()
    }
    fn current_fixture(proof:&kie::KieProof,reason:&str)->PdFailureWitness {
        let mut witness=fixture(proof.relation,reason);
        witness.model=Some(KieModelAssessment::from_proof(proof).unwrap());
        witness.inquiry=Some(crate::iz_frontier::IzFrontier::from_observation(Some(reason),Some(proof)).unwrap());
        witness.lgc_tip=sha(&witness.body());
        witness
    }
    // Captured transport data is used only for the inquiry projection tests.
    // Mutated observations below are synthetic data, never native admission.
    fn r27_request_fixture() -> (crate::ShadowCycleRequest, kie::KieProof) {
        let bytes=include_bytes!("../../shadow/src/test_fixtures/late_refusal_r25/attempt.pdt1");
        let (request,_)=crate::pd_cycle::decode_cycle(bytes).unwrap();
        let request=crate::decode_cycle_request(&request).unwrap();
        let bytes=include_bytes!("../../shadow/src/test_fixtures/late_refusal_r25/proof.kie");
        let length=u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let proof=kie::decode_response(&bytes[8+length..]).unwrap().proof;
        (request,proof)
    }
    #[test]
    fn r29_original_authority_failure_survives_scientific_counter_in_iz() {
        // Captured request; relation variation tests the data contract only.
        let (request, mut proof) = r27_request_fixture();
        crate::pd_memory::failure::validate_missing_selection(
            &proof.historical_request, &proof.historical_response,
            crate::pd_memory::failure::MISSING_POSITIVE).unwrap();
        proof.relation = 1;
        proof.kis_mass = 0;
        proof.kic_mass = kie::PREDICATE_SCALE;
        let proof = proof.seal_data().unwrap();
        let witness = PdFailureWitness::from_request_current(&request, Some(&proof)).unwrap();
        let text = witness.negative_question().unwrap();
        assert!(text.contains("authority=ASHT route=ESS_MAI outcome=MISSING_EXACT_POSITIVE"),
            "the actual selection failure must survive the scientific result: {text}");
        assert!(text.contains("counter=CURRENT_KIC"));
        assert!(text.contains("ASHT_POSITIVE_EVIDENCE_MISSING"));
        assert!(text.contains(&crate::pd_memory::hex(&sha(&proof.historical_response))));
        witness.validate_for_request(&request).unwrap();
        witness.validate_model(Some(&proof)).unwrap();
        assert_eq!(PdFailureWitness::decode(&witness.encode().unwrap()).unwrap().negative_question().unwrap(), text);
    }

    #[test]
    fn r27_current_request_missing_positive_and_actual_counter_both_reach_question() {
        let (request,mut proof)=r27_request_fixture();
        let (_,measured)=request.quantum.final_evidence.candidate_measurement().unwrap().unwrap();
        assert_eq!(measured.positive_observed.count,0);
        // Synthetic classification variation: no claim of scientific execution.
        proof.relation=1;proof.kis_mass=0;proof.kic_mass=kie::PREDICATE_SCALE;
        let proof=proof.seal_data().unwrap();
        let witness=PdFailureWitness::from_request_current(&request,Some(&proof)).unwrap();
        let text=witness.negative_question().unwrap();
        assert!(text.contains("PRO_POSITIVE_MEASUREMENT_MISSING"),"the measured empty PRO side must survive the current counter: {text}");
        assert!(text.contains("counter=CURRENT_KIC"));
        assert!(text.contains("positive_count=0"));
        assert!(text.contains("negative_condition_count=4"));
        witness.validate_for_request(&request).unwrap();
        let encoded=witness.encode().unwrap();
        assert_eq!(PdFailureWitness::decode(&encoded).unwrap().negative_question().unwrap(),text);
    }
    #[test]
    fn r27_measured_zero_is_not_missing_but_denied_positive_family_is_distinct() {
        use crate::candidate_measurement::{CandidateLedger,CandidateObservation,CandidatePolarity};
        let (mut request,proof)=r27_request_fixture();
        let evidence=&mut request.quantum.final_evidence;
        let ledger=CandidateLedger::new(sha(&evidence.light_input_bytes),vec![CandidateObservation {
            polarity:CandidatePolarity::Positive,operator:1,fragment_refs:vec![0],value:0,
        }]).unwrap().into_current_matrix();
        evidence.quantum_action_evidence.iter_mut().find(|action|action.stage==crate::ACTION_STAGE_MPRO)
            .unwrap().evidence_words=ledger.to_words();
        let witness=PdFailureWitness::from_request_current(&request,Some(&proof)).unwrap();
        let text=witness.negative_question().unwrap();
        assert!(!text.contains("PRO_POSITIVE_MEASUREMENT_MISSING"));
        assert!(text.contains("PRO_POSITIVE_ADMISSION_MISSING"),"an observed zero and denied family are different obligations: {text}");
        assert!(text.contains("positive_count=1") && text.contains("positive_sum=0"));
    }

    #[test]
    fn r25_complete_enumeration_plus_scoped_record_respects_existing_128_bound() {
        let proof=full_scoped_proof();
        let witness=current_fixture(&proof,"OPERATOR_EVIDENCE_UNRESOLVED: exact source deficit");
        assert_eq!(witness.model.as_ref().unwrap().historical_negative_count,128);
        witness.validate_model(Some(&proof)).unwrap();
        let bytes=witness.encode().unwrap();
        assert_eq!(&bytes[8..10],&3u16.to_le_bytes());
        let restored=PdFailureWitness::decode(&bytes).unwrap();
        restored.validate_model(Some(&proof)).unwrap();
        assert_eq!(restored.negative_question().unwrap(),witness.negative_question().unwrap());
        assert!(restored.negative_question().unwrap().contains("candidate_id=128"));
        let mut legacy_v2=witness.clone();
        legacy_v2.inquiry=None;
        legacy_v2.lgc_tip=sha(&legacy_v2.body());
        let mut legacy_v1=legacy_v2.clone();
        legacy_v1.model.as_mut().unwrap().history_assessment_sha256=None;
        legacy_v1.lgc_tip=sha(&legacy_v1.body());
        for (version,valid) in [(1,legacy_v1),(2,legacy_v2),(3,witness)] {
            let bytes=valid.encode().unwrap();
            assert_eq!(&bytes[8..10],&(version as u16).to_le_bytes());
            assert_eq!(PdFailureWitness::decode(&bytes).unwrap().encode().unwrap(),bytes);
            for positive in [false,true] {
                let mut excess=valid.clone();
                let model=excess.model.as_mut().unwrap();
                if positive {model.historical_positive_count=129;} else {model.historical_negative_count=129;}
                excess.lgc_tip=sha(&excess.body());
                assert_eq!(excess.validate().unwrap_err(),"PD_WITNESS_MODEL_SHAPE");
            }
        }
    }
    #[test]
    fn r25_full_limit_cannot_claim_complete_enumeration_for_either_polarity() {
        for positive in [false,true] {
            let mut proof=crate::iz_frontier::test_proof(0);
            proof.history_assessment=None;
            let mut response=crate::decode_selection_response(&proof.historical_response).unwrap();
            let domain=if positive {crate::pd_memory::failure::relevance::TERM_PRIMITIVE_DOMAIN}
                else {crate::pd_memory::failure::relevance::TERM_NEGATIVE_DOMAIN};
            let values=(1..=128u8).map(|id|crate::CandidateStatisticWire {
                candidate_id:u64::from(id),polarity:u8::from(positive),
                domain:if positive {"primitive"}else{"negative"}.into(),
                term_sha256:crate::sha256_hex(&[domain,b"typed-model"]),source_sha256:crate::pd_memory::hex(&[id;32]),
                security_y:1,trust_x:1,confidence_fixed:3500,frequency:1,domain_count:1,legacy_score_fixed:3500,
            }).collect();
            if positive {response.positive=values;} else {response.negative=values;}
            response.selection_sha256=crate::pd_memory::failure::relevance::selection_digest(&response);
            proof.historical_response=crate::encode_selection_response(&response);
            let error=proof.seal_data().unwrap_err();
            assert!(error.contains(if positive {"cannot prove complete positive enumeration"}
                else {"cannot prove complete negative enumeration"}),"{error}");
        }
    }
    #[test]
    fn r25_final_question_bound_refuses_expansion_without_truncation() {
        let proof=full_scoped_proof();
        let reason=format!("OPERATOR_EVIDENCE_UNRESOLVED: {}",'\u{1}'.to_string().repeat(8000));
        assert!(reason.len()<=8192);
        let witness=current_fixture(&proof,&reason);
        let before=witness.clone();
        assert_eq!(witness.validate().unwrap_err(),"PD_WITNESS_QUESTION_LIMIT");
        assert_eq!(witness.encode().unwrap_err(),"PD_WITNESS_QUESTION_LIMIT");
        assert_eq!(witness.negative_question().unwrap_err(),"PD_WITNESS_QUESTION_LIMIT");
        assert_eq!(witness,before,"no data or source text is shortened to fit");
        assert_eq!(witness.refusal.as_deref(),Some(reason.as_str()));
        let legacy=fixture(1,&reason);
        let bytes=legacy.encode().unwrap();
        assert_eq!(PdFailureWitness::decode(&bytes).unwrap().encode().unwrap(),bytes);
        assert!(legacy.negative_question().unwrap().contains(&display_reason(&reason)),"legacy rendering remains exact");
    }
    #[test]
    fn r25_arithmetic_frontier_bound_covers_actual_escaped_rendering() {
        for proof in [crate::iz_frontier::test_proof(0),crate::iz_frontier::test_proof(1),
            crate::iz_frontier::test_proof(2),full_scoped_proof()] {
            for reason in ["OPERATOR_EVIDENCE_UNRESOLVED: source","SHADOW_MISSING_NEGATIVE_PROOF",
                "KIE_HISTORY_ORIGIN_UNRESOLVED: provenance","unstructured cause"] {
                let frontier=crate::iz_frontier::IzFrontier::from_observation(Some(reason),Some(&proof)).unwrap();
                assert!(frontier.render().unwrap().len()<=frontier.rendered_byte_bound().unwrap());
            }
        }
        let mut proof=crate::iz_frontier::test_proof(2);
        proof.canonical_proposition="x|\\\n\u{1}α".repeat(700);
        let history=kie::test_history(&proof.canonical_proposition);
        proof.historical_request=history.0;proof.historical_response=history.1;
        proof.history_assessment.as_mut().unwrap().proposition_sha256=sha(proof.canonical_proposition.as_bytes());
        let proof=proof.seal_data().unwrap();
        let frontier=crate::iz_frontier::IzFrontier::from_observation(None,Some(&proof)).unwrap();
        assert!(frontier.render().unwrap().len()<=frontier.rendered_byte_bound().unwrap());
    }
    #[test]
    fn r25_question_does_not_lose_missing_condition_when_counterproof_exists() {
        let proof=crate::iz_frontier::test_proof(1);
        let mut witness=fixture(1,"OPERATOR_EVIDENCE_UNRESOLVED: missing causal observation");
        witness.model=Some(KieModelAssessment::from_proof(&proof).unwrap());
        witness.inquiry=Some(crate::iz_frontier::IzFrontier::from_observation(witness.refusal.as_deref(),Some(&proof)).unwrap());
        witness.lgc_tip=sha(&witness.body());
        let question=witness.negative_question().unwrap();
        assert!(question.contains("counter=CURRENT_KIC"));
        assert!(question.contains("condition=OPERATOR_EVIDENCE_UNRESOLVED") && question.contains("prove shkakore ose dalluese"),
            "counterproof must not hide the separate causal-evidence question: {question}");
        let encoded=witness.encode().unwrap();
        assert_eq!(&encoded[8..10],&3u16.to_le_bytes());
        let restored=PdFailureWitness::decode(&encoded).unwrap();
        restored.validate_model(Some(&proof)).unwrap();
        assert_eq!(restored.negative_question().unwrap(),question);
        let mut altered=encoded;let last=altered.len()-1;altered[last]^=1;
        assert!(PdFailureWitness::decode(&altered).is_err());
    }
    #[test]
    fn r25_legacy_witness_questions_and_bytes_keep_their_version() {
        for relation in 0..=2 {
            let witness=fixture(relation,"OPERATOR_EVIDENCE_UNRESOLVED: original observation");
            let bytes=witness.encode().unwrap();
            assert_eq!(&bytes[8..10],&1u16.to_le_bytes());
            let decoded=PdFailureWitness::decode(&bytes).unwrap();
            assert!(decoded.inquiry.is_none());
            assert_eq!(decoded.encode().unwrap(),bytes);
            assert_eq!(decoded.negative_question().unwrap(),witness.negative_question().unwrap());
        }
    }
    #[test]
    fn witness_questions_keep_model_truth_process_failure_and_uncertainty_separate(){
        let supported=fixture(0,"pa material primitiv: Xi=∅ ∧ Yi=∅");
        let q=supported.negative_question().unwrap();assert!(q.contains("KiS mbeshtet"));assert!(q.contains("nuk eshte vete kunderprove"));
        let counter=fixture(1,"negative parity gate R1");assert!(counter.negative_question().unwrap().contains("testuar kundershtimin KiC"));
        let uncertain=fixture(2,"KiE unresolved");assert!(uncertain.negative_question().unwrap().contains("barazia nuk eshte kunderprove"));
        for witness in [supported,counter,uncertain]{let bytes=witness.encode().unwrap();assert_eq!(PdFailureWitness::decode(&bytes).unwrap(),witness);
            let mut bad=bytes;let n=bad.len();bad[n-1]^=1;assert!(PdFailureWitness::decode(&bad).is_err());}
    }
    #[test]
    fn refusal_display_preserves_exact_witness_and_survives_line_transport() {
        let original = "source|row\nnext\r\\n\tend\u{7f}";
        let expected = r"source\u{7c}row\nnext\r\\n\tend\u{7f}";
        assert_eq!(display_reason(original), expected);
        let witness = fixture(0, original);
        let before = witness.encode().unwrap();
        let question = witness.negative_question().unwrap();
        assert!(question.contains(expected));
        assert!(!question.chars().any(|ch| ch == '|' || ch.is_control()));
        assert_eq!(question.replace('|', "/").replace('\n', " ").replace('\r', " "), question);
        assert_eq!(witness.refusal.as_deref(), Some(original));
        assert_eq!(witness.encode().unwrap(), before);
        let decoded = PdFailureWitness::decode(&before).unwrap();
        assert_eq!(decoded.refusal.as_deref(), Some(original));
        assert_eq!(decoded.negative_question().unwrap(), question);
        // Literal escape-looking text must remain distinct from an actual delimiter.
        assert_ne!(display_reason(r"\u{7c}"), display_reason("|"));
        assert_ne!(display_reason(r"\n"), display_reason("\n"));
    }

    #[test]
    fn actual_source_deficits_get_distinct_questions_without_disproving_candidates() {
        let one_sided = fixture(0, "hapësirë e njëanshme: XiYi kërkon të dyja");
        let q = one_sided.negative_question().unwrap();
        assert!(q.contains("Ana primitive e pranishme ruhet"));
        assert!(q.contains("anes Xi ose Yi qe mungon"));
        assert!(q.contains("nuk verteton se pohimi eshte i gabuar"));
        let no_cooccurrence = fixture(0, "XiYi=∅: asnjë bashkëprezencë (x∈Xi, y∈Yi) në fragmente");
        let q = no_cooccurrence.negative_question().unwrap();
        assert!(q.contains("Kandidatet Xi/Yi ruhen"));
        assert!(q.contains("fragment me burim dokumenton bashkeprezencen"));
        assert!(q.contains("nuk eshte vete kunderprove"));
        let unresolved = fixture(0, "OPERATOR_EVIDENCE_UNRESOLVED: exact candidate preserved");
        assert!(unresolved.negative_question().unwrap().contains("Dobesia e operatorit nuk verteton"));
        for witness in [one_sided, no_cooccurrence, unresolved] {
            let q = witness.negative_question().unwrap();
            assert!(q.contains("KiS mbeshtet"));
            assert!(!q.contains("KiC kundershton"));
            assert!(q.contains("nuk parashikon i0"));
        }
    }

    #[test]
    fn changing_a_reason_or_model_cannot_keep_the_previous_trace(){
        let mut w=fixture(0,"pa material primitiv: Xi=∅ ∧ Yi=∅");w.refusal=Some("different cause".into());assert!(w.validate().is_err());
        let mut w=fixture(0,"cause");w.model.as_mut().unwrap().relation=1;assert!(w.validate().is_err());
    }

    #[test]
    fn r21_witness_records_classified_computation_without_issuing_authority() {
        let mut w=fixture(0,"OPERATOR_EVIDENCE_UNRESOLVED: retained observation");
        let legacy=w.encode().unwrap();
        assert_eq!(u16::from_le_bytes(legacy[8..10].try_into().unwrap()),1);
        assert_eq!(PdFailureWitness::decode(&legacy).unwrap().encode().unwrap(),legacy);
        let m=w.model.as_mut().unwrap();
        m.history_assessment_sha256=Some([7;32]);
        m.historical_negative_count=1;
        m.historical_negative_mass=0;
        m.historical_veto=false;
        w.lgc_tip=sha(&w.body());
        // Data shape alone cannot tell which origin/role justifies this result.
        // The actual proof comparison is mandatory at the authority boundary.
        w.validate().unwrap();
        assert!(w.validate_model(None).is_err());
        let encoded=w.encode().unwrap();
        assert_eq!(u16::from_le_bytes(encoded[8..10].try_into().unwrap()),2);
        assert_eq!(PdFailureWitness::decode(&encoded).unwrap(),w);
        assert!(w.negative_question().unwrap().contains("Masat historike nuk jane pesha te KiS/KiC"));
        let mut changed=w.clone();
        changed.model.as_mut().unwrap().history_assessment_sha256=Some([8;32]);
        assert!(changed.validate().is_err());
        changed.lgc_tip=sha(&changed.body());
        assert!(changed.validate_model(None).is_err());
        let mut zero=w;
        zero.model.as_mut().unwrap().history_assessment_sha256=Some([0;32]);
        zero.lgc_tip=sha(&zero.body());
        assert!(zero.validate().is_err());
        for version in [0_u16,3,u16::MAX] {
            let mut malformed=encoded.clone();
            malformed[8..10].copy_from_slice(&version.to_le_bytes());
            assert!(PdFailureWitness::decode(&malformed).is_err());
        }
    }

    #[test]
    fn r21_rehashed_witness_must_match_the_separately_computed_assessment() {
        // Data-only fixture. Shadow's model and durable-origin verification
        // are separate mandatory checks; this test exercises the witness link.
        let identity=crate::pd_memory::PdIdentity {
            owner:"owner".into(),obligation:"model".into(),case_id:"case".into(),
            scope:"scope".into(),domain:"science".into(),lineage_root:"root".into(),
        };
        let (historical_request,historical_response)=kie::test_history("typed-model");
        let proof=kie::KieProof {
            context:kie::KieContext {identity:identity.clone(),operation_id:"op".into(),record_id:[3;32]},
            query_bytes:vec![1],candidate_bytes:vec![2],canonical_proposition:"typed-model".into(),
            historical_request,historical_response,kis_mass:10_000,kic_mass:0,relation:0,
            proof_sha256:[0;32],prior_adjudication:None,
            history_assessment:Some(kie::KieHistoryAssessment {
                scope_sha256:identity.key().unwrap(),proposition_sha256:sha(b"typed-model"),entries:vec![],scoped_evidence:vec![],
            }),
        }.seal_data().unwrap();
        let mut w=fixture(0,"condition failed after model computation");
        w.model=Some(KieModelAssessment::from_proof(&proof).unwrap());
        w.lgc_tip=sha(&w.body());
        w.validate_model(Some(&proof)).unwrap();
        assert_eq!(PdFailureWitness::decode(&w.encode().unwrap()).unwrap(),w);
        for mutation in 0..3 {
            let mut changed=w.clone();
            let model=changed.model.as_mut().unwrap();
            match mutation {
                0=>model.history_assessment_sha256=Some([9;32]),
                1=>model.historical_veto=true,
                _=>model.unresolved_history_count=1,
            }
            changed.lgc_tip=sha(&changed.body());
            changed.validate().unwrap();
            assert_eq!(changed.validate_model(Some(&proof)).unwrap_err(),"PD_WITNESS_ACTUAL_MODEL");
        }
        let mut unknown=w;
        unknown.model.as_mut().unwrap().unresolved_history_count=1;
        unknown.lgc_tip=sha(&unknown.body());
        let question=unknown.negative_question().unwrap();
        assert!(question.contains("prejardhje te pazgjidhur"));
        assert!(!question.contains("Cila matje, saktesi"));
    }
}
