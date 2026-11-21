"""The login screen's tessera-ui (20 Nov audit #71, #24): root copies yours
for greetd only when it's Tessera's signed release build (its record keeps
the signed manifest, checked against root's own copy of the release key)
or a build of yours its record vouches for (the sha256 recorded right after
it was built). Anything else is refused and the login screen falls back to
the QML one. An update's root step refreshes it whenever yours is another
program now (sysupdate.py login_ui_stale), with the one password it asks
anyway. Throwaway keys, homes and system folders only."""
import base64
import hashlib
import importlib.util
import json
import os
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
SCRIPTS = ROOT / "shell/assets/scripts"
HAS_CRYPTO = importlib.util.find_spec("cryptography") is not None


def load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


R = load(SCRIPTS / "tessera-root.py", "tessera_root_trust")


class Key:
    """A throwaway minisign key (as tests/test_native_install.py's)"""

    def __init__(self):
        from cryptography.hazmat.primitives import serialization
        from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
        self.priv = Ed25519PrivateKey.generate()
        self.keyid = os.urandom(8)
        pk = self.priv.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)
        self.pub = ("untrusted comment: minisign public key " + self.keyid[::-1].hex().upper() + "\n"
                    + base64.b64encode(b"Ed" + self.keyid + pk).decode() + "\n")

    def sign(self, data, trusted, prehash=True):
        msg = hashlib.blake2b(data, digest_size=64).digest() if prehash else data
        sig = self.priv.sign(msg)
        glob = self.priv.sign(sig + trusted.encode())
        return ("untrusted comment: signature from minisign secret key\n"
                + base64.b64encode((b"ED" if prehash else b"Ed") + self.keyid + sig).decode() + "\n"
                + "trusted comment: " + trusted + "\n" + base64.b64encode(glob).decode() + "\n")


PROGRAM = b"\x7fELF the login screen"
TREE = "a" * 40


def sha(data):
    return hashlib.sha256(data).hexdigest()


def ready_made(key, data=PROGRAM, tree=TREE, edit=None, trusted=None):
    """A ready-made install's record, as native-install.py writes it"""
    manifest = {"format": 1, "tree": tree, "commit": "b" * 40,
                "variants": {"x86-64-v3": {"programs": {"tessera-ui": {"sha256": sha(data)}, "tessera-events": {"sha256": "c" * 64}}},
                             "generic": {"programs": {"tessera-ui": {"sha256": "d" * 64}}}}}
    if edit:
        edit(manifest)
    text = json.dumps(manifest)
    return {"source": "ready-made", "variant": "x86-64-v3", "tree": tree, "sha256": sha(data), "manifest": text,
            "minisig": key.sign(text.encode(), trusted or f"tessera-ui-{tree}.manifest.json tree={tree} commit={'b' * 40}")}


@unittest.skipUnless(HAS_CRYPTO, "python-cryptography isn't installed")
class Vouched(unittest.TestCase):
    def setUp(self):
        self.key = Key()

    def test_signed_and_named_by_its_manifest(self):
        self.assertEqual(R.login_ui_vouched(PROGRAM, ready_made(self.key), key=self.key.pub), "signed")
        # (the generic build's checksum counts too: any variant the manifest signs)
        rec = ready_made(self.key, edit=lambda m: m["variants"]["generic"]["programs"]["tessera-ui"].update(sha256=sha(b"\x7fELF g")))
        self.assertEqual(R.login_ui_vouched(b"\x7fELF g", rec, key=self.key.pub), "signed")

    def test_a_bad_signature_is_refused(self):
        rec = ready_made(Key())  # (signed by another key)
        with self.assertRaisesRegex(R.Refused, "another key"):
            R.login_ui_vouched(PROGRAM, rec, key=self.key.pub)
        rec = ready_made(self.key)
        rec["manifest"] = rec["manifest"].replace('"format": 1', '"format": 2')  # (changed after it was signed)
        with self.assertRaisesRegex(R.Refused, "signature doesn't match"):
            R.login_ui_vouched(PROGRAM, rec, key=self.key.pub)
        rec = ready_made(self.key)
        rec["minisig"] = "not a signature"
        with self.assertRaises(R.Refused):
            R.login_ui_vouched(PROGRAM, rec, key=self.key.pub)
        rec = ready_made(self.key, trusted="tessera-ui tree=" + "e" * 40)  # (signed for another version)
        with self.assertRaisesRegex(R.Refused, "another version"):
            R.login_ui_vouched(PROGRAM, rec, key=self.key.pub)

    def test_a_checksum_the_manifest_doesnt_name_is_refused(self):
        with self.assertRaisesRegex(R.Refused, "checksum differs"):
            R.login_ui_vouched(PROGRAM + b"!", ready_made(self.key), key=self.key.pub)

    def test_an_older_record_without_the_manifest_is_refused(self):
        rec = ready_made(self.key)
        del rec["manifest"], rec["minisig"]
        with self.assertRaisesRegex(R.Refused, "wasn't kept"):
            R.login_ui_vouched(PROGRAM, rec, key=self.key.pub)

    def test_the_real_release_key_is_roots_own_copy(self):
        self.assertEqual(R.RELEASE_KEY.strip(), (ROOT / "tools/tessera-ui.pub").read_text().strip())
        with self.assertRaises(R.Refused):
            R.login_ui_vouched(PROGRAM, ready_made(self.key))  # (a throwaway key isn't Tessera's)


class BuiltHere(unittest.TestCase):
    def test_a_build_that_matches_its_record(self):
        for source in ("built", "built-tuned"):
            self.assertEqual(R.login_ui_vouched(PROGRAM, {"source": source, "sha256": sha(PROGRAM)}), "built")

    def test_a_build_changed_since_is_refused(self):
        with self.assertRaisesRegex(R.Refused, "changed after it was built"):
            R.login_ui_vouched(PROGRAM + b"tampered", {"source": "built", "sha256": sha(PROGRAM)})

    def test_no_record_is_refused(self):
        for rec in (None, {}, {"source": "built"}, {"source": "somewhere"}, ["built"]):
            with self.assertRaises(R.Refused):
                R.login_ui_vouched(PROGRAM, rec)

    def test_it_says_built_on_this_computer(self):
        self.assertEqual(R.LOGIN_UI_HOW["built"], "built on this computer")


class FromYourHome(unittest.TestCase):
    """read_login_ui: the file checks (ELF, owner, modes, no link) and the record"""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.home = Path(self.tmp.name) / "home"
        self.bin = self.home / ".local/share/tessera/bin"
        self.bin.mkdir(parents=True)
        self.bin.chmod(0o755)

    def tearDown(self):
        self.tmp.cleanup()

    def put(self, data=PROGRAM, record=None):
        (self.bin / "tessera-ui").write_bytes(data)
        (self.bin / "tessera-ui").chmod(0o755)
        if record is not False:
            (self.bin / "tessera-ui.json").write_text(json.dumps(record or {"source": "built", "sha256": sha(data)}))
            (self.bin / "tessera-ui.json").chmod(0o644)

    def test_a_build_of_yours(self):
        self.put()
        self.assertEqual(R.read_login_ui(os.getuid(), str(self.home)), (PROGRAM, "built"))

    @unittest.skipUnless(HAS_CRYPTO, "python-cryptography isn't installed")
    def test_a_signed_build(self):
        key = Key()
        self.put(record=ready_made(key))
        self.assertEqual(R.read_login_ui(os.getuid(), str(self.home), key=key.pub), (PROGRAM, "signed"))

    def test_without_its_record(self):
        self.put(record=False)
        with self.assertRaisesRegex(R.Refused, "no record"):
            R.read_login_ui(os.getuid(), str(self.home))

    def test_a_record_others_could_write(self):
        self.put()
        (self.bin / "tessera-ui.json").chmod(0o666)
        with self.assertRaisesRegex(R.Refused, "another account"):
            R.read_login_ui(os.getuid(), str(self.home))

    def test_tampered(self):
        self.put()
        (self.bin / "tessera-ui").write_bytes(PROGRAM + b"!")
        with self.assertRaisesRegex(R.Refused, "changed after it was built"):
            R.read_login_ui(os.getuid(), str(self.home))


class Stale(unittest.TestCase):
    """sysupdate.py login_ui_stale: the root step refreshes the login
    screen's tessera-ui only when yours is another program root would take"""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        d = Path(self.tmp.name)
        self.system, self.home = d / "root", d / "home"
        self.env = mock.patch.dict(os.environ, {"TESSERA_SYSTEM_ROOT": str(self.system), "XDG_RUNTIME_DIR": str(d / "run")})
        self.env.start()
        self.su = load(SCRIPTS / "sysupdate.py", "sysupdate_stale_" + os.urandom(3).hex())
        self.share = self.system / "usr/share/tessera/bin"
        self.mine = self.home / ".local/share/tessera/bin/tessera-ui"
        self.mine.parent.mkdir(parents=True)

    def tearDown(self):
        self.env.stop()
        self.tmp.cleanup()

    def put(self, data, record):
        self.mine.write_bytes(data)
        self.mine.with_name("tessera-ui.json").write_text(json.dumps(record))

    def stale(self):
        return self.su.login_ui_stale(str(self.home))

    def test_no_login_screen_of_tesseras(self):
        self.put(PROGRAM, {"source": "built", "sha256": sha(PROGRAM)})
        self.assertFalse(self.stale())

    def test_the_same_program_needs_nothing(self):
        self.share.mkdir(parents=True)
        (self.share / "tessera-ui").write_bytes(PROGRAM)
        self.put(PROGRAM, {"source": "built", "sha256": sha(PROGRAM)})
        self.assertFalse(self.stale())

    def test_another_program_is_refreshed(self):
        self.share.mkdir(parents=True)
        (self.share / "tessera-ui").write_bytes(b"\x7fELF older")
        self.put(PROGRAM, {"source": "built", "sha256": sha(PROGRAM)})
        self.assertTrue(self.stale())
        # (fallen back to the QML one: its bin/ is still there)
        (self.share / "tessera-ui").unlink()
        self.assertTrue(self.stale())

    def test_one_root_would_refuse_asks_for_nothing(self):
        self.share.mkdir(parents=True)
        (self.share / "tessera-ui").write_bytes(b"\x7fELF older")
        self.put(PROGRAM, {"source": "built", "sha256": sha(b"another")})
        self.assertFalse(self.stale())
        self.put(PROGRAM, {"source": "ready-made"})  # (an older record: no manifest kept)
        self.assertFalse(self.stale())
        manifest = json.dumps({"variants": {"x86-64-v3": {"programs": {"tessera-ui": {"sha256": sha(PROGRAM)}}}}})
        self.put(PROGRAM, {"source": "ready-made", "manifest": manifest, "minisig": "untrusted comment: x"})
        self.assertTrue(self.stale())  # (the signature is root's to check)

    def test_the_plan_takes_the_setup_step_for_it(self):
        text = (SCRIPTS / "sysupdate.py").read_text()
        self.assertIn('p["setup"] = not fake and (not helper_current(p["setup_dir"]) or login_ui_stale())', text)
        self.assertIn('"login": login_ui_stale()', text)
        rs = (ROOT / "native/tessera-ui/host/system/src/sysinfo/software_update.rs").read_text()
        self.assertIn('if (p["setup"] == true || self.login_stale) && !incoming.is_empty()', rs)


if __name__ == "__main__":
    unittest.main()
