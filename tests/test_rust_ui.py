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
import os
import re
import shutil
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CRATE = ROOT / "native/tessera-ui"


def known_surfaces():
    src = (CRATE / "src/surfaces/mod.rs").read_text()
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
        rs = (CRATE / "src/surfaces/mod.rs").read_text()
        for word in ('"ready"', '"unknown"', '"ui"'):
            self.assertIn(word, qml)
            self.assertIn(word, rs)
        self.assertIn('"--surfaces"', qml)

    def test_panels_drawn_in_rust_open_and_close_through_the_shell(self):
        # RustUi forwards Panels' open and close as commands on tessera-ui's
        # stdin; tessera-ui says when a panel closed itself and asks for
        # what only the shell does (src/shell.rs)
        qml = (ROOT / "shell/services/RustUi.qml").read_text()
        shell_rs = (CRATE / "src/shell.rs").read_text()
        for word in ('cmd: "open"', 'cmd: "close"', "proc.write(", 'e.kind === "closed"', 'e.kind === "call"', "Panels.close()"):
            self.assertIn(word, qml)
        for word in ('"cmd"', '"surface"', '"screen"', '"closed"', '"call"'):
            self.assertIn(word, shell_rs)
        main_rs = (CRATE / "src/main.rs").read_text()
        self.assertIn("Command::parse", main_rs)
        # every call a part asks for is one RustUi.qml answers
        asked = set()
        for p in (CRATE / "src").rglob("*.rs"):
            asked |= set(re.findall(r'shell::ask\("([a-zA-Z.]+)"', p.read_text()))
        answered = set(re.findall(r'case "([a-zA-Z.]+)":', qml))
        self.assertTrue(asked, "no calls found")
        self.assertEqual(sorted(asked - answered), [], "asked of the shell, never answered")

    def test_panel_twins_step_aside_through_their_loader(self):
        # a panel's twin steps aside with its PanelLoader (`elsewhere`), so
        # Panels' opening goes to tessera-ui and the QML window never loads
        loader = (ROOT / "shell/common/PanelLoader.qml").read_text()
        self.assertRegex(loader, r"wanted: !elsewhere &&")
        for name, path in (("emoji", "shell/modules/emoji/EmojiPicker.qml"), ("clipboard", "shell/modules/clipboard/ClipboardPanel.qml")):
            src = (ROOT / path).read_text()
            self.assertIn(f'RustUi.drawing("{name}")', src)
            self.assertIn("elsewhere:", src)
    def test_the_osd_and_the_shell_tell_each_other(self):
        # tessera-ui says when its capsule shows (the banners move under it);
        # the shell tells it what its own sliders set (no OSD for those) and
        # passes `qs ipc call osd preview` on, as stdin lines {"to": "osd", …}
        rust_ui = (ROOT / "shell/services/RustUi.qml").read_text()
        osd_rs = (CRATE / "src/surfaces/osd/mod.rs").read_text()
        main_rs = (CRATE / "src/main.rs").read_text()
        self.assertIn('e.kind === "osd"', rust_ui)
        self.assertIn('"kind": "osd", "shown"', osd_rs)
        self.assertIn("send(Object.assign({ to: surface }", rust_ui)
        self.assertIn('v["to"]', main_rs)
        self.assertIn('RustUi.tell("osd", { hush: "sound" })', (ROOT / "shell/services/Audio.qml").read_text())
        self.assertIn('RustUi.tell("osd", { hush: "brightness" })', (ROOT / "shell/services/Brightness.qml").read_text())
        osd_qml = (ROOT / "shell/modules/osd/Osd.qml").read_text()
        self.assertIn('RustUi.tell("osd", { preview: kind })', osd_qml)
        self.assertIn("RustUi.osdShown", osd_qml)
        for word in ('v["hush"]', 'v["preview"]'):
            self.assertIn(word, osd_rs)

    def test_never_started_with_the_old_list(self):
        # Bound straight to the list, the process can start in the same instant
        # the list changes, before its command has the new list: with the old
        # (empty) one it ends at once and waits out a 10 s back-off
        qml = (ROOT / "shell/services/RustUi.qml").read_text()
        self.assertNotRegex(qml, r"wanted:\s*root\.wanted\.length")
        self.assertRegex(qml, r"wanted:\s*root\.armed")
        self.assertIn("Qt.callLater", qml)


class Bridge(unittest.TestCase):
    """The Rust menu bar and the QML shell talk both ways (src/services/bridge.rs):
    what the bar asks the shell to do must exist there, and what the shell
    sends must be read."""

    def rust(self):
        return "\n".join(p.read_text() for p in (CRATE / "src").rglob("*.rs"))

    def test_every_act_the_bar_asks_for_is_answered(self):
        rs = self.rust()
        asked = set(re.findall(r'"rustui", "act", "([A-Za-z]+)"', rs)) | set(re.findall(r'Action::Act\("([A-Za-z]+)"', rs))
        asked |= set(re.findall(r'\bact\("([A-Za-z]+)", ', (CRATE / "src/surfaces/menubar/mod.rs").read_text()))
        self.assertGreater(len(asked), 15, asked)
        qml = (ROOT / "shell/modules/menubar/RustBridge.qml").read_text()
        answered = set(re.findall(r'case "([A-Za-z]+)":', qml))
        self.assertEqual(sorted(asked - answered), [], "the Rust bar asks for these, RustBridge.qml has no case")
        self.assertIn('target: "rustui"', qml)
        # (the bridge is made by the shell, and steps in only while Rust draws the bar)
        self.assertIn("RustBridge {}", (ROOT / "shell/TesseraShell.qml").read_text())
        self.assertIn('RustUi.drawing("menubar")', qml)

    def test_the_island_forwards_what_rust_reads(self):
        island = (ROOT / "shell/services/Island.qml").read_text()
        for op in ("post", "update", "remove", "dismiss"):
            self.assertIn(f'forward("{op}"', island)
            self.assertIn(f'"{op}"', (CRATE / "src/surfaces/menubar/mod.rs").read_text())
        # every line kind the shell writes is parsed
        bridge = (CRATE / "src/services/bridge.rs").read_text()
        sent = set(re.findall(r'kind: "([a-z]+)"', (ROOT / "shell/modules/menubar/RustBridge.qml").read_text())) | {"island"}
        for kind in sent:
            self.assertIn(f'"{kind}" =>', bridge)
        self.assertIn("function send(", (ROOT / "shell/services/RustUi.qml").read_text())

    def test_toggled_panels_exist_in_the_shell(self):
        # the status items open the QML popovers by name (Panels)
        rs = "\n".join(p.read_text() for p in (CRATE / "src/surfaces/menubar").glob("*.rs"))
        panels = set(re.findall(r'toggle_at\(&output, "([A-Za-z]+)"', rs))
        self.assertGreater(len(panels), 10)
        qml = "\n".join(p.read_text(errors="replace") for p in qml_files())
        for p in panels:
            self.assertRegex(qml, rf'(panel|popoverName|name):\s*"{p}"', p)


def binary():
    for kind in ("release", "debug"):
        b = CRATE / "target" / kind / "tessera-ui"
        if b.exists():
            return b
    return None


@unittest.skipUnless(binary(), "tessera-ui isn't built (tools/build-native.sh)")
class Binary(unittest.TestCase):
    # (never with a display: a surface it can draw would appear on the
    # session running the tests)
    ENV = {k: v for k, v in os.environ.items() if k not in ("WAYLAND_DISPLAY", "WAYLAND_SOCKET", "DISPLAY")}

    def run_ui(self, *args, input=None):
        return subprocess.run([str(binary()), *args], capture_output=True, text=True, timeout=30, env=self.ENV, input=input, stdin=None if input is not None else subprocess.DEVNULL)

    def test_a_list_it_cant_draw_is_answered_without_a_display(self):
        # (no Wayland needed: nothing to draw, so it says so and ends)
        r = self.run_ui("--surfaces", "lock,greeter")
        self.assertEqual(r.returncode, 0, r.stderr)
        lines = [json.loads(l) for l in r.stdout.splitlines()]
        self.assertEqual(lines[-1], {"src": "ui", "kind": "ready", "surfaces": []})
        self.assertEqual({l["surface"] for l in lines[:-1]}, {"lock", "greeter"})

    def test_commands_on_stdin_and_its_end(self):
        # (nothing to draw, it answers and ends whatever comes on stdin; how
        # the lines are read is the crate's own test: shell::tests)
        r = self.run_ui("--surfaces", "lock", input='nonsense\n{"cmd":"open","surface":"lock"}\n{"to":"osd","hush":"sound"}\n')
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(json.loads(r.stdout.splitlines()[-1]), {"src": "ui", "kind": "ready", "surfaces": []})

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
