"""Keyboard and screen-reader basics of the shared controls.

Runs a small page with each control in iRice's Quickshell, offscreen, and
drives it with real key and mouse events (QtTest's TestEvent): Tab visits
every control in order, Space and Enter act, the arrows move sliders and
choices, each control carries an accessible role and name, and the focus
ring shows for keyboard focus but not after a click.
"""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
BUNDLE = Path.home() / ".local/share/irice/quickshell"
QS = BUNDLE / "bin/qs"

PAGE = r'''
import QtQuick
import QtTest
import Quickshell
import qs.common
import qs.modules.settings as S
import qs.modules.controlcenter as CC

ShellRoot {
    settings.watchFiles: false
    property var log: []
    FloatingWindow {
        id: win
        implicitWidth: 420
        implicitHeight: 640
        Column {
            spacing: 12
            Switch { id: sw; objectName: "switch"; accessibleName: "Wi-Fi"; onToggled: checked = !checked }
            IconButton { id: ib; objectName: "iconbutton"; icon: "x"; tooltip: "Close"; onClicked: log.push("iconbutton") }
            Segmented { id: seg; objectName: "segmented"; options: ["Off", "On", "Auto"]; onSelected: i => current = i }
            Slider { id: sl; objectName: "slider"; from: 0; to: 10; stepSize: 1; value: 5; onMoved: v => value = v }
            Dropdown { id: dd; objectName: "dropdown"; accessibleName: "Theme"; options: [{ value: "a", label: "Alpha" }, { value: "b", label: "Beta" }]; value: "a"; onChosen: v => value = v }
            S.SettingsButton { id: sb; objectName: "settingsbutton"; text: "Save"; onClicked: log.push("save") }
            CC.ListRow { id: lr; objectName: "listrow"; width: 300; title: "Home Wi-Fi"; status: "Connected"; onClicked: log.push("row") }
            CC.CircleToggle { id: ct; objectName: "circle"; icon: "moon"; accessibleName: "Focus"; onClicked: checked = !checked }
            CC.CapsuleSlider { id: cs; objectName: "capsule"; width: 200; height: 40; icon: "sun"; accessibleName: "Brightness"; value: 0.5; onMoved: v => value = v }
        }
    }
    TestEvent { id: ev }
    function ring(item) {
        for (const c of item.children)
            if (c.shown !== undefined && c.keyboard !== undefined)
                return c.shown;
        return null;
    }
    function out(k, v) { console.log("A11Y " + k + "=" + JSON.stringify(v)); }
    Timer {
        interval: 400
        running: true
        onTriggered: {
            win.contentItem.forceActiveFocus();
            const items = [sw, ib, seg, sl, dd, sb, lr, ct, cs];
            const order = [];
            const rings = [];
            for (let i = 0; i < items.length; i++) {
                ev.keyClick(Qt.Key_Tab, 0, -1);
                const f = items.find(x => x.activeFocus);
                order.push(f ? f.objectName : "none");
                rings.push(f ? ring(f) : null);
            }
            out("order", order);
            out("rings", rings);
            out("roles", items.map(x => x.Accessible.role));
            out("names", items.map(x => x.Accessible.name));

            // Space and Enter act; arrows move
            sw.forceActiveFocus(Qt.TabFocusReason); ev.keyClick(Qt.Key_Space, 0, -1); out("switch", sw.checked);
            ib.forceActiveFocus(Qt.TabFocusReason); ev.keyClick(Qt.Key_Return, 0, -1);
            sb.forceActiveFocus(Qt.TabFocusReason); ev.keyClick(Qt.Key_Space, 0, -1);
            lr.forceActiveFocus(Qt.TabFocusReason); ev.keyClick(Qt.Key_Enter, 0, -1);
            out("log", log);
            seg.forceActiveFocus(Qt.TabFocusReason); ev.keyClick(Qt.Key_Right, 0, -1); ev.keyClick(Qt.Key_End, 0, -1); out("seg", seg.current);
            sl.forceActiveFocus(Qt.TabFocusReason); ev.keyClick(Qt.Key_Right, 0, -1); ev.keyClick(Qt.Key_PageUp, 0, -1); out("slider", sl.value);
            ev.keyClick(Qt.Key_Home, 0, -1); out("sliderHome", sl.value);
            dd.forceActiveFocus(Qt.TabFocusReason); ev.keyClick(Qt.Key_Down, 0, -1); out("dropdown", dd.value);
            ct.forceActiveFocus(Qt.TabFocusReason); ev.keyClick(Qt.Key_Space, 0, -1); out("circle", ct.checked);

            cs.forceActiveFocus(Qt.TabFocusReason); ev.keyClick(Qt.Key_Up, 0, -1); out("capsule", Math.round(cs.value * 100));
            out("accChecked", [sw.Accessible.checked, ct.Accessible.checked]);
            out("sliderValue", sl.Accessible.description);

            // A click that focuses a control doesn't ring it
            win.contentItem.forceActiveFocus();
            ev.mouseClick(sb, 10, 10, Qt.LeftButton, 0, -1);
            out("clickFocus", sb.activeFocus);
            out("clickRing", ring(sb));
            Qt.quit();
        }
    }
}
'''


@unittest.skipUnless(QS.exists(), "requires bundled Quickshell")
class KeyboardAndScreenReader(unittest.TestCase):
    def run_page(self):
        with tempfile.TemporaryDirectory(prefix="irice-a11y-") as d:
            home = Path(d) / "shell"
            home.mkdir()
            for part in ("common", "glass", "modules", "services", "assets"):
                (home / part).symlink_to(ROOT / "shell" / part)
            (home / "shell.qml").write_text(PAGE)
            env = dict(os.environ, QT_QPA_PLATFORM="offscreen", XDG_RUNTIME_DIR=str(Path(d) / "run"),
                       XDG_STATE_HOME=str(Path(d) / "state"), XDG_CONFIG_HOME=str(Path(d) / "config"),
                       XDG_CACHE_HOME=str(Path(d) / "cache"), QML_IMPORT_PATH=str(BUNDLE / "lib/qt6/qml"))
            (Path(d) / "run").mkdir(mode=0o700)
            env.pop("WAYLAND_DISPLAY", None)
            env.pop("HYPRLAND_INSTANCE_SIGNATURE", None)
            proc = subprocess.run([str(QS), "-p", str(home)], env=env, capture_output=True, text=True, timeout=30)
        text = proc.stdout + proc.stderr
        values = {}
        for line in text.splitlines():
            if "A11Y " in line:
                k, _, v = line.split("A11Y ", 1)[1].partition("=")
                import json
                values[k] = json.loads(v)
        self.assertIn("order", values, text)
        for bad in ("TypeError", "ReferenceError"):
            self.assertNotIn(bad, "\n".join(l for l in text.splitlines() if "shell.qml" in l or "common/" in l or "modules/" in l), text)
        return values

    def test_keyboard_and_accessibility(self):
        v = self.run_page()
        self.assertEqual(v["order"], ["switch", "iconbutton", "segmented", "slider", "dropdown", "settingsbutton",
                                      "listrow", "circle", "capsule"])
        self.assertEqual(v["rings"], [True] * 9)
        self.assertNotIn(0, v["roles"])  # every control has a role
        self.assertEqual(v["names"][0], "Wi-Fi")
        self.assertEqual(v["names"][1], "Close")  # an icon-only button is named by its tooltip
        self.assertEqual(v["names"][4], "Theme")
        self.assertEqual(v["names"][5], "Save")
        self.assertEqual(v["names"][6], "Home Wi-Fi")
        self.assertTrue(v["switch"])
        self.assertEqual(v["log"], ["iconbutton", "save", "row"])
        self.assertEqual(v["seg"], 2)
        self.assertEqual(v["slider"], 10)  # 5 → 6 (arrow) → 10 (page up, capped)
        self.assertEqual(v["sliderHome"], 0)
        self.assertEqual(v["dropdown"], "b")
        self.assertTrue(v["circle"])
        self.assertEqual(v["capsule"], 55)
        self.assertEqual(v["accChecked"], [True, True])
        self.assertEqual(v["sliderValue"], "0")
        self.assertTrue(v["clickFocus"])  # SettingsButton takes focus on click…
        self.assertFalse(v["clickRing"])  # …but a click doesn't ring it


if __name__ == "__main__":
    unittest.main()
