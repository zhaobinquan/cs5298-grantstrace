"""Offline diagnostics tests for Member A's observe.sh."""

import contextlib
import io
import json
import os
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parent.parent
SCRIPT = (ROOT / "scripts/observe.sh").read_text(encoding="utf-8")
PYTHON = compile(SCRIPT.split("python3 - <<'PY'\n", 1)[1].rsplit("\nPY", 1)[0],
                 "scripts/observe.sh", "exec")
SECRETS = ("secret_user", "secret_password", "secret_token", "secret_bearer", "secret_cookie")
UNTRUSTED = (
    "https://secret_user:secret_password@rpc.example/?api_key=secret_token\n"
    "Authorization: Bearer secret_bearer\nCookie: session=secret_cookie"
)


def abi_reply(payload):
    body = json.dumps(payload).encode()
    raw = ((32).to_bytes(32, "big") + len(body).to_bytes(32, "big")
           + body + bytes(-len(body) % 32))
    return json.dumps("0x" + raw.hex()).encode()


class ObserveDiagnostics(unittest.TestCase):
    def run_observe(self, case):
        candidate = json.loads((ROOT / "fixtures/offline_candidate.json").read_text(encoding="utf-8"))
        business = {"code": "E_RATE_LIMIT", "message": UNTRUSTED, "retryable": True}
        env = {"GRANTS_ADDRESS": "0x" + "1" * 40,
               **{f"RPC{i}": f"{UNTRUSTED.splitlines()[0]}&node={i}" for i in range(1, 5)}}

        def fake_cast(args, **kwargs):
            self.assertEqual(kwargs["stderr"], subprocess.DEVNULL)
            if args[1] == "sig":
                return subprocess.CompletedProcess(args, 0, b"0x12345678")
            node = int(args[-1][-1])
            view = {"node_id": "Node-" + "a" * 55 + "2345"[node - 1], "current_round_id": 1,
                    "observation": {"round_id": 1, "ready": True, "candidate": candidate,
                                    "error": None, "page_count": 1, "observed_at": 2000}}
            envelope = {"ok": True, "data": view, "error": None}
            if node == 1:
                if case == "cast_error":
                    return subprocess.CompletedProcess(args, 1, UNTRUSTED.encode(), UNTRUSTED.encode())
                if case == "timeout":
                    raise subprocess.TimeoutExpired(args, 90, stderr=UNTRUSTED.encode())
                if case == "invalid_json":
                    return subprocess.CompletedProcess(args, 0, UNTRUSTED.encode())
                if case == "invalid_utf8":
                    return subprocess.CompletedProcess(args, 0, b"\xff" + UNTRUSTED.encode())
                if case == "business":
                    envelope = {"ok": False, "data": None, "error": business}
                if case == "observation":
                    view["observation"]["error"] = business
                if case == "unknown_code":
                    envelope = {"ok": False, "data": None, "error": dict(business, code=UNTRUSTED)}
                if case == "malicious_node":
                    view["node_id"] = UNTRUSTED
            return subprocess.CompletedProcess(args, 0, abi_reply(envelope))

        stdout, stderr = io.StringIO(), io.StringIO()
        with patch.dict(os.environ, env, clear=True), patch("subprocess.run", fake_cast), \
             contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr), \
             self.assertRaises(SystemExit) as result:
            exec(PYTHON, {})
        output = stdout.getvalue() + stderr.getvalue()
        for secret in SECRETS:
            self.assertNotIn(secret, output)
        return result.exception.code, output

    def test_success_preserves_node_summaries(self):
        code, output = self.run_observe("success")
        self.assertEqual(code, 0)
        self.assertIn("All four nodes agree", output)

    def test_cast_failures_and_timeouts_omit_untrusted_details(self):
        for case, message in (("cast_error", "Cast failed (exit 1)"), ("timeout", "Cast timed out")):
            with self.subTest(case=case):
                code, output = self.run_observe(case)
                self.assertEqual(code, 1)
                self.assertIn(message, output)

    def test_invalid_rpc_output_is_not_echoed(self):
        for case, message in (("invalid_json", "invalid RPC JSON"), ("invalid_utf8", "invalid UTF-8")):
            with self.subTest(case=case):
                code, output = self.run_observe(case)
                self.assertEqual(code, 1)
                self.assertIn(message, output)

    def test_known_business_codes_survive_without_the_raw_message(self):
        for case in ("business", "observation"):
            with self.subTest(case=case):
                code, output = self.run_observe(case)
                self.assertEqual(code, 1)
                self.assertIn("E_RATE_LIMIT: GitHub rate-limited the request (retryable)", output)

    def test_unknown_error_codes_and_invalid_node_ids_are_not_echoed(self):
        for case, message in (("unknown_code", "unknown observation error"),
                              ("malicious_node", "invalid node identity")):
            with self.subTest(case=case):
                code, output = self.run_observe(case)
                self.assertEqual(code, 1)
                self.assertIn(message, output)


if __name__ == "__main__":
    unittest.main()
