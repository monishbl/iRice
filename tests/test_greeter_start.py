"""The login screen's start (shell/assets/scripts/greeter-start.sh): the
style and look from greeter.json, a config.json of the greeter's own, the
Material root assembled for it, and the text one when another won't start.
A fake qs stands in; nothing is shown."""
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPT = ROOT / "shell/assets/scripts/greeter-start.sh"

# Says how it was started; the Material root "fails" when FAKE_FAIL is set
FAKE_QS = r'''#!/bin/sh
cfg=""
[ -n "$XDG_CONFIG_HOME" ] && cfg=$(cat "$XDG_CONFIG_HOME/tessera/config.json")
printf '%s\t%s\t%s\n' "$TESSERA_GREETER_STYLE" "$2" "$cfg" >> "$FAKE_LOG"
case "$2" in *material-shell*) [ -n "$FAKE_FAIL" ] && exit 3 ;; esac
exit 0
'''


class GreeterStart(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.d = Path(self.tmp.name)
        self.share = self.d / "share"
        self.share.mkdir()
        (self.share / "shell").symlink_to(ROOT / "shell")
        (self.share / "material").symlink_to(ROOT / "material")
        self.qs = self.d / "qs"
        self.qs.write_text(FAKE_QS)
        self.qs.chmod(0o755)
        self.log = self.d / "log"
        (self.d / "run").mkdir()

    def tearDown(self):
        self.tmp.cleanup()

    def start(self, look=None, **env):
        if look is not None:
            (self.share / "greeter.json").write_text(json.dumps(look))
        e = dict(os.environ, TESSERA_SHARE=str(self.share), TESSERA_QS=str(self.qs), FAKE_LOG=str(self.log), XDG_RUNTIME_DIR=str(self.d / "run"), **env)
        r = subprocess.run(["bash", str(SCRIPT)], env=e, capture_output=True, text=True, timeout=60)
        runs = [line.split("\t") for line in self.log.read_text().splitlines()] if self.log.exists() else []
        return r, runs

    LOOK = {"style": "look", "look": "material", "mode": "light", "seed": "#6750A4", "materialStyle": "vibrant",
            "clockFace": "dynamic", "clockWeight": 300, "clockWidth": 40, "clockRound": False}

    def test_material_gets_its_root_and_your_settings(self):
        r, runs = self.start(self.LOOK)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(len(runs), 1)
        style, root, cfg = runs[0]
        self.assertEqual(style, "look")
        self.assertIn("material-shell", root)
        c = json.loads(cfg)
        self.assertEqual(c["appearance"], {"look": "material", "mode": "light", "materialSeed": "#6750A4", "materialStyle": "vibrant"})
        self.assertEqual(c["lock"], {"clockFace": "dynamic", "clockWeight": 300, "clockWidth": 40, "clockRound": False})
        self.assertEqual(c["wallpaper"]["path"], str(self.share / "wallpaper.jpg"))
        self.assertEqual(list((self.d / "run").iterdir()), [])  # (its folder goes when it ends)

    def test_glass_and_the_others_run_the_shell(self):
        for style, look in (("look", "glass"), ("plain", "material"), ("text", "material")):
            self.log.unlink(missing_ok=True)
            _, runs = self.start(dict(self.LOOK, style=style, look=look))
            self.assertEqual([(s, rt) for s, rt, _ in runs], [(style, str(self.share / "shell") + "/greeter.qml")])

    def test_one_that_wont_start_gives_way_to_text(self):
        r, runs = self.start(self.LOOK, FAKE_FAIL="1")
        self.assertEqual(r.returncode, 0)
        self.assertEqual([s for s, _, _ in runs], ["look", "text"])
        self.assertIn("text one instead", r.stderr)

    def test_without_greeter_json(self):
        _, runs = self.start()
        self.assertEqual(runs[0][0], "look")
        self.assertEqual(runs[0][1], str(self.share / "shell") + "/greeter.qml")


if __name__ == "__main__":
    unittest.main()


# The Rust login screen (tessera-ui --greeter): says how it was started;
# fails at once when FAKE_UI_FAIL is set
FAKE_UI = r'''#!/bin/sh
cfg=""
[ -n "$XDG_CONFIG_HOME" ] && cfg=$(cat "$XDG_CONFIG_HOME/tessera/config.json")
printf 'rust\t%s\t%s\n' "$*" "$cfg" >> "$FAKE_LOG"
[ -n "$FAKE_UI_FAIL" ] && exit 4
exit 0
'''


class GreeterStartRust(GreeterStart):
    def setUp(self):
        super().setUp()
        self.ui = self.d / "tessera-ui"
        self.ui.write_text(FAKE_UI)
        self.ui.chmod(0o755)

    def start(self, look=None, **env):
        return super().start(look, TESSERA_GREETER_UI=str(self.ui), **env)

    # (the QML-only cases above don't apply when the Rust one runs)
    test_material_gets_its_root_and_your_settings = None
    test_glass_and_the_others_run_the_shell = None
    test_one_that_wont_start_gives_way_to_text = None
    test_without_greeter_json = None

    def test_rust_runs_with_the_style_look_and_settings(self):
        r, runs = self.start(self.LOOK)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(len(runs), 1)
        who, args, cfg = runs[0]
        self.assertEqual((who, args), ("rust", "--greeter --style look --look material"))
        self.assertEqual(json.loads(cfg)["appearance"]["materialSeed"], "#6750A4")

    def test_a_rust_one_that_fails_gives_way_to_the_qml_one(self):
        r, runs = self.start(self.LOOK, FAKE_UI_FAIL="1")
        self.assertEqual(r.returncode, 0)
        self.assertEqual([x[0] for x in runs], ["rust", "look"])
        self.assertIn("the QML one instead", r.stderr)
