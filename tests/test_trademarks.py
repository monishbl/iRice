"""iRice's own features don't wear other companies' product names.

Renamed on 3 November 2026 (SECURITY-AUDIT.md section 7): Spotlight is
Search, Mission Control All Windows, App Exposé App Windows, App Library the
App Drawer, Stage Manager Stages, StandBy Bedside Mode, Smart Stack Widget
Stack, Quick Look Quick Preview, App Store Software, Control Center Quick
Settings, Liquid Glass Glass, and "Tahoe" names nothing of iRice's.

What's read, as the person sees it:
  - QML and JS in shell/ and material/: every string literal outside comments
    (labels, titles, tooltips, details, notifications, SettingsMap's names)
  - the keymap and gestures (hypr/irice/*.json) and their generator
  - Python, shell and C++ strings the installer, `irice`, the health check
    and the scripts show or send (docstrings and comments aren't shown)
  - the website (site/: visible text and alt, aria-label, title and meta
    text) and README.md

Allowed on purpose:
  - search keywords (`keywords: "…"` in services/Search.qml) and Settings
    search's former names (SettingsMap.formerNames), so a search for an old
    name still finds the feature;
  - Apple's own wallpaper files, named as Apple names them ("Tahoe Light.jpg")
    when iRice downloads or looks for them;
  - ids, config keys, IPC names and file names (spotlight, missionControl,
    stageManager, standby, tahoe-day.jpg): they're lower or camel case and
    never shown, and renaming them would break people's settings.

Saying what iRice works with, by its owner's name, is fine and not checked:
"works with Android's Quick Share", "Search an area with Google Lens",
"Sign in with your iCloud account", "like macOS", "Material You colours".
"""
import ast
import html.parser
import json
import re
import shutil
import subprocess
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

# The renamed marks (case matters: ids like `spotlight` aren't shown)
MARKS = {
    "Spotlight": r"\bSpotlight\b",
    "Mission Control": r"\bMission Control\b",
    "Exposé": r"\bExpos[eé]\b",
    "App Library": r"\bApp Library\b",
    "Launchpad": r"\bLaunchpad\b",
    "Stage Manager": r"\bStage Manager\b",
    "StandBy": r"\bStandBy\b",
    "Smart Stack": r"\bSmart Stacks?\b",
    "Quick Look": r"\bQuick Look\b",
    "App Store": r"\bApp Store\b",
    "Control Center": r"\bControl Cent(?:er|re)\b",
    "Dynamic Island": r"\b[Dd]ynamic [Ii]sland\b",
    "Liquid Glass": r"\b[Ll]iquid [Gg]lass\b",
    "Tahoe": r"\bTahoe\b",
    "Finder": r"\bFinder\b",
}
MARK_RX = re.compile("|".join(f"(?:{p})" for p in MARKS.values()))

# Descriptive uses that may contain a mark: removed before checking
ALLOWED = [
    # Apple's wallpapers, as Apple names the files (downloaded on request)
    r"Tahoe (?:Light|Dark)\b",
    r"Tahoe Beach",
    r"\b\d+-Tahoe-[\w-]+\.png",
]
ALLOWED_RX = re.compile("|".join(ALLOWED))

# Settings options that are other companies' product names
OPTION_MARKS = {"macOS", "iOS", "Android", "Finder", "Windows 11"}


def hits(text):
    return MARK_RX.findall(ALLOWED_RX.sub("", text))


def literals(text, comment="//", block=True):
    """(offset, literal body) of each string literal outside comments, for
    QML/JS/C++ ("//" and /* */) or Lua ("--")."""
    i, n = 0, len(text)
    while i < n:
        if text.startswith(comment, i):
            j = text.find("\n", i)
            i = n if j < 0 else j
            continue
        if block and text.startswith("/*", i):
            j = text.find("*/", i + 2)
            i = n if j < 0 else j + 2
            continue
        c = text[i]
        if c in "\"'`":
            j = i + 1
            while j < n and text[j] != c:
                if text[j] == "\\":
                    j += 1
                elif text[j] == "\n" and c != "`":
                    break
                j += 1
            yield i, text[i + 1:j]
            i = j + 1
            continue
        i += 1


def rel(p):
    return str(p.relative_to(ROOT))


def files(*globs):
    out = []
    for g in globs:
        out += [p for p in ROOT.glob(g) if p.is_file() and "node_modules" not in p.parts]
    return sorted(set(out))


def without_former_names(text):
    """SettingsMap.js minus its formerNames table (old names on purpose)."""
    return re.sub(r"var formerNames = \{.*?\n\};", "", text, flags=re.S)


class VisibleText(html.parser.HTMLParser):
    """The text a page shows or reads out: text nodes outside script and
    style, plus alt, aria-label, title, placeholder and meta content."""
    ATTRS = {"alt", "aria-label", "title", "placeholder", "content", "value"}

    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.out = []
        self.skip = 0

    def handle_starttag(self, tag, attrs):
        if tag in ("script", "style"):
            self.skip += 1
        for k, v in attrs:
            if k in self.ATTRS and v:
                self.out.append((self.getpos()[0], v))

    def handle_endtag(self, tag):
        if tag in ("script", "style") and self.skip:
            self.skip -= 1

    def handle_data(self, data):
        if not self.skip and data.strip():
            self.out.append((self.getpos()[0], data))


def python_strings(src):
    """String constants a Python file can show: not docstrings."""
    tree = ast.parse(src)
    docs = set()
    for node in ast.walk(tree):
        if isinstance(node, (ast.Module, ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
            body = node.body
            if body and isinstance(body[0], ast.Expr) and isinstance(body[0].value, ast.Constant) and isinstance(body[0].value.value, str):
                docs.add(id(body[0].value))
    for node in ast.walk(tree):
        if isinstance(node, ast.Constant) and isinstance(node.value, str) and id(node) not in docs:
            yield node.lineno, node.value


def shell_lines(src):
    """Lines a shell script runs or prints: not comments (a heredoc's text
    is printed, so it counts)."""
    for no, line in enumerate(src.splitlines(), 1):
        s = line.strip()
        if s.startswith("#"):
            continue
        # a trailing comment after code
        s = re.sub(r"\s#\s.*$", "", s) if s.count('"') % 2 == 0 and "'" not in s.split(" #")[-1] else s
        yield no, s


class UserFacingText(unittest.TestCase):
    def check(self, found):
        found = [f"{where}: {', '.join(sorted(set(h)))}: {text.strip()[:120]!r}" for where, text, h in found if h]
        self.assertEqual(found, [], "another company's product name as an iRice name:\n" + "\n".join(found))

    def test_qml_and_js(self):
        found = []
        for p in files("shell/**/*.qml", "shell/**/*.js", "material/**/*.qml", "material/**/*.js"):
            text = p.read_text(encoding="utf-8")
            if p.name == "SettingsMap.js":
                text = without_former_names(text)
            for at, lit in literals(text):
                # search keywords keep the old names on purpose
                if re.search(r"\bkeywords\s*:\s*$", text[max(0, at - 40):at]):
                    continue
                found.append((f"{rel(p)}:{text.count(chr(10), 0, at) + 1}", lit, hits(lit)))
        self.check(found)

    def test_keymap_and_gestures(self):
        found = []

        def walk(v, where):
            if isinstance(v, str):
                found.append((where, v, hits(v)))
            elif isinstance(v, dict):
                for k, x in v.items():
                    walk(x, f"{where}.{k}")
            elif isinstance(v, list):
                for i, x in enumerate(v):
                    walk(x, f"{where}[{i}]")
        for p in files("hypr/irice/*.json"):
            walk(json.loads(p.read_text(encoding="utf-8")), rel(p))
        self.check(found)

    def test_python(self):
        found = []
        for p in files("tools/**/*.py", "shell/assets/scripts/**/*.py", "material/assets/scripts/**/*.py", "site/tools/*.py"):
            if p.name.endswith("_pb2.py"):
                continue
            for no, s in python_strings(p.read_text(encoding="utf-8")):
                found.append((f"{rel(p)}:{no}", s, hits(s)))
        self.check(found)

    def test_shell_scripts_and_installer(self):
        found = []
        scripts = files("*.sh", "tools/*.sh", "tools/irice", "shell/assets/scripts/**/*.sh", "material/assets/scripts/**/*.sh", "site/build.sh", "site/public/get", "site/package/*.sh")
        for p in scripts:
            for no, s in shell_lines(p.read_text(encoding="utf-8")):
                found.append((f"{rel(p)}:{no}", s, hits(s)))
        self.check(found)

    def test_cpp_and_lua(self):
        found = []
        for p in files("hypr/**/*.cpp", "hypr/**/*.hpp"):
            text = p.read_text(encoding="utf-8")
            for at, lit in literals(text):
                found.append((f"{rel(p)}:{text.count(chr(10), 0, at) + 1}", lit, hits(lit)))
        for p in files("hypr/**/*.lua", "sandbox/**/*.lua"):
            text = p.read_text(encoding="utf-8")
            for at, lit in literals(text, comment="--", block=False):
                found.append((f"{rel(p)}:{text.count(chr(10), 0, at) + 1}", lit, hits(lit)))
        self.check(found)

    def test_website(self):
        found = []
        for p in files("site/**/*.html", "site/**/*.astro"):
            if "engine" in p.parts:
                continue
            v = VisibleText()
            v.feed(p.read_text(encoding="utf-8"))
            found += [(f"{rel(p)}:{no}", t, hits(t)) for no, t in v.out]
        for p in files("site/**/*.md", "site/**/*.txt"):
            if "engine" in p.parts:
                continue
            for no, line in enumerate(p.read_text(encoding="utf-8").splitlines(), 1):
                found.append((f"{rel(p)}:{no}", line, hits(line)))
        self.check(found)

    def test_readme(self):
        text = (ROOT / "README.md").read_text(encoding="utf-8")
        self.check([(f"README.md:{no}", line, hits(line)) for no, line in enumerate(text.splitlines(), 1)])

    def test_options_are_not_product_names(self):
        """A choice in Settings is named for what it does ("Strip",
        "Classic"), not after another system ("macOS"). Key names for a
        keyboard ("⌘ macOS") say which keys you have, and stay."""
        found = []
        for p in files("shell/modules/settings/**/*.qml", "material/modules/settings/**/*.qml"):
            text = p.read_text(encoding="utf-8")
            for m in re.finditer(r"\boptions\s*:\s*\[([^\]]*)\]", text):
                names = set(re.findall(r'"([^"]*)"', m.group(1)))
                bad = sorted(names & OPTION_MARKS)
                if bad:
                    found.append((f"{rel(p)}:{text.count(chr(10), 0, m.start()) + 1}", m.group(0), bad))
        self.check(found)

    def test_the_scanner_finds_marks(self):
        # (so a broken scanner can't pass quietly)
        self.assertTrue(hits('text: "Open Spotlight"'))
        self.assertTrue(hits("Liquid glass, like macOS"))
        self.assertFalse(hits('"Tahoe Light.jpg"'))
        self.assertFalse(hits("panel: spotlight; missionControl; stageManager"))
        lits = [s for _, s in literals('a: "x Spotlight" // "Mission Control"\n/* "StandBy" */ b: \'App Store\'')]
        self.assertEqual(lits, ["x Spotlight", "App Store"])


class FormerNamesStillFind(unittest.TestCase):
    """Searching an old name finds the feature under its new one."""
    RENAMED = {
        "spotlight": "search",
        "mission control": "all windows",
        "app exposé": "app windows",
        "app library": "app drawer",
        "stage manager": "stages",
        "standby": "bedside mode",
        "smart stack": "widget stack",
        "quick look": "quick preview",
        "app store": "software",
        "control center": "quick settings",
        "liquid glass": "glass",
    }

    def test_settings_search_knows_every_former_name(self):
        text = (ROOT / "shell/common/SettingsMap.js").read_text(encoding="utf-8")
        table = re.search(r"var formerNames = \{(.*?)\n\};", text, re.S)
        self.assertIsNotNone(table)
        pairs = dict(re.findall(r'"([^"]+)":\s*"([^"]+)"', table.group(1)))
        for old, new in self.RENAMED.items():
            self.assertEqual(pairs.get(old), new, old)
        # both looks' Settings search through it
        for f in ("shell/modules/settings/Settings.qml", "material/modules/settings/Settings.qml"):
            self.assertIn("SettingsMap.searchQuery(", (ROOT / f).read_text(encoding="utf-8"), f)

    @unittest.skipUnless(shutil.which("node"), "node isn't installed")
    def test_search_query_swaps_old_names(self):
        js = r"""
const fs = require("fs"), vm = require("vm");
const ctx = {};
vm.runInNewContext(fs.readFileSync(process.argv[1], "utf8").replace(/^\.pragma library\s*$/m, ""), ctx);
console.log(JSON.stringify(["spotlight", "mission control", "standby clock", "app exposé", "dock"].map(ctx.searchQuery)));
"""
        out = subprocess.run(["node", "-e", js, str(ROOT / "shell/common/SettingsMap.js")], capture_output=True, text=True, timeout=30)
        self.assertEqual(out.returncode, 0, out.stderr)
        self.assertEqual(json.loads(out.stdout), ["search", "all windows", "bedside mode clock", "app windows", "dock"])

    def test_shell_search_keeps_old_names_as_keywords(self):
        text = (ROOT / "shell/services/Search.qml").read_text(encoding="utf-8")
        actions = {m.group(1): m.group(2) for m in re.finditer(r'\{ title: "([^"]+)", glyph: "[^"]+", keywords: "([^"]+)"', text)}
        for title, old in {"Stages": "stage manager", "Software": "app store", "App Drawer": "app library",
                           "All Windows": "mission control", "Quick Settings": "control center"}.items():
            self.assertIn(title, actions)
            self.assertIn(old, actions[title], title)


if __name__ == "__main__":
    unittest.main()
