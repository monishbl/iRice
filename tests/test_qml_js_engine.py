"""JavaScript the shell can't run.

QML runs its JavaScript in Qt's own engine, which lacks some newer array
methods. The logic tests run in Node, which has them, so a call like
`list.flatMap(...)` passes there and then throws in the shell: a monthly
repeating event once broke every calendar widget that way. This scans the
shell's QML and JS for those methods.
"""
import re
import unittest
from pathlib import Path

SHELL = Path(__file__).resolve().parent.parent / "shell"
MISSING = re.compile(r"\.(flatMap|flat|findLast|findLastIndex|toSorted|toReversed|toSpliced)\(")


class EngineCompatibility(unittest.TestCase):
    def test_no_array_methods_the_engine_lacks(self):
        found = []
        for path in sorted(SHELL.rglob("*")):
            if path.suffix not in (".qml", ".js") or "build" in path.parts:
                continue
            for n, line in enumerate(path.read_text(errors="replace").splitlines(), 1):
                code = line.split("//", 1)[0]
                if MISSING.search(code):
                    found.append(f"{path.relative_to(SHELL)}:{n}: {line.strip()}")
        self.assertEqual(found, [], "Qt's JavaScript engine lacks these; use concat/reduce/slice:\n" + "\n".join(found))


if __name__ == "__main__":
    unittest.main()
