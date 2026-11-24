#!/usr/bin/env python3
"""CLI adapters for GrantsTrace. A transaction receipt is never business success."""

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parent.parent
U64_MAX = 2**64 - 1


class CliError(Exception):
    pass


class TransportError(CliError):
    pass


def require(ok, message):
    if not ok:
        raise CliError(message)


def run(args, timeout=15):
    try:
        result = subprocess.run(args, cwd=ROOT, capture_output=True, text=True,
                                timeout=timeout, stdin=subprocess.DEVNULL)
    except subprocess.TimeoutExpired:
        raise TransportError(f"{args[0]} timed out; inspect state before retrying") from None
    except OSError:
        raise CliError(f"{args[0]} is unavailable") from None
    # Do not echo arguments, RPC bodies or stderr: they can contain credentials.
    if result.returncode:
        raise TransportError(f"{args[0]} failed (exit {result.returncode}); inspect state before retrying")
    return result.stdout.strip()


def decode(raw):
    try:
        return json.loads(raw)
    except (ValueError, TypeError):
        raise CliError("invalid JSON response") from None


def envelope(raw):
    value = decode(raw)
    if isinstance(value, str):
        value = decode(value)
    require(isinstance(value, dict) and set(value) == {"ok", "data", "error"},
            "invalid business response")
    if value["ok"] is False:
        error = value["error"]
        code = error.get("code") if isinstance(error, dict) else None
        safe = code if isinstance(code, str) and re.fullmatch(r"E_[A-Z_]+", code) else "unknown error"
        raise CliError(f"business error: {safe}")
    require(value["ok"] is True and value["error"] is None and isinstance(value["data"], dict),
            "invalid success response")
    return value["data"]


def address(value):
    require(isinstance(value, str) and re.fullmatch(r"0x[0-9a-fA-F]{40}", value)
            and int(value, 16) != 0, "a nonzero EVM address is required")
    return value.lower()


def configured_address():
    a, b = os.environ.get("GRANTS_ADDRESS"), os.environ.get("LYQUID_ADDRESS")
    require(a or b, "set GRANTS_ADDRESS (or LYQUID_ADDRESS)")
    require(not (a and b) or address(a) == address(b), "GRANTS_ADDRESS and LYQUID_ADDRESS disagree")
    return address(a or b)


def rpc_url():
    a, b = os.environ.get("RPC1"), os.environ.get("RPC_URL")
    require(not (a and b) or a == b, "RPC1 and RPC_URL disagree")
    return a or b or "http://127.0.0.1:10087/api"


def rpc_urls():
    explicit = [os.environ.get(f"RPC{i}") for i in range(1, 5)]
    if any(explicit):
        require(all(explicit), "set all RPC1 through RPC4")
        nodes = explicit
        if os.environ.get("RPC_URLS"):
            require(nodes == os.environ["RPC_URLS"].split(), "RPC_URLS and RPC1..RPC4 disagree")
    else:
        nodes = os.environ.get("RPC_URLS", " ".join(
            f"http://127.0.0.1:{port}/api" for port in (10087, 11087, 12087, 13087))).split()
    require(len(nodes) == 4 and len(set(nodes)) == 4, "four distinct RPC endpoints are required")
    require(rpc_url() == nodes[0], "primary RPC must be the first of the four endpoints")
    return nodes


def call(addr, signature, *args, rpc=None, timeout=15):
    return run(["cast", "call", "--rpc-url", rpc or rpc_url(), addr, signature, *args], timeout)


def view(addr, method, rpc=None, timeout=15):
    return envelope(call(addr, f"{method}()(string)", rpc=rpc, timeout=timeout))


def status(addr, timeout=15):
    result = view(addr, "get_grant_status", timeout=timeout)
    state = result.get("state")
    require(isinstance(state, dict) and set(state) == {"config", "round_id", "snapshot", "claimed", "credits"},
            "invalid grant state")
    require(type(state["round_id"]) is int and 0 <= state["round_id"] <= U64_MAX
            and type(state["credits"]) is int and 0 <= state["credits"] <= U64_MAX
            and type(state["claimed"]) is bool and type(result.get("eligible")) is bool,
            "invalid grant state values")
    return result


def key(name):
    value = os.environ.get(name)
    require(value, f"set {name} to a local devnet account key")
    return value


def wallet(private_key):
    return address(run(["cast", "wallet", "address", "--private-key", private_key]))


def send(addr, signature, private_key, *args):
    receipt = decode(run(["cast", "send", "--json", "--rpc-url", rpc_url(), "--private-key",
                          private_key, addr, signature, *args], 60))
    require(isinstance(receipt, dict) and receipt.get("status") in ("0x1", 1),
            "transaction was not successful; inspect state before retrying")


def poll(label, check):
    deadline = time.monotonic() + 60
    while (remaining := deadline - time.monotonic()) > 0:
        started = time.monotonic()
        try:
            result = check(min(15, remaining))
            if result is not None and time.monotonic() <= deadline:
                return result
        except TransportError:
            pass  # Only reads are retried. No transaction is automatically resubmitted.
        time.sleep(max(0, min(deadline, started + 1) - time.monotonic()))
    raise CliError(f"{label} unconfirmed after 60 seconds; query shared state before retrying")


def configure(addr):
    config_path = os.environ.get("GRANT_CONFIG_FILE")
    require(config_path, "set GRANT_CONFIG_FILE to a GrantConfig JSON file")
    config = decode(Path(config_path).read_text())
    require(isinstance(config, dict), "GrantConfig must be a JSON object")
    before = status(addr)
    private_key = key("ADMIN_KEY")
    require(wallet(private_key) == address(before["admin"]), "E_UNAUTHORIZED: admin key does not match")
    require(before["state"]["config"] is None, "E_ALREADY_CONFIGURED: grant already configured")
    send(addr, "configure_grant(string)", private_key, json.dumps(config, separators=(",", ":")))
    def confirmed(timeout):
        after = status(addr, timeout)
        if after["state"]["config"] is None:
            return None
        expected = dict(before["state"], config=config)
        require(after["state"] == expected, "configuration produced unexpected state")
        return after
    return poll("grant configuration", confirmed)


def start_round(addr):
    before = status(addr)
    state = before["state"]
    private_key = key("ADMIN_KEY")
    require(wallet(private_key) == address(before["admin"]), "E_UNAUTHORIZED: admin key does not match")
    require(state["config"] is not None, "E_NOT_CONFIGURED")
    require(state["snapshot"] is None, "E_FINALIZED")
    require(state["round_id"] < U64_MAX, "E_INTERNAL: round overflow")
    committee = view(addr, "get_committee_status")
    require(committee.get("active") is True and committee.get("configured") is True
            and committee.get("threshold") == 3 and len(committee.get("node_ids", [])) == 4,
            "E_COMMITTEE_NOT_READY: activate the four-node committee before starting a round")
    expected = dict(state, round_id=state["round_id"] + 1)
    send(addr, "start_round()", private_key)
    def confirmed(timeout):
        after = status(addr, timeout)
        if after["state"] == state:
            return None
        require(after["state"] == expected, "unexpected state while opening round")
        return after
    return poll("start round", confirmed)


def claim(addr):
    before = status(addr)
    state = before["state"]
    config = state["config"]
    require(config is not None, "E_NOT_CONFIGURED")
    private_key = key("BENEFICIARY_KEY")
    require(wallet(private_key) == address(config["beneficiary"]), "E_UNAUTHORIZED: beneficiary key does not match")
    require(not state["claimed"], "E_ALREADY_CLAIMED")
    require(state["snapshot"] is not None and before["eligible"], "E_NOT_ELIGIBLE")
    credits = state["credits"] + config["reward_credits"]
    require(credits <= U64_MAX, "E_INTERNAL: credit overflow")
    expected = dict(state, claimed=True, credits=credits)
    send(addr, "claim()", private_key)
    def confirmed(timeout):
        after = status(addr, timeout)
        if after["state"] == state:
            return None
        require(after["state"] == expected and after["eligible"], "unexpected state while claiming")
        return after
    return poll("claim", confirmed)


def certify(addr):
    candidate = view(addr, "get_candidate")
    submission = view(addr, "request_certification", timeout=40)
    require(submission == {"grant_id": candidate["config"]["grant_id"],
                           "round_id": candidate["round_id"], "submitted": True},
            "certification submission does not match the candidate")
    def confirmed(timeout):
        after = status(addr, timeout)
        snapshot = after["state"]["snapshot"]
        if snapshot is None:
            return None
        require(snapshot["candidate"] == candidate, "certified snapshot differs from proposed candidate")
        return after
    return poll("certification", confirmed)


def committee(addr):
    nodes = rpc_urls()
    lyquid_id = os.environ.get("LYQUID_ID")
    if not lyquid_id:
        endpoint = rpc_url().replace("http://", "ws://").replace("/api", "/ws")
        listing = decode(run(["shaker", "list", "--hosted-only", "-o", "json", "-e", endpoint]))
        matches = [item["lyquid_id"] for item in listing.get("lyquids", [])
                   if isinstance(item, dict) and str(item.get("contract", "")).lower() == addr.lower()]
        require(len(matches) == 1, "cannot resolve the deployed Lyquid ID from its contract")
        lyquid_id = matches[0]
    target = address(run(["shaker", "to-hex", lyquid_id]))
    expected = decode(os.environ.get("NODE_IDS_JSON", "null"))
    require(isinstance(expected, list) and len(expected) == 4
            and all(isinstance(x, str) and re.fullmatch(r"Node-[a-z2-7]{56}", x) for x in expected)
            and len(set(expected)) == 4, "NODE_IDS_JSON must contain four distinct canonical Node IDs")
    def states(timeout):
        return [view(addr, "get_committee_status", rpc=node, timeout=timeout / 4) for node in nodes]
    def matches(item):
        return item["configured"] and item["threshold"] == 3 and sorted(item["node_ids"]) == sorted(expected)
    initial = states(15)
    if not all(matches(item) for item in initial):
        require(all(item["threshold"] == 0 for item in initial), "committee differs or staging is still propagating; inspect before retrying")
        private_key = key("ADMIN_KEY")
        require(wallet(private_key) == address(status(addr)["admin"]), "E_UNAUTHORIZED: admin key does not match")
        send(addr, "configure_committee(string)", private_key, json.dumps({"node_ids": expected}))
        poll("committee staging", lambda timeout: True if all(matches(x) for x in states(timeout)) else None)
    if all(item["active"] and matches(item) for item in states(15)):
        return {"active": True, "node_ids": sorted(expected)}
    def epoch(timeout):
        raw = call(addr, "__lyquor_oracle_dest_epoch_info(string,bool)(uint64,bytes32,uint32,bytes)",
                   "releases", "false", timeout=timeout)
        try:
            return int(raw.splitlines()[0].split()[0])
        except (ValueError, IndexError):
            raise CliError("invalid destination epoch response") from None
    if epoch(15) == 0:
        require(call(addr, "__lyquor_oracle_advance_epoch(string,address,bool)(bool)",
                     "releases", target, "false", timeout=40) == "true", "epoch advance not submitted; inspect state before retrying")
        poll("destination epoch", lambda timeout: True if epoch(timeout) > 0 else None)
    require(call(addr, "__lyquor_oracle_finalize_epoch(string,address,bool)(bool)",
                 "releases", target, "false", timeout=40) == "true", "epoch finalize not submitted; inspect state before retrying")
    poll("committee activation", lambda timeout: True if all(matches(x) and x["active"] for x in states(timeout)) else None)
    return {"active": True, "node_ids": sorted(expected)}


def deploy():
    require(os.environ.get("ALLOW_DEPLOY") == "1", "set ALLOW_DEPLOY=1 to create a fresh deployment")
    private_key = key("ADMIN_KEY")
    admin = wallet(private_key)
    nodes = rpc_urls()
    result = decode(run(["shaker", "deploy", "Cargo.toml", "--output", "json",
                         "--reference", os.environ.get("LYQUID_REFERENCE", "http://127.0.0.1:8000/lyquids"),
                         "--endpoint", os.environ.get("LYQUOR_ENDPOINT", "ws://127.0.0.1:10087/ws"),
                         "--private-key", private_key], 180))
    require(isinstance(result, dict) and isinstance(result.get("lyquid_id"), str)
            and result["lyquid_id"].startswith("Lyquid-"), "invalid shaker deployment result; inspect before retrying")
    addr = address(result.get("contract"))
    # Save actual deployment identifiers before waiting, so a timeout is recoverable.
    if os.environ.get("DEPLOY_ADDRESS_FILE"):
        Path(os.environ["DEPLOY_ADDRESS_FILE"]).write_text(addr + "\n")
    if os.environ.get("DEPLOY_INFO_FILE"):
        Path(os.environ["DEPLOY_INFO_FILE"]).write_text(json.dumps(result) + "\n")
    def hosted(timeout):
        views = [view(addr, "get_grant_status", rpc=node, timeout=timeout / 4) for node in nodes]
        initial = {"config": None, "round_id": 0, "snapshot": None, "claimed": False, "credits": 0}
        require(all(address(x["admin"]) == admin and x["state"] == initial and x["eligible"] is False for x in views),
                "deployed constructor state does not match the requested admin")
        return True
    poll("four-node hosting and constructor", hosted)
    if os.environ.get("GRANT_CONFIG_FILE"):
        configure(addr)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["deploy", "configure", "start-round", "claim", "committee", "certify"])
    args = parser.parse_args()
    commands = {"configure": configure, "start-round": start_round, "claim": claim,
                "committee": committee, "certify": certify}
    result = deploy() if args.action == "deploy" else commands[args.action](configured_address())
    print(json.dumps(result, separators=(",", ":")))


if __name__ == "__main__":
    try:
        main()
    except (CliError, OSError) as error:
        sys.exit(f"grantstrace: {error}")
    except (KeyError, TypeError, ValueError):
        sys.exit("grantstrace: malformed configuration or RPC data")
