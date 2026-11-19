#!/usr/bin/env python3
"""Tessera's desktop look: the same apps and terminal on every install,
as Tessera was designed on.

  Apps
  - the apps' font: Tessera Sans Flex, Tessera's own copy of Google Sans
    Flex (SIL Open Font License 1.1, no reserved names; renamed so Google's
    trademark doesn't name a product: the Material look's font,
    material/assets/fonts) for GTK (gsettings font-name) and Qt (kdeglobals:
    font, menuFont, toolBarFont, smallestReadableFont), and JetBrains Mono
    Nerd Font as the fixed one. Not installed: copied with its licence into
    ~/.local/share/fonts/tessera; without Tessera's copy, the original
    downloaded there from Google Fonts' repository (a pinned commit and
    SHA-256). An installed Google Sans Flex is used as it is
  - Qt apps' style Darkly and Breeze Plus icons (light or dark), when they're
    installed (the installer's app style, from the AUR); Tessera's colour
    scheme stays desktop-theme.py's
  - grayscale font smoothing (fontconfig conf.d/75-tessera-grayscale.conf)
  Terminal
  - fish without a greeting (CachyOS's cachyos-fish-config greets with
    fastfetch: its fish_greeting is replaced after it's read, the rest of
    it stays as it is)
  - the starship prompt in Tessera's colours, light or dark and with the
    accent (starship.toml made from them, and made again when they change:
    app-theme.sh calls `refresh`), in fish, zsh and bash; a prompt framework
    there (oh-my-zsh, powerlevel10k, prezto, oh-my-bash, bash-it, tide…) is
    left alone, and said so
  - kitty to match: tessera-look.conf (font, cursor, padding, keys) included
    first in kitty.conf, so your own lines after it win; the colours stay
    Match apps' (tessera-colors.conf)

What it replaces is kept first as <file>.pre-tessera (once: the first one
is yours); a setting it changes (gsettings, a kdeglobals key) is kept in its
state, and put back when the look is turned off, unless you changed it
since. Lines it adds to your files sit between two marker lines and go
again; whole files it wrote go, and a file it replaced comes back from its
.pre-tessera copy. A font it downloaded stays (nothing uses it then).

  desktop-look.py status                       JSON: on, what's set up, notes
  desktop-look.py install [--dry-run] [--text] set it up unless it was turned off
                                               (the installer; marks it migrated)
  desktop-look.py migrate                      once per machine (the shell's start
                                               after an update): install, then never again
  desktop-look.py on | off                     Settings → Appearance → Tessera's
                                               desktop look
  desktop-look.py refresh [--mode dark|light] [--accent #rrggbb]
                                               the prompt's colours and the icons'
                                               light or dark again (only what it set
                                               and you haven't changed)

State: $XDG_STATE_HOME/tessera/desktop-look.json. For the tests:
TESSERA_LOGIN_SHELL (the login shell), TESSERA_FONTS (the font families
there are, "|"-separated, instead of fc-list), TESSERA_GSETTINGS (the
gsettings command), TESSERA_SYSTEM_ROOT (where /usr is), TESSERA_FONT_FILE
(Tessera's copy of the font; empty: none), TESSERA_FONT_URL and
TESSERA_FONT_SHA256 (where the original comes from).
"""
import argparse
import hashlib
import shlex
import urllib.request
import json
import os
import pwd
import re
import shutil
import subprocess
import sys

BEGIN = "# >>> Tessera's terminal look >>>"
END = "# <<< Tessera's terminal look <<<"
HEADER = "# Tessera's terminal look"
BACKUP = ".pre-tessera"
WHERE = "Settings → Appearance → Tessera's desktop look"
SYS = os.environ.get("TESSERA_SYSTEM_ROOT", "")
# Google Sans Flex, the original from Google Fonts' repository (google/fonts,
# ofl/googlesansflex at this commit), and its licence
GF = "https://raw.githubusercontent.com/google/fonts/fa826bb2a3e26df8c1bee99351fc0d039aeed7d2/ofl/googlesansflex/"
FONT_URL = GF + "GoogleSansFlex%5BROND,opsz,slnt,wdth,wght%5D.ttf"
FONT_SHA256 = "a9ac770798857d5e8ac593ced42b22e40a7c08cec98319f92b0eccc2783a77ba"
OFL_URL = GF + "OFL.txt"
OFL_SHA256 = "fc13d69f63e36d284b6e383d4d1463d8ad404f0aa065e57cf961252d51063137"
# The apps' font: Tessera's own copy of Google Sans Flex, renamed "Tessera
# Sans Flex" (material/assets/fonts, tools/gen-material.sh; the rename keeps
# Google's trademark from naming a product); else the original, downloaded
UI_FONT = "Tessera Sans Flex"
UI_FONTS = ("Tessera Sans Flex", "Google Sans Flex")


def env_dir(name, default):
    v = os.environ.get(name, "")
    return v if os.path.isabs(v) else os.path.join(os.path.expanduser("~"), default)


class Paths:
    def __init__(self):
        self.home = os.path.expanduser("~")
        self.cfg = env_dir("XDG_CONFIG_HOME", ".config")
        self.state_dir = os.path.join(env_dir("XDG_STATE_HOME", ".local/state"), "tessera")
        self.state = os.path.join(self.state_dir, "desktop-look.json")
        self.starship = os.path.join(self.cfg, "starship.toml")
        self.fish = os.path.join(self.cfg, "fish")
        self.fish_conf = os.path.join(self.fish, "config.fish")
        self.fish_ours = os.path.join(self.fish, "conf.d", "tessera-terminal.fish")
        zdot = os.environ.get("ZDOTDIR") or self.home
        self.zshrc = os.path.join(zdot, ".zshrc")
        self.bashrc = os.path.join(self.home, ".bashrc")
        self.kitty = os.path.join(self.cfg, "kitty")
        self.kitty_conf = os.path.join(self.kitty, "kitty.conf")
        self.kitty_ours = os.path.join(self.kitty, "tessera-look.conf")
        self.tessera_cfg = os.path.join(self.cfg, "tessera", "config.json")
        self.kdeglobals = os.path.join(self.cfg, "kdeglobals")
        self.fontconfig = os.path.join(self.cfg, "fontconfig", "conf.d", "75-tessera-grayscale.conf")
        self.fonts = os.path.join(env_dir("XDG_DATA_HOME", ".local/share"), "fonts", "tessera")
        self.apps = os.path.join(env_dir("XDG_DATA_HOME", ".local/share"), "applications")


# ── Small things ──
def read(path):
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            return f.read()
    except OSError:
        return None


def sha(text):
    return hashlib.sha256(text.encode()).hexdigest()


def load_state(p):
    try:
        with open(p.state) as f:
            s = json.load(f)
        return s if isinstance(s, dict) else {}
    except (OSError, ValueError):
        return {}


def save_state(p, s):
    os.makedirs(p.state_dir, exist_ok=True)
    tmp = p.state + ".tmp"
    with open(tmp, "w") as f:
        json.dump(s, f, indent=1, sort_keys=True)
    os.replace(tmp, p.state)


def login_shell():
    s = os.environ.get("TESSERA_LOGIN_SHELL")
    if s is None:
        try:
            s = pwd.getpwuid(os.getuid()).pw_shell
        except KeyError:
            s = ""
    return os.path.basename(s or "")


_FAMILIES = None


def families(refresh=False):
    """The font families installed (fc-list, once)"""
    global _FAMILIES
    v = os.environ.get("TESSERA_FONTS")
    if v is not None:
        return set(x for x in v.split("|") if x)
    if _FAMILIES is None or refresh:
        try:
            out = subprocess.run(["fc-list", ":", "family"], capture_output=True, text=True, timeout=20).stdout
        except (OSError, subprocess.SubprocessError):
            out = ""
        _FAMILIES = set(f.strip() for line in out.splitlines() for f in line.split(","))
    return _FAMILIES


def nerd_font():
    return any("Nerd Font" in f or re.search(r"\bNF\b", f) for f in families())


def ui_family():
    """The apps' font family installed (Tessera's first), or None"""
    have = families()
    for name in UI_FONTS:
        if any(f == name or f.startswith(name + " ") for f in have):
            return name
    return None


def has_ui_font():
    return ui_family() is not None


def bundled_font():
    """Tessera's copy of the font (the Material look's), from this checkout or
    the one Tessera was installed from"""
    v = os.environ.get("TESSERA_FONT_FILE")
    if v is not None:
        return v if v and os.path.isfile(v) else None
    here = os.path.dirname(os.path.realpath(__file__))
    roots = [os.path.join(here, "..", "..", "..")]
    src = read(os.path.join(env_dir("XDG_DATA_HOME", ".local/share"), "tessera", "source"))
    if src and src.strip():
        roots.append(src.strip())
    roots.append(os.path.join(here, "..", "..", "..", "material", ".."))  # (installed: ~/.config/quickshell/material)
    for r in roots:
        f = os.path.join(r, "material", "assets", "fonts", "GoogleSansFlex.ttf")
        if os.path.isfile(f):
            return os.path.normpath(f)
    return None

def strip_block(text):
    """The text without the marked lines (and the blank line before them)"""
    out = re.sub(r"\n?" + re.escape(BEGIN) + r".*?" + re.escape(END) + r"[^\n]*\n?", "\n", text, flags=re.S)
    return out.lstrip("\n") if text.startswith(BEGIN) else out


def without_comments(text):
    return "\n".join(l.split("#", 1)[0] for l in strip_block(text).splitlines())


class Doer:
    """Changes files, or (a dry run) only says what it would change"""

    def __init__(self, dry):
        self.dry = dry
        self.did = []

    def write(self, path, text, what):
        if read(path) == text:
            return False
        self.did.append(what)
        if not self.dry:
            os.makedirs(os.path.dirname(path), exist_ok=True)
            tmp = path + ".tessera-tmp"
            with open(tmp, "w", encoding="utf-8") as f:
                f.write(text)
            if os.path.exists(path):
                shutil.copymode(path, tmp)
            os.replace(tmp, path)
        return True

    def backup(self, path):
        """<file>.pre-tessera, once (the first copy is the one that's yours)"""
        if os.path.exists(path) and not os.path.exists(path + BACKUP):
            self.did.append("kept " + home(path) + " as " + os.path.basename(path) + BACKUP)
            if not self.dry:
                shutil.copy2(path, path + BACKUP)

    def remove(self, path, what):
        if os.path.lexists(path):
            self.did.append(what)
            if not self.dry:
                os.remove(path)

    def restore(self, path, what):
        self.did.append(what)
        if not self.dry:
            os.replace(path + BACKUP, path)


def home(path):
    h = os.path.expanduser("~")
    return "~" + path[len(h):] if path.startswith(h + "/") else path


# ── Colours ──
NEUTRAL = {
    # (The Quiet Interface's neutrals: muted pill, raised path, primary and secondary text)
    "dark": {"pill": "#2C2C31", "pill_text": "#EDEDEF", "path": "#232327", "path_text": "#A1A1AA", "accent": "#818CF8", "error": "#F87171"},
    "light": {"pill": "#E4E4E7", "pill_text": "#18181B", "path": "#F4F4F5", "path_text": "#52525B", "accent": "#4F46E5", "error": "#B91C1C"},
}


def hex_ok(c):
    return isinstance(c, str) and re.fullmatch(r"#[0-9a-fA-F]{6}", c) is not None


def current_mode(p, state):
    """dark or light: what refresh last heard (app-theme.sh knows), else
    Settings' Appearance (auto: the desktop's preference)"""
    if state.get("mode") in ("dark", "light"):
        return state["mode"]
    try:
        with open(p.tessera_cfg) as f:
            m = (json.load(f).get("appearance") or {}).get("mode")
    except (OSError, ValueError, AttributeError):
        m = None
    if m in ("dark", "light"):
        return m
    try:
        out = subprocess.run(gsettings() + ["get", "org.gnome.desktop.interface", "color-scheme"], capture_output=True, text=True, timeout=5).stdout
        if "light" in out:
            return "light"
    except (OSError, subprocess.SubprocessError):
        pass
    return "dark"


def palette(mode, accent):
    c = dict(NEUTRAL["light" if mode == "light" else "dark"])
    if hex_ok(accent):
        c["accent"] = accent.upper()
    return c


def starship_toml(mode, accent, nerd):
    c = palette(mode, accent)
    pal = "\n".join('%s = "%s"' % (k, v) for k, v in c.items())
    head = (HEADER + " (" + WHERE + ").\n"
            "# Made by Tessera in its colours (" + mode + "), and made again when they change.\n"
            "# Edit it and Tessera leaves it as it is from then on; turning the look off\n"
            "# puts back the one you had (starship.toml" + BACKUP + ").\n")
    if nerd:
        body = '''add_newline = false
format = """
$cmd_duration $directory$git_branch$git_state
  $character"""
palette = "tessera"

[palettes.tessera]
%s

[character]
success_symbol = "[\U0000ea71 \U0000f04b](bold accent)"
error_symbol = "[\U0000f00d \U0000f04b](bold error)"

[package]
disabled = true

[cmd_duration]
min_time = 0
format = "[\U0000e0b6](fg:pill)[\U000f0aa2 $duration](bg:pill fg:pill_text)[\U0000e0b4](fg:pill)"

[directory]
home_symbol = "\U0000f46d "
read_only = " \U0000f456 "
truncation_length = 2
truncation_symbol = "…/"
style = "bg:path fg:path_text"
format = "[\U0000e0b6](fg:path)[\U000f024b → $path$read_only]($style)[\U0000e0b4](fg:path)"

[directory.substitutions]
"Desktop" = " \U0000f4a9 "
"Documents" = " \U0000f4a5 "
"Downloads" = " \U0000f409 "
"Music" = " \U000f0388 "
"Pictures" = " \U0000f40f "
"Videos" = " \U0000f52c "

[git_branch]
symbol = "\U000f062c"
truncation_length = 12
truncation_symbol = ""
format = " \U000f0725 [\U0000e0b6](fg:pill)[$symbol $branch(:$remote_branch)](fg:pill_text bg:pill)[\U0000e0b4](fg:pill)"

[git_state]
format = '[ \\($state( $progress_current of $progress_total)\\)](fg:error) '
''' % pal
    else:
        # (no Nerd Font: no powerline caps or icons, which would show as boxes)
        body = '''add_newline = false
format = """
$cmd_duration $directory$git_branch$git_state
  $character"""
palette = "tessera"

[palettes.tessera]
%s

[character]
success_symbol = "[❯](bold accent)"
error_symbol = "[❯](bold error)"

[package]
disabled = true

[cmd_duration]
min_time = 0
format = "[ $duration ](bg:pill fg:pill_text)"

[directory]
home_symbol = "~"
read_only = " (read-only)"
truncation_length = 2
truncation_symbol = "…/"
style = "bg:path fg:path_text"
format = "[ $path$read_only ]($style)"

[git_branch]
truncation_length = 12
truncation_symbol = ""
format = " [ $branch(:$remote_branch) ](fg:pill_text bg:pill)"

[git_state]
format = '[ \\($state( $progress_current of $progress_total)\\)](fg:error) '
''' % pal
    return head + body


# ── Shells ──
FISH_FRAMEWORKS = (("tide", "tide"), ("pure", "pure"), ("hydro", "hydro"), ("bobthefish", "bobthefish"), ("spacefish", "spacefish"), ("oh-my-posh", "oh-my-posh"))


def fish_prompt_owner(p):
    """What already draws fish's prompt, or None"""
    conf = without_comments(read(p.fish_conf) or "")
    if "starship init" in conf:
        return "your config.fish (it starts starship itself)"
    if os.path.exists(os.path.join(p.fish, "functions", "fish_prompt.fish")):
        return "your own fish_prompt"
    plugins = (read(os.path.join(p.fish, "fish_plugins")) or "").lower()
    for key, name in FISH_FRAMEWORKS:
        if key in plugins:
            return name
    if os.path.exists(os.path.join(p.fish, "conf.d", "_tide_init.fish")):
        return "tide"
    if os.path.isdir(os.path.join(p.home, ".local", "share", "omf")):
        return "oh-my-fish"
    if re.search(r"^\s*function\s+fish_prompt\b", conf, re.M):
        return "your own fish_prompt"
    return None


def fish_ours(starship):
    lines = [HEADER + " (" + WHERE + "):",
             "# no greeting" + (", and the starship prompt" if starship else "") + ". Tessera writes this file; turning the look off removes it.",
             "if status is-interactive",
             "    set -g fish_greeting",
             "    function fish_greeting",
             "    end"]
    if starship:
        lines += ["    if test \"$TERM\" != linux; and command -q starship",
                  "        function starship_transient_prompt_func",
                  "            starship module character",
                  "        end",
                  "        starship init fish | source",
                  "        functions -q enable_transience; and enable_transience",
                  "    end"]
    lines += ["end", ""]
    return "\n".join(lines)


FISH_QUIET = BEGIN + "\n# (" + WHERE + ": no greeting; your config.fish sources one that greets)\nfunction fish_greeting\nend\n" + END + "\n"


def fish_needs_quiet(conf):
    """config.fish reads a greeting after conf.d (CachyOS's) and doesn't
    define its own"""
    c = without_comments(conf)
    if re.search(r"^\s*function\s+fish_greeting\b", c, re.M):
        return False
    return "cachyos-config.fish" in c or re.search(r"^\s*source\s+/usr/share/\S+\.fish", c, re.M) is not None


def rc_owner(text, kind):
    """A prompt framework in a zsh or bash rc, or None"""
    t = without_comments(text)
    checks = {
        "zsh": (("powerlevel10k", "powerlevel10k"), ("p10k", "powerlevel10k"), ("oh-my-zsh", "oh-my-zsh"), ("$ZSH/", "oh-my-zsh"),
                ("zprezto", "prezto"), ("spaceship", "spaceship"), ("oh-my-posh", "oh-my-posh"), ("starship init", "starship (already started there)")),
        "bash": (("oh-my-bash", "oh-my-bash"), ("bash_it", "bash-it"), ("BASH_IT", "bash-it"), ("liquidprompt", "liquidprompt"),
                 ("powerline", "powerline"), ("synth-shell", "synth-shell"), ("oh-my-posh", "oh-my-posh"), ("starship init", "starship (already started there)")),
    }[kind]
    for key, name in checks:
        if key in t:
            return name
    return None


RC_BLOCK = {
    "zsh": BEGIN + "\n# (" + WHERE + ": the starship prompt; turning the look off removes these lines)\n"
           "if [[ -o interactive && $TERM != linux ]] && command -v starship >/dev/null; then eval \"$(starship init zsh)\"; fi\n" + END + "\n",
    "bash": BEGIN + "\n# (" + WHERE + ": the starship prompt; turning the look off removes these lines)\n"
            "if [[ $- == *i* && $TERM != linux ]] && command -v starship >/dev/null; then eval \"$(starship init bash)\"; fi\n" + END + "\n",
}


def add_block(d, path, block, top=False):
    """The marked lines in a file (at the end, or the top), once; the file
    kept as .pre-tessera before it's first changed. Returns whether it made the file"""
    cur = read(path)
    made = cur is None
    base = strip_block(cur or "")
    if top:
        new = block + ("\n" + base if base.strip() else "")
    else:
        new = (base.rstrip("\n") + "\n\n" if base.strip() else "") + block
    if cur is not None and BEGIN not in cur:
        d.backup(path)
    d.write(path, new, ("made " if made else "changed ") + home(path))
    return made


def remove_block(d, path, created):
    cur = read(path)
    if cur is None or BEGIN not in cur:
        return
    new = strip_block(cur)
    if not new.strip() and created:
        d.remove(path, "removed " + home(path))
    else:
        d.write(path, new.rstrip("\n") + "\n" if new.strip() else "", "took Tessera's lines out of " + home(path))


# ── Sign-ins kept: Electron and Chromium apps told which keyring to use ──
# They pick one from the desktop's name and don't know "Hyprland", so on a
# fresh install VS Code, Discord, Chrome… saved sign-ins nowhere and asked
# again every start (19 Nov: system1; the user's own laptop had these lines)
FLAG_FILES = ("code-flags.conf", "electron-flags.conf", "chrome-flags.conf", "chromium-flags.conf",
              "brave-flags.conf", "thorium-flags.conf", "discord-flags.conf")
KEYRING_BLOCK = BEGIN + "\n--password-store=gnome-libsecret\n" + END + "\n"


def keyring_ready():
    """gnome-keyring (or another Secret Service) is there to keep them"""
    return which("gnome-keyring-daemon") or which("kwalletd6") or which("keepassxc")


def apply_keyring(p, d, created):
    notes = []
    if not keyring_ready():
        return ["! No keyring is installed, so apps can't keep sign-ins (sudo pacman -S gnome-keyring)"]
    for name in FLAG_FILES:
        path = os.path.join(p.cfg, name)
        cur = read(path) or ""
        # (a --password-store of your own wins)
        if "--password-store" in strip_block(cur):
            continue
        if add_block(d, path, KEYRING_BLOCK):
            created.add(name)
    keyring_launchers(p, d)
    notes.append("apps: sign-ins kept in your keyring (VS Code, Chrome, Discord and other Electron apps)")
    return notes


def restore_keyring(p, d, created):
    for name in FLAG_FILES:
        remove_block(d, os.path.join(p.cfg, name), name in created)
    for name in LAUNCHER_APPS:
        path = os.path.join(p.apps, name)
        if (read(path) or "").startswith(DESKTOP_MARK):
            d.remove(path, "removed Tessera's " + home(path))


# Apps whose launcher reads no flag file (Discord's shell script runs its own
# copy): an entry of the same name in ~/.local/share/applications, which wins
# over the system's, with the switch on its Exec lines (19 Nov: system1)
LAUNCHER_APPS = ("discord.desktop", "discord-ptb.desktop", "discord-canary.desktop")
DESKTOP_MARK = "# Tessera's desktop look: sign-ins kept in the keyring (Settings → Appearance)\n"


def keyring_launchers(p, d):
    for name in LAUNCHER_APPS:
        mine = os.path.join(p.apps, name)
        cur = read(mine)
        if cur is not None and not cur.startswith(DESKTOP_MARK):
            continue  # (your own entry: left alone)
        system = next((t for t in (read(os.path.join(SYS + base, name)) for base in ("/usr/share/applications", "/usr/local/share/applications")) if t), None)
        if system is None:
            if cur is not None:
                d.remove(mine, "removed Tessera's " + home(mine))
            continue
        lines = []
        for line in system.splitlines():
            if line.startswith("Exec=") and "--password-store" not in line:
                cmd = line[5:].split(" ", 1)
                line = "Exec=" + cmd[0] + " --password-store=gnome-libsecret" + (" " + cmd[1] if len(cmd) > 1 else "")
            lines.append(line)
        text = DESKTOP_MARK + "\n".join(lines) + "\n"
        if text != cur:
            d.write(mine, text, "wrote " + home(mine))


# ── kitty ──
def kitty_ours(nerd):
    font = "JetBrainsMono Nerd Font" if nerd else "JetBrains Mono"
    return "\n".join([
        HEADER + " for kitty (" + WHERE + ").",
        "# Tessera writes this file; your own settings go in kitty.conf, after its include line (they win).",
        "# The colours are Match apps' (tessera-colors.conf).",
        "font_family      " + font,
        "font_size        11.0",
        "cursor_shape     beam",
        "cursor_trail     1",
        "window_margin_width 21.75",
        "confirm_os_window_close 0",
        "map ctrl+c       copy_or_interrupt",
        "map page_up      scroll_page_up",
        "map page_down    scroll_page_down",
        "map ctrl+plus    change_font_size all +1",
        "map ctrl+equal   change_font_size all +1",
        "map ctrl+kp_add  change_font_size all +1",
        "map ctrl+minus   change_font_size all -1",
        "map ctrl+underscore change_font_size all -1",
        "map ctrl+kp_subtract change_font_size all -1",
        "map ctrl+0       change_font_size all 0",
        "map ctrl+kp_0    change_font_size all 0",
        ""])


KITTY_BLOCK = BEGIN + "\ninclude tessera-look.conf\n" + END + "\n"


def which(name):
    return shutil.which(name) is not None


# ── Apps: fonts, Qt style, smoothing ──
def gsettings():
    return shlex.split(os.environ.get("TESSERA_GSETTINGS", "gsettings"))


def gs_get(key):
    try:
        r = subprocess.run(gsettings() + ["get", "org.gnome.desktop.interface", key], capture_output=True, text=True, timeout=10)
    except (OSError, subprocess.SubprocessError):
        return None
    return r.stdout.strip() if r.returncode == 0 else None


def gs_set(key, value):
    """value as gsettings prints it ('…' for a string)"""
    try:
        return subprocess.run(gsettings() + ["set", "org.gnome.desktop.interface", key, value], capture_output=True, timeout=10).returncode == 0
    except (OSError, subprocess.SubprocessError):
        return False


def ini_get(text, group, key):
    m = re.search(r"^\[" + re.escape(group) + r"\]\s*$(.*?)(?=^\[|\Z)", text, re.M | re.S)
    if not m:
        return None
    k = re.search(r"^" + re.escape(key) + r"\s*=(.*)$", m.group(1), re.M)
    return k.group(1).strip() if k else None


def ini_set(text, group, key, value):
    """One key in a group set (value None: the key taken out); the rest as it is"""
    m = re.search(r"^\[" + re.escape(group) + r"\]\s*$", text, re.M)
    if not m:
        if value is None:
            return text
        return (text.rstrip("\n") + "\n\n" if text.strip() else "") + "[" + group + "]\n" + key + "=" + value + "\n"
    start = m.end()
    nxt = re.search(r"^\[", text[start:], re.M)
    end = start + nxt.start() if nxt else len(text)
    body = text[start:end]
    pat = re.compile(r"^" + re.escape(key) + r"\s*=.*\n?", re.M)
    if value is None:
        body = pat.sub("", body)
        if not body.strip():
            # (a group left empty goes, with the blank line before it)
            head = text[:m.start()].rstrip("\n")
            return head + ("\n" if head else "") + ("\n" + text[end:] if nxt and head else text[end:])
    elif pat.search(body):
        body = pat.sub(lambda _m: key + "=" + value + "\n", body, count=1)
    else:
        body = body.rstrip("\n") + "\n" + key + "=" + value + "\n" + ("\n" if nxt else "")
    return text[:start] + body + text[end:]


def qt_font(family, size, weight=500, style=""):
    return "%s,%d,-1,5,%d,0,0,0,0,0,0,0,0,0,0,1%s" % (family, size, weight, "," + style if style else "")


def darkly():
    return os.path.exists(SYS + "/usr/lib/qt6/plugins/styles/darkly6.so")


def breeze_plus(mode):
    name = "breeze-plus-dark" if mode == "dark" else "breeze-plus"
    return name if os.path.exists(SYS + "/usr/share/icons/" + name + "/index.theme") else None


def kde_wanted(mode):
    """kdeglobals keys the look sets, as (group, key, value)"""
    want = []
    fam = ui_family()
    if fam:
        want += [("General", "font", qt_font(fam, 11, 500, "Medium")), ("General", "menuFont", qt_font(fam, 10, 500, "Medium")),
                 ("General", "toolBarFont", qt_font(fam, 10, 500, "Medium")), ("General", "smallestReadableFont", qt_font(fam, 9, 500, "Medium"))]
    if nerd_font():
        want.append(("General", "fixed", qt_font("JetBrainsMono Nerd Font", 11, 400)))
    if darkly():
        want.append(("KDE", "widgetStyle", "Darkly"))
    icons = breeze_plus(mode)
    if icons:
        want.append(("Icons", "Theme", icons))
    return want


def set_kde(p, state, d, want):
    """The keys set, each one's value before kept (once) to put back"""
    text = read(p.kdeglobals)
    if text is None:
        text = ""
    saved = state.setdefault("kde", {})
    new = text
    for group, key, value in want:
        name = group + "/" + key
        cur = ini_get(new, group, key)
        if name not in saved:
            saved[name] = {"before": cur, "set": value}
        elif cur != saved[name]["set"] and cur != value:
            continue  # (changed by you since: yours)
        saved[name]["set"] = value
        new = ini_set(new, group, key, value)
    if new != text:
        if text:
            d.backup(p.kdeglobals)
        d.write(p.kdeglobals, new, "set Qt apps' fonts, style and icons in " + home(p.kdeglobals))


def unset_kde(p, state, d):
    text = read(p.kdeglobals)
    saved = state.get("kde", {})
    if text is None or not saved:
        state.pop("kde", None)
        return
    new = text
    for name, v in saved.items():
        group, key = name.split("/", 1)
        if ini_get(new, group, key) == v["set"]:
            new = ini_set(new, group, key, v["before"])
    d.write(p.kdeglobals, new, "put back Qt apps' fonts, style and icons in " + home(p.kdeglobals))
    state.pop("kde", None)


GRAYSCALE = """<?xml version="1.0"?>
<!DOCTYPE fontconfig SYSTEM "urn:fontconfig:fonts.dtd">
<!-- Tessera's desktop look (Settings → Appearance): grayscale font smoothing.
     Tessera writes this file; turning the look off removes it. -->
<fontconfig>
  <match target="font">
    <edit name="rgba" mode="assign"><const>none</const></edit>
  </match>
</fontconfig>
"""


def fetch(url, digest, dest):
    """A file downloaded, kept only when its SHA-256 is the one expected"""
    with urllib.request.urlopen(url, timeout=60) as r:
        data = r.read(64 << 20)
    if hashlib.sha256(data).hexdigest() != digest:
        raise OSError("the download wasn't the file expected (SHA-256)")
    tmp = dest + ".part"
    with open(tmp, "wb") as f:
        f.write(data)
    os.replace(tmp, dest)


def font_installed(p, name):
    """fontconfig told about a font just put in ~/.local/share/fonts/tessera"""
    try:
        subprocess.run(["fc-cache", "-f", p.fonts], capture_output=True, timeout=120)
    except (OSError, subprocess.SubprocessError):
        pass
    if os.environ.get("TESSERA_FONTS") is None:
        families(refresh=True)
    else:
        os.environ["TESSERA_FONTS"] += "|" + name


def get_ui_font(p, d):
    """The apps' font when it isn't installed: Tessera's own copy (with its
    licence) into ~/.local/share/fonts/tessera; without one, the original
    Google Sans Flex downloaded there. Returns a note when it can't"""
    if has_ui_font():
        return None
    copy = bundled_font()
    if copy:
        d.did.append("installed " + UI_FONT + " (SIL Open Font License) in " + home(p.fonts))
        if not d.dry:
            os.makedirs(p.fonts, exist_ok=True)
            shutil.copyfile(copy, os.path.join(p.fonts, "TesseraSansFlex.ttf"))
            lic = os.path.join(os.path.dirname(copy), "LICENSE-google-sans-flex")
            if os.path.isfile(lic):
                shutil.copyfile(lic, os.path.join(p.fonts, "LICENSE-tessera-sans-flex"))
            font_installed(p, UI_FONT)
        return None
    if os.environ.get("TESSERA_NO_DOWNLOAD") == "1":
        return "! " + UI_FONT + " isn't installed, so apps keep their font"
    d.did.append("downloaded Google Sans Flex (SIL Open Font License) to " + home(p.fonts))
    if d.dry:
        return None
    try:
        os.makedirs(p.fonts, exist_ok=True)
        fetch(os.environ.get("TESSERA_FONT_URL", FONT_URL), os.environ.get("TESSERA_FONT_SHA256", FONT_SHA256), os.path.join(p.fonts, "GoogleSansFlex.ttf"))
        if "TESSERA_FONT_URL" not in os.environ:
            fetch(OFL_URL, OFL_SHA256, os.path.join(p.fonts, "GoogleSansFlex-OFL.txt"))
    except (OSError, ValueError) as e:
        d.did.pop()
        return "! The apps' font couldn't be downloaded (" + str(e) + "), so apps keep theirs; turn the look off and on to try again"
    font_installed(p, "Google Sans Flex")
    return None


def apply_apps(p, state, d, mode):
    notes = []
    n = get_ui_font(p, d)
    if n:
        notes.append(n)
    # GTK's font (its value before kept once, to put back)
    fam = ui_family()
    if fam:
        want = "'" + fam + " Medium 11'"
        cur = gs_get("font-name")
        if cur is None:
            notes.append("! gsettings isn't there, so GTK apps keep their font")
        else:
            g = state.setdefault("gtk", {})
            if "before" not in g:
                g["before"] = cur
            if cur != want and (cur == g["before"] or cur == g.get("set")):
                d.did.append("set GTK apps' font to " + fam)
                if not d.dry:
                    gs_set("font-name", want)
            g["set"] = want
    set_kde(p, state, d, kde_wanted(mode))
    if not darkly():
        notes.append("! Darkly isn't installed, so Qt apps keep their style (yay -S darkly-bin)")
    d.write(p.fontconfig, GRAYSCALE, "wrote " + home(p.fontconfig))
    notes.append("apps: " + (fam or "their own font") + ", grayscale font smoothing" + (", Darkly and Breeze Plus for Qt apps" if darkly() else ""))
    return notes


def restore_apps(p, state, d):
    g = state.get("gtk")
    if g and "before" in g:
        if gs_get("font-name") == g.get("set"):
            d.did.append("put back GTK apps' font")
            if not d.dry:
                gs_set("font-name", g["before"])
    state.pop("gtk", None)
    unset_kde(p, state, d)
    d.remove(p.fontconfig, "removed " + home(p.fontconfig))


# ── On and off ──
def apply(p, state, d):
    """Everything set up; returns notes (lines starting "! " need a look)"""
    notes = []
    shell = login_shell()
    created = set(state.get("created", []))
    have_starship = which("starship")

    mode = current_mode(p, state)
    notes += apply_apps(p, state, d, mode)
    nerd = nerd_font()

    # starship.toml
    text = starship_toml(mode, state.get("accent", ""), nerd)
    cur = read(p.starship)
    if cur is not None and cur.startswith(HEADER) and state.get("starship") and sha(cur) != state.get("starship"):
        notes.append("starship.toml: your changes to it are kept")
    else:
        if cur is not None and not cur.startswith(HEADER):
            d.backup(p.starship)
        d.write(p.starship, text, "wrote " + home(p.starship))
        state["starship"] = sha(text)

    # fish
    if which("fish") or os.path.isdir(p.fish) or shell == "fish":
        owner = fish_prompt_owner(p)
        d.write(p.fish_ours, fish_ours(owner is None), "wrote " + home(p.fish_ours))
        conf = read(p.fish_conf)
        if conf is not None and fish_needs_quiet(conf):
            add_block(d, p.fish_conf, FISH_QUIET)
        elif conf is not None and BEGIN in conf:
            remove_block(d, p.fish_conf, False)
        if owner:
            notes.append("fish: no greeting; the prompt is left to " + owner)
        else:
            notes.append("fish: no greeting, the starship prompt")

    # zsh and bash: starship, unless something else draws the prompt
    for kind, path in (("zsh", p.zshrc), ("bash", p.bashrc)):
        text = read(path)
        if text is None and shell != kind:
            continue
        owner = rc_owner(text or "", kind)
        if owner:
            if text and BEGIN in text:
                remove_block(d, path, kind + "rc" in created)
            notes.append(kind + ": " + owner + " sets your prompt, so it's left alone")
            continue
        if add_block(d, path, RC_BLOCK[kind]):
            created.add(kind + "rc")
        notes.append(kind + ": the starship prompt")

    # kitty
    if which("kitty") or os.path.isdir(p.kitty):
        d.write(p.kitty_ours, kitty_ours(nerd), "wrote " + home(p.kitty_ours))
        if add_block(d, p.kitty_conf, KITTY_BLOCK, top=True):
            created.add("kitty.conf")
        notes.append("kitty: Tessera's font, cursor and padding (your kitty.conf's own lines win)")

    notes += apply_keyring(p, d, created)

    if not have_starship:
        notes.append("! starship isn't installed: the prompt comes once it is (sudo pacman -S starship)")
    if not nerd:
        notes.append("! No Nerd Font, so the prompt has no icons (sudo pacman -S ttf-jetbrains-mono-nerd, then turn the look off and on)")
    state["created"] = sorted(created)
    return notes


def restore(p, state, d):
    """Everything as it was: Tessera's files gone, its lines taken out, a
    file it replaced back from .pre-tessera"""
    notes = []
    restore_apps(p, state, d)
    created = set(state.get("created", []))
    cur = read(p.starship)
    if cur is not None and cur.startswith(HEADER):
        if os.path.exists(p.starship + BACKUP):
            d.restore(p.starship, "put back your starship.toml")
        else:
            d.remove(p.starship, "removed Tessera's starship.toml")
    elif cur is not None:
        notes.append("starship.toml isn't Tessera's any more: left as it is")
    d.remove(p.fish_ours, "removed " + home(p.fish_ours))
    remove_block(d, p.fish_conf, False)
    remove_block(d, p.zshrc, "zshrc" in created)
    remove_block(d, p.bashrc, "bashrc" in created)
    remove_block(d, p.kitty_conf, "kitty.conf" in created)
    d.remove(p.kitty_ours, "removed " + home(p.kitty_ours))
    restore_keyring(p, d, created)
    state.pop("starship", None)
    state["created"] = []
    notes.append("Your apps and terminal are as they were (the copies kept as *" + BACKUP + " stay)")
    return notes


def status(p, state):
    cur = read(p.starship)
    return {
        "on": state.get("on", True) is not False,
        "applied": bool(state.get("starship")) or os.path.exists(p.fish_ours) or os.path.exists(p.kitty_ours) or bool(state.get("kde")),
        "migrated": bool(state.get("migrated")),
        "starship": which("starship"),
        "starshipToml": "tessera" if cur is not None and cur.startswith(HEADER) else ("yours" if cur is not None else "none"),
        "edited": bool(cur is not None and cur.startswith(HEADER) and state.get("starship") and sha(cur) != state["starship"]),
        "notes": state.get("notes", []),
    }


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("command", choices=["status", "install", "migrate", "on", "off", "refresh"])
    ap.add_argument("--dry-run", action="store_true")
    ap.add_argument("--text", action="store_true", help="lines for people, not JSON")
    ap.add_argument("--mode", choices=["dark", "light"])
    ap.add_argument("--accent")
    a = ap.parse_args(argv)
    p = Paths()
    state = load_state(p)
    d = Doer(a.dry_run)
    out = {}
    if a.mode:
        state["mode"] = a.mode
    if a.accent and hex_ok(a.accent):
        state["accent"] = a.accent
    if a.command == "status":
        out = status(p, state)
    elif a.command == "refresh":
        cur = read(p.starship)
        if state.get("on", True) is not False and cur is not None and cur.startswith(HEADER) and state.get("starship") == sha(cur):
            text = starship_toml(current_mode(p, state), state.get("accent", ""), nerd_font())
            d.write(p.starship, text, "wrote " + home(p.starship))
            state["starship"] = sha(text)
        if state.get("on", True) is not False and "Icons/Theme" in state.get("kde", {}):
            icons = breeze_plus(current_mode(p, state))
            if icons:
                set_kde(p, state, d, [("Icons", "Theme", icons)])
        out = {"did": d.did}
    elif a.command in ("install", "migrate") and (state.get("on", True) is False or (a.command == "migrate" and state.get("migrated"))):
        out = {"did": [], "notes": ["Tessera's desktop look is off (" + WHERE + ")" if state.get("on", True) is False else "Set up before"]}
        state["migrated"] = True
    elif a.command in ("install", "migrate", "on"):
        notes = apply(p, state, d)
        state["on"] = True
        state["migrated"] = True
        state["notes"] = notes
        out = {"did": d.did, "notes": notes}
    elif a.command == "off":
        notes = restore(p, state, d)
        state["on"] = False
        state["notes"] = []
        out = {"did": d.did, "notes": notes}
    if not a.dry_run and a.command != "status":
        try:
            save_state(p, state)
        except OSError as e:
            out["error"] = "Couldn't save " + p.state + ": " + str(e)
    out.setdefault("on", state.get("on", True) is not False)
    if a.text:
        for line in ([("would: " if a.dry_run else "") + x for x in out.get("did", [])] if a.dry_run else []) + out.get("notes", []):
            print(line)
        if out.get("error"):
            print("! " + out["error"])
    else:
        print(json.dumps(out))
    return 1 if out.get("error") else 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except OSError as e:
        print(json.dumps({"error": str(e)}))
        sys.exit(1)
