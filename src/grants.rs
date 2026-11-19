//! Replace the explicit placeholders with the functions assigned in docs/interfaces.md.

use crate::model::*;

pub fn validate_config(config: &GrantConfig) -> AppResult<()> {
    let _ = (config,);
    Err(AppError::not_implemented("validate_config"))
}

pub fn initialize_grant(state: &GrantState, config: GrantConfig) -> AppResult<GrantState> {
    let _ = (state, config);
    Err(AppError::not_implemented("initialize_grant"))
}

pub fn advance_round(state: &GrantState) -> AppResult<GrantState> {
    let _ = (state,);
    Err(AppError::not_implemented("advance_round"))
}

pub fn apply_snapshot(
    state: &GrantState,
    candidate: Candidate,
    signer_node_ids: Vec<String>,
) -> AppResult<GrantState> {
    let _ = (state, candidate, signer_node_ids);
    Err(AppError::not_implemented("apply_snapshot"))
}

pub fn apply_claim(state: &GrantState, caller: &str) -> AppResult<GrantState> {
    let _ = (state, caller);
    Err(AppError::not_implemented("apply_claim"))
}

/// Qualification is independent of whether it has already been claimed.
/// Acceptance/claim functions must validate authenticity, authorization and one-time use.
pub fn is_eligible(state: &GrantState) -> bool {
    match (&state.config, &state.snapshot) {
        (Some(config), Some(snapshot)) => {
            snapshot.candidate.config == *config
                && snapshot.candidate.round_id == state.round_id
                && snapshot.candidate.stable_count >= config.min_stable_releases
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[lyquid_test::test]
    fn eligibility_requires_matching_config_round_and_threshold() {
        assert!(!is_eligible(&GrantState::default()));
        let candidate: Candidate =
            serde_json::from_str(include_str!("../fixtures/offline_candidate.json")).unwrap();
        let mut state = GrantState {
            config: Some(candidate.config.clone()),
            round_id: 1,
            snapshot: Some(CertifiedSnapshot {
                candidate,
                signer_node_ids: Vec::new(),
            }),
            claimed: false,
            credits: 0,
        };
        // This is a pure predicate test with synthetic state, not an authenticated snapshot.
        assert!(is_eligible(&state));
        state.snapshot.as_mut().unwrap().candidate.stable_count = 1;
        assert!(!is_eligible(&state));
        state.snapshot.as_mut().unwrap().candidate.stable_count = 2;
        state.round_id = 2;
        assert!(!is_eligible(&state));
        state.round_id = 1;
        state.config.as_mut().unwrap().repo_id = 456;
        assert!(!is_eligible(&state));
    }
}
