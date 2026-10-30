"""The installer's decisions, against fake commands on PATH: the versions it
reads from compat.json, where the "start Hyprland on tty1" lines go for your
login shell, the keyboard layout it writes (tools/keyboard-lua.sh) and the
packages Software Update would install (tools/irice-packages.sh). Nothing
real is installed or asked; install.sh's functions are run on their own."""
import os
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


class Fakes(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.d = Path(self.tmp.name)
        self.bin = self.d / "bin"
        self.bin.mkdir()
        self.home = self.d / "home"
        self.home.mkdir()
        self.env = dict(os.environ, PATH=f"{self.bin}:/usr/bin:/bin", HOME=str(self.home), XDG_CONFIG_HOME=str(self.home / ".config"))
        self.env.pop("ZDOTDIR", None)

    def tearDown(self):
        self.tmp.cleanup()

    def fake(self, name, script):
        f = self.bin / name
        f.write_text("#!/bin/sh\n" + textwrap.dedent(script))
        f.chmod(0o755)


class InstallerFunctions(Fakes):
    def bash(self, code):
        """install.sh's helper functions (not its steps), then code."""
        functions = subprocess.run(
            ["sed", "-n", "/^compat() {/,/^}/p; /^version_ge() /p; /^hypr_tested() {/,/^}/p; /^login_profile() {/,/^}/p",
             str(ROOT / "install.sh")], capture_output=True, text=True, check=True).stdout
        script = f'ROOT={ROOT}; CONFIG="$XDG_CONFIG_HOME"\n{functions}\n' + \
            'NEED_HYPRLAND=$(compat hyprland min) HYPR_MAX=$(compat hyprland max)\n' + \
            'QS_BUNDLED=$(compat quickshell bundled) NEED_QUICKSHELL=$(compat quickshell min)\n' + code
        return subprocess.run(["bash", "-c", script], env=self.env, capture_output=True, text=True, timeout=10).stdout.strip()

    def test_versions_come_from_compat_json(self):
        import json
        c = json.loads((ROOT / "compat.json").read_text())
        self.assertEqual(self.bash('echo "$NEED_HYPRLAND $HYPR_MAX $NEED_QUICKSHELL $QS_BUNDLED"'),
                         f'{c["hyprland"]["min"]} {c["hyprland"]["max"]} {c["quickshell"]["min"]} {c["quickshell"]["bundled"]}')

    def test_newer_hyprland_than_tested(self):
        out = self.bash('HYPR_MAX=0.56.x; for v in 0.56.0 0.56.2 0.57.0 0.60.1 1.0.0; do hypr_tested $v && echo "$v ok" || echo "$v newer"; done; '
                        'HYPR_MAX=0.57.1; hypr_tested 0.57.1 && echo exact-ok; hypr_tested 0.57.2 || echo exact-newer')
        self.assertEqual(out.split("\n"), ["0.56.0 ok", "0.56.2 ok", "0.57.0 newer", "0.60.1 newer", "1.0.0 newer", "exact-ok", "exact-newer"])

    def test_tty_autostart_goes_where_your_login_shell_looks(self):
        def profile(shell, *existing):
            for f in existing:
                (self.home / f).write_text("")
            self.fake("getent", f'echo "me:x:1000:1000::/home/me:{shell}"\n')
            out = self.bash("login_profile")
            for f in existing:
                (self.home / f).unlink()
            return out.replace(str(self.home), "~")
        self.assertEqual(profile("/bin/bash"), "~/.bash_profile")
        self.assertEqual(profile("/bin/bash", ".profile"), "~/.profile")  # (bash reads that one when there's no .bash_profile)
        self.assertEqual(profile("/usr/bin/bash", ".bash_profile", ".profile"), "~/.bash_profile")
        self.assertEqual(profile("/usr/bin/zsh"), "~/.zprofile")
        self.assertEqual(profile("/usr/bin/fish"), "~/.config/fish/conf.d/irice-start-hyprland.fish")
        self.assertEqual(profile("/bin/dash"), "~/.profile")


class KeyboardLayout(Fakes):
    def write(self, status):
        self.fake("localectl", f"cat <<'EOF'\n{status}\nEOF\n")
        out = self.d / "irice/keyboard.lua"
        p = subprocess.run([str(ROOT / "tools/keyboard-lua.sh"), str(out)], env=self.env, capture_output=True, text=True, timeout=10)
        self.assertEqual(p.returncode, 0, p.stderr)
        return out.read_text() if out.exists() else None, p.stdout

    def test_your_layout_variant_model_and_options(self):
        lua, said = self.write("   System Locale: LANG=de_DE.UTF-8\n       VC Keymap: de-latin1\n      X11 Layout: de,us\n"
                               "       X11 Model: pc105\n     X11 Variant: nodeadkeys,\n     X11 Options: grp:alt_shift_toggle,caps:escape")
        self.assertIn('hl.config({ input = { kb_layout = "de,us", kb_variant = "nodeadkeys,", kb_model = "pc105", '
                      'kb_options = "grp:alt_shift_toggle,caps:escape" } })', lua)
        self.assertIn("de,us", said)
        lua, _ = self.write("      X11 Layout: fr")
        self.assertIn('{ kb_layout = "fr" }', lua)

    def test_no_layout_or_an_odd_one_leaves_the_default(self):
        self.write("      X11 Layout: fr")
        self.assertIsNone(self.write("   System Locale: LANG=C.UTF-8\n       VC Keymap: n/a\n      X11 Layout: n/a")[0])
        self.assertIsNone(self.write('      X11 Layout: us" } }) os.execute("rm -rf ~')[0])


class PackagesForSoftwareUpdate(Fakes):
    def status(self, installed=""):
        # pacman: nothing iRice lists is installed except what's named; every
        # package is in the repositories
        self.fake("pacman", f'''
            case "$1" in
                -T) shift; [ "$1" = -- ] && shift; for p; do case " {installed} " in *" $p "*) ;; *) echo "$p" ;; esac; done ;;
                -Si|-Sg) exit 0 ;;
                -Qq) shift; [ "$1" = -- ] && shift; case " {installed} " in *" $1 "*) exit 0 ;; esac; exit 1 ;;
            esac
        ''')
        self.fake("systemctl", "exit 3\n")
        p = subprocess.run([str(ROOT / "tools/irice-packages.sh"), "status"], env=self.env, capture_output=True, text=True, timeout=30)
        import json
        return json.loads(p.stdout)["missing"]

    def test_no_power_profiles_daemon_next_to_tlp(self):
        self.assertIn("power-profiles-daemon", self.status())
        missing = self.status(installed="tlp")
        self.assertNotIn("power-profiles-daemon", missing)
        self.assertIn("upower", missing)  # (the rest still comes)
        self.assertNotIn("power-profiles-daemon", self.status(installed="auto-cpufreq"))


if __name__ == "__main__":
    unittest.main()
