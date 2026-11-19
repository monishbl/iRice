"""Tessera's desktop look (shell/assets/scripts/desktop-look.py) in a fake
home: what it sets up on a CachyOS-like install, what it backs up, that it
puts everything back when turned off, and what it leaves alone."""
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "shell/assets/scripts/desktop-look.py"
BEGIN = "# >>> Tessera's terminal look >>>"

CACHY_FISH = "source /usr/share/cachyos-fish-config/cachyos-config.fish\n\n# overwrite greeting\n#function fish_greeting\n#end\n"
USER_STARSHIP = "format = '$all'\n[character]\nsuccess_symbol = '[>](bold green)'\n"
ARCH_BASHRC = "#\n# ~/.bashrc\n#\n\n[[ $- != *i* ]] && return\n\nalias ls='ls --color=auto'\nPS1='[\\u@\\h \\W]\\$ '\n"
KDEGLOBALS = "[General]\nColorScheme=Tessera\n\n[KDE]\nSingleClick=false\n"


class Home(unittest.TestCase):
    def setUp(self):
        self.d = tempfile.TemporaryDirectory()
        h = Path(self.d.name)
        self.home, self.cfg, self.state = h / "home", h / "home/.config", h / "home/.local/state"
        self.data, self.bin, self.sys = h / "home/.local/share", h / "bin", h / "sys"
        for p in (self.cfg, self.state, self.data, self.bin):
            p.mkdir(parents=True)
        for name in ("fish", "kitty", "starship"):
            self.fake(name, "exit 0\n")
        # gsettings: values in a file
        self.gs = h / "gsettings.json"
        self.gs.write_text(json.dumps({"font-name": "'Adwaita Sans 11'"}))
        self.fake("gsettings", f"""exec {sys.executable} -c '
import json, sys
f = "{self.gs}"
d = json.load(open(f))
if sys.argv[1] == "get": print(d.get(sys.argv[3], "''"))
else: d[sys.argv[3]] = sys.argv[4]; json.dump(d, open(f, "w"))
' "$@"
""")
        (self.sys / "usr/lib/qt6/plugins/styles").mkdir(parents=True)
        (self.sys / "usr/lib/qt6/plugins/styles/darkly6.so").write_text("")
        for t in ("breeze-plus", "breeze-plus-dark"):
            (self.sys / "usr/share/icons" / t).mkdir(parents=True)
            (self.sys / "usr/share/icons" / t / "index.theme").write_text("[Icon Theme]\n")
        self.env = {"HOME": str(self.home), "XDG_CONFIG_HOME": str(self.cfg), "XDG_STATE_HOME": str(self.state), "XDG_DATA_HOME": str(self.data),
                    "PATH": str(self.bin), "TESSERA_LOGIN_SHELL": "/usr/bin/fish", "TESSERA_FONTS": "Google Sans Flex|JetBrainsMono Nerd Font|Inter",
                    "TESSERA_GSETTINGS": str(self.bin / "gsettings"), "TESSERA_SYSTEM_ROOT": str(self.sys), "TESSERA_NO_DOWNLOAD": "1", "TESSERA_FONT_FILE": ""}

    def tearDown(self):
        self.d.cleanup()

    def fake(self, name, body):
        f = self.bin / name
        f.write_text("#!/bin/sh\n" + body)
        f.chmod(0o755)

    def run_look(self, *args, **env):
        p = subprocess.run([sys.executable, "-I", str(SCRIPT), *args], capture_output=True, text=True, env=dict(self.env, **env), timeout=60)
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        return json.loads(p.stdout) if "--text" not in args else p.stdout

    def put(self, rel, text):
        p = self.home / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text)
        return p

    def text(self, rel):
        p = self.home / rel
        return p.read_text() if p.exists() else None

    def cachyos(self):
        self.put(".config/fish/config.fish", CACHY_FISH)
        self.put(".config/starship.toml", USER_STARSHIP)
        self.put(".bashrc", ARCH_BASHRC)
        self.put(".config/kitty/kitty.conf", "include tessera-colors.conf\nfont_size 14\n")
        self.put(".config/kdeglobals", KDEGLOBALS)

    def snapshot(self):
        return {p: self.text(p) for p in (".config/fish/config.fish", ".config/starship.toml", ".bashrc", ".config/kitty/kitty.conf", ".config/kdeglobals")}

    def gtk_font(self):
        return json.loads(self.gs.read_text())["font-name"]

    def test_a_cachyos_install_gets_the_whole_look(self):
        self.cachyos()
        out = self.run_look("install")
        self.assertTrue(out["on"])
        # The prompt: Tessera's starship.toml (yours kept), valid TOML in Tessera's colours
        star = self.text(".config/starship.toml")
        self.assertTrue(star.startswith("# Tessera's terminal look"))
        self.assertEqual(self.text(".config/starship.toml.pre-tessera"), USER_STARSHIP)
        t = tomllib.loads(star)
        self.assertEqual(t["palette"], "tessera")
        self.assertEqual(t["palettes"]["tessera"]["pill"], "#2C2C31")  # (dark: the Quiet Interface's muted)
        self.assertIn("$directory", t["format"])
        # fish: no greeting (CachyOS's fastfetch replaced after it's read), starship
        ours = self.text(".config/fish/conf.d/tessera-terminal.fish")
        self.assertIn("function fish_greeting", ours)
        self.assertIn("starship init fish | source", ours)
        conf = self.text(".config/fish/config.fish")
        self.assertTrue(conf.startswith(CACHY_FISH.rstrip("\n")))
        self.assertIn(BEGIN + "\n", conf)
        self.assertIn("function fish_greeting\nend\n", conf)
        self.assertEqual(self.text(".config/fish/config.fish.pre-tessera"), CACHY_FISH)
        # bash: starship after your lines (a plain PS1 isn't a framework)
        self.assertIn('eval "$(starship init bash)"', self.text(".bashrc"))
        self.assertEqual(self.text(".bashrc.pre-tessera"), ARCH_BASHRC)
        # kitty: Tessera's piece first, your lines after it (they win)
        kitty = self.text(".config/kitty/kitty.conf")
        self.assertTrue(kitty.startswith(BEGIN + "\ninclude tessera-look.conf\n"))
        self.assertTrue(kitty.endswith("include tessera-colors.conf\nfont_size 14\n"))
        self.assertIn("font_family      JetBrainsMono Nerd Font", self.text(".config/kitty/tessera-look.conf"))
        # apps: fonts, Qt style and icons (the colour scheme stays), smoothing, GTK's font
        kde = self.text(".config/kdeglobals")
        for line in ("ColorScheme=Tessera", "font=Google Sans Flex,11,", "menuFont=Google Sans Flex,10,", "fixed=JetBrainsMono Nerd Font,11,",
                     "widgetStyle=Darkly", "Theme=breeze-plus-dark", "SingleClick=false"):
            self.assertIn(line, kde)
        self.assertIn("<const>none</const>", self.text(".config/fontconfig/conf.d/75-tessera-grayscale.conf"))
        self.assertEqual(self.gtk_font(), "'Google Sans Flex Medium 11'")
        # Again: nothing changes
        before = self.snapshot()
        self.assertEqual(self.run_look("install")["did"], [])
        self.assertEqual(self.snapshot(), before)

    def test_off_puts_everything_back_and_stays_off(self):
        self.cachyos()
        original = self.snapshot()
        self.run_look("install")
        out = self.run_look("off")
        self.assertFalse(out["on"])
        self.assertEqual(self.snapshot(), original)
        self.assertEqual(self.gtk_font(), "'Adwaita Sans 11'")
        for rel in (".config/fish/conf.d/tessera-terminal.fish", ".config/kitty/tessera-look.conf", ".config/fontconfig/conf.d/75-tessera-grayscale.conf"):
            self.assertIsNone(self.text(rel), rel)
        # An install (or an update's migration) leaves it off; the switch turns it on
        self.assertEqual(self.run_look("install")["did"], [])
        self.assertEqual(self.snapshot(), original)
        self.assertFalse(self.run_look("status")["on"])
        self.run_look("on")
        self.assertIn("font=Google Sans Flex", self.text(".config/kdeglobals"))

    def test_what_you_changed_since_stays_yours(self):
        self.cachyos()
        self.run_look("install")
        # your own Qt font and GTK font after the look set them
        kde = self.text(".config/kdeglobals").replace("font=Google Sans Flex,11,-1,5,500,0,0,0,0,0,0,0,0,0,0,1,Medium", "font=Inter,12")
        self.put(".config/kdeglobals", kde)
        self.gs.write_text(json.dumps({"font-name": "'Inter 12'"}))
        self.run_look("off")
        self.assertIn("font=Inter,12", self.text(".config/kdeglobals"))
        self.assertNotIn("widgetStyle=Darkly", self.text(".config/kdeglobals"))
        self.assertEqual(self.gtk_font(), "'Inter 12'")

    def test_new_files_go_again(self):
        # nothing of yours: made, and removed again
        out = self.run_look("install", TESSERA_LOGIN_SHELL="/bin/zsh")
        self.assertIn('eval "$(starship init zsh)"', self.text(".zshrc"))
        self.assertIsNone(self.text(".config/starship.toml.pre-tessera"))
        self.run_look("off")
        for rel in (".zshrc", ".config/starship.toml", ".config/kitty/kitty.conf", ".config/kitty/tessera-look.conf"):
            self.assertIsNone(self.text(rel), rel)
        self.assertNotIn("Google Sans", self.text(".config/kdeglobals") or "")

    def test_prompt_frameworks_are_left_alone(self):
        omz = "export ZSH=\"$HOME/.oh-my-zsh\"\nZSH_THEME=robbyrussell\nsource $ZSH/oh-my-zsh.sh\n"
        self.put(".zshrc", omz)
        p10k = "source ~/powerlevel10k/powerlevel10k.zsh-theme\n[[ -f ~/.p10k.zsh ]] && source ~/.p10k.zsh\n"
        out = self.run_look("install", TESSERA_LOGIN_SHELL="/bin/zsh")
        self.assertEqual(self.text(".zshrc"), omz)
        self.assertTrue(any("oh-my-zsh" in n and "left alone" in n for n in out["notes"]), out["notes"])
        self.put(".zshrc", p10k)
        out = self.run_look("on", TESSERA_LOGIN_SHELL="/bin/zsh")
        self.assertEqual(self.text(".zshrc"), p10k)
        self.assertTrue(any("powerlevel10k" in n for n in out["notes"]), out["notes"])
        # fish with your own prompt: no greeting, but no starship either
        self.put(".config/fish/functions/fish_prompt.fish", "function fish_prompt\n echo '> '\nend\n")
        self.run_look("on")
        ours = self.text(".config/fish/conf.d/tessera-terminal.fish")
        self.assertIn("function fish_greeting", ours)
        self.assertNotIn("starship", ours)
        # bash with oh-my-bash
        omb = "export OSH=~/.oh-my-bash\nsource \"$OSH\"/oh-my-bash.sh\n"
        self.put(".bashrc", omb)
        self.run_look("on")
        self.assertEqual(self.text(".bashrc"), omb)

    def test_refresh_follows_light_and_dark_but_not_your_edits(self):
        self.cachyos()
        self.run_look("install")
        self.run_look("refresh", "--mode", "light", "--accent", "#2563EB")
        t = tomllib.loads(self.text(".config/starship.toml"))
        self.assertEqual(t["palettes"]["tessera"]["pill"], "#E4E4E7")
        self.assertEqual(t["palettes"]["tessera"]["accent"], "#2563EB")
        self.assertIn("Theme=breeze-plus\n", self.text(".config/kdeglobals"))
        # your edit to Tessera's file: kept from then on
        edited = self.text(".config/starship.toml").replace("add_newline = false", "add_newline = true")
        self.put(".config/starship.toml", edited)
        self.run_look("refresh", "--mode", "dark")
        self.assertEqual(self.text(".config/starship.toml"), edited)
        self.assertTrue(self.run_look("status")["edited"])

    def test_dry_run_changes_nothing(self):
        self.cachyos()
        before = self.snapshot()
        out = self.run_look("install", "--dry-run", "--text")
        self.assertIn("would: ", out)
        self.assertEqual(self.snapshot(), before)
        self.assertEqual(self.gtk_font(), "'Adwaita Sans 11'")
        self.assertFalse((self.state / "tessera/desktop-look.json").exists())

    def test_migrate_runs_once(self):
        self.cachyos()
        self.run_look("migrate")
        self.assertTrue((self.home / ".config/fish/conf.d/tessera-terminal.fish").exists())
        (self.home / ".config/fish/conf.d/tessera-terminal.fish").unlink()
        self.run_look("migrate")
        self.assertFalse((self.home / ".config/fish/conf.d/tessera-terminal.fish").exists())
        self.assertIn('"migrated": true', (self.state / "tessera/desktop-look.json").read_text())

    def test_the_apps_font_is_downloaded_only_as_expected(self):
        font = Path(self.d.name, "font.ttf")
        font.write_bytes(b"not really a font")
        # (no copy of Tessera's: the original is downloaded)
        env = {"TESSERA_FONTS": "JetBrainsMono Nerd Font", "TESSERA_NO_DOWNLOAD": "0", "TESSERA_FONT_URL": font.as_uri()}
        # the wrong file: not kept, apps keep their font, said so
        out = self.run_look("install", TESSERA_FONT_SHA256="0" * 64, **env)
        self.assertFalse((self.data / "fonts/tessera/GoogleSansFlex.ttf").exists())
        self.assertTrue(any("couldn't be downloaded" in n for n in out["notes"]), out["notes"])
        self.assertEqual(self.gtk_font(), "'Adwaita Sans 11'")
        # the right one
        self.run_look("on", TESSERA_FONT_SHA256=hashlib.sha256(font.read_bytes()).hexdigest(), **env)
        self.assertEqual((self.data / "fonts/tessera/GoogleSansFlex.ttf").read_bytes(), b"not really a font")
        self.assertEqual(self.gtk_font(), "'Google Sans Flex Medium 11'")

    def test_tesseras_own_copy_of_the_font_goes_first(self):
        src = Path(self.d.name, "material/assets/fonts")
        src.mkdir(parents=True)
        (src / "GoogleSansFlex.ttf").write_bytes(b"tessera sans flex")
        (src / "LICENSE-google-sans-flex").write_text("SIL Open Font License")
        self.run_look("install", TESSERA_FONTS="JetBrainsMono Nerd Font", TESSERA_FONT_FILE=str(src / "GoogleSansFlex.ttf"), TESSERA_NO_DOWNLOAD="0",
                      TESSERA_FONT_URL="file:///nonexistent")
        self.assertEqual((self.data / "fonts/tessera/TesseraSansFlex.ttf").read_bytes(), b"tessera sans flex")
        self.assertTrue((self.data / "fonts/tessera/LICENSE-tessera-sans-flex").exists())
        self.assertEqual(self.gtk_font(), "'Tessera Sans Flex Medium 11'")
        self.assertIn("font=Tessera Sans Flex,11,", self.text(".config/kdeglobals"))

    def test_the_repository_has_the_font(self):
        # (what bundled_font finds from the checkout)
        self.assertTrue((ROOT / "material/assets/fonts/GoogleSansFlex.ttf").is_file())

    def test_without_a_nerd_font_or_darkly(self):
        (self.sys / "usr/lib/qt6/plugins/styles/darkly6.so").unlink()
        (self.bin / "starship").unlink()
        out = self.run_look("install", TESSERA_FONTS="Inter")
        t = tomllib.loads(self.text(".config/starship.toml"))
        self.assertNotIn("", self.text(".config/starship.toml"))  # (no powerline caps without the font)
        self.assertEqual(t["character"]["success_symbol"], "[❯](bold accent)")
        notes = " ".join(out["notes"])
        self.assertIn("Darkly isn't installed", notes)
        self.assertIn("starship isn't installed", notes)
        self.assertNotIn("widgetStyle", self.text(".config/kdeglobals") or "")


if __name__ == "__main__":
    unittest.main()


class Keyring(Home):
    """Electron and Chromium apps told to keep sign-ins in the keyring (19 Nov)"""

    def test_flags_written_kept_and_restored(self):
        self.fake("gnome-keyring-daemon", "exit 0\n")
        self.put(".config/chrome-flags.conf", "--ozone-platform-hint=wayland\n")
        self.put(".config/thorium-flags.conf", "--password-store=basic\n")  # (the user's own choice)
        self.run_look("install")
        self.assertIn("--password-store=gnome-libsecret", self.text(".config/code-flags.conf"))
        chrome = self.text(".config/chrome-flags.conf")
        self.assertIn("--ozone-platform-hint=wayland", chrome)
        self.assertIn("--password-store=gnome-libsecret", chrome)
        self.assertNotIn("gnome-libsecret", self.text(".config/thorium-flags.conf"))
        self.run_look("install")  # (once, however often)
        self.assertEqual(self.text(".config/chrome-flags.conf").count("--password-store"), 1)
        self.run_look("off")
        self.assertIsNone(self.text(".config/code-flags.conf"))
        self.assertEqual(self.text(".config/chrome-flags.conf").strip(), "--ozone-platform-hint=wayland")

    def test_no_keyring_no_flags(self):
        self.run_look("install")
        self.assertIsNone(self.text(".config/code-flags.conf"))


class DiscordLauncher(Home):
    """Discord's launcher reads no flag file: an app-menu entry of its own
    with the keyring switch (19 Nov)"""

    def test_override_written_and_removed(self):
        self.fake("gnome-keyring-daemon", "exit 0\n")
        apps = self.sys / "usr/share/applications"
        apps.mkdir(parents=True, exist_ok=True)
        (apps / "discord.desktop").write_text("[Desktop Entry]\nName=Discord\nExec=/usr/bin/discord --url -- %u\n")
        self.run_look("install")
        mine = self.text(".local/share/applications/discord.desktop")
        self.assertIn("Exec=/usr/bin/discord --password-store=gnome-libsecret --url -- %u", mine)
        self.run_look("off")
        self.assertIsNone(self.text(".local/share/applications/discord.desktop"))

    def test_your_own_entry_is_left_alone(self):
        self.fake("gnome-keyring-daemon", "exit 0\n")
        apps = self.sys / "usr/share/applications"
        apps.mkdir(parents=True, exist_ok=True)
        (apps / "discord.desktop").write_text("[Desktop Entry]\nExec=/usr/bin/discord\n")
        self.put(".local/share/applications/discord.desktop", "[Desktop Entry]\nExec=/usr/bin/discord --mine\n")
        self.run_look("install")
        self.assertIn("--mine", self.text(".local/share/applications/discord.desktop"))
