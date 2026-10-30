//! Recoverable V5 node obligations in the existing durable LGC description.
//! Data only: a trace never grants a write capability.
use crate::{knowledge_lineage::Lineage, types::{ShadowNode, ShadowPassage}};
use sha2::{Digest, Sha256};
pub(crate) const MARKER: &str = "|MODEL_JUDGMENT_V5=";

fn digest(text: &str) -> u64 {
    let h = Sha256::digest(text.as_bytes());
    u64::from_le_bytes(h[..8].try_into().expect("eight bytes"))
}
pub(crate) fn attach(passage: &ShadowPassage, lineage: &mut Lineage, dsl: &mut String) {
    if let Some(binding) = passage.model_case_binding {
        let hex: String = binding.iter().map(|v| format!("{v:02x}")).collect();
        let nodes = passage.verdicts.iter().map(|v|
            format!("{},{},{},{:08x}", v.node as u8, v.verdict, v.reason_code, v.score.to_bits()))
            .collect::<Vec<_>>().join(";");
        let record = format!("{hex}:{:016x}:{:016x}:{}:{:016x}:{nodes}",
            passage.package.final_evidence_digest, passage.input_id, passage.path.as_u8(), lineage.derivation);
        lineage.derivation = digest(&record);
        dsl.push_str("|MODEL_OUTPUT_V1"); dsl.push_str(MARKER); dsl.push_str(&record);
    }
}
pub(crate) fn check_binding(dsl: &str, binding: Option<([u8;32],u64)>) -> Result<(), String> {
    match (dsl.split_once(MARKER), binding) {
        (None, None) => Ok(()),
        (Some((_, record)), Some((case, evidence))) => {
            let hex: String = case.iter().map(|v|format!("{v:02x}")).collect();
            if case == [0;32] || evidence == 0 || !record.starts_with(&format!("{hex}:{evidence:016x}:")) {
                return Err("MODEL_TRACE_CASE_EVIDENCE_BINDING".into());
            }
            Ok(())
        },
        _ => Err("MODEL_TRACE_POLICY_PRESENCE".into()),
    }
}
pub(crate) fn validate(dsl: &str, lineage: &Lineage, expected_y: u8,
    binding: Option<([u8;32],u64)>) -> Result<(), String> {
    check_binding(dsl, binding)?;
    let Some((_, record)) = dsl.split_once(MARKER) else { return Ok(()); };
    if record.contains(MARKER) || digest(record) != lineage.derivation {
        return Err("MODEL_TRACE_DIGEST".into());
    }
    let parts: Vec<_> = record.split(':').collect();
    if parts.len() != 6 || parts[0].len()!=64
        || !parts[0].bytes().all(|v| v.is_ascii_hexdigit())
        || parts[0].bytes().all(|v| v==b'0') {
        return Err("MODEL_TRACE_FORMAT".into());
    }
    let number = |s: &str| u64::from_str_radix(s, 16).map_err(|_| "MODEL_TRACE_NUMBER".to_string());
    if number(parts[1])? == 0 || number(parts[2])? != lineage.input_id || number(parts[4])? == 0 {
        return Err("MODEL_TRACE_BINDING".into());
    }
    let path = parts[3].parse::<u8>().map_err(|_| "MODEL_TRACE_PATH")?;
    if path > 2 { return Err("MODEL_TRACE_PATH".into()); }
    const NODES: [ShadowNode; 11] = [ShadowNode::Router, ShadowNode::Matrix, ShadowNode::Gen5,
        ShadowNode::Type, ShadowNode::Temporal, ShadowNode::Sovereign, ShadowNode::Emergence,
        ShadowNode::Consensus, ShadowNode::NightWatch, ShadowNode::Judiciary, ShadowNode::MultiVerification];
    let mut p = ShadowPassage::new(lineage.input_id, Default::default(), Default::default(),
        crate::types::ShadowPath::from_u8(path));
    for row in parts[5].split(';') {
        let a: Vec<_> = row.split(',').collect();
        if a.len()!=4 { return Err("MODEL_TRACE_NODE".into()); }
        let id = a[0].parse::<usize>().map_err(|_| "MODEL_TRACE_NODE")?;
        let node = *NODES.get(id).ok_or("MODEL_TRACE_NODE")?;
        let bit = a[1].parse::<u8>().map_err(|_| "MODEL_TRACE_BIT")?;
        let reason = a[2].parse::<u8>().map_err(|_| "MODEL_TRACE_REASON")?;
        let score = f32::from_bits(u32::from_str_radix(a[3],16).map_err(|_| "MODEL_TRACE_SCORE")?);
        if bit>1 || !score.is_finite() || !(0.0..=1.0).contains(&score) {
            return Err("MODEL_TRACE_RANGE".into());
        }
        if !matches!(node, ShadowNode::MultiVerification | ShadowNode::NightWatch)
            && (reason & 0x80 == 0 || bit != u8::from(reason & 0x7f == 0)) {
            return Err("MODEL_TRACE_PREDICATES".into());
        }
        p.push_verdict(crate::types::ShadowVerdict{node,verdict:bit,reason_code:reason,score,ts_ns:0});
    }
    let expected = crate::sovereign_guard::expected_path(&p);
    if !p.verdicts.iter().map(|v| v.node).eq(expected.iter().copied())
        || p.verdicts.len()!=usize::from(lineage.chain_count)
        || p.judiciary_verdict()!=expected_y
        || (expected_y==1 && !p.verdicts.iter().all(|v|v.verdict==1))
        || p.verdicts.iter().fold(0u16, |m,v| m | (u16::from(v.verdict==1) << v.node as u16))!=lineage.chain_mask {
        return Err("MODEL_TRACE_SEQUENCE_OR_VERDICT".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn r33_trace_recovers_predicates_and_detects_changed_mask_or_policy() {
        use crate::types::*;
        let mut p=ShadowPassage::new(19,PassPackage {final_evidence_digest:7,..Default::default()},
            LightEnvelope::sealed("x","model",vec![1,2,3],vec![]),ShadowPath::Fast);
        p.model_case_binding=Some([4;32]);
        for node in [ShadowNode::Router,ShadowNode::Matrix,ShadowNode::MultiVerification,ShadowNode::Judiciary] {
            p.push_verdict(ShadowVerdict {node,score:0.1,verdict:1,reason_code:0x80,ts_ns:0});
        }
        let mut lineage=Lineage::derive_chain(19,&p.verdicts,&p.light.proof_chain);
        let old=lineage;let mut text=String::from("LGC");
        attach(&p,&mut lineage,&mut text);
        assert_ne!(old.derivation,lineage.derivation);
        assert!(validate("LGC",&lineage,1,Some(([4;32],7))).is_err());
        assert!(validate(&text,&lineage,1,None).is_err());
        assert!(validate(&text,&lineage,1,Some(([4;32],8))).is_err());
        assert!(validate(&text,&lineage,1,Some(([5;32],7))).is_err());
        // Rehashing an internally changed record does not repair semantic masks.
        let altered=text.replace("0,1,128", "0,1,129");
        let mut coherent=lineage;
        coherent.derivation=digest(altered.split_once(MARKER).unwrap().1);
        assert!(validate(&altered,&coherent,1,Some(([4;32],7))).is_err());
        validate(&text,&lineage,1,Some(([4;32],7))).unwrap();
        assert!(text.contains("0,1,128,3dcccccd"));
        assert!(validate(&text.replace("0,1,128","0,1,129"),&lineage,1,Some(([4;32],7))).is_err());
        assert!(validate(&text,&lineage,0,Some(([4;32],7))).is_err());
    }
}
