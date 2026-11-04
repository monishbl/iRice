"""Tessera's licence (GPL-3.0-or-later) and the notices for what it bundles
from others (THIRD_PARTY_NOTICES.md, the texts in LICENSES/)."""
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(p):
    return (ROOT / p).read_text(encoding="utf-8")


class Licence(unittest.TestCase):
    def test_gpl3_text_is_complete(self):
        text = read("LICENSE")
        self.assertTrue(text.lstrip().startswith("GNU GENERAL PUBLIC LICENSE"))
        self.assertIn("Version 3, 29 June 2007", text)
        self.assertIn("END OF TERMS AND CONDITIONS", text)
        self.assertIn("How to Apply These Terms to Your New Programs", text)

    def test_stated_as_or_later(self):
        self.assertIn("GPL-3.0-or-later", read("README.md"))
        self.assertIn("## Licence", read("README.md"))
        self.assertIn("GPL-3.0-or-later", read("site/public/index.html"))
        self.assertIn("GPL-3.0-or-later", read("THIRD_PARTY_NOTICES.md"))

    def test_licence_texts(self):
        apache = read("LICENSES/Apache-2.0.txt")
        self.assertIn("Apache License", apache)
        self.assertIn("Version 2.0, January 2004", apache)
        self.assertIn("END OF TERMS AND CONDITIONS", apache)
        ofl = read("LICENSES/OFL-1.1.txt")
        self.assertIn("SIL OPEN FONT LICENSE Version 1.1", ofl)
        self.assertIn("DISCLAIMER", ofl)
        self.assertIn("UNICODE LICENSE V3", read("LICENSES/Unicode-3.0.txt"))
        # the bundled fonts keep their own copies, next to them
        for p in ("shell/assets/fonts/LICENSE-phosphor", "material/assets/fonts/LICENSE-google-sans-flex",
                  "material/assets/fonts/LICENSE-material-symbols", "material/assets/fonts/NOTICE"):
            self.assertTrue((ROOT / p).is_file(), p)

    def test_notices_cover_what_is_bundled(self):
        text = read("THIRD_PARTY_NOTICES.md")
        for part in ("Phosphor", "Google Sans Flex", "Tessera Sans Flex", "Reserved Font Name", "Material Symbols",
                     "Tessera Symbols", "material-color-utilities", "Nearby", "UKEY2", "emoji-test.txt", "CLDR",
                     "emojibase", "RQuickShare", "end4", "Geist", "Unsplash"):
            self.assertIn(part, text, part)
        # modified Apache-2.0 files say so (section 4(b))
        self.assertRegex(text, r"Changed by Tessera[^\n]*subset")
        # every bundled sound is Tessera's, made by its generator (the Material
        # set too, once of unknown source)
        self.assertRegex(text, r"material/assets/sounds/material[^\n]*made by Tessera[^\n]*make-sounds\.py material")
        self.assertNotIn("source unrecorded", text)

    def test_licence_paths_named_in_files_exist(self):
        """A header that points at a licence file points at one that's there
        (MaterialColor.js once pointed at shell/assets/fonts/, where it isn't)."""
        for f in ("shell/common/MaterialColor.js", "tools/gen-material.sh", "shell/assets/quickshare/proto/NOTICE",
                  "README.md", "THIRD_PARTY_NOTICES.md"):
            text = read(f)
            for p in set(re.findall(r"\b((?:LICENSES|shell|material|site)/[\w./-]*(?:LICENSE|NOTICE|OFL)[\w.-]*)", text)):
                self.assertTrue((ROOT / p).exists(), f"{f}: {p}")


if __name__ == "__main__":
    unittest.main()
