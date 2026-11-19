//! Replace the explicit placeholders with the functions assigned in docs/interfaces.md.

use crate::model::*;

pub fn parse_repo_id(body: &[u8]) -> AppResult<u64> {
    let _ = (body,);
    Err(AppError::not_implemented("parse_repo_id"))
}

pub fn parse_release_page(body: &[u8]) -> AppResult<Vec<ReleaseRecord>> {
    let _ = (body,);
    Err(AppError::not_implemented("parse_release_page"))
}

pub fn fetch_releases(config: &GrantConfig) -> AppResult<FetchResult> {
    let _ = (config,);
    Err(AppError::not_implemented("fetch_releases"))
}
