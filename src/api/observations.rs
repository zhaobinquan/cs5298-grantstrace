#[method::instance(export = eth)]
fn refresh_observation(ctx: &mut _) -> LyquidResult<String> {
    let grant = &*ctx.network.grant;
    let node_id = ctx.node_id.to_string();
    let local = ctx.instance.observation.read().clone();
    let next = observations::refresh_state(grant, &local, observations::fetch_releases);
    if let Some(state) = next {
        let mut stored = ctx.instance.observation.write();
        // Another refresh may have finished while this call was fetching.
        if grant.config.is_some() && observations::is_ready_in_round(&stored, grant.round_id) {
            let response = observations::observation_response(grant, &stored, node_id);
            return api_to_json(&response).map_err(to_lyquid_error);
        }
        let response = match &state.error {
            Some(error) => api_error::<ObservationView>(error.clone()),
            None => observations::observation_response(grant, &state, node_id),
        };
        let json = api_to_json(&response).map_err(to_lyquid_error)?;
        *stored = state;
        Ok(json)
    } else {
        let response = observations::observation_response(grant, &local, node_id);
        api_to_json(&response).map_err(to_lyquid_error)
    }
}

#[method::instance(export = eth)]
fn get_observation(ctx: &_) -> LyquidResult<String> {
    let response = observations::observation_response(
        &ctx.network.grant,
        &ctx.instance.observation.read(),
        ctx.node_id.to_string(),
    );
    api_to_json(&response).map_err(to_lyquid_error)
}
