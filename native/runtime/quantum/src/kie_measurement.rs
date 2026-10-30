//! Current model measurement; no confidence, candidate count or authority is synthesized.
pub fn measure(proof: &shadow_contracts::kie::KieProof)
    -> Result<shadow_contracts::kie_measurement::KieMeasurement, String> {
    validate_model_scope(proof)?;
    let measured = science_domains::kie::measure_kie(&proof.query_bytes, &proof.candidate_bytes)
        .map_err(|e| format!("KIE_MEASURED_MODEL:{e}"))?;
    let actual = measured.candidate;
    let science_domains::ScienceTask::EarthRotationAngle { claim_radians, tolerance_radians, .. } = actual.query.task;
    shadow_contracts::kie_measurement::KieMeasurement::from_comparison(proof,
        &shadow_contracts::pd_memory::iz_science_math::EraComparison {
            query_sha256:actual.query_sha256, result_sha256:actual.result_sha256,
            model_sha256:actual.provenance.compiled_model_sha256, claim_radians,
            computed_radians:actual.computed_radians,
            residual_radians:actual.circular_residual_radians, tolerance_radians,
        })
}




/// Exact declared scope is checked before either scientific endpoint can run.
fn validate_model_scope(proof: &shadow_contracts::kie::KieProof) -> Result<(), String> {
    proof.validate()?;
    let query = science_domains::decode_query(&proof.query_bytes)
        .map_err(|error| format!("KIE_MEASURED_QUERY:{error}"))?;
    let canonical = science_domains::kie::canonical_proposition(&query)
        .map_err(|error| format!("KIE_MEASURED_PROPOSITION:{error}"))?;
    let identity = &proof.context.identity;
    if query.session_id != shadow_contracts::pd_memory::hex(&identity.key()?)
        || query.case_id != identity.case_id || query.scope_id != identity.scope
        || query.obligation_id != identity.obligation
        || canonical != proof.canonical_proposition
    {
        return Err("KIE_MEASURED_MODEL_SCOPE".into());
    }
    Ok(())
}

/// Execute a scoped scientific predicate under the current physical lease.
/// The lexical operator mask is not reopened. A held endpoint produces no
/// scientific candidate; a completed zero predicate is not a countercandidate.
pub(crate) fn measure_endpoint<H: crate::pro::HwScaffoldView>(
    hw: &H,
    proof: &shadow_contracts::kie::KieProof,
    directive: &crate::ultimatum_collapse_law::AshtOperatorDirective,
    sealed: bool,
) -> Result<Option<shadow_contracts::kie_measurement::KieMeasurement>, String> {
    validate_model_scope(proof)?;
    if !directive.shape_is_valid()
        || directive.selection_sha256 != proof.selection_sha256()
        || directive.allowed_candidate_sha256 != proof.predicate_references()
    {
        return Err("KIE_OPERATOR_DIRECTIVE_BINDING".into());
    }
    let Some(lease) = hw.runtime_lease() else { return Ok(None); };
    let permitted = shadow_contracts::model_operator_measurement::resource_capacity_permitted(
        proof, sealed, lease.lease_id, lease.granted_bytes,
        lease.granted_parallel, lease.active,
    )? && hw.work_permitted();
    if !permitted { return Ok(None); }
    measure(proof).map(Some)
}
