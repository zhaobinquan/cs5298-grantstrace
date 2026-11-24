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
    // Pin the application's committee independently of LDK staging helpers.
    network committee_nodes: Vec<String> = Vec::new();
    network oracle releases;
    instance observation: ObservationState = ObservationState::default();
}

fn to_lyquid_error(error: AppError) -> LyquidError {
    LyquidError::LyquidRuntime(error.to_string().into())
}

fn committee_matches(nodes: &[String], config: &lyquid::runtime::oracle::OracleConfig) -> bool {
    let mut actual = config
        .committee
        .keys()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    actual.sort();
    nodes.len() == COMMITTEE_SIZE
        && nodes.windows(2).all(|pair| pair[0] < pair[1])
        && config.threshold == COMMITTEE_THRESHOLD
        && actual == nodes
}

// These files are expanded at crate root. Do not add repeated top-level imports in them.
include!("api/observations.rs");
include!("api/verification.rs");
include!("api/grants.rs");

#[cfg(test)]
mod committee_tests {
    use super::*;

    #[lyquid_test::test]
    fn committee_policy_rejects_replacement_and_threshold_changes() {
        let ids = (1..=4).map(NodeID::from).collect::<Vec<_>>();
        let mut pinned = ids.iter().map(ToString::to_string).collect::<Vec<_>>();
        pinned.sort();
        let target = OracleTarget {
            seq_id: [0_u8; 32].into(),
            target: OracleServiceTarget::LVM(LyquidID::from(1)),
        };
        let mut source = lyquid::runtime::oracle::OracleSrc::new("releases");
        assert!(source.initialize(target, ids.clone(), COMMITTEE_THRESHOLD));
        let (mut config, _) = source
            .source_state(target)
            .unwrap()
            .materialize_prefix(5)
            .unwrap();
        assert!(committee_matches(&pinned, &config));
        assert!(!committee_matches(&[], &config));
        config.threshold = 1;
        assert!(!committee_matches(&pinned, &config));
        config.threshold = COMMITTEE_THRESHOLD;
        config.committee.remove(&ids[0]);
        assert!(!committee_matches(&pinned, &config));
        // LDK permits replacement before activation. The independent pin must reject it.
        let replacement = (5..=8).map(NodeID::from).collect();
        assert!(source.initialize(target, replacement, COMMITTEE_THRESHOLD));
        let (replaced, _) = source
            .source_state(target)
            .unwrap()
            .materialize_prefix(5)
            .unwrap();
        assert!(!committee_matches(&pinned, &replaced));
    }
}
