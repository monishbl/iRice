"""tessera-ui, the interface in Rust (native/tessera-ui, docs/rust-ui-plan.md).

The QML shell and tessera-ui split the surfaces between them: Settings lists
the ones Rust draws (`rust.surfaces`, empty by default), services/RustUi.qml
starts tessera-ui with that list, and each QML surface steps aside only while
tessera-ui says it draws it. These tests hold that agreement:
  - every surface tessera-ui knows has a QML twin that checks RustUi.drawing,
    so a surface is never drawn twice or not at all;
  - by default nothing moves (the list is empty);
  - the binary answers the shell as RustUi.qml expects;
  - the crate's own tests pass (its colours against the shell's
    MaterialColor.js and the wallpaper seed against matugen among them).
"""
import json
import re
import shutil
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CRATE = ROOT / "native/tessera-ui"


def known_surfaces():
    src = (CRATE / "src/surfaces.rs").read_text()
    m = re.search(r"pub const KNOWN: &\[&str\] = &\[([^\]]*)\]", src)
    return re.findall(r'"([a-z]+)"', m.group(1))


def qml_files():
    return [p for p in list(ROOT.glob("shell/**/*.qml")) + list(ROOT.glob("material/**/*.qml")) if "build" not in p.parts]


class Agreement(unittest.TestCase):
    def test_every_rust_surface_has_a_qml_twin_that_steps_aside(self):
        known = known_surfaces()
        self.assertTrue(known, "no surfaces found in src/surfaces.rs")
        checked = set()
        for p in qml_files():
            checked |= set(re.findall(r'RustUi\.drawing\("([a-z]+)"\)', p.read_text(errors="replace")))
        self.assertEqual(sorted(set(known) - checked), [], "drawn in Rust but the QML one never steps aside")
        self.assertEqual(sorted(checked - set(known)), [], "QML steps aside for a surface tessera-ui doesn't draw")

    def test_nothing_moves_by_default(self):
        src = (ROOT / "shell/common/Config.qml").read_text()
        m = re.search(r"property JsonObject rust: JsonObject \{[^}]*property list<string> surfaces: \[([^\]]*)\]", src)
        self.assertIsNotNone(m, "Config.qml has no rust.surfaces")
        self.assertEqual(m.group(1).strip(), "")

    def test_the_shell_reads_what_the_binary_writes(self):
        qml = (ROOT / "shell/services/RustUi.qml").read_text()
        rs = (CRATE / "src/surfaces.rs").read_text()
        for word in ('"ready"', '"unknown"', '"ui"'):
            self.assertIn(word, qml)
            self.assertIn(word, rs)
        self.assertIn('"--surfaces"', qml)

    def test_never_started_with_the_old_list(self):
        # Bound straight to the list, the process can start in the same instant
        # the list changes, before its command has the new list: with the old
        # (empty) one it ends at once and waits out a 10 s back-off
        qml = (ROOT / "shell/services/RustUi.qml").read_text()
        self.assertNotRegex(qml, r"wanted:\s*root\.wanted\.length")
        self.assertRegex(qml, r"wanted:\s*root\.armed")
        self.assertIn("Qt.callLater", qml)


def binary():
    for kind in ("release", "debug"):
        b = CRATE / "target" / kind / "tessera-ui"
        if b.exists():
            return b
    return None


@unittest.skipUnless(binary(), "tessera-ui isn't built (tools/build-native.sh)")
class Binary(unittest.TestCase):
    def run_ui(self, *args):
        return subprocess.run([str(binary()), *args], capture_output=True, text=True, timeout=30, stdin=subprocess.DEVNULL)

    def test_a_list_it_cant_draw_is_answered_without_a_display(self):
        # (no Wayland needed: nothing to draw, so it says so and ends)
        r = self.run_ui("--surfaces", "dock,osd")
        self.assertEqual(r.returncode, 0, r.stderr)
        lines = [json.loads(l) for l in r.stdout.splitlines()]
        self.assertEqual(lines[-1], {"src": "ui", "kind": "ready", "surfaces": []})
        self.assertEqual({l["surface"] for l in lines[:-1]}, {"dock", "osd"})

    def test_version(self):
        r = self.run_ui("--version")
        self.assertEqual(r.returncode, 0)
        self.assertTrue(r.stdout.startswith("tessera-ui "))

    def test_material_roles_match_the_shell_for_a_seed(self):
        r = self.run_ui("--material-roles", "#3b6ea5", "dark", "tonalSpot")
        roles = dict(l.split() for l in r.stdout.splitlines())
        self.assertGreaterEqual(len(roles), 40)
        self.assertRegex(roles["primary"], r"^#[0-9a-f]{6}$")

    @unittest.skipUnless(shutil.which("matugen"), "needs matugen")
    def test_seed_matches_matugen(self):
        pic = ROOT / "site/film/hero/assets/leaves.jpg"
        r = self.run_ui("--seed", str(pic))
        m = subprocess.run(["matugen", "image", str(pic), "--json", "hex", "--dry-run", "--source-color-index", "0"], capture_output=True, text=True, timeout=60)
        src = json.loads(m.stdout)["colors"]["source_color"]
        want = (src.get("default") or src.get("dark"))["color"].lower()
        self.assertEqual(r.stdout.strip(), want)


@unittest.skipUnless(shutil.which("cargo"), "needs cargo")
class CrateTests(unittest.TestCase):
    def test_cargo_test(self):
        r = subprocess.run(["cargo", "test", "--locked", "--quiet", "--manifest-path", str(CRATE / "Cargo.toml"), "--bin", "tessera-ui"], capture_output=True, text=True, timeout=3600)
        self.assertEqual(r.returncode, 0, r.stdout[-3000:] + r.stderr[-3000:])


if __name__ == "__main__":
    unittest.main()
