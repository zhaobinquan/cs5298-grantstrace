// B owns committee setup, candidate queries, certification and the validation hook.

#[method::network(export = eth)]
fn configure_committee(ctx: &mut _, committee_json: String) -> LyquidResult<bool> {
    let _ = (&ctx, committee_json);
    unfinished_lyquid("configure_committee")
}

#[method::instance(export = eth)]
fn get_committee_status(ctx: &_) -> LyquidResult<String> {
    let _ = ctx;
    unfinished_json::<CommitteeView>("get_committee_status")
}

#[method::instance(export = eth)]
fn get_candidate(ctx: &_) -> LyquidResult<String> {
    let _ = ctx;
    unfinished_json::<Candidate>("get_candidate")
}

#[method::instance(export = eth)]
fn request_certification(ctx: &mut _) -> LyquidResult<String> {
    let _ = ctx;
    unfinished_json::<CertificationSubmission>("request_certification")
}

// Fail closed: the scaffold never signs off on a proposal.
#[method::instance(group = oracle::single_phase::releases)]
fn validate(
    ctx: &mut _,
    params: CallParams,
    extra: Bytes,
    target: OracleTarget,
) -> LyquidResult<bool> {
    let _ = (&ctx, params, extra, target);
    Ok(false)
}
