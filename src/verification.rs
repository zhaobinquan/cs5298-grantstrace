//! Replace the explicit placeholders with the functions assigned in docs/interfaces.md.

use crate::model::*;

fn error(code: &str, message: impl Into<String>) -> AppError {
    AppError::new(code, message, false)
}

fn stable_count(releases: &[ReleaseRecord]) -> AppResult<u32> {
    u32::try_from(
        releases
            .iter()
            .filter(|release| !release.draft && !release.prerelease)
            .count(),
    )
    .map_err(|_| {
        error(
            error_codes::E_INTERNAL,
            "stable release count overflowed u32",
        )
    })
}

pub fn build_candidate(
    config: &GrantConfig,
    round_id: u64,
    releases: &[ReleaseRecord],
) -> AppResult<Candidate> {
    if round_id == 0 {
        return Err(error(
            error_codes::E_ROUND_MISMATCH,
            "round_id must be greater than zero",
        ));
    }

    let mut normalized = releases.to_vec();
    normalized.sort_by_key(|release| release.id);

    let mut unique: Vec<ReleaseRecord> = Vec::with_capacity(normalized.len());
    let mut previous: Option<ReleaseRecord> = None;
    for release in normalized {
        if let Some(previous) = &previous {
            if previous.id == release.id {
                if previous != &release {
                    return Err(error(
                        error_codes::E_DUPLICATE_CONFLICT,
                        format!("release id {} has conflicting fields", release.id),
                    ));
                }
                continue;
            }
        }

        if release.published_at >= config.window_start && release.published_at < config.window_end {
            unique.push(release.clone());
        }
        previous = Some(release);
    }

    if unique.len() > MAX_WINDOW_RECORDS {
        return Err(error(
            error_codes::E_TOO_LARGE,
            format!("window contains more than {MAX_WINDOW_RECORDS} release records"),
        ));
    }

    Ok(Candidate {
        schema_version: SCHEMA_VERSION,
        round_id,
        config: config.clone(),
        stable_count: stable_count(&unique)?,
        releases: unique,
    })
}

pub fn validate_candidate(
    config: &GrantConfig,
    round_id: u64,
    candidate: &Candidate,
) -> AppResult<()> {
    if round_id == 0 || candidate.round_id != round_id {
        return Err(error(
            error_codes::E_ROUND_MISMATCH,
            "candidate round does not match",
        ));
    }
    if candidate.schema_version != SCHEMA_VERSION {
        return Err(error(
            error_codes::E_CANDIDATE_MISMATCH,
            "candidate schema version is unsupported",
        ));
    }
    if candidate.config != *config {
        return Err(error(
            error_codes::E_CANDIDATE_MISMATCH,
            "candidate configuration does not match",
        ));
    }
    if candidate.releases.len() > MAX_WINDOW_RECORDS {
        return Err(error(
            error_codes::E_TOO_LARGE,
            "candidate has too many release records",
        ));
    }

    let mut previous_id = None;
    for release in &candidate.releases {
        if release.id == 0
            || release.published_at < config.window_start
            || release.published_at >= config.window_end
        {
            return Err(error(
                error_codes::E_CANDIDATE_MISMATCH,
                "candidate has an invalid release",
            ));
        }
        if previous_id.is_some_and(|id| release.id <= id) {
            return Err(error(
                error_codes::E_CANDIDATE_MISMATCH,
                "candidate releases must be strictly ordered by id",
            ));
        }
        previous_id = Some(release.id);
    }

    if candidate.stable_count != stable_count(&candidate.releases)? {
        return Err(error(
            error_codes::E_CANDIDATE_MISMATCH,
            "candidate stable_count does not match its releases",
        ));
    }

    if serde_json::to_string(candidate)
        .map_err(|serialization_error| error(error_codes::E_JSON, serialization_error.to_string()))?
        .len()
        > MAX_CANDIDATE_BYTES
    {
        return Err(error(
            error_codes::E_TOO_LARGE,
            "candidate JSON exceeds the size limit",
        ));
    }

    Ok(())
}

pub fn candidate_to_json(candidate: &Candidate) -> AppResult<String> {
    let json = serde_json::to_string(candidate).map_err(|serialization_error| {
        error(error_codes::E_JSON, serialization_error.to_string())
    })?;
    if json.len() > MAX_CANDIDATE_BYTES {
        return Err(error(
            error_codes::E_TOO_LARGE,
            "candidate JSON exceeds the size limit",
        ));
    }
    Ok(json)
}

pub fn candidate_from_json(input: &str) -> AppResult<Candidate> {
    if input.len() > MAX_CANDIDATE_BYTES {
        return Err(error(
            error_codes::E_TOO_LARGE,
            "candidate JSON exceeds the size limit",
        ));
    }
    let candidate = serde_json::from_str::<Candidate>(input)
        .map_err(|parse_error| error(error_codes::E_JSON, parse_error.to_string()))?;
    validate_candidate(&candidate.config, candidate.round_id, &candidate)?;
    if candidate_to_json(&candidate)? != input {
        return Err(error(
            error_codes::E_CANDIDATE_MISMATCH,
            "candidate JSON is not in canonical compact form",
        ));
    }
    Ok(candidate)
}

pub fn verify_observation(
    config: &GrantConfig,
    round_id: u64,
    local: &ObservationState,
    proposed: &Candidate,
) -> AppResult<()> {
    validate_candidate(config, round_id, proposed)?;
    if !local.ready || local.error.is_some() || local.candidate.is_none() {
        return Err(error(error_codes::E_NOT_READY, "observation is not ready"));
    }
    if local.round_id != round_id {
        return Err(error(
            error_codes::E_ROUND_MISMATCH,
            "observation belongs to another round",
        ));
    }
    if local.candidate.as_ref() != Some(proposed) {
        return Err(error(
            error_codes::E_CANDIDATE_MISMATCH,
            "proposed candidate differs from this node's observation",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> GrantConfig {
        GrantConfig {
            grant_id: "grant-001".into(),
            repo_owner: "demo-owner".into(),
            repo_name: "demo-repo".into(),
            repo_id: 123,
            window_start: 1_000,
            window_end: 2_000,
            min_stable_releases: 2,
            beneficiary: "0x1111111111111111111111111111111111111111".into(),
            reward_credits: 100,
            rule_version: RULE_VERSION,
        }
    }

    fn release(id: u64, published_at: u64, draft: bool, prerelease: bool) -> ReleaseRecord {
        ReleaseRecord {
            id,
            published_at,
            draft,
            prerelease,
        }
    }

    #[lyquid_test::test]
    fn build_candidate_filters_deduplicates_sorts_and_counts_stable_releases() {
        let records = vec![
            release(12, 1_600, false, true),
            release(10, 1_000, false, false),
            release(11, 1_500, false, false),
            release(10, 1_000, false, false),
            release(99, 2_000, false, false),
        ];

        let candidate = build_candidate(&config(), 1, &records).unwrap();

        assert_eq!(
            candidate
                .releases
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            [10, 11, 12]
        );
        assert_eq!(candidate.stable_count, 2);
    }

    #[lyquid_test::test]
    fn build_candidate_rejects_conflicting_duplicate_ids() {
        let records = vec![
            release(10, 1_000, false, false),
            release(10, 1_001, false, false),
        ];

        let error = build_candidate(&config(), 1, &records).unwrap_err();

        assert_eq!(error.code, error_codes::E_DUPLICATE_CONFLICT);
    }

    #[lyquid_test::test]
    fn validate_candidate_rejects_a_count_that_does_not_match_its_records() {
        let mut candidate = build_candidate(
            &config(),
            1,
            &[
                release(10, 1_000, false, false),
                release(11, 1_500, false, true),
            ],
        )
        .unwrap();
        candidate.stable_count = 2;

        let error = validate_candidate(&config(), 1, &candidate).unwrap_err();

        assert_eq!(error.code, error_codes::E_CANDIDATE_MISMATCH);
    }

    #[lyquid_test::test]
    fn candidate_json_round_trip_is_compact_and_exact() {
        let candidate = build_candidate(&config(), 1, &[release(10, 1_000, false, false)]).unwrap();

        let encoded = candidate_to_json(&candidate).unwrap();

        assert!(!encoded.contains(char::is_whitespace));
        assert_eq!(candidate_from_json(&encoded).unwrap(), candidate);
    }

    #[lyquid_test::test]
    fn verify_observation_rejects_a_different_local_candidate() {
        let proposed = build_candidate(&config(), 1, &[release(10, 1_000, false, false)]).unwrap();
        let local = ObservationState {
            round_id: 1,
            ready: true,
            candidate: Some(
                build_candidate(&config(), 1, &[release(11, 1_500, false, false)]).unwrap(),
            ),
            error: None,
            page_count: 1,
            observed_at: 1_700,
        };

        let error = verify_observation(&config(), 1, &local, &proposed).unwrap_err();

        assert_eq!(error.code, error_codes::E_CANDIDATE_MISMATCH);
    }
}
