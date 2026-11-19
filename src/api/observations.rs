// A owns these entry points. They currently report E_NOT_IMPLEMENTED.

#[method::instance(export = eth)]
fn refresh_observation(ctx: &mut _) -> LyquidResult<String> {
    let _ = ctx;
    unfinished_json::<ObservationView>("refresh_observation")
}

#[method::instance(export = eth)]
fn get_observation(ctx: &_) -> LyquidResult<String> {
    let _ = ctx;
    unfinished_json::<ObservationView>("get_observation")
}
