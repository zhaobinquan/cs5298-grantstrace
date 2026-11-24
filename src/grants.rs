//! Deterministic grant state transitions; runtime authorization stays in the API layer.

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
    if state.snapshot.is_some() {
        return Err(AppError::new(
            error_codes::E_FINALIZED,
            "snapshot already finalized",
            false,
        ));
    }
    if state.config.is_some() {
        return Err(AppError::new(
            error_codes::E_ALREADY_CONFIGURED,
            "grant already configured",
            false,
        ));
    }
    validate_config(&config)?;
    let mut next = state.clone();
    next.config = Some(config);
    Ok(next)
}

/// Only call after LDK verified the certificate against our pinned 3-of-4 policy.
/// Extra signer IDs are not evidence of extra signatures in LDK 0.8.4.
pub(crate) fn verified_signer_prefix(signers: &[u32]) -> AppResult<&[u32]> {
    let prefix = signers.get(..COMMITTEE_THRESHOLD as usize).ok_or_else(|| {
        AppError::new(
            error_codes::E_INTERNAL,
            "fewer than three verified signers",
            false,
        )
    })?;
    if !prefix.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(AppError::new(
            error_codes::E_INTERNAL,
            "invalid verified signer order",
            false,
        ));
    }
    Ok(prefix)
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

#[cfg(test)]
mod review_tests {
    use crate::{grants::*, verification::*};
    fn sample() -> Candidate {
        serde_json::from_str(include_str!("../fixtures/offline_candidate.json")).unwrap()
    }
    fn open() -> GrantState {
        advance_round(&initialize_grant(&GrantState::default(), sample().config).unwrap()).unwrap()
    }
    fn signers() -> Vec<String> {
        ["fixture-a", "fixture-b", "fixture-c"]
            .into_iter()
            .map(String::from)
            .collect()
    }
    fn accepted() -> GrantState {
        apply_snapshot(&open(), sample(), signers()).unwrap()
    }
    #[lyquid_test::test]
    fn configuration_rejects_invalid_boundaries() {
        let good = sample().config;
        assert!(validate_config(&good).is_ok());
        let mut bad = Vec::new();
        let mut c = good.clone();
        c.grant_id = "!".into();
        bad.push(c);
        let mut c = good.clone();
        c.repo_owner = "a/b".into();
        bad.push(c);
        let mut c = good.clone();
        c.repo_name = "..".into();
        bad.push(c);
        let mut c = good.clone();
        c.repo_id = 0;
        bad.push(c);
        let mut c = good.clone();
        c.window_end = c.window_start;
        bad.push(c);
        let mut c = good.clone();
        c.min_stable_releases = 33;
        bad.push(c);
        let mut c = good.clone();
        c.reward_credits = 0;
        bad.push(c);
        let mut c = good.clone();
        c.beneficiary = "0x0000000000000000000000000000000000000000".into();
        bad.push(c);
        let mut c = good.clone();
        c.beneficiary = "0xAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".into();
        bad.push(c);
        let mut c = good.clone();
        c.rule_version = 2;
        bad.push(c);
        for c in bad {
            assert_eq!(
                validate_config(&c).unwrap_err().code,
                error_codes::E_BAD_CONFIG
            );
        }
    }
    #[lyquid_test::test]
    fn configuration_and_round_are_one_way() {
        assert_eq!(
            advance_round(&GrantState::default()).unwrap_err().code,
            error_codes::E_NOT_CONFIGURED
        );
        let initial = initialize_grant(&GrantState::default(), sample().config).unwrap();
        assert_eq!(initial.round_id, 0);
        assert_eq!(
            initialize_grant(&initial, sample().config)
                .unwrap_err()
                .code,
            error_codes::E_ALREADY_CONFIGURED
        );
        let s = advance_round(&initial).unwrap();
        assert_eq!(s.round_id, 1);
        assert_eq!(initial.round_id, 0);
        assert_eq!(advance_round(&s).unwrap().round_id, 2);
    }
    #[lyquid_test::test]
    fn snapshot_rejects_bad_metadata_and_stale_round_and_freezes() {
        let s = open();
        let before = s.clone();
        assert_eq!(
            apply_snapshot(&s, sample(), vec!["a".into(), "a".into(), "b".into()])
                .unwrap_err()
                .code,
            error_codes::E_INTERNAL
        );
        let mut stale = sample();
        stale.round_id = 2;
        assert_eq!(
            apply_snapshot(&s, stale, signers()).unwrap_err().code,
            error_codes::E_ROUND_MISMATCH
        );
        assert_eq!(s, before);
        let done = apply_snapshot(
            &s,
            sample(),
            vec!["c".into(), "a".into(), "b".into(), "a".into()],
        )
        .unwrap();
        assert_eq!(
            done.snapshot.as_ref().unwrap().signer_node_ids,
            vec!["a", "b", "c"]
        );
        assert_eq!(
            apply_snapshot(&done, sample(), signers()).unwrap_err().code,
            error_codes::E_FINALIZED
        );
        assert_eq!(
            advance_round(&done).unwrap_err().code,
            error_codes::E_FINALIZED
        );
    }
    #[lyquid_test::test]
    fn claim_requires_beneficiary_and_is_once_only() {
        let s = accepted();
        let caller = s.config.as_ref().unwrap().beneficiary.clone();
        let before = s.clone();
        assert_eq!(
            apply_claim(&s, "0x2222222222222222222222222222222222222222")
                .unwrap_err()
                .code,
            error_codes::E_UNAUTHORIZED
        );
        assert_eq!(s, before);
        let done = apply_claim(&s, &caller).unwrap();
        assert!(done.claimed);
        assert_eq!(done.credits, 100);
        assert!(is_eligible(&done));
        assert_eq!(
            apply_claim(&done, &caller).unwrap_err().code,
            error_codes::E_ALREADY_CLAIMED
        );
        assert_eq!(s, before);
    }
    #[lyquid_test::test]
    fn missing_or_ineligible_snapshot_cannot_claim() {
        let s = open();
        let caller = s.config.as_ref().unwrap().beneficiary.clone();
        assert_eq!(
            apply_claim(&s, &caller).unwrap_err().code,
            error_codes::E_NOT_ELIGIBLE
        );
        let zero = build_candidate(s.config.as_ref().unwrap(), 1, &[]).unwrap();
        let done = apply_snapshot(&s, zero, signers()).unwrap();
        assert!(!is_eligible(&done));
        assert_eq!(
            apply_claim(&done, &caller).unwrap_err().code,
            error_codes::E_NOT_ELIGIBLE
        );
    }
    #[lyquid_test::test]
    fn overflow_preserves_input_state() {
        let mut s = open();
        s.round_id = u64::MAX;
        let before = s.clone();
        assert_eq!(advance_round(&s).unwrap_err().code, error_codes::E_INTERNAL);
        assert_eq!(s, before);
        let mut s = accepted();
        s.credits = u64::MAX;
        let before = s.clone();
        assert_eq!(
            apply_claim(&s, &s.config.as_ref().unwrap().beneficiary)
                .unwrap_err()
                .code,
            error_codes::E_INTERNAL
        );
        assert_eq!(s, before);
    }
}

#[cfg(test)]
mod certificate_tests {
    use super::*;
    #[lyquid_test::test]
    fn records_only_verified_signer_prefix() {
        assert_eq!(verified_signer_prefix(&[1, 2, 3, 99]).unwrap(), &[1, 2, 3]);
        assert!(verified_signer_prefix(&[1, 2]).is_err());
        assert!(verified_signer_prefix(&[1, 1, 3]).is_err());
        assert!(verified_signer_prefix(&[3, 2, 1]).is_err());
    }
}
