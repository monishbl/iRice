"""Joining enterprise and hidden Wi-Fi (shell/assets/scripts/wifi-enterprise.py).

A fake `nmcli` on PATH stands in for NetworkManager: it records every call's
arguments and what it was given on stdin, keeps "profiles" in a JSON file,
and can be told to fail. The point that matters most: passwords reach nmcli
only on stdin, never on a command line, and nothing is left in a file.
"""
import json
import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parents[1] / "shell/assets/scripts/wifi-enterprise.py"

FAKE_NMCLI = textwrap.dedent(r'''
    #!/usr/bin/env python3
    import json, os, sys
    state = os.environ["FAKE_NM_STATE"]
    db = json.load(open(state)) if os.path.exists(state) else {"profiles": {}, "calls": []}
    args = sys.argv[1:]
    stdin = "" if sys.stdin.isatty() else sys.stdin.read()
    db["calls"].append({"argv": args, "stdin": stdin})
    out, code = "", 0
    if args[:4] == ["-t", "-f", "NAME,UUID,TYPE", "connection"]:
        out = "".join(f"{p['name']}:{u}:802-11-wireless\n" for u, p in db["profiles"].items())
    elif args[:2] == ["-g", "802-11-wireless.ssid"]:
        out = db["profiles"].get(args[-1], {}).get("ssid", "") + "\n"
    elif args[:2] == ["-t", "-f"] and args[3:5] == ["connection", "show"]:
        p = db["profiles"].get(args[-1], {})
        out = "".join(f"{k}:{v}\n" for k, v in p.get("settings", {}).items())
    elif args[:2] == ["connection", "edit"]:
        if os.environ.get("FAKE_NM_REFUSE_SAVE"):
            out = "Error: failed to save"
        else:
            uuid = args[3] if args[2] == "uuid" else "uuid-%d" % (len(db["profiles"]) + 1)
            p = db["profiles"].setdefault(uuid, {"name": args[-1] if args[2] == "type" else uuid, "settings": {}})
            for line in stdin.splitlines():
                if line.startswith("set "):
                    key, _, value = line[4:].partition(" ")
                    p["settings"][key] = value
                    if key == "802-11-wireless.ssid":
                        p["ssid"] = value
    elif args[:3] == ["connection", "delete", "uuid"]:
        if os.environ.get("FAKE_NM_REFUSE_DELETE"):
            print("Error: not authorized", file=sys.stderr)
            code = 1
        else:
            db["profiles"].pop(args[3], None)
    elif "up" in args:
        err = os.environ.get("FAKE_NM_UP_ERROR")
        if err:
            print(err, file=sys.stderr)
            code = 4
    json.dump(db, open(state, "w"))
    sys.stdout.write(out)
    sys.exit(code)
''').lstrip()


class WifiEnterprise(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="irice-wifi-")
        self.addCleanup(self.tmp.cleanup)
        root = Path(self.tmp.name)
        (root / "bin").mkdir()
        nm = root / "bin" / "nmcli"
        nm.write_text(FAKE_NMCLI)
        nm.chmod(0o755)
        self.state = root / "nm.json"
        self.runtime = root / "run"
        self.runtime.mkdir(mode=0o700)
        self.ca = root / "ca.pem"
        self.ca.write_text("-----BEGIN CERTIFICATE-----\n")
        self.env = dict(os.environ, PATH=f"{root / 'bin'}:{os.environ['PATH']}", FAKE_NM_STATE=str(self.state), XDG_RUNTIME_DIR=str(self.runtime))

    def run_script(self, *args, request=None, **env):
        e = dict(self.env, **env)
        r = subprocess.run([sys.executable, "-I", str(SCRIPT), *args], input=json.dumps(request) if request is not None else "", capture_output=True, text=True, env=e, timeout=20)
        lines = [json.loads(l) for l in r.stdout.splitlines() if l.strip()]
        return r, lines

    def calls(self):
        return json.loads(self.state.read_text())["calls"] if self.state.exists() else []

    def assert_secret_never_on_argv(self, *secrets):
        for call in self.calls():
            for s in secrets:
                self.assertNotIn(s, " ".join(call["argv"]), "a secret reached nmcli's command line")

    def test_peap_join_keeps_the_password_off_the_command_line(self):
        req = {"ssid": "eduroam", "security": "enterprise", "eap": "peap", "phase2": "mschapv2", "identity": "me@uni.edu",
               "password": "s3cret pass!", "domain": "radius.uni.edu", "ca": "system"}
        r, out = self.run_script("join", request=req)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual([m["state"] for m in out], ["saving", "connecting", "connected"])
        self.assert_secret_never_on_argv("s3cret pass!")
        edits = [c for c in self.calls() if c["argv"][:2] == ["connection", "edit"]]
        self.assertEqual(len(edits), 1)
        self.assertIn("set 802-1x.password s3cret pass!", edits[0]["stdin"])
        self.assertIn("set 802-1x.system-ca-certs yes", edits[0]["stdin"])
        self.assertIn("set 802-1x.domain-suffix-match radius.uni.edu", edits[0]["stdin"])
        self.assertNotIn("s3cret", r.stdout + r.stderr)
        # nothing left behind: no secret files
        self.assertEqual(list(self.runtime.iterdir()), [])

    def test_tls_with_key_password_and_ca_file(self):
        cert, key = Path(self.tmp.name) / "me.crt", Path(self.tmp.name) / "me.key"
        cert.write_text("c")
        key.write_text("k")
        req = {"ssid": "Corp", "security": "enterprise", "eap": "tls", "identity": "me", "ca": str(self.ca),
               "clientCert": str(cert), "privateKey": str(key), "keyPassword": "keypw"}
        r, out = self.run_script("join", request=req)
        self.assertEqual(out[-1]["state"], "connected", out)
        self.assert_secret_never_on_argv("keypw")
        stdin = [c for c in self.calls() if c["argv"][:2] == ["connection", "edit"]][0]["stdin"]
        self.assertIn(f"set 802-1x.ca-cert {self.ca}", stdin)
        self.assertIn("set 802-1x.private-key-password keypw", stdin)
        self.assertNotIn("phase2-auth mschapv2", stdin)

    def test_a_saved_profile_is_updated_not_duplicated(self):
        req = {"ssid": "eduroam", "security": "enterprise", "eap": "ttls", "phase2": "pap", "identity": "a", "password": "p1", "ca": "none"}
        self.run_script("join", request=req)
        self.run_script("join", request=dict(req, password="p2"))
        db = json.loads(self.state.read_text())
        self.assertEqual(len(db["profiles"]), 1)
        edits = [c for c in self.calls() if c["argv"][:2] == ["connection", "edit"]]
        self.assertEqual(edits[1]["argv"][2], "uuid")

    def test_line_breaks_cant_inject_editor_commands(self):
        req = {"ssid": "x", "security": "enterprise", "eap": "peap", "identity": "me\nset 802-1x.ca-cert /evil", "password": "p"}
        r, out = self.run_script("join", request=req)
        self.assertEqual(out[-1]["state"], "error")
        self.assertFalse(any(c["argv"][:2] == ["connection", "edit"] for c in self.calls()))

    def test_errors_say_how_to_fix_them(self):
        req = {"ssid": "eduroam", "security": "enterprise", "eap": "peap", "identity": "a", "password": "wrong", "domain": "radius.uni.edu", "ca": "system"}
        _, out = self.run_script("join", request=req, FAKE_NM_UP_ERROR="Error: Connection activation failed: Secrets were required, but not provided.")
        self.assertEqual(out[-1]["message"], "Couldn't sign in. Check the username and password.")
        _, out = self.run_script("join", request=req, FAKE_NM_UP_ERROR="Error: Connection activation failed: the 802.1X supplicant failed (certificate verify failed)")
        self.assertIn("didn't match radius.uni.edu", out[-1]["message"])
        _, out = self.run_script("join", request=dict(req, ssid="new"), FAKE_NM_REFUSE_SAVE="1")
        self.assertIn("Couldn't save", out[-1]["message"])
        _, out = self.run_script("join", request=dict(req, identity=""))
        self.assertIn("username", out[-1]["message"])

    def test_hidden_psk_network(self):
        req = {"ssid": "Hidden Net", "hidden": True, "security": "psk", "psk": "hunter22"}
        r, out = self.run_script("join", request=req)
        self.assertEqual(out[-1]["state"], "connected")
        self.assert_secret_never_on_argv("hunter22")
        stdin = [c for c in self.calls() if c["argv"][:2] == ["connection", "edit"]][0]["stdin"]
        self.assertIn("set 802-11-wireless.hidden yes", stdin)
        self.assertIn("set 802-11-wireless.ssid Hidden Net", stdin)
        _, out = self.run_script("join", request=dict(req, psk="short"))
        self.assertIn("8 to 63", out[-1]["message"])

    def test_show_returns_settings_without_secrets(self):
        req = {"ssid": "eduroam", "security": "enterprise", "eap": "peap", "identity": "me", "password": "topsecret", "domain": "d.edu", "ca": "system"}
        self.run_script("join", request=req)
        r, out = self.run_script("show", "eduroam")
        info = out[-1]
        self.assertTrue(info["exists"])
        self.assertEqual(info["identity"], "me")
        self.assertEqual(info["domain"], "d.edu")
        self.assertNotIn("topsecret", r.stdout)


    def seed(self, profiles):
        self.state.write_text(json.dumps({"profiles": profiles, "calls": []}))

    def test_known_lists_saved_networks_once_each(self):
        self.seed({"u1": {"name": "Home", "ssid": "Home"}, "u2": {"name": "Home 1", "ssid": "Home"},
                   "u3": {"name": "Cafe", "ssid": "Cafe: Guest"}})
        r, out = self.run_script("known")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(out[-1]["known"], [{"ssid": "Cafe: Guest", "name": "Cafe"}, {"ssid": "Home", "name": "Home"}])

    def test_forget_deletes_every_profile_for_that_name_only(self):
        self.seed({"u1": {"name": "Home", "ssid": "Home"}, "u2": {"name": "Home 1", "ssid": "Home"},
                   "u3": {"name": "Cafe", "ssid": "Cafe"}})
        r, out = self.run_script("forget", "Home")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(out[-1], {"forgotten": 2})
        self.assertEqual(list(json.loads(self.state.read_text())["profiles"]), ["u3"])

    def test_forget_says_what_went_wrong(self):
        self.seed({"u1": {"name": "Home", "ssid": "Home"}})
        r, out = self.run_script("forget", "Home", FAKE_NM_REFUSE_DELETE="1")
        self.assertEqual(r.returncode, 1)
        self.assertEqual(out[-1]["state"], "error")
        self.assertIn("Couldn't forget Home", out[-1]["message"])
        self.assertIn("not authorized", out[-1]["message"])


if __name__ == "__main__":
    unittest.main()
