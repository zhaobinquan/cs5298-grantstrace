//! Shared contract v1. Maintained by C; A and B import these types unchanged.

use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u16 = 1;
pub const RULE_VERSION: u16 = 1;
pub const MAX_PAGES: u8 = 3;
pub const PAGE_SIZE: u32 = 100;
pub const MAX_WINDOW_RECORDS: usize = 32;
pub const MAX_CANDIDATE_BYTES: usize = 16_384;
pub const MAX_PAGE_BYTES: usize = 524_288;
pub const HTTP_TIMEOUT_MS: u64 = 10_000;
pub const CERT_TIMEOUT_MS: u64 = 30_000;
pub const COMMITTEE_SIZE: usize = 4;
pub const COMMITTEE_THRESHOLD: u16 = 3;

pub type AppResult<T> = Result<T, AppError>;

pub mod error_codes {
    pub const E_NOT_CONFIGURED: &str = "E_NOT_CONFIGURED";
    pub const E_ALREADY_CONFIGURED: &str = "E_ALREADY_CONFIGURED";
    pub const E_UNAUTHORIZED: &str = "E_UNAUTHORIZED";
    pub const E_BAD_CONFIG: &str = "E_BAD_CONFIG";
    pub const E_BAD_INPUT: &str = "E_BAD_INPUT";
    pub const E_JSON: &str = "E_JSON";
    pub const E_HTTP: &str = "E_HTTP";
    pub const E_RATE_LIMIT: &str = "E_RATE_LIMIT";
    pub const E_TOO_LARGE: &str = "E_TOO_LARGE";
    pub const E_INCOMPLETE: &str = "E_INCOMPLETE";
    pub const E_REPO_MISMATCH: &str = "E_REPO_MISMATCH";
    pub const E_DUPLICATE_CONFLICT: &str = "E_DUPLICATE_CONFLICT";
    pub const E_ROUND_MISMATCH: &str = "E_ROUND_MISMATCH";
    pub const E_NOT_READY: &str = "E_NOT_READY";
    pub const E_CANDIDATE_MISMATCH: &str = "E_CANDIDATE_MISMATCH";
    pub const E_FINALIZED: &str = "E_FINALIZED";
    pub const E_COMMITTEE_NOT_READY: &str = "E_COMMITTEE_NOT_READY";
    pub const E_CERT_REJECTED: &str = "E_CERT_REJECTED";
    pub const E_SUBMIT: &str = "E_SUBMIT";
    pub const E_NOT_ELIGIBLE: &str = "E_NOT_ELIGIBLE";
    pub const E_ALREADY_CLAIMED: &str = "E_ALREADY_CLAIMED";
    pub const E_NOT_IMPLEMENTED: &str = "E_NOT_IMPLEMENTED";
    pub const E_INTERNAL: &str = "E_INTERNAL";
}

// Using this deserializer requires the key to exist; its value may still be null.
fn required_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrantConfig {
    pub grant_id: String,
    pub repo_owner: String,
    pub repo_name: String,
    pub repo_id: u64,
    pub window_start: u64,
    pub window_end: u64,
    pub min_stable_releases: u32,
    pub beneficiary: String,
    pub reward_credits: u64,
    pub rule_version: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseRecord {
    pub id: u64,
    pub published_at: u64,
    pub draft: bool,
    pub prerelease: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub schema_version: u16,
    pub round_id: u64,
    pub config: GrantConfig,
    pub releases: Vec<ReleaseRecord>,
    pub stable_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FetchResult {
    pub releases: Vec<ReleaseRecord>,
    pub page_count: u8,
    pub observed_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ObservationState {
    pub round_id: u64,
    pub ready: bool,
    #[serde(deserialize_with = "required_option")]
    pub candidate: Option<Candidate>,
    #[serde(deserialize_with = "required_option")]
    pub error: Option<AppError>,
    pub page_count: u8,
    pub observed_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationView {
    pub node_id: String,
    pub current_round_id: u64,
    pub observation: ObservationState,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CertifiedSnapshot {
    pub candidate: Candidate,
    pub signer_node_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct GrantState {
    #[serde(deserialize_with = "required_option")]
    pub config: Option<GrantConfig>,
    pub round_id: u64,
    #[serde(deserialize_with = "required_option")]
    pub snapshot: Option<CertifiedSnapshot>,
    pub claimed: bool,
    pub credits: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrantStatus {
    pub admin: String,
    pub state: GrantState,
    pub eligible: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitteeConfig {
    pub node_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct CommitteeView {
    pub configured: bool,
    pub active: bool,
    pub threshold: u16,
    pub node_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CertificationSubmission {
    pub grant_id: String,
    pub round_id: u64,
    pub submitted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub struct ApiResponse<T> {
    pub ok: bool,
    #[serde(deserialize_with = "required_option")]
    pub data: Option<T>,
    #[serde(deserialize_with = "required_option")]
    pub error: Option<AppError>,
}

impl AppError {
    pub fn new(code: impl Into<String>, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable,
        }
    }

    pub fn not_implemented(function: &str) -> Self {
        Self::new(
            error_codes::E_NOT_IMPLEMENTED,
            format!("{function} is not implemented in the scaffold"),
            false,
        )
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

pub fn api_ok<T>(data: T) -> ApiResponse<T> {
    ApiResponse {
        ok: true,
        data: Some(data),
        error: None,
    }
}

pub fn api_error<T>(error: AppError) -> ApiResponse<T> {
    ApiResponse {
        ok: false,
        data: None,
        error: Some(error),
    }
}

pub fn api_to_json<T: Serialize>(response: &ApiResponse<T>) -> AppResult<String> {
    serde_json::to_string(response)
        .map_err(|error| AppError::new(error_codes::E_JSON, error.to_string(), false))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example_candidate() -> Candidate {
        serde_json::from_str(include_str!("../fixtures/offline_candidate.json")).unwrap()
    }

    #[lyquid_test::test]
    fn candidate_rejects_unknown_and_missing_fields() {
        let mut value = serde_json::to_value(example_candidate()).unwrap();
        value["config"]["unexpected"] = serde_json::json!(true);
        assert!(serde_json::from_value::<Candidate>(value).is_err());

        let mut value = serde_json::to_value(example_candidate()).unwrap();
        value["config"].as_object_mut().unwrap().remove("repo_id");
        assert!(serde_json::from_value::<Candidate>(value).is_err());
    }

    #[lyquid_test::test]
    fn null_is_allowed_but_missing_option_keys_are_rejected() {
        let state = GrantState::default();
        let mut value = serde_json::to_value(&state).unwrap();
        assert!(value["config"].is_null());
        assert_eq!(
            serde_json::from_value::<GrantState>(value.clone()).unwrap(),
            state
        );
        value.as_object_mut().unwrap().remove("config");
        assert!(serde_json::from_value::<GrantState>(value).is_err());
    }

    #[lyquid_test::test]
    fn response_envelopes_have_explicit_null_fields() {
        let success = api_to_json(&api_ok(7_u64)).unwrap();
        assert_eq!(success, r#"{"ok":true,"data":7,"error":null}"#);

        let failure = api_to_json(&api_error::<u64>(AppError::new(
            error_codes::E_NOT_READY,
            "not ready",
            true,
        )))
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&failure).unwrap();
        assert_eq!(value["ok"], false);
        assert!(value["data"].is_null());
        assert_eq!(value["error"]["code"], error_codes::E_NOT_READY);
        assert_eq!(value["error"]["retryable"], true);
        let decoded: ApiResponse<u64> = serde_json::from_str(&failure).unwrap();
        assert!(!decoded.ok);
        assert!(decoded.data.is_none());
    }

    #[lyquid_test::test]
    fn json_preserves_u64_ids_and_candidate_field_order() {
        let mut candidate = example_candidate();
        candidate.config.repo_id = u64::MAX;
        let encoded = serde_json::to_string(&candidate).unwrap();
        assert!(
            encoded.starts_with(
                r#"{"schema_version":1,"round_id":1,"config":{"grant_id":"grant-001","#
            )
        );
        assert!(encoded.contains("18446744073709551615"));
        assert_eq!(
            serde_json::from_str::<Candidate>(&encoded).unwrap(),
            candidate
        );
    }
}
