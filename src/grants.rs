//! Replace the explicit placeholders with the functions assigned in docs/interfaces.md.

use crate::model::*;

pub fn validate_config(config: &GrantConfig) -> AppResult<()> {
    if config.grant_id.is_empty()
        || config.grant_id.len() > 64
        || !config
            .grant_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        || config.repo_id == 0
        || config.window_start >= config.window_end
        || !(1..=32).contains(&config.min_stable_releases)
        || !(1..=1_000_000).contains(&config.reward_credits)
        || config.rule_version != RULE_VERSION
        || config.beneficiary.len() != 42
        || !config.beneficiary.starts_with("0x")
        || !config.beneficiary[2..]
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || config.beneficiary[2..].bytes().all(|b| b == b'0')
        || config.repo_owner.is_empty()
        || config.repo_owner.len() > 100
        || !config
            .repo_owner
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        || config.repo_name.is_empty()
        || config.repo_name.len() > 100
        || matches!(config.repo_name.as_str(), "." | "..")
        || !config
            .repo_name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    {
        return Err(AppError::new(
            error_codes::E_BAD_CONFIG,
            "invalid grant configuration",
            false,
        ));
    }
    Ok(())
}

pub fn initialize_grant(state: &GrantState, config: GrantConfig) -> AppResult<GrantState> {
    validate_config(&config)?;
    if state.config.is_some() || state.snapshot.is_some() {
        return Err(AppError::new(
            error_codes::E_ALREADY_CONFIGURED,
            "grant already configured",
            false,
        ));
    }
    let mut next = state.clone();
    next.config = Some(config);
    Ok(next)
}

pub fn advance_round(state: &GrantState) -> AppResult<GrantState> {
    if state.config.is_none() {
        return Err(AppError::new(
            error_codes::E_NOT_CONFIGURED,
            "grant missing",
            false,
        ));
    }
    if state.snapshot.is_some() {
        return Err(AppError::new(
            error_codes::E_FINALIZED,
            "snapshot already finalized",
            false,
        ));
    }
    let mut next = state.clone();
    next.round_id = next
        .round_id
        .checked_add(1)
        .ok_or_else(|| AppError::new(error_codes::E_INTERNAL, "round overflow", false))?;
    Ok(next)
}

pub fn apply_snapshot(
    state: &GrantState,
    candidate: Candidate,
    signer_node_ids: Vec<String>,
) -> AppResult<GrantState> {
    let config = state
        .config
        .as_ref()
        .ok_or_else(|| AppError::new(error_codes::E_NOT_CONFIGURED, "grant missing", false))?;
    if state.snapshot.is_some() {
        return Err(AppError::new(
            error_codes::E_FINALIZED,
            "snapshot already finalized",
            false,
        ));
    }
    crate::verification::validate_candidate(config, state.round_id, &candidate)?;
    let mut signers = signer_node_ids;
    signers.sort();
    signers.dedup();
    if signers.len() < COMMITTEE_THRESHOLD as usize || signers.len() > COMMITTEE_SIZE {
        return Err(AppError::new(
            error_codes::E_INTERNAL,
            "invalid signer metadata",
            false,
        ));
    }
    let mut next = state.clone();
    next.snapshot = Some(CertifiedSnapshot {
        candidate,
        signer_node_ids: signers,
    });
    Ok(next)
}

pub fn apply_claim(state: &GrantState, caller: &str) -> AppResult<GrantState> {
    let config = state
        .config
        .as_ref()
        .ok_or_else(|| AppError::new(error_codes::E_NOT_CONFIGURED, "grant missing", false))?;
    if caller != config.beneficiary {
        return Err(AppError::new(
            error_codes::E_UNAUTHORIZED,
            "beneficiary only",
            false,
        ));
    }
    if state.claimed {
        return Err(AppError::new(
            error_codes::E_ALREADY_CLAIMED,
            "already claimed",
            false,
        ));
    }
    if !is_eligible(state) {
        return Err(AppError::new(
            error_codes::E_NOT_ELIGIBLE,
            "grant not eligible",
            false,
        ));
    }
    let mut next = state.clone();
    next.credits = next
        .credits
        .checked_add(config.reward_credits)
        .ok_or_else(|| AppError::new(error_codes::E_INTERNAL, "credit overflow", false))?;
    next.claimed = true;
    Ok(next)
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
