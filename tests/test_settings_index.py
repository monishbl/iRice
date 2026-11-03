"""Settings search finds only what the look shows. Both looks share the page
files, so SettingsIndex.js leaves a part shown in one look only (`visible:`
or `active:` Theme.material / !Theme.material) out of the other look's
index, and takes `Theme.material ? "a" : "b"` captions in the look's words.
"""
import json
import shutil
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
INDEX = ROOT / "shell/modules/settings/SettingsIndex.js"
PAGES = ROOT / "shell/modules/settings/pages"
# (the pattern Settings.qml runs over the pages)
GREP = ["grep", "-HnE", r"[{}]|(label|title|detail|pageTitle): |(visible|active): *!?Theme[.]material"]

RUN = r"""
const fs = require("fs"), vm = require("vm");
const ctx = {};
vm.runInNewContext(fs.readFileSync(process.argv[1], "utf8").replace(/^\.pragma library\s*$/m, ""), ctx);
const text = fs.readFileSync(0, "utf8");
console.log(JSON.stringify([ctx.captions(text, false), ctx.captions(text, true)]));
"""


def captions(text):
    out = subprocess.run(["node", "-e", RUN, str(INDEX)], input=text, capture_output=True, text=True, timeout=30)
    if out.returncode != 0:
        raise AssertionError(out.stderr)
    return json.loads(out.stdout)


def grep(text):
    return "\n".join(f"page.qml:{n}:{line}" for n, line in enumerate(text.splitlines(), 1))


@unittest.skipUnless(shutil.which("node"), "node isn't installed")
class SettingsIndex(unittest.TestCase):
    def test_one_look_parts_are_left_out_of_the_other(self):
        glass, material = captions(grep("""Column {
    pageTitle: Theme.material ? "Match apps" : "Glass"
    Group {
        title: "Both"
        Row { label: "Shared row" }
        Row {
            label: "Material row"
            visible: Theme.material
        }
        Row { visible: !Theme.material && ready; label: "Glass row" }
    }
    Group {
        visible: Theme.material
        Row { label: "In a Material group" }
    }
    Loader {
        active: Theme.material
        sourceComponent: Component { Text { property string label: "Loaded for Material" } }
    }
    Row { label: "After" }
}
"""))
        texts = lambda lines: [line.split(": ", 1)[1].strip('"') for line in lines]
        self.assertEqual(texts(glass), ["Glass", "Both", "Shared row", "Glass row", "After"])
        self.assertEqual(texts(material), ["Match apps", "Both", "Shared row", "Material row",
                                           "In a Material group", "Loaded for Material", "After"])
        self.assertEqual(glass[0], 'page.qml:2:pageTitle: "Glass"')

    def test_the_pages_index_each_look_its_own_rows(self):
        files = sorted(p.name for p in PAGES.glob("*.qml"))
        text = subprocess.run(GREP + files, cwd=PAGES, capture_output=True, text=True, timeout=30).stdout
        glass, material = captions(text)
        has = lambda lines, s: any(line.endswith(f': "{s}"') for line in lines)
        for s in ("Colour style", "Glass belongs to the Glass look", "App names in the App Drawer"):
            self.assertFalse(has(glass, s), s)
            self.assertTrue(has(material, s), s)
        for s in ("Accent colour", "Reduce transparency", "Index of refraction"):
            self.assertTrue(has(glass, s), s)
            self.assertFalse(has(material, s), s)


if __name__ == "__main__":
    unittest.main()
