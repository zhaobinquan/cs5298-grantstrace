//! GrantsTrace scaffold: shared contract and fail-closed entry points.
//! Business modules are intentionally incomplete; see README.md.

use lyquid::prelude::*;
use model::*;

pub mod grants;
pub mod model;
pub mod observations;
pub mod verification;

state! {
    network admin: Address = Address::ZERO;
    network grant: GrantState = GrantState::default();
    network oracle releases;
    instance observation: ObservationState = ObservationState::default();
}

fn to_lyquid_error(error: AppError) -> LyquidError {
    LyquidError::LyquidRuntime(error.to_string().into())
}

fn unfinished_lyquid<T>(function: &str) -> LyquidResult<T> {
    Err(to_lyquid_error(AppError::not_implemented(function)))
}

fn unfinished_json<T: serde::Serialize>(function: &str) -> LyquidResult<String> {
    api_to_json(&api_error::<T>(AppError::not_implemented(function))).map_err(to_lyquid_error)
}

// These files are expanded at crate root. Do not add repeated top-level imports in them.
include!("api/observations.rs");
include!("api/verification.rs");
include!("api/grants.rs");
