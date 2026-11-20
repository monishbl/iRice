"""Settings shows what the shell does (20 Nov audit #36): every default a
Rust reader falls back to when a setting isn't in config.json is the one
Config.qml has (Settings, the QML fallback and the updater's conflict check
all read Config.qml's). The scan looks at the readers written the usual
ways: `cfg["a"]["b"].as_bool().unwrap_or(X)` and `cfg_num(cfg, "a.b", X)`.
A reader that isn't asking for the default (a one-time migration looking
for an old value) is listed in NOT_DEFAULTS with why."""
import importlib.machinery
import importlib.util
import re
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
RUST = ROOT / "native/tessera-ui"

# (file, setting): readers that compare with something other than the default
NOT_DEFAULTS = {
    # (Config.qml's onLoaded migrations: looking for old values)
    ("host/shell/src/common.rs", "appearance.menuIcon"),
    ("host/shell/src/common.rs", "screenshot.editor"),
    ("host/shell/src/common.rs", "workspaces.dimOthers"),
    ("host/shell/src/common.rs", "appearance.iconStyle"),
    ("host/shell/src/common.rs", "appearance.cursor"),
    # ("" reads as the dynamic face, Config.qml's default, in material_clock)
    ("surfaces/greeter/src/view.rs", "lock.clockFace"),
    # ("" reads as metric, Config.qml's default, in weather::settings)
    ("host/desktop/src/today/weather.rs", "weather.units"),
}

INDEXED = re.compile(r'(?:cfg\(\)|cfg|\bc|\ba)((?:\["[A-Za-z0-9_]+"\]){1,3})\s*\.as_(?:bool|f64|i64|u64|str)\(\)\s*'
                     r'\.unwrap_or\(\s*("(?:[^"\\]|\\.)*"|-?[0-9.]+|true|false)\s*\)')
HELPER = re.compile(r'cfg_(?:num|bool|str)\(\s*[a-z_&.()]+,\s*"([A-Za-z0-9_.]+)",\s*("(?:[^"\\]|\\.)*"|-?[0-9.]+|true|false)\s*\)')
CURSOR = re.compile(r'\["appearance"\]\["cursor"\]\s*==\s*"bibata"')


def defaults():
    loader = importlib.machinery.SourceFileLoader("tu_defaults", str(ROOT / "tools/tessera-update.py"))
    spec = importlib.util.spec_from_loader("tu_defaults", loader)
    mod = importlib.util.module_from_spec(spec)
    loader.exec_module(mod)
    return mod.config_defaults((ROOT / "shell/common/Config.qml").read_text())


def literal(v):
    if v in ("true", "false"):
        return v == "true"
    if v.startswith('"'):
        return v[1:-1]
    return float(v)


def same(rust, qml):
    if isinstance(qml, bool) or isinstance(rust, bool):
        return rust is qml
    if isinstance(qml, (int, float)) and isinstance(rust, float):
        return abs(qml - rust) < 1e-9
    return rust == qml


def sources():
    for p in sorted(RUST.rglob("*.rs")):
        if "target" in p.parts or "vendor" in p.parts:
            continue
        yield str(p.relative_to(RUST)), p.read_text(errors="replace")


class RustDefaults(unittest.TestCase):
    def test_every_rust_default_is_config_qmls(self):
        d = defaults()
        wrong = []
        for rel, text in sources():
            found = [(m, ".".join(re.findall(r'"([^"]+)"', m.group(1))), m.group(2)) for m in INDEXED.finditer(text)]
            found += [(m, m.group(1), m.group(2)) for m in HELPER.finditer(text)]
            for m, key, value in found:
                if key not in d or (rel, key) in NOT_DEFAULTS:
                    continue
                if not same(literal(value), d[key]):
                    line = text[:m.start()].count("\n") + 1
                    wrong.append(f"{rel}:{line} {key}: Rust {value}, Config.qml {d[key]!r}")
        self.assertEqual(wrong, [], "Settings would show one thing and the shell do another")

    def test_the_scan_sees_the_readers_it_checks(self):
        # (the patterns still match how the readers are written)
        text = dict(sources())
        self.assertRegex(text["surfaces/wallpaper/src/background/mod.rs"], INDEXED)
        self.assertRegex(text["host/shell/src/sounds.rs"], HELPER)

    def test_the_pointer_theme_reads_a_missing_setting_as_bibata(self):
        # (the real pointer does, system.rs: so do the app theme and the shake pointer)
        for rel, text in sources():
            self.assertNotRegex(text, CURSOR, rel)


if __name__ == "__main__":
    unittest.main()
