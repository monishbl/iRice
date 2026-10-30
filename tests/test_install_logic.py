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


class SetupFunctions(Fakes):
    """The keyring at login, the GPU file, the AUR helper and the old
    hyprland.conf's suggestions."""
    def fn(self, names, code, **env):
        pattern = "; ".join(f"/^{n}() {{/,/^}}/p" for n in names)
        functions = subprocess.run(["sed", "-n", pattern + "; /^KEYRING_MARK=/p", str(ROOT / "install.sh")],
                                   capture_output=True, text=True, check=True).stdout
        p = subprocess.run(["/bin/bash", "-c", functions + "\n" + code], env=dict(self.env, **env), capture_output=True, text=True, timeout=10)
        return p.returncode, p.stdout

    # Arch's /etc/pam.d/greetd (the greetd package)
    GREETD = "#%PAM-1.0\n\nauth       required     pam_securetty.so\nauth       requisite    pam_nologin.so\n" \
             "auth       include      system-local-login\naccount    include      system-local-login\n" \
             "session    include      system-local-login\n"

    def test_keyring_lines_after_the_last_auth_and_session(self):
        pam = self.d / "greetd"
        pam.write_text(self.GREETD)
        code, out = self.fn(["pam_with_keyring"], f"pam_with_keyring {pam}")
        self.assertEqual(code, 0)
        lines = out.splitlines()
        i = lines.index("auth       include      system-local-login")
        self.assertEqual(lines[i + 1:i + 3], ["# irice: unlock the keyring with the login password",
                                              "-auth      optional     pam_gnome_keyring.so"])
        self.assertEqual(lines[-2:], ["# irice: unlock the keyring with the login password",
                                      "-session   optional     pam_gnome_keyring.so auto_start"])
        # once only; never next to KWallet's; nothing for a missing file
        pam.write_text(out)
        self.assertEqual(self.fn(["pam_with_keyring"], f"pam_with_keyring {pam}")[0], 1)
        pam.write_text(self.GREETD + "-session   optional     pam_kwallet5.so auto_start\n")
        self.assertEqual(self.fn(["pam_with_keyring"], f"pam_with_keyring {pam}")[0], 1)
        self.assertEqual(self.fn(["pam_with_keyring"], f"pam_with_keyring {self.d}/none")[0], 1)
        # uninstall.sh's sed takes exactly those lines out again
        pam.write_text(out)
        subprocess.run(["sed", "-i", "/# irice: unlock the keyring/,+1d", str(pam)], check=True)
        self.assertEqual(pam.read_text(), self.GREETD)

    def gpus(self, *vendors, nvidia=None, lspci=""):
        sys_root = self.d / "sys-root"
        subprocess.run(["rm", "-rf", str(sys_root)])
        for i, v in enumerate(vendors):
            card = sys_root / f"sys/class/drm/card{i}/device"
            card.mkdir(parents=True)
            (card / "vendor").write_text(v + "\n")
        if nvidia:
            (sys_root / "sys/module/nvidia_drm").mkdir(parents=True)
            (sys_root / "sys/module/nvidia").mkdir(parents=True)
            (sys_root / "sys/module/nvidia/version").write_text(nvidia + "\n")
        self.fake("lspci", f"printf '%s\\n' '{lspci}'\n")
        env = {"IRICE_SYSTEM_ROOT": str(sys_root)}
        vendors_out = self.fn(["gpu_vendors"], "gpu_vendors", **env)[1]
        code, lua = self.fn(["gpu_lua"], f'gpu_lua "$(cat <<"EOF"\n{vendors_out}EOF\n)"', **env)
        return vendors_out.split(), code, lua

    def test_gpu_file_only_for_nvidia(self):
        self.assertEqual(self.gpus("0x8086")[:2], (["intel"], 1))
        self.assertEqual(self.gpus(lspci="01:00.0 VGA compatible controller [0300]: AMD [1002:73df]")[:2], (["amd"], 1))
        vendors, code, lua = self.gpus("0x10de", nvidia="570.144")
        self.assertEqual((vendors, code), (["nvidia"], 0))
        self.assertIn('\nhl.env("LIBVA_DRIVER_NAME", "nvidia")', lua)
        self.assertIn('\nhl.env("__GLX_VENDOR_LIBRARY_NAME", "nvidia")', lua)
        self.assertIn('\nhl.env("NVD_BACKEND", "direct")', lua)
        self.assertNotIn("no_hardware_cursors", lua)  # (555 and newer: Hyprland's default works)
        self.assertIn("no_hardware_cursors = true", self.gpus("0x10de", nvidia="550.120")[2])
        # hybrid graphics: nothing forced, the lines are suggestions
        vendors, code, lua = self.gpus("0x8086", "0x10de", nvidia="570.144")
        self.assertEqual((vendors, code), (["intel", "nvidia"], 0))
        self.assertIn('-- hl.env("LIBVA_DRIVER_NAME", "nvidia")', lua)
        self.assertNotIn('\nhl.env(', lua)
        luac = subprocess.run(["sh", "-c", "command -v luac"], capture_output=True, text=True).stdout.strip()
        if luac:
            f = self.d / "gpu.lua"
            f.write_text(self.gpus("0x10de", nvidia="550.1")[2])
            self.assertEqual(subprocess.run([luac, "-p", str(f)], capture_output=True).returncode, 0)

    def install_yay(self, *tools, answer="n"):
        # (only these tools: the computer's own yay or paru mustn't count)
        for t in ("basename", "mktemp", "rm", "dirname"):
            (self.bin / t).symlink_to(subprocess.run(["sh", "-c", f"command -v {t}"], capture_output=True, text=True).stdout.strip())
        for t in tools:
            self.fake(t, "")
        self.fake("git", "echo git \"$@\"\n")
        code = ('DEPS=1 DRY=1 YES=0 DIM="" R=""; note() { echo "note: $*"; }; say() { echo "$*"; }; '
                f'ask() {{ echo "asked: $1"; [ {answer} = y ]; }}; run() {{ echo "would run: $*"; }}; install_yay; echo "exit $?"')
        out = self.fn(["aur_helper", "install_yay"], code, PATH=str(self.bin))[1]
        for t in list(self.bin.iterdir()):
            t.unlink()
        return out

    def test_aur_helper_yay_paru_or_ask_on_manjaro(self):
        self.assertIn("note: paru is installed", self.install_yay("paru"))
        self.assertIn("note: yay is installed", self.install_yay("yay", "paru"))
        out = self.install_yay()
        self.assertNotIn("asked", out)  # (plain Arch: always set up)
        self.assertIn("would run: git clone", out)
        out = self.install_yay("pamac")
        self.assertIn("asked: Install yay too", out)
        self.assertNotIn("git clone", out)
        self.assertIn("exit 0", out)
        self.assertIn("would run: git clone", self.install_yay("pamac", answer="y"))


class OldConfig(Fakes):
    def test_suggestions_from_an_old_hyprland_conf(self):
        hypr = self.home / ".config/hypr"
        hypr.mkdir(parents=True)
        (hypr / "monitors.conf").write_text("monitor = DP-1, 2560x1440@144, 0x0, 1.25\nmonitor = HDMI-A-1, disable\n")
        (hypr / "hyprland.conf.pre-irice").write_text(textwrap.dedent("""\
            # my config
            $terminal = kitty
            source = ~/.config/hypr/monitors.conf
            env = LIBVA_DRIVER_NAME,nvidia
            env = XCURSOR_SIZE,24
            exec-once = waybar
            exec-once = $terminal --class "dropdown" ## not a comment
            input {
                kb_layout = de
                kb_variant = nodeadkeys
                touchpad {
                    natural_scroll = true
                }
            }
            """))
        out = hypr / "irice/from-your-old-config.lua"
        out.parent.mkdir()
        p = subprocess.run(["python3", "-I", str(ROOT / "tools/old-hypr-config.py"), str(hypr / "hyprland.conf.pre-irice"), str(out)],
                           env=self.env, capture_output=True, text=True, timeout=10)
        self.assertEqual(p.returncode, 0, p.stderr)
        self.assertIn("2 display lines, keyboard de (nodeadkeys), 2 env lines (NVIDIA: LIBVA_DRIVER_NAME), 2 exec-once", p.stdout)
        self.assertIn("~/.config/hypr/irice/from-your-old-config.lua", p.stdout)
        lua = out.read_text()
        self.assertIn('-- hl.monitor({ output = "DP-1", mode = "2560x1440@144", position = "0x0", scale = 1.25 })', lua)
        self.assertIn("-- (by hand, from: monitor = HDMI-A-1, disable)", lua)
        self.assertIn('-- hl.config({ input = { kb_layout = "de", kb_variant = "nodeadkeys" } })', lua)
        self.assertIn('-- hl.env("XCURSOR_SIZE", "24")', lua)
        self.assertIn('hl.exec_cmd("waybar") end) -- (iRice does this itself)', lua)
        self.assertIn('hl.exec_cmd("kitty --class \\"dropdown\\" # not a comment")', lua)
        # every line is off
        self.assertEqual([l for l in lua.splitlines() if l and not l.startswith("--")], [])
        # nothing to carry over: no file, nothing said
        (hypr / "empty.conf").write_text("# nothing\ngeneral {\n gaps_in = 4\n}\n")
        p = subprocess.run(["python3", "-I", str(ROOT / "tools/old-hypr-config.py"), str(hypr / "empty.conf"), str(self.d / "x.lua")],
                           env=self.env, capture_output=True, text=True, timeout=10)
        self.assertEqual((p.returncode, p.stdout), (0, ""))
        self.assertFalse((self.d / "x.lua").exists())


class QtTheme(Fakes):
    def test_qt6ct_only_when_set_up_and_kde_isnt_there_or_asked_for(self):
        fn = subprocess.run(["sed", "-n", "/^qt6ct_chosen() {/,/^}/p", str(ROOT / "shell/assets/scripts/launch-shell.sh")],
                            capture_output=True, text=True, check=True).stdout
        plugins = self.d / "root/usr/lib/qt6/plugins/platformthemes"
        plugins.mkdir(parents=True)
        cfg = self.home / ".config"

        def chosen():
            return subprocess.run(["bash", "-c", fn + "\nqt6ct_chosen"], env=dict(self.env, IRICE_SYSTEM_ROOT=str(self.d / "root")),
                                  capture_output=True, timeout=10).returncode == 0
        (plugins / "KDEPlasmaPlatformTheme6.so").write_text("")
        self.assertFalse(chosen())  # (no qt6ct)
        (cfg / "qt6ct").mkdir(parents=True)
        (cfg / "qt6ct/qt6ct.conf").write_text("[Appearance]\n")
        (plugins / "libqt6ct.so").write_text("")
        self.assertFalse(chosen())  # KDE's is there and nothing asks for qt6ct
        (cfg / "environment.d").mkdir()
        (cfg / "environment.d/qt.conf").write_text("QT_QPA_PLATFORMTHEME=qt6ct\n")
        self.assertTrue(chosen())
        (cfg / "environment.d/qt.conf").unlink()
        (plugins / "KDEPlasmaPlatformTheme6.so").unlink()
        self.assertTrue(chosen())  # (no KDE theme to use)


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
