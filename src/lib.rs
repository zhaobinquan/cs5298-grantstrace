//! GrantsTrace shared state and API entry points.

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

// These files are expanded at crate root. Do not add repeated top-level imports in them.
include!("api/observations.rs");
include!("api/verification.rs");
include!("api/grants.rs");
