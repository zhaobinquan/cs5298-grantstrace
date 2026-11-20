#!/usr/bin/env bash
set +x
set -euo pipefail

# Requires Bash, Foundry Cast, Python 3, and a configured GrantsTrace deployment.
# Export GRANTS_ADDRESS (the deployed Lyquid contract's EVM address) and
# RPC1, RPC2, RPC3, RPC4 (the four hosting nodes' Ethereum JSON-RPC URLs).
# From the project root, run: bash scripts/observe.sh

for variable in GRANTS_ADDRESS RPC1 RPC2 RPC3 RPC4; do
    if [[ -z "${!variable:-}" ]]; then
        printf 'observe: set %s before running this script\n' "$variable" >&2
        exit 1
    fi
    export "$variable"
done

for command in cast python3; do
    if ! command -v "$command" >/dev/null; then
        printf 'observe: %s is required on PATH\n' "$command" >&2
        exit 1
    fi
done

python3 - <<'PY'
import json
import os
import re
import subprocess
import sys


class ObservationError(Exception):
    """A diagnostic built from fixed text and validated values."""


def require(condition, message):
    if not condition:
        raise ObservationError(message)


def load_json(value, message):
    try:
        return json.loads(value)
    except (ValueError, RecursionError):
        raise ObservationError(message) from None


def uint(value, minimum=0, maximum=2**64 - 1):
    return type(value) is int and minimum <= value <= maximum


def object_fields(value, *fields):
    require(isinstance(value, dict) and set(value) == set(fields),
            "invalid or missing fields: " + ", ".join(fields))
    return value


def decode_response(stdout):
    raw = stdout.strip()
    require(re.fullmatch(r"0x(?:[0-9a-fA-F]{2})+", raw), "missing or invalid ABI response")
    data = bytes.fromhex(raw[2:])
    require(len(data) >= 64 and int.from_bytes(data[:32], "big") == 32,
            "expected ABI encoding for one string")
    size = int.from_bytes(data[32:64], "big")
    require(len(data) == 64 + (size + 31) // 32 * 32 and not any(data[64 + size:]),
            "invalid ABI string size or padding")
    return load_json(data[64:64 + size], "invalid observation JSON")


def business_error(value):
    error = object_fields(value, "code", "message", "retryable")
    require(isinstance(error["code"], str) and bool(error["code"])
            and isinstance(error["message"], str) and type(error["retryable"]) is bool,
            "invalid business error")
    # Omit untrusted error text in full, including any URLs, credentials or headers.
    messages = {
        "E_NOT_CONFIGURED": "grant is not configured",
        "E_ROUND_MISMATCH": "observation is from a different round or no round has started",
        "E_NOT_READY": "observation is not ready",
        "E_BAD_CONFIG": "grant configuration is invalid for observation",
        "E_BAD_INPUT": "GitHub returned invalid observation data",
        "E_JSON": "observation data could not be decoded",
        "E_HTTP": "GitHub request failed",
        "E_RATE_LIMIT": "GitHub rate-limited the request",
        "E_TOO_LARGE": "observation exceeded a size limit",
        "E_INCOMPLETE": "GitHub release pagination was incomplete",
        "E_REPO_MISMATCH": "GitHub repository ID did not match the configuration",
        "E_DUPLICATE_CONFLICT": "duplicate releases contain conflicting data",
        "E_CANDIDATE_MISMATCH": "candidate did not match the expected observation",
        "E_NOT_IMPLEMENTED": "observation API is not implemented",
        "E_INTERNAL": "internal observation error",
    }
    code = error["code"]
    if code not in messages:
        return "unknown observation error (details withheld)"
    retry = " (retryable)" if error["retryable"] else ""
    return f"{code}: {messages[code]}{retry}"


def validate_response(payload):
    body = object_fields(payload, "ok", "data", "error")
    if body["ok"] is False:
        require(body["data"] is None, "failed response contains data")
        raise ObservationError(business_error(body["error"]))
    require(body["ok"] is True and body["error"] is None, "invalid success envelope")
    view = object_fields(body["data"], "node_id", "current_round_id", "observation")
    require(isinstance(view["node_id"], str) and re.fullmatch(r"Node-[a-z2-7]{56}", view["node_id"]),
            "missing or invalid node identity")
    obs = object_fields(view["observation"], "round_id", "ready", "candidate", "error",
                        "page_count", "observed_at")
    if obs["error"] is not None:
        raise ObservationError(business_error(obs["error"]))
    require(obs["ready"] is True, "observation is not ready")
    round_id = view["current_round_id"]
    require(uint(round_id, 1) and uint(obs["round_id"], 1) and obs["round_id"] == round_id,
            "observation round does not match the current round")
    require(uint(obs["page_count"], 1, 3), "page_count must be between 1 and 3")
    require(uint(obs["observed_at"], 1), "missing or invalid observation time")
    candidate = object_fields(obs["candidate"], "schema_version", "round_id", "config",
                              "releases", "stable_count")
    require(uint(candidate["schema_version"], 1, 1)
            and uint(candidate["round_id"], 1) and candidate["round_id"] == round_id,
            "candidate schema or round does not match")
    config = object_fields(candidate["config"], "grant_id", "repo_owner", "repo_name",
                           "repo_id", "window_start", "window_end", "min_stable_releases",
                           "beneficiary", "reward_credits", "rule_version")
    require(all(isinstance(config[key], str) and config[key]
                for key in ("grant_id", "repo_owner", "repo_name", "beneficiary"))
            and uint(config["repo_id"], 1) and uint(config["window_start"])
            and uint(config["window_end"]) and config["window_start"] < config["window_end"]
            and uint(config["min_stable_releases"], 1, 32)
            and uint(config["reward_credits"], 1, 1000000) and uint(config["rule_version"], 1, 1),
            "invalid candidate configuration")
    releases = candidate["releases"]
    require(isinstance(releases, list) and len(releases) <= 32, "invalid candidate release list")
    previous_id = stable_count = 0
    for item in releases:
        record = object_fields(item, "id", "published_at", "draft", "prerelease")
        require(uint(record["id"], previous_id + 1)
                and uint(record["published_at"], config["window_start"], config["window_end"] - 1)
                and type(record["draft"]) is bool and type(record["prerelease"]) is bool,
                "invalid or unordered release record")
        previous_id = record["id"]
        stable_count += not record["draft"] and not record["prerelease"]
    require(uint(candidate["stable_count"], 0, 32) and candidate["stable_count"] == stable_count,
            "stable_count does not match the release records")
    return view


def cast(*args):
    try:
        result = subprocess.run(["cast", *args], stdout=subprocess.PIPE,
                                stderr=subprocess.DEVNULL, timeout=90)
    except subprocess.TimeoutExpired:
        raise ObservationError("Cast timed out") from None
    except (OSError, ValueError):
        raise ObservationError("could not run Cast") from None
    require(result.returncode == 0, f"Cast failed (exit {result.returncode})")
    try:
        return result.stdout.decode("utf-8")
    except UnicodeError:
        raise ObservationError("Cast returned invalid UTF-8") from None


def main():
    address = os.environ["GRANTS_ADDRESS"]
    require(re.fullmatch(r"0x[0-9a-fA-F]{40}", address) and int(address, 16) != 0,
            "GRANTS_ADDRESS must be a nonzero 20-byte EVM address")
    nodes = {f"RPC{i}": os.environ[f"RPC{i}"] for i in range(1, 5)}
    require(len(set(nodes.values())) == 4, "RPC1 through RPC4 must be distinct endpoints")
    views = {}
    failed = False
    for method in ("refresh_observation", "get_observation"):
        selector = cast("sig", f"{method}()").strip()
        require(re.fullmatch(r"0x[0-9a-fA-F]{8}", selector), f"invalid selector for {method}")
        for label, rpc_url in nodes.items():
            try:
                call = json.dumps({"to": address, "data": selector})
                raw = load_json(cast("rpc", "eth_call", call, "latest", "--rpc-url", rpc_url),
                                "missing or invalid RPC JSON")
                require(isinstance(raw, str), "missing or invalid RPC result")
                view = validate_response(decode_response(raw))
                obs = view["observation"]
                candidate = obs["candidate"]
                print(f'{label} {method}: node={view["node_id"]} round={obs["round_id"]} '
                      f'pages={obs["page_count"]} releases={len(candidate["releases"])} '
                      f'stable={candidate["stable_count"]} observed_at={obs["observed_at"]}',
                      flush=True)
                if method == "refresh_observation":
                    views[label] = view
                elif label in views:
                    require(view == views[label], "queried observation differs from refresh")
            except ObservationError as error:
                print(f"observe: {label} {method}: {error}", file=sys.stderr)
                failed = True
    if failed:
        return 1
    require(len({view["node_id"] for view in views.values()}) == 4,
            "the four RPC endpoints did not return four distinct node identities")
    candidates = [view["observation"]["candidate"] for view in views.values()]
    for label, candidate in zip(list(nodes)[1:], candidates[1:]):
        require(candidate == candidates[0], f"{label} candidate differs from RPC1")
    print(f'All four nodes agree on the complete candidate for round {candidates[0]["round_id"]}.')
    return 0


try:
    sys.exit(main())
except ObservationError as error:
    sys.exit(f"observe: {error}")
PY
