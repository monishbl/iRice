"""Nothing anchors to a Glass's `shape` from outside that Glass.

`shape` is a child of the Glass item, so only the Glass's own children
(`parent.shape` from inside it) may anchor to it. Anchoring a sibling to
`glass.shape` is refused at run time ("Cannot anchor to an item that isn't a
parent or sibling") and the item keeps no size: Material's fills over a glass
panel (the screen tools bar, the password prompt) drew nothing. Fill the
Glass's own box inset by its shadow margin instead:
`anchors { fill: parent; margins: glass.shadowMargin }`.
"""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[1]
OUTSIDE = re.compile(r"anchors\.(?:fill|centerIn)\s*:\s*(?!parent\b)(\w+)\.shape\b")


class GlassShapeAnchors(unittest.TestCase):
    def test_no_anchor_to_another_items_shape(self):
        found = []
        for top in ("shell", "material"):
            for f in sorted((ROOT / top).rglob("*.qml")):
                for n, line in enumerate(f.read_text(errors="replace").splitlines(), 1):
                    if OUTSIDE.search(line):
                        found.append(f"{f.relative_to(ROOT)}:{n}: {line.strip()}")
        self.assertEqual(found, [], "anchored to a Glass's shape from outside it:\n" + "\n".join(found))


if __name__ == "__main__":
    unittest.main()
