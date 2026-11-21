"""tools/rust-defaults.json (20 Nov audit #96): every settings default the
Rust shell reads, written from its sources by tools/rust-defaults.py. It
fails here when it drifts from the sources (run tools/rust-defaults.py
--write), and the updater's conflict check compares it between versions,
so a setting only the Rust shell has is a question too when its default
changes. Throwaway repositories only."""
import importlib.machinery
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


def load(path, name):
    loader = importlib.machinery.SourceFileLoader(name + os.urandom(3).hex(), str(path))
    spec = importlib.util.spec_from_loader(loader.name, loader)
    mod = importlib.util.module_from_spec(spec)
    loader.exec_module(mod)
    return mod


RD = load(ROOT / "tools/rust-defaults.py", "rust_defaults_")


def git(repo, *args):
    r = subprocess.run(["git", *ID, "-C", str(repo), *args], capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError(" ".join(args) + ": " + r.stderr)
    return r.stdout.strip()


class TheFile(unittest.TestCase):
    def test_it_is_what_the_rust_sources_read(self):
        defaults, clashes = RD.scan()
        self.assertEqual(clashes, [], "one setting read with two defaults")
        on_disk = json.loads((ROOT / "tools/rust-defaults.json").read_text())
        self.assertEqual(on_disk, defaults, "tools/rust-defaults.json is behind the Rust sources: run tools/rust-defaults.py --write")
        self.assertEqual((ROOT / "tools/rust-defaults.json").read_text(), RD.text_of(defaults))

    def test_the_scan_sees_each_way_readers_are_written(self):
        found = {(rel, key) for rel, _, key, _ in RD.readers()}
        keys = {k for _, k in found}
        self.assertIn("dock.iconSize", keys)  # cfg["dock"]["iconSize"]…unwrap_or
        self.assertTrue(any(rel == "host/shell/src/sounds.rs" for rel, _ in found))  # cfg_num(…)
        # (let a = &cfg["appearance"]; a["…"]: a section through a name)
        self.assertTrue(any(rel == "surfaces/controlcenter/src/defs.rs" and k == "appearance.glass" for rel, k in found))
        self.assertGreater(len(keys), 100)

    def test_a_reader_through_a_name_is_read_as_its_section(self):
        with tempfile.TemporaryDirectory() as d:
            src = Path(d) / "x.rs"
            src.write_text('fn f(cfg: &Value) {\n    let a = &cfg["appearance"];\n    let r = a["onlyRust"].as_f64().unwrap_or(0.25);\n'
                           '    let b = cfg["dock"]["rustSize"].as_i64().unwrap_or(7);\n    let c = cfg_bool(cfg, "dock.rustFlag", true);\n}\n')
            got = {k: v for _, _, k, v in RD.readers(d, secs={"appearance", "dock"})}
        self.assertEqual(got, {"appearance.onlyRust": 0.25, "dock.rustSize": 7, "dock.rustFlag": True})

    def test_it_agrees_with_config_qml_where_both_have_a_setting(self):
        tu = load(ROOT / "tools/tessera-update.py", "tu_rd_")
        qml = tu.config_defaults((ROOT / "shell/common/Config.qml").read_text())
        rust = json.loads((ROOT / "tools/rust-defaults.json").read_text())
        wrong = [f"{k}: Rust {v!r}, Config.qml {qml[k]!r}" for k, v in rust.items() if k in qml and not RD.same(v, qml[k])]
        self.assertEqual(wrong, [])


CONFIG = """Singleton {
    JsonAdapter {
        property JsonObject dock: JsonObject {
            property bool magnify: true
        }
    }
}
"""


class Conflicts(unittest.TestCase):
    """check(): a setting only the Rust shell has whose default changes"""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        t = Path(self.tmp.name)
        self.remote, seed, self.repo, self.config = t / "remote.git", t / "seed", t / "copy", t / "config"
        self.config.mkdir()
        subprocess.run(["git", "init", "-q", "--bare", "-b", "main", str(self.remote)], check=True)
        subprocess.run(["git", "init", "-q", "-b", "main", str(seed)], check=True)
        (seed / "shell/common").mkdir(parents=True)
        (seed / "tools").mkdir()
        (seed / "shell/common/Config.qml").write_text(CONFIG)
        (seed / "tools/rust-defaults.json").write_text(json.dumps({"dock.magnify": True, "dock.rustOnly": 1.0, "dock.rustGone": "a"}))
        git(seed, "add", "-A")
        git(seed, "commit", "-qm", "one")
        git(seed, "remote", "add", "origin", str(self.remote))
        git(seed, "push", "-q", "origin", "main")
        subprocess.run(["git", "clone", "-q", "--branch", "main", "--single-branch", str(self.remote), str(self.repo)], check=True)
        self.seed = seed
        self.env = mock.patch.dict(os.environ, {"XDG_STATE_HOME": str(t / "state"), "XDG_DATA_HOME": str(t / "data"),
                                                "XDG_CACHE_HOME": str(t / "cache"), "XDG_RUNTIME_DIR": str(t / "run"),
                                                "XDG_CONFIG_HOME": str(t / "xdgconfig"), "TESSERA_MIRRORS": ""})
        self.env.start()
        self.m = load(ROOT / "tools/tessera-update.py", "tu_conf_")
        self.m.REPO = str(self.repo)
        self.m.native_plan = lambda *a, **k: None
        (self.config / "config.json").write_text(json.dumps({"update": {"channel": "stable"}}))

    def tearDown(self):
        self.env.stop()
        self.tmp.cleanup()

    def publish(self, rust):
        (self.seed / "tools/rust-defaults.json").write_text(json.dumps(rust))
        git(self.seed, "commit", "-qam", "two")
        git(self.seed, "push", "-q", "origin", "main")

    def test_a_rust_only_default_that_changes_is_asked(self):
        self.publish({"dock.magnify": True, "dock.rustOnly": 2.0, "dock.rustGone": "a"})
        r = self.m.check(str(self.config))
        self.assertNotIn("error", r)
        c = {c["id"]: c for c in r["conflicts"]}
        self.assertIn("setting:dock.rustOnly", c)
        self.assertEqual((c["setting:dock.rustOnly"]["old"], c["setting:dock.rustOnly"]["new"]), (1.0, 2.0))

    def test_one_you_set_yourself_asks_nothing(self):
        (self.config / "config.json").write_text(json.dumps({"update": {"channel": "stable"}, "dock": {"rustOnly": 5}}))
        self.publish({"dock.magnify": True, "dock.rustOnly": 2.0, "dock.rustGone": "a"})
        ids = [c["id"] for c in self.m.check(str(self.config))["conflicts"]]
        self.assertNotIn("setting:dock.rustOnly", ids)

    def test_a_rust_only_setting_dropped_that_you_changed(self):
        (self.config / "config.json").write_text(json.dumps({"update": {"channel": "stable"}, "dock": {"rustGone": "b"}}))
        self.publish({"dock.magnify": True, "dock.rustOnly": 1.0})
        ids = [c["id"] for c in self.m.check(str(self.config))["conflicts"]]
        self.assertIn("removed:dock.rustGone", ids)

    def test_config_qml_wins_where_it_has_the_setting(self):
        # (the two agree there: test_settings_defaults.py; Config.qml's is the one asked about)
        self.publish({"dock.magnify": False, "dock.rustOnly": 1.0, "dock.rustGone": "a"})
        ids = [c["id"] for c in self.m.check(str(self.config))["conflicts"]]
        self.assertNotIn("setting:dock.magnify", ids)

    def test_a_version_without_the_file_reads_config_qml_only(self):
        git(self.seed, "rm", "-q", "tools/rust-defaults.json")
        git(self.seed, "commit", "-qm", "two")
        git(self.seed, "push", "-q", "origin", "main")
        r = self.m.check(str(self.config))
        self.assertEqual([c["id"] for c in r["conflicts"]], [])  # (settings left as they were: nothing to ask)
        self.assertEqual(self.m.setting_defaults(r["target"]), {"dock.magnify": True})


if __name__ == "__main__":
    unittest.main()
