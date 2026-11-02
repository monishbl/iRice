"""The rest of the system follows the look (Glass or Material): Hyprland's
window corners (hypr/irice/look.lua), the pointer's "auto" (cursor.lua),
the Material sound set and the Material default wallpapers."""
import importlib.util
import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HYPR = ROOT / "hypr" / "irice"
SOUNDS = ROOT / "shell" / "assets" / "sounds"
WALLPAPERS = ROOT / "shell" / "assets" / "wallpapers"

# A stand-in for Hyprland's `hl`: records hl.env and hl.config, ignores the rest
STUB = r'''
local envs, configs = {}, {}
hl = setmetatable({
    env = function(k, v) envs[k] = v end,
    config = function(t) configs[#configs + 1] = t end,
}, { __index = function() return function() end end })
dofile(arg[1])
print("XCURSOR_THEME=" .. tostring(envs.XCURSOR_THEME))
local power
for _, c in ipairs(configs) do
    if c.decoration and c.decoration.rounding_power then power = c.decoration.rounding_power end
end
print("rounding_power=" .. tostring(power))
'''


@unittest.skipUnless(shutil.which("lua"), "lua not installed")
class Hyprland(unittest.TestCase):
    def setUp(self):
        self.home = Path(tempfile.mkdtemp(prefix="irice-look-"))
        (self.home / ".config" / "irice").mkdir(parents=True)
        for theme in ("macOS", "Bibata-Modern-Classic"):
            d = self.home / ".local" / "share" / "icons" / theme
            d.mkdir(parents=True)
            (d / "index.theme").write_text("[Icon Theme]\n")
        self.stub = self.home / "stub.lua"
        self.stub.write_text(STUB)

    def tearDown(self):
        shutil.rmtree(self.home, ignore_errors=True)

    def run_lua(self, script, appearance):
        cfg = {"appearance": appearance, "screenshots": {"cursor": True}}
        (self.home / ".config" / "irice" / "config.json").write_text(json.dumps(cfg, indent=4))
        env = dict(os.environ, HOME=str(self.home), XDG_CONFIG_HOME=str(self.home / ".config"), QT_QPA_PLATFORMTHEME="kde")
        r = subprocess.run(["lua", str(self.stub), str(HYPR / script)], env=env, capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stderr)
        return dict(line.split("=", 1) for line in r.stdout.split())

    def test_auto_cursor_follows_the_look(self):
        self.assertEqual(self.run_lua("cursor.lua", {"look": "material", "cursor": "auto"})["XCURSOR_THEME"], "Bibata-Modern-Classic")
        self.assertEqual(self.run_lua("cursor.lua", {"look": "glass", "cursor": "auto"})["XCURSOR_THEME"], "macOS")

    def test_chosen_cursor_stays(self):
        self.assertEqual(self.run_lua("cursor.lua", {"look": "material", "cursor": "macos"})["XCURSOR_THEME"], "macOS")
        self.assertEqual(self.run_lua("cursor.lua", {"look": "glass", "cursor": "bibata"})["XCURSOR_THEME"], "Bibata-Modern-Classic")

    def test_window_corners_follow_the_look(self):
        # Glass: continuous (squircle) corners; Material: circular arcs
        self.assertEqual(self.run_lua("look.lua", {"look": "glass"})["rounding_power"], "4.0")
        self.assertEqual(self.run_lua("look.lua", {"look": "material"})["rounding_power"], "2.0")


class Sounds(unittest.TestCase):
    def test_material_set_has_every_sound(self):
        want = sorted(p.name for p in (SOUNDS / "marimba").glob("*.ogg"))
        have = sorted(p.name for p in (SOUNDS / "material").glob("*.ogg"))
        self.assertEqual(have, want)

    def test_sounds_qml_knows_the_set(self):
        qml = (ROOT / "shell" / "services" / "Sounds.qml").read_text()
        self.assertIn('"material"', qml)
        self.assertIn('cfg.pack === "auto"', qml)


class Wallpapers(unittest.TestCase):
    def test_bundled_pair(self):
        try:
            from PIL import Image
        except ImportError:
            self.skipTest("Pillow not installed")
        for mode in ("light", "dark"):
            path = WALLPAPERS / f"material-{mode}.jpg"
            with Image.open(path) as im:
                self.assertEqual(im.size, (3840, 2160))
            self.assertLess(path.stat().st_size, 2.5e6)

    def test_generator_is_deterministic(self):
        try:
            import PIL  # noqa: F401
        except ImportError:
            self.skipTest("Pillow not installed")
        spec = importlib.util.spec_from_file_location("mmw", ROOT / "tools" / "make-material-wallpapers.py")
        mmw = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mmw)
        for pal in mmw.PALETTES.values():
            a, b = mmw.render(pal, 192, 108), mmw.render(pal, 192, 108)
            self.assertEqual(a.size, (192, 108))
            self.assertEqual(a.tobytes(), b.tobytes())


if __name__ == "__main__":
    unittest.main()
