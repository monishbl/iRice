"""Popups and pickers follow Tessera (21 Nov, system1: GTK 3 dialogs came up
white because GTK 3 has no "Adwaita-dark" theme, and the open/save picker was
Nautilus' with Dolphin as the Files app).

file-picker.py: the portal's FileChooser follows the Files app.
desktop-theme.py: GTK 3's theme and settings.ini follow light/dark (adw-gtk3
when installed, which takes Tessera's colours), and undo-gtk puts it back.
"""
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PICKER = ROOT / "shell/assets/scripts/file-picker.py"
THEME = ROOT / "shell/assets/scripts/desktop-theme.py"


class Base(unittest.TestCase):
    def setUp(self):
        self.d = tempfile.TemporaryDirectory()
        h = Path(self.d.name)
        self.home, self.cfg, self.bin = h / "home", h / "home/.config", h / "bin"
        self.portals, self.themes = h / "portals", h / "themes"
        for p in (self.cfg, self.bin, self.portals, self.themes):
            p.mkdir(parents=True)
        self.gs = h / "gsettings.json"
        self.gs.write_text(json.dumps({"gtk-theme": "'Adwaita-dark'"}))
        self.fake("gsettings", f"""exec {sys.executable} -c '
import json, sys
f = "{self.gs}"
d = json.load(open(f))
if sys.argv[1] == "get": print(d.get(sys.argv[3], "''"))
elif sys.argv[1] == "reset": d.pop(sys.argv[3], None); json.dump(d, open(f, "w"))
else: d[sys.argv[3]] = "\\x27" + sys.argv[4] + "\\x27"; json.dump(d, open(f, "w"))
' "$@"
""")
        self.env = {"HOME": str(self.home), "XDG_CONFIG_HOME": str(self.cfg), "XDG_DATA_HOME": str(self.home / ".local/share"),
                    "PATH": str(self.bin), "TESSERA_PORTALS_DIR": str(self.portals),
                    "TESSERA_PICKER_NO_RESTART": "1", "TESSERA_SYSTEM_THEMES": str(self.themes)}

    def tearDown(self):
        self.d.cleanup()

    def fake(self, name, body="exit 0\n"):
        f = self.bin / name
        f.write_text("#!/bin/sh\n" + body)
        f.chmod(0o755)

    def run_py(self, script, *args):
        p = subprocess.run([sys.executable, "-I", str(script), *args], env=self.env, capture_output=True, text=True, timeout=30)
        self.assertEqual(p.returncode, 0, p.stderr)
        return p.stdout

    def text(self, rel):
        p = self.cfg / rel
        return p.read_text() if p.exists() else None


class Picker(Base):
    def conf(self):
        return self.text("xdg-desktop-portal/hyprland-portals.conf")

    def test_follows_the_files_app(self):
        for p in ("kde", "gnome", "gtk"):
            (self.portals / (p + ".portal")).write_text("")
        self.fake("nautilus")
        self.fake("dolphin")
        # (Automatic: the first installed, Nautilus, as the shell picks it)
        out = json.loads(self.run_py(PICKER, "apply", "--json"))
        self.assertEqual((out["app"], out["picker"], out["changed"]), ("nautilus", "gnome", True))
        self.assertIn("org.freedesktop.impl.portal.FileChooser=gnome", self.conf())
        self.assertIn("default=hyprland;gtk", self.conf())
        # Dolphin chosen in Settings: KDE's
        (self.cfg / "tessera").mkdir()
        (self.cfg / "tessera/config.json").write_text(json.dumps({"desktop": {"fileManager": "dolphin"}}))
        self.assertEqual(json.loads(self.run_py(PICKER, "apply", "--json"))["picker"], "kde")
        self.assertIn("FileChooser=kde", self.conf())
        # told the choice (Settings, before the config is written): it wins
        self.assertEqual(json.loads(self.run_py(PICKER, "apply", "--app", "nautilus", "--json"))["picker"], "gnome")
        # nothing changed: nothing written
        self.assertFalse(json.loads(self.run_py(PICKER, "apply", "--app", "nautilus", "--json"))["changed"])

    def test_gtk_when_the_portal_is_missing_or_another_app(self):
        (self.portals / "gtk.portal").write_text("")
        self.fake("dolphin")
        self.assertEqual(json.loads(self.run_py(PICKER, "apply", "--json"))["picker"], "gtk")
        self.fake("thunar")
        self.assertEqual(json.loads(self.run_py(PICKER, "apply", "--app", "thunar", "--json"))["picker"], "gtk")

    def test_yours_is_kept_and_remove_takes_only_ours(self):
        (self.portals / "kde.portal").write_text("")
        self.fake("dolphin")
        mine = "[preferred]\ndefault=gtk\n"
        (self.cfg / "xdg-desktop-portal").mkdir()
        (self.cfg / "xdg-desktop-portal/hyprland-portals.conf").write_text(mine)
        out = json.loads(self.run_py(PICKER, "apply", "--json"))
        self.assertIn("skipped", out)
        self.run_py(PICKER, "remove")
        self.assertEqual(self.conf(), mine)
        (self.cfg / "xdg-desktop-portal/hyprland-portals.conf").unlink()
        self.run_py(PICKER, "apply")
        self.assertIn("FileChooser=kde", self.conf())
        self.run_py(PICKER, "remove")
        self.assertIsNone(self.conf())


class Gtk(Base):
    def gtk_theme(self):
        return json.loads(self.gs.read_text()).get("gtk-theme")

    def test_dark_gtk3_is_a_theme_gtk3_has(self):
        # (no adw-gtk3: Adwaita, dark through settings.ini; never "Adwaita-dark")
        self.run_py(THEME, "dark", "--keep-kde")
        self.assertEqual(self.gtk_theme(), "'Adwaita'")
        ini = self.text("gtk-3.0/settings.ini")
        self.assertIn("gtk-theme-name=Adwaita\n", ini)
        self.assertIn("gtk-application-prefer-dark-theme=true", ini)
        self.assertIn("gtk-application-prefer-dark-theme=true", self.text("gtk-4.0/settings.ini"))
        self.run_py(THEME, "light", "--keep-kde")
        self.assertIn("gtk-application-prefer-dark-theme=false", self.text("gtk-3.0/settings.ini"))

    def test_adw_gtk3_takes_tessera_colours(self):
        (self.themes / "adw-gtk3").mkdir()
        (self.themes / "adw-gtk3-dark").mkdir()
        self.run_py(THEME, "dark", "--keep-kde")
        self.assertEqual(self.gtk_theme(), "'adw-gtk3-dark'")
        self.assertIn("gtk-theme-name=adw-gtk3-dark", self.text("gtk-3.0/settings.ini"))
        self.run_py(THEME, "light", "--keep-kde")
        self.assertEqual(self.gtk_theme(), "'adw-gtk3'")

    def test_a_theme_of_yours_stays(self):
        self.gs.write_text(json.dumps({"gtk-theme": "'Orchis-Dark'"}))
        self.run_py(THEME, "dark", "--keep-kde")
        self.assertEqual(self.gtk_theme(), "'Orchis-Dark'")
        self.assertIsNone(self.text("gtk-3.0/settings.ini"))

    def test_undo_puts_gtk_back(self):
        (self.cfg / "gtk-3.0").mkdir()
        (self.cfg / "gtk-3.0/gtk.css").write_text("@import 'tessera-colors.css';\n.mine { color: red; }\n")
        (self.cfg / "gtk-3.0/tessera-colors.css").write_text("@define-color accent_color #fff;\n")
        (self.cfg / "gtk-4.0").mkdir()
        (self.cfg / "gtk-4.0/gtk.css").write_text("@import 'tessera-colors.css';\n")
        self.run_py(THEME, "dark", "--keep-kde")
        self.run_py(THEME, "undo-gtk")
        self.assertEqual(self.text("gtk-3.0/gtk.css"), ".mine { color: red; }\n")
        self.assertIsNone(self.text("gtk-3.0/tessera-colors.css"))
        self.assertIsNone(self.text("gtk-4.0/gtk.css"))
        self.assertIsNone(self.text("gtk-3.0/settings.ini"))
        self.assertIsNone(self.gtk_theme())  # (reset: GTK's default)


if __name__ == "__main__":
    unittest.main()
