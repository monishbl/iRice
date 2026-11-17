"""Quick Share's identity is made once and kept (17 Nov: the phone listed
the computer once per visibility toggle, a new endpoint id each time)."""
import asyncio
import json
import os
import stat
import sys
import tempfile
import unittest
from pathlib import Path

ENGINE = Path(__file__).resolve().parent.parent / "shell/assets/scripts/quickshare"
sys.path.insert(0, str(ENGINE))

from nearby import endpoint, identity  # noqa: E402

try:
    from nearby.service import QuickShareService  # noqa: E402
except ImportError:  # (needs python-cryptography / protobuf)
    QuickShareService = None


class Identity(unittest.TestCase):
    def test_made_once_then_reused(self):
        with tempfile.TemporaryDirectory() as d:
            f = os.path.join(d, "tessera/quickshare/identity.json")
            a, how = identity.load(f)
            self.assertEqual(how, "new")
            b, how = identity.load(f)
            self.assertEqual(how, "kept")
            self.assertEqual(a, b)
            self.assertEqual(len(a["endpoint_id"]), 4)
            # only you can read it
            self.assertEqual(stat.S_IMODE(os.stat(f).st_mode), 0o600)
            self.assertEqual(stat.S_IMODE(os.stat(os.path.dirname(f)).st_mode), 0o700)

    def test_a_damaged_file_is_made_again(self):
        with tempfile.TemporaryDirectory() as d:
            f = os.path.join(d, "identity.json")
            Path(f).write_text("{not json")
            a, how = identity.load(f)
            self.assertEqual(how, "new")
            Path(f).write_text(json.dumps({"endpoint_id": "ab", "salt": "00", "token": "0000"}))
            self.assertEqual(identity.load(f)[1], "new")
            self.assertEqual(identity.load(f)[1], "kept")

    def test_the_state_folder_follows_xdg(self):
        with tempfile.TemporaryDirectory() as d:
            old = os.environ.get("XDG_STATE_HOME")
            os.environ["XDG_STATE_HOME"] = d
            try:
                self.assertEqual(identity.path(), os.path.join(d, "tessera/quickshare/identity.json"))
            finally:
                if old is None:
                    del os.environ["XDG_STATE_HOME"]
                else:
                    os.environ["XDG_STATE_HOME"] = old

    def test_the_endpoint_info_is_the_same_each_time(self):
        me = identity.new()
        salt = bytes.fromhex(me["salt"])
        one = endpoint.endpoint_info("Monish-Work", salt=salt)
        self.assertEqual(one, endpoint.endpoint_info("Monish-Work", salt=salt))
        self.assertEqual(one[1:17], salt)
        self.assertEqual(endpoint.parse_endpoint_info(one)["name"], "Monish-Work")
        # (without a salt: random, as before)
        self.assertNotEqual(endpoint.endpoint_info("x")[1:17], endpoint.endpoint_info("x")[1:17])


@unittest.skipIf(QuickShareService is None, "needs python-cryptography and python-protobuf")
class ServiceKeepsIt(unittest.TestCase):
    def test_two_starts_advertise_one_device(self):
        with tempfile.TemporaryDirectory() as d:
            f = os.path.join(d, "identity.json")
            first = QuickShareService(lambda ev: None, None, name="Monish-Work", identity=identity.load(f)[0])
            again = QuickShareService(lambda ev: None, None, name="Monish-Work", identity=identity.load(f)[0])
            self.assertEqual(first.endpoint_id, again.endpoint_id)
            self.assertEqual(first.device_token, again.device_token)
            self.assertEqual(first._endpoint_info(), again._endpoint_info())
            self.assertEqual(endpoint.service_name(first.endpoint_id), endpoint.service_name(again.endpoint_id))
            # a rename keeps the id (the name is only in the info)
            asyncio.run(again.configure(name="Desk"))
            self.assertEqual(again.endpoint_id, first.endpoint_id)
            self.assertEqual(again._endpoint_info()[1:17], first._endpoint_info()[1:17])


if __name__ == "__main__":
    unittest.main()
