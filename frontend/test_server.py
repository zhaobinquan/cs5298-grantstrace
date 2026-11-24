import unittest
from unittest.mock import patch

import server


class LocalActionTests(unittest.TestCase):
    def test_oracle_epoch_uses_call_and_waits_for_destination(self):
        with patch.object(server, "DEMO_MODE", True), patch.object(server, "require_devnet"), \
             patch.object(server, "run", side_effect=["true", "1\n0x00\n0\n0x"] ) as run:
            result = server.send_action({"action": "advanceEpoch", "address": "0x" + "1" * 40,
                                         "target": "0x" + "2" * 40})
        self.assertEqual(result, {"submitted": True})
        self.assertEqual(run.call_args_list[0].args[0][1], "call")
        self.assertTrue(any("__lyquor_oracle_advance_epoch" in part for part in run.call_args_list[0].args[0]))
        self.assertIn("__lyquor_oracle_dest_epoch_info(string,bool)(uint64,bytes32,uint32,bytes)",
                      run.call_args_list[1].args[0])

    def test_unlisted_action_is_rejected(self):
        with patch.object(server, "DEMO_MODE", True), patch.object(server, "require_devnet"):
            with self.assertRaisesRegex(ValueError, "not allowed"):
                server.send_action({"action": "arbitraryMethod", "address": "0x" + "1" * 40})


if __name__ == "__main__":
    unittest.main()
