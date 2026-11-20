"""tools/tessera-update.py, the 20 Nov audit's fixes, each on its own with a
throwaway repository and state folder (no network, no real install):

  #36  defaults Config.qml took over from the Rust shell ask nothing at an
       update (asking would write the old default and change the shell)
  #37  a take() that fails is put back and leaves no journal (the next
       start's recover() would reset the checkout over newer edits)
  #93  the journal is done once the checkout and programs are in: the parts
       and the rename after that are best effort
  #96  tools/settings-migrations.json is there, and well formed
  #129 Cancel never signals a pid that's no longer the job's
  #132 a job cut off with nothing to resume ends with a fresh status
"""
import importlib.machinery
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
ID = ["-c", "user.name=Test", "-c", "user.email=test@example.invalid", "-c", "commit.gpgsign=false"]


def load():
    name = "tu_audit_" + os.urandom(3).hex()
    loader = importlib.machinery.SourceFileLoader(name, str(ROOT / "tools/tessera-update.py"))
    spec = importlib.util.spec_from_loader(name, loader)
    mod = importlib.util.module_from_spec(spec)
    loader.exec_module(mod)
    return mod


def git(repo, *args):
    r = subprocess.run(["git", *ID, "-C", str(repo), *args], capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError(" ".join(args) + ": " + r.stderr)
    return r.stdout.strip()


CONFIG_OLD = """Singleton {
    JsonAdapter {
        property JsonObject wallpaper: JsonObject {
            property bool livePauseOnBattery: true
        }
        property JsonObject dock: JsonObject {
            property bool magnify: true
        }
    }
}
"""
CONFIG_NEW = CONFIG_OLD.replace("livePauseOnBattery: true", "livePauseOnBattery: false").replace("magnify: true", "magnify: false")


class Repo(unittest.TestCase):
    """A remote at "two" (Config.qml's defaults changed), a clone at "one"
    with an edit of yours"""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        t = Path(self.tmp.name)
        self.remote, seed, self.repo, self.config = t / "remote.git", t / "seed", t / "copy", t / "config"
        self.config.mkdir()
        subprocess.run(["git", "init", "-q", "--bare", "-b", "main", str(self.remote)], check=True)
        subprocess.run(["git", "init", "-q", "-b", "main", str(seed)], check=True)
        (seed / "shell/common").mkdir(parents=True)
        (seed / "shell/common/Config.qml").write_text(CONFIG_OLD)
        (seed / "one.txt").write_text("one")
        git(seed, "add", "-A")
        git(seed, "commit", "-qm", "one")
        git(seed, "remote", "add", "origin", str(self.remote))
        git(seed, "push", "-q", "origin", "main")
        subprocess.run(["git", "clone", "-q", "--branch", "main", "--single-branch", str(self.remote), str(self.repo)], check=True)
        (seed / "shell/common/Config.qml").write_text(CONFIG_NEW)
        (seed / "two.txt").write_text("two")
        git(seed, "add", "-A")
        git(seed, "commit", "-qm", "two")
        git(seed, "push", "-q", "origin", "main")
        self.env = mock.patch.dict(os.environ, {"XDG_STATE_HOME": str(t / "state"), "XDG_DATA_HOME": str(t / "data"),
                                                "XDG_CACHE_HOME": str(t / "cache"), "XDG_RUNTIME_DIR": str(t / "run"),
                                                "TESSERA_MIRRORS": ""})
        self.env.start()
        self.m = load()
        self.m.REPO = str(self.repo)
        self.m.native_plan = lambda *a, **k: None
        self.m.native_install = lambda *a, **k: {"ok": True, "changed": False}
        stub = t / "ni-stub.py"
        stub.write_text("print('{}')\n")
        self.m.NATIVE = str(stub)
        (self.config / "config.json").write_text(json.dumps({"update": {"channel": "stable"}}))

    def tearDown(self):
        self.env.stop()
        self.tmp.cleanup()

    def head(self):
        return git(self.repo, "log", "-1", "--format=%s")

    def test_aligned_defaults_ask_nothing_others_still_do(self):
        r = self.m.check(str(self.config))
        self.assertNotIn("error", r)
        ids = [c["id"] for c in r["conflicts"]]
        self.assertIn("setting:dock.magnify", ids)
        self.assertNotIn("setting:wallpaper.livePauseOnBattery", ids, "the Rust shell already did false: nothing changes")

    def test_a_take_that_fails_is_put_back_with_no_journal_left(self):
        self.m.check(str(self.config))
        (self.repo / "one.txt").write_text("my edit")

        def boom(report, undo=None):
            undo.update({"here": "main", "branch": "main", "had": None, "upstream": None})
            raise RuntimeError("git checkout refused")
        self.m.take = boom
        with self.assertRaises(RuntimeError):
            self.m.apply(str(self.config), {"setting:dock.magnify": "new"})
        self.assertFalse(os.path.exists(self.m.journal_path()), "nothing left for recover() to redo")
        self.assertEqual(self.head(), "one")
        self.assertEqual((self.repo / "one.txt").read_text(), "my edit")
        # (you keep working; the next start finds nothing to put back)
        (self.repo / "one.txt").write_text("my newer edit")
        self.assertEqual(self.m.recover(), {"recovered": False})
        self.assertEqual((self.repo / "one.txt").read_text(), "my newer edit")

    def test_a_merge_that_no_longer_fits_leaves_no_journal(self):
        self.m.check(str(self.config))
        self.m.three_way = lambda f, new: None
        (self.repo / "one.txt").write_text("my edit")
        with self.assertRaises(SystemExit):
            self.m.apply(str(self.config), {"setting:dock.magnify": "new", "code:one.txt": "both"})
        self.assertFalse(os.path.exists(self.m.journal_path()))

    def test_the_journal_is_done_before_the_parts(self):
        self.m.check(str(self.config))
        seen = []

        def rebuild(todo, *a, **k):
            seen.append(os.path.exists(self.m.journal_path()))
            return [], []
        self.m.rebuild = rebuild
        r = self.m.apply(str(self.config), {"setting:dock.magnify": "new"})
        self.assertTrue(r["applied"], r)
        self.assertEqual(self.head(), "two")
        self.assertEqual(seen, [False], "the commit point is before the parts")


class Migrations(unittest.TestCase):
    def test_the_file_is_there_and_points_at_settings_that_exist(self):
        m = load()
        path = ROOT / m.MIGRATIONS
        self.assertTrue(path.exists())
        moves = json.loads(path.read_text())
        self.assertIsInstance(moves, dict)
        now = m.config_defaults((ROOT / m.CONFIG_QML).read_text())
        for old, new in moves.items():
            self.assertNotIn(old, now, f"{old} still exists: no move needed")
            self.assertIn(new, now, f"{old} moves to {new}, which isn't a setting")

    def test_aligned_defaults_are_settings(self):
        m = load()
        now = m.config_defaults((ROOT / m.CONFIG_QML).read_text())
        for key in m.ALIGNED_DEFAULTS:
            self.assertIn(key, now)


class Jobs(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.env = mock.patch.dict(os.environ, {"XDG_STATE_HOME": self.tmp.name})
        self.env.start()
        self.m = load()
        stub = Path(self.tmp.name) / "ni-stub.py"
        stub.write_text("print('{}')\n")
        self.m.NATIVE = str(stub)
        self.d = Path(self.m.job_dir())
        self.d.mkdir(parents=True)

    def tearDown(self):
        self.env.stop()
        self.tmp.cleanup()

    def test_the_pid_file_carries_the_start_time(self):
        job = self.m.Job()
        pid, start = self.m.read_job_pid()
        self.assertEqual(pid, os.getpid())
        self.assertEqual(start, self.m.proc_start(os.getpid()))
        job.end("done")
        self.assertIsNone(self.m.read_job_pid())
        (self.d / "pid").write_text("4242")  # (an older job's file: the pid alone)
        self.assertEqual(self.m.read_job_pid(), (4242, None))

    def test_only_the_jobs_own_group_is_ours(self):
        ours = self.m.job_group_is_ours
        updater = lambda p: "python3 -I /x/tools/tessera-update.py stage"  # noqa: E731
        other = lambda p: "/usr/bin/foot"  # noqa: E731
        self.assertTrue(ours(10, 500, started=lambda p: 500, cmdline=updater))
        self.assertFalse(ours(10, 500, started=lambda p: 900, cmdline=updater), "the pid was given to another process")
        self.assertFalse(ours(10, None, started=lambda p: 900, cmdline=other), "not an updater")
        self.assertTrue(ours(10, None, started=lambda p: 900, cmdline=updater))
        self.assertTrue(ours(10, 500, started=lambda p: None, cmdline=other), "gone: its group, if any, is the job's")

    def test_cancel_never_signals_a_reused_pid(self):
        p = subprocess.Popen(["sleep", "30"], start_new_session=True)
        try:
            (self.d / "pid").write_text(f"{p.pid} 1")  # (a stale file: its start time isn't sleep's)
            (self.d / "status.json").write_text(json.dumps({"state": "running", "pid": p.pid, "started": 1}))
            self.assertTrue(self.m.cancel_job(grace=0.2))
            time.sleep(0.2)
            self.assertIsNone(p.poll(), "the other program is left alone")
            self.assertEqual(json.loads((self.d / "status.json").read_text())["state"], "cancelled")
            self.assertFalse((self.d / "pid").exists())
        finally:
            p.kill()
            p.wait()

    def test_a_cut_off_job_with_nothing_to_resume_ends_freshly_dated(self):
        (self.d / "status.json").write_text(json.dumps({"state": "running", "pid": 999999, "started": 1}))

        class A:
            rebuild = ""
            channel = None
            config = ""
        t = time.time()
        out = self.m.resume(A())
        self.assertFalse(out["resumed"])
        st = json.loads((self.d / "status.json").read_text())
        self.assertEqual(st["state"], "error")
        self.assertGreaterEqual(st["started"], t - 1, "newer than the shell's ask: it's seen")
        # (once ended, a second start doesn't pick it up again)
        self.assertEqual(self.m.resume(A()), {"resumed": False})


if __name__ == "__main__":
    unittest.main()
