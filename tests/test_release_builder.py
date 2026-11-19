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

    def test_patches_from_the_build_before_are_made_checked_signed_and_used(self):
        rb, t = self.rb, self.tree
        # The build before, published: its archives still kept in built/
        old = "0" * 39 + "1"
        for variant, _ in rb.VARIANTS:
            rel = Path(rb.TARGETS) / variant / "release"
            fake_program(rel / "tessera-ui", "tessera-ui 0.4.1 " + "x" * 2000)
        old_variants = {v: rb.pack(os.path.join(rb.BUILT, old), old, v, self.epoch) for v, _ in rb.VARIANTS}
        for v, vj in old_variants.items():  # (pack names files by tree; keep them under the old tree)
            self.assertTrue(vj["file"].startswith(f"tessera-ui-{old}"))
        pub = Path(rb.OUT, "tessera-ui-beta", old)
        pub.mkdir(parents=True)
        (pub / "PUBLISHED").write_text(json.dumps({"tag": "tessera-ui-beta", "tree": old, "published": "2026-11-18T10:00:00+0000"}))
        keep_old = {v: Path(self.d, "old-" + v) for v, _ in rb.VARIANTS}
        for v, d in keep_old.items():
            d.mkdir()
            rb.unpack_programs(os.path.join(rb.BUILT, old, old_variants[v]["file"]), str(d))
        # This build: tessera-ui changed, tessera-events didn't
        for variant, _ in rb.VARIANTS:
            fake_program(Path(rb.TARGETS) / variant / "release" / "tessera-ui", "tessera-ui 0.4.2 " + "x" * 2000)
        listed = self.build_set()
        s = listed[0]
        self.assertEqual(s["patches"], sorted(f"tessera-ui-{t}-{v}.tessera-ui.from-{old[:12]}.zst" for v, _ in rb.VARIANTS))
        m = json.loads((self.set_dir() / f"tessera-ui-{t}.manifest.json").read_text())
        pe = m["variants"]["generic"]["patches"][old]
        self.assertEqual(sorted(pe), ["tessera-ui"], "an unchanged program gets no patch")
        self.password("correct horse\n")
        dest, _ = self.pr.fetch_and_check(s)
        self.pr.sign(dest, s)
        self.assertEqual(sorted(n for n in os.listdir(dest) if not n.startswith(".")), self.pr.expected_files(t, s["patches"]))
        # A patch the manifest doesn't list, or listed with other bytes, is refused
        bad = dict(s, patches=s["patches"] + [f"tessera-ui-{t}-generic.tessera-events.from-{old[:12]}.zst"])
        with self.assertRaises(self.pr.Refused):
            self.pr.fetch_and_check(bad)
        with self.assertRaises(self.pr.Refused):
            self.pr.fetch_and_check(dict(s, patches=["../../etc/passwd"]))
        # native-install.py takes the patch from the old programs
        shutil.copytree(dest, self.d / "releases/tessera-ui-beta")
        have = {"tree": old, "variant": "generic", "programs": {n: str(keep_old["generic"] / n) for n in ("tessera-ui", "tessera-events")}}
        got = ni.fetch(t, self.commit, "generic", str(self.d / "work-p"), base="file://" + str(self.d / "releases") + "/{tag}",
                       tags=["tessera-ui-beta"], pubkey=self.pub.read_text(), repo=self.rb.SRC, have=have)
        self.assertEqual(got["info"]["how"], "patch")
        self.assertEqual(ni.program_version(got["files"]["tessera-ui"]), "tessera-ui 0.4.2 " + "x" * 2000)

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


class Health(unittest.TestCase):
    """The publisher's look every 2 minutes: alerts once each, quiet when
    server1 is out of reach, the health Settings shows"""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        d = Path(self.tmp.name)
        self.pr = pr = load("tessera_publish_health", PUBLISHER)
        pr.CACHE, pr.LOG, pr.STATUS = str(d), str(d / "p.log"), str(d / "p.json")
        pr.STATE.clear()
        self.sent = []
        self.p = mock.patch.object(pr.subprocess, "run", lambda cmd, **k: self.sent.append(cmd) or subprocess.CompletedProcess(cmd, 0, "", ""))
        self.p.start()

    def tearDown(self):
        self.p.stop()
        self.tmp.cleanup()

    def test_a_failed_build_is_said_once(self):
        b = {"state": "failed", "error": "the generic build failed its test before publishing: it did not answer IPC within 30 s"}
        self.assertEqual(self.pr.alert_health([], b), ["builder"])
        self.assertEqual(self.pr.alert_health([], b), [], "the same failure isn't said again")
        self.assertEqual(self.sent[0][0], "notify-send")
        self.assertEqual(self.pr.STATE["health"]["builder"]["state"], "failed")

    def test_a_set_waiting_over_an_hour_is_said(self):
        import time as _t
        s = {"tag": "tessera-ui-beta", "tree": "a" * 40, "ready": "2026-11-19T07:00:00+0530"}
        t0 = _t.mktime(_t.strptime("2026-11-19T07:00:00", "%Y-%m-%dT%H:%M:%S"))
        self.assertEqual(self.pr.alert_health([s], {}, now_t=t0 + 600), [])
        self.assertEqual(self.pr.alert_health([s], {}, now_t=t0 + 3700), ["wait:tessera-ui-beta/" + "a" * 40])
        self.assertEqual(self.pr.STATE["health"]["waiting"][0]["set"], "tessera-ui-beta/" + "a" * 12)

    def test_network_failures_are_tried_again(self):
        n = []

        def flaky():
            n.append(1)
            if len(n) < 3:
                raise self.pr.Refused("gh release upload failed: connection reset by peer (Connection)")
            return "done"
        with mock.patch.object(self.pr.time, "sleep", lambda s: None):
            self.assertEqual(self.pr.retry("upload", flaky), "done")
        self.assertEqual(len(n), 3)
        with self.assertRaises(self.pr.Refused), mock.patch.object(self.pr.time, "sleep", lambda s: None):
            self.pr.retry("check", lambda: (_ for _ in ()).throw(self.pr.Refused("the manifest names another build")))

    def test_the_timer_looks_often_and_after_login(self):
        t = (ROOT / "tools/release-builder/tessera-publish.timer").read_text()
        self.assertIn("OnUnitActiveSec=2min", t)
        self.assertIn("OnStartupSec=", t)


class Trigger(unittest.TestCase):
    def test_push_release_pokes_the_builder_and_keeps_the_timer(self):
        sh = (ROOT / "tools/push-release").read_text()
        self.assertIn("systemctl --user start --no-block tessera-release.service", sh)
        self.assertIn("OnCalendar=*:0/10", (ROOT / "tools/release-builder/tessera-release.timer").read_text())

    def test_the_builder_tests_before_it_offers(self):
        rb = (ROOT / "tools/release-builder/tessera-release").read_text()
        self.assertLess(rb.index("pretest(tree, variants)"), rb.index("make_set(t, tree, variants)"))
        pt = (ROOT / "tools/release-builder/pretest.sh").read_text()
        for check in ("within 30 s", "lock lock", "lock unlock", "shell ready", "radios"):
            self.assertIn(check, pt)


class BuildEnv(unittest.TestCase):
    """What a release build tells tessera-ui about itself (its build.rs:
    `tessera-ui --version`, Settings → About)"""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="tessera-release-env-")
        self.rb = load("tessera_release_builder_env", BUILDER)
        self.rb.SRC = src = os.path.join(self.tmp.name, "src")
        self.rb.TARGETS = os.path.join(self.tmp.name, "target")
        self.rb.STATE.clear()
        self.rb.ROOT = self.rb.CACHE = self.tmp.name
        self.rb.STATUS = os.path.join(self.tmp.name, "r.json")
        os.makedirs(src)
        subprocess.run(["git", "init", "-q", "-b", "main", src], check=True)
        Path(src, "a").write_text("a\n")
        git(src, "add", "-A")
        subprocess.run(["git", "-C", src, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "one",
                        "--date", "2026-11-17T08:00:00+00:00"], check=True,
                       env=dict(os.environ, GIT_COMMITTER_DATE="2026-11-17T08:00:00+00:00"))
        self.commit = git(src, "rev-parse", "HEAD")

    def tearDown(self):
        self.tmp.cleanup()

    def test_channel_variant_kind_commit_and_date(self):
        for channel in ("stable", "beta", "release"):
            t = {"name": "x", "kind": "branch", "tag": "t", "channel": channel, "commit": self.commit}
            env = self.rb.build_env(t, "x86-64-v3")
            self.assertEqual(env, {"TESSERA_BUILD_CHANNEL": channel, "TESSERA_BUILD_VARIANT": "x86-64-v3",
                                   "TESSERA_BUILD_KIND": "release", "TESSERA_BUILD_COMMIT": self.commit,
                                   "TESSERA_BUILD_DATE": "2026-11-17"})

    def test_cargo_gets_them(self):
        seen = {}

        class P:
            def __init__(self, cmd, env=None, **kw):
                seen.update(env)
                self.stdout = iter(())

            def wait(self):
                return 0

        t = {"name": "beta", "kind": "branch", "tag": "tessera-ui-beta", "channel": "beta", "commit": self.commit}
        extra = self.rb.build_env(t, "generic")
        with mock.patch.object(self.rb.subprocess, "Popen", P), open(os.path.join(self.tmp.name, "log"), "w") as logf:
            self.rb.cargo(["build"], "generic", "", "tessera-ui", logf, extra)
        self.assertEqual((seen["TESSERA_BUILD_CHANNEL"], seen["TESSERA_BUILD_VARIANT"], seen["TESSERA_BUILD_KIND"]), ("beta", "generic", "release"))
        self.assertEqual(seen["RUSTFLAGS"], "")
        self.assertIn('build_env(t, variant)', BUILDER.read_text())


class Units(unittest.TestCase):
    def test_timers(self):
        # server1 builds every ten minutes (a push starts it at once too:
        # tools/push-release); the laptop, which signs, looks 30 s after login
        # and every 2 minutes, counting only while awake (19 Nov)
        self.assertIn("OnCalendar=*:0/10", (ROOT / "tools/release-builder/tessera-release.timer").read_text())
        pub = (ROOT / "tools/release-builder/tessera-publish.timer").read_text()
        self.assertIn("OnStartupSec=30s", pub)
        self.assertIn("OnUnitActiveSec=2min", pub)
        self.assertNotIn("OnCalendar", pub)
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
