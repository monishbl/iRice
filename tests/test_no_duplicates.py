"""No copied code between the looks. A material/ file overrides the shell/
file at the same path; it should only exist where Material's view is really
different, and never repeat the Glass file's logic. Each override has a
budget of lines it may share with its Glass counterpart
(duplication_budget.json): the refactor lowers them, nothing may raise them,
and a new override may share at most a quarter of its Glass file. When you
merge an override back or shrink it, run this with UPDATE_BUDGET=1 to write
the lower numbers."""
import difflib
import json
import os
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BUDGET_FILE = Path(__file__).resolve().parent / "duplication_budget.json"
NEW_SHARE = 0.25


def overrides():
    for m in sorted((ROOT / "material").rglob("*.qml")):
        s = ROOT / "shell" / m.relative_to(ROOT / "material")
        if s.exists():
            yield str(m.relative_to(ROOT / "material")), s, m


def shared_lines(a, b):
    a, b = a.read_text().split("\n"), b.read_text().split("\n")
    return sum(bl.size for bl in difflib.SequenceMatcher(None, a, b, autojunk=False).get_matching_blocks()), len(a)


class NoDuplicates(unittest.TestCase):
    def test_overrides_stay_within_their_budget(self):
        budget = json.loads(BUDGET_FILE.read_text())
        now, over = {}, []
        for rel, s, m in overrides():
            shared, glass = shared_lines(s, m)
            now[rel] = shared
            if rel in budget:
                if shared > budget[rel]:
                    over.append(f"{rel}: shares {shared} lines with shell/ (budget {budget[rel]})")
            elif shared > NEW_SHARE * glass:
                over.append(f"{rel}: a new override sharing {shared} of {glass} lines with shell/ "
                            "(put the shared part in shell/ and keep only Material's view here)")
        if os.environ.get("UPDATE_BUDGET") == "1":
            lowered = {k: min(v, budget.get(k, v)) for k, v in now.items() if k in budget}
            BUDGET_FILE.write_text(json.dumps(lowered, indent=1, sort_keys=True) + "\n")
        self.assertEqual(over, [], "\n" + "\n".join(over))

    def test_the_budget_lists_no_removed_overrides(self):
        # (merged back: its line goes, so the total can only fall)
        budget = json.loads(BUDGET_FILE.read_text())
        gone = [rel for rel in budget if not (ROOT / "material" / rel).exists()]
        self.assertEqual(gone, [], "run with UPDATE_BUDGET=1 to drop: " + ", ".join(gone))


if __name__ == "__main__":
    unittest.main()
