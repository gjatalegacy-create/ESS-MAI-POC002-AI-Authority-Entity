use crate::lab_contracts::{NegativePath, PrimitiveSplit};
use crate::shadow_commit::KnowledgeWrite;
use crate::types::{NegativeContext, ShadowError};
use crate::vault_transaction::sha256_parts;
use std::collections::HashSet;

const REASON_COLLAPSE_ELIM: u32 = 0xA451;

/// Identifikues teknik legacy për claim-in; nuk përdoret si provë integriteti.
fn technical_claim_id(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn semantics(
    split: Option<&PrimitiveSplit>,
    target: &NegativePath,
    xy_x: Option<&str>,
    xy_y: Option<&str>,
    unique: bool,
) -> bool {
    let anchored = match split {
        Some(material) => match target.space {
            "Xi" => material.xi.iter().any(|term| term == &target.term),
            "Yi" => material.yi.iter().any(|term| term == &target.term),
            _ => false,
        },
        None => false,
    };
    anchored
        & (xy_x != Some(target.term.as_str()))
        & (xy_y != Some(target.term.as_str()))
        & unique
}

#[derive(Debug, Clone)]
pub struct PendingNegativeBatch {
    writes: Vec<KnowledgeWrite>,
    batch_digest: [u8; 32],
    pub count: usize,
}

impl PendingNegativeBatch {
    pub fn stage(
        wires: &[String],
        split: Option<&PrimitiveSplit>,
        xy_x: Option<&str>,
        xy_y: Option<&str>,
        mass: f32,
        ts_ns: u64,
    ) -> Result<Self, ShadowError> {
        Self::stage_classified(wires,split,xy_x,xy_y,mass,ts_ns,false,false)
    }

    /// KIP3 records independently checked model operator observations without
    /// turning lexical weakness into a semantic counterexample. The strict
    /// model gate may discharge only selected-term exclusion, never anchoring,
    /// uniqueness, the courts, or the final Shadow decision.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn stage_kie(
        wires:&[String], split:Option<&PrimitiveSplit>, xy_x:Option<&str>, xy_y:Option<&str>,
        mass:f32, ts_ns:u64, proof:&shadow_contracts::kie::KieProof,
        parent:&crate::negative_asset::ValidatedNegativeAsset,
    )->Result<Self,ShadowError> {
        crate::kie_selection::validate_proof_model(proof).map_err(ShadowError::SealInvalid)?;
        let envelope=shadow_contracts::negative_asset::NegativeAssetEnvelope::decode(&parent.wire.bytes)
            .map_err(|_|ShadowError::SealInvalid("KIE_OBSERVATION_PARENT".into()))?;
        let actual_token=sha256_parts(&[shadow_contracts::negative_asset::NEGATIVE_ASSET_TOKEN_DOMAIN,&envelope.canonical_body()]);
        match proof.history_assessment.is_some() && parent.project_id==0 && !parent.experimental_failure
            && envelope.shape_is_valid(shadow_contracts::MASS_SCALE)
            && envelope.token_sha256==actual_token && parent.token_sha256==actual_token
            && envelope.input_sha256==shadow_contracts::kie::digest(proof.canonical_proposition.as_bytes())
            && !envelope.experimental_failure && envelope.project_id==0
            && envelope.targets.len()==wires.len()
            && envelope.targets.iter().map(|t|t.path_wire()).eq(wires.iter().cloned()) {
            true=>{}, false=>return Err(ShadowError::SealInvalid("KIE_OBSERVATION_ACTUAL_MODEL_BINDING".into())),
        }
        let permit_selected=proof.relation==0 && shadow_contracts::kie::combined_evaluation(proof)
            .map_err(ShadowError::SealInvalid)?.permits_positive;
        Self::stage_classified(wires,split,xy_x,xy_y,mass,ts_ns,true,permit_selected)
    }

    #[allow(clippy::too_many_arguments)]
    fn stage_classified(wires:&[String],split:Option<&PrimitiveSplit>,xy_x:Option<&str>,xy_y:Option<&str>,
        mass:f32,ts_ns:u64,observations:bool,permit_selected:bool)->Result<Self,ShadowError> {
        let mut targets = wires
            .iter()
            .map(|wire| {
                NegativePath::from_wire(wire).ok_or_else(|| {
                    ShadowError::TransportCorrupt(
                        "negative wire i keqformuar".to_string(),
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut seen = HashSet::new();
        for target in &targets {
            let key = (target.space, target.term.as_str());
            let unique = seen.insert(key);
            let checked_xy=match permit_selected {true=>(None,None),false=>(xy_x,xy_y)};
            match semantics(split, target, checked_xy.0, checked_xy.1, unique) {
                true => {}
                false => {
                    return Err(ShadowError::SealInvalid(format!(
                        "negative target '{}' në {} nuk është anchored/unik/excluded",
                        target.term, target.space,
                    )))
                }
            }
        }
        targets.sort_by(|left, right| {
            (left.space, left.term.as_str(), left.reason.as_str()).cmp(&(
                right.space,
                right.term.as_str(),
                right.reason.as_str(),
            ))
        });
        let mut canonical = Vec::new();
        let writes = targets
            .into_iter()
            .map(|target| {
                canonical.extend_from_slice(&(target.space.len() as u64).to_le_bytes());
                canonical.extend_from_slice(target.space.as_bytes());
                canonical.extend_from_slice(&(target.term.len() as u64).to_le_bytes());
                canonical.extend_from_slice(target.term.as_bytes());
                canonical.extend_from_slice(&(target.reason.len() as u64).to_le_bytes());
                canonical.extend_from_slice(target.reason.as_bytes());
                let claim_id = technical_claim_id(target.term.as_bytes());
                let process = format!(
                    "KOLAPS/2:-{}(term={};{})",
                    target.space, target.term, target.reason,
                );
                KnowledgeWrite::Negative {
                    input_id: claim_id,
                    raw_bytes: target.term.into_bytes(),
                    context: NegativeContext {
                        failure_reason_code: match observations {
                            true=>shadow_contracts::negative_asset::PROCESS_OBSERVATION_REASON,
                            false=>REASON_COLLAPSE_ELIM,
                        },
                        process,
                        mass_at_fail: mass,
                        frequency: 1,
                    },
                    ts_ns,
                }
            })
            .collect::<Vec<_>>();
        let batch_digest = sha256_parts(&[b"ESSMAI_PENDING_NEGATIVE_V1", &canonical]);
        let count = writes.len();
        Ok(Self {
            writes,
            batch_digest,
            count,
        })
    }

    pub const fn digest(&self) -> [u8; 32] {
        self.batch_digest
    }

    pub fn into_writes(self) -> Vec<KnowledgeWrite> {
        self.writes
    }

    pub fn is_complete_for(&self, expected: usize) -> bool {
        self.count == expected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split() -> PrimitiveSplit {
        PrimitiveSplit {
            xi: vec!["x1".to_string(), "x2".to_string()],
            yi: vec!["y1".to_string(), "y2".to_string()],
        }
    }


    #[test]
    fn r15_delimited_negative_term_is_preserved_only_with_existing_semantic_guards() {
        let exact = "jd_part2=54388.0,|provë~e\u{308}videncë";
        let reason = "Reverse^-1:kusht,|arsye~e pandryshuar";
        let material = PrimitiveSplit {
            xi: vec!["selected-x".into(), exact.into()],
            yi: vec!["selected-y".into()],
        };
        let target = NegativePath { term: exact.into(), space: "Xi", reason: reason.into() };
        let wire = target.to_wire();
        let batch = PendingNegativeBatch::stage(&[wire.clone()], Some(&material),
            Some("selected-x"), Some("selected-y"), 0.4, 7).expect("anchored excluded target");
        assert!(batch.is_complete_for(1));
        match &batch.writes[0] {
            KnowledgeWrite::Negative { input_id, raw_bytes, context, ts_ns } => {
                assert_eq!(raw_bytes.as_slice(), exact.as_bytes());
                assert_eq!(*input_id, technical_claim_id(exact.as_bytes()));
                assert_eq!(context.process, format!("KOLAPS/2:-Xi(term={exact};{reason})"));
                assert_eq!(context.failure_reason_code, REASON_COLLAPSE_ELIM);
                assert_eq!(context.mass_at_fail, 0.4);
                assert_eq!(context.frequency, 1);
                assert_eq!(*ts_ns, 7);
            }
            KnowledgeWrite::Primitive { .. } => panic!("transport must not promote a negative to positive"),
        }

        assert!(matches!(PendingNegativeBatch::stage(&[wire.clone()], None,
            Some("selected-x"), Some("selected-y"), 0.4, 7), Err(ShadowError::SealInvalid(_))));
        assert!(matches!(PendingNegativeBatch::stage(&[wire.clone()], Some(&material),
            Some(exact), Some("selected-y"), 0.4, 7), Err(ShadowError::SealInvalid(_))));
        assert!(matches!(PendingNegativeBatch::stage(&[wire.clone(), wire], Some(&material),
            Some("selected-x"), Some("selected-y"), 0.4, 7), Err(ShadowError::SealInvalid(_))));

        let changed_term = NegativePath { term: exact.replace(',', ";"), ..target.clone() };
        assert!(matches!(PendingNegativeBatch::stage(&[changed_term.to_wire()], Some(&material),
            Some("selected-x"), Some("selected-y"), 0.4, 7), Err(ShadowError::SealInvalid(_))));
        let changed_space = NegativePath { space: "Yi", ..target };
        assert!(matches!(PendingNegativeBatch::stage(&[changed_space.to_wire()], Some(&material),
            Some("selected-x"), Some("selected-y"), 0.4, 7), Err(ShadowError::SealInvalid(_))));
    }
    #[test]
    fn invalid_second_target_produces_no_batch() {
        let wires = vec![
            "x2~Xi~weak".to_string(),
            "outside~Yi~weak".to_string(),
        ];
        assert!(PendingNegativeBatch::stage(
            &wires,
            Some(&split()),
            Some("x1"),
            Some("y1"),
            0.4,
            7,
        )
        .is_err());
    }

    #[test]
    fn duplicate_target_is_rejected() {
        let wires = vec!["x2~Xi~weak".to_string(), "x2~Xi~weak".to_string()];
        assert!(PendingNegativeBatch::stage(
            &wires,
            Some(&split()),
            Some("x1"),
            Some("y1"),
            0.4,
            7,
        )
        .is_err());
    }

    #[test]
    fn batch_digest_is_order_independent_and_complete() {
        let first = vec!["x2~Xi~weak".to_string(), "y2~Yi~weak".to_string()];
        let second = vec!["y2~Yi~weak".to_string(), "x2~Xi~weak".to_string()];
        let a = PendingNegativeBatch::stage(
            &first,
            Some(&split()),
            Some("x1"),
            Some("y1"),
            0.4,
            7,
        )
        .expect("batch a");
        let b = PendingNegativeBatch::stage(
            &second,
            Some(&split()),
            Some("x1"),
            Some("y1"),
            0.4,
            7,
        )
        .expect("batch b");
        assert_eq!(a.digest(), b.digest());
        assert!(a.is_complete_for(2));
        assert_ne!(a.digest(), [0u8; 32]);
    }
}
