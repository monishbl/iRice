"""Download sources (tools/sources.txt): GitHub, its GitLab mirror, GitHub's
tarball. The updater fetches from the next source when one is dead, by URL,
without rewriting the remote; a source that doesn't answer is given up on in
seconds; setup.sh carries the same list and clones from the first that works.
The mirror-list check of tools/pac.sh is tests/pac_mirrors.test.sh (run here).
Nothing here uses the network: dead https sources are a closed local port."""
import importlib.util
import json
import os
import re
import subprocess
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
ID = ["-c", "user.name=Test", "-c", "user.email=test@example.invalid", "-c", "commit.gpgsign=false"]
DEAD_HTTPS = "https://127.0.0.1:9/dead/iRice.git"  # (the discard port: nothing listens, refused at once)


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def git(repo, *args):
    r = subprocess.run(["git", *ID, "-C", str(repo), *args], capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError(" ".join(args) + ": " + r.stderr)
    return r.stdout.strip()


SOURCES = load("tessera_sources_test", ROOT / "tools/tessera_sources.py")


class TheList(unittest.TestCase):
    def test_setup_sh_carries_the_same_list(self):
        sh = (ROOT / "setup.sh").read_text()
        block = sh[sh.index("SOURCES=("):sh.index(")", sh.index("SOURCES=("))]
        in_setup = [w.strip('"') for w in block[len("SOURCES=("):].split()]
        with mock.patch.dict(os.environ, {"TESSERA_MIRRORS": ""}):
            self.assertEqual(in_setup, SOURCES.sources())

    def test_order_github_gitlab_tarball(self):
        with mock.patch.dict(os.environ, {"TESSERA_MIRRORS": ""}):
            s = SOURCES.sources()
        self.assertTrue(s[0].startswith("https://github.com/"))
        self.assertTrue(s[1].startswith("https://gitlab.com/"))
        self.assertIn("{branch}", s[2])
        self.assertTrue(s[2].startswith("https://codeload.github.com/"))

    def test_env_overrides(self):
        with mock.patch.dict(os.environ, {"TESSERA_MIRRORS": "a.git, b.git c/{branch}"}):
            self.assertEqual(SOURCES.sources(), ["a.git", "b.git", "c/{branch}"])
            self.assertEqual(SOURCES.git_sources(), ["a.git", "b.git"])

    def test_release_bases(self):
        self.assertEqual(SOURCES.release_base("https://github.com/monishbl/iRice.git"),
                         "https://github.com/monishbl/iRice/releases/download/{tag}")
        self.assertEqual(SOURCES.release_base("https://gitlab.com/monishbl/iRice.git"),
                         "https://gitlab.com/api/v4/projects/monishbl%2FiRice/packages/generic/tessera-ui/{tag}")
        self.assertEqual(SOURCES.release_base("git@gitlab.com:group/sub/proj.git"),
                         "https://gitlab.com/api/v4/projects/group%2Fsub%2Fproj/packages/generic/tessera-ui/{tag}")
        self.assertIsNone(SOURCES.release_base("https://example.org/x.git"))

    def test_same_repo_however_written(self):
        self.assertTrue(SOURCES.same_repo("git@github.com:monishbl/iRice.git", "https://github.com/monishbl/iRice"))
        self.assertTrue(SOURCES.is_official("https://github.com/monishbl/iRice/"))
        self.assertFalse(SOURCES.is_official("https://github.com/someone/fork.git"))

    def test_a_dead_source_fails_fast(self):
        t = time.time()
        self.assertFalse(SOURCES.reachable(DEAD_HTTPS))
        self.assertLess(time.time() - t, 8)
        self.assertTrue(SOURCES.reachable("/a/local/path"))  # (left to git)


class UpdaterFallback(unittest.TestCase):
    """tools/tessera-update.py: origin dead, the next source has the update."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        t = Path(self.tmp.name)
        self.good, seed, self.repo, self.config = t / "mirror.git", t / "seed", t / "copy", t / "config"
        self.config.mkdir()
        subprocess.run(["git", "init", "-q", "--bare", "-b", "main", str(self.good)], check=True)
        subprocess.run(["git", "init", "-q", "-b", "main", str(seed)], check=True)
        (seed / "shell/common").mkdir(parents=True)
        (seed / "shell/common/Config.qml").write_text("JsonAdapter {\n}\n")
        self.seed = seed
        self.commit("one")
        git(seed, "remote", "add", "origin", str(self.good))
        git(seed, "push", "-q", "origin", "main")
        subprocess.run(["git", "clone", "-q", "--branch", "main", "--single-branch", str(self.good), str(self.repo)], check=True)
        self.commit("two")
        git(seed, "push", "-q", "origin", "main")
        self.m = load("tessera_update_mirrors", ROOT / "tools/tessera-update.py")
        self.m.REPO = str(self.repo)
        self.m.native_plan = lambda *a, **k: None
        self.env = mock.patch.dict(os.environ, {"XDG_STATE_HOME": str(t / "state")})
        self.env.start()

    def tearDown(self):
        self.env.stop()
        self.tmp.cleanup()

    def commit(self, name):
        (self.seed / (name + ".txt")).write_text(name)
        git(self.seed, "add", "-A")
        git(self.seed, "commit", "-qm", name)

    def test_dead_origin_falls_through_to_the_mirror(self):
        dead = str(Path(self.tmp.name) / "gone.git")
        git(self.repo, "remote", "set-url", "origin", dead)
        with mock.patch.dict(os.environ, {"TESSERA_MIRRORS": f"{dead} {DEAD_HTTPS} {self.good}"}):
            t = time.time()
            p = self.m.position(str(self.config), fetch=True)
            self.assertLess(time.time() - t, 15)
        self.assertNotIn("error", p)
        self.assertEqual((p["kind"], p["behind"], p["source"]), ("update", 1, str(self.good)))
        # (the remote isn't rewritten: it's still the dead one)
        self.assertEqual(git(self.repo, "remote", "get-url", "origin"), dead)
        r = self.m.apply(str(self.config), {})
        self.assertNotIn("error", r)
        self.assertEqual(git(self.repo, "log", "-1", "--format=%s"), "two")

    def test_every_source_dead_says_so(self):
        dead = str(Path(self.tmp.name) / "gone.git")
        git(self.repo, "remote", "set-url", "origin", dead)
        with mock.patch.dict(os.environ, {"TESSERA_MIRRORS": f"{dead} {DEAD_HTTPS}"}):
            p = self.m.position(str(self.config), fetch=True)
        self.assertIn("Couldn't reach the update server", p["error"])

    def test_a_fork_never_falls_back_to_tesseras_mirrors(self):
        dead = str(Path(self.tmp.name) / "gone.git")
        git(self.repo, "remote", "set-url", "origin", dead)
        with mock.patch.dict(os.environ, {"TESSERA_MIRRORS": ""}):
            f = self.m.fetch_branch("origin", "main", "origin/main")
        self.assertFalse(f["ok"])
        self.assertEqual(len(f["why"]), 1)  # (origin only)

    def test_fetch_command_for_tessera_update(self):
        dead = str(Path(self.tmp.name) / "gone.git")
        git(self.repo, "remote", "set-url", "origin", dead)
        env = dict(os.environ, TESSERA_MIRRORS=f"{dead} {self.good}")
        # (a copy of the updater in the checkout, as tools/tessera runs it)
        (self.repo / "tools").mkdir()
        for f in ("tessera-update.py", "tessera_sources.py", "sources.txt"):
            (self.repo / "tools" / f).write_text((ROOT / "tools" / f).read_text())
        r = subprocess.run(["python3", "-I", str(self.repo / "tools/tessera-update.py"), "fetch", "--config", str(self.config)],
                           capture_output=True, text=True, env=env, timeout=60)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertEqual(json.loads(r.stdout)["source"], str(self.good))
        self.assertEqual(git(self.repo, "log", "-1", "--format=%s", "origin/main"), "two")


class SetupScript(unittest.TestCase):
    """setup.sh: a dead first source, the clone from the next one."""

    def test_clones_from_the_first_source_that_works(self):
        with tempfile.TemporaryDirectory() as d:
            d = Path(d)
            good, seed, home, fake = d / "good.git", d / "seed", d / "home", d / "bin"
            for p in (home, fake):
                p.mkdir()
            subprocess.run(["git", "init", "-q", "--bare", "-b", "main", str(good)], check=True)
            subprocess.run(["git", "init", "-q", "-b", "main", str(seed)], check=True)
            (seed / "install.sh").write_text("#!/bin/sh\necho installer ran\n")
            (seed / "install.sh").chmod(0o755)
            git(seed, "add", "-A")
            git(seed, "commit", "-qm", "one")
            git(seed, "push", "-q", str(good), "main")
            (fake / "pacman").write_text("#!/bin/sh\nexit 0\n")
            (fake / "pacman").chmod(0o755)
            env = dict(os.environ, HOME=str(home), XDG_DATA_HOME=str(home / "data"), PATH=f"{fake}:/usr/bin:/bin",
                       TESSERA_MIRRORS=f"{DEAD_HTTPS} {d / 'gone.git'} {good}")
            env.pop("TESSERA_REPO", None)
            r = subprocess.run(["bash", str(ROOT / "setup.sh")], capture_output=True, text=True, env=env, timeout=60,
                               stdin=subprocess.DEVNULL)
            repo = home / "data/tessera/repo"
            self.assertTrue((repo / "install.sh").exists(), r.stdout + r.stderr)
            self.assertIn("trying the next source", r.stderr)
            self.assertIn(f"(from {good})", r.stdout)
            # (updates look at the first source first)
            self.assertEqual(git(repo, "remote", "get-url", "origin"), DEAD_HTTPS)
            self.assertFalse((home / "data/tessera/repo.incoming").exists())


class PacMirrors(unittest.TestCase):
    def test_mirror_list_decisions(self):
        r = subprocess.run(["bash", str(ROOT / "tests/pac_mirrors.test.sh")], capture_output=True, text=True, timeout=60)
        self.assertEqual(r.returncode, 0, r.stdout + r.stderr)
        self.assertIn("all passed", r.stdout)

    def test_called_before_the_first_install_and_in_the_plan(self):
        sh = (ROOT / "install.sh").read_text()
        body = sh[sh.index("install_packages() {"):]
        self.assertLess(body.index("pac_mirrors_check"), body.index("pac -Syu"))
        self.assertIn('plan_row Mirrors "re-ranked first', sh)
        tty = (ROOT / "tools/setup-tty.sh").read_text()
        self.assertLess(tty.index("pac_mirrors_check"), tty.index("pac -S --needed"))

    def test_software_update_asks_and_root_refreshes(self):
        su = (ROOT / "shell/assets/scripts/sysupdate.py").read_text()
        self.assertIn('plan["mirrors"] = stale_mirrors()', su)
        root = load("tessera_root_mirrors", ROOT / "shell/assets/scripts/tessera-root.py")
        p = root.parse_update(["--system", "--mirrors", "/etc/pacman.d/mirrorlist,/etc/pacman.d/cachyos-v3-mirrorlist"])
        self.assertEqual(p["mirrors"], ["/etc/pacman.d/mirrorlist", "/etc/pacman.d/cachyos-v3-mirrorlist"])
        for bad in ("/etc/passwd", "/etc/pacman.d/../shadow", "/tmp/mirrorlist"):
            with self.assertRaises(root.Refused):
                root.parse_update(["--mirrors", bad])

    def test_root_refresh_backs_up_once_and_uses_fixed_commands(self):
        root = load("tessera_root_mirrors2", ROOT / "shell/assets/scripts/tessera-root.py")
        with tempfile.TemporaryDirectory() as d:
            lst = Path(d) / "etc/pacman.d/mirrorlist"
            lst.parent.mkdir(parents=True)
            lst.write_text("Server = old\n")
            ran, said = [], []

            def run(cmd, **kw):
                ran.append(cmd)
                return subprocess.CompletedProcess(cmd, 0)
            ok = root.refresh_mirrors(["/etc/pacman.d/mirrorlist"], said.append, None, prefix=d, run=run,
                                      which=lambda n: "/usr/bin/" + n if n == "reflector" else None)
            self.assertTrue(ok)
            self.assertEqual((Path(d) / "etc/pacman.d/mirrorlist.tessera-backup").read_text(), "Server = old\n")
            self.assertEqual(ran[0][:3], ["reflector", "--latest", "20"])
            lst.write_text("Server = newer\n")
            root.refresh_mirrors(["/etc/pacman.d/mirrorlist"], said.append, None, prefix=d, run=run,
                                 which=lambda n: "/usr/bin/" + n)
            self.assertEqual(ran[-1], ["cachyos-rate-mirrors"])
            self.assertEqual((Path(d) / "etc/pacman.d/mirrorlist.tessera-backup").read_text(), "Server = old\n")


class Workflow(unittest.TestCase):
    def test_gitlab_mirror_skips_without_its_secret(self):
        wf = (ROOT / ".github/workflows/gitlab-mirror.yml").read_text()
        self.assertIn("GITLAB_TOKEN", wf)
        self.assertRegex(wf, r"branches: \[main, beta\]")
        self.assertIn("::notice", wf)  # (no secret: a notice, not a failure)
        ui = (ROOT / ".github/workflows/tessera-ui.yml").read_text()
        self.assertIn("packages/generic/tessera-ui", ui)
        # (the signatures go too: installs from GitLab check them the same way)
        self.assertRegex(ui, r"for f in out/\*")


if __name__ == "__main__":
    unittest.main()
