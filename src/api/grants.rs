// C owns these entry points.

#[method::network(export = eth)]
fn constructor(ctx: &mut _) {
    *ctx.network.admin = ctx.caller;
    *ctx.network.grant = GrantState::default();
    *ctx.network.committee_nodes = Vec::new();
}

#[method::network(export = eth)]
fn configure_grant(ctx: &mut _, config_json: String) -> LyquidResult<bool> {
    if ctx.caller != *ctx.network.admin {
        return Err(to_lyquid_error(AppError::new(
            error_codes::E_UNAUTHORIZED,
            "admin only",
            false,
        )));
    }
    let config: GrantConfig = serde_json::from_str(&config_json)
        .map_err(|e| to_lyquid_error(AppError::new(error_codes::E_JSON, e.to_string(), false)))?;
    *ctx.network.grant =
        grants::initialize_grant(&ctx.network.grant, config).map_err(to_lyquid_error)?;
    Ok(true)
}

#[method::network(export = eth)]
fn start_round(ctx: &mut _) -> LyquidResult<u64> {
    if ctx.caller != *ctx.network.admin {
        return Err(to_lyquid_error(AppError::new(
            error_codes::E_UNAUTHORIZED,
            "admin only",
            false,
        )));
    }
    let next = grants::advance_round(&ctx.network.grant).map_err(to_lyquid_error)?;
    let target = OracleTarget {
        seq_id: lyquor_api::sequence_backend_id()?,
        target: OracleServiceTarget::LVM(ctx.lyquid_id),
    };
    let active = ctx.network.releases.config_current(&ctx, target);
    let staged = ctx.network.releases.config_staging(&ctx, target);
    let mut active_nodes = active
        .committee
        .keys()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    active_nodes.sort();
    if active.threshold != COMMITTEE_THRESHOLD
        || active_nodes.len() != COMMITTEE_SIZE
        || !committee_matches(&active_nodes, staged)
        || (!ctx.network.committee_nodes.is_empty()
            && !committee_matches(&ctx.network.committee_nodes, active))
    {
        return Err(to_lyquid_error(AppError::new(
            error_codes::E_COMMITTEE_NOT_READY,
            "the configured 3-of-4 committee must be active and unchanged",
            true,
        )));
    }
    let round = next.round_id;
    *ctx.network.grant = next;
    if ctx.network.committee_nodes.is_empty() {
        *ctx.network.committee_nodes = active_nodes;
    }
    Ok(round)
}

// No export=eth: callers must go through the LDK's certified route.
#[method::network(group = oracle::certified::releases)]
fn accept_snapshot(ctx: &mut _, candidate_json: String) -> LyquidResult<bool> {
    if ctx.origin != Address::ZERO || ctx.caller != Address::from(ctx.lyquid_id) {
        return Err(to_lyquid_error(AppError::new(
            error_codes::E_UNAUTHORIZED,
            "invalid certified caller",
            false,
        )));
    }
    let target = OracleTarget {
        seq_id: lyquor_api::sequence_backend_id()?,
        target: OracleServiceTarget::LVM(ctx.lyquid_id),
    };
    let active = ctx.network.releases.config_current(&ctx, target);
    // Source policy must agree with the certificate already verified by LDK at the destination.
    if !committee_matches(&ctx.network.committee_nodes, active)
        || ctx.cert.header.config_hash
            != active
                .to_hash(lyquid::lyquor_primitives::Cipher::Ed25519)
                .into()
        || ctx.cert.header.epoch != active.epoch
    {
        return Err(to_lyquid_error(AppError::new(
            error_codes::E_COMMITTEE_NOT_READY,
            "certificate does not match the pinned active committee",
            true,
        )));
    }
    let candidate = verification::candidate_from_json(&candidate_json).map_err(to_lyquid_error)?;
    // LDK 0.8.4 verifies only the first `threshold` entries, not an optional suffix.
    let signer_ids = grants::verified_signer_prefix(&ctx.cert.signers)
        .map_err(to_lyquid_error)?
        .to_vec();
    let signers = signer_ids
        .into_iter()
        .map(|id| ctx.signer_node_id(id as u64).map(|node| node.to_string()))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            to_lyquid_error(AppError::new(
                error_codes::E_INTERNAL,
                "unresolved signer ID",
                false,
            ))
        })?;
    if signers
        .iter()
        .any(|node| !ctx.network.committee_nodes.contains(node))
    {
        return Err(to_lyquid_error(AppError::new(
            error_codes::E_INTERNAL,
            "signer outside pinned committee",
            false,
        )));
    }
    *ctx.network.grant =
        grants::apply_snapshot(&ctx.network.grant, candidate, signers).map_err(to_lyquid_error)?;
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
