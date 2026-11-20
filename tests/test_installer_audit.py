"""The installer, uninstaller and session scripts after the audit (20 Nov):
uninstall.sh undoes every system change and never stops halfway, install.sh
--yes sets up only the essentials, the empty wallpaper step is gone, the
shell's start (launch-shell.sh) picks Rust, the previous build or the QML
fallback as it should (also from a copied install), idle undim only puts
back a dim of its own, `tessera logs` follows the Rust shell's log, and
setup.sh keeps your edits. Everything runs against fake commands in a
throwaway home and system root: nothing real is changed, asked or started."""
import json
import os
import re
import shutil
import subprocess
import tempfile
import textwrap
import time
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPTS = ROOT / "shell/assets/scripts"


class Temp(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.d = Path(self.tmp.name)
        self.home = self.d / "home"
        self.home.mkdir()
        self.bin = self.d / "bin"
        self.bin.mkdir()
        self.sys = self.d / "sys"
        self.log = self.d / "calls.log"
        self.env = {"PATH": f"{self.bin}:/usr/bin:/bin", "HOME": str(self.home), "LANG": "C.UTF-8",
                    "XDG_CONFIG_HOME": str(self.home / ".config"), "XDG_DATA_HOME": str(self.home / ".local/share"),
                    "XDG_STATE_HOME": str(self.home / ".local/state"), "XDG_CACHE_HOME": str(self.home / ".cache"),
                    "XDG_RUNTIME_DIR": str(self.d / "run"), "TESSERA_SYSTEM_ROOT": str(self.sys), "LOG": str(self.log),
                    "GIT_CONFIG_GLOBAL": "/dev/null", "GIT_CONFIG_NOSYSTEM": "1"}
        (self.d / "run").mkdir()

    def tearDown(self):
        self.tmp.cleanup()

    def write(self, path, text="", mode=None):
        p = Path(path)
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text)
        if mode:
            p.chmod(mode)
        return p

    def fake(self, name, script):
        self.write(self.bin / name, "#!/bin/sh\n" + textwrap.dedent(script), 0o755)

    def calls(self):
        return self.log.read_text() if self.log.exists() else ""


# ── uninstall.sh ─────────────────────────────────────────────────────────
class Uninstall(Temp):
    def setUp(self):
        super().setUp()
        self.fake("sudo", 'exec "$@"\n')
        # systemctl: greetd on; the check timer never installed (an install
        # from before 19 Nov) fails to disable; a login manager can be made
        # to fail to come back (SDDM_FAILS=1)
        self.fake("systemctl", '''\
            echo "systemctl $*" >> "$LOG"
            case "$*" in
                "is-enabled --quiet greetd") exit 0 ;;
                "disable tessera-login-check.timer") exit 1 ;;
                "enable sddm") [ "${SDDM_FAILS:-0}" = 1 ] && exit 1; exit 0 ;;
            esac
            exit 0
            ''')
        self.c, self.data, self.cache = self.home / ".config", self.home / ".local/share", self.home / ".cache"

    def run_uninstall(self, *args, stdin=None, **env):
        return subprocess.run(["bash", str(ROOT / "uninstall.sh"), *args], env=dict(self.env, **env), input=stdin,
                              capture_output=True, text=True, timeout=60)

    def login_screen(self, config='command = "/usr/share/tessera/greeter-session.sh"\n'):
        self.write(self.sys / "usr/share/tessera/shell/greeter.qml")
        self.write(self.sys / "etc/greetd/config.toml", config)

    def test_a_login_screen_from_before_the_check_timer_never_stops_it(self):
        self.login_screen()
        self.write(self.sys / "etc/greetd/config.toml.pre-tessera", "# before\n")
        self.write(self.sys / "usr/lib/tessera/x")
        self.write(self.sys / "etc/polkit-1/rules.d/49-tessera.rules")
        self.write(self.home / ".local/bin/tessera", "", 0o755)
        self.write(self.data / "tessera/previous-login-manager", "sddm\n")
        p = self.run_uninstall("--yes")
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        for path in (self.sys / "usr/share/tessera", self.sys / "usr/lib/tessera", self.sys / "etc/polkit-1/rules.d/49-tessera.rules",
                     self.home / ".local/bin/tessera"):
            self.assertFalse(path.exists(), path)
        self.assertEqual((self.sys / "etc/greetd/config.toml").read_text(), "# before\n")
        self.assertIn("systemctl enable sddm", self.calls())
        self.assertIn("Done.", p.stdout)
        # (a timer that is there and won't disable doesn't stop it either)
        self.login_screen()
        self.write(self.sys / "etc/systemd/system/tessera-login-check.timer")
        p = self.run_uninstall("--yes")
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        self.assertFalse((self.sys / "etc/systemd/system/tessera-login-check.timer").exists())

    def test_greetd_with_another_greeter_before_stays_on(self):
        self.login_screen()
        self.write(self.sys / "etc/greetd/config.toml.pre-tessera", 'command = "tuigreet"\n')
        self.write(self.sys / "etc/tessera/login-switch.json", json.dumps({"previous": "greetd", "state": "active"}))
        p = self.run_uninstall("--yes")
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        self.assertNotIn("systemctl disable greetd", self.calls())
        self.assertEqual((self.sys / "etc/greetd/config.toml").read_text(), 'command = "tuigreet"\n')
        self.assertFalse((self.sys / "etc/tessera").exists())

    def test_a_login_manager_that_cant_come_back_leaves_greetd_on(self):
        self.login_screen()
        self.write(self.sys / "etc/greetd/config.toml.pre-tessera", "# before\n")
        self.write(self.data / "tessera/previous-login-manager", "sddm\n")
        p = self.run_uninstall("--yes", SDDM_FAILS="1")
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        calls = self.calls()
        self.assertIn("systemctl disable greetd", calls)
        self.assertGreater(calls.rindex("systemctl enable greetd"), calls.index("systemctl enable sddm"))
        self.assertIn("couldn't be turned back on", p.stdout)
        self.assertFalse((self.sys / "usr/share/tessera").exists())

    def test_every_system_change_goes(self):
        s = self.sys
        face = self.write(s / "etc/systemd/system/polkit-agent-helper@.service.d/50-tessera-face.conf", "[Service]\nBindReadOnlyPaths=x\n")
        bt = self.write(s / "etc/systemd/system/bluetooth.service.d/tessera-no-rap.conf", "[Service]\n")
        fp = self.write(s / "etc/systemd/system/fprintd.service.d/tessera-timeout.conf", "[Service]\n")
        mods = [self.write(s / "etc/modprobe.d/tessera-phonecam.conf"), self.write(s / "etc/modules-load.d/tessera-phonecam.conf")]
        self.write(s / "var/lib/tessera/face/polkit-1", "auth sufficient pam_howdy.so\n")
        self.write(s / "etc/pam.d/sudo", "#%PAM-1.0\nauth       sufficient   pam_fprintd.so max-tries=5 timeout=10   # Tessera fingerprint\nauth include system-auth\n")
        self.write(s / "etc/pam.d/polkit-1", "auth       sufficient   pam_fprintd.so   # Tessera fingerprint\nauth include system-auth\n")
        self.write(s / "etc/bluetooth/main.conf", "[General]\nExperimental = true\n")
        self.write(s / "etc/bluetooth/main.conf.pre-tessera", "[General]\n")
        self.write(s / "etc/systemd/system/polkit-agent-helper@.service.d/other.conf", "[Service]\n")
        p = self.run_uninstall("--yes")
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        for path in (face, bt, fp, *mods, s / "var/lib/tessera", bt.parent, fp.parent):
            self.assertFalse(path.exists(), path)
        self.assertTrue((s / "etc/systemd/system/polkit-agent-helper@.service.d/other.conf").exists())  # (not ours: stays)
        self.assertEqual((s / "etc/pam.d/sudo").read_text(), "#%PAM-1.0\nauth include system-auth\n")
        self.assertEqual((s / "etc/pam.d/polkit-1").read_text(), "auth include system-auth\n")
        self.assertEqual((s / "etc/bluetooth/main.conf").read_text(), "[General]\n")
        self.assertIn("systemctl daemon-reload", self.calls())

    def test_the_battery_guard_and_the_menus_go(self):
        unit = self.write(self.home / ".config/systemd/user/tessera-battery-guard.service", "[Service]\n")
        dolphin = self.write(self.data / "kio/servicemenus/tessera-share.desktop", "[Desktop Entry]\n")
        nemo = self.write(self.data / "nemo/actions/tessera-share.nemo_action", "[Nemo Action]\n")
        self.write(self.c / "quickshell/tessera/shell.qml")
        p = self.run_uninstall("--yes")
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        self.assertFalse(unit.exists())
        self.assertIn("systemctl --user disable --now tessera-battery-guard.service", self.calls())
        self.assertIn("systemctl --user daemon-reload", self.calls())
        self.assertFalse(dolphin.exists())
        self.assertFalse(nemo.exists())
        self.assertFalse((self.c / "quickshell/tessera").exists())

    def test_without_a_backup_tesseras_hypr_config_moves_aside(self):
        self.write(self.c / "hypr/hyprland.lua", 'dofile("tessera/execs.lua")\n')
        self.write(self.c / "hypr/tessera/execs.lua", "-- launch-shell.sh every 5 s\n")
        p = self.run_uninstall("--yes")
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        self.assertFalse((self.c / "hypr").exists())
        self.assertTrue((self.c / "hypr.tessera-removed/tessera/execs.lua").exists())
        # (and a config that isn't Tessera's is left alone)
        self.write(self.c / "hypr/hyprland.lua", "-- mine\n")
        p = self.run_uninstall("--yes")
        self.assertEqual((self.c / "hypr/hyprland.lua").read_text(), "-- mine\n")

    def big_parts(self):
        return [self.write(self.data / "tessera/repo/install.sh"), self.write(self.data / "tessera/bin/tessera-ui", "", 0o755),
                self.write(self.data / "tessera/quickshell/bin/qs"), self.write(self.data / "tessera/build/quickshell/x"),
                self.write(self.data / "tessera/backups/installed-1/shell.qml"), self.write(self.cache / "tessera/cargo/generic/x")]

    def test_the_big_parts_after_one_question(self):
        rc = self.write(self.home / ".bashrc", 'alias x=y\n\nexport PATH="$HOME/.local/bin:$PATH" # tessera: local bin\n')
        parts = self.big_parts()
        settings = self.write(self.c / "tessera/config.json", "{}")
        # no: kept
        p = self.run_uninstall(stdin="y\nn\n")
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        for f in parts:
            self.assertTrue(f.exists(), f)
        self.assertIn("Kept", p.stdout)
        self.assertEqual(rc.read_text(), "alias x=y\n\n")
        # yes: gone (and --yes says yes to it too)
        p = self.run_uninstall(stdin="y\ny\n")
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        for f in parts:
            self.assertFalse(f.exists(), f)
        parts = self.big_parts()
        p = self.run_uninstall("--yes")
        for f in parts:
            self.assertFalse(f.exists(), f)
        self.assertTrue(settings.exists())  # (your settings stay)

    def test_a_dry_run_changes_nothing(self):
        self.login_screen()
        parts = self.big_parts()
        unit = self.write(self.home / ".config/systemd/user/tessera-battery-guard.service", "[Service]\n")
        self.write(self.sys / "etc/systemd/system/bluetooth.service.d/tessera-no-rap.conf")
        p = self.run_uninstall("--dry-run")
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        for f in (*parts, unit, self.sys / "usr/share/tessera", self.sys / "etc/systemd/system/bluetooth.service.d/tessera-no-rap.conf"):
            self.assertTrue(f.exists(), f)
        self.assertIn("would run", p.stdout)


# ── install.sh ───────────────────────────────────────────────────────────
FEATURE_LIST = [  # id, set up, group, title, detail, packages, aur
    ("fingerprint", "0", "Hardware", "Fingerprint", "x", "fprintd", "libfprint-tod"),
    ("face", "0", "Hardware", "Face unlock", "x", "", "howdy-next"),
    ("power", "1", "Hardware", "Energy modes", "x", "", ""),
    ("quickshare", "0", "Sharing", "Quick Share", "x", "avahi", ""),
    ("login", "0", "Look and login", "Tessera's login screen", "x", "greetd", ""),
    ("look", "0", "Look and login", "Tessera's desktop look", "x", "", "darkly-bin"),
    ("glass", "0", "Look and login", "Glass plugin", "x", "", ""),
]


class InstallerFeatures(Temp):
    """decide_features and record_features, with features.py's list faked"""
    MULTI = ("decide_features", "features_follow", "essential_features", "record_features", "missing_of")
    ONE = ("unattended", "essentials_only", "feat_line", "feature_on")

    def setUp(self):
        super().setUp()
        self.root = self.d / "root"
        rows = "\n".join("\t".join(r) for r in FEATURE_LIST)
        self.write(self.root / "shell/assets/scripts/features.py", textwrap.dedent(f'''\
            import os, sys
            if sys.argv[1] == "list":
                print({rows!r})
            elif sys.argv[1] == "record":
                open(os.environ["LOG"], "a").write("record " + " ".join(sys.argv[2:]) + "\\n")
            '''))

    def decide(self, yes=1, features="-", greeter="ask", other_dm=""):
        src = (ROOT / "install.sh").read_text()
        pattern = "; ".join([f"/^{n}() {{/,/^}}/p" for n in self.MULTI] + [f"/^{n}() /p" for n in self.ONE] +
                            ["/^ESSENTIAL_FEATURES=/p", "/^FEAT_UNASKED=/p"])
        functions = subprocess.run(["sed", "-n", pattern], input=src, capture_output=True, text=True, check=True).stdout
        script = (f'ROOT={self.root}; YES={yes}; DRY=0; PLAN_ONLY=0; FEATURES="{features}"; GREETER={greeter}; '
                  f'OTHER_DM="{other_dm}"; EXTRAS=1; EXTRAS_LIST=-; STYLE_AUR=(); EMOJI_AUR=(); WANT_STYLE=0; WANT_EMOJI=0\n'
                  'say() { :; }; note() { :; }; ask() { return 0; }; pkg_installed() { return 0; }\n'
                  f'FEAT_PLAN="" FEAT_LATER=0\n{functions}\n'
                  'decide_features; echo "FEATURES=$(xargs <<<"$FEATURES") GREETER=$GREETER UNASKED=$FEAT_UNASKED"; record_features\n')
        p = subprocess.run(["bash", "-c", script], env=self.env, capture_output=True, text=True, timeout=30)
        self.assertEqual(p.returncode, 0, p.stderr)
        return p.stdout.strip().splitlines()[-1]

    def test_yes_sets_up_only_the_essentials(self):
        self.assertEqual(self.decide(), "FEATURES=glass GREETER=no UNASKED=1")
        # (nothing recorded as declined, the offer untouched: nobody was asked)
        self.assertEqual(self.calls().strip(), "record --chosen glass")
        self.assertNotIn("--declined", self.calls())

    def test_yes_with_features_named_sets_those_up(self):
        self.assertEqual(self.decide(features="face quickshare"), "FEATURES=face quickshare GREETER=no UNASKED=0")
        self.assertIn("--declined", self.calls())  # (asked by name: the rest were left out on purpose)

    def test_yes_with_the_greeter_asked_for_by_name(self):
        self.assertEqual(self.decide(greeter="yes"), "FEATURES=glass login GREETER=yes UNASKED=1")

    def test_interactive_keeps_its_questions(self):
        # (the interactive installer, answering yes to all: everything not set up)
        out = self.decide(yes=0)
        self.assertEqual(out, "FEATURES=fingerprint face quickshare login look glass GREETER=yes UNASKED=0")


class InstallerSteps(unittest.TestCase):
    def test_no_wallpaper_step_and_the_count_matches(self):
        sh = (ROOT / "install.sh").read_text()
        steps = re.findall(r'^step "([^"]+)"', sh, re.M)
        self.assertNotIn("Wallpapers (optional)", sh)
        self.assertNotIn("WANT_WALLS", sh)
        self.assertEqual(int(re.search(r"^STEP=0 TOTAL=(\d+)", sh, re.M).group(1)), len(steps))
        start = sh[sh.index("screen_start \"Checking"):]
        start = start[:start.index("\nadopt_old_names")]
        self.assertEqual(re.findall(r'"([^"]+)" \d+', start), steps)
        self.assertNotIn("macOS wallpapers", (ROOT / "README.md").read_text().split("The installer then:")[-1][:3000])

    def test_only_tesseras_plugin_folder_is_copied(self):
        sh = (ROOT / "install.sh").read_text()
        self.assertIn('cp -r "$ROOT/hypr/plugin/tessera-glass" "$DATA/tessera/src/hypr/plugin/tessera-glass"', sh)
        self.assertNotIn('cp -r "$ROOT/hypr/plugin" ', sh)

    def test_setup_tty_uses_the_root_helpers_pairing_fix(self):
        tty = (ROOT / "tools/setup-tty.sh").read_text()
        self.assertIn('tessera-root.py" fix btpairing', tty)
        self.assertNotIn("ExecStart=/usr/lib/bluetooth/bluetoothd", tty)


class Doctor(Temp):
    def test_the_rust_shells_fallback_and_crash_are_said(self):
        src = (ROOT / "install.sh").read_text()
        functions = subprocess.run(["sed", "-n", "/^meh() /p; /^shell_health() {/,/^}/p"], input=src, capture_output=True, text=True,
                                   check=True).stdout
        state = self.home / ".local/state/tessera"
        script = f'YEL=""; R=""; DIM=""; WARNS=0\n{functions}\nshell_health; echo "warns=$WARNS"\n'
        p = subprocess.run(["bash", "-c", script], env=self.env, capture_output=True, text=True, timeout=10)
        self.assertEqual(p.stdout.strip(), "warns=0")
        self.write(state / "fallback", "The Rust shell crashed twice\n1\nsig\n")
        self.write(state / "tessera-ui.log", "start\nthread 'main' panicked at src/x.rs:1: boom\n")
        p = subprocess.run(["bash", "-c", script], env=self.env, capture_output=True, text=True, timeout=10)
        self.assertIn("Running the basic QML fallback: The Rust shell crashed twice", p.stdout)
        self.assertIn("panicked at src/x.rs:1: boom", p.stdout)
        self.assertIn("warns=2", p.stdout)


# ── tessera logs ─────────────────────────────────────────────────────────
class Logs(Temp):
    def logs(self, rust, qml, log=True):
        self.fake("pgrep", f'case "$*" in *tessera-ui*) exit {0 if rust else 1} ;; esac; exit {0 if qml else 1}\n')
        self.fake("tail", 'echo "tail $*"\n')
        self.fake("qs", 'echo "qs $*"\n')
        path = self.home / ".local/state/tessera/tessera-ui.log"
        if log:
            self.write(path, "x\n")
        elif path.exists():
            path.unlink()
        p = subprocess.run(["bash", str(ROOT / "tools/tessera"), "logs", "20"], env=self.env, capture_output=True, text=True, timeout=10)
        return p.stdout.strip().replace(str(path), "LOG")

    def test_the_rust_shells_log_when_its_the_shell(self):
        self.assertEqual(self.logs(rust=True, qml=False), "tail -n 20 -F LOG")
        self.assertEqual(self.logs(rust=False, qml=False), "tail -n 20 -F LOG")  # (no shell running: the last one's log)
        self.assertEqual(self.logs(rust=False, qml=True), "qs log -c tessera -t 20 -f")  # (the QML fallback runs)
        self.assertEqual(self.logs(rust=False, qml=False, log=False), "qs log -c tessera -t 20 -f")


# ── idle-action.sh: dim and undim ────────────────────────────────────────
class IdleDim(Temp):
    def setUp(self):
        super().setUp()
        self.state = self.d / "brightness"
        # brightnessctl with a level, a max of 1000 and a saved level (-s / -r)
        self.fake("brightnessctl", f'''\
            S={self.state}
            cur=$(cat "$S/cur"); saved=$(cat "$S/saved" 2>/dev/null || echo "")
            case "$*" in
                get) echo "$cur" ;;
                max) echo 1000 ;;
                "-q -s set "*) echo "$cur" > "$S/saved"; echo "${{4}}" > "$S/cur" ;;
                "-q -r") [ -n "$saved" ] && echo "$saved" > "$S/cur" ;;
            esac
            ''')
        self.state.mkdir()
        self.idle = self.d / "idle"
        self.idle.mkdir()

    def level(self, value=None):
        if value is not None:
            (self.state / "cur").write_text(f"{value}\n")
        return int((self.state / "cur").read_text())

    def step(self, action):
        subprocess.run(["bash", str(SCRIPTS / "idle-action.sh"), "any", action], env=dict(self.env, TESSERA_IDLE_DIR=str(self.idle)),
                       check=True, timeout=10)

    def test_undim_puts_back_only_its_own_dim(self):
        self.level(800)
        self.step("dim")
        self.assertEqual(self.level(), 100)
        self.step("undim")
        self.assertEqual(self.level(), 800)
        # already at a tenth or less: dim leaves it, and undim doesn't bring
        # back the afternoon's saved level
        self.level(50)
        self.step("dim")
        self.step("undim")
        self.assertEqual(self.level(), 50)
        # changed by you while dimmed: yours stays
        self.level(800)
        self.step("dim")
        self.level(300)
        self.step("undim")
        self.assertEqual(self.level(), 300)
        self.assertFalse((self.idle / "dimmed").exists())


# ── battery-guard-install.sh ─────────────────────────────────────────────
class BatteryGuardUnit(Temp):
    def test_quoted_path_and_a_start_limit(self):
        self.fake("systemctl", 'echo "systemctl $*" >> "$LOG"; exit 0\n')
        subprocess.run(["bash", str(SCRIPTS / "battery-guard-install.sh"), "enable"], env=self.env, capture_output=True, timeout=10)
        unit = (self.home / ".config/systemd/user/tessera-battery-guard.service").read_text()
        self.assertIn(f'ExecStart=/usr/bin/env python3 -I "{SCRIPTS}/battery-guard.py" --sleep-at 3', unit)
        self.assertIn("StartLimitBurst=5", unit)
        self.assertLess(unit.index("StartLimitIntervalSec"), unit.index("[Service]"))


# ── launch-shell.sh: Rust, the build before, or the QML fallback ─────────
class LaunchFallback(Temp):
    SIG = "session-one"

    def setUp(self):
        super().setUp()
        self.checkout = self.d / "checkout"
        for f in ("launch-shell.sh", "old-names.sh"):
            self.write(self.checkout / "shell/assets/scripts" / f, (SCRIPTS / f).read_text(), 0o755)
        self.mark = self.d / "mark"
        stub = 'import os, sys\nopen(os.environ["MARK"], "a").write("{} " + " ".join(sys.argv[1:]) + "\\n")\n'
        self.write(self.checkout / "tools/tessera-update.py", stub.replace("{}", "update"))
        self.write(self.checkout / "tools/native-install.py", stub.replace("{}", "native-install"))
        self.ui = self.write(self.d / "tessera-ui", '#!/bin/sh\necho "rust $*" >> "$MARK"\n', 0o755)
        self.fake("qs", 'echo "qml $*" >> "$MARK"\n')
        self.fake("setsid", "exit 0\n")
        self.fake("gsettings", "exit 1\n")
        self.state = self.home / ".local/state/tessera"
        self.state.mkdir(parents=True)
        self.env.update(MARK=str(self.mark), TESSERA_UI=str(self.ui), HYPRLAND_INSTANCE_SIGNATURE=self.SIG)
        del self.env["TESSERA_SYSTEM_ROOT"]

    def launch(self, script=None, **env):
        if self.mark.exists():
            self.mark.unlink()
        script = script or self.checkout / "shell/assets/scripts/launch-shell.sh"
        subprocess.run(["bash", str(script), "-c", "tessera"], env=dict(self.env, **env), capture_output=True, text=True, timeout=30)
        return self.mark.read_text().splitlines() if self.mark.exists() else []

    def starts(self, n=None, sig=SIG):
        f = self.state / "standalone-starts"
        if n is not None:
            f.write_text(f"{n}\n{sig}\n")
        return f.read_text().splitlines() if f.exists() else []

    def test_the_rust_shell_counted_per_login(self):
        self.assertEqual(self.launch(), ["rust --standalone"])
        self.assertEqual(self.starts(), ["1", self.SIG])
        # one crash: tried again
        self.assertEqual(self.launch(), ["rust --standalone"])
        self.assertEqual(self.starts(), ["2", self.SIG])
        # a second that didn't last: the QML fallback, with a note for this session
        self.assertEqual(self.launch(), ["qml -c tessera"])
        note = (self.state / "fallback").read_text().splitlines()
        self.assertTrue(note[0].startswith("The Rust shell crashed twice"))
        self.assertEqual(note[2], self.SIG)
        self.assertIn('"ev":"fallback"', (self.state / "health.jsonl").read_text())
        # a new login tries Rust again
        self.assertEqual(self.launch(HYPRLAND_INSTANCE_SIGNATURE="session-two"), ["rust --standalone"])
        self.assertFalse((self.state / "fallback").exists())

    def test_turned_off_or_forbidden_is_the_qml_shell(self):
        self.assertEqual(self.launch(TESSERA_STANDALONE="0"), ["qml -c tessera"])
        self.write(self.home / ".config/tessera/config.json", json.dumps({"rust": {"standalone": False}}))
        self.assertEqual(self.launch(), ["qml -c tessera"])
        self.assertEqual(self.launch(TESSERA_STANDALONE="1"), ["rust --standalone"])

    def test_a_fresh_update_that_crashes_goes_back_to_the_build_before(self):
        fresh = self.write(self.state / "fresh-update.json", "{}")
        self.starts(2)
        self.assertEqual(self.launch(), ["native-install rollback --json", "rust --standalone"])
        self.assertFalse(fresh.exists())
        self.assertEqual(self.starts(), ["1", self.SIG])
        # (over an hour old: not this update's fault, the QML fallback)
        fresh = self.write(self.state / "fresh-update.json", "{}")
        old = time.time() - 7200
        os.utime(fresh, (old, old))
        self.starts(2)
        self.assertEqual(self.launch(), ["qml -c tessera"])

    def test_a_copied_install_finds_its_checkout(self):
        copy = self.home / ".config/quickshell/tessera/assets/scripts"
        for f in ("launch-shell.sh", "old-names.sh"):
            self.write(copy / f, (SCRIPTS / f).read_text(), 0o755)
        self.write(self.home / ".local/share/tessera/source", f"{self.checkout}\n")
        self.write(self.state / "update-staged/staged.json", "{}")
        fresh = self.write(self.state / "fresh-update.json", "{}")
        self.starts(2)
        out = self.launch(copy / "launch-shell.sh")
        self.assertTrue(out[0].startswith("update finish --if-ready --config "), out)
        self.assertEqual(out[1:], ["native-install rollback --json", "rust --standalone"])
        self.assertFalse(fresh.exists())


# ── setup.sh run again ───────────────────────────────────────────────────
class SetupAgain(Temp):
    def git(self, repo, *args):
        return subprocess.run(["git", "-C", str(repo), *args], env=self.env, capture_output=True, text=True, check=True).stdout.strip()

    def test_your_edits_are_kept_aside(self):
        good, seed = self.d / "good.git", self.d / "seed"
        subprocess.run(["git", "init", "-q", "--bare", "-b", "main", str(good)], env=self.env, check=True)
        subprocess.run(["git", "init", "-q", "-b", "main", str(seed)], env=self.env, check=True)
        self.write(seed / "install.sh", "#!/bin/sh\necho installer ran\n", 0o755)
        self.write(seed / "README", "one\n")
        ident = ["-c", "user.name=Test", "-c", "user.email=test@example.invalid", "-c", "commit.gpgsign=false"]
        self.git(seed, "add", "-A")
        self.git(seed, *ident, "commit", "-qm", "one")
        self.git(seed, "push", "-q", str(good), "main")
        repo = self.home / ".local/share/tessera/repo"
        subprocess.run(["git", "clone", "-q", "--branch", "main", str(good), str(repo)], env=self.env, check=True)
        (repo / "README").write_text("my edit\n")
        self.fake("pacman", "exit 0\n")
        env = dict(self.env, TESSERA_MIRRORS=str(good))
        env.pop("TESSERA_SYSTEM_ROOT")
        p = subprocess.run(["bash", str(ROOT / "setup.sh")], env=env, capture_output=True, text=True, timeout=60, stdin=subprocess.DEVNULL)
        self.assertIn("kept aside", p.stdout, p.stdout + p.stderr)
        self.assertEqual((repo / "README").read_text(), "one\n")
        self.assertIn("your edits before it updated", self.git(repo, "stash", "list"))
        self.assertEqual(self.git(repo, "show", "stash@{0}:README"), "my edit")


if __name__ == "__main__":
    unittest.main()
