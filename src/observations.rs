//! Fetch and parse GitHub repository metadata and release pages.

use crate::model::*;
use crate::verification::{build_candidate, validate_candidate};
use lyquid::{
    http::{Header, Method, Request, RequestOptions, Response},
    prelude::lyquor_api,
};
use serde::{Deserialize, Deserializer, de::DeserializeOwned};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[derive(Deserialize)]
struct GitHubRepo {
    id: u64,
}

#[derive(Deserialize)]
struct GitHubRelease {
    id: u64,
    #[serde(deserialize_with = "required_nullable_timestamp")]
    published_at: Option<String>,
    draft: bool,
    prerelease: bool,
}

// Serde's derived struct decoder also accepts positional arrays. GitHub records are objects.
struct JsonObject<T>(T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for JsonObject<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(JsonObject(std::marker::PhantomData))
    }
}

impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for JsonObject<std::marker::PhantomData<T>> {
    type Value = JsonObject<T>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a JSON object")
    }

    fn visit_map<M: serde::de::MapAccess<'de>>(self, map: M) -> Result<Self::Value, M::Error> {
        T::deserialize(serde::de::value::MapAccessDeserializer::new(map)).map(JsonObject)
    }
}

// An explicit null is allowed here, but an absent key is a deserialization error.
fn required_nullable_timestamp<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(deserializer)
}

fn parse_json<T: DeserializeOwned>(body: &[u8]) -> AppResult<T> {
    serde_json::from_slice(body).map_err(|error| {
        let code = if error.is_data() {
            error_codes::E_BAD_INPUT
        } else {
            error_codes::E_JSON
        };
        AppError::new(code, error.to_string(), false)
    })
}

fn bad_input(message: impl Into<String>) -> AppError {
    AppError::new(error_codes::E_BAD_INPUT, message, false)
}

pub fn parse_repo_id(body: &[u8]) -> AppResult<u64> {
    let JsonObject(repo): JsonObject<GitHubRepo> = parse_json(body)?;
    if repo.id == 0 {
        return Err(bad_input("repository ID must be positive"));
    }
    Ok(repo.id)
}

pub fn parse_release_page(body: &[u8]) -> AppResult<Vec<ReleaseRecord>> {
    let releases: Vec<JsonObject<GitHubRelease>> = parse_json(body)?;
    normalize_releases(releases)
}

fn normalize_releases(releases: Vec<JsonObject<GitHubRelease>>) -> AppResult<Vec<ReleaseRecord>> {
    let mut records = Vec::with_capacity(releases.len());
    for JsonObject(release) in releases {
        if release.id == 0 {
            return Err(bad_input("release ID must be positive"));
        }
        let published_at = match release.published_at {
            Some(value) => value,
            None if release.draft => continue,
            None => return Err(bad_input("non-draft release has null published_at")),
        };
        let timestamp = OffsetDateTime::parse(&published_at, &Rfc3339)
            .map_err(|error| {
                bad_input(format!(
                    "invalid published_at for release {}: {error}",
                    release.id
                ))
            })?
            .unix_timestamp();
        let published_at = u64::try_from(timestamp)
            .map_err(|_| bad_input(format!("negative published_at for release {}", release.id)))?;
        records.push(ReleaseRecord {
            id: release.id,
            published_at,
            draft: release.draft,
            prerelease: release.prerelease,
        });
    }
    Ok(records)
}

fn not_configured() -> AppError {
    AppError::new(
        error_codes::E_NOT_CONFIGURED,
        "grant is not configured",
        false,
    )
}

fn round_mismatch(message: &str) -> AppError {
    AppError::new(error_codes::E_ROUND_MISMATCH, message, false)
}

pub(crate) fn is_ready_in_round(local: &ObservationState, round_id: u64) -> bool {
    round_id != 0 && local.round_id == round_id && local.ready
}

pub(crate) fn refresh_state(
    grant: &GrantState,
    local: &ObservationState,
    fetch: impl FnOnce(&GrantConfig) -> AppResult<FetchResult>,
) -> Option<ObservationState> {
    let round_id = grant.round_id;
    let mut next = ObservationState {
        round_id,
        ..ObservationState::default()
    };
    let result = (|| {
        let config = grant.config.as_ref().ok_or_else(not_configured)?;
        if round_id == 0 {
            return Err(round_mismatch("no observation round has started"));
        }
        if is_ready_in_round(local, round_id) {
            return Ok(None);
        }
        let fetched = fetch(config)?;
        next.page_count = fetched.page_count;
        next.observed_at = fetched.observed_at;
        let candidate = build_candidate(config, round_id, &fetched.releases)?;
        validate_candidate(config, round_id, &candidate)?;
        Ok(Some(candidate))
    })();
    match result {
        Ok(None) => return None,
        Ok(Some(candidate)) => {
            next.ready = true;
            next.candidate = Some(candidate);
        }
        Err(error) => next.error = Some(error),
    }
    Some(next)
}

pub(crate) fn observation_response(
    grant: &GrantState,
    local: &ObservationState,
    node_id: String,
) -> ApiResponse<ObservationView> {
    if grant.config.is_none() {
        return api_error(not_configured());
    }
    let current_round_id = grant.round_id;
    let observation = if local.round_id == current_round_id && current_round_id != 0 {
        local.clone()
    } else {
        let error = if current_round_id == 0 {
            round_mismatch("no observation round has started")
        } else {
            round_mismatch("local observation belongs to a different round")
        };
        ObservationState {
            round_id: local.round_id,
            error: Some(error),
            ..ObservationState::default()
        }
    };
    api_ok(ObservationView {
        node_id,
        current_round_id,
        observation,
    })
}

pub fn fetch_releases(config: &GrantConfig) -> AppResult<FetchResult> {
    let observed_at = lyquor_api::systime().map_err(|error| {
        AppError::new(
            error_codes::E_INTERNAL,
            format!("host clock: {error:?}"),
            false,
        )
    })? / 1_000;
    fetch_with(config, observed_at, |request, options| {
        lyquor_api::http_request(request, Some(options))
            .map_err(|error| http_error(format!("GitHub request failed: {error:?}")))
    })
}

fn fetch_with(
    config: &GrantConfig,
    observed_at: u64,
    mut send: impl FnMut(Request, RequestOptions) -> AppResult<Response>,
) -> AppResult<FetchResult> {
    let owner = &config.repo_owner;
    let name = &config.repo_name;
    if !(1..=100).contains(&owner.len())
        || !owner
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        || !(1..=100).contains(&name.len())
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        || matches!(name.as_str(), "." | "..")
    {
        return Err(AppError::new(
            error_codes::E_BAD_CONFIG,
            "invalid GitHub repository path",
            false,
        ));
    }
    if config.window_end > observed_at {
        return Err(AppError::new(
            error_codes::E_BAD_CONFIG,
            "the observation window has not ended",
            false,
        ));
    }

    let repo_url = format!("https://api.github.com/repos/{owner}/{name}");
    let metadata = get(&mut send, repo_url.clone())?;
    let id = parse_repo_id(&metadata.body)?;
    if id != config.repo_id {
        return Err(AppError::new(
            error_codes::E_REPO_MISMATCH,
            format!(
                "configured repository ID {} differs from GitHub ID {id}",
                config.repo_id
            ),
            false,
        ));
    }

    let mut releases = Vec::new();
    for page_count in 1..=MAX_PAGES {
        let url = format!("{repo_url}/releases?per_page={PAGE_SIZE}&page={page_count}");
        let response = get(&mut send, url)?;
        let next = next_page_url(&response.headers)?;
        if let Some(target) = next {
            validate_next_page(target, &repo_url, id, page_count + 1)?;
        }
        if next.is_some() && page_count == MAX_PAGES {
            return Err(AppError::new(
                error_codes::E_INCOMPLETE,
                "GitHub reports more release pages",
                false,
            ));
        }
        let page: Vec<JsonObject<GitHubRelease>> = parse_json(&response.body)?;
        if page.len() > PAGE_SIZE as usize {
            return Err(AppError::new(
                error_codes::E_TOO_LARGE,
                "GitHub release page exceeds the record limit",
                false,
            ));
        }
        releases.extend(normalize_releases(page)?);
        if next.is_none() {
            return Ok(FetchResult {
                releases,
                page_count,
                observed_at,
            });
        }
    }
    Err(AppError::new(
        error_codes::E_INTERNAL,
        "release pagination ended without a result",
        false,
    ))
}

fn http_error(message: impl Into<String>) -> AppError {
    AppError::new(error_codes::E_HTTP, message, true)
}

fn get(
    send: &mut impl FnMut(Request, RequestOptions) -> AppResult<Response>,
    url: String,
) -> AppResult<Response> {
    let headers = [
        ("User-Agent", "GrantsTrace"),
        ("Accept", "application/vnd.github+json"),
        ("X-GitHub-Api-Version", "2022-11-28"),
    ]
    .into_iter()
    .map(|(name, value)| Header {
        name: name.into(),
        value: value.as_bytes().to_vec(),
    })
    .collect();
    let response = send(
        Request {
            method: Method::Get,
            url,
            headers,
            body: None,
        },
        RequestOptions {
            timeout_ms: Some(HTTP_TIMEOUT_MS),
        },
    )?;
    if response.body.len() > MAX_PAGE_BYTES {
        return Err(AppError::new(
            error_codes::E_TOO_LARGE,
            "GitHub response exceeds the byte limit",
            false,
        ));
    }
    if response.status != 200 {
        let header = |name: &str| {
            response
                .headers
                .iter()
                .find(|header| header.name.eq_ignore_ascii_case(name))
                .and_then(|header| std::str::from_utf8(&header.value).ok())
                .map(str::trim)
        };
        let remaining = header("x-ratelimit-remaining");
        let retry_after = header("retry-after");
        let rate_limited = response.status == 429
            || (response.status == 403
                && (remaining == Some("0")
                    || retry_after.is_some()
                    || serde_json::from_slice::<serde_json::Value>(&response.body)
                        .ok()
                        .and_then(|body| {
                            body.get("message")
                                .and_then(|message| message.as_str())
                                .map(|message| message.to_ascii_lowercase().contains("rate limit"))
                        })
                        .unwrap_or(false)));
        if rate_limited {
            let message = format!(
                "GitHub rate limited the request (HTTP {}, Retry-After: {}, X-RateLimit-Reset: {})",
                response.status,
                retry_after.unwrap_or("unavailable"),
                header("x-ratelimit-reset").unwrap_or("unavailable")
            );
            return Err(AppError::new(error_codes::E_RATE_LIMIT, message, true));
        }
        return Err(http_error(format!(
            "unexpected GitHub HTTP status {}",
            response.status
        )));
    }
    Ok(response)
}

// GitHub may use the repository's numeric route in a Link. After validating it,
// fetch the equivalent sequential page using the configured route and page size.
fn validate_next_page(target: &str, repo_url: &str, repo_id: u64, expected: u8) -> AppResult<()> {
    let supported = target.split_once('?').is_some_and(|(route, query)| {
        if route != format!("{repo_url}/releases")
            && route != format!("https://api.github.com/repositories/{repo_id}/releases")
        {
            return false;
        }
        let mut page = None;
        let mut size = None;
        for parameter in query.split('&') {
            match parameter.split_once('=') {
                Some(("page", value)) if page.is_none() => page = Some(value),
                Some(("per_page", value)) if size.is_none() => size = Some(value),
                _ => return false,
            }
        }
        page == Some(expected.to_string().as_str())
            && size.is_none_or(|value| value == PAGE_SIZE.to_string())
    });
    if !supported {
        return Err(AppError::new(
            error_codes::E_INCOMPLETE,
            "GitHub advertised an unsupported release continuation URL or page",
            false,
        ));
    }
    Ok(())
}

fn malformed_link() -> AppError {
    http_error("invalid GitHub Link header")
}

fn link_token(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
}

fn link_parameter_value(input: &str) -> AppResult<(&str, &str)> {
    if let Some(quoted) = input.strip_prefix('"') {
        let mut escaped = false;
        for (i, byte) in quoted.bytes().enumerate() {
            if byte.is_ascii_control() && byte != b'\t' {
                return Err(malformed_link());
            }
            if !escaped && byte == b'"' {
                return Ok((&quoted[..i], &quoted[i + 1..]));
            }
            escaped = !escaped && byte == b'\\';
        }
        return Err(malformed_link());
    }
    let end = input.find([',', ';', ' ', '\t']).unwrap_or(input.len());
    let (value, rest) = input.split_at(end);
    if !link_token(value) {
        return Err(malformed_link());
    }
    Ok((value, rest))
}

fn next_page_url(headers: &[Header]) -> AppResult<Option<&str>> {
    let mut next = None;
    for header in headers
        .iter()
        .filter(|header| header.name.eq_ignore_ascii_case("link"))
    {
        let mut links = std::str::from_utf8(&header.value).map_err(|_| malformed_link())?;
        loop {
            let (target, mut rest) = links
                .trim_start()
                .strip_prefix('<')
                .and_then(|link| link.split_once('>'))
                .filter(|(target, _)| {
                    !target.is_empty()
                        && !target
                            .contains(|c: char| c.is_whitespace() || c.is_control() || c == '<')
                })
                .ok_or_else(malformed_link)?;
            let mut relation = None;
            while let Some(parameter) = rest.trim_start().strip_prefix(';') {
                let parameter = parameter.trim_start();
                let name_end = parameter
                    .find(['=', ';', ',', ' ', '\t'])
                    .unwrap_or(parameter.len());
                let (name, suffix) = parameter.split_at(name_end);
                if !link_token(name) {
                    return Err(malformed_link());
                }
                rest = suffix;
                let mut value = None;
                if let Some(input) = suffix.trim_start().strip_prefix('=') {
                    let (parsed, tail) = link_parameter_value(input.trim_start())?;
                    value = Some(parsed);
                    rest = tail;
                }
                if name.eq_ignore_ascii_case("rel") {
                    if relation.is_some() {
                        return Err(malformed_link());
                    }
                    relation = Some(
                        value
                            .filter(|v| {
                                !v.trim().is_empty() && v.split_ascii_whitespace().all(link_token)
                            })
                            .ok_or_else(malformed_link)?,
                    );
                }
            }
            let relation = relation.ok_or_else(malformed_link)?;
            if relation
                .split_ascii_whitespace()
                .any(|rel| rel.eq_ignore_ascii_case("next"))
                && next.replace(target).is_some()
            {
                return Err(malformed_link());
            }
            let rest = rest.trim_start();
            if rest.is_empty() {
                break;
            }
            links = rest.strip_prefix(',').ok_or_else(malformed_link)?;
        }
    }
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[lyquid_test::test]
    fn refresh_checks_configuration_and_round_before_fetching() {
        let mut grant = GrantState::default();
        for code in [error_codes::E_NOT_CONFIGURED, error_codes::E_ROUND_MISMATCH] {
            let state = refresh_state(&grant, &ObservationState::default(), |_| {
                panic!("unexpected fetch")
            })
            .unwrap();
            assert_eq!(state.error.unwrap().code, code);
            assert!(!state.ready);
            assert!(state.candidate.is_none());
            grant.config = Some(fetch_config());
        }
    }

    #[lyquid_test::test]
    fn refresh_builds_a_candidate_and_reuses_it_in_the_same_round() {
        let grant = GrantState {
            config: Some(fetch_config()),
            round_id: 1,
            ..GrantState::default()
        };
        let state = refresh_state(&grant, &ObservationState::default(), |_| {
            Ok(FetchResult {
                releases: parse_release_page(include_bytes!(
                    "../fixtures/offline_github_releases.json"
                ))
                .unwrap(),
                page_count: 1,
                observed_at: 2000,
            })
        })
        .unwrap();
        assert!(state.ready);
        assert_eq!(state.page_count, 1);
        assert_eq!(state.observed_at, 2000);
        assert_eq!(state.candidate.as_ref().unwrap().stable_count, 1);
        assert!(refresh_state(&grant, &state, |_| panic!("same round refetched")).is_none());
    }

    #[lyquid_test::test]
    fn query_masks_old_observation_and_refresh_keeps_the_actual_error() {
        let grant = GrantState {
            config: Some(fetch_config()),
            round_id: 2,
            ..GrantState::default()
        };
        let local = ObservationState {
            round_id: 1,
            ready: true,
            candidate: Some(build_candidate(grant.config.as_ref().unwrap(), 1, &[]).unwrap()),
            observed_at: 2000,
            ..ObservationState::default()
        };
        let stale = observation_response(&grant, &local, "offline-test-node".into())
            .data
            .unwrap();
        assert_eq!(stale.current_round_id, 2);
        assert!(!stale.observation.ready);
        assert!(stale.observation.candidate.is_none());
        assert_eq!(
            stale.observation.error.unwrap().code,
            error_codes::E_ROUND_MISMATCH
        );

        let expected = http_error("simulated timeout");
        let state = refresh_state(&grant, &local, |_| Err(expected.clone())).unwrap();
        assert_eq!(state.round_id, 2);
        assert!(!state.ready);
        assert!(state.candidate.is_none());
        let view = observation_response(&grant, &state, "offline-test-node".into())
            .data
            .unwrap();
        assert_eq!(view.observation.error, Some(expected));
    }

    fn fetch_config() -> GrantConfig {
        GrantConfig {
            grant_id: "grant-001".into(),
            repo_owner: "offline-owner".into(),
            repo_name: "offline-repo".into(),
            repo_id: u64::MAX,
            window_start: 1000,
            window_end: 2000,
            min_stable_releases: 2,
            beneficiary: "0x1111111111111111111111111111111111111111".into(),
            reward_credits: 100,
            rule_version: RULE_VERSION,
        }
    }

    fn response(status: u16, body: &[u8], headers: &[(&str, &str)]) -> Response {
        Response {
            status,
            body: body.to_vec(),
            headers: headers
                .iter()
                .map(|(name, value)| Header {
                    name: (*name).into(),
                    value: value.as_bytes().to_vec(),
                })
                .collect(),
        }
    }

    fn metadata() -> Response {
        response(
            200,
            include_bytes!("../fixtures/offline_github_repo.json"),
            &[],
        )
    }

    #[lyquid_test::test]
    fn fetch_follows_link_headers_and_preserves_unfiltered_records() {
        let mut replies = [
            metadata(),
            response(200, include_bytes!("../fixtures/offline_github_releases.json"), &[
                ("lInK", r#"<https://api.github.com/repositories/18446744073709551615/releases?page=2&per_page=100>; rel="next", <https://api.github.com/repositories/18446744073709551615/releases?page=2>; rel="last""#),
            ]),
            response(200, b"[]", &[("Link", r#"<https://api.github.com/repositories/123/releases?page=1>; rel="prev""#)]),
        ].into_iter();
        let repo = "https://api.github.com/repos/offline-owner/offline-repo";
        let urls = [
            repo.to_string(),
            format!("{repo}/releases?per_page=100&page=1"),
            format!("{repo}/releases?per_page=100&page=2"),
        ];
        let mut urls = urls.into_iter();
        let result = fetch_with(&fetch_config(), 2000, |request, options| {
            assert_eq!(request.url, urls.next().unwrap());
            assert_eq!(request.method, Method::Get);
            assert_eq!(options.timeout_ms, Some(HTTP_TIMEOUT_MS));
            for (name, value) in [
                ("User-Agent", "GrantsTrace"),
                ("Accept", "application/vnd.github+json"),
                ("X-GitHub-Api-Version", "2022-11-28"),
            ] {
                assert!(
                    request
                        .headers
                        .iter()
                        .any(|header| header.name == name && header.value == value.as_bytes())
                );
            }
            Ok(replies.next().unwrap())
        })
        .unwrap();
        assert!(replies.next().is_none());
        assert_eq!(result.page_count, 2);
        assert_eq!(result.observed_at, 2000);
        assert_eq!(
            result
                .releases
                .iter()
                .map(|record| record.id)
                .collect::<Vec<_>>(),
            [12, 10, 13]
        );
    }

    #[lyquid_test::test]
    fn fetch_rejects_invalid_paths_and_open_windows_before_http() {
        for (owner, name, now) in [
            ("owner/slash", "repo", 2000),
            ("owner", "..", 2000),
            ("owner", "repo?query", 2000),
            ("owner", "repo", 1999),
        ] {
            let mut config = fetch_config();
            config.repo_owner = owner.into();
            config.repo_name = name.into();
            let error =
                fetch_with(&config, now, |_, _| panic!("unexpected HTTP request")).unwrap_err();
            assert_eq!(error.code, error_codes::E_BAD_CONFIG);
            assert!(!error.retryable);
        }
    }

    #[lyquid_test::test]
    fn fetch_stops_on_mismatched_repository_id() {
        let mut replies = [response(200, br#"{"id":123}"#, &[])].into_iter();
        let error =
            fetch_with(&fetch_config(), 2000, |_, _| Ok(replies.next().unwrap())).unwrap_err();
        assert_eq!(error.code, error_codes::E_REPO_MISMATCH);
        assert!(!error.retryable);
    }

    #[lyquid_test::test]
    fn fetch_refuses_to_return_a_truncated_third_page() {
        let mut calls = 0;
        let error = fetch_with(&fetch_config(), 2000, |_, _| {
            calls += 1;
            if calls == 1 {
                return Ok(metadata());
            }
            let link = format!(
                r#"<https://api.github.com/repos/offline-owner/offline-repo/releases?page={calls}>; rel="next""#
            );
            Ok(response(200, b"[]", &[("Link", &link)]))
        })
        .unwrap_err();
        assert_eq!(calls, 4);
        assert_eq!(error.code, error_codes::E_INCOMPLETE);
        assert!(!error.retryable);
    }

    #[lyquid_test::test]
    fn fetch_propagates_later_page_failures_without_partial_results() {
        for (failure, code, retryable) in [
            (
                Ok(response(403, b"{}", &[("X-RateLimit-Remaining", "0")])),
                error_codes::E_RATE_LIMIT,
                true,
            ),
            (
                Ok(response(429, b"{}", &[])),
                error_codes::E_RATE_LIMIT,
                true,
            ),
            (Ok(response(403, b"{}", &[])), error_codes::E_HTTP, true),
            (
                Err(http_error("offline transport timeout")),
                error_codes::E_HTTP,
                true,
            ),
            (Ok(response(200, b"[", &[])), error_codes::E_JSON, false),
            (
                Ok(response(200, &vec![b' '; MAX_PAGE_BYTES + 1], &[])),
                error_codes::E_TOO_LARGE,
                false,
            ),
        ] {
            let mut replies = [
                Ok(metadata()),
                Ok(response(
                    200,
                    include_bytes!("../fixtures/offline_github_releases.json"),
                    &[(
                        "Link",
                        r#"<https://api.github.com/repos/offline-owner/offline-repo/releases?page=2>; rel="next""#,
                    )],
                )),
                failure,
            ]
            .into_iter();
            let error =
                fetch_with(&fetch_config(), 2000, |_, _| replies.next().unwrap()).unwrap_err();
            assert!(replies.next().is_none());
            assert_eq!(error.code, code);
            assert_eq!(error.retryable, retryable);
        }
    }

    #[lyquid_test::test]
    fn fetch_rejects_nonsequential_or_unsupported_continuation_without_requesting_it() {
        for target in [
            "https://api.github.com/repos/offline-owner/offline-repo/releases?page=3",
            "https://api.github.com/repos/offline-owner/offline-repo/releases?page=1",
            "https://api.github.com/repos/offline-owner/offline-repo/releases?page=2&page=3",
            "https://api.github.com/repos/offline-owner/offline-repo/releases?page=2&per_page=50",
            "https://api.github.com/repos/offline-owner/offline-repo/releases?page=2&cursor=abc",
            "https://api.github.com/repos/offline-owner/offline-repo/releases?page=2#fragment",
            "https://api.github.com/repositories/123/releases?page=2",
            "https://api.github.com@elsewhere.test/repos/offline-owner/offline-repo/releases?page=2",
        ] {
            let link = format!(r#"<{target}>; rel="next""#);
            let mut replies = [metadata(), response(200, b"[]", &[("Link", &link)])].into_iter();
            let error = fetch_with(&fetch_config(), 2000, |_, _| {
                Ok(replies
                    .next()
                    .expect("invalid continuation must not be requested"))
            })
            .unwrap_err();
            assert_eq!(error.code, error_codes::E_INCOMPLETE, "{target}");
            assert!(!error.retryable);
        }
    }

    #[lyquid_test::test]
    fn fetch_accepts_quoted_commas_and_escaped_quotes_in_link_parameters() {
        let mut replies = [
            metadata(),
            response(200, b"[]", &[("Link", r#"<https://api.github.com/repos/offline-owner/offline-repo/releases?per_page=100&page=2>; title="release, \"next\"; test"; rel="next", <https://api.github.com/repos/offline-owner/offline-repo/releases?page=3>; rel="last""#)]),
            response(200, b"[]", &[]),
        ].into_iter();
        let result = fetch_with(&fetch_config(), 2000, |_, _| Ok(replies.next().unwrap())).unwrap();
        assert_eq!(result.page_count, 2);
        assert!(replies.next().is_none());
    }

    #[lyquid_test::test]
    fn fetch_rejects_malformed_pagination_instead_of_finishing_early() {
        for header in [
            "",
            r#"<https://api.github.com/releases?page=2; rel="next""#,
            r#"<https://api.github.com/releases?page=2>; title="release, next; rel=next"#,
            r#"<https://api.github.com/releases?page=2>; rel"#,
            r#"<https://api.github.com/releases?page=2>; rel="next,""#,
            r#"<https://api.github.com/releases?page=2>; rel="next"; rel="last""#,
            r#"<https://api.github.com/releases?page=2>; rel="next","#,
            r#"<https://api.github.com/releases?page=2>; rel=next, <https://api.github.com/releases?page=3>; rel=next"#,
        ] {
            let mut replies = [metadata(), response(200, b"[]", &[("Link", header)])].into_iter();
            let error = fetch_with(&fetch_config(), 2000, |_, _| {
                Ok(replies.next().expect("malformed Link must not be followed"))
            })
            .unwrap_err();
            assert_eq!(error.code, error_codes::E_HTTP, "{header}");
            assert!(error.retryable);
        }
    }

    #[lyquid_test::test]
    fn fetch_enforces_metadata_byte_limit_and_raw_release_count() {
        let error = fetch_with(&fetch_config(), 2000, |_, _| {
            Ok(response(200, &vec![b' '; MAX_PAGE_BYTES + 1], &[]))
        })
        .unwrap_err();
        assert_eq!(error.code, error_codes::E_TOO_LARGE);

        let draft = serde_json::json!({"id":1,"published_at":null,"draft":true,"prerelease":false});
        let body = serde_json::to_vec(&vec![draft; PAGE_SIZE as usize + 1]).unwrap();
        let mut replies = [metadata(), response(200, &body, &[])].into_iter();
        let error =
            fetch_with(&fetch_config(), 2000, |_, _| Ok(replies.next().unwrap())).unwrap_err();
        assert_eq!(error.code, error_codes::E_TOO_LARGE);
    }

    #[lyquid_test::test]
    fn parses_repository_id_without_losing_integer_precision() {
        let body = include_bytes!("../fixtures/offline_github_repo.json");
        assert_eq!(parse_repo_id(body).unwrap(), u64::MAX);
    }

    #[lyquid_test::test]
    fn rejects_missing_or_invalid_repository_ids() {
        for body in [
            "{}",
            "[123]",
            r#"{"id":0}"#,
            r#"{"id":-1}"#,
            r#"{"id":"123"}"#,
            r#"{"id":1.5}"#,
            r#"{"id":null}"#,
            r#"{"id":18446744073709551616}"#,
        ] {
            assert_eq!(
                parse_repo_id(body.as_bytes()).unwrap_err().code,
                error_codes::E_BAD_INPUT,
                "{body}"
            );
        }
    }

    #[lyquid_test::test]
    fn parses_release_records_and_normalizes_offsets() {
        let body = include_bytes!("../fixtures/offline_github_releases.json");
        assert_eq!(
            parse_release_page(body).unwrap(),
            [
                ReleaseRecord {
                    id: 12,
                    published_at: 1600,
                    draft: false,
                    prerelease: true
                },
                ReleaseRecord {
                    id: 10,
                    published_at: 1000,
                    draft: false,
                    prerelease: false
                },
                ReleaseRecord {
                    id: 13,
                    published_at: 2000,
                    draft: true,
                    prerelease: false
                },
            ]
        );
    }

    #[lyquid_test::test]
    fn accepts_empty_release_page() {
        assert!(parse_release_page(b"[]").unwrap().is_empty());
    }

    #[lyquid_test::test]
    fn rejects_missing_release_fields() {
        let mut record = serde_json::json!({
            "id": 1, "published_at": null, "draft": true, "prerelease": false
        });
        for field in ["id", "published_at", "draft", "prerelease"] {
            let value = record.as_object_mut().unwrap().remove(field).unwrap();
            let body = serde_json::to_vec(&[&record]).unwrap();
            assert_eq!(
                parse_release_page(&body).unwrap_err().code,
                error_codes::E_BAD_INPUT,
                "{field}"
            );
            record[field] = value;
        }
    }

    #[lyquid_test::test]
    fn rejects_wrong_release_field_types() {
        for (field, value) in [
            ("id", serde_json::json!("1")),
            ("published_at", serde_json::json!(1000)),
            ("draft", serde_json::json!("false")),
            ("prerelease", serde_json::json!(0)),
        ] {
            let mut record = serde_json::json!({
                "id": 1, "published_at": "1970-01-01T00:00:00Z", "draft": false, "prerelease": false
            });
            record[field] = value;
            let body = serde_json::to_vec(&[record]).unwrap();
            assert_eq!(
                parse_release_page(&body).unwrap_err().code,
                error_codes::E_BAD_INPUT,
                "{field}"
            );
        }
    }

    #[lyquid_test::test]
    fn rejects_invalid_release_ids_even_for_skippable_drafts() {
        for id in ["0", "-1", "1.5", "null", "18446744073709551616"] {
            let body =
                format!(r#"[{{"id":{id},"published_at":null,"draft":true,"prerelease":false}}]"#);
            assert_eq!(
                parse_release_page(body.as_bytes()).unwrap_err().code,
                error_codes::E_BAD_INPUT,
                "{id}"
            );
        }
    }

    #[lyquid_test::test]
    fn skips_draft_with_null_timestamp() {
        let body = br#"[{"id":1,"published_at":null,"draft":true,"prerelease":false}]"#;
        assert!(parse_release_page(body).unwrap().is_empty());
    }

    #[lyquid_test::test]
    fn rejects_non_draft_with_null_timestamp() {
        let body = br#"[{"id":1,"published_at":null,"draft":false,"prerelease":false}]"#;
        assert_eq!(
            parse_release_page(body).unwrap_err().code,
            error_codes::E_BAD_INPUT
        );
    }

    #[lyquid_test::test]
    fn rejects_invalid_dates() {
        for date in ["not a date", "2026-02-30T00:00:00Z", "2026-01-01T00:00:00"] {
            let body =
                format!(r#"[{{"id":1,"published_at":"{date}","draft":false,"prerelease":false}}]"#);
            assert_eq!(
                parse_release_page(body.as_bytes()).unwrap_err().code,
                error_codes::E_BAD_INPUT,
                "{date}"
            );
        }
    }

    #[lyquid_test::test]
    fn rejects_pre_epoch_instants() {
        for date in ["1969-12-31T23:59:59.999Z", "1970-01-01T00:00:00+01:00"] {
            let body =
                format!(r#"[{{"id":1,"published_at":"{date}","draft":false,"prerelease":false}}]"#);
            assert_eq!(
                parse_release_page(body.as_bytes()).unwrap_err().code,
                error_codes::E_BAD_INPUT,
                "{date}"
            );
        }
    }

    #[lyquid_test::test]
    fn rejects_malformed_json() {
        assert_eq!(parse_repo_id(b"{").unwrap_err().code, error_codes::E_JSON);
        assert_eq!(
            parse_release_page(b"[").unwrap_err().code,
            error_codes::E_JSON
        );
    }
}
