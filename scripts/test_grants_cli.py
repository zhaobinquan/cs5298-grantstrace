#!/usr/bin/env python3
"""Offline script regression tests. No RPCs, transactions or signatures are real."""
import copy
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import grants_cli as cli

ADDR = "0x" + "1" * 40
CANDIDATE = json.loads((cli.ROOT / "fixtures/offline_candidate.json").read_text())


def state(snapshot=True):
    return {"admin": ADDR, "eligible": snapshot, "state": {
        "config": copy.deepcopy(CANDIDATE["config"]), "round_id": 1,
        "snapshot": {"candidate": copy.deepcopy(CANDIDATE), "signer_node_ids": ["a", "b", "c"]} if snapshot else None,
        "claimed": False, "credits": 0}}


class ScriptTests(unittest.TestCase):
    def setUp(self):
        self.env = patch.dict(os.environ, {"ADMIN_KEY": "synthetic", "BENEFICIARY_KEY": "synthetic"}, clear=True)
        self.env.start()
        self.addCleanup(self.env.stop)

    def test_envelopes_preserve_u64_and_reject_business_errors(self):
        raw = json.dumps(json.dumps({"ok": True, "data": {"id": 2**64 - 1}, "error": None}))
        self.assertEqual(cli.envelope(raw)["id"], 2**64 - 1)
        with self.assertRaisesRegex(cli.CliError, "E_NOT_READY"):
            cli.envelope(json.dumps({"ok": False, "data": None, "error": {"code": "E_NOT_READY", "message": "SECRET"}}))
        for raw in ('null', '{}', '"garbage"', '{"ok":1,"data":{},"error":null}'):
            with self.assertRaises(cli.CliError):
                cli.envelope(raw)

    def test_a_and_b_environment_aliases(self):
        with patch.dict(os.environ, {"GRANTS_ADDRESS": ADDR, "RPC1": "node1", "RPC2": "node2", "RPC3": "node3", "RPC4": "node4"}):
            self.assertEqual(cli.configured_address(), ADDR)
            self.assertEqual(cli.rpc_urls(), ["node1", "node2", "node3", "node4"])
            with patch.dict(os.environ, {"LYQUID_ADDRESS": "0x" + "2" * 40}):
                with self.assertRaises(cli.CliError):
                    cli.configured_address()

    def test_transport_errors_do_not_echo_secrets(self):
        with patch.object(cli.subprocess, "run", return_value=subprocess.CompletedProcess([], 1, "SECRET", "SECRET")):
            with self.assertRaises(cli.TransportError) as caught:
                cli.run(["cast", "--private-key", "SECRET"])
            self.assertNotIn("SECRET", str(caught.exception))

    def test_receipt_does_not_complete_claim(self):
        before = state()
        after = copy.deepcopy(before)
        after["state"].update(claimed=True, credits=100)
        with patch.object(cli, "status", side_effect=[before, before, after]) as reads, patch.object(cli, "wallet", return_value=ADDR), patch.object(cli, "send") as send, patch.object(cli.time, "sleep"):
            self.assertEqual(cli.claim(ADDR), after)
            self.assertEqual(reads.call_count, 3)
            send.assert_called_once()

    def test_wrong_claim_credits_fail(self):
        before = state()
        after = copy.deepcopy(before)
        after["state"].update(claimed=True, credits=101)
        with patch.object(cli, "status", side_effect=[before, after]), patch.object(cli, "wallet", return_value=ADDR), patch.object(cli, "send"):
            with self.assertRaisesRegex(cli.CliError, "unexpected state"):
                cli.claim(ADDR)

    def test_claim_preconditions_prevent_sending(self):
        variants = [state(False), state(), state()]
        variants[1]["state"]["claimed"] = True
        variants[2]["state"]["config"]["beneficiary"] = "0x" + "2" * 40
        for before in variants:
            with patch.object(cli, "status", return_value=before), patch.object(cli, "wallet", return_value=ADDR), patch.object(cli, "send") as send:
                with self.assertRaises(cli.CliError):
                    cli.claim(ADDR)
                send.assert_not_called()

    def test_round_requires_exact_increment(self):
        before = state(False)
        after = copy.deepcopy(before)
        after["state"]["round_id"] += 2
        with patch.object(cli, "status", side_effect=[before, after]), patch.object(cli, "wallet", return_value=ADDR), patch.object(cli, "view", return_value={"configured": True, "active": True, "threshold": 3, "node_ids": ["a", "b", "c", "d"]}), patch.object(cli, "send"):
            with self.assertRaisesRegex(cli.CliError, "unexpected state"):
                cli.start_round(ADDR)

    def test_timeout_uses_wall_clock(self):
        with patch.object(cli.time, "monotonic", side_effect=[0, 0, 0, 61, 61]), patch.object(cli.time, "sleep"):
            with self.assertRaisesRegex(cli.CliError, "unconfirmed after 60"):
                cli.poll("snapshot", lambda timeout: None)

    def test_certification_checks_full_candidate(self):
        after = state()
        after["state"]["snapshot"]["candidate"]["releases"][0]["id"] = 999
        submission = {"grant_id": CANDIDATE["config"]["grant_id"], "round_id": 1, "submitted": True}
        with patch.object(cli, "view", side_effect=[CANDIDATE, submission]), patch.object(cli, "status", return_value=after):
            with self.assertRaisesRegex(cli.CliError, "differs"):
                cli.certify(ADDR)

    def test_committee_waits_for_destination_before_finalize(self):
        ids = ["Node-" + char * 56 for char in "abcd"]
        staged = {"configured": True, "active": False, "threshold": 3, "node_ids": ids}
        active = dict(staged, active=True)
        events = []
        epochs = iter(["0\n0x00\n0\n0x", "0\n0x00\n0\n0x", "1\n0x00\n5\n0x"])
        def call(addr, sig, *args, **kwargs):
            if "dest_epoch" in sig:
                result = next(epochs)
                events.append("epoch" + result[0])
                return result
            events.append("finalize" if "finalize" in sig else "advance")
            return "true"
        with patch.dict(os.environ, {"NODE_IDS_JSON": json.dumps(ids), "LYQUID_ID": "synthetic"}), patch.object(cli, "run", return_value=ADDR), patch.object(cli, "view", side_effect=[staged]*8 + [active]*4), patch.object(cli, "call", side_effect=call), patch.object(cli.time, "sleep"):
            self.assertTrue(cli.committee(ADDR)["active"])
        self.assertEqual(events, ["epoch0", "advance", "epoch0", "epoch1", "finalize"])

    def test_false_epoch_result_is_not_ignored(self):
        ids = ["Node-" + char * 56 for char in "abcd"]
        staged = {"configured": True, "active": False, "threshold": 3, "node_ids": ids}
        with patch.dict(os.environ, {"NODE_IDS_JSON": json.dumps(ids), "LYQUID_ID": "synthetic"}), patch.object(cli, "run", return_value=ADDR), patch.object(cli, "view", return_value=staged), patch.object(cli, "call", side_effect=["0\n0x00\n0\n0x", "false"]) as calls:
            with self.assertRaisesRegex(cli.CliError, "advance not submitted"):
                cli.committee(ADDR)
            self.assertEqual(calls.call_count, 2)

    def test_configure_confirms_entire_state(self):
        before = state(False)
        before["state"].update(config=None, round_id=0)
        after = copy.deepcopy(before)
        after["state"]["config"] = CANDIDATE["config"]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "config.json"
            path.write_text(json.dumps(CANDIDATE["config"]))
            with patch.dict(os.environ, {"GRANT_CONFIG_FILE": str(path)}), patch.object(cli, "status", side_effect=[before, after]), patch.object(cli, "wallet", return_value=ADDR), patch.object(cli, "send") as send:
                self.assertEqual(cli.configure(ADDR), after)
                send.assert_called_once()

    def test_deploy_exports_ids_and_confirms_all_nodes(self):
        initial = state(False)
        initial["state"].update(config=None, round_id=0)
        deployment = {"contract": ADDR, "lyquid_id": "Lyquid-synthetic", "os_version": "0.8"}
        with tempfile.TemporaryDirectory() as directory:
            out = Path(directory) / "address"
            info = Path(directory) / "info.json"
            with patch.dict(os.environ, {"ALLOW_DEPLOY": "1", "DEPLOY_ADDRESS_FILE": str(out), "DEPLOY_INFO_FILE": str(info)}), patch.object(cli, "wallet", return_value=ADDR), patch.object(cli, "run", return_value=json.dumps(deployment)), patch.object(cli, "view", return_value=initial) as reads, patch.object(cli.sys, "stderr"):
                self.assertEqual(cli.deploy(), deployment)
                self.assertEqual(reads.call_count, 4)
                self.assertEqual(out.read_text().strip(), ADDR)
                self.assertEqual(json.loads(info.read_text()), deployment)

    def test_e2e_runs_unmodified_a_orchestrator_with_c_adapters(self):
        source = (cli.ROOT / "scripts/e2e.sh").read_text().split("python3 - <<'PY'\n", 1)[1].rsplit("\nPY", 1)[0]
        source = source.rsplit("\ntry:\n    main()", 1)[0]
        ns = {}
        exec(source, ns)
        current = state(False)
        current["state"].update(config=None, round_id=0)
        active = False
        seen = []
        def step(label, command, env):
            nonlocal active
            if "deployment" in label:
                Path(env["DEPLOY_ADDRESS_FILE"]).write_text(ADDR)
            elif "configuration" in label:
                current["state"]["config"] = copy.deepcopy(CANDIDATE["config"])
            elif "committee" in label:
                self.assertEqual(env["GRANTS_ADDRESS"], ADDR)
                active = True
            elif "start round" in label:
                current["state"]["round_id"] = 1
            elif "certification" in label:
                current["state"]["snapshot"] = {"candidate": copy.deepcopy(CANDIDATE), "signer_node_ids": ["a", "b", "c"]}
                current["eligible"] = True
            elif "claim" in label:
                current["state"].update(claimed=True, credits=100)
            seen.append(label)
        ns.update(run_step=step, rpc=lambda *a, **k: "0x1", deployed=lambda *a: True,
                  grant_status=lambda *a: copy.deepcopy(current),
                  committee_status=lambda *a: {"a", "b", "c", "d"} if active else None,
                  read_view=lambda *a: {"node_id": "a", "current_round_id": 1, "observation": {
                      "ready": True, "error": None, "round_id": 1, "candidate": CANDIDATE}}, log=lambda *a: None)
        env = {f"RPC{i}": f"http://node{i}" for i in range(1, 5)}
        env.update(ALLOW_DEPLOY="1")
        for role, script in [("DEPLOY", "deploy"), ("CONFIGURE", "configure"), ("START_ROUND", "start-round"), ("CERTIFY", "c-certify"), ("CLAIM", "claim"), ("COMMITTEE", "c-committee")]:
            env[role + "_SCRIPT"] = str(cli.ROOT / "scripts" / (script + ".sh"))
        with patch.dict(os.environ, env, clear=True):
            ns["main"]()
        self.assertIn("Member C claim", seen)

    def test_committee_resolves_lyquid_id_from_deployment(self):
        ids = ["Node-" + char * 56 for char in "abcd"]
        active = {"configured": True, "active": True, "threshold": 3, "node_ids": ids}
        listing = {"lyquids": [{"contract": ADDR, "lyquid_id": "Lyquid-resolved"}]}
        def command(args, timeout=15):
            if args[:2] == ["shaker", "list"]:
                return json.dumps(listing)
            self.assertEqual(args, ["shaker", "to-hex", "Lyquid-resolved"])
            return ADDR
        with patch.dict(os.environ, {"NODE_IDS_JSON": json.dumps(ids)}), patch.object(cli, "run", side_effect=command) as calls, patch.object(cli, "view", return_value=active):
            self.assertTrue(cli.committee(ADDR)["active"])
            self.assertEqual(calls.call_count, 2)


if __name__ == "__main__":
    unittest.main()
