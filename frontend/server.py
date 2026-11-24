#!/usr/bin/env python3
"""Loopback-only GrantsTrace demo server. Never use with real accounts or public RPCs."""
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import threading
import time
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
FRONTEND = ROOT / "frontend"
LAST_DEPLOY = FRONTEND / ".local-deploy.json"
PORT = int(os.environ.get("GRANTSTRACE_FRONTEND_PORT", "4173"))
RPCS = [f"http://127.0.0.1:{port}/api" for port in (10087, 11087, 12087, 13087)]
MNEMONIC = "test test test test test test test test test test test junk"
DEMO_MODE = os.environ.get("GRANTSTRACE_DEMO_KEYS") == "1"
ADDRESS = re.compile(r"^0x[0-9a-fA-F]{40}$")
TEST_STATE = {"running": False, "done": False, "exit_code": None, "lines": []}
TEST_LOCK = threading.Lock()


def run(command, timeout=60, env=None):
    result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, timeout=timeout, env=env)
    if result.returncode:
        tail = (result.stderr or result.stdout).strip().splitlines()[-1:]
        raise RuntimeError(tail[0] if tail else f"command exited {result.returncode}")
    return result.stdout.strip()


def demo_key(index):
    name = "ADMIN_KEY" if index == 0 else "BENEFICIARY_KEY"
    return os.environ.get(name) or run(["cast", "wallet", "private-key", MNEMONIC, str(index)])


def address_for(index):
    return run(["cast", "wallet", "address", "--private-key", demo_key(index)]).lower()


def require_devnet():
    payload = json.dumps({"jsonrpc": "2.0", "id": 1, "method": "eth_chainId", "params": []}).encode()
    request = urllib.request.Request(RPCS[0], data=payload, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(request, timeout=5) as response:
        chain = json.load(response).get("result")
    if chain != "0x7a69":
        raise ValueError("only the local Anvil chain 31337 is supported")


def node_ids():
    ids = []
    for rpc in RPCS:
        request = urllib.request.Request(rpc.removesuffix("/api") + "/lyquor.node.v1.NodeService/GetNodeInfo",
                                         data=b"{}", headers={"Content-Type": "application/json"})
        with urllib.request.urlopen(request, timeout=10) as response:
            ids.append(json.load(response)["nodeId"]["value"])
    return ids


def send_action(data):
    if not DEMO_MODE:
        raise ValueError("local demo signing is disabled")
    require_devnet()
    action = data.get("action")
    addr = data.get("address")
    if not isinstance(addr, str) or not ADDRESS.fullmatch(addr):
        raise ValueError("invalid contract address")
    one_string = {"configureGrant": "configure_grant(string)",
                  "configureCommittee": "configure_committee(string)"}
    no_args = {"startRound": "start_round()", "claim": "claim()"}
    if action in one_string:
        value = data.get("value")
        if not isinstance(value, str) or len(value) > 16_384:
            raise ValueError("invalid JSON argument")
        json.loads(value)
        signature, args = one_string[action], [value]
    elif action in no_args:
        signature, args = no_args[action], []
    elif action in ("advanceEpoch", "finalizeEpoch"):
        target = data.get("target")
        if not isinstance(target, str) or not ADDRESS.fullmatch(target):
            raise ValueError("invalid LVM target address")
        name = "advance" if action == "advanceEpoch" else "finalize"
        signature = f"__lyquor_oracle_{name}_epoch(string,address,bool)"
        args = ["releases", target, "false"]
    else:
        raise ValueError("action is not allowed")
    if action in ("advanceEpoch", "finalizeEpoch"):
        output = run(["cast", "call", "--rpc-url", RPCS[0], addr, signature + "(bool)", *args], timeout=45)
        if output.strip() != "true":
            raise RuntimeError(f"{action} was not submitted")
        if action == "advanceEpoch":
            for _ in range(60):
                info = run(["cast", "call", "--rpc-url", RPCS[0], addr,
                            "__lyquor_oracle_dest_epoch_info(string,bool)(uint64,bytes32,uint32,bytes)",
                            "releases", "false"], timeout=15)
                if int(info.splitlines()[0].split()[0]) > 0:
                    break
                time.sleep(1)
            else:
                raise RuntimeError("destination epoch was not confirmed")
        return {"submitted": True}
    index = 1 if action == "claim" else 0
    output = run(["cast", "send", "--rpc-url", RPCS[0], "--private-key", demo_key(index),
                  addr, signature, *args, "--json"], timeout=45)
    receipt = json.loads(output)
    return {"hash": receipt.get("transactionHash"), "receipt_status": receipt.get("status")}


def deploy():
    if not DEMO_MODE:
        raise ValueError("local demo deployment is disabled")
    require_devnet()
    output = run(["shaker", "deploy", "Cargo.toml", "--reference", "http://127.0.0.1:8000/lyquids",
                  "--output", "json"], timeout=180,
                 env={**os.environ, "NO_PROXY": "127.0.0.1,localhost", "no_proxy": "127.0.0.1,localhost"})
    result = json.loads(output.splitlines()[-1])
    result["target"] = run(["shaker", "to-hex", result["lyquid_id"]])
    LAST_DEPLOY.write_text(json.dumps(result))
    return result


def acceptance_worker(config, ids):
    code = 1
    try:
        with tempfile.TemporaryDirectory(prefix="grantstrace-ui-") as directory:
            path = Path(directory) / "config.json"
            path.write_text(json.dumps(config))
            env = {**os.environ, "ADMIN_KEY": demo_key(0), "BENEFICIARY_KEY": demo_key(1),
                   "GRANT_CONFIG_FILE": str(path), "NODE_IDS_JSON": json.dumps(ids),
                   "ALLOW_DEPLOY": "1", "DEVNET_NODE_CONTAINER": os.environ.get("DEVNET_NODE_CONTAINER", "multi-node1-1"),
                   "NO_PROXY": "127.0.0.1,localhost", "no_proxy": "127.0.0.1,localhost"}
            process = subprocess.Popen(["python3", "scripts/test_devnet.py"], cwd=ROOT, env=env,
                                       stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, bufsize=1)
            for line in process.stdout:
                with TEST_LOCK:
                    TEST_STATE["lines"].append(line.rstrip()[:1200])
                    TEST_STATE["lines"] = TEST_STATE["lines"][-200:]
            code = process.wait()
    except Exception as error:
        with TEST_LOCK:
            TEST_STATE["lines"].append(f"ERROR: {error}")
    finally:
        with TEST_LOCK:
            TEST_STATE.update(running=False, done=True, exit_code=code)


def start_acceptance(data):
    if not DEMO_MODE:
        raise ValueError("local acceptance is disabled")
    require_devnet()
    config = data.get("config")
    ids = data.get("node_ids")
    if not isinstance(config, dict) or not isinstance(ids, list) or len(ids) != 4 or len(set(ids)) != 4:
        raise ValueError("valid config and four Node IDs required")
    if config.get("beneficiary", "").lower() != address_for(1):
        raise ValueError("beneficiary must match the local demo beneficiary")
    with TEST_LOCK:
        if TEST_STATE["running"]:
            raise ValueError("acceptance is already running")
        TEST_STATE.update(running=True, done=False, exit_code=None, lines=[])
    threading.Thread(target=acceptance_worker, args=(config, ids), daemon=True).start()
    return {"started": True}


class Handler(SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=str(FRONTEND), **kwargs)

    def json_response(self, status, value):
        body = json.dumps(value, ensure_ascii=False).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Cache-Control", "no-store")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def check_origin(self):
        origin = self.headers.get("Origin")
        return not origin or origin in (f"http://127.0.0.1:{PORT}", f"http://localhost:{PORT}")

    def do_GET(self):
        if self.path == "/local/status":
            try:
                require_devnet()
                self.json_response(200, {"ok": True, "demo_signing": DEMO_MODE,
                                         "admin": address_for(0) if DEMO_MODE else None,
                                         "beneficiary": address_for(1) if DEMO_MODE else None,
                                         "node_ids": node_ids(),
                                         "last_deploy": json.loads(LAST_DEPLOY.read_text()) if LAST_DEPLOY.exists() else None})
            except Exception as error:
                self.json_response(503, {"ok": False, "error": str(error)})
        elif self.path == "/local/acceptance/status":
            with TEST_LOCK:
                self.json_response(200, {"ok": True, **TEST_STATE})
        else:
            super().do_GET()

    def do_POST(self):
        if not self.check_origin():
            return self.json_response(403, {"ok": False, "error": "origin not allowed"})
        if self.headers.get("Content-Type", "").split(";")[0] != "application/json":
            return self.json_response(415, {"ok": False, "error": "JSON required"})
        try:
            length = int(self.headers.get("Content-Length", "0"))
            if length <= 0 or length > 32_768:
                raise ValueError("request size invalid")
            data = json.loads(self.rfile.read(length))
            if self.path == "/local/send":
                result = send_action(data)
            elif self.path == "/local/deploy":
                result = deploy()
            elif self.path == "/local/acceptance/start":
                result = start_acceptance(data)
            else:
                return self.json_response(404, {"ok": False, "error": "unknown endpoint"})
            self.json_response(200, {"ok": True, "data": result})
        except Exception as error:
            self.json_response(400, {"ok": False, "error": str(error)})


if __name__ == "__main__":
    print(f"GrantsTrace local UI: http://127.0.0.1:{PORT}/ (demo signing: {DEMO_MODE})", flush=True)
    ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
