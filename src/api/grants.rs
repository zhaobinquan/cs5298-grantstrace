// C owns these entry points.

#[method::network(export = eth)]
fn constructor(ctx: &mut _) {
    *ctx.network.admin = ctx.caller;
    *ctx.network.grant = GrantState::default();
}

#[method::network(export = eth)]
fn configure_grant(ctx: &mut _, config_json: String) -> LyquidResult<bool> {
    if ctx.caller != *ctx.network.admin { return Err(to_lyquid_error(AppError::new(error_codes::E_UNAUTHORIZED, "admin only", false))); }
    let config: GrantConfig = serde_json::from_str(&config_json).map_err(|e| to_lyquid_error(AppError::new(error_codes::E_JSON, e.to_string(), false)))?;
    *ctx.network.grant = grants::initialize_grant(&ctx.network.grant, config).map_err(to_lyquid_error)?;
    Ok(true)
}

#[method::network(export = eth)]
fn start_round(ctx: &mut _) -> LyquidResult<u64> {
    if ctx.caller != *ctx.network.admin { return Err(to_lyquid_error(AppError::new(error_codes::E_UNAUTHORIZED, "admin only", false))); }
    let next = grants::advance_round(&ctx.network.grant).map_err(to_lyquid_error)?;
    let round = next.round_id;
    *ctx.network.grant = next;
    Ok(round)
}

// No export=eth: callers must go through the LDK's certified route.
#[method::network(group = oracle::certified::releases)]
fn accept_snapshot(ctx: &mut _, candidate_json: String) -> LyquidResult<bool> {
    let candidate = verification::candidate_from_json(&candidate_json).map_err(to_lyquid_error)?;
    let signer_ids = ctx.cert.signers.clone();
    let signers = signer_ids.into_iter().filter_map(|id| ctx.signer_node_id(id as u64)).map(|id| id.to_string()).collect();
    *ctx.network.grant = grants::apply_snapshot(&ctx.network.grant, candidate, signers).map_err(to_lyquid_error)?;
    Ok(true)
}

#[method::network(export = eth)]
fn get_grant_status(ctx: &_) -> LyquidResult<String> {
    let state = (*ctx.network.grant).clone();
    let status = GrantStatus {
        admin: (*ctx.network.admin).to_string().to_ascii_lowercase(),
        eligible: grants::is_eligible(&state),
        state,
    };
    api_to_json(&api_ok(status)).map_err(to_lyquid_error)
}

#[method::network(export = eth)]
fn claim(ctx: &mut _) -> LyquidResult<u64> {
    let caller = ctx.caller.to_string().to_ascii_lowercase();
    let next = grants::apply_claim(&ctx.network.grant, &caller).map_err(to_lyquid_error)?;
    let credits = next.credits;
    *ctx.network.grant = next;
    Ok(credits)
}
