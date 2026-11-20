#!/usr/bin/env bash
set -euo pipefail

cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.."
for command in bash cast python3; do
    if ! command -v "$command" >/dev/null; then
        printf 'e2e: %s is required on PATH\n' "$command" >&2
        exit 1
    fi
done

python3 - <<'PY'
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import time


def require(condition, message):
    if not condition:
        raise ValueError(message)


def log(message):
    print(f"e2e: {message}", flush=True)


def adapter(role, owner):
    name = f"{role}_SCRIPT"
    path = os.environ.get(name)
    require(path and Path(path).is_file(), f"{owner} dependency unavailable: set {name} to an existing script")
    try:
        args = json.loads(os.environ.get(f"{role}_ARGS_JSON", "[]"))
    except ValueError:
        raise ValueError(f"{role}_ARGS_JSON must be a JSON array of strings") from None
    require(isinstance(args, list) and all(isinstance(arg, str) and "\x00" not in arg for arg in args),
            f"{role}_ARGS_JSON must be a JSON array of strings")
    return ["bash", "--", path, *args]


def run_step(label, command, env):
    log(label)
    # Team scripts can log sensitive input. Keep their output out of this transcript.
    result = subprocess.run(command, env=env, stdin=subprocess.DEVNULL,
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    require(result.returncode == 0, f"{label} failed (exit {result.returncode}); check the script in its own environment")


def cast(args, label, timeout=15):
    try:
        result = subprocess.run(["cast", *args], capture_output=True, text=True, timeout=timeout)
    except subprocess.TimeoutExpired:
        raise ValueError(f"{label}: Cast timed out") from None
    require(result.returncode == 0, f"{label}: Cast failed (exit {result.returncode})")
    return result.stdout.strip()


def rpc(url, label, method, *args, timeout=15):
    try:
        return json.loads(cast(["rpc", method, *args, "--rpc-url", url], label, timeout))
    except json.JSONDecodeError:
        raise ValueError(f"{label}: missing or invalid RPC JSON") from None


def read_view(address, url, method, timeout=15):
    budget = time.monotonic() + timeout
    selector = cast(["sig", f"{method}()"], method, timeout)
    require(re.fullmatch(r"0x[0-9a-fA-F]{8}", selector), f"{method}: invalid selector")
    call = json.dumps({"to": address, "data": selector})
    remaining = budget - time.monotonic()
    require(remaining > 0, f"{method}: call timed out before RPC query")
    raw = rpc(url, method, "eth_call", call, "latest", timeout=remaining)
    require(isinstance(raw, str) and re.fullmatch(r"0x(?:[0-9a-fA-F]{2})+", raw),
            f"{method}: missing or invalid ABI result")
    data = bytes.fromhex(raw[2:])
    require(len(data) >= 64 and int.from_bytes(data[:32], "big") == 32,
            f"{method}: expected one ABI string")
    size = int.from_bytes(data[32:64], "big")
    require(len(data) == 64 + (size + 31) // 32 * 32 and not any(data[64 + size:]),
            f"{method}: invalid ABI string length or padding")
    try:
        envelope = json.loads(data[64:64 + size].decode("utf-8"))
    except ValueError:
        raise ValueError(f"{method}: invalid business JSON") from None
    require(isinstance(envelope, dict) and set(envelope) == {"ok", "data", "error"},
            f"{method}: invalid business response")
    if envelope["ok"] is False and isinstance(envelope["error"], dict):
        code = envelope["error"].get("code")
        code = code if isinstance(code, str) and re.fullmatch(r"E_[A-Z_]+", code) else "unknown error"
        raise ValueError(f"{method}: business error {code}")
    require(envelope["ok"] is True and envelope["error"] is None and isinstance(envelope["data"], dict),
            f"{method}: invalid success response")
    return envelope["data"]


def grant_status(address, url, timeout=15):
    status = read_view(address, url, "get_grant_status", timeout)
    state = status.get("state")
    require(isinstance(state, dict) and set(state) == {"config", "round_id", "snapshot", "claimed", "credits"}
            and type(state["round_id"]) is int and state["round_id"] >= 0
            and type(state["credits"]) is int and state["credits"] >= 0
            and type(state["claimed"]) is bool and type(status.get("eligible")) is bool
            and (state["config"] is None or isinstance(state["config"], dict))
            and (state["snapshot"] is None or isinstance(state["snapshot"], dict)),
            "get_grant_status: invalid grant state")
    return status


def poll(label, check):
    deadline = time.monotonic() + 60
    while (remaining := deadline - time.monotonic()) > 0:
        started = time.monotonic()
        result = check(remaining)
        if result is not None and time.monotonic() <= deadline:
            return result
        time.sleep(max(0, min(started + 1, deadline) - time.monotonic()))
    raise ValueError(f"{label} unconfirmed after 60 seconds; inspect state before retrying")


def committee_status(address, nodes, budget=15):
    deadline = time.monotonic() + budget
    views = []
    for url in nodes:
        remaining = deadline - time.monotonic()
        require(remaining > 0, "committee status query timed out")
        view = read_view(address, url, "get_committee_status", remaining)
        require(type(view.get("active")) is bool and type(view.get("configured")) is bool,
                "get_committee_status: invalid committee response")
        views.append(view)
    if not all(view["active"] for view in views):
        return None
    require(all(view == views[0] for view in views), "active committee differs across nodes")
    ids = views[0].get("node_ids")
    require(views[0]["configured"] and views[0].get("threshold") == 3
            and isinstance(ids, list) and len(ids) == 4
            and all(isinstance(node_id, str) and node_id for node_id in ids)
            and len(set(ids)) == 4, "active committee is not configured for three of four nodes")
    return set(ids)


def checked_address(address):
    require(re.fullmatch(r"0x[0-9a-fA-F]{40}", address) and int(address, 16) != 0,
            "GRANTS_ADDRESS must be a nonzero EVM address")
    return address


def deployed(address, url):
    code = rpc(url, "eth_getCode", "eth_getCode", address, "latest")
    require(isinstance(code, str) and re.fullmatch(r"0x(?:[0-9a-fA-F]{2})*", code),
            "eth_getCode: invalid deployment response")
    return code != "0x"


def main():
    env = os.environ.copy()
    nodes = [env.get(f"RPC{i}", "") for i in range(1, 5)]
    require(all(nodes) and len(set(nodes)) == 4, "set RPC1 through RPC4 to four distinct devnet RPC endpoints")
    chain_ids = [rpc(url, f"RPC{i} eth_chainId", "eth_chainId") for i, url in enumerate(nodes, 1)]
    require(all(isinstance(chain_id, str) and re.fullmatch(r"0x[0-9a-fA-F]+", chain_id)
                for chain_id in chain_ids) and len({int(chain_id, 16) for chain_id in chain_ids}) == 1,
            "node RPCs did not return a common chain ID")
    log("four devnet RPCs are reachable on the same chain")

    address = env.get("GRANTS_ADDRESS", "")
    existing = False
    initial = None
    if address:
        address = checked_address(address)
        code_present = [deployed(address, url) for url in nodes]
        require(all(code_present) or not any(code_present), "deployment is visible on only some nodes")
        existing = all(code_present)
        if existing:
            initial = grant_status(address, nodes[0])["state"]
            require(initial["snapshot"] is None and not initial["claimed"] and initial["credits"] == 0,
                    "deployment already has a snapshot or claim; use a fresh deployment")
            log("existing deployment and grant status verified")
    if not existing:
        require(env.get("ALLOW_DEPLOY") == "1",
                "no deployed application found; provide GRANTS_ADDRESS or explicitly set ALLOW_DEPLOY=1")
        deploy = adapter("DEPLOY", "Member C")

    configure = None
    if (initial is None or initial["config"] is None) and (existing or env.get("CONFIGURE_SCRIPT")):
        configure = adapter("CONFIGURE", "Member C")
    round_script = adapter("START_ROUND", "Member C")
    certify = adapter("CERTIFY", "Member B")
    claim = adapter("CLAIM", "Member C")
    negative = adapter("NEGATIVE", "team negative checks") if env.get("NEGATIVE_SCRIPT") else None
    committee = None
    if existing:
        committee = committee_status(address, nodes)
    setup_committee = adapter("COMMITTEE", "Member B") if committee is None else None
    require(Path("scripts/observe.sh").is_file(), "Member A dependency unavailable: scripts/observe.sh")

    if not existing:
        log("deployment explicitly enabled; invoking Member C's adapter for a fresh deployment")
        with tempfile.TemporaryDirectory(prefix="grantstrace-e2e-") as directory:
            output = Path(directory) / "address"
            deploy_env = dict(env, DEPLOY_ADDRESS_FILE=str(output))
            run_step("Member C deployment", deploy, deploy_env)
            require(output.is_file(), "deployment adapter did not write DEPLOY_ADDRESS_FILE")
            address = checked_address(output.read_text(encoding="utf-8").strip())
        require(all(deployed(address, url) for url in nodes), "new deployment is unavailable on one or more nodes")

    env["GRANTS_ADDRESS"] = address
    state = grant_status(address, nodes[0])["state"]
    require(state["snapshot"] is None and not state["claimed"] and state["credits"] == 0,
            "expected an unfinalized, unclaimed grant with zero credits")
    if state["config"] is None:
        if configure is None:
            log("waiting for deployment adapter to finish grant configuration")
            config_check = "grant configuration (supply Member C's CONFIGURE_SCRIPT if deployment does not configure it)"
        else:
            run_step("Member C grant configuration", configure, env)
            config_check = "grant configuration"
        def configured(remaining):
            result = grant_status(address, nodes[0], remaining)["state"]
            return result if result["config"] is not None else None
        state = poll(config_check, configured)
    config = state["config"]
    reward = config.get("reward_credits")
    require(type(reward) is int and 1 <= reward <= 1000000, "configured reward_credits is invalid")
    log("grant configuration confirmed")

    if setup_committee is not None:
        committee = committee_status(address, nodes)
        if committee is None:
            run_step("Member B committee setup and activation", setup_committee, env)
            committee = poll("committee activation", lambda remaining: committee_status(address, nodes, remaining))
    log("committee activation confirmed on all four nodes")

    previous = grant_status(address, nodes[0])["state"]
    require(previous["config"] == config and previous["snapshot"] is None
            and not previous["claimed"] and previous["credits"] == 0,
            "grant changed before starting a round")
    expected_round = previous["round_id"] + 1
    run_step("Member C start round", round_script, env)

    def current(remaining):
        status = grant_status(address, nodes[0], remaining)
        state = status["state"]
        require(state["config"] == config and state["round_id"] in (expected_round - 1, expected_round),
                "unexpected grant configuration or round change")
        return status

    def round_started(remaining):
        status = current(remaining)
        require(status["state"]["snapshot"] is None, "grant finalized before observation")
        return status if status["state"]["round_id"] == expected_round else None

    poll("new round", round_started)
    run_step("Member A four-node observation", ["bash", "scripts/observe.sh"], env)
    view = read_view(address, nodes[0], "get_observation")
    observation = view.get("observation")
    require(isinstance(observation, dict) and observation.get("ready") is True
            and "error" in observation and observation["error"] is None
            and view.get("node_id") in committee and view.get("current_round_id") == expected_round
            and observation.get("round_id") == expected_round, "observation no longer belongs to the new round")
    candidate = observation.get("candidate")
    require(isinstance(candidate, dict) and candidate.get("config") == config
            and candidate.get("round_id") == expected_round, "observed candidate does not match the configured round")
    run_step("Member B certification submission", certify, env)

    def certified(remaining):
        status = current(remaining)
        state = status["state"]
        snap = state["snapshot"]
        if snap is None:
            return None
        signers = snap.get("signer_node_ids")
        require(state["round_id"] == expected_round and snap.get("candidate") == candidate,
                "certified snapshot does not match the observed candidate")
        require(isinstance(signers, list) and 3 <= len(signers) <= 4
                and all(isinstance(signer, str) and signer in committee for signer in signers)
                and len(set(signers)) == len(signers), "certified snapshot has invalid committee signers")
        require(status["eligible"] and not state["claimed"] and state["credits"] == 0,
                "certified grant is ineligible or already claimed")
        return snap

    snapshot = poll("certified snapshot", certified)
    log("certified snapshot confirmed")
    run_step("Member C claim", claim, env)

    def claimed(remaining):
        state = current(remaining)["state"]
        require(state["round_id"] == expected_round and state["snapshot"] == snapshot,
                "certified round or snapshot changed during claim")
        require(state["credits"] == (reward if state["claimed"] else 0),
                "claim credits do not match configured reward")
        return state if state["claimed"] else None

    poll("claim", claimed)
    log("claim confirmed with the configured reward credits")
    if negative is not None:
        run_step("team negative checks (prepared environment)", negative, env)
    else:
        log("negative checks pending; set NEGATIVE_SCRIPT when the team prepares the environment")


try:
    main()
except ValueError as error:
    sys.exit(f"e2e: {error}")
except OSError:
    sys.exit("e2e: a required command or file could not be accessed")
PY
