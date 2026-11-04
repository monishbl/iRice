"""app-theme-render.py: Match apps in the Material look fills the colour
templates (assets/theming) with the shell's own Material roles, not
matugen's, so apps match the shell exactly."""
import importlib.util
import json
import re
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "material/assets/scripts/app-theme-render.py"
THEMING = ROOT / "shell/assets/theming"
spec = importlib.util.spec_from_file_location("app_theme_render", SCRIPT)
render = importlib.util.module_from_spec(spec)
spec.loader.exec_module(render)

# A dark scheme as Theme.m3 hands it over (camelCase, #rrggbb)
ROLES = {
    "background": "#0d0e12", "surface": "#0d0e12", "surfaceDim": "#0d0e12", "surfaceBright": "#292c34",
    "surfaceContainerLowest": "#000000", "surfaceContainerLow": "#111318", "surfaceContainer": "#171a1f",
    "surfaceContainerHigh": "#1d2026", "surfaceContainerHighest": "#23262d", "onSurface": "#e3e5f0",
    "onSurfaceVariant": "#a8abb5", "outline": "#73757e", "outlineVariant": "#454850", "primary": "#b6c6ee",
    "onPrimary": "#304060", "primaryContainer": "#425274", "secondary": "#c0c6d9", "secondaryContainer": "#3c4253",
    "tertiary": "#e3c2f2", "error": "#ff6e84",
}
EXTRA = {"tessera": {"accent": "#b6c6ee", "accent_stripped": "b6c6ee", "accent_rgb": "182,198,238", "wallpaper": "/w.jpg",
                   "red": "F87171", "green": "4ADE80", "yellow": "FBBF24", "blue": "60A5FA", "magenta": "E087CE", "cyan": "25C2C2"}}


class Fill(unittest.TestCase):
    def roles(self):
        return render.parse_roles(json.dumps(ROLES))

    def test_roles_are_snake_case(self):
        r = self.roles()
        self.assertEqual(r["surface_container_highest"], "#23262d")
        self.assertEqual(r["on_surface_variant"], "#a8abb5")

    def test_formats(self):
        out = render.fill("{{colors.primary.default.hex}} {{ colors.primary.dark.hex_stripped }} "
                          "{{colors.primary.light.red}},{{colors.primary.default.green}},{{colors.primary.default.blue}}", self.roles(), EXTRA)
        self.assertEqual(out, "#b6c6ee b6c6ee 182,198,238")

    def test_tessera_values(self):
        self.assertEqual(render.fill("{{tessera.accent}}|{{tessera.red}}", self.roles(), EXTRA), "#b6c6ee|F87171")

    def test_white_on_accent_becomes_on_primary(self):
        # Material's primary is light in dark mode: white text on it wouldn't read
        out = render.fill("fg=#FFFFFF\nsel=ffffff\nkde=255,255,255\nkeep=#ffffffcc 1ffffff\n", self.roles(), EXTRA)
        self.assertEqual(out, "fg=#304060\nsel=304060\nkde=48,64,96\nkeep=#ffffffcc 1ffffff\n")

    def test_placeholder_values_are_not_rewritten(self):
        roles = dict(ROLES, surface="#ffffff")
        out = render.fill("{{colors.surface.default.hex}}", render.parse_roles(json.dumps(roles)), EXTRA)
        self.assertEqual(out, "#ffffff")

    def test_unknown_placeholder_fails(self):
        with self.assertRaises(KeyError):
            render.fill("{{colors.nothing.default.hex}}", self.roles(), EXTRA)
        with self.assertRaises(KeyError):
            render.fill("{{image}}", self.roles(), EXTRA)

    def test_roles_need_primary_and_surface(self):
        with self.assertRaises(ValueError):
            render.parse_roles(json.dumps({"primary": "#112233"}))

    def test_every_bundled_template_fills(self):
        # Each template only uses what this renderer understands
        for tpl in sorted(THEMING.iterdir()):
            with self.subTest(tpl.name):
                out = render.fill(tpl.read_text(), self.roles(), EXTRA)
                self.assertNotIn("{{", out)
                self.assertNotIn("}}", out)

    def test_gtk_uses_the_shell_roles(self):
        out = render.fill((THEMING / "gtk.css").read_text(), self.roles(), EXTRA)
        self.assertIn("@define-color window_bg_color #0d0e12;", out)
        self.assertIn("@define-color accent_fg_color #304060;", out)
        self.assertIn("@define-color popover_bg_color #1d2026;", out)


class Command(unittest.TestCase):
    def setUp(self):
        self.dir = Path(tempfile.mkdtemp(prefix="tessera-render-"))

    def tearDown(self):
        shutil.rmtree(self.dir, ignore_errors=True)

    def run_script(self, *pairs):
        return subprocess.run(["python3", "-I", str(SCRIPT), json.dumps(ROLES), json.dumps(EXTRA), *pairs],
                              capture_output=True, text=True)

    def test_writes_outputs(self):
        out = self.dir / "kitty" / "tessera-colors.conf"
        r = self.run_script(f"{THEMING / 'kitty.conf'}={out}")
        self.assertEqual(r.returncode, 0, r.stderr)
        text = out.read_text()
        self.assertIn("background #0d0e12", text)
        self.assertIn("selection_foreground #304060", text)
        self.assertFalse(re.search(r"\{\{", text))

    def test_a_bad_template_writes_nothing(self):
        bad = self.dir / "bad.conf"
        bad.write_text("x={{colors.nope.default.hex}}\n")
        good_out, bad_out = self.dir / "good.conf", self.dir / "bad-out.conf"
        r = self.run_script(f"{THEMING / 'kitty.conf'}={good_out}", f"{bad}={bad_out}")
        self.assertEqual(r.returncode, 1)
        self.assertFalse(good_out.exists())
        self.assertFalse(bad_out.exists())

    @unittest.skipUnless(shutil.which("node"), "node not installed")
    def test_shell_scheme_has_every_role_the_templates_use(self):
        # The roles Theme.m3 really has (common/MaterialColor.js) cover the templates
        src = (ROOT / "shell/common/MaterialColor.js").read_text().replace(".pragma library", "")
        js = "const MCU = new Function(require('fs').readFileSync(0, 'utf8') + ';return MCU;')();" \
             "console.log(JSON.stringify(MCU.scheme('#6750A4', false, 'tonalSpot', 0)))"
        r = subprocess.run(["node", "-e", js], input=src, capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stderr)
        roles = render.parse_roles(r.stdout)
        for tpl in sorted(THEMING.iterdir()):
            with self.subTest(tpl.name):
                self.assertNotIn("{{", render.fill(tpl.read_text(), roles, EXTRA))


if __name__ == "__main__":
    unittest.main()
