#!/usr/bin/env python3
"""Tessera's interactive installer: full screen, arrow keys (./install.sh
offers it, or ./install.sh --tui).

It asks everything first (look, light or dark, login screen, icon themes,
extras, what this computer's hardware is missing), shows the whole plan
(install.sh --plan with those choices), and only then runs install.sh with
them as flags (--cli --yes …), showing its steps live and a summary at the
end. install.sh does all the installing: nothing here changes the system.

  tui.py [install.sh's options]   (--dev, --no-deps, --no-snapshot,
                                   --dry-run: show the plan and stop)

For tests, without a terminal and changing nothing:
  tui.py --script "down space enter …" --no-run [--facts FILE] [--width N]
    keys from the list (up down left right space enter esc q pgup pgdn),
    each screen printed as text, then the install.sh command it would run.

Needs python-rich; without it, install.sh's own prompts take over.
"""
import importlib.util
import io
import os
import re
import select
import shlex
import shutil
import signal
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.realpath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
INSTALL = os.path.join(ROOT, "install.sh")

try:
    from rich.console import Console, Group
    from rich.live import Live
    from rich.padding import Padding
    from rich.table import Table
    from rich.text import Text
except ImportError:  # (install.sh offers to install it; this is the way out if it's gone since)
    Console = None


def load_hardware():
    spec = importlib.util.spec_from_file_location("tessera_hardware", os.path.join(HERE, "hardware.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


hardware = load_hardware()

# The Quiet Interface in a terminal: neutral, one accent for selection and
# focus, red only for what failed
ACCENT = "#6366F1"
OK, WARN, BAD = "green", "yellow", "red"

ICON_THEMES = [("colloid", "Colloid", "AUR"), ("tela-circle", "Tela circle", "AUR"),
               ("numix-circle", "Numix Circle", "AUR"), ("flat-remix", "Flat Remix", "AUR"),
               ("papirus", "Papirus", "repositories")]
ICON_PACKAGES = {"colloid": "colloid-icon-theme-git", "tela-circle": "tela-circle-icon-theme-git",
                 "numix-circle": "numix-circle-icon-theme-git", "flat-remix": "flat-remix", "papirus": "papirus-icon-theme"}
# What install.sh's own flags mean here: passed on as they are
PASS = ("--dev", "--link", "--no-deps", "--no-snapshot", "--no-extras", "--no-greeter", "--greeter", "--dry-run")


# ── Choices ─────────────────────────────────────────────────────────────
class Radio:
    def __init__(self, options, value):
        self.options = options  # [(value, label, detail)]
        self.cursor = next((i for i, o in enumerate(options) if o[0] == value), 0)
        self.value = options[self.cursor][0]

    def key(self, k):
        if k in ("up", "down"):
            self.cursor = (self.cursor + (1 if k == "down" else -1)) % len(self.options)
        if k in ("up", "down", "space", "enter"):
            self.value = self.options[self.cursor][0]


class Checks:
    def __init__(self, items):
        self.items = items  # [{id, label, detail, ticked, locked, group}]
        self.cursor = next((i for i, it in enumerate(items) if not it.get("locked")), 0)

    def key(self, k):
        if not self.items:
            return
        if k in ("up", "down"):
            step = 1 if k == "down" else -1
            for _ in range(len(self.items)):
                self.cursor = (self.cursor + step) % len(self.items)
                if not self.items[self.cursor].get("locked"):
                    break
        elif k == "space" and not self.items[self.cursor].get("locked"):
            self.items[self.cursor]["ticked"] = not self.items[self.cursor]["ticked"]

    def chosen(self):
        return [i["id"] for i in self.items if i["ticked"] and not i.get("locked")]


def apps_missing():
    groups = [("foot", "kitty", "alacritty", "wezterm", "konsole", "gnome-terminal", "ghostty"),
              ("thunar", "nautilus", "dolphin", "nemo", "pcmanfm"),
              ("firefox", "chromium", "google-chrome-stable", "brave", "zen-browser", "librewolf")]
    return any(not any(shutil.which(c) for c in g) for g in groups)


class Installer:
    def __init__(self, facts, passthrough, system=None):
        self.facts = facts
        self.passthrough = [a for a in passthrough if a in PASS]
        self.dry = "--dry-run" in passthrough
        have = set(facts.get("installed", ()))
        sysinfo = system or {}
        dm = facts.get("display_manager", "")
        self.look = Radio([("glass", "Glass", "Translucent glass, like macOS. App icons start Clear."),
                           ("material", "Material", "Android's Material You. App icons start Themed.")], "glass")
        self.mode = Radio([("auto", "Automatic", "Light by day, dark at night"),
                           ("light", "Light", ""), ("dark", "Dark", "")], "auto")
        keep = ("keep", "Keep " + dm, "Your login screen stays; pick Hyprland there") if dm else \
            ("keep", "No login screen", "Hyprland starts when you log in on the first console (tty1)")
        self.login = Radio([("look", "Tessera, following the look", "Glass or Material, like the lock screen"),
                            ("text", "Tessera, text", "A clock and a password field on your wallpaper"),
                            ("plain", "Tessera, plain", "The smallest: a name and a password")]
                           + [keep], "keep" if dm else "look")
        if dm:
            # (another login screen is only replaced when you pick a Tessera one;
            # it's turned off, not removed, and tessera uninstall turns it back on)
            self.login.options[:3] = [(v, l, d + "; " + dm + " is turned off") for v, l, d in self.login.options[:3]]
        self.icons = Checks([{"id": i, "label": label, "ticked": False,
                              "detail": "installed" if ICON_PACKAGES[i] in have else "from the " + src,
                              "locked": ICON_PACKAGES[i] in have} for i, label, src in ICON_THEMES])
        extras = []
        if sysinfo.get("apps_missing", apps_missing()):
            extras.append({"id": "apps", "label": "Basic apps", "detail": "kitty, Files, Firefox, mpv, an image viewer", "ticked": True})
        if "bibata-cursor-theme-bin" not in have:
            extras.append({"id": "pointer", "label": "Bibata pointer", "detail": "the pointer in both looks (from the AUR)", "ticked": True})
        if not {"ttf-apple-emoji", "ttf-fluent-emoji"} <= have:
            extras.append({"id": "emoji", "label": "Apple and Fluent emoji", "detail": "Noto emoji is the default (from the AUR)", "ticked": False})
        self.extras = Checks(extras)
        system_items = []
        if facts.get("pulseaudio"):
            system_items.append({"id": "pulse", "label": "Replace PulseAudio with PipeWire", "ticked": True,
                                 "detail": "Tessera's sound needs PipeWire, which can't be installed next to it. Unticked, the install stops before packages"})
        if facts.get("network"):
            system_items.append({"id": "network", "label": "Switch the network from {} to NetworkManager".format(facts["network"]),
                                 "ticked": True, "detail": "Tessera's Wi-Fi menus use it; takes effect after a reboot. Unticked, " + facts["network"] + " stays"})
        self.system = Checks(system_items)
        items = hardware.plan(facts)
        self.hw_groups = hardware.GROUPS
        self.hardware = Checks([{"id": i["id"], "label": i["title"], "detail": i["why"], "ticked": i["ticked"] or i["locked"],
                                 "locked": i["locked"], "group": i["group"], "ask": i["ask"],
                                 "packages": i["packages"] + i["aur"]} for i in items])
        no_extras = "--no-extras" in passthrough
        self.pages = [p for p in [
            ("welcome", "Welcome"), ("look", "Look"), ("mode", "Light or dark"), ("login", "Login screen"),
            None if no_extras else ("icons", "Icon themes"), None if no_extras or not extras else ("extras", "Extras"),
            ("system", "Your system") if system_items else None, ("hardware", "This computer"),
            ("review", "Review"), ("install", "Install")] if p]
        self.page = 0
        self.plan_text = ""
        self.scroll = 0
        # Installing
        self.steps = []  # [{title, state: run|ok|failed|skipped, lines}]
        self.checks = []  # verify's lines: (mark, text)
        self.warnings = []
        self.result = ""
        self.code = None
        self.started = 0.0

    # The install.sh options for these choices
    def args(self):
        a = ["--cli", "--yes", "--look=" + self.look.value, "--mode=" + self.mode.value, "--login=" + self.login.value]
        if "--no-extras" not in self.passthrough:
            a += ["--icons=" + (",".join(self.icons.chosen()) or "none"),
                  "--extras=" + (",".join(self.extras.chosen()) or "none")]
        a.append("--hardware=" + (",".join(self.hardware.chosen()) or "none"))
        sysc = {i["id"]: i["ticked"] for i in self.system.items}
        if sysc.get("pulse"):
            a.append("--replace-pulseaudio")
        if "network" in sysc:
            a.append("--network=" + ("networkmanager" if sysc["network"] else "keep"))
        return a + [p for p in self.passthrough if p != "--dry-run"]

    def command(self):
        return [INSTALL] + self.args()

    @property
    def name(self):
        return self.pages[self.page][0]

    # ── Keys ──
    def key(self, k):
        """One key; returns "quit", "install" or None."""
        name = self.name
        if k == "q" and name not in ("install",):
            return "quit"
        if k in ("left", "esc", "backspace"):
            if self.page > 0 and name != "install":
                self.page -= 1
                self.scroll = 0
            return None
        if name == "review":
            if k in ("up", "down", "pgup", "pgdn"):
                d = {"up": -1, "down": 1, "pgup": -10, "pgdn": 10}[k]
                self.scroll = max(0, min(self.scroll + d, max(0, len(self.plan_text.splitlines()) - 5)))
            elif k == "enter":
                return "install"
            return None
        widget = self.widget()
        if widget is not None:
            widget.key(k)
        if k in ("enter", "right") and self.page < len(self.pages) - 1:
            self.page += 1
            self.scroll = 0
            if self.name == "review":
                return "review"
        return None

    def widget(self):
        return {"look": self.look, "mode": self.mode, "login": self.login, "icons": self.icons,
                "extras": self.extras, "system": self.system, "hardware": self.hardware}.get(self.name)

    # ── Installing: install.sh's output, line by line ──
    STEP = re.compile(r"^\[(\d+)/(\d+)\] (.+)$")

    def feed(self, line):
        line = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", line).rstrip()
        s = line.strip()
        m = self.STEP.match(s) or re.match(r"^\[✓\] (.+)$", s)
        if m:
            if self.steps and self.steps[-1]["state"] == "run":
                self.steps[-1]["state"] = "ok"
            self.steps.append({"title": m.groups()[-1], "state": "run", "lines": []})
            return
        if self.steps:
            self.steps[-1]["lines"].append(line)
        if re.match(r"^✗ .+ didn't finish", s) and self.steps:
            self.steps[-1]["state"] = "failed"
        elif s.startswith("! Skipped") and self.steps:
            self.steps[-1]["state"] = "skipped"
        elif self.steps and self.steps[-1]["title"] == "Checking the result" and s[:2] in ("✓ ", "! ", "✗ "):
            self.checks.append((s[0], s[2:]))
        elif s.startswith("! "):
            self.warnings.append(s[2:])
        elif s.startswith("Tessera is ready") or s.startswith("Tessera is installed"):
            self.result = s

    def finish(self, code):
        self.code = code
        if self.steps and self.steps[-1]["state"] == "run":
            self.steps[-1]["state"] = "ok" if code == 0 else "failed"

    # ── Drawing ──
    def render(self, width=100, height=40, spin=0):
        side = width >= 76
        body = self.body(height, spin)
        head = Text.assemble(("Tessera setup", "bold"), ("   " + self.pages[self.page][1], "dim") if not side else "")
        if side:
            nav = Table.grid(padding=(0, 1))
            for i, (_, title) in enumerate(self.pages):
                if i == self.page:
                    nav.add_row(Text("›", style=ACCENT + " bold"), Text(title, style="bold"))
                elif i < self.page:
                    nav.add_row(Text("✓", style="dim"), Text(title, style="dim"))
                else:
                    nav.add_row(Text(" "), Text(title, style="dim"))
            grid = Table.grid(padding=(0, 3))
            grid.add_column(width=20)
            grid.add_column(ratio=1)
            grid.add_row(nav, body)
            body = grid
        return Group(head, Text(""), body, Text(""), Text(self.hints(), style="dim"))

    def hints(self):
        n = self.name
        if n == "install":
            return "Installing… Ctrl+C stops it (finished steps are quick the next time)" if self.code is None else "enter finish"
        if n == "review":
            return "↑↓ scroll   enter " + ("finish (dry run: nothing changes)" if self.dry else "install") + "   ← back   q quit"
        if isinstance(self.widget(), Checks):
            return "↑↓ move   space tick   enter continue   ← back   q quit"
        if isinstance(self.widget(), Radio):
            return "↑↓ choose   enter continue   ← back   q quit"
        return "enter begin   q quit"

    def body(self, height, spin):
        n = self.name
        if n == "welcome":
            return self.welcome()
        if n == "review":
            return self.review(height)
        if n == "install":
            return self.installing(height, spin)
        titles = {"look": ("Choose a look", "Settings → Appearance changes it any time."),
                  "mode": ("Light or dark", ""),
                  "login": ("The login screen", "Only what you pick changes; the current one is never removed."),
                  "icons": ("Icon themes (optional)", "Without one, Tessera styles the apps' own icons. Tick any you'd like as well."),
                  "extras": ("Extras (optional)", ""),
                  "system": ("Your system", "These replace or turn off something of yours, so they're up to you."),
                  "hardware": ("This computer", "Only what's missing for the hardware found here. Asked ones start unticked.")}
        title, sub = titles[n]
        out = [Text(title, style="bold")]
        if sub:
            out.append(Text(sub, style="dim"))
        out.append(Text(""))
        w = self.widget()
        if isinstance(w, Radio):
            for i, (value, label, detail) in enumerate(w.options):
                on = i == w.cursor
                out.append(Text.assemble(("› " if on else "  ", ACCENT + " bold"), ("● " if value == w.value else "○ ", ACCENT if value == w.value else "dim"),
                                         (label, "bold" if on else "")))
                if detail:
                    out.append(Padding(Text(detail, style="dim"), (0, 0, 0, 6)))
        else:
            group = None
            if not w.items:
                out.append(Text("Nothing to add here: it's all set.", style="dim"))
            for i, it in enumerate(w.items):
                if it.get("group") and it["group"] != group:
                    group = it["group"]
                    out.append(Text(("" if i == 0 else "\n") + group, style="bold dim"))
                on = i == w.cursor
                box = "[•]" if it.get("locked") else "[✓]" if it["ticked"] else "[ ]"
                out.append(Text.assemble(("› " if on else "  ", ACCENT + " bold"),
                                         (box + " ", "dim" if it.get("locked") else ACCENT if it["ticked"] else ""),
                                         (it["label"], "bold" if on else "dim" if it.get("locked") else ""),
                                         ("   always" if it.get("locked") and it.get("group") else "", "dim")))
                # (what's always done says why only when you're on it)
                if it.get("detail") and (on or not it.get("locked") or not it.get("group")):
                    out.append(Padding(Text(it["detail"], style="dim"), (0, 0, 0, 8)))
                if on and it.get("packages"):
                    out.append(Padding(Text("Packages: " + " ".join(it["packages"]), style="dim"), (0, 0, 0, 8)))
        return Group(*out)

    def welcome(self):
        f = self.facts
        gpus = [d["name"] for d in f.get("pci", []) if d["cls"] in ("0300", "0302", "0380")]
        rows = Table.grid(padding=(0, 2))
        rows.add_row(Text("Computer", style="dim"), Text(("Laptop" if f.get("laptop") else "Desktop")
                                                         + ({"intel": ", Intel processor", "amd": ", AMD processor"}.get(f.get("cpu"), ""))))
        if gpus:
            rows.add_row(Text("Graphics", style="dim"), Text("; ".join(gpus)))
        for s in f.get("screens", []):
            rows.add_row(Text("Screen", style="dim"), Text("{} {}×{}, {:.1f}\"".format(s["name"], s["width"], s["height"], s["inches"])))
        if f.get("fingerprint"):
            rows.add_row(Text("Fingerprint", style="dim"), Text(f["fingerprint"]["name"]))
        if f.get("ir_camera"):
            rows.add_row(Text("Face", style="dim"), Text(f["ir_camera"]["name"]))
        if f.get("hyprland"):
            rows.add_row(Text("Hyprland", style="dim"), Text(f["hyprland"]))
        return Group(Text("Set up Tessera", style="bold"),
                     Text("A few choices, then the whole plan. Nothing changes until you choose Install.", style="dim"),
                     Text(""), rows, Text(""),
                     Text("Your Hyprland config and anything else replaced is backed up first; "
                          "a snapper or Timeshift snapshot is taken when one is set up.", style="dim"))

    def review(self, height):
        lines = self.plan_text.splitlines() or ["(the plan)"]
        room = max(5, height - 10)
        shown = lines[self.scroll:self.scroll + room]
        out = [Text("Review the plan", style="bold"),
               Text("Everything that will be installed and changed. Nothing has changed yet.", style="dim"), Text("")]
        out += [Text(line) for line in shown]
        if self.scroll + room < len(lines):
            out.append(Text("  … {} more (↓)".format(len(lines) - self.scroll - room), style="dim"))
        out += [Text(""), Text.assemble(("  " + ("Finish" if self.dry else "Install Tessera") + "  ", "bold white on " + ACCENT),
                                        ("   enter", "dim"))]
        return Group(*out)

    def installing(self, height, spin):
        frames = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"
        marks = {"ok": ("✓", OK), "failed": ("✗", BAD), "skipped": ("–", "dim"), "run": (frames[spin % len(frames)], ACCENT)}
        t = Table.grid(padding=(0, 1))
        for st in self.steps:
            m, style = marks[st["state"]]
            extra = {"failed": "  didn't finish", "skipped": "  skipped"}.get(st["state"], "")
            t.add_row(Text(m, style=style), Text(st["title"] + extra, style="bold" if st["state"] == "run" else ""))
        out = [Text("Installing" if self.code is None else self.result or ("Finished" if self.code == 0 else "Stopped"), style="bold"),
               Text("Elapsed {}:{:02d}".format(*divmod(int(time.monotonic() - self.started), 60)) if self.started else "", style="dim"),
               Text(""), t]
        if self.steps and self.code is None:
            tail = [x for x in self.steps[-1]["lines"] if x.strip()][-max(3, height - len(self.steps) - 12):]
            out += [Text("")] + [Text(x[:200], style="dim") for x in tail]
        return Group(*out)

    def summary(self, log):
        """What happened, for the scrollback after the full screen closes."""
        out = [Text(self.result or ("Tessera is installed." if self.code == 0 else "The install stopped."), style="bold"), Text("")]
        t = Table.grid(padding=(0, 1))
        for st in self.steps:
            m, style = {"ok": ("✓", OK), "failed": ("✗", BAD), "skipped": ("–", "dim"), "run": ("…", "dim")}[st["state"]]
            t.add_row(Text(m, style=style), Text(st["title"] + {"failed": ": didn't finish", "skipped": ": skipped"}.get(st["state"], "")))
        out.append(t)
        bad = [c for c in self.checks if c[0] != "✓"]
        if bad:
            out += [Text(""), Text("To look at", style="bold")]
            out += [Text.assemble(("  " + m + " ", BAD if m == "✗" else WARN), (text,)) for m, text in bad]
        if self.warnings:
            out += [Text(""), Text("Notes", style="bold")]
            out += [Text("  ! " + w, style="dim") for w in self.warnings[-8:]]
        out += [Text(""), Text("Restart to begin. Super+/ lists the shortcuts; Super+, opens Settings.", style="dim"),
                Text("The tessera command: tessera doctor, tessera update, tessera rebuild, tessera repair, tessera uninstall.", style="dim"),
                Text("Log: " + log, style="dim")]
        return Group(*out)


# ── The terminal ────────────────────────────────────────────────────────
KEYS = {b"\x1b[A": "up", b"\x1b[B": "down", b"\x1b[C": "right", b"\x1b[D": "left", b"\x1bOA": "up", b"\x1bOB": "down",
        b"\x1bOC": "right", b"\x1bOD": "left", b"\x1b[5~": "pgup", b"\x1b[6~": "pgdn", b"\x1b": "esc", b"\n": "enter",
        b"\r": "enter", b" ": "space", b"\x7f": "backspace", b"\x08": "backspace", b"k": "up", b"j": "down",
        b"h": "left", b"l": "right", b"q": "q", b"Q": "q", b"\t": "down"}


class Terminal:
    def __init__(self):
        import termios
        import tty
        self.termios = termios
        self.fd = sys.stdin.fileno()
        self.tty = tty
        self.old = termios.tcgetattr(self.fd)
        self.cbreak()

    def cbreak(self):
        self.tty.setcbreak(self.fd)  # (Ctrl+C still interrupts)

    def key(self, timeout=None):
        r, _, _ = select.select([self.fd], [], [], timeout)
        if not r:
            return None
        b = os.read(self.fd, 1)
        if b == b"\x1b":
            while select.select([self.fd], [], [], 0.03)[0]:
                b += os.read(self.fd, 1)
                if len(b) >= 4 or b[-1:].isalpha() or b[-1:] == b"~":
                    break
        return KEYS.get(b)

    def restore(self):
        self.termios.tcsetattr(self.fd, self.termios.TCSADRAIN, self.old)


def plan_text(ins, width):
    """install.sh's own plan for these choices (it only reads), as wide as
    the review has room for."""
    try:
        p = subprocess.run([INSTALL] + ins.args() + ["--plan"], capture_output=True, text=True, timeout=120,
                           stdin=subprocess.DEVNULL, env=dict(os.environ, COLUMNS=str(max(60, width - (25 if width >= 76 else 0)))))
    except (OSError, subprocess.SubprocessError) as e:
        return "The plan couldn't be made: {}".format(e)
    text = re.sub(r"\x1b\[[0-9;]*[A-Za-z]", "", p.stdout + p.stderr)
    start = text.find("The plan")
    return (text[start:] if start >= 0 else text).rstrip() or "install.sh printed no plan (exit {})".format(p.returncode)


def run_install(ins, console, term):
    """install.sh with the choices, its steps live; returns its exit code."""
    term.restore()
    console.print(Text("Your password, for the system changes (sudo):", style="bold"))
    if subprocess.run(["sudo", "-v"]).returncode != 0:
        console.print(Text("sudo didn't accept it: nothing was changed.", style=BAD))
        return 1
    term.cbreak()
    ins.started = time.monotonic()
    proc = subprocess.Popen(ins.command(), stdout=subprocess.PIPE, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL, bufsize=0)
    buf = b""
    spin = 0
    with Live(ins.render(console.width, console.height), console=console, screen=True, auto_refresh=False) as live:
        fd = proc.stdout.fileno()
        while True:
            r, _, _ = select.select([fd], [], [], 0.1)
            if r:
                chunk = os.read(fd, 65536)
                if not chunk:
                    break
                buf += chunk
                *lines, buf = re.split(rb"\r?\n|\r", buf)
                for line in lines:
                    ins.feed(line.decode("utf-8", "replace"))
            spin += 1
            live.update(ins.render(console.width, console.height, spin), refresh=True)
        if buf:
            ins.feed(buf.decode("utf-8", "replace"))
        ins.finish(proc.wait())
        live.update(ins.render(console.width, console.height), refresh=True)
        while term.key() not in ("enter", "q", "esc"):
            pass
    return ins.code


def interactive(ins):
    console = Console()
    term = Terminal()
    code = 0
    try:
        with Live(ins.render(console.width, console.height), console=console, screen=True, auto_refresh=False) as live:
            while True:
                k = term.key()
                if k is None:
                    continue
                act = ins.key(k)
                if act == "review":
                    ins.plan_text = "Making the plan…"
                    live.update(ins.render(console.width, console.height), refresh=True)
                    ins.plan_text = plan_text(ins, console.width)
                if act in ("quit", "install"):
                    break
                live.update(ins.render(console.width, console.height), refresh=True)
        if act == "quit":
            console.print("Nothing changed.")
            return 0
        if ins.dry:
            console.print(Text("Dry run: nothing changed. The plan for your choices:", style="bold"))
            console.print(ins.plan_text, markup=False, highlight=False)
            console.print(Text("To install with these choices: " + shlex.join(ins.command()), style="dim"))
            return 0
        ins.page = len(ins.pages) - 1
        try:
            code = run_install(ins, console, term)
        except KeyboardInterrupt:
            ins.finish(130)
            code = 130
        log = os.path.join(os.environ.get("XDG_CACHE_HOME") or os.path.expanduser("~/.cache"), "tessera", "install.log")
        console.print(ins.summary(log))
        return code
    except KeyboardInterrupt:
        console.print("Stopped. Nothing changed." if not ins.steps else "Stopped.")
        return 130
    finally:
        term.restore()


def scripted(ins, keys, width):
    """Each screen as text for the given keys; then the command (tests)."""
    console = Console(file=io.StringIO(), width=width, height=40, force_terminal=False, color_system=None)
    out = sys.stdout

    def frame():
        console.print(ins.render(width, 40))
        out.write(console.file.getvalue() + "\f\n")
        console.file.seek(0)
        console.file.truncate()

    frame()
    for k in keys:
        act = ins.key(k)
        if act == "review":
            ins.plan_text = "(install.sh --plan, not run in a scripted test)"
        if act in ("quit", "install"):
            out.write("ACTION: " + act + "\n")
            break
        frame()
    out.write("COMMAND: " + shlex.join(ins.command()) + "\n")
    return 0


def main(argv):
    keys = None
    facts_file = None
    width = 100
    run = True
    rest = []
    it = iter(argv)
    for a in it:
        if a == "--script":
            keys = next(it, "").replace(",", " ").split()
        elif a == "--facts":
            facts_file = next(it, None)
        elif a == "--width":
            width = int(next(it, "100"))
        elif a == "--no-run":
            run = False
        elif a not in ("--tui", "--yes", "-y"):
            rest.append(a)
    if Console is None:
        print("The interactive installer needs python-rich (sudo pacman -S python-rich). Using the classic prompts.")
        if keys is not None or not run:
            return 1
        os.execv(INSTALL, [INSTALL, "--cli"] + rest)
    if facts_file:
        import json
        with open(facts_file) as f:
            facts = json.load(f)
        system = {"apps_missing": facts.get("apps_missing", False), "walls_ready": facts.get("walls_ready", True)}
    else:
        facts, system = hardware.collect(), None
    ins = Installer(facts, rest, system)
    if keys is not None:
        return scripted(ins, keys, width)
    if not run or not (sys.stdin.isatty() and sys.stdout.isatty()):
        print("The interactive installer needs a terminal; ./install.sh --cli asks its questions as text.")
        return 1
    signal.signal(signal.SIGTSTP, signal.SIG_IGN)
    return interactive(ins)


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
