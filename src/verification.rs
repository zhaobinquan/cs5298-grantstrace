//! Replace the explicit placeholders with the functions assigned in docs/interfaces.md.

use crate::model::*;

pub fn build_candidate(
    config: &GrantConfig,
    round_id: u64,
    releases: &[ReleaseRecord],
) -> AppResult<Candidate> {
    let _ = (config, round_id, releases);
    Err(AppError::not_implemented("build_candidate"))
}

pub fn validate_candidate(
    config: &GrantConfig,
    round_id: u64,
    candidate: &Candidate,
) -> AppResult<()> {
    let _ = (config, round_id, candidate);
    Err(AppError::not_implemented("validate_candidate"))
}

pub fn candidate_to_json(candidate: &Candidate) -> AppResult<String> {
    let _ = (candidate,);
    Err(AppError::not_implemented("candidate_to_json"))
}

pub fn candidate_from_json(input: &str) -> AppResult<Candidate> {
    let _ = (input,);
    Err(AppError::not_implemented("candidate_from_json"))
}

pub fn verify_observation(
    config: &GrantConfig,
    round_id: u64,
    local: &ObservationState,
    proposed: &Candidate,
) -> AppResult<()> {
    let _ = (config, round_id, local, proposed);
    Err(AppError::not_implemented("verify_observation"))
}
