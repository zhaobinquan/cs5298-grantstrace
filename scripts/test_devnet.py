#!/usr/bin/env python3
"""Real local-devnet acceptance checks; creates new deployments, never resets nodes."""
import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import subprocess
import tempfile
from urllib.parse import urlparse

import grants_cli as cli


def rejected(addr, signature, private_key, expected_code, *args):
    """Require both the actual runtime error and unchanged business state."""
    before = cli.status(addr)
    since = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
    cli.send(addr, signature, private_key, *args)
    container = os.environ["DEVNET_NODE_CONTAINER"]
    def logged(timeout):
        result = subprocess.run(["docker", "logs", "--since", since, container],
                                capture_output=True, text=True, timeout=timeout)
        cli.require(result.returncode == 0, "cannot read local node logs")
        lines = (result.stdout + result.stderr).splitlines()
        return True if any(expected_code in line and os.environ["LYQUID_ID"] in line for line in lines) else None
    cli.poll(f"runtime rejection {expected_code}", logged)
    cli.require(cli.status(addr) == before, f"{expected_code}: rejected call changed state")
    print(f"PASS {signature}: {expected_code}; state unchanged", flush=True)


def refresh(addr, indices):
    candidates = []
    for index in indices:
        view = cli.view(addr, "refresh_observation", rpc=cli.rpc_urls()[index], timeout=45)
        observation = view["observation"]
        cli.require(observation["ready"] and observation["error"] is None, "observation not ready")
        candidates.append(observation["candidate"])
    cli.require(all(x == candidates[0] for x in candidates), "nodes produced different candidates")
    return candidates[0]


def business_error(addr, method, code):
    before = cli.status(addr)
    try:
        cli.view(addr, method, timeout=40)
    except cli.CliError as error:
        cli.require(str(error) == f"business error: {code}", f"expected {code}, got {error}")
    else:
        raise cli.CliError(f"{method} unexpectedly succeeded")
    cli.require(cli.status(addr) == before, "rejected instance call changed grant state")
    print(f"PASS {method}: {code}; state unchanged", flush=True)


def new_deployment():
    config_path = os.environ.pop("GRANT_CONFIG_FILE")
    try:
        result = cli.deploy()
    finally:
        os.environ["GRANT_CONFIG_FILE"] = config_path
    os.environ["GRANTS_ADDRESS"] = result["contract"]
    os.environ.pop("LYQUID_ADDRESS", None)
    os.environ["LYQUID_ID"] = result["lyquid_id"]
    # Confirm all four nodes host the same image and same Lyquid.
    digests = set()
    for rpc in cli.rpc_urls():
        endpoint = rpc.replace("http://", "ws://").replace("/api", "/ws")
        listing = cli.decode(cli.run(["shaker", "list", "--hosted-only", "-o", "json", "-e", endpoint]))
        matches = [x for x in listing["lyquids"] if x["lyquid_id"] == result["lyquid_id"]]
        cli.require(len(matches) == 1, "deployment is not hosted on all nodes")
        digests.add(matches[0]["image_digest"])
    cli.require(len(digests) == 1, "nodes host different images")
    print("HOSTING", result["contract"], result["lyquid_id"], next(iter(digests)), flush=True)
    return result["contract"]


def resolve_hosted_lyquid(addr):
    endpoint = cli.rpc_url().replace("http://", "ws://").replace("/api", "/ws")
    listing = cli.decode(cli.run(["shaker", "list", "--hosted-only", "-o", "json", "-e", endpoint]))
    matches = [x for x in listing["lyquids"] if x["contract"].lower() == addr.lower()]
    cli.require(len(matches) == 1, "resume deployment is not hosted on the selected node")
    os.environ["LYQUID_ID"] = matches[0]["lyquid_id"]


def finish_ineligible(addr):
    resolve_hosted_lyquid(addr)
    before = cli.status(addr)
    cli.require(before["state"]["snapshot"] is None and not before["state"]["claimed"],
                "resume requires an unfinalized, unclaimed deployment")
    candidate = refresh(addr, [0, 1, 2, 3])
    cli.require(candidate["stable_count"] < candidate["config"]["min_stable_releases"],
                "resume input does not describe an ineligible candidate")
    ineligible = cli.certify(addr)
    cli.require(not ineligible["eligible"], "expected certified but ineligible state")
    rejected(addr, "claim()", cli.key("BENEFICIARY_KEY"), "E_NOT_ELIGIBLE")
    print("INELIGIBLE", json.dumps(ineligible), flush=True)


def finish_qualified(addr, config):
    candidate = refresh(addr, [0, 1, 2])
    before = cli.status(addr)
    try:
        cli.send(addr, "accept_snapshot(string)", cli.key("ADMIN_KEY"), json.dumps(candidate, separators=(",", ":")))
    except cli.TransportError:
        pass  # Generated EVM ABI has no ordinary accept_snapshot method.
    else:
        raise cli.CliError("ordinary snapshot transaction unexpectedly accepted")
    cli.require(cli.status(addr) == before, "ordinary snapshot attempt changed state")
    print("PASS ordinary accept_snapshot unavailable; state unchanged", flush=True)
    certified = cli.certify(addr)
    cli.require(certified["eligible"], "choose a historical window meeting the configured threshold")
    cli.require(len(certified["state"]["snapshot"]["signer_node_ids"]) == 3, "expected three verified signers")
    rejected(addr, "claim()", cli.key("ADMIN_KEY"), "E_UNAUTHORIZED")
    claimed = cli.claim(addr)
    cli.require(claimed["state"]["credits"] == config["reward_credits"] and claimed["eligible"], "claim mismatch")
    rejected(addr, "claim()", cli.key("BENEFICIARY_KEY"), "E_ALREADY_CLAIMED")
    rejected(addr, "start_round()", cli.key("ADMIN_KEY"), "E_FINALIZED")
    rejected(addr, "configure_grant(string)", cli.key("ADMIN_KEY"), "E_FINALIZED", json.dumps(config))
    # B's unchanged public submitter may retry, but C rejects a second snapshot.
    print("QUALIFIED", json.dumps(claimed), flush=True)
    return candidate


def main(resume_qualified=None, resume_ineligible=None):
    cli.require(all(urlparse(url).hostname in ("localhost", "127.0.0.1", "::1") for url in cli.rpc_urls()),
                "this acceptance suite is for local devnet only")
    cli.require(os.environ.get("DEVNET_NODE_CONTAINER"), "set DEVNET_NODE_CONTAINER for runtime error evidence")
    admin, beneficiary = cli.key("ADMIN_KEY"), cli.key("BENEFICIARY_KEY")
    cli.require(cli.wallet(admin) != cli.wallet(beneficiary), "use distinct admin and beneficiary test accounts")
    config = cli.decode(Path(os.environ["GRANT_CONFIG_FILE"]).read_text())
    cli.require(config["beneficiary"] == cli.wallet(beneficiary), "config beneficiary/key mismatch")
    if resume_ineligible:
        finish_ineligible(cli.address(resume_ineligible))
        print("PASS resumed ineligible scenario", flush=True)
        return
    if resume_qualified:
        addr = cli.address(resume_qualified)
        resolve_hosted_lyquid(addr)
        before = cli.status(addr)["state"]
        cli.require(before["config"] == config and before["round_id"] == 2
                    and before["snapshot"] is None and not before["claimed"],
                    "resume requires the expected unfinalized second round")
    else:
        addr = new_deployment()
        rejected(addr, "start_round()", admin, "E_NOT_CONFIGURED")
        rejected(addr, "configure_grant(string)", beneficiary, "E_UNAUTHORIZED", json.dumps(config))
        invalid = dict(config, reward_credits=0)
        rejected(addr, "configure_grant(string)", admin, "E_BAD_CONFIG", json.dumps(invalid))
        cli.configure(addr)
        rejected(addr, "configure_grant(string)", admin, "E_ALREADY_CONFIGURED", json.dumps(config))
        rejected(addr, "start_round()", beneficiary, "E_UNAUTHORIZED")
        rejected(addr, "claim()", beneficiary, "E_NOT_ELIGIBLE")
        rejected(addr, "start_round()", admin, "E_COMMITTEE_NOT_READY")
        cli.committee(addr)
        rejected(addr, "configure_committee(string)", beneficiary, "E_UNAUTHORIZED",
                 json.dumps({"node_ids": cli.decode(os.environ["NODE_IDS_JSON"])}))
        rejected(addr, "configure_committee(string)", admin, "E_ALREADY_CONFIGURED",
                 json.dumps({"node_ids": cli.decode(os.environ["NODE_IDS_JSON"])}))
        cli.start_round(addr)
        refresh(addr, [0, 1])
        business_error(addr, "request_certification", "E_CERT_REJECTED")
        cli.start_round(addr)
        business_error(addr, "get_candidate", "E_NOT_READY")
    candidate = finish_qualified(addr, config)

    # A distinct immutable config tests certified but ineligible state.
    count = candidate["stable_count"]
    cli.require(count < 32, "choose a window with fewer than 32 stable releases")
    with tempfile.TemporaryDirectory(prefix="grantstrace-ineligible-") as directory:
        path = Path(directory) / "config.json"
        path.write_text(json.dumps(dict(config, min_stable_releases=count + 1)))
        os.environ["GRANT_CONFIG_FILE"] = str(path)
        addr = new_deployment()
        cli.configure(addr)
        cli.committee(addr)
        cli.start_round(addr)
        finish_ineligible(addr)
    print("PASS local devnet acceptance suite", flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--resume-qualified", metavar="ADDRESS", help="resume the expected unfinalized second round after rate-limit recovery")
    parser.add_argument("--resume-ineligible", metavar="ADDRESS", help="resume an existing unfinalized ineligible deployment after rate-limit recovery")
    args = parser.parse_args()
    main(args.resume_qualified, args.resume_ineligible)
