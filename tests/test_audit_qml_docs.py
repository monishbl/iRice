"""The 20 Nov audit's fixes in the QML fallback, the scripts that restart a
shell, the website and the docs: each rule as it stands now, so it can't
quietly come undone. Nothing here touches the real system: files are read
from the checkout, and site/build.sh's scrub runs on a throwaway copy."""
import json
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(rel):
    return (ROOT / rel).read_text(encoding="utf-8")


class RestartsDecideAfresh(unittest.TestCase):
    """#6: the QML shell tessera-ui became when EGL didn't start has
    TESSERA_STANDALONE=0; Try again (repair-shell.sh) and an update's restart
    (update-run.sh) must not hand it on, or the launcher picks QML again."""

    def strip_is_unconditional(self, rel, guard):
        text = read(rel)
        strip = text.find('[[ $e == TESSERA_STANDALONE=* ]] || fresh+=("$e")')
        self.assertGreater(strip, 0, rel)
        start = text.find(guard)
        self.assertGreater(start, 0, rel)
        fi = text.find("\n    fi\n", start)
        self.assertGreater(fi, start, rel)
        # the strip comes after the template-only block closed, not inside it
        self.assertGreater(strip, fi, f"{rel}: TESSERA_STANDALONE is only dropped for a tessera-ui template")
        self.assertIn('tenv=("${fresh[@]}")', text[strip:strip + 200], rel)

    def test_repair_shell(self):
        self.strip_is_unconditional("shell/assets/scripts/repair-shell.sh", 'if is_standalone "$template"; then')

    def test_update_run(self):
        self.strip_is_unconditional("shell/assets/scripts/update-run.sh", '== tessera-ui ]]; then')

    def test_the_launcher_still_honours_an_explicit_zero(self):
        # (=0 by hand still forbids standalone: only the restarts drop it)
        self.assertIn('[[ "${TESSERA_STANDALONE:-}" == 0 ]] && return 1', read("shell/assets/scripts/launch-shell.sh"))


class FallbackHandsNothingToRust(unittest.TestCase):
    """#45: in the basic fallback, a hybrid-era rust.surfaces list doesn't
    start `tessera-ui --surfaces` (the lock among them)."""

    def test_wanted_is_empty_in_fallback(self):
        qml = read("shell/services/RustUi.qml")
        self.assertRegex(qml, r"readonly property list<string> wanted: Config\.ready && !fallback \? Config\.options\.rust\.surfaces : \[\]")
        self.assertIn('Quickshell.env("TESSERA_STANDALONE") === "0"', qml)
        # the same note FallbackNotice shows
        self.assertIn('Paths.stateHome + "/tessera/fallback"', qml)
        self.assertIn('readonly property string note: dir + "/fallback"', read("shell/modules/fallback/FallbackNotice.qml"))
        self.assertIn('readonly property string dir: Paths.stateHome + "/tessera"', read("shell/modules/fallback/FallbackNotice.qml"))


class TenMinutesStayTen(unittest.TestCase):
    """#125: the fallback reads the end the Rust shell kept for "Everyone for
    10 min" instead of granting a fresh 10 minutes, and clears it after."""

    def test_same_file_as_rust(self):
        rust = read("native/tessera-ui/host/sharing/src/visibility.rs")
        self.assertIn('join("tessera/sharing-visible-until")', rust)
        qml = read("shell/services/QuickShare.qml")
        self.assertIn('Paths.stateHome + "/tessera/sharing-visible-until"', qml)

    def test_load_reads_the_kept_end_and_a_choice_starts_fresh(self):
        qml = read("shell/services/QuickShare.qml")
        self.assertIn("everyoneUntil = Config.ready ? Date.now() + 10 * 60000 : keptDeadline();", qml)
        # (visibility.rs deadline(): kept only when no more than 10 min away)
        self.assertIn("kept > 0 && kept <= now + 10 * 60000 ? kept : now + 10 * 60000", qml)
        # turned off (or to Everyone) after loading: the kept end goes
        self.assertRegex(qml, r"\} else if \(Config\.ready\) \{\s*keepUntil\(0\);")
        self.assertIn('Quickshell.execDetached(["rm", "-f", root.untilPath]);', qml)
        local = read("shell/services/LocalSend.qml")
        self.assertIn("Config.ready ? Date.now() + 10 * 60000 : QuickShare.keptDeadline()", local)


class FallbackKeepsRustOnlySettings(unittest.TestCase):
    """#126: the QML Config writes only the keys it declares, so every key
    the Rust shell writes with set_config is declared there too."""

    def declared(self):
        text = read("shell/common/Config.qml")
        out = {}
        for m in re.finditer(r"property JsonObject (\w+): JsonObject \{", text):
            depth, i = 1, m.end()
            while depth and i < len(text):
                depth += {"{": 1, "}": -1}.get(text[i], 0)
                i += 1
            body = text[m.end():i]
            out.setdefault(m.group(1), set()).update(re.findall(r"property [\w<>]+ (\w+)", body))
        return out

    def test_every_key_rust_writes_is_declared(self):
        written = set()
        for p in (ROOT / "native/tessera-ui").rglob("*.rs"):
            if {"target", "vendor", "review"} & set(p.relative_to(ROOT).parts):
                continue
            written |= set(re.findall(r'(?:set_config|config_set)\(\s*"(\w+)\.(\w+)"', p.read_text(encoding="utf-8", errors="replace")))
        self.assertIn(("performance", "gpu"), written)
        have = self.declared()
        missing = sorted(f"{s}.{k}" for s, k in written if k not in have.get(s, set()))
        self.assertEqual(missing, [], "keys the Rust shell writes that the QML fallback would drop")

    def test_gpu_default_is_rusts(self):
        # (core/src/gpu.rs chosen(): "low" when unset)
        self.assertIn('.unwrap_or_else(|| "low".into())', read("native/tessera-ui/core/src/gpu.rs"))
        self.assertIn('property string gpu: "low"', read("shell/common/Config.qml"))


class SharedSettingsCarryNoAccount(unittest.TestCase):
    """#90: the website's settings package keeps no account's calendar or
    task list, and no face camera."""

    def scrub(self, cfg):
        script = read("site/build.sh")
        py = re.search(r"<<'PY'\n(.*?)\nPY\n", script, re.S).group(1)
        with tempfile.TemporaryDirectory() as d:
            src, out = Path(d, "in.json"), Path(d, "out.json")
            src.write_text(json.dumps(cfg))
            r = subprocess.run([sys.executable, "-I", "-c", py, str(src), str(out)], capture_output=True, text=True, timeout=30)
            return r, (json.loads(out.read_text()) if out.exists() else None)

    def test_account_keys_are_dropped(self):
        r, d = self.scrub({"google": {"newEventCalendar": "cloud|ms-1|cal1", "taskListId": "google|a|b", "hiddenCalendars": ["x"], "other": 1},
                           "cloud": {"hiddenCalendars": ["y"]}, "lock": {"faceDevice": "/dev/video2", "faceCount": 2, "face": True},
                           "sharing": {"name": "Someone's laptop"}})
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(d["google"], {"other": 1})
        self.assertEqual(d["cloud"], {})
        self.assertEqual(d["lock"], {"face": True})
        self.assertEqual(d["sharing"]["name"], "")

    def test_an_account_id_elsewhere_stops_the_build(self):
        r, _ = self.scrub({"calendar": {"default": "google|me@example.com|primary"}})
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("account", r.stderr)

    def test_the_kept_package_is_clean(self):
        d = json.loads(read("site/package/config.json"))
        text = json.dumps(d)
        self.assertNotRegex(text, r'"(?:cloud|google)\|')
        for sec, key in (("google", "newEventCalendar"), ("google", "taskListId"), ("lock", "faceDevice"), ("lock", "faceCount")):
            self.assertNotIn(key, d.get(sec, {}), f"{sec}.{key}")


class DocsSayWhatIs(unittest.TestCase):
    def test_site_exit_takes_two_presses(self):
        # (#92: the session ends on a second press within 3 s)
        html = read("site/public/index.html")
        self.assertNotIn("<kbd>Esc</kbd> ends it.", html)
        self.assertIn("<kbd>Esc</kbd> twice", html)

    def test_no_an_tessera(self):
        for rel in ("site/public/get", "site/public/index.html", "README.md", "CHANGELOG.md"):
            self.assertNotRegex(read(rel), r"\b[Aa]n Tessera\b", rel)

    def test_fallback_window_is_a_minute(self):
        # (#86: main.rs clears the count after 60 s)
        self.assertIn("from_secs(60)", read("native/tessera-ui/src/main.rs"))
        self.assertNotIn("stayed up 5 minutes", read("shell/assets/scripts/launch-shell.sh"))
        self.assertNotIn("didn't last 5 minutes (or", read("native/tessera-ui/RUST-ONLY.md"))

    def test_bug_worker_docs_use_the_database_name(self):
        # (#91: wrangler.jsonc's database_name, kept on purpose)
        name = re.search(r'"database_name":\s*"([^"]+)"', read("site/bugs/wrangler.jsonc")).group(1)
        readme = read("site/bugs/README.md")
        self.assertIn(f"d1 migrations apply {name} ", readme)
        self.assertIn(f"d1 create {name}", readme)

    def test_readme_describes_the_rust_shell(self):
        # (#35)
        readme = read("README.md")
        self.assertNotIn("written from scratch in [Quickshell]", readme)
        self.assertIn("native/tessera-ui", readme)
        self.assertNotIn("Settings → Sharing", readme)
        self.assertNotIn("subscribed calendars are read-only (CalDAV", readme)
        self.assertNotIn("Apple's macOS wallpapers", readme)

    def test_pending_is_current(self):
        # (#89: no 4 Nov plan; one row per change in RUST-ONLY)
        self.assertNotIn("helpers first", read("PENDING.md"))
        rows = [l for l in read("native/tessera-ui/RUST-ONLY.md").splitlines() if l.startswith("| Nearby sharing (17 Nov, branch `sharing-fix`)")]
        self.assertEqual(len(rows), 1)


if __name__ == "__main__":
    unittest.main()
