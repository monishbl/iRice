"""Update channels (tools/tessera-update.py): Stable follows the branch main,
Beta the branch beta. A real remote and clone in a temporary folder: the
check finds the channel's version, applying moves the checkout to the
channel's branch (Beta to Stable is going back, and says so), a failed apply
goes back to the branch it was on, and the background notice is said once
per version (again a day later). Nothing here touches the network."""
import importlib.util
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
ID = ["-c", "user.name=Test", "-c", "user.email=test@example.invalid", "-c", "commit.gpgsign=false"]


def load():
    spec = importlib.util.spec_from_file_location("tessera_update_channels", ROOT / "tools/tessera-update.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def git(repo, *args):
    r = subprocess.run(["git", *ID, "-C", str(repo), *args], capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError(" ".join(args) + ": " + r.stderr)
    return r.stdout.strip()


class Channels(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        t = Path(self.tmp.name)
        self.remote, seed, self.repo, self.config = t / "remote.git", t / "seed", t / "copy", t / "config"
        self.config.mkdir()
        subprocess.run(["git", "init", "-q", "--bare", "-b", "main", str(self.remote)], check=True)
        subprocess.run(["git", "init", "-q", "-b", "main", str(seed)], check=True)
        (seed / "shell/common").mkdir(parents=True)
        (seed / "shell/common/Config.qml").write_text("JsonAdapter {\n}\n")
        self.seed = seed
        for name in ("one", "two"):
            self.commit(name)
        git(seed, "checkout", "-q", "-b", "beta")
        for name in ("three", "four"):
            self.commit(name)
        git(seed, "remote", "add", "origin", str(self.remote))
        git(seed, "push", "-q", "origin", "main", "beta")
        # (as setup.sh downloads it: the branch main only)
        subprocess.run(["git", "clone", "-q", "--branch", "main", "--single-branch", str(self.remote), str(self.repo)], check=True)
        self.m = load()
        self.m.REPO = str(self.repo)
        self.m.native_install = lambda *a, **k: {"ok": True, "changed": False}
        self.m.native_plan = lambda *a, **k: None
        self.state = mock.patch.dict(os.environ, {"XDG_STATE_HOME": str(t / "state")})
        self.state.start()

    def tearDown(self):
        self.state.stop()
        self.tmp.cleanup()

    def commit(self, name, branch=None):
        if branch:
            git(self.seed, "checkout", "-q", branch)
        (self.seed / (name + ".txt")).write_text(name)
        git(self.seed, "add", "-A")
        git(self.seed, "commit", "-qm", name)

    def choose(self, channel):
        (self.config / "config.json").write_text(json.dumps({"update": {"channel": channel}}))

    def go(self, channel):
        """Software Update: the channel, a check, Update."""
        self.choose(channel)
        self.m.check(str(self.config))
        return self.m.apply(str(self.config), {})

    def head(self):
        return git(self.repo, "log", "-1", "--format=%s"), git(self.repo, "symbolic-ref", "--short", "HEAD"), git(self.repo, "rev-parse", "--abbrev-ref", "@{u}")

    def test_until_chosen_the_branch_followed_decides(self):
        self.assertEqual(self.m.follow(str(self.config)), ("stable", "origin", "main"))
        p = self.m.position(str(self.config), fetch=True)
        self.assertEqual((p["channel"], p["kind"], p["available"]), ("stable", "none", False))
        # (a developer's checkout on another branch keeps following it)
        git(self.repo, "checkout", "-q", "-b", "dev", "--track", "origin/main")
        git(self.repo, "config", "branch.dev.merge", "refs/heads/feature")
        git(self.repo, "config", "--add", "remote.origin.fetch", "+refs/heads/feature:refs/remotes/origin/feature")
        git(self.repo, "update-ref", "refs/remotes/origin/feature", "HEAD")
        self.assertEqual(self.m.follow(str(self.config))[0], "custom")

    def test_stable_to_beta_is_an_update(self):
        self.choose("beta")
        r = self.m.check(str(self.config))
        self.assertEqual((r["channel"], r["kind"], r["behind"], r["downgrade"]), ("beta", "update", 2, False))
        self.assertTrue(r["available"])
        self.assertEqual(r["notes"], ["four", "three"])
        out = self.m.apply(str(self.config), {})
        self.assertTrue(out["applied"])
        self.assertEqual(self.head(), ("four", "beta", "origin/beta"))

    def test_beta_to_stable_is_going_back_and_says_so(self):
        self.go("beta")
        self.choose("stable")
        r = self.m.check(str(self.config))
        self.assertEqual((r["kind"], r["downgrade"], r["ahead"], r["behind"]), ("switch", True, 2, 0))
        self.assertTrue(r["available"])
        self.assertEqual(r["leaving"], ["four", "three"])
        self.assertEqual(r["channelName"], "Stable")
        self.m.apply(str(self.config), {})
        self.assertEqual(self.head(), ("two", "main", "origin/main"))
        self.assertEqual(git(self.repo, "rev-parse", "beta"), git(self.repo, "rev-parse", "origin/beta"))  # (Beta's version stays to go back to)
        # Promoted later: Stable moves on, as an ordinary update
        git(self.seed, "checkout", "-q", "main")
        git(self.seed, "merge", "-q", "--ff-only", "beta")
        git(self.seed, "push", "-q", "origin", "main")
        r = self.m.check(str(self.config))
        self.assertEqual((r["kind"], r["behind"]), ("update", 2))

    def test_commits_of_your_own_stop_it(self):
        self.go("beta")
        (self.repo / "mine.txt").write_text("mine")
        git(self.repo, "add", "mine.txt")
        git(self.repo, "commit", "-qm", "mine")
        self.choose("stable")
        r = self.m.check(str(self.config))
        self.assertFalse(r["available"])
        self.assertIn("1 commit(s) of its own", r["error"])

    def test_a_failed_apply_goes_back_to_the_branch_it_was_on(self):
        before = git(self.repo, "rev-parse", "HEAD")
        self.choose("beta")
        self.m.check(str(self.config))

        def fail(*a, **k):
            raise RuntimeError("injected")
        self.m.native_install = fail
        with self.assertRaises(SystemExit) as e:
            self.m.apply(str(self.config), {})
        self.assertIn("Nothing was changed", str(e.exception))
        self.assertEqual(git(self.repo, "rev-parse", "HEAD"), before)
        self.assertEqual(self.head()[1:], ("main", "origin/main"))
        self.assertEqual(git(self.repo, "branch", "--list", "beta"), "")

    def test_a_channel_not_published_yet(self):
        git(self.seed, "push", "-q", "origin", "--delete", "beta")
        self.choose("beta")
        r = self.m.check(str(self.config))
        self.assertFalse(r["available"])
        self.assertIn("isn't published yet", r["error"])

    def test_the_notice_once_per_version_then_a_day_later(self):
        self.choose("beta")
        first = self.m.peek(str(self.config), mark=True, now=1000)
        self.assertTrue(first["notify"])
        self.assertFalse(self.m.peek(str(self.config), mark=True, now=2000)["notify"])
        self.assertTrue(self.m.peek(str(self.config), mark=True, now=1000 + 86400)["notify"])
        self.commit("five", "beta")
        git(self.seed, "push", "-q", "origin", "beta")
        self.assertTrue(self.m.peek(str(self.config), mark=True, now=1000 + 86401)["notify"])
        # Going back to another channel is never pushed as a notice
        self.m.apply(str(self.config), {})
        self.choose("stable")
        self.assertFalse(self.m.peek(str(self.config), mark=True, now=10 ** 9)["notify"])

    def test_info_downloads_nothing(self):
        # (info is position() without a fetch: a single-branch copy has no beta yet)
        p = self.m.position(str(self.config), False, "beta")
        self.assertEqual((p["channel"], p["channelName"], p["branch"]), ("beta", "Beta", "beta"))
        self.assertIn("hasn't downloaded the Beta channel", p["error"])
        self.assertEqual(p["current"], git(self.repo, "rev-parse", "--short=8", "HEAD"))
        self.assertGreater(p["date"], 0)


class Wiring(unittest.TestCase):
    def test_config_and_both_settings(self):
        cfg = (ROOT / "shell/common/Config.qml").read_text()
        self.assertIn('property JsonObject update: JsonObject {', cfg)
        self.assertIn('property string channel: ""', cfg)
        page = (ROOT / "shell/modules/settings/pages/update.qml").read_text()
        self.assertIn('SoftwareUpdate.setChannel(["stable", "beta"][i])', page)
        rust = (ROOT / "native/tessera-ui/surfaces/settings/src/pages/about/update.rs").read_text()
        self.assertIn('cx.ask("about", "su.channel", json!([ch]))', rust)
        bridge = (ROOT / "shell/modules/settings/RustBridgeAbout.qml").read_text()
        self.assertIn('case "su.channel":', bridge)

    def test_notice_asks_never_installs(self):
        for f in ("shell/services/TesseraUpdate.qml", "native/tessera-ui/host/system/src/sysinfo/tessera_update.rs"):
            src = (ROOT / f).read_text()
            self.assertIn('"update=Update"', src, f)
            self.assertIn('"later=Later"', src, f)
            self.assertIn('"Update available"', src, f)

    def test_workflow_builds_both_branches(self):
        wf = (ROOT / ".github/workflows/tessera-ui.yml").read_text()
        self.assertIn("branches: [main, beta]", wf)
        self.assertIn("tag=tessera-ui-beta", wf)


if __name__ == "__main__":
    unittest.main()
