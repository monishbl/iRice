"""The fonts the Material look loads answer to the names its code asks for.

The shell loads material/assets/fonts by file (FontLoader) and then asks for
them by family name ("Tessera Symbols" for the icons, "Tessera Sans Flex"
for the type). tools/gen-material.sh renames them in their name tables; a
font left with an older name (as after the iRice → Tessera rename) makes
every Material icon fall back to an unrelated glyph and the type to the
system font, with no error anywhere.
"""
import re
import shutil
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FONTS = ROOT / "material/assets/fonts"


def families(path):
    out = subprocess.run(["fc-scan", "--format", "%{family}\n", str(path)], capture_output=True, text=True, check=True).stdout
    return {f for line in out.splitlines() for f in line.split(",") if f}


def asked_for():
    """Family names in the shell's QML and JS that name one of these fonts."""
    names = set()
    for p in list(ROOT.glob("shell/**/*.qml")) + list(ROOT.glob("shell/**/*.js")) + list(ROOT.glob("material/**/*.qml")):
        for m in re.finditer(r"['\"](Tessera (?:Symbols|Sans Flex))['\"]", p.read_text(errors="replace")):
            names.add(m.group(1))
    return names


@unittest.skipUnless(shutil.which("fc-scan"), "needs fc-scan (fontconfig)")
class MaterialFontNames(unittest.TestCase):
    def test_every_name_asked_for_is_a_bundled_font(self):
        have = set()
        for f in FONTS.glob("*.ttf"):
            have |= families(f)
        wanted = asked_for()
        self.assertTrue(wanted, "found no font names in the shell's code")
        self.assertEqual(sorted(wanted - have), [], f"asked for but not in material/assets/fonts (it has {sorted(have)})")

    def test_no_font_keeps_an_old_name(self):
        for f in FONTS.glob("*.ttf"):
            for fam in families(f):
                self.assertNotIn("iRice", fam, f"{f.name} is still called {fam!r}")


if __name__ == "__main__":
    unittest.main()
