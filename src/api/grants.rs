// C owns these entry points.

#[method::network(export = eth)]
fn constructor(ctx: &mut _) {
    *ctx.network.admin = ctx.caller;
    *ctx.network.grant = GrantState::default();
}

#[method::network(export = eth)]
fn configure_grant(ctx: &mut _, config_json: String) -> LyquidResult<bool> {
    let _ = (&ctx, config_json);
    unfinished_lyquid("configure_grant")
}

#[method::network(export = eth)]
fn start_round(ctx: &mut _) -> LyquidResult<u64> {
    let _ = ctx;
    unfinished_lyquid("start_round")
}

// No export=eth: callers must go through the LDK's certified route.
#[method::network(group = oracle::certified::releases)]
fn accept_snapshot(ctx: &mut _, candidate_json: String) -> LyquidResult<bool> {
    let _ = (&ctx, candidate_json);
    unfinished_lyquid("accept_snapshot")
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
    let _ = ctx;
    unfinished_lyquid("claim")
}
