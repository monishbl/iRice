"""Going back to before an update: what each run changed is written down
(sysupdate.py), going back finds the earlier package files, and the root
helper only installs package files from where they're kept."""
import importlib.util
import os
import tempfile
import unittest
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent.parent / "shell/assets/scripts"


def load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


class History(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.su = load(SCRIPTS / "sysupdate.py", "sysupdate_undo")
        self.su.STATE_DIR = self.dir.name
        self.su.HISTORY = os.path.join(self.dir.name, "updates.json")

    def tearDown(self):
        self.dir.cleanup()

    def update(self, before, after, kind="update", at=None):
        return self.su.record(before, after, kind, at=at)

    def test_changes_are_written_down(self):
        e = self.update({"mesa": "1-1", "old": "2-1", "same": "1-1"}, {"mesa": "2-1", "new": "1-1", "same": "1-1"}, at=100)
        self.assertEqual(e["changes"], [["mesa", "1-1", "2-1"], ["new", None, "1-1"], ["old", "2-1", None]])
        self.assertIsNone(self.update({"a": "1"}, {"a": "1"}))  # (nothing changed: nothing written)
        self.assertEqual(len(self.su.history()["updates"]), 1)

    def test_only_the_last_few_are_kept(self):
        for i in range(8):
            self.update({"a": str(i)}, {"a": str(i + 1)}, at=i)
        self.assertEqual([u["at"] for u in self.su.history()["updates"]], [3, 4, 5, 6, 7])

    def test_plan_finds_files_and_leaves_added_packages(self):
        self.update({"mesa": "1-1", "old": "2-1"}, {"mesa": "2-1", "new": "1-1"}, at=100)
        files = {("mesa", "1-1"): "/var/cache/pacman/pkg/mesa-1-1-x86_64.pkg.tar.zst"}
        plan = self.su.undo_plan(installed={"mesa": "2-1", "new": "1-1"}, files=files, archive=lambda n, v: "")
        self.assertEqual(plan["at"], 100)
        self.assertEqual([p["name"] for p in plan["packages"]], ["mesa", "old"])  # (new stays)
        self.assertEqual(plan["files"], [files[("mesa", "1-1")]])
        self.assertEqual(plan["missing"], ["old 2-1"])

    def test_archive_when_the_cache_lost_it(self):
        self.update({"mesa": "1-1"}, {"mesa": "2-1"})
        url = "https://archive.archlinux.org/packages/m/mesa/mesa-1-1-x86_64.pkg.tar.zst"
        plan = self.su.undo_plan(installed={"mesa": "2-1"}, files={}, archive=lambda n, v: url)
        self.assertEqual((plan["files"], plan["missing"]), ([url], []))

    def test_going_back_marks_it_and_skips_the_bad_versions(self):
        self.update({"mesa": "1-1"}, {"mesa": "2-1"}, at=100)
        plan = self.su.undo_plan(installed={"mesa": "2-1"}, files={("mesa", "1-1"): "/x"}, archive=lambda n, v: "")
        self.su.gone_back(plan["at"], plan["packages"])
        self.update({"mesa": "2-1"}, {"mesa": "1-1"}, kind="undo", at=101)
        h = self.su.history()
        self.assertEqual(h["skip"], {})  # (the undo run changed mesa: …)
        self.su.gone_back(100, plan["packages"])  # (… so it's written after, as run() does)
        h = self.su.history()
        self.assertEqual(h["skip"], {"mesa": "2-1"})
        self.assertIsNone(self.su.last_update(h))  # (nothing more to go back from)
        # A newer mesa arrives in a later update: no longer skipped
        self.update({"mesa": "1-1"}, {"mesa": "3-1"}, at=102)
        h = self.su.history()
        self.assertEqual(h["skip"], {})
        self.assertEqual(self.su.last_update(h)["at"], 102)

    def test_installs_alone_are_not_updates(self):
        self.update({}, {"whisper-cpp": "1-1"}, kind="install")
        self.assertEqual(self.su.undo_plan(installed={}, files={}, archive=lambda n, v: ""), {})

    def test_cache_folders_are_read_with_yay_subfolders(self):
        d = Path(self.dir.name)
        (d / "pkg").mkdir()
        (d / "pkg/mesa-1.0-1-x86_64.pkg.tar.zst").write_text("")
        (d / "pkg/mesa-1.0-1-x86_64.pkg.tar.zst.sig").write_text("")
        (d / "yay/apple_cursor").mkdir(parents=True)
        (d / "yay/apple_cursor/apple_cursor-2.0.1-1-any.pkg.tar.zst").write_text("")
        found = self.su.package_files([str(d / "pkg"), str(d / "yay")])
        self.assertEqual(set(found), {("mesa", "1.0-1"), ("apple_cursor", "2.0.1-1")})


class RootChecksFiles(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.r = load(SCRIPTS / "irice-root.py", "irice_root_undo")
        self.cache = Path(self.dir.name, "cache")
        self.cache.mkdir()
        self.ok = self.cache / "mesa-1.0-1-x86_64_v3.pkg.tar.zst"
        self.ok.write_text("")

    def tearDown(self):
        self.dir.cleanup()

    def test_from_the_cache_or_the_archive(self):
        p = self.r.parse_update(["--downgrade", str(self.ok) + ",https://archive.archlinux.org/packages/m/mesa/mesa-1.0-1-x86_64.pkg.tar.zst"],
                                [str(self.cache)])
        self.assertEqual(p["downgrade"][0], str(self.ok))

    def test_refused_elsewhere(self):
        other = Path(self.dir.name, "mesa-1.0-1-x86_64.pkg.tar.zst")
        other.write_text("")
        for bad in (str(other), str(self.cache / "../mesa-1.0-1-x86_64.pkg.tar.zst"), str(self.cache / "notes.txt"),
                    "https://example.com/packages/m/mesa/mesa-1.0-1-x86_64.pkg.tar.zst", "--overwrite=*", "relative.pkg.tar.zst"):
            with self.assertRaises(self.r.Refused, msg=bad):
                self.r.parse_update(["--downgrade", bad], [str(self.cache)])


if __name__ == "__main__":
    unittest.main()


class IncomingHelper(unittest.TestCase):
    """The update's root step installs the helper as the version about to
    be installed has it (git REF), not this folder's older copy."""

    def test_staged_from_the_ref(self):
        import subprocess
        su = load(SCRIPTS / "sysupdate.py", "sysupdate_incoming")
        with tempfile.TemporaryDirectory() as d:
            repo = Path(d, "repo")
            files = [rel for rel, _, _ in su.root_helper().FILES]
            for rel in files:
                p = repo / "shell" / rel
                p.parent.mkdir(parents=True, exist_ok=True)
                p.write_text("old " + rel)
            git = lambda *a: subprocess.run(["git", "-C", str(repo), *a], capture_output=True, check=True)
            git("init", "-q")
            git("-c", "user.email=t@t", "-c", "user.name=t", "add", "-A")
            git("-c", "user.email=t@t", "-c", "user.name=t", "commit", "-qm", "old")
            (repo / "shell" / files[0]).write_text("new")
            git("-c", "user.email=t@t", "-c", "user.name=t", "commit", "-qam", "new")
            git("checkout", "-q", "HEAD~1")  # (the folder is still the old version)
            su.SHELL = str(repo / "shell")
            su.DIR = str(Path(d, "run"))
            os.makedirs(su.DIR)
            out = su.stage_incoming("master") or su.stage_incoming("main")
            self.assertIsNotNone(out)
            self.assertEqual(Path(out, files[0]).read_text(), "new")
            self.assertEqual(Path(out, files[1]).read_text(), "old " + files[1])
            self.assertIsNone(su.stage_incoming("--upload-pack=x"))  # (not a ref: refused)
            self.assertIsNone(su.stage_incoming("no-such-ref"))


class HelperCurrent(unittest.TestCase):
    def test_a_file_only_root_may_read_counts_as_current(self):
        su = load(SCRIPTS / "sysupdate.py", "sysupdate_current")
        with tempfile.TemporaryDirectory() as d:
            su.SYSTEM = d
            for rel, dest, mode in su.root_helper().FILES:
                p = Path(d, dest.lstrip("/"))
                p.parent.mkdir(parents=True, exist_ok=True)
                p.write_bytes((SCRIPTS.parent.parent / rel).read_bytes())
            self.assertTrue(su.helper_current())
            rules = Path(d, "etc/polkit-1/rules.d")
            rules.chmod(0o000)  # (as on a real system: 0700 root)
            try:
                self.assertTrue(su.helper_current())
                Path(d, "usr/lib/irice/irice-root").write_text("older")
                self.assertFalse(su.helper_current())
            finally:
                rules.chmod(0o755)
