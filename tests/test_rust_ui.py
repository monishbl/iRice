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
        for name, path in (("emoji", "shell/modules/emoji/EmojiPicker.qml"), ("clipboard", "shell/modules/clipboard/ClipboardPanel.qml"),
                           ("spotlight", "shell/modules/spotlight/SpotlightGlass.qml"), ("spotlight", "shell/modules/spotlight/SearchView.qml")):
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

    def test_the_shell_never_fights_tessera_ui_for_the_notification_server(self):
        # Quickshell's server holds org.freedesktop.Notifications for good once
        # made, so it's made only behind the gate (never while tessera-ui is to
        # serve them), and tessera-ui says it draws them only once the name is ours
        qml = (ROOT / "shell/services/Notifications.qml").read_text()
        self.assertEqual(qml.count("NotificationServer {"), 1)
        self.assertRegex(qml, r"LazyLoader \{\s*id: serverLoader\s*active: root\.ownServer")
        self.assertIn('RustUi.drawing("notifications")', qml)
        part = (CRATE / "src/surfaces/notifications/mod.rs").read_text()
        self.assertIn("st.shell.surfaces.hold(NAME)", part)
        self.assertIn("st.shell.surfaces.release(NAME)", part)
        # (queued behind a holder, never taking the name from it)
        serv = (CRATE / "src/services/notify.rs").read_text()
        self.assertIn("request_name_with_flags(NAME, Default::default())", serv)

    def test_the_mirror_and_tessera_ui_speak_the_same_lines(self):
        qml = (ROOT / "shell/services/Notifications.qml").read_text() + (ROOT / "shell/services/RustUi.qml").read_text()
        part = (CRATE / "src/surfaces/notifications/mod.rs").read_text()
        self.assertIn('tell("notifications"', part)
        self.assertIn('e.kind === "notifications"', qml)
        sent = set(re.findall(r'do: "([a-z]+)"', qml))
        handled = set(re.findall(r'^\s*"([a-z]+)" =>', part, re.M))
        self.assertTrue(sent, "the shell sends nothing")
        self.assertEqual(sorted(sent - handled), [], "lines the part doesn't handle")
        for route in re.findall(r'n\.route === "([a-z]+)"', qml):
            self.assertIn(f'Route::{route.capitalize()} => "{route}"', (CRATE / "src/surfaces/notifications/model.rs").read_text())

    def test_notification_center_moves_with_the_notifications(self):
        # Panels opens it as before; the QML one steps aside while tessera-ui
        # serves notifications, and every act the panel asks for is answered
        nc = (ROOT / "shell/modules/notifications/NotificationCenter.qml").read_text()
        self.assertIn('elsewhere: RustUi.drawing("notifications")', nc)
        qml = (ROOT / "shell/services/Notifications.qml").read_text()
        self.assertIn('cmd: "open", surface: "notifications"', qml)
        self.assertIn('e.kind === "nc"', qml)
        panel = (CRATE / "src/surfaces/notifications/panel.rs").read_text()
        acts = set(re.findall(r'\bnc\("([a-z.]+)"', panel))
        answered = set(re.findall(r'case "([a-z.]+)":', qml))
        self.assertTrue(acts, "the panel asks nothing")
        self.assertEqual(sorted(acts - answered), [], "acts the shell doesn't answer")
        # (the timer's buttons, whose acts come from the markup)
        slint = (CRATE / "ui/notification-center.slint").read_text()
        for act in re.findall(r'act\("([a-z.]+)"\)', slint):
            self.assertIn(f'case "{act}":', qml)

    def test_search_ranks_the_same_actions_in_rust(self):
        # Search's actions (title, glyph, keywords) are ranked in Rust too
        # (src/surfaces/spotlight/rank.rs): the same list, in the same order
        qml = (ROOT / "shell/services/Search.qml").read_text()
        rs = (CRATE / "src/surfaces/spotlight/rank.rs").read_text()
        block = qml[qml.index("readonly property var actions: ["):qml.index("// \"Cast to <TV>\"")]
        q = re.findall(r'\{ title: "([^"]+)", glyph: "([^"]+)", keywords: "([^"]+)"', block)
        r = re.findall(r'\("([^"]+)", "([^"]+)", "([^"]+)"\),', rs[rs.index("pub const ACTIONS"):])
        self.assertGreater(len(q), 20)
        self.assertEqual(q, r[:len(q)])
        self.assertEqual(len(q), len(r))
        # the shell answers what the Rust search asks of it
        rust_ui = (ROOT / "shell/services/RustUi.qml").read_text()
        for call in ("search.query", "search.run", "search.action", "ai.quickAsk"):
            self.assertIn(f'case "{call}":', rust_ui)
        self.assertIn('RustUi.tell("spotlight", { extras: out })', qml)

    @unittest.skipUnless(shutil.which("node"), "needs node")
    def test_search_golden_is_what_search_qml_gives(self):
        # rank.rs's tests compare the Rust ranking with golden.json, which
        # golden.js makes by running Search.qml's own functions under node:
        # a change to Search.qml's ranking must reach both
        d = CRATE / "src/surfaces/spotlight"
        r = subprocess.run(["node", str(d / "golden.js")], capture_output=True, text=True, timeout=60)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(json.loads(r.stdout), json.loads((d / "golden.json").read_text()),
                         "Search.qml ranks differently now: run node golden.js > golden.json and match rank.rs")

    def test_the_sidebar_hand_off_agrees(self):
        # The sidebar's twin steps aside through its PanelLoader; the shell
        # sends it its own commands (the tab, pictures and Spotlight's
        # exchange for its chat) and every one is handled (src/surfaces/sidebar)
        sidebar_qml = (ROOT / "shell/modules/sidebar/Sidebar.qml").read_text()
        self.assertIn('elsewhere: RustUi.drawing("sidebar")', sidebar_qml)
        qml = "".join((ROOT / f"shell/services/{n}.qml").read_text() for n in ("RustUi", "Ai", "ScreenTools"))
        sent = set(re.findall(r'cmd: "([a-z]+)", surface: "sidebar"', qml))
        self.assertEqual(sent, {"tab", "attach", "continue"})
        rs = (CRATE / "src/surfaces/sidebar/mod.rs").read_text()
        handled = set(re.findall(r'^\s*"([a-z]+)" =>', rs.split("fn command(")[1].split("fn output_removed")[0], re.M))
        self.assertEqual((sent | {"open", "close"}) - handled, set(), "sent but not handled")
        self.assertIn("shell::closed(NAME)", rs)
    def test_the_wallpaper_and_its_picker(self):
        # the picker is a panel (its PanelLoader steps aside; choosing and
        # Open folder are the shell's); the background hears the day's sun
        # and the lock's settle from Background.qml, which moves its own
        # window up to the bottom layer, clear, while Rust draws the picture
        picker = (ROOT / "shell/modules/wallpaper/WallpaperPicker.qml").read_text()
        self.assertIn('elsewhere: RustUi.drawing("wallpaper")', picker)
        bg = (ROOT / "shell/modules/background/Background.qml").read_text()
        self.assertIn('RustUi.drawing("background")', bg)
        self.assertIn("rust ? WlrLayer.Bottom : WlrLayer.Background", bg)
        for word in ('RustUi.tell("background", { sunrise:', 'RustUi.tell("wallpaper", { sunrise:', 'RustUi.tell("background", { settle: step'):
            self.assertIn(word, bg)
        lockfx = (ROOT / "shell/common/LockFx.qml").read_text()
        for step in ("prepare", "reveal", "rest"):
            self.assertIn(f'stepped("{step}"', lockfx)
        rs = (CRATE / "src/surfaces/background/mod.rs").read_text()
        for word in ('"prepare"', '"reveal"', '"rest"', 'v["settle"]', 'v["zoom"]', 'v["ms"]'):
            self.assertIn(word, rs)
        self.assertIn('v["sunrise"]', (CRATE / "src/surfaces/background/wallpaper.rs").read_text())
        # (the namespaces the Hyprland rules keep unblurred)
        rules = (ROOT / "hypr/tessera/rules.lua").read_text()
        self.assertRegex(rules, r"namespace = \"tessera:\([^)]*\bbackground\b")

    def test_today_opens_through_the_shell_and_asks_back(self):
        # Today.qml keeps deciding when it opens; TodayScreen.qml steps aside
        # and tells tessera-ui (open, close, options, the accounts' items),
        # which asks back for closing, pinning, its options and to-do ticks
        screen = (ROOT / "shell/modules/today/TodayScreen.qml").read_text()
        today_rs = (CRATE / "src/surfaces/today/mod.rs").read_text()
        self.assertIn('active: root.alive && !RustUi.drawing("today")', screen)
        for word in ('RustUi.tell("today", { open: false })', "open: true", "accounts:", 'RustUi.tell("today", { options: true })'):
            self.assertIn(word, screen)
        for word in ('v["open"]', 'v["options"]', '"accounts"', 'v["screen"]', 'v["effect"]', 'v["motion"]'):
            self.assertIn(word, today_rs)

    def test_control_center_steps_aside_and_its_bridge_talks(self):
        # Panels calls it "controlCenter", tessera-ui "controlcenter": RustUi
        # maps the one to the other both ways; both looks' loaders step
        # aside; the bridge sends the shell's state and answers its calls
        rust_ui = (ROOT / "shell/services/RustUi.qml").read_text()
        self.assertIn('controlcenter: "controlCenter"', rust_ui)
        self.assertIn("surfaceOf(Panels.open)", rust_ui)
        self.assertIn("panelOf(e.surface)", rust_ui)
        cc = (ROOT / "shell/modules/controlcenter/ControlCenter.qml").read_text()
        self.assertEqual(cc.count('elsewhere: RustUi.drawing("controlcenter")'), 2)
        self.assertIn("RustBridge {}", cc)
        bridge = (ROOT / "shell/modules/controlcenter/RustBridge.qml").read_text()
        for word in ('RustUi.tell("controlcenter", { state:', 'RustUi.tell("controlcenter", { page:', "function onCcCall(", 'case "cc.block":'):
            self.assertIn(word, bridge)
        part = (CRATE / "src/surfaces/controlcenter/mod.rs").read_text()
        for word in ('v.get("state")', 'v.get("page")', 'v["ccPage"]', 'v["editing"]'):
            self.assertTrue(word in part or word in (CRATE / "src/surfaces/controlcenter/bridge.rs").read_text(), word)

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
