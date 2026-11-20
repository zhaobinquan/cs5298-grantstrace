# Member A Contribution — Data Observation

## Responsibilities and implemented files

I implemented GrantsTrace’s data observation layer. It retrieves and normalizes GitHub releases, maintains an independent cache on each hosting node, and supplies normalized records to Member B’s candidate builder. I also implemented the observation script and coordinated the end-to-end workflow through adapters. Member B owns committee activation and certification. Member C owns deployment, grant configuration, round advancement, certified state updates, and claims.

| File | Contribution |
| --- | --- |
| `src/observations.rs` | GitHub JSON parsing and HTTP retrieval, pagination validation, observation state transitions, and offline guest tests. |
| `src/api/observations.rs` | Instance entry points for refreshing and querying node-local observations. |
| `fixtures/offline_github_repo.json`, `fixtures/offline_github_releases.json`, and `fixtures/README.md` | Synthetic GitHub-shaped test inputs and fixture provenance. |
| `scripts/observe.sh` and `scripts/test_observe.py` | Four-endpoint observation checks, complete candidate comparison, safe diagnostics, and diagnostic regression tests. |
| `scripts/e2e.sh` | Integration orchestration with configurable adapters, business-state checks, and bounded polling. |
| `docs/contribution-a.md` | Contribution record, test evidence, and integration dependencies. |

## GitHub integration, parsing, and pagination

The observation layer uses Host HTTP from an Instance method in the pinned Lyquor LDK 0.8.4. It validates the owner and repository name, checks that the observation window has ended using the node clock, and verifies the numeric repository ID against GitHub metadata before requesting releases. Each request uses a GrantsTrace User-Agent, the GitHub JSON media type, API version `2022-11-28`, and a 10-second timeout.

Serde decodes JSON objects with typed required fields. Repository and release IDs must be positive unsigned 64-bit integers. A draft with an explicit null `published_at` is skipped. Absent fields, invalid types, null timestamps on non-drafts, invalid RFC 3339 dates, and instants before the Unix epoch cause errors. Valid timestamps, including timestamps with explicit offsets, are converted to UTC Unix seconds.

Each fetch requests up to three pages of 100 releases. It parses GitHub’s `Link` headers, including quoted commas, and validates the advertised continuation against the configured repository or its verified numeric route and the expected sequential page. It rejects malformed headers, unsupported continuations, and a third page that advertises a fourth. The 512 KiB body limit applies to metadata and release responses. The project’s error codes distinguish transport and HTTP failures, rate limits, malformed data, repository mismatches, and incomplete pagination. A failure on any page prevents the fetch from returning partial success.

## Node-local state and candidate integration

`refresh_observation` reads the configuration and round from Network state. It rejects an absent configuration or an unopened round and reuses a ready observation for the current round. On a fresh observation, it fetches releases and calls Member B’s `build_candidate` and `validate_candidate`. B’s logic filters the window, deduplicates and orders releases, and computes the stable-release count. The candidate binds the configuration and round to the normalized release records and count. Page count and observation time are stored separately as diagnostics.

A successful refresh stores the complete candidate and marks the Instance observation ready. A failed refresh stores the actual error with `ready=false` and no candidate, then returns the JSON error envelope through a successful outer runtime result. `get_observation` uses the runtime-provided node identity and masks stale caches with `E_ROUND_MISMATCH`. The pinned LDK source and compiled guest confirm the accessors and return types. Persistence across calls on a deployed node remains unverified.

## Tests and observed results

| Scope | Command or method | Actual result |
| --- | --- | --- |
| Formatting and compilation | `cargo fmt`, `cargo fmt --check`, `cargo check --locked`, `cargo check --locked --tests --offline`, `cargo build --locked`, and `cargo test --locked --offline --no-run` | All exited 0. Cargo printed the known upstream LDK template package-name diagnostic and completed successfully. The last command compiled the guest tests. |
| Offline guest tests | Local Node harness invoked with `node - target/wasm32-unknown-unknown/debug/deps/cs5298_grantstrace-32f010ade8f3bfb5.wasm`, with host imports stubbed. | Exit 0. **34 passed, 0 failed, 0 skipped**. All 31 existing tests and 3 new pagination tests passed. The total includes 24 Member A tests. |
| Offline script tests | `python scripts/test_observe.py -v`, `bash -n scripts/observe.sh`, `bash -n scripts/e2e.sh`, and `git diff --check` | All exited 0. The Python suite ran **5 tests** covering safe diagnostics and successful summaries. |
| Mock RPC integration | Ran `bash scripts/observe.sh` with Cast 1.7.1 against four local mock RPCs. The final diagnostic check used `bash -x scripts/observe.sh`. | Eight calls per scenario. Matching candidates exited 0. An injected RPC error exited 1. Neither final run printed the synthetic secret. Earlier mock runs also rejected same-count candidate disagreement, duplicate identities, missing responses, business errors, and invalid page counts. |
| Simulated orchestration | Exercised the Python body of `scripts/e2e.sh` with mock status responses and adapter calls. | Seven scenarios passed, covering existing and fresh deployments, a missing adapter, certification timeout, snapshot mismatch, observation failure, and incorrect claim credits. |
| Official guest runner | `shaker build Cargo.toml` and `shaker test Cargo.toml` | Shaker was unavailable in the accessible environment. Neither command ran. |
| Live GitHub API | No live GitHub API test was run for Member A. | Host egress, live headers, and timeout behavior remain unverified. |
| Live four-node integration | No four-node Lyquor devnet run was completed. | Independent host fetches, Instance persistence, certification, and claims remain unverified on live nodes. |

The guest tests cover typed JSON parsing, UTC conversion, repository ID binding, pagination and error classification, failure on a later page, candidate construction, same-round reuse, and stale observation handling. The fixtures and mock RPC responses are synthetic.

## Running the scripts and integration dependencies

The scripts require Bash, Foundry Cast, and Python 3. Export `RPC1` through `RPC4` as the four hosting nodes’ Ethereum JSON-RPC URLs and `GRANTS_ADDRESS` as the deployed Lyquid’s EVM address. Run `bash scripts/observe.sh` on a configured deployment with an open round. Run `bash scripts/e2e.sh` with the applicable adapters supplied.

The orchestrator polls business state at one-second intervals for up to 60 seconds per transition. It checks the certified snapshot against the complete observed candidate and verifies that claim credits equal the configured reward. A fresh deployment requires explicit opt-in. An existing deployment with a snapshot, a claim, or nonzero credits is rejected.

| Adapter | Owner and integration requirement |
| --- | --- |
| `DEPLOY_SCRIPT` with `ALLOW_DEPLOY=1` | C. Used when no deployment is found. The adapter writes the resulting address to `DEPLOY_ADDRESS_FILE`. |
| `CONFIGURE_SCRIPT` | C. Required for an unconfigured existing deployment. A fresh deployment adapter may handle configuration itself. |
| `COMMITTEE_SCRIPT` | B. Required when the four nodes have yet to report the same active committee. |
| `START_ROUND_SCRIPT` | C. Submits authorized round advancement. |
| `CERTIFY_SCRIPT` | B. Submits certification for the current candidate. |
| `CLAIM_SCRIPT` | C. Submits the beneficiary claim. |
| `NEGATIVE_SCRIPT` | Optional team adapter for agreed rejection tests in a prepared environment. |

Each adapter accepts an optional `<ROLE>_ARGS_JSON` string array. Actual paths and arguments remain pending coordination with Members B and C. The inspected checkout still contains placeholders for the required B/C write operations, so the live workflow has yet to be verified.

## Git provenance

- `e981c92fcb6466ebffe5b78e1f8c0a96acbe9815` — Initialize GrantsTrace scaffold with shared types and passing tests.
- `00c5a7faa50503bd5b541a138ec06102503c1267` — Implement candidate verification. This was the checked-out HEAD at documentation time and contains Member B’s candidate logic.

The Member A implementation is in the working tree and has no commit ID at documentation time. The reported results apply to that working-tree implementation.

## Technical limitations and AI assistance

Observation covers records visible through the public GitHub API at fetch time. Deleted releases and private or inaccessible records remain outside its scope. Pagination does not provide an atomic snapshot across requests, so concurrent changes may lead nodes to produce different candidates. Nodes with a shared outbound IP may share a rate-limit budget. Unsupported pagination and over-limit responses are rejected explicitly.

OpenAI Codex assisted with implementation, test design, source review, diagnostic hardening, and drafting this contribution record. The observed results above distinguish offline tests and mock integration from live verification. Author review and the team’s live integration testing remain pending.
