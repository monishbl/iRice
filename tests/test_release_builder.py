"""The release builder (docs/releases.md): server1's tools/release-builder/
tessera-release builds and leaves a set ready (unsigned: no key there); the
laptop's publish-release checks it, signs it and publishes it. Here their own
code runs on small script "programs" (no cargo), signed with a throwaway
password-protected minisign key made in each test (never the release key): a
set the builder makes, signed by the publisher, is exactly what
tools/native-install.py accepts; a tampered set, a wrong password or a key
that isn't the committed one never gets signed or published. Needs the
minisign program; no network, no ssh, nothing published."""
import importlib.machinery
import importlib.util
import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
BUILDER = ROOT / "tools/release-builder/tessera-release"
PUBLISHER = ROOT / "tools/release-builder/publish-release"
HAS_MINISIGN = shutil.which("minisign") is not None
HAS_CRYPTO = importlib.util.find_spec("cryptography") is not None


def load(name, path):
    loader = importlib.machinery.SourceFileLoader(name, str(path))
    spec = importlib.util.spec_from_loader(name, loader)
    mod = importlib.util.module_from_spec(spec)
    loader.exec_module(mod)
    return mod


ni = load("tessera_native_install_rb", ROOT / "tools/native-install.py")


def git(repo, *args):
    return subprocess.run(["git", "-C", str(repo), "-c", "user.name=t", "-c", "user.email=t@t", *args],
                          capture_output=True, text=True, check=True).stdout.strip()


def fake_program(path, said):
    path.write_text("#!/bin/sh\n[ \"$1\" = --version ] && echo '" + said + "' && exit 0\nexit 3\n")
    path.chmod(0o755)


def make_key(d, password):
    """A throwaway minisign key pair with a password: (pub, key) paths."""
    d.mkdir(parents=True, exist_ok=True)
    pub, key = d / "test.pub", d / "test.key"
    subprocess.run(["minisign", "-G", "-f", "-p", str(pub), "-s", str(key)], input=f"{password}\n{password}\n",
                   capture_output=True, text=True, check=True)
    return pub, key


@unittest.skipUnless(HAS_MINISIGN and HAS_CRYPTO, "needs minisign and python-cryptography")
class Release(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="tessera-release-")
        self.d = Path(self.tmp.name)
        # ── server1's half, in this test's folder
        self.rb = rb = load("tessera_release_builder", BUILDER)
        root = self.d / "server/tessera-release"
        rb.ROOT, rb.SRC, rb.OUT, rb.BUILT = str(root), str(root / "src"), str(root / "out"), str(root / "built")
        rb.TARGETS, rb.LOGS = str(root / "target"), str(root / "logs")
        rb.CACHE, rb.LOG, rb.STATUS = str(self.d / "server/cache"), str(self.d / "server/cache/r.log"), str(self.d / "server/cache/r.json")
        rb.STATE.clear()
        src = Path(rb.SRC)
        (src / "native/tessera-ui").mkdir(parents=True)
        (src / "native/tessera-ui/Cargo.toml").write_text('[package]\nname = "tessera-ui"\nversion = "0.4.2"\n')
        (src / "tools").mkdir()
        self.pub, self.key = make_key(self.d / "laptop/.minisign", "correct horse")
        shutil.copy(self.pub, src / "tools/tessera-ui.pub")
        subprocess.run(["git", "init", "-q", "-b", "beta", str(src)], check=True)
        git(src, "add", "-A")
        git(src, "commit", "-q", "-m", "one")
        self.commit = git(src, "rev-parse", "HEAD")
        self.tree = git(src, "rev-parse", "HEAD:native")
        self.epoch = git(src, "log", "-1", "--format=%ct")
        self.target = {"name": "beta", "kind": "branch", "tag": "tessera-ui-beta", "channel": "beta", "commit": self.commit}
        for variant, _ in rb.VARIANTS:  # ("built" programs where cargo would have put them)
            rel = Path(rb.TARGETS) / variant / "release"
            rel.mkdir(parents=True)
            for name in ("tessera-ui", "tessera-events"):
                fake_program(rel / name, f"{name} 0.4.2")
        # ── the laptop's half
        self.pr = pr = load("tessera_publish_release", PUBLISHER)
        pr.STAGE = str(self.d / "laptop/stage")
        pr.CACHE, pr.LOG, pr.STATUS = str(self.d / "laptop/cache"), str(self.d / "laptop/cache/p.log"), str(self.d / "laptop/cache/p.json")
        pr.KEY = str(self.key)
        pr.PASSWORD = str(self.d / "laptop/.minisign/tessera-ui.password")
        pr.STATE.clear()
        real_run = pr.run

        def fake_run(cmd, what, timeout=600, **kw):
            if cmd[0] == "scp":  # (from server1's set folder, not over ssh)
                *srcs, dest = [c for c in cmd if ":" in c or c.endswith("/")]
                for s in srcs:
                    shutil.copy(Path(rb.OUT, s.split(":", 1)[1].split("/out/", 1)[1]), dest)
                return ""
            return real_run(cmd, what, timeout, **kw)

        self.patches = [mock.patch.object(pr, "run", fake_run), mock.patch.object(pr, "REPO", rb.SRC),
                        mock.patch.object(pr, "pubkey_for", lambda c: (self.pub.read_text(), "the test"))]
        for p in self.patches:
            p.start()

    def tearDown(self):
        for p in self.patches:
            p.stop()
        self.tmp.cleanup()

    def password(self, text, mode=0o600):
        Path(self.pr.PASSWORD).write_text(text)
        os.chmod(self.pr.PASSWORD, mode)

    def build_set(self):
        built = os.path.join(self.rb.BUILT, self.tree)
        os.makedirs(built, exist_ok=True)
        variants = {v: self.rb.pack(built, self.tree, v, self.epoch) for v, _ in self.rb.VARIANTS}
        self.rb.make_set(self.target, self.tree, variants)
        return self.rb.list_ready()

    def set_dir(self):
        return Path(self.rb.OUT, "tessera-ui-beta", self.tree)

    def test_server_set_signed_on_the_laptop_is_what_native_install_accepts(self):
        listed = self.build_set()
        t = self.tree
        self.assertEqual([(s["tag"], s["tree"], s["commit"], s["name"]) for s in listed],
                         [("tessera-ui-beta", t, self.commit, "beta")])
        # (server1 signs nothing)
        self.assertEqual(sorted(os.listdir(self.set_dir())), sorted(
            ["READY", f"tessera-ui-{t}-x86-64-v3.tar.zst", f"tessera-ui-{t}-generic.tar.zst", f"tessera-ui-{t}.manifest.json"]))
        m = json.loads((self.set_dir() / f"tessera-ui-{t}.manifest.json").read_text())
        self.assertEqual((m["format"], m["name"], m["commit"], m["tree"], m["version"], m["channel"], m["tag"]),
                         (1, "tessera-ui", self.commit, t, "0.4.2", "beta", "tessera-ui-beta"))
        self.assertEqual(m["variants"]["generic"]["programs"]["tessera-events"]["version"], "tessera-events 0.4.2")
        self.password("correct horse\n")
        dest, _ = self.pr.fetch_and_check(listed[0])
        self.pr.sign(dest, listed[0])
        self.assertEqual(sorted(n for n in os.listdir(dest) if not n.startswith(".")), self.pr.expected_files(t))
        sig = Path(dest, f"tessera-ui-{t}.manifest.json.minisig").read_text().splitlines()[2]
        self.assertEqual(sig, f"trusted comment: tessera-ui-{t}.manifest.json tree={t} commit={self.commit}")
        # As a file:// release, native-install.py fetches and checks it
        shutil.copytree(dest, self.d / "releases/tessera-ui-beta")
        for variant in ("x86-64-v3", "generic"):
            got = ni.fetch(t, self.commit, variant, str(self.d / ("work-" + variant)),
                           base="file://" + str(self.d / "releases") + "/{tag}",
                           tags=["tessera-ui-beta"], pubkey=self.pub.read_text(), repo=self.rb.SRC)
            self.assertEqual((got["info"]["source"], got["info"]["variant"], got["info"]["tag"]),
                             ("ready-made", variant, "tessera-ui-beta"))
            self.assertEqual(sorted(got["files"]), ["tessera-events", "tessera-ui"])
        # Packing is reproducible: the same programs give the same archive
        again = self.rb.pack(os.path.join(self.rb.BUILT, t), t, "generic", self.epoch)
        self.assertEqual(again["sha256"], m["variants"]["generic"]["sha256"])

    def test_mark_published(self):
        self.build_set()
        self.assertEqual(self.rb.mark_published("tessera-ui-beta", self.tree), 0)
        self.assertEqual(self.rb.list_ready(), [])
        self.assertTrue((self.set_dir() / "PUBLISHED").exists())
        self.assertEqual(self.rb.mark_published("tessera-ui-beta", self.tree), 0)  # (again: fine)
        self.assertEqual(self.rb.mark_published("tessera-ui-beta", "0" * 40), 1)
        self.assertEqual(self.rb.load_status()["targets"]["beta"]["result"], "published")

    def test_tampered_sets_are_refused(self):
        listed = self.build_set()[0]
        self.pr.fetch_and_check(listed)
        # A set that claims another release than its branch's
        with self.assertRaises(self.pr.Refused):
            self.pr.fetch_and_check(dict(listed, tag="tessera-ui-latest"))
        # An archive that isn't the one listed
        tar = self.set_dir() / f"tessera-ui-{self.tree}-generic.tar.zst"
        tar.write_bytes(tar.read_bytes() + b"x")
        with self.assertRaises(self.pr.Refused):
            self.pr.fetch_and_check(listed)
        # Listed to match, but not the manifest's
        self.assertRaisesRegex(self.pr.Refused, "doesn't match the signed manifest", self.pr.fetch_and_check, dict(
            listed, files=dict(listed["files"], **{tar.name: {"size": tar.stat().st_size,
                                                              "sha256": self.pr.sha256_of(str(tar))}})))

    def test_no_password_stops_before_fetching_and_the_set_waits(self):
        self.build_set()
        with mock.patch.object(self.pr, "list_ready", self.rb.list_ready), mock.patch.object(self.pr, "can_publish", lambda: None):
            self.assertEqual(self.pr.main([]), 1)
        st = self.pr.load_status()
        self.assertEqual(st["state"], "failed")
        self.assertIn("tessera-ui.password", st["error"])
        self.assertEqual(st["waiting"], [f"tessera-ui-beta/{self.tree}"])
        self.assertFalse(os.path.exists(self.pr.STAGE))
        self.assertTrue((self.set_dir() / "READY").exists())

    def test_password_readable_by_others_is_refused(self):
        self.password("correct horse\n", 0o644)
        with self.assertRaisesRegex(self.pr.Refused, "chmod 600"):
            self.pr.read_password()

    def test_wrong_password_signs_nothing_and_never_says_it(self):
        listed = self.build_set()[0]
        self.password("zebra-17-nope\n")
        dest, _ = self.pr.fetch_and_check(listed)
        with self.assertRaises(self.pr.Refused) as e:
            self.pr.sign(dest, listed)
        self.assertIn("password", str(e.exception))
        self.assertNotIn("zebra-17", str(e.exception))
        self.assertFalse(any(n.endswith(".minisig") for n in os.listdir(dest)))

    def test_a_key_that_isnt_the_committed_one_is_refused(self):
        listed = self.build_set()[0]
        _, other = make_key(self.d / "other", "correct horse")
        self.pr.KEY = str(other)
        self.password("correct horse\n")
        dest, _ = self.pr.fetch_and_check(listed)
        with self.assertRaisesRegex(self.pr.Refused, "against tools/tessera-ui.pub"):
            self.pr.sign(dest, listed)

    def test_the_password_never_reaches_a_log_or_status(self):
        listed = self.build_set()[0]
        self.password("correct horse\n")
        dest, _ = self.pr.fetch_and_check(listed)
        self.pr.sign(dest, listed)
        for f in (self.pr.LOG, self.pr.STATUS, self.rb.LOG, self.rb.STATUS):
            if os.path.exists(f):
                self.assertNotIn("correct horse", Path(f).read_text())


class Units(unittest.TestCase):
    def test_timers_every_ten_minutes(self):
        self.assertIn("OnCalendar=*:0/10", (ROOT / "tools/release-builder/tessera-release.timer").read_text())
        self.assertIn("OnCalendar=*:5/10", (ROOT / "tools/release-builder/tessera-publish.timer").read_text())
        self.assertIn("ExecStart=%h/bin/tessera-release", (ROOT / "tools/release-builder/tessera-release.service").read_text())
        self.assertIn("ExecStart=%h/tessera-verify/publish-release.sh",
                      (ROOT / "tools/release-builder/tessera-publish.service").read_text())

    def test_builds_go_through_tbuild_not_urgent_full_release(self):
        src = BUILDER.read_text()
        self.assertIn('TBUILD_URGENT=""', src)
        self.assertIn('"build", "--release", "--locked"', src)
        self.assertNotIn("--profile", src)
        self.assertIn('("x86-64-v3", "-C target-cpu=x86-64-v3"), ("generic", "")', src)

    def test_server1_never_signs_or_publishes(self):
        src = BUILDER.read_text()
        for word in ('"minisign"', ".minisign", "PASSWORD", '"gh"', "gitlab", "GH_TOKEN", "release upload"):
            self.assertNotIn(word, src)

    def test_workflow_no_longer_publishes_on_push(self):
        wf = (ROOT / ".github/workflows/tessera-ui.yml").read_text()
        on = wf[wf.index("\non:"):wf.index("\npermissions:")]
        self.assertNotIn("\n  push:", on)
        self.assertIn("workflow_dispatch", on)


if __name__ == "__main__":
    unittest.main()
