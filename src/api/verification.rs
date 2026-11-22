// B owns committee setup, candidate queries, certification and the validation hook.

#[method::network(export = eth)]
fn configure_committee(ctx: &mut _, committee_json: String) -> LyquidResult<bool> {
    if ctx.caller != *ctx.network.admin {
        return Err(to_lyquid_error(AppError::new(error_codes::E_UNAUTHORIZED, "admin only", false)));
    }
    let input: CommitteeConfig = serde_json::from_str(&committee_json)
        .map_err(|e| to_lyquid_error(AppError::new(error_codes::E_JSON, e.to_string(), false)))?;
    if input.node_ids.len() != COMMITTEE_SIZE {
        return Err(to_lyquid_error(AppError::new(error_codes::E_BAD_CONFIG, "four nodes required", false)));
    }
    let ids: Vec<NodeID> = input.node_ids.iter().map(|id| id.parse::<NodeID>()
        .map_err(|_| to_lyquid_error(AppError::new(error_codes::E_BAD_CONFIG, "invalid node ID", false))))
        .collect::<LyquidResult<_>>()?;
    let mut unique = input.node_ids.clone();
    unique.sort();
    unique.dedup();
    if unique.len() != COMMITTEE_SIZE {
        return Err(to_lyquid_error(AppError::new(error_codes::E_BAD_CONFIG, "duplicate node ID", false)));
    }
    let target = OracleTarget { seq_id: lyquor_api::sequence_backend_id()?, target: OracleServiceTarget::LVM(ctx.lyquid_id) };
    if ctx.network.releases.config_staging(&ctx, target).threshold != 0 {
        return Err(to_lyquid_error(AppError::new(error_codes::E_ALREADY_CONFIGURED, "committee already staged", false)));
    }
    if !ctx.network.releases.clone().initialize(&mut ctx, target, ids, COMMITTEE_THRESHOLD) {
        return Err(to_lyquid_error(AppError::new(error_codes::E_BAD_CONFIG, "committee initialization failed", false)));
    }
    Ok(true)
}

#[method::instance(export = eth)]
fn get_committee_status(ctx: &_) -> LyquidResult<String> {
    let target = OracleTarget { seq_id: lyquor_api::sequence_backend_id()?, target: OracleServiceTarget::LVM(ctx.lyquid_id) };
    let staged = ctx.network.releases.config_staging(&ctx, target);
    let active = ctx.network.releases.config_current(&ctx, target);
    let mut node_ids = staged.committee.keys().map(ToString::to_string).collect::<Vec<_>>();
    node_ids.sort();
    api_to_json(&api_ok(CommitteeView {
        configured: staged.threshold == COMMITTEE_THRESHOLD && node_ids.len() == COMMITTEE_SIZE,
        active: active.threshold == COMMITTEE_THRESHOLD && active.committee.len() == COMMITTEE_SIZE,
        threshold: staged.threshold,
        node_ids,
    })).map_err(to_lyquid_error)
}

#[method::instance(export = eth)]
fn get_candidate(ctx: &_) -> LyquidResult<String> {
    let grant = &*ctx.network.grant;
    let local = ctx.instance.observation.read();
    let response = match (&grant.config, &local.candidate) {
        (Some(config), Some(candidate)) if local.ready && local.round_id == grant.round_id =>
            match verification::verify_observation(config, grant.round_id, &local, candidate) {
                Ok(()) => api_ok(candidate.clone()),
                Err(error) => api_error(error),
            },
        _ => api_error(AppError::new(error_codes::E_NOT_READY, "current observation is not ready", true)),
    };
    api_to_json(&response).map_err(to_lyquid_error)
}

#[method::instance(export = eth)]
fn request_certification(ctx: &mut _) -> LyquidResult<String> {
    let grant = (*ctx.network.grant).clone();
    let Some(config) = grant.config.as_ref() else {
        return api_to_json(&api_error::<CertificationSubmission>(AppError::new(error_codes::E_NOT_CONFIGURED, "grant missing", false))).map_err(to_lyquid_error);
    };
    let local = ctx.instance.observation.read().clone();
    let Some(candidate) = local.candidate.as_ref() else {
        return api_to_json(&api_error::<CertificationSubmission>(AppError::new(error_codes::E_NOT_READY, "candidate missing", true))).map_err(to_lyquid_error);
    };
    if let Err(error) = verification::verify_observation(config, grant.round_id, &local, candidate) {
        return api_to_json(&api_error::<CertificationSubmission>(error)).map_err(to_lyquid_error);
    }
    let json = verification::candidate_to_json(candidate).map_err(to_lyquid_error)?;
    let target = OracleTarget { seq_id: lyquor_api::sequence_backend_id()?, target: OracleServiceTarget::LVM(ctx.lyquid_id) };
    let oracle = ctx.network.releases.clone();
    let certified = oracle.certify(&mut ctx, CertifiedCallParams {
        origin: Address::ZERO, method: "accept_snapshot".into(),
        input: encode_by_fields!(candidate_json: String = json).into(), target,
    }, Bytes::new(), None, Some(CERT_TIMEOUT_MS))?;
    let submitted = match certified { Some(call) => { let _ = submit_certified_call!(call)?; true }, None => false };
    let response = if submitted { api_ok(CertificationSubmission { grant_id: config.grant_id.clone(), round_id: grant.round_id, submitted }) }
        else { api_error(AppError::new(error_codes::E_CERT_REJECTED, "certification threshold not reached", true)) };
    api_to_json(&response).map_err(to_lyquid_error)
}

// Fail closed: the scaffold never signs off on a proposal.
#[method::instance(group = oracle::single_phase::releases)]
fn validate(
    ctx: &mut _,
    params: CallParams,
    extra: Bytes,
    target: OracleTarget,
) -> LyquidResult<bool> {
    if target.seq_id != lyquor_api::sequence_backend_id()? || !matches!(target.target, OracleServiceTarget::LVM(id) if id == ctx.lyquid_id)
        || params.method != "accept_snapshot" || params.group != "releases" || params.origin != Address::ZERO
        || params.caller != Address::from(ctx.lyquid_id) || params.abi != lyquid::lyquor_primitives::InputABI::Lyquor || !extra.is_empty() {
        return Ok(false);
    }
    let Some(decoded) = decode_by_fields!(&params.input, candidate_json: String) else { return Ok(false) };
    let Ok(candidate) = verification::candidate_from_json(&decoded.candidate_json) else { return Ok(false) };
    let grant = &*ctx.network.grant;
    let Some(config) = grant.config.as_ref() else { return Ok(false) };
    if grant.snapshot.is_some() { return Ok(false) }
    let local = ctx.instance.observation.read();
    Ok(verification::verify_observation(config, grant.round_id, &local, &candidate).is_ok())
}
