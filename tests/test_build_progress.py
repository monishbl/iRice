"""How far a build is (tools/build-progress.py): cargo's messages counted
against an exact or estimated total, the crate it's on, the link phase,
the ETA from a previous build, the progress file, the log and the stats;
a failure's readable tail. A fake cargo stands in for the real one."""
import importlib.util
import io
import json
import os
import stat
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TOOL = ROOT / "tools/build-progress.py"
spec = importlib.util.spec_from_file_location("build_progress", TOOL)
bp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bp)


def art(name, kind="lib", fresh=False, pkg=None):
    return {"reason": "compiler-artifact", "package_id": "registry+https://github.com/rust-lang/crates.io-index#{}@1.0.0".format(pkg or name),
            "target": {"name": name.replace("-", "_"), "kind": [kind]}, "fresh": fresh}


class Counting(unittest.TestCase):
    def test_units_crate_and_the_link(self):
        p = bp.Progress("tessera-ui", final="tessera-ui", total=4, exact=True, now=0, title="the shell")
        self.assertEqual(p.phase, "prepare")
        p.feed_line("    Updating crates.io index", now=1)
        p.feed_line("  Downloaded slint v1.9.0", now=2)
        self.assertEqual((p.phase, p.downloaded), ("download", 1))
        p.feed_line("   Compiling serde v1.0.0", now=3)
        p.feed_line("   Compiling slint v1.9.0", now=3)
        self.assertEqual((p.phase, p.crate), ("compile", "slint"))
        p.feed_json(art("serde"), now=5)
        self.assertEqual(p.crate, "slint")
        self.assertEqual(p.step_text(), "Compiling slint (1 of 4)")
        p.feed_line("   Compiling tessera-ui v0.1.0 (/x/native/tessera-ui)", now=6)
        p.feed_json(art("slint"), now=8)
        # (only the program is left: that's the link)
        self.assertEqual(p.phase, "link")
        self.assertEqual(p.step_text(), "Linking the shell (the longest step, a few minutes)")
        # (its lib isn't the program: still linking)
        p.feed_json(art("tessera-ui", "lib", pkg="tessera-ui"), now=9)
        self.assertEqual(p.phase, "link")
        p.feed_json(art("tessera-ui", "bin", pkg="tessera-ui"), now=200)
        p.finish(0, now=201)
        self.assertEqual((p.phase, p.total, p.exact, p.fraction()), ("done", 4, True, 1.0))
        h = p.history_after()
        self.assertEqual(h["units"], 4)
        self.assertAlmostEqual(h["link_s"], 193.0)
        self.assertIn("profile", h)  # (all built, nothing fresh: a full build)

    def test_an_estimate_says_about_and_claims_no_percent(self):
        p = bp.Progress("x", total=2, exact=False, now=0)
        p.feed_line("   Compiling a v1.0.0", now=1)
        for _ in range(5):
            p.feed_json(art("a"), now=2)
        self.assertIn("of about 2", p.step_text())
        self.assertIsNone(p.fraction())  # (unsure: a count, not a made-up percent)
        p.exact = True
        self.assertEqual(p.fraction(), 0.99)  # (exact, but cargo said more: never 100 before it ends)

    def test_no_total_counts_only(self):
        p = bp.Progress("x", now=0)
        p.feed_line("   Compiling a v1.0.0", now=1)
        p.feed_json(art("a"), now=2)
        self.assertIsNone(p.fraction())
        self.assertEqual(p.step_text(), "Compiling a (1 built)")

    def test_warnings_counted_errors_kept(self):
        p = bp.Progress("x", now=0)
        for line in ["warning: unused import: `Path`", "  --> src/lib.rs:2:5", "warning: `x` (lib) generated 1 warning",
                     "error[E0308]: mismatched types", "error: could not compile `x` (lib) due to 1 previous error"]:
            p.feed_line(line)
        self.assertEqual(p.warnings, 1)
        self.assertEqual(p.errors, ["error[E0308]: mismatched types"])

    def test_package_names_from_ids(self):
        self.assertEqual(bp.Progress._package("registry+https://github.com/rust-lang/crates.io-index#slint@1.9.0"), "slint")
        self.assertEqual(bp.Progress._package("path+file:///home/a/native/tessera-ui#0.1.0"), "tessera-ui")
        self.assertEqual(bp.Progress._package("path+file:///home/a/x#tessera-core@0.1.0"), "tessera-core")
        self.assertEqual(bp.Progress._package("slint 1.9.0 (registry+https://…)"), "slint")


class Eta(unittest.TestCase):
    HISTORY = {"units": 100, "compile_s": 400.0, "link_s": 200.0, "profile": {"0": 0.0, "50": 100.0, "95": 190.0}}

    def build(self):
        p = bp.Progress("x", final="x", total=100, exact=True, history=self.HISTORY, now=0)
        p.feed_line("   Compiling a v1.0.0", now=0)
        return p

    def test_none_without_history(self):
        p = bp.Progress("x", total=100, exact=True, now=0)
        p.feed_line("   Compiling a v1.0.0", now=0)
        for _ in range(50):
            p.feed_json(art("a"), now=10)
        self.assertIsNone(p.eta(now=100))

    def test_a_full_build_as_fast_as_last_time(self):
        p = self.build()
        for _ in range(50):
            p.feed_json(art("b"), now=50)
        # 50 % after 100 s, as last time: 300 s to go
        self.assertAlmostEqual(p.eta(now=100), 300.0)
        # twice as slow so far: twice as long to go
        self.assertAlmostEqual(p.eta(now=200), 600.0)
        # (and the bar goes by time then, as the ETA does)
        self.assertAlmostEqual(p.fraction(now=100, eta=300.0), 0.25)

    def test_a_partial_build_only_knows_the_link(self):
        p = self.build()
        for _ in range(60):
            p.feed_json(art("b", fresh=True), now=1)
        self.assertIsNone(p.eta(now=10))
        p.feed_line("   Compiling x v0.1.0", now=10)
        p.flying.pop("a", None)
        p._phase(10)
        self.assertEqual(p.phase, "link")
        self.assertAlmostEqual(p.eta(now=70), 140.0)
        # (past last time's link: no made-up number)
        self.assertIsNone(p.eta(now=300))
        p2 = bp.Progress("x", final="x", total=100, exact=True, now=0)
        p2.feed_line("   Compiling x v0.1.0", now=1)
        self.assertEqual(p2.phase, "link")
        self.assertIsNone(p2.fraction(now=5))  # (no history: the bar sweeps while linking)


class Estimate(unittest.TestCase):
    def test_metadata_units(self):
        def pkg(name, kinds):
            return {"id": name, "name": name, "targets": [{"kind": [k]} for k in kinds]}
        meta = {"packages": [pkg("app", ["bin", "lib"]), pkg("serde", ["lib", "custom-build"]), pkg("derive", ["proc-macro"]),
                             pkg("syn", ["lib"]), pkg("cc", ["lib"]), pkg("devonly", ["lib"])],
                "resolve": {"root": "app", "nodes": [
                    {"id": "app", "deps": [{"pkg": "serde", "dep_kinds": [{"kind": None}]}, {"pkg": "derive", "dep_kinds": [{"kind": None}]},
                                           {"pkg": "cc", "dep_kinds": [{"kind": "build"}]}, {"pkg": "devonly", "dep_kinds": [{"kind": "dev"}]}]},
                    {"id": "serde", "deps": []}, {"id": "derive", "deps": [{"pkg": "syn", "dep_kinds": [{"kind": None}]}]},
                    {"id": "syn", "deps": []}, {"id": "cc", "deps": []}, {"id": "devonly", "deps": []}]}}
        # app (lib), serde, derive, syn (for the macro: the build machine's),
        # cc (a build dependency), serde's build script, and the program
        self.assertEqual(bp.estimate_units(meta), 7)
        self.assertIsNone(bp.estimate_units({"packages": [], "resolve": None}))


FAKE_CARGO = """#!/usr/bin/env python3
import json, os, sys, time
fail = os.environ.get("FAKE_FAIL") == "1"
units = ["serde", "slint", "tessera-core"]
for u in units:
    print("   Compiling {} v1.0.0".format(u), file=sys.stderr, flush=True)
    print(json.dumps({"reason": "compiler-artifact", "package_id": "registry+x#{}@1.0.0".format(u), "target": {"name": u, "kind": ["lib"]}, "fresh": False}), flush=True)
print("   Compiling tessera-ui v0.1.0 (/x)", file=sys.stderr, flush=True)
if fail:
    for i in range(50):
        print("note: context line {}".format(i), file=sys.stderr)
    print("error[E0308]: mismatched types", file=sys.stderr)
    print("  --> src/main.rs:3:5", file=sys.stderr)
    print("error: could not compile `tessera-ui` (bin \\"tessera-ui\\") due to 1 previous error", file=sys.stderr, flush=True)
    sys.exit(101)
time.sleep(0.3)
print(json.dumps({"reason": "compiler-artifact", "package_id": "path+file:///x/tessera-ui#0.1.0", "target": {"name": "tessera-ui", "kind": ["bin"]}, "fresh": False}), flush=True)
print(json.dumps({"reason": "build-finished", "success": True}), flush=True)
"""


class Run(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.d = Path(self.tmp.name)
        cargo = self.d / "cargo"
        cargo.write_text(FAKE_CARGO)
        cargo.chmod(cargo.stat().st_mode | stat.S_IEXEC)
        (self.d / "proj").mkdir()
        (self.d / "proj/Cargo.toml").write_text("[package]\nname = 'tessera-ui'\n")
        (self.d / "proj/Cargo.lock").write_text("# lock\n")
        (self.d / "target").mkdir()
        self.env = dict(os.environ, XDG_STATE_HOME=str(self.d / "state"), PATH=str(self.d) + ":" + os.environ["PATH"], TESSERA_PROGRESS="lines")
        self.env.pop("TESSERA_PROGRESS_TTY", None)

    def tearDown(self):
        self.tmp.cleanup()

    def run_it(self, **env):
        cmd = [sys.executable, "-I", str(TOOL), "cargo", "--label", "tessera-ui", "--title", "the shell", "--final", "tessera-ui", "--",
               str(self.d / "cargo"), "build", "--release", "--quiet", "--manifest-path", str(self.d / "proj/Cargo.toml"), "--target-dir", str(self.d / "target")]
        return subprocess.run(cmd, capture_output=True, text=True, timeout=60, env=dict(self.env, **env))

    def test_a_build_writes_progress_log_and_stats(self):
        r = self.run_it()
        self.assertEqual(r.returncode, 0, r.stderr)
        snap = json.loads((self.d / "state/tessera/build-progress.json").read_text())
        self.assertEqual((snap["phase"], snap["done"], snap["total"], snap["exact"], snap["title"]), ("done", 4, 4, True, "the shell"))
        log = (self.d / "state/tessera/build.log").read_text()
        self.assertIn("Compiling slint", log)
        self.assertIn("--message-format=json-render-diagnostics", log)
        self.assertNotIn("--quiet", log)
        stats = json.loads((self.d / "target/tessera-build-stats.json").read_text())
        self.assertEqual(stats["tessera-ui"]["units"], 4)
        # Plain lines (not a terminal): cargo's own, a timestamped summary, BUILD lines, the result
        self.assertIn("   Compiling slint v1.0.0", r.stderr)
        self.assertRegex(r.stderr, r"\[\d\d:\d\d:\d\d\] ")
        builds = [json.loads(line[6:]) for line in r.stderr.splitlines() if line.startswith("BUILD {")]
        self.assertTrue(builds)
        self.assertIn("✓ Built the shell in", r.stderr)
        # The second time the total is exact from the start
        r = self.run_it()
        builds = [json.loads(line[6:]) for line in r.stderr.splitlines() if line.startswith("BUILD {")]
        self.assertTrue(all(b["total"] == 4 and b["exact"] for b in builds), builds)

    def test_a_failure_shows_the_tail_and_the_log(self):
        r = self.run_it(FAKE_FAIL="1")
        self.assertEqual(r.returncode, 101)
        self.assertIn("✗ The shell didn't build (exit 101", r.stderr)
        tail = r.stderr.split("The compiler's errors:")[1]
        self.assertIn("error[E0308]: mismatched types", tail)
        self.assertLessEqual(len([l for l in tail.splitlines() if l.startswith("    ")]), bp.TAIL)
        self.assertIn("The whole log: " + str(self.d / "state/tessera/build.log"), r.stderr)
        snap = json.loads((self.d / "state/tessera/build-progress.json").read_text())
        self.assertEqual((snap["phase"], snap["error"]), ("failed", "error[E0308]: mismatched types"))
        self.assertIn("error[E0308]: mismatched types", snap["tail"])
        self.assertFalse((self.d / "target/tessera-build-stats.json").exists())

    def test_make_is_phase_only(self):
        cmd = [sys.executable, "-I", str(TOOL), "make", "--label", "tessera-glass", "--title", "the glass plugin", "--", "sh", "-c", "echo g++ -c glass.cpp"]
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=30, env=self.env)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("g++ -c glass.cpp", r.stderr)
        builds = [json.loads(line[6:]) for line in r.stderr.splitlines() if line.startswith("BUILD {")]
        self.assertEqual(builds[0]["step"], "Building the glass plugin")
        self.assertIsNone(builds[0]["fraction"])
        self.assertEqual(json.loads((self.d / "state/tessera/build-progress.json").read_text())["phase"], "done")
        # (its length is kept beside the progress file: next time there's an ETA and a percent)
        stats = json.loads((self.d / "state/tessera/tessera-build-stats.json").read_text())
        self.assertIn("make_s", stats["tessera-glass"])

    def test_make_knows_its_last_length(self):
        p = bp.Progress("tessera-glass", "make", history={"make_s": 40.0}, now=1000.0, title="the glass plugin")
        self.assertEqual(p.eta(1016.0), 24.0)
        self.assertAlmostEqual(p.fraction(1016.0, p.eta(1016.0)), 0.4)
        # (no history: no claim)
        q = bp.Progress("tessera-glass", "make", now=1000.0)
        self.assertIsNone(q.fraction(1016.0, q.eta(1016.0)))

    def test_make_failure_says_the_compilers_error(self):
        p = bp.Progress("tessera-glass", "make")
        p.feed_line("In file included from main.cpp:3:")
        p.feed_line("main.cpp:120:5: error: 'CHyprRenderer' has no member named 'foo'")
        p.feed_line("error: Hyprland's headers (0.57.2) don't match the running Hyprland (0.57.1)")
        p.finish(1)
        self.assertEqual(p.snapshot()["error"], "main.cpp:120:5: error: 'CHyprRenderer' has no member named 'foo'")

    def test_the_log_rotates_for_a_new_session(self):
        log = self.d / "state/tessera/build.log"
        log.parent.mkdir(parents=True)
        log.write_text("old\n")
        os.utime(log, (1, 1))
        bp.open_log(str(log)).close()
        self.assertEqual((self.d / "state/tessera/build.log.1").read_text(), "old\n")


class Lines(unittest.TestCase):
    def test_terminal_line_fits(self):
        snap = {"step": "Compiling a-very-long-crate-name-indeed (212 of 431)", "elapsed_s": 192, "eta_s": 250, "fraction": 0.49}
        line = bp.live_line(snap, 60)
        self.assertLessEqual(len(line), 60)
        self.assertIn("3:12", line)
        self.assertIn("about 4 min left", bp.live_line(snap, 120))
        self.assertTrue(bp.summary_line(snap, 0).endswith("Compiling a-very-long-crate-name-indeed (212 of 431) · 3:12 · about 4 min left"))


if __name__ == "__main__":
    unittest.main()
