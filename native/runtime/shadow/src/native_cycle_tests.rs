//! Snapshot/replay fixtures, NOT a real constitutional judgement.
//! Fixtures are copied from shadow-contracts/src/lib.rs test helpers; only the
//! fixture PA parent is normalized to i0-0000000000000029 for the native wire.
//! SOURCE_SHA256: 69e361903774c2711af50f575bdee10443fcb95f52fda38115c2557881bb559a
//! Actual tests use production TX codecs, WAL sync/reopen and native replay API.
use super::*;
use crate::knowledge_vault::{KnowledgeVault, VaultBackend};
use crate::vault_transaction::{CommitStatus, VaultTransaction};
use shadow_contracts::*;
use std::path::PathBuf;

fn expected_action_word_count(stage: u8) -> Option<usize> {
    match stage {
        ACTION_STAGE_HPRO => Some(5),
        ACTION_STAGE_PRO => Some(ProActionEvidence::WORD_COUNT),
        ACTION_STAGE_NPRO => Some(4),
        ACTION_STAGE_SRK => Some(6),
        ACTION_STAGE_APRO => Some(4),
        ACTION_STAGE_MPRO => Some(21),
        ACTION_STAGE_PIM => Some(6),
        ACTION_STAGE_NPIM => Some(5),
        ACTION_STAGE_HCP => Some(7),
        _ => None,
    }
}

fn converge_action_words(words: &[u64]) -> u64 {
    words
        .iter()
        .enumerate()
        .fold(0x6A09_E667_F3BC_C909u64, |state, (index, word)| {
            state.rotate_left(((index as u32) % 47) + 5)
                ^ word
                    .rotate_left(((index as u32) % 31) + 1)
                    .wrapping_add(0x3C6E_F372_FE94_F82B ^ index as u64)
        })
}

fn action_stage_word(stage: u8) -> u64 {
    (u64::from(stage) + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

fn replay_action_evidence(evidence: &[ActionEvidenceWire]) -> (u64, u64, bool) {
    let mut state = 0u64;
    let mut mask = 0u64;
    let mut unique = true;
    for entry in evidence {
        let bit = match entry.stage < 64 {
            true => 1u64 << u32::from(entry.stage),
            false => {
                unique = false;
                0
            }
        };
        match mask & bit {
            0 => {}
            _ => unique = false,
        }
        let contribution =
            converge_action_words(&entry.evidence_words) ^ action_stage_word(entry.stage);
        match expected_action_word_count(entry.stage) == Some(entry.evidence_words.len())
            && (entry.stage != ACTION_STAGE_PRO
                || ProActionEvidence::try_from_words(&entry.evidence_words).is_ok())
        {
            true => {}
            false => unique = false,
        }
        state = state
            .rotate_left(11)
            .wrapping_add(contribution)
            .rotate_left(7);
        mask |= bit;
    }
    (state, mask, unique)
}

fn layer(
    layer_mask: u8,
    activation_id: u64,
    activation_contract_digest: u64,
    gcl_process_digest: u64,
    parent_result_digest: u64,
    result_material: Vec<u8>,
) -> PdLayerEvidenceWire {
    let result_material_digest = digest_parts(&[result_material.as_slice()]);
    let mut layer = PdLayerEvidenceWire {
        layer_mask,
        activation_id,
        activation_contract_digest,
        gcl_process_digest,
        parent_result_digest,
        result_material,
        result_material_digest,
        result_digest: 0,
        completed: true,
    };
    layer.result_digest = layer.recompute_digest();
    layer
}

fn pd_spine() -> PdSpineEvidenceWire {
    let contract_version = PdSpineEvidenceWire::CONTRACT_VERSION;
    let session_digest = digest_parts(&[b"s"]);
    let parent_i0_digest = digest_parts(&[b"i0-0000000000000029"]);
    let trace_digest = digest_parts(&[b"trace"]);
    let turn_digest = digest_parts(&[b"turn"]);
    let created_at = 10u64;

    let initial_i_digest = digest_parts(&[b"s", b"GCL_INITIAL_I"]);
    let user_input_digest = digest_parts(&[b"input"]);
    let stimulus_digest = digest_parts(&[
        &PdContinuumEvidenceWire::CONTRACT_VERSION.to_le_bytes(),
        &session_digest.to_le_bytes(),
        &initial_i_digest.to_le_bytes(),
        &user_input_digest.to_le_bytes(),
        &parent_i0_digest.to_le_bytes(),
        &created_at.to_le_bytes(),
        b"I_PLUS_U_TO_I0",
    ]);
    let question_digest = digest_parts(&[b"question"]);
    let response_kind_digest = digest_parts(&[b"CLARIFY"]);
    let increment_digest = digest_parts(&[
        &PdContinuumEvidenceWire::CONTRACT_VERSION.to_le_bytes(),
        &stimulus_digest.to_le_bytes(),
        &PdContinuumEvidenceWire::QUESTION_INCREMENT_UNITS.to_le_bytes(),
        &question_digest.to_le_bytes(),
        &response_kind_digest.to_le_bytes(),
        &created_at.to_le_bytes(),
        b"I0_PLUS_ONE_QUESTION_IQ",
    ]);
    let continuum_activation_digest = digest_parts(&[
        &PdContinuumEvidenceWire::CONTRACT_VERSION.to_le_bytes(),
        &stimulus_digest.to_le_bytes(),
        &increment_digest.to_le_bytes(),
        b"PD_CONTINUUM_ACTIVATION",
    ]);
    let continuum = PdContinuumEvidenceWire {
        contract_version: PdContinuumEvidenceWire::CONTRACT_VERSION,
        session_digest,
        initial_i_digest,
        user_input_digest,
        i0_digest: parent_i0_digest,
        stimulus_digest,
        stimulus_created_at: created_at,
        stimulus_state: 0,
        unit_count: PdContinuumEvidenceWire::QUESTION_INCREMENT_UNITS,
        question_digest,
        response_kind_digest,
        increment_digest,
        increment_created_at: created_at,
        increment_state: 1,
        activation_digest: continuum_activation_digest,
        activation_state: 2,
    };

    let mut activation = PdActivationEvidenceWire {
        mode_code: 0,
        origin_code: 0,
        response_kind_digest,
        genius_signal_digest: 0,
        trace_mass: 0,
        structural_coherence: 0,
        required_layer_mask: PdSpineEvidenceWire::ALL_LAYERS_MASK,
        contract_digest: 0,
    };
    activation.contract_digest = activation.recompute_contract_digest().unwrap();
    let activation_contract_digest = activation.contract_digest;

    let gcl_law_seal = digest_parts(&[b"gjata_collapse_law"]);
    let gcl_system_laws_seal = digest_parts(&[b"ESS_MAI_SYSTEM_LAWS"]);
    let gcl_phase_digest = digest_parts(&[b"KOLAPS_ARSYETIMI"]);
    let gcl_process_digest = digest_parts(&[
        &contract_version.to_le_bytes(),
        &gcl_law_seal.to_le_bytes(),
        &gcl_system_laws_seal.to_le_bytes(),
        &gcl_phase_digest.to_le_bytes(),
        &session_digest.to_le_bytes(),
        &parent_i0_digest.to_le_bytes(),
        &continuum_activation_digest.to_le_bytes(),
        &activation_contract_digest.to_le_bytes(),
        &created_at.to_le_bytes(),
        b"GCL_PD_SPINE_PROCESS_V160",
    ]);
    let activation_id = digest_parts(&[
        &contract_version.to_le_bytes(),
        &session_digest.to_le_bytes(),
        &parent_i0_digest.to_le_bytes(),
        &trace_digest.to_le_bytes(),
        &turn_digest.to_le_bytes(),
        &continuum_activation_digest.to_le_bytes(),
        &activation_contract_digest.to_le_bytes(),
        &gcl_process_digest.to_le_bytes(),
        &created_at.to_le_bytes(),
        b"PD_SPINE9_ACTIVATION_V160",
    ]);
    let layer1 = layer(
        0b001,
        activation_id,
        activation_contract_digest,
        gcl_process_digest,
        0,
        11u64.to_le_bytes().to_vec(),
    );
    let layer2 = layer(
        0b010,
        activation_id,
        activation_contract_digest,
        gcl_process_digest,
        layer1.result_digest,
        12u64.to_le_bytes().to_vec(),
    );
    let layer3 = layer(
        0b100,
        activation_id,
        activation_contract_digest,
        gcl_process_digest,
        layer2.result_digest,
        13u64.to_le_bytes().to_vec(),
    );
    let mut spine = PdSpineEvidenceWire {
        contract_version,
        activation_id,
        session_digest,
        parent_i0_digest,
        trace_digest,
        turn_digest,
        created_at,
        continuum,
        activation,
        continuum_activation_digest,
        activation_contract_digest,
        gcl_law_seal,
        gcl_system_laws_seal,
        gcl_phase_digest,
        gcl_process_digest,
        layer_mask: PdSpineEvidenceWire::ALL_LAYERS_MASK,
        layer1,
        layer2,
        layer3,
        evidence_digest: 14,
        mpro_mass: 5000,
        ready_for_shadow: true,
        completed_at: 15,
        completion_digest: 0,
    };
    spine.completion_digest = spine.recompute_completion_digest();
    spine
}

fn negative() -> NegativeKnowledgeWire {
    NegativeKnowledgeWire {
        input_id: 7,
        reason: 9,
        process: "npim".into(),
        mass: 0.4,
        frequency: 2,
        suggestion_code: 1,
        bytes: vec![1, 2, 3],
        ts_ns: 11,
    }
}

fn muscle_evidence() -> MuscleEvidenceWire {
    let mut evidence = MuscleEvidenceWire {
        besa_hold_id: "besa-hold".into(),
        besa_hold_sha256: "11".repeat(32),
        besa_selection_sha256: "22".repeat(32),
        besa_attestation_sha256: "33".repeat(32),
        asht_hold_id: "asht-hold".into(),
        asht_hold_sha256: "44".repeat(32),
        asht_selection_sha256: "55".repeat(32),
        asht_attestation_sha256: "66".repeat(32),
        split_sha256: "77".repeat(32),
        combined_sha256: String::new(),
    };
    evidence.combined_sha256 = evidence.recompute_combined_sha256();
    evidence
}

fn cycle_fixture() -> ShadowCycleRequest {
    ShadowCycleRequest {
        quantum: QuantumInboundWire {
            package_id: "p".into(),
            session_id: "s".into(),
            territory: "t".into(),
            epistemic_mass: 0.5,
            evidence_density: 0.6,
            logical_coherence: 0.7,
            causal_integrity: 0.8,
            convergence_strength: 0.9,
            reproducibility: 1.0,
            potentiality_score: 0.4,
            ibe_score: 0.3,
            chain_coherence: 0.2,
            conservation_ok: true,
            candidate_scores: vec![0.9, 0.1],
            suggested_verdict: 1,
            primitive_flags: 0xA451,
            raw_bytes: vec![4, 5],
            xy_x: Some("x".into()),
            xy_y: Some("y".into()),
            xy_mass: Some(500),
            pd_binding_digest: Some(12),
            pd_continuum_activation_digest: Some(13),
            pd_parent_i0: Some("i0-0000000000000029".into()),
            collapse_refused: None,
            negative_paths: vec!["n".into()],
            hcp_input_id: Some(1),
            hcp_token_gen: Some(2),
            hcp_token_nonce: Some(3),
            hcp_directive_bit: Some(1),
            final_evidence: {
                let mut e = FinalEvidenceWire {
                    pd_spine: pd_spine(),
                    muscle_evidence: muscle_evidence(),
                    quantum_action_state: {
                        let evidence = vec![
                            ActionEvidenceWire {
                                stage: 1,
                                evidence_words: vec![11; 5],
                            },
                            ActionEvidenceWire {
                                stage: 2,
                                evidence_words: vec![2, 9000, 1000, 2, 1, 40],
                            },
                            ActionEvidenceWire {
                                stage: 3,
                                evidence_words: vec![33; 4],
                            },
                            ActionEvidenceWire {
                                stage: 8,
                                evidence_words: vec![88; 5],
                            },
                            ActionEvidenceWire {
                                stage: 4,
                                evidence_words: vec![44; 6],
                            },
                            ActionEvidenceWire {
                                stage: 7,
                                evidence_words: vec![77; 6],
                            },
                            ActionEvidenceWire {
                                stage: 5,
                                evidence_words: vec![55; 4],
                            },
                            ActionEvidenceWire {
                                stage: 6,
                                evidence_words: vec![66; 21],
                            },
                            ActionEvidenceWire {
                                stage: 9,
                                evidence_words: vec![99; 7],
                            },
                        ];
                        replay_action_evidence(&evidence).0
                    },
                    quantum_action_evidence: vec![
                        ActionEvidenceWire {
                            stage: 1,
                            evidence_words: vec![11; 5],
                        },
                        ActionEvidenceWire {
                            stage: 2,
                            evidence_words: vec![2, 9000, 1000, 2, 1, 40],
                        },
                        ActionEvidenceWire {
                            stage: 3,
                            evidence_words: vec![33; 4],
                        },
                        ActionEvidenceWire {
                            stage: 8,
                            evidence_words: vec![88; 5],
                        },
                        ActionEvidenceWire {
                            stage: 4,
                            evidence_words: vec![44; 6],
                        },
                        ActionEvidenceWire {
                            stage: 7,
                            evidence_words: vec![77; 6],
                        },
                        ActionEvidenceWire {
                            stage: 5,
                            evidence_words: vec![55; 4],
                        },
                        ActionEvidenceWire {
                            stage: 6,
                            evidence_words: vec![66; 21],
                        },
                        ActionEvidenceWire {
                            stage: 9,
                            evidence_words: vec![99; 7],
                        },
                    ],
                    quantum_action_mask: REQUIRED_ACTION_MASK,
                    quantum_required_action_mask: REQUIRED_ACTION_MASK,
                    light_input_sha256: "00".repeat(32),
                    light_input_bytes: b"input".to_vec(),
                    pim_evidence_density: 6000,
                    pim_logical_coherence: 7000,
                    pim_causal_integrity: 8000,
                    pim_convergence_strength: 9000,
                    pim_reproducibility: 10000,
                    pim_potentiality_score: 4000,
                    pim_suggestion: 1,
                    pim_proof_chain: vec!["proof".into()],
                    npim_failure_density: 4000,
                    npim_contradiction_strength: 3000,
                    npim_elimination_certainty: 5000,
                    npim_negativity_score: 4000,
                    npim_suggestion: 1,
                    npim_arguments: vec!["negative".into()],
                    npim_arguments_blob_digest: fnv1a64(&[1, 2, 3]),
                    mpro_measurements: vec![1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0],
                    mpro_positives: 8,
                    mpro_total: 16,
                    mpro_vector_mass: 5000,
                    mpro_evidence_mass: 5000,
                    mpro_factic_mass: 2500,
                    scientific_project: None,
                    package_digest: 0,
                };
                e.package_digest = e.recompute_digest();
                e
            },
        },
        light: LightInboundWire {
            session_id: "s".into(),
            territory: "t".into(),
            primitive_flags: 0xA451,
            epistemic_mass: 0.5,
            proof_chain: vec![8],
            payload: vec![9],
        },
        negative: negative(),
    }
}

fn isolated_path(label: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let number = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "essmai_native_snapshot_{label}_{}_{}_{number}.wal",
        std::process::id(),
        nanos
    ))
}

// Construct once in this helper so future optional prepared data is not spread
// across tests. The receipt values below are declared synthetic fixture data.
fn fixture() -> (ShadowCycleRequest, NativeRequestBinding, VaultTransaction, ShadowCycleResponse) {
    fixture_with_experimental_negative(false)
}

fn fixture_with_experimental_negative(persist_negative: bool) -> (
    ShadowCycleRequest, NativeRequestBinding, VaultTransaction, ShadowCycleResponse,
) {
    let request = cycle_fixture();
    let bytes = encode_cycle_request(&request);
    let binding =
        NativeRequestBinding::from_request(&request, &bytes).expect("canonical fixture request");
    let mut verdict = crate::vault_transaction::test_support::sample_supreme(1, 1);
    verdict.living_trust_intensity = 10_000;
    verdict.living_trust_kind = crate::living_trust_contract::TRUST_KIND_CONSTRUCTIVE;
    verdict.living_trust_sovereign_value = crate::living_trust_contract::SOVEREIGN_SEAL_PRIMITIVE;
    let trust = crate::living_trust_contract::LivingTrustSeal {
        identity_sha256: verdict.living_trust_sha256,
        intensity: verdict.living_trust_intensity,
        kind: verdict.living_trust_kind,
        sovereign_value: verdict.living_trust_sovereign_value,
    };
    let mut receipt = crate::vault_transaction::test_support::sample_transaction().receipt;
    receipt.session_id = request.quantum.session_id.clone();
    receipt.parent_i0 = request.quantum.pd_parent_i0.clone().unwrap();
    receipt.primitive_anchor = 41;
    receipt.pd_binding_digest = request.quantum.pd_binding_digest.unwrap();
    receipt.pd_continuum_activation_digest =
        request.quantum.pd_continuum_activation_digest.unwrap();
    receipt.living_trust_digest = trust.identity_digest();
    receipt.seal = crate::sovereign_ffi_gate::S_SHADOW_VERIFY;
    receipt.receipt_id = crate::lab_contracts::verification_receipt::receipt_id(
        &receipt.session_id,
        &receipt.parent_i0,
        receipt.primitive_anchor,
        receipt.xy_digest,
        receipt.pd_binding_digest,
        receipt.pd_continuum_activation_digest,
        &receipt.living_trust_sha256,
        receipt.y_verdict,
        receipt.x_verdict,
        receipt.generation,
        receipt.seal,
    );
    let response = response_wire(ShadowResponse {
        session_id: receipt.session_id.clone(),
        verdict: verdict.clone(),
        known_negative: None,
        verification_receipt: receipt.clone(),
        negative_persisted: persist_negative,
    });
    let confirmed =
        crate::types::LawConfirmedVerdict::confirm(verdict, false).expect("fixture verdict");
    let mut base = crate::vault_transaction::test_support::sample_transaction();
    if persist_negative {
        // Storage fixture for source stage_parent_negative's constructive +
        // experimental-failure branch; this is not a live validated asset.
        base.knowledge_writes.push(crate::shadow_commit::KnowledgeWrite::Negative {
            input_id: 0xEA01,
            raw_bytes: b"fixture failed experimental parent".to_vec(),
            context: crate::types::NegativeContext {
                failure_reason_code: 0xA451, process: "fixture experiment".into(),
                mass_at_fail: 0.5, frequency: 1,
            },
            ts_ns: 41,
        });
    }
    let tx = VaultTransaction::build(
        binding.identity_digest,
        base.cycle_digest,
        base.anchor_generation,
        base.negative_batch_digest,
        base.runtime_delta_digest,
        base.vds_close_digest,
        base.law0_digest,
        &confirmed,
        base.knowledge_writes,
        base.lineage,
        base.gcl_reinforcement,
        receipt,
    );
    (request, binding, tx, response)
}

fn attach(
    tx: VaultTransaction,
    binding: &NativeRequestBinding,
    wire: ShadowCycleResponse,
) -> Result<VaultTransaction, VaultError> {
    let snapshot = NativeCycleSnapshot::new(binding, wire).expect("bounded snapshot fixture");
    tx.with_native_cycle(snapshot)
}

#[test]
fn request_binding_is_canonical_and_scoped_to_actual_parent() {
    let (request, binding, _, _) = fixture();
    assert_eq!(
        binding.identity_digest,
        identity_digest("s", "i0-0000000000000029", 41)
    );
    let mut noncanonical = encode_cycle_request(&request);
    noncanonical.push(0);
    assert!(NativeRequestBinding::from_request(&request, &noncanonical).is_err());
    for parent in [
        None,
        Some("i0-29".to_string()),
        Some("i0-0000000000000000".to_string()),
    ] {
        let mut changed = request.clone();
        changed.quantum.pd_parent_i0 = parent;
        assert!(
            NativeRequestBinding::from_request(&changed, &encode_cycle_request(&changed)).is_err()
        );
    }
    let mut changed = request.clone();
    changed.light.session_id.push('x');
    assert!(NativeRequestBinding::from_request(&changed, &encode_cycle_request(&changed)).is_err());
    let mut changed = request;
    changed.quantum.raw_bytes.push(0x77);
    let other =
        NativeRequestBinding::from_request(&changed, &encode_cycle_request(&changed)).unwrap();
    assert_eq!(other.identity_digest, binding.identity_digest);
    assert_ne!(other.request_sha256, binding.request_sha256);
}

#[test]
fn tx2_snapshot_survives_exact_wal_reopen_and_historical_retry() {
    for exact in [false, true] {
    let path = isolated_path("roundtrip");
    let (_, binding, tx, wire) = fixture();
    let expected = encode_cycle_response(&wire);
    let tx = attach(tx, &binding, wire).expect("attach valid snapshot");
    let tx = if exact { tx } else { tx.legacy_projection_fixture() };
    {
        let vault = KnowledgeVault::open_disk(&path).expect("disk vault");
        let committed = vault.commit_test_transaction(tx.clone()).expect("synced TX2");
        assert_eq!(committed.status, CommitStatus::Committed);
        assert_eq!(committed.native_cycle.unwrap().response, expected);
    }
    let bytes = std::fs::read(&path).expect("WAL bytes");
    // FILE header12, protected frame header12, transaction tag then TX version.
    assert_eq!(bytes[25], if exact { 5 } else { 2 }, "explicit storage projection version");
    if exact { assert_eq!(bytes[26], 2, "TX5 retains the snapshot-only profile"); }
    {
        let vault = KnowledgeVault::open_disk(&path).expect("replay TX2");
        let committed = vault
            .committed_identity(&binding.identity_digest)
            .unwrap()
            .unwrap();
        assert_eq!(committed.transaction_id, tx.transaction_id);
        assert_eq!(committed.native_cycle.unwrap().response, expected);
        assert_eq!(vault.primitive_count(), 1);
        let retry = vault.commit_test_transaction(tx).expect("exact retry");
        assert_eq!(retry.status, CommitStatus::AlreadyCommitted);
        assert_eq!(std::fs::metadata(&path).unwrap().len(), bytes.len() as u64);
        assert_eq!(vault.primitive_count(), 1);
    }
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn constructive_snapshot_with_actual_experimental_negative_write_survives_reopen() {
    let path = isolated_path("constructive_experimental_negative");
    let (_, binding, tx, wire) = fixture_with_experimental_negative(true);
    assert_eq!((wire.verification_receipt.y_verdict,wire.verification_receipt.x_verdict),(1,1));
    assert!(wire.negative_persisted);
    let expected = encode_cycle_response(&wire);
    let tx = attach(tx,&binding,wire).expect("positive with actual negative write");
    {
        let vault = KnowledgeVault::open_disk(&path).unwrap();
        vault.commit_test_transaction(tx.clone()).unwrap();
        assert_eq!(vault.primitive_count(),1);
        assert_eq!(vault.negative_count(),1);
    }
    {
        let vault = KnowledgeVault::open_disk(&path).unwrap();
        let prior = vault.committed_identity(&binding.identity_digest).unwrap().unwrap();
        assert_eq!(prior.native_cycle.unwrap().response,expected);
        assert_eq!(vault.primitive_count(),1);
        assert_eq!(vault.negative_count(),1);
        assert_eq!(vault.commit_test_transaction(tx).unwrap().status,CommitStatus::AlreadyCommitted);
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn changed_receipt_response_and_trust_are_rejected_before_attachment() {
    let (_, binding, tx, wire) = fixture();
    for case in 0..8 {
        let mut changed = wire.clone();
        match case {
            0 => changed.verification_receipt.generation += 1,
            1 => changed.verdict.verified = 0,
            2 => changed.verdict.living_trust_sha256 = "44".repeat(32),
            3 => changed.verdict.living_trust_intensity -= 1,
            4 => {
                changed.verdict.living_trust_kind =
                    crate::living_trust_contract::TRUST_KIND_RIGOROUS_NEGATIVE
            }
            5 => changed.verdict.living_trust_sovereign_value += 1,
            6 => changed.session_id.push('x'),
            7 => changed.negative_persisted = true, // No Negative write in fixture.
            _ => unreachable!(),
        }
        assert!(
            attach(tx.clone(), &binding, changed).is_err(),
            "invalid response case {case}"
        );
    }
    let mut altered = attach(tx, &binding, wire).unwrap();
    altered.native_cycle.as_mut().unwrap().request_sha256[0] ^= 1;
    assert!(
        altered.validate().is_err(),
        "TX ID must bind request snapshot"
    );
}

#[test]
fn same_cycle_changed_snapshot_is_conflict_in_live_commit_and_replay() {
    let (_, binding, tx, wire) = fixture();
    let original = attach(tx.clone(), &binding, wire.clone()).unwrap();
    let mut changed_binding = binding.clone();
    changed_binding.request_sha256[0] ^= 1;
    let request_changed = attach(tx.clone(), &changed_binding, wire.clone()).unwrap();
    let mut changed_wire = wire;
    changed_wire
        .verdict
        .lgc_dsl
        .push_str(";different-fixture-response");
    let response_changed = attach(tx, &binding, changed_wire).unwrap();
    for changed in [request_changed, response_changed] {
        assert_eq!(changed.identity_digest, original.identity_digest);
        assert_eq!(changed.cycle_digest, original.cycle_digest);
        assert_ne!(changed.transaction_id, original.transaction_id);
        let path = isolated_path("conflict");
        {
            let vault = KnowledgeVault::open_disk(&path).unwrap();
            vault.commit_test_transaction(original.clone()).unwrap();
            let length = std::fs::metadata(&path).unwrap().len();
            assert!(vault.commit_test_transaction(changed.clone()).is_err());
            assert_eq!(std::fs::metadata(&path).unwrap().len(), length);
            assert_eq!(vault.primitive_count(), 1);
        }
        // A raw internally valid conflicting frame must also fail semantic replay.
        {
            let (backend, _) = crate::vault_disk::DiskBackend::open(&path).unwrap();
            backend.commit_test_transaction(&changed).unwrap();
        }
        let before = std::fs::read(&path).unwrap();
        assert!(KnowledgeVault::open_disk(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        std::fs::remove_file(path).unwrap();
    }
}

#[test]
fn native_gateway_exact_replay_refuses_changed_request_and_legacy_snapshot() {
    let (_, binding, tx, wire) = fixture();
    let expected = encode_cycle_response(&wire);
    let shadow = crate::Shadow::new().expect("test Shadow");
    shadow
        .vault()
        .commit_test_transaction(attach(tx, &binding, wire).unwrap())
        .unwrap();
    assert_eq!(
        shadow.replay_native_cycle(&binding).unwrap(),
        Some(expected)
    );
    let mut changed = binding.clone();
    changed.request_sha256[0] ^= 1;
    assert!(shadow.replay_native_cycle(&changed).is_err());
    assert_eq!(shadow.vault().primitive_count(), 1);
    changed.identity_digest[0] ^= 1;
    assert_eq!(shadow.replay_native_cycle(&changed).unwrap(), None);
    let (_, binding, tx, _) = fixture();
    let legacy = crate::Shadow::new().unwrap();
    legacy.vault().commit_test_transaction(tx).unwrap();
    assert!(
        legacy.replay_native_cycle(&binding).is_err(),
        "TX1 has no recoverable native response"
    );
}

#[test]
fn export_native_replay_fixture_for_process_test() {
    let output = crate::test_output::OutputDir::new("ESSMAI_NATIVE_REPLAY_FIXTURE_DIR", "replay");
    let dir = output.path();
    assert!(dir.is_dir());
    let path = dir.join("snapshot_fixture.svlt");
    assert!(!path.exists());
    let (request, binding, tx, wire) = fixture();
    let expected = encode_cycle_response(&wire);
    let request_bytes = encode_cycle_request(&request);
    // R20: E(D(b)) = b and H(request) = bound request. These are integrity
    // relations, never proof that the synthetic verdict is scientifically true.
    assert_eq!(encode_cycle_request(&decode_cycle_request(&request_bytes).unwrap()), request_bytes);
    assert_eq!(encode_cycle_response(&decode_cycle_response(&expected).unwrap()), expected);
    assert_eq!(binding.request_sha256.as_slice(), Sha256::digest(&request_bytes).as_slice());
    let original = attach(tx.clone(), &binding, wire.clone()).unwrap();
    {
        let vault = KnowledgeVault::open_disk(&path).unwrap();
        assert_eq!(vault.commit_test_transaction(original.clone()).unwrap().status, CommitStatus::Committed);
        assert_eq!((vault.primitive_count(), vault.negative_count()), (1, 0));
    }
    let durable = std::fs::read(&path).unwrap();
    for punctuation in [b',', b'.', b';'] {
        // R20: replay(reopen(S), q) = replay(S, q), and an internally rehashed
        // different request under the same identity cannot replace its result.
        let vault = KnowledgeVault::open_disk(&path).unwrap();
        let stored = vault.committed_identity(&binding.identity_digest).unwrap().unwrap();
        assert_eq!(stored.transaction_id, original.transaction_id);
        let snapshot = stored.native_cycle.unwrap();
        assert_eq!(snapshot.response, expected);
        assert_eq!(snapshot.request_sha256, binding.request_sha256);
        assert_eq!(vault.commit_test_transaction(original.clone()).unwrap().status, CommitStatus::AlreadyCommitted);
        let mut changed = request.clone();
        changed.quantum.raw_bytes.push(punctuation);
        let changed_bytes = encode_cycle_request(&changed);
        let changed_binding = NativeRequestBinding::from_request(&changed, &changed_bytes).unwrap();
        assert_eq!(changed_binding.identity_digest, binding.identity_digest);
        assert_ne!(changed_binding.request_sha256, binding.request_sha256);
        let changed_tx = attach(tx.clone(), &changed_binding, wire.clone()).unwrap();
        match vault.commit_test_transaction(changed_tx).unwrap_err() {
            crate::types::ShadowError::VaultWriteError(message) =>
                assert_eq!(message, crate::vault_transaction::VaultError::ConflictingReplay.to_string()),
            other => panic!("expected same-identity material conflict, got {other:?}"),
        }
        assert_eq!((vault.primitive_count(), vault.negative_count()), (1, 0));
        drop(vault);
        assert_eq!(std::fs::read(&path).unwrap(), durable, "retry/refusal must preserve every WAL byte");
    }
    std::fs::write(dir.join("request.wire"), request_bytes).unwrap();
    std::fs::write(dir.join("expected_response.wire"), expected).unwrap();
    let mut changed = request;
    changed.quantum.raw_bytes.push(0x77);
    std::fs::write(
        dir.join("changed_request.wire"),
        encode_cycle_request(&changed),
    )
    .unwrap();
    std::fs::write(dir.join("FIXTURE_SCOPE.txt"), "Synthetic data-only source contract fixture. Native replay of stored canonical TX2 is under test, not original scientific admission or full GCL success.\n").unwrap();
    println!("R20_MATH {{\"test\":\"export_native_replay_fixture_for_process_test\",\"checks\":[\"codec_roundtrip\",\"request_sha\",\"restart_exact\",\"rehashed_request_conflict\",\"no_write_on_refusal\"]}}");
}


// Actual pinned numerical model; source record IDs remain synthetic fixture data.
fn kie_fixture() -> (shadow_contracts::kie::KieProof, science_domains::CandidateEvidence) {
    use science_domains::*;
    let identity = shadow_contracts::pd_memory::PdIdentity {
        owner:"storage-fixture".into(), obligation:"model-predicate".into(), case_id:"source-case".into(),
        scope:"astronomy".into(),domain:"astronomy".into(),lineage_root:"source-lineage".into(),
    };
    let query=ScienceQuery {
        adapter:AdapterId::Astronomy,session_id:shadow_contracts::pd_memory::hex(&identity.key().unwrap()),
        case_id:identity.case_id.clone(),scope_id:identity.scope.clone(),obligation_id:identity.obligation.clone(),
        query_id:"query-one".into(),input_source_id:"caller-UT1".into(),
        task:ScienceTask::EarthRotationAngle {jd_part1:2451545.0,jd_part2:0.0,time_scale:TimeScale::Ut1,
            claim_radians:0.0,tolerance_radians:0.1},
    };
    let candidate=evaluate(&query).unwrap();
    let query_bytes=encode_query(&query).unwrap();
    let candidate_bytes=encode_candidate(&candidate).unwrap();
    let actual=science_domains::kie::evaluate_kie(&query_bytes,&candidate_bytes).unwrap();
    let proof=shadow_contracts::kie::KieProof {
        context:shadow_contracts::kie::KieContext {identity,operation_id:"source-operation".into(),record_id:[4;32]},
        historical_request:kie_historical_fixture(&actual.canonical_proposition).0,
        historical_response:kie_historical_fixture(&actual.canonical_proposition).1,
        query_bytes,candidate_bytes,canonical_proposition:actual.canonical_proposition,
        kis_mass:actual.kis_mass,kic_mass:actual.kic_mass,relation:actual.relation,proof_sha256:[0;32],
        prior_adjudication:None,
        history_assessment:None,
    }.seal_data().unwrap();
    (proof,candidate)
}

#[test]
fn r21_model_counter_reason_cannot_replace_actual_native_scientific_origin() {
    let path=isolated_path("r21-counter-marker");
    let vault=KnowledgeVault::open_disk(&path).unwrap();
    let (_,binding,mut tx,mut response)=fixture();
    tx.knowledge_writes.push(crate::shadow_commit::KnowledgeWrite::Negative {
        input_id:999,raw_bytes:b"harmless mathematical claim".to_vec(),
        context:crate::types::NegativeContext {failure_reason_code:shadow_contracts::negative_asset::MODEL_COUNTERPROOF_REASON,
            process:"caller-asserted-model".into(),mass_at_fail:0.0,frequency:1},ts_ns:1});
    response.negative_persisted=true;
    let tx=attach(tx,&binding,response).unwrap();
    tx.validate().unwrap();
    drop(vault);
    let before=std::fs::read(&path).unwrap();
    let vault=KnowledgeVault::open_disk(&path).unwrap();
    let error=vault.commit_test_transaction(tx).unwrap_err();
    assert!(format!("{error:?}").contains("KIE_COUNTER_SOURCE_MISSING"),"{error:?}");
    assert_eq!((vault.primitive_count(),vault.negative_count()),(0,0));
    drop(vault);
    assert_eq!(std::fs::read(&path).unwrap(),before);
    std::fs::remove_file(&path).unwrap();
}

#[test]
fn r21_self_consistent_history_classification_needs_actual_vault_provenance() {
    use shadow_contracts::kie::*;
    let (mut proof,_)=kie_fixture();
    let mut response=shadow_contracts::decode_selection_response(&proof.historical_response).unwrap();
    response.negative.push(shadow_contracts::CandidateStatisticWire {
        candidate_id:99,polarity:0,domain:"asserted-observation".into(),
        term_sha256:shadow_contracts::sha256_hex(&[b"ESSMAI/SELECTION/TERM/NEGATIVE",proof.canonical_proposition.as_bytes()]),
        source_sha256:"e".repeat(64),security_y:0,trust_x:0,confidence_fixed:10000,frequency:1,domain_count:1,legacy_score_fixed:0});
    response.selection_sha256=shadow_contracts::pd_memory::failure::relevance::selection_digest(&response);
    proof.historical_response=shadow_contracts::encode_selection_response(&response);
    let vault=KnowledgeVault::new();
    let mut assessment=vault.kie_history_assessment(&proof).unwrap();
    assert_eq!(assessment.entries[0].class,KieHistoryClass::LegacyUnscoped);
    assessment.entries[0].class=KieHistoryClass::ProcessFailure;
    assessment.entries[0].provenance=Some(KieHistoryProvenance {
        transaction_id:[7;32],proof_sha256:[8;32],model_family_sha256:[9;32]});
    proof.history_assessment=Some(assessment);
    let proof=proof.seal_data().unwrap();
    assert_eq!(vault.validate_kie_history(&proof).unwrap_err(),"KIE_HISTORY_ACTUAL_SOURCE_MISMATCH");
    assert_eq!((vault.primitive_count(),vault.negative_count()),(0,0));
}

// Unit-only source data: executes the real pinned model but does not claim
// durable PDS origin, GCL admission, a sovereign receipt or native T1 success.
fn r21_supported_scoped_proof(scope:&str,jd_part2:f64)->shadow_contracts::kie::KieProof {
    let (mut proof,mut model)=kie_fixture();
    proof.context.identity.scope=scope.into();
    model.query.scope_id=scope.into();
    model.query.session_id=shadow_contracts::pd_memory::hex(&proof.context.identity.key().unwrap());
    let science_domains::ScienceTask::EarthRotationAngle {jd_part2:ref mut part,..}=model.query.task;
    *part=jd_part2;
    let evaluated=science_domains::evaluate(&model.query).unwrap();
    let science_domains::ScienceTask::EarthRotationAngle {ref mut claim_radians,..}=model.query.task;
    *claim_radians=evaluated.computed_radians;
    let evaluated=science_domains::evaluate(&model.query).unwrap();
    proof.query_bytes=science_domains::encode_query(&evaluated.query).unwrap();
    proof.candidate_bytes=science_domains::encode_candidate(&evaluated).unwrap();
    let actual=science_domains::kie::evaluate_kie(&proof.query_bytes,&proof.candidate_bytes).unwrap();
    proof.canonical_proposition=actual.canonical_proposition;
    proof.relation=actual.relation;proof.kis_mass=actual.kis_mass;proof.kic_mass=actual.kic_mass;
    (proof.historical_request,proof.historical_response)=kie_historical_fixture(&proof.canonical_proposition);
    proof.history_assessment=Some(KnowledgeVault::new().kie_history_assessment(&proof).unwrap());
    proof.seal_data().unwrap()
}

#[test]
fn r21_changed_scope_or_model_parameters_cannot_manufacture_positive_domain_diversity() {
    let judge=crate::vault_transaction::test_support::sample_transaction().knowledge_writes.remove(0);
    let mut domains=std::collections::BTreeSet::new();
    let mut families=std::collections::BTreeSet::new();
    for n in 0..6 {
        let proof=r21_supported_scoped_proof(&format!("scope-{n}"),n as f64);
        let model=science_domains::decode_candidate(&proof.candidate_bytes).unwrap();
        families.insert(kie_family(&model));
        let write=crate::kie_history::conditional_write(&proof,crate::types::ConstitutionalVerdict::Constructive,
            Some(&judge)).unwrap().unwrap();
        match write {crate::shadow_commit::KnowledgeWrite::Primitive {domain,..}=>{domains.insert(domain);},
            _=>panic!("unit support fixture")}
    }
    assert_eq!(families.len(),6);
    assert_eq!(domains.into_iter().collect::<Vec<_>>(),vec!["KIE/MODEL-CONDITIONAL/1/astronomy"]);
}

#[test]
fn r21_conditional_write_cannot_invent_metrics_beyond_its_original_judge_write() {
    let proof=r21_supported_scoped_proof("metric-scope",0.0);
    let (_,binding,mut tx,response)=fixture();
    let scoped=crate::kie_history::conditional_write(&proof,tx.verdict_kind,tx.knowledge_writes.first()).unwrap().unwrap();
    tx.knowledge_writes.push(scoped);
    let mut snapshot=NativeCycleSnapshot::new(&binding,response).unwrap();
    snapshot.kie_proof=shadow_contracts::kie::encode_proof(&proof).unwrap();
    tx.native_cycle=Some(snapshot);
    // This specifically tests the pure additional-write derivation. The full
    // commit validator additionally requires actual durable PDS and native PD.
    crate::kie_history::validate_new_markers(&tx).unwrap();
    for field in 0..4 {
        let mut changed=tx.clone();
        match changed.knowledge_writes.last_mut().unwrap() {
            crate::shadow_commit::KnowledgeWrite::Primitive {vector_points,temporal_score,mass,ts_ns,..}=>match field {
                0=>*vector_points=0.2,1=>*temporal_score=0.2,2=>*mass=0.2,_=>*ts_ns+=1,
            },
            _=>panic!("unit support fixture"),
        }
        assert_eq!(crate::kie_history::validate_new_markers(&changed).unwrap_err(),"KIE_CONDITIONAL_WRITE_METRICS_OR_SOURCE");
    }
}

#[test]
fn kie_attachment_binds_current_text_and_requires_prepared_pd_for_t1() {
    let (proof,_) = kie_fixture();
    let (mut request,_,tx,response)=fixture();
    request.quantum.final_evidence.light_input_bytes=proof.canonical_proposition.as_bytes().to_vec();
    request.quantum.final_evidence.light_input_sha256=format!("{:x}",Sha256::digest(proof.canonical_proposition.as_bytes()));
    let mut binding=NativeRequestBinding::from_request(&request,&encode_cycle_request(&request)).unwrap();
    binding.attach_kie(&proof,&request).unwrap();
    assert_eq!(shadow_contracts::kie::decode_proof(&binding.kie_proof).unwrap(),proof);
    assert!(attach(tx,&binding,response).unwrap_err().to_string().contains("KIE_PREPARED_REQUIRED"));
    let before=binding.kie_proof.clone();
    request.quantum.final_evidence.light_input_bytes.push(b' ');
    assert!(binding.attach_kie(&proof,&request).is_err());
    assert_eq!(binding.kie_proof,before,"failed attachment cannot replace original material");
}

#[test]
fn kie_family_does_not_count_renamed_query_as_independent_evidence() {
    let (_,original)=kie_fixture();
    let mut query=original.query.clone();
    query.query_id="renamed-query".into();
    query.input_source_id="renamed-source-label".into();
    let renamed=science_domains::evaluate(&query).unwrap();
    assert_ne!(original.query_sha256,renamed.query_sha256);
    assert_eq!(kie_family(&original),kie_family(&renamed));
    let science_domains::ScienceTask::EarthRotationAngle {ref mut tolerance_radians,..}=query.task;
    *tolerance_radians=0.2;
    assert_ne!(kie_family(&original),kie_family(&science_domains::evaluate(&query).unwrap()));
}

#[test]
fn kie_proof_cannot_populate_historical_map_without_actual_t1_and_source_record() {
    let (proof,candidate)=kie_fixture();
    let vault=KnowledgeVault::new();
    assert!(vault.kie_adjudicated(&proof.context.identity,&proof.canonical_proposition,&proof.query_bytes).unwrap().is_none());
    let mut other=proof.context.identity.clone();
    other.scope.push_str("-other");
    assert!(vault.kie_adjudicated(&other,&proof.canonical_proposition,&proof.query_bytes).is_err());
    assert_eq!(candidate.authority,science_domains::CandidateAuthority::None);
    assert_eq!(vault.primitive_count(),0);
    assert_eq!(vault.negative_count(),0);
}


// Exact empty selection shape copied from shadow-contracts::kie tests; not a real held authority.
fn kie_historical_fixture(text: &str) -> (Vec<u8>, Vec<u8>) {
    let q = shadow_contracts::KnowledgeSelectionRequestWire {
        version: shadow_contracts::PROTOCOL_VERSION,
        requester: shadow_contracts::SelectionRequester::AshtQuantum,
        purpose: shadow_contracts::SelectionPurpose::EvidenceCandidates,
        session_id: "attempt".into(),
        parent_i0: format!("{:x}",Sha256::digest(text.as_bytes())),
        primitive_anchor: 1,
        trace_id: "attempt".into(),
        split_sha256: "a".repeat(64),
        xi: vec![text.into()],
        yi: vec![],
        upstream_hold_sha256: Some("b".repeat(64)),
        evidence_sha256: "c".repeat(64),
        max_positive: shadow_contracts::SELECTION_MAX_CANDIDATES,
        max_negative: shadow_contracts::SELECTION_MAX_CANDIDATES,
    };
    let qb = shadow_contracts::encode_selection_request(&q);
    let mut s = shadow_contracts::KnowledgeSelectionResponseWire {
        version: shadow_contracts::PROTOCOL_VERSION,
        requester: q.requester,
        session_id: q.session_id.clone(),
        parent_i0: q.parent_i0.clone(),
        primitive_anchor: q.primitive_anchor,
        split_sha256: q.split_sha256.clone(),
        upstream_hold_sha256: q.upstream_hold_sha256.clone(),
        request_sha256: shadow_contracts::sha256_hex(&[b"ESSMAI/SELECTION/REQUEST/V178", &qb]),
        selection_sha256: String::new(),
        hold_id: "historical-fixture".into(),
        hold_sha256: "d".repeat(64),
        positive: vec![],
        negative: vec![],
        law_seal: 1,
        system_laws_seal: 1,
        expires_at_ns: 1,
    };
    s.selection_sha256 = shadow_contracts::pd_memory::failure::relevance::selection_digest(&s);
    (qb, shadow_contracts::encode_selection_response(&s))
}


#[test]
fn real_counter_model_cannot_be_persisted_as_positive_native_verdict() {
    let (counter,mut candidate)=kie_fixture();
    assert_eq!(candidate.relation,science_domains::ModelRelation::CountersClaim);
    assert!(validate_kie_verdict(&counter,1,1).unwrap_err().to_string().contains("KIE_POSITIVE_CONTRADICTS_CRITICAL_PROOF"));
    validate_kie_verdict(&counter,0,0).unwrap();
    let science_domains::ScienceTask::EarthRotationAngle {ref mut claim_radians,..}=candidate.query.task;
    *claim_radians=candidate.computed_radians;
    let actual=science_domains::evaluate(&candidate.query).unwrap();
    assert_eq!(actual.relation,science_domains::ModelRelation::SupportsClaim);
    let mut support=counter;
    support.query_bytes=science_domains::encode_query(&actual.query).unwrap();
    support.candidate_bytes=science_domains::encode_candidate(&actual).unwrap();
    let evaluation=science_domains::kie::evaluate_kie(&support.query_bytes,&support.candidate_bytes).unwrap();
    support.canonical_proposition=evaluation.canonical_proposition;
    (support.historical_request,support.historical_response)=kie_historical_fixture(&support.canonical_proposition);
    support.kis_mass=evaluation.kis_mass;support.kic_mass=evaluation.kic_mass;support.relation=evaluation.relation;
    let support=support.seal_data().unwrap();
    validate_kie_verdict(&support,1,1).unwrap(); // admissible data only, not a minted verdict.
    validate_kie_verdict(&support,0,0).unwrap(); // another constitutional gate may still refuse.
}


#[test]
fn caller_declared_adjudication_hashes_do_not_create_a_prior_vault_receipt() {
    let (mut proof,candidate)=kie_fixture();
    let vault=KnowledgeVault::new();
    proof.prior_adjudication=Some(shadow_contracts::kie::KieAdjudicatedRef {
        identity_digest:[11;32],transaction_id:[12;32],request_sha256:[13;32],
        proof_sha256:[14;32],evidence_family_digest:kie_family(&candidate),
        y:0,x:0,reason_code:None,
    });
    let proof=proof.seal_data().unwrap();
    assert!(crate::kie_selection::validate_prior_adjudication(&vault,&proof).is_err());
    assert_eq!(vault.primitive_count(),0);
    assert_eq!(vault.negative_count(),0);
}


// R10: actual pinned model evaluation with synthetic source-contract request/hold
// fixtures. These tests validate the admission binding, not a native GCL verdict.
#[test]
fn r10_failure_witness_rejects_changed_actual_request_material() {
    use shadow_contracts::pd_failure_witness::PdFailureWitness;
    let mut request=cycle_fixture();
    request.quantum.collapse_refused=Some("pa material primitiv: Xi=empty and Yi=empty".into());
    let witness=PdFailureWitness::from_request(&request,None).unwrap();
    validate_failure_witness(Some(&witness),Some(&request),None).unwrap();
    for field in 0..5 {
        let mut changed=request.clone();
        match field {
            0=>changed.quantum.final_evidence.light_input_bytes.push(b'!'),
            1=>changed.quantum.collapse_refused=Some("different refusal".into()),
            2=>changed.quantum.final_evidence.pd_spine.trace_digest^=1,
            3=>changed.quantum.final_evidence.muscle_evidence.asht_hold_sha256="ab".repeat(32),
            4=>changed.quantum.negative_paths.push("different path".into()),
            _=>unreachable!(),
        }
        assert!(validate_failure_witness(Some(&witness),Some(&changed),None).is_err(),
            "changed actual request field {field} must not retain a witness");
    }
    let mut changed=witness.clone();changed.lgc_tip[0]^=1;
    assert!(validate_failure_witness(Some(&changed),None,None).is_err());
}

#[test]
fn r10_failure_witness_requires_exact_optional_scientific_origin() {
    use shadow_contracts::pd_failure_witness::PdFailureWitness;
    let (proof,_)=kie_fixture();
    let generic_request=cycle_fixture();
    let generic=PdFailureWitness::from_request(&generic_request,None).unwrap();
    let mut scientific_request=generic_request.clone();
    scientific_request.quantum.final_evidence.muscle_evidence.asht_hold_id="kie-data-fixture".into();
    scientific_request.quantum.final_evidence.light_input_bytes=proof.canonical_proposition.as_bytes().to_vec();
    let scientific=PdFailureWitness::from_request(&scientific_request,Some(&proof)).unwrap();
    validate_failure_witness(Some(&scientific),Some(&scientific_request),Some(&proof)).unwrap();
    // Snapshot validation must reject both missing origin and falsely added origin.
    assert!(validate_failure_witness(Some(&scientific),None,None).is_err());
    assert!(validate_failure_witness(Some(&generic),None,Some(&proof)).is_err());
    assert!(validate_failure_witness(Some(&scientific),Some(&generic_request),Some(&proof)).is_err());
    assert!(PdFailureWitness::from_request(&generic_request,Some(&proof)).is_err());
    assert!(PdFailureWitness::from_request(&scientific_request,None).is_err());
    // Legacy None has no new assertion; original request/proof gates remain external.
    validate_failure_witness(None,Some(&generic_request),None).unwrap();
}

#[test]
fn r10_failure_witness_recomputes_model_instead_of_trusting_consistent_hashes() {
    use shadow_contracts::pd_failure_witness::PdFailureWitness;
    let (mut fabricated,candidate)=kie_fixture();
    assert_eq!(candidate.relation,science_domains::ModelRelation::CountersClaim);
    // Re-seal deliberately false data to prove hash consistency is insufficient.
    fabricated.relation=0;fabricated.kis_mass=10000;fabricated.kic_mass=0;
    let fabricated=fabricated.seal_data().unwrap();
    let mut request=cycle_fixture();
    request.quantum.final_evidence.muscle_evidence.asht_hold_id="kie-data-fixture".into();
    request.quantum.final_evidence.light_input_bytes=fabricated.canonical_proposition.as_bytes().to_vec();
    let witness=PdFailureWitness::from_request(&request,Some(&fabricated)).unwrap();
    witness.validate_for_request(&request).unwrap();
    witness.validate_model(Some(&fabricated)).unwrap(); // data consistency only
    let error=validate_failure_witness(Some(&witness),Some(&request),Some(&fabricated)).unwrap_err();
    assert!(error.contains("KIE_MODEL_PREDICATE_MISMATCH"),"{error}");
}

#[test]
fn r27_owner_recomputes_numeric_output_even_when_hashes_and_relation_agree() {
    use shadow_contracts::{pd_failure_witness::PdFailureWitness, kie_measurement::KieMeasurement,
        pd_memory::iz_science_math::EraComparison};
    let captured=include_bytes!("test_fixtures/late_refusal_r25/attempt.pdt1");
    let (request_bytes,_)=shadow_contracts::pd_cycle::decode_cycle(captured).unwrap();
    let request=shadow_contracts::decode_cycle_request(&request_bytes).unwrap();
    let bytes=include_bytes!("test_fixtures/late_refusal_r25/proof.kie");
    let n=u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let proof=shadow_contracts::kie::decode_response(&bytes[8+n..]).unwrap().proof;
    let candidate=science_domains::decode_candidate(&proof.candidate_bytes).unwrap();
    let baseline=PdFailureWitness::from_request_current(&request,Some(&proof)).unwrap();
    assert_eq!(validate_failure_witness(Some(&baseline),Some(&request),Some(&proof))
        .unwrap_err(),"KIE_MEASUREMENT_OWNER_REQUIRED");
    let measurement=crate::kie_selection::measure_current_model(&proof).unwrap();
    let bound=baseline.clone().with_model_measurement(&proof,measurement.clone()).unwrap();
    validate_failure_witness(Some(&bound),Some(&request),Some(&proof)).unwrap();
    validate_failure_witness(Some(&bound),None,Some(&proof)).unwrap();
    let science_domains::ScienceTask::EarthRotationAngle{claim_radians,tolerance_radians,..}=candidate.query.task;
    // Harmless coherent data forgery: same relation and query, false numerical output.
    let computed_radians=match candidate.relation {
        science_domains::ModelRelation::SupportsClaim if candidate.computed_radians.to_bits()!=claim_radians.to_bits()=>claim_radians,
        science_domains::ModelRelation::SupportsClaim=>(claim_radians+tolerance_radians*0.5).rem_euclid(std::f64::consts::TAU),
        _=>(candidate.computed_radians+0.000001).rem_euclid(std::f64::consts::TAU),
    };
    let mut signed=(computed_radians-claim_radians).rem_euclid(std::f64::consts::TAU);
    if signed>std::f64::consts::PI {signed-=std::f64::consts::TAU;}
    let altered=KieMeasurement::from_comparison(&proof,&EraComparison {
        query_sha256:candidate.query_sha256,result_sha256:candidate.result_sha256,
        model_sha256:candidate.provenance.compiled_model_sha256,claim_radians,computed_radians,
        residual_radians:signed.abs(),tolerance_radians,
    }).unwrap();
    assert_ne!(altered,measurement);
    let fake=baseline.with_model_measurement(&proof,altered).unwrap();
    fake.validate_model(Some(&proof)).unwrap(); // Data coherence is insufficient.
    for actual_request in [Some(&request),None] {
        assert_eq!(validate_failure_witness(Some(&fake),actual_request,Some(&proof))
            .unwrap_err(),"KIE_MEASUREMENT_OWNER_RECOMPUTE");
    }
}

