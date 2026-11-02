"""The Material look (material/): its sound set, default wallpapers, and the
root of links the shell runs it from (shell/assets/scripts/material-root.py)."""
import importlib.util
import os
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MATERIAL = ROOT / "material"
SHELL = ROOT / "shell"


def load(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


class Sounds(unittest.TestCase):
    def test_material_set_has_every_sound(self):
        want = sorted(p.name for p in (SHELL / "assets/sounds/marimba").glob("*.ogg"))
        have = sorted(p.name for p in (MATERIAL / "assets/sounds/material").glob("*.ogg"))
        self.assertEqual(have, want)

    def test_sounds_follow_their_own_setting(self):
        qml = (MATERIAL / "services/Sounds.qml").read_text()
        self.assertIn("materialSounds", qml)
        # (never a value Glass doesn't know written to the shared settings)
        self.assertNotIn('"auto"', qml)


class Wallpapers(unittest.TestCase):
    def test_bundled_pair(self):
        try:
            from PIL import Image
        except ImportError:
            self.skipTest("Pillow not installed")
        for mode in ("light", "dark"):
            path = MATERIAL / f"assets/wallpapers/material-{mode}.jpg"
            with Image.open(path) as im:
                self.assertEqual(im.size, (3840, 2160))
            self.assertLess(path.stat().st_size, 2.5e6)


class Root(unittest.TestCase):
    def setUp(self):
        self.mr = load(SHELL / "assets/scripts/material-root.py", "material_root")

    def test_material_files_win_and_the_rest_is_glass(self):
        with tempfile.TemporaryDirectory() as t:
            root = Path(self.mr.build(str(SHELL), str(MATERIAL), os.path.join(t, "material-shell")))
            # its own Theme, shell.qml and control
            for rel in ("common/Theme.qml", "shell.qml", "common/Switch.qml", "common/MaterialShape.qml"):
                self.assertEqual(os.path.realpath(root / rel), str((MATERIAL / rel).resolve()), rel)
            # shared: the services and settings data, unchanged
            for rel in ("services/Audio.qml", "common/Config.qml", "common/Panels.qml", "assets/scripts/launch-shell.sh"):
                self.assertEqual(os.path.realpath(root / rel), str((SHELL / rel).resolve()), rel)
            # every file of both is there, nothing else
            want = {str(p.relative_to(SHELL)) for p in SHELL.rglob("*") if p.is_file() and "build" not in p.relative_to(SHELL).parts and "__pycache__" not in p.parts and p.suffix != ".pyc"}
            want |= {str(p.relative_to(MATERIAL)) for p in MATERIAL.rglob("*") if p.is_file() and "__pycache__" not in p.parts and p.suffix != ".pyc"}
            have = {str(p.relative_to(root)) for p in root.rglob("*") if p.is_symlink()}
            self.assertEqual(have, want)

    def test_rebuilt_whole(self):
        with tempfile.TemporaryDirectory() as t:
            out = os.path.join(t, "material-shell")
            self.mr.build(str(SHELL), str(MATERIAL), out)
            stray = Path(out) / "common/Stray.qml"
            stray.symlink_to("/nonexistent")
            self.mr.build(str(SHELL), str(MATERIAL), out)
            self.assertFalse(stray.is_symlink())
            self.assertFalse(Path(out + ".new").exists() or Path(out + ".old").exists())



FAKE_QS = r"""#!/bin/sh
# A fake qs: "ipc show" lists targets only for the root named in $RUNNING;
# anything else is recorded (one argument per line) and exits
if [ "$3" = ipc ] && [ "$4" = show ]; then
    if [ "$2" = "$RUNNING" ]; then echo "target lock"; echo "target panels"; else echo "No running instances"; fi
    exit 0
fi
printf '%s\n' "$@" > "$QS_CALL"
"""


class Launch(unittest.TestCase):
    """launch-shell.sh starts the look's root; shell-ipc.sh finds the running one."""

    def setUp(self):
        import json
        self.tmp = tempfile.TemporaryDirectory()
        t = Path(self.tmp.name)
        (t / "bin").mkdir()
        (t / "bin/qs").write_text(FAKE_QS)
        (t / "bin/qs").chmod(0o755)
        (t / "config/irice").mkdir(parents=True)
        self.config = t / "config/irice/config.json"
        self.root = t / "cache/irice/material-shell"
        self.env = dict(os.environ, HOME=str(t), XDG_CONFIG_HOME=str(t / "config"), XDG_CACHE_HOME=str(t / "cache"),
                        XDG_DATA_HOME=str(t / "data"), XDG_RUNTIME_DIR=str(t), QS_CALL=str(t / "call"),
                        PATH=f"{t / 'bin'}:/usr/bin:/bin", RUNNING=str(self.root), QS_ICON_THEME="hicolor")
        self.env.pop("HYPRLAND_INSTANCE_SIGNATURE", None)
        self.json = json

    def tearDown(self):
        self.tmp.cleanup()

    def launch(self, look, *args):
        import subprocess
        self.config.write_text(self.json.dumps({"appearance": {"look": look}}))
        subprocess.run(["bash", str(SHELL / "assets/scripts/launch-shell.sh"), *args], env=self.env, timeout=30,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        return Path(self.env["QS_CALL"]).read_text().split("\n")[:2]

    def test_glass_keeps_its_arguments(self):
        self.assertEqual(self.launch("glass", "-c", "irice"), ["-c", "irice"])
        self.assertEqual(self.launch("glass", "-p", str(SHELL)), ["-p", str(SHELL)])
        # (a restart from the Material root goes back to this folder)
        self.assertEqual(self.launch("glass", "-p", str(self.root)), ["-p", str(SHELL.resolve())])

    def test_material_starts_its_root(self):
        self.assertEqual(self.launch("material", "-c", "irice"), ["-p", str(self.root)])
        self.assertEqual(self.launch("material", "-p", str(SHELL)), ["-p", str(self.root)])
        self.assertTrue((self.root / "common/Theme.qml").is_symlink())

    def test_ipc_reaches_the_running_root(self):
        import subprocess
        r = subprocess.run(["bash", str(SHELL / "assets/scripts/shell-ipc.sh"), "call", "lock", "lock"], env=dict(self.env, QS="qs"),
                           timeout=20, capture_output=True, text=True)
        self.assertNotEqual(r.returncode, 0)  # (no root built yet: nothing runs)
        self.launch("material", "-c", "irice")  # builds the root
        r = subprocess.run(["bash", str(SHELL / "assets/scripts/shell-ipc.sh"), "call", "lock", "lock"], env=dict(self.env, QS="qs"),
                           timeout=20, capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(Path(self.env["QS_CALL"]).read_text().split("\n")[:5], ["-p", str(self.root), "ipc", "call", "lock"])



class IdleListener(unittest.TestCase):
    """start-idle.sh replaces a listener from before shell-ipc.sh (its lock
    command named one shell folder, so it couldn't lock the Material look)."""

    def test_old_listener_is_replaced(self):
        import subprocess
        import time
        with tempfile.TemporaryDirectory(dir="/tmp") as t:
            t = Path(t)
            (t / "bin").mkdir()
            (t / "bin/hypridle").write_text("#!/bin/sh\nexec sleep 30\n")
            (t / "bin/hypridle").chmod(0o755)
            d = t / "irice-idle-sig"
            d.mkdir()
            old = subprocess.Popen(["bash", "-c", 'exec 9>"$1/lock"; flock 9; echo "qs -p /old/shell ipc call lock lock" > "$1/lock-command"; exec sleep 30', "_", str(d)])
            try:
                time.sleep(0.4)
                env = {"PATH": f"{t / 'bin'}:/usr/bin:/bin", "HOME": str(t), "XDG_RUNTIME_DIR": str(t),
                       "HYPRLAND_INSTANCE_SIGNATURE": "sig", "XDG_CONFIG_HOME": str(t / "cfg")}
                subprocess.run(["setsid", "-f", "bash", str(SHELL / "assets/scripts/start-idle.sh"), "/usr/bin/qs"], env=env,
                               stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10)
                for _ in range(40):
                    if old.poll() is not None and "shell-ipc.sh" in (d / "lock-command").read_text() and (d / "hypridle.pid").exists():
                        break
                    time.sleep(0.1)
                self.assertIsNotNone(old.poll(), "the old listener still runs")
                self.assertIn("shell-ipc.sh call lock lock", (d / "lock-command").read_text())
                self.assertTrue((d / "hypridle.pid").exists(), "no new hypridle")
            finally:
                old.kill()
                r = subprocess.run(["fuser", str(d / "lock")], capture_output=True, text=True)
                for pid in r.stdout.split():
                    subprocess.run(["kill", pid], capture_output=True)


if __name__ == "__main__":
    unittest.main()
