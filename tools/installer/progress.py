#!/usr/bin/env python3
"""The classic installer's screen (install.sh in a terminal): the steps with
ticks (✓ done, a spinner for the one running, ✗ failed) and one progress bar,
while everything the commands print (pacman, makepkg, cargo, git…) goes to
the install log instead of the terminal. d shows the log's end, following it
(↑ ↓ PgUp PgDn scroll back, End follows again), q or Esc goes back. A step
that fails says what happened, how to fix it and where the log is, on the
steps themselves.

  progress.py mode [--plain] [--stdout-tty 0|1]
      "ui" or "plain": plain with --plain, when the output isn't a terminal,
      for TERM=dumb (or none), without /dev/tty, or on a terminal too small
  progress.py serve --log FILE [--ack FIFO]
      the screen: install.sh's output on stdin, the screen on stdout, keys
      from /dev/tty

What install.sh writes on the stream, besides the commands' own output: a
line "\\x1eUI\\t<what>\\t<fields…>" (tab-separated):
  plan TITLE WEIGHT TITLE WEIGHT …   the steps, and how much of the bar each is
  step N                             step N (from 1) starts (again, on a retry)
  msg say|note|warn TEXT             the installer's own words (into the log too)
  done N | skip N | fail N [FIX]     how step N went; FIX: what to do now
  error TEXT                         the install stops: why
  pause TEXT                         the user is needed (a question, sudo's
                                     password): the terminal is handed back,
                                     "ok" written to the ack FIFO
  resume                             the screen again
  quit                               the last frame, the terminal back, "ok"

Builds (tools/build-progress.py with TESSERA_PROGRESS=lines) add "BUILD
{json}" lines and pacman its "( 3/90) installing foo" ones: they move the
bar within their step.
"""
import argparse
import codecs
import collections
import json
import os
import re
import select
import signal
import sys
import time
import unicodedata

MARK = b"\x1eUI\t"
ANSI = re.compile(r"\x1b(\[[0-9;?<=>]*[ -/]*[@-~]|\][^\x07\x1b]*(\x07|\x1b\\)?|[()][0-9A-Za-z]|[=>78DEHMc])")
PACMAN = re.compile(r"^\(\s*(\d+)/(\d+)\) (installing|upgrading|reinstalling|removing|downgrading|checking keys|checking package integrity"
                    r"|loading package files|checking for file conflicts|checking available disk space|downloading)\s*(\S*)")
TAIL = 4000  # log lines kept for the details view

UTF8 = {"ok": "✓", "fail": "✗", "skip": "–", "wait": "·", "warn": "!", "you": "›",
        "spin": "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏", "full": "━", "empty": "─", "dot": "·", "more": "…", "up": "↑", "down": "↓"}
ASCII = {"ok": "+", "fail": "x", "skip": "-", "wait": ".", "warn": "!", "you": ">",
         "spin": "|/-\\", "full": "#", "empty": "-", "dot": "-", "more": "~", "up": "Up", "down": "Down"}


def choose_mode(plain, stdout_tty, term, tty_ok, cols, rows):
    """ui or plain (and why, for the tests)"""
    if plain:
        return "plain", "--plain"
    if not stdout_tty:
        return "plain", "the output isn't a terminal"
    if not term or term == "dumb":
        return "plain", "TERM=" + (term or "(none)")
    if not tty_ok:
        return "plain", "no /dev/tty"
    if cols < 40 or rows < 12:
        return "plain", "the terminal is too small"
    return "ui", ""


def glyphs_for(term, env):
    """Box and braille characters need a UTF-8 terminal with a font that has
    them: the Linux console's usually doesn't"""
    loc = env.get("LC_ALL") or env.get("LC_CTYPE") or env.get("LANG") or ""
    if term == "linux" or ("UTF-8" not in loc.upper() and "UTF8" not in loc.upper()):
        return ASCII
    return UTF8


def cwidth(s):
    """Columns a string takes (wide characters count two)"""
    return sum(0 if unicodedata.combining(c) else 2 if unicodedata.east_asian_width(c) in "WF" else 1 for c in s)


def fit(s, w, more="…"):
    """At most w columns, cut with … when it's longer"""
    if cwidth(s) <= w:
        return s
    out, n = "", 0
    for c in s:
        cw = cwidth(c)
        if n + cw > w - cwidth(more):
            break
        out += c
        n += cw
    return out + more


def wrap(s, w):
    """Words to lines of at most w columns (a long word is cut)"""
    if w < 8:
        w = 8
    lines, cur = [], ""
    for word in s.split(" "):
        while cwidth(word) > w:
            if cur:
                lines.append(cur)
                cur = ""
            head = fit(word, w, "")
            lines.append(head)
            word = word[len(head):]
        if not cur:
            cur = word
        elif cwidth(cur) + 1 + cwidth(word) <= w:
            cur += " " + word
        else:
            lines.append(cur)
            cur = word
    if cur or not lines:
        lines.append(cur)
    return lines


def clean(s):
    """A line as text: no colours or cursor moves, no control characters"""
    s = ANSI.sub("", s)
    return "".join(c if c >= " " or c == "\t" else "" for c in s).replace("\t", "    ")


def mmss(s):
    s = int(max(0, s))
    return "%d:%02d:%02d" % (s // 3600, s // 60 % 60, s % 60) if s >= 3600 else "%d:%02d" % (s // 60, s % 60)


class Model:
    """What the screen shows: the steps, the log's end, the progress"""

    def __init__(self, log_path, log=None, now=time.monotonic):
        self.log_path = log_path
        self.log = log  # (binary, append)
        self.now = now
        self.steps = []  # {title, weight, state: wait|run|ok|skip|fail, msgs, warns, fix, t0, t1, raw0}
        self.cur = -1
        self.tail = collections.deque(maxlen=TAIL)  # (text, raw?) of the log
        self.raw_count = 0  # raw lines so far (the commands' own output)
        self.raw = collections.deque(maxlen=200)  # (index, text) of those
        self.partial = ""
        self.decoder = codecs.getincrementaldecoder("utf-8")("replace")
        self.buf = b""
        self.error = ""
        self.build = None
        self.pacman = None
        self.best = 0.0
        self.t0 = now()
        self.waiting = ""  # (paused: what for)
        self.finished = False  # quit came
        self.stopped = False  # the stream ended without a quit
        self.changed = True

    # ── The stream ──
    def feed(self, data):
        """install.sh's output: returns what the screen has to do (pause,
        resume, quit) in order"""
        self.buf += data
        acts = []
        while self.buf:
            i = self.buf.find(MARK)
            if i < 0:
                # (the start of a mark may still be coming)
                keep = 0
                for k in range(len(MARK) - 1, 0, -1):
                    if self.buf.endswith(MARK[:k]):
                        keep = k
                        break
                if len(self.buf) > keep:
                    self.text(self.buf[:len(self.buf) - keep])
                self.buf = self.buf[len(self.buf) - keep:]
                break
            if i > 0:
                self.text(self.buf[:i])
            end = self.buf.find(b"\n", i)
            if end < 0:
                self.buf = self.buf[i:]
                break
            line = self.buf[i + len(MARK):end].decode("utf-8", "replace")
            self.buf = self.buf[end + 1:]
            # (the installer's own line ends one the commands left open)
            if self.partial:
                self.line(self.partial)
                self.partial = ""
            a = self.control(line.split("\t"))
            if a:
                acts.append(a)
        return acts

    def write_log(self, b):
        if self.log:
            try:
                self.log.write(b)
                self.log.flush()
            except OSError:
                pass

    def text(self, b):
        """The commands' own output: into the log as it is, and the lines
        kept for the details view (a carriage return starts the line again,
        as a terminal would show it)"""
        self.write_log(b)
        s = self.decoder.decode(b)
        parts = s.split("\n")
        for p in parts[:-1]:
            self.line(self.partial + p)
            self.partial = ""
        self.partial += parts[-1]
        if "\r" in self.partial:
            self.partial = self.partial.rstrip("\r").rsplit("\r", 1)[-1]

    def line(self, s):
        s = s.rstrip("\r")
        if "\r" in s:
            s = s.rsplit("\r", 1)[-1]
        t = clean(s).rstrip()
        st = t.strip()
        if st.startswith("BUILD {"):
            try:
                self.build = json.loads(st[6:])
                self.changed = True
            except ValueError:
                pass
            return
        p = PACMAN.match(st)
        if p:
            self.pacman = {"n": int(p[1]), "of": int(p[2]), "what": p[3], "name": p[4]}
        self.tail.append(t)
        if st:
            self.raw_count += 1
            self.raw.append((self.raw_count, t))
        self.changed = True

    def step(self, n):
        i = n - 1
        while len(self.steps) <= i:
            self.steps.append({"title": "Step %d" % (len(self.steps) + 1), "weight": 1.0, "state": "wait", "msgs": [], "warns": [], "fix": "", "t0": None, "t1": None, "raw0": 0})
        return self.steps[i]

    def control(self, f):
        what, args = f[0], f[1:]
        self.changed = True
        if what == "plan":
            self.steps = []
            for k in range(0, len(args) - 1, 2):
                try:
                    w = float(args[k + 1])
                except ValueError:
                    w = 1.0
                self.step(k // 2 + 1).update(title=args[k], weight=max(0.0, w))
        elif what == "step" and args:
            n = self.num(args[0])
            st = self.step(n)
            st.update(state="run", msgs=[], warns=[], fix="", t0=self.now(), t1=None, raw0=self.raw_count + 1)
            self.cur = n - 1
            self.build = self.pacman = None
            self.error = ""
        elif what in ("done", "skip", "fail") and args:
            st = self.step(self.num(args[0]))
            st["state"] = {"done": "ok", "skip": "skip", "fail": "fail"}[what]
            st["t1"] = self.now()
            if what == "fail":
                st["fix"] = args[1] if len(args) > 1 else ""
            if what != "fail":
                self.build = self.pacman = None
        elif what == "msg" and len(args) >= 2:
            kind, text = args[0], clean("\t".join(args[1:])).strip()
            self.write_log(("  " + ("! " if kind == "warn" else "") + text + "\n").encode())
            self.tail.append("  " + ("! " if kind == "warn" else "") + text)
            if 0 <= self.cur < len(self.steps):
                st = self.steps[self.cur]
                (st["warns"] if kind == "warn" else st["msgs"]).append((kind, text))
        elif what == "error":
            self.error = clean("\t".join(args)).strip()
            self.write_log(("✗ " + self.error + "\n").encode())
        elif what == "pause":
            self.waiting = clean("\t".join(args)).strip() or "Your answer"
            return "pause"
        elif what == "resume":
            self.waiting = ""
            return "resume"
        elif what == "quit":
            self.finished = True
            return "quit"
        return None

    @staticmethod
    def num(s):
        try:
            return max(1, int(s))
        except ValueError:
            return 1

    def end(self):
        """The stream ended: the last partial line, and a step still running
        stopped with it (Ctrl+C, the installer killed)"""
        rest = self.decoder.decode(b"", final=True)
        if self.buf:
            self.write_log(self.buf)
            rest = self.buf.decode("utf-8", "replace") + rest
            self.buf = b""
        if self.partial or rest:
            self.line(self.partial + rest)
            self.partial = ""
        if not self.finished:
            self.stopped = True
            for st in self.steps:
                if st["state"] == "run":
                    st["state"] = "fail"
                    st["t1"] = self.now()

    # ── What it shows ──
    def sub(self):
        """How far the step running is, 0…1, when something says (a build's
        count, pacman's)"""
        b = self.build
        if b and b.get("phase") not in ("done", "failed") and isinstance(b.get("fraction"), (int, float)):
            return max(0.0, min(1.0, float(b["fraction"])))
        p = self.pacman
        if p and p["of"]:
            f = p["n"] / p["of"]
            return 0.3 * f if p["what"] == "downloading" else 0.3 + 0.7 * f if p["what"] in ("installing", "upgrading", "reinstalling", "downgrading") else 0.3 * f
        return 0.0

    def fraction(self):
        total = sum(s["weight"] for s in self.steps) or 1.0
        done = sum(s["weight"] for s in self.steps if s["state"] in ("ok", "skip"))
        cur = self.steps[self.cur] if 0 <= self.cur < len(self.steps) else None
        if cur and cur["state"] == "run":
            done += cur["weight"] * self.sub()
        self.best = max(self.best, min(1.0, done / total))
        return self.best

    def activity(self):
        """A line on what the running step is doing now"""
        b = self.build
        if b and b.get("step") and b.get("phase") not in ("done", "failed"):
            return str(b["step"])
        p = self.pacman
        if p:
            return "%s %s (%d of %d)" % (p["what"].capitalize(), p["name"], p["n"], p["of"]) if p["name"] else "%s (%d of %d)" % (p["what"].capitalize(), p["n"], p["of"])
        for t in reversed(self.tail):
            if t.strip():
                return t.strip()
        return ""

    def failed_lines(self, st):
        """The commands' last lines while a failed step ran"""
        return [t for i, t in self.raw if i >= st["raw0"]][-6:]


def home(path):
    h = os.path.expanduser("~")
    return "~" + path[len(h):] if h and h != "/" and path.startswith(h + "/") else path


class Painter:
    """Lines of (text, style) pieces, cut to the width, coloured or not"""
    STYLES = {"": "", "b": "1", "dim": "2", "ok": "32", "bad": "31", "warn": "33", "acc": "34", "accb": "1;34", "badb": "1;31"}

    def __init__(self, width, color):
        self.width = max(20, width)
        self.color = color

    def __call__(self, *pieces):
        room = self.width - 1
        out = ""
        for text, style in pieces:
            if room <= 0:
                break
            if cwidth(text) > room:
                text = fit(text, room)
            room -= cwidth(text)
            code = self.STYLES.get(style, "")
            out += "\x1b[%sm%s\x1b[0m" % (code, text) if self.color and code and text else text
        return out


def render(m, width, height, spin=0, g=UTF8, color=True):
    """The steps view: a list of lines (no newlines), at most height - 1"""
    P = Painter(width, color)
    w = P.width - 1
    now = m.now()
    if m.stopped:
        head = "Stopped"
    elif m.error:
        head = "Stopped"
    elif m.finished:
        head = "Install finished" if not any(s["state"] == "fail" for s in m.steps) else "Install finished, with problems"
    elif m.waiting:
        head = "Waiting for you"
    else:
        head = "Installing"
    top = [P(("Tessera", "b"), ("  " + head, "dim")), ""]

    # Each step, and what goes under it
    blocks = []
    for i, st in enumerate(m.steps):
        state = st["state"]
        took = ""
        if st["t0"] is not None and state in ("run", "ok", "fail"):
            took = mmss((st["t1"] if st["t1"] is not None else now) - st["t0"])
        if state == "run":
            mark = (g["you"], "accb") if m.waiting else (g["spin"][spin % len(g["spin"])], "acc")
        else:
            mark = {"ok": (g["ok"], "ok"), "fail": (g["fail"], "badb"), "skip": (g["skip"], "dim"), "wait": (g["wait"], "dim")}[state]
        style = "b" if state in ("run", "fail") else "dim" if state in ("wait", "skip") else ""
        title = st["title"] + (" (skipped)" if state == "skip" else "")
        gap = w - 4 - cwidth(title) - cwidth(took)
        if took and gap >= 2:
            line = P(("  ", ""), mark, (" " + title, style), (" " * gap + took, "dim"))
        else:
            line = P(("  ", ""), mark, (" " + title, style))
        lines = [line]
        if state == "run" and not m.waiting:
            a = m.activity()
            if a:
                lines.append(P(("      " + a, "dim")))
        if state == "run" and m.waiting:
            lines.append(P(("      " + m.waiting + " (below)", "b")))
        for kind, text in st["warns"][:3]:
            for k, part in enumerate(wrap(text, w - 8)[:2]):
                lines.append(P(("      ", ""), (g["warn"] if k == 0 else " ", "warn"), (" " + part, "")))
        if len(st["warns"]) > 3:
            lines.append(P(("        %d more in the details (d)" % (len(st["warns"]) - 3), "dim")))
        if state == "fail":
            said = [t for k, t in st["msgs"] if k == "say"] or [t for k, t in st["msgs"]][-3:]
            for t in said[-4:]:
                for part in wrap(t, w - 6)[:3]:
                    lines.append(P(("      " + part, "")))
            raw = m.failed_lines(st)
            if raw:
                lines.append(P(("      From the log:", "dim")))
                for t in raw[-4:]:
                    lines.append(P(("        " + t.strip(), "dim")))
            fix = st["fix"] or "Fix the problem above, then run the installer again; finished steps are quick the second time."
            for part in wrap(fix, w - 6):
                lines.append(P(("      " + part, "b")))
            lines.append(P(("      Full log: " + home(m.log_path), "dim")))
        blocks.append((st, lines))

    bottom = []
    if m.error:
        bottom.append("")
        for k, part in enumerate(wrap(m.error, w - 4)):
            bottom.append(P(("  ", ""), (g["fail"] if k == 0 else " ", "badb"), (" " + part, "b")))
        if not any(s["state"] == "fail" for s in m.steps):
            bottom.append(P(("    Full log: " + home(m.log_path), "dim")))
    # The bar
    f = 1.0 if m.finished and not m.error and all(s["state"] in ("ok", "skip") for s in m.steps) else m.fraction()
    pct = "%3d%%" % int(f * 100)
    elapsed = mmss(now - m.t0)
    bw = max(10, min(48, w - 4 - len(pct) - len(elapsed) - 4))
    fill = int(round(f * bw))
    bottom += ["", P(("  ", ""), (g["full"] * fill, "acc"), (g["empty"] * (bw - fill), "dim"), ("  " + pct, "b"), ("  " + elapsed, "dim"))]
    if not m.finished and not m.stopped and not m.waiting:
        bottom += ["", P(("  d", "b"), (" details  " + g["dot"] + "  ", "dim"), ("Ctrl+C", "b"), (" stop  " + g["dot"] + "  log " + home(m.log_path), "dim"))]
    elif m.finished or m.stopped:
        bottom += ["", P(("  Log: " + home(m.log_path), "dim"))]

    # Fit the height: steps done (with nothing under them) fold into one line first
    room = max(4, height - 1) - len(top) - len(bottom)
    body = [l for _, ls in blocks for l in ls]
    if len(body) > room:
        folded = 0
        while len(body) > room and folded < len(blocks):
            st, ls = blocks[folded]
            if st["state"] not in ("ok", "skip") or len(ls) > 1:
                break
            folded += 1
            rest = [l for _, ls2 in blocks[folded:] for l in ls2]
            body = [P(("  ", ""), (g["ok"], "ok"), (" %d steps done" % folded, "dim"))] + rest
        if len(body) > room:
            # (still too tall: what's running or failed, and around it)
            keep = [l for st, ls in blocks if st["state"] in ("run", "fail") for l in ls]
            body = keep[-room:] if keep else body[-room:]
    return top + body + bottom


def wrapped_tail(m, width):
    out = []
    for t in m.tail:
        out += wrap(t, width - 1) if t else [""]
    return out


def render_details(m, width, height, offset, g=UTF8, color=True):
    """The details view: the log's end (offset lines up from it), full screen"""
    P = Painter(width, color)
    body_h = max(1, height - 2)
    lines = wrapped_tail(m, P.width)
    offset = max(0, min(offset, max(0, len(lines) - body_h)))
    end = len(lines) - offset
    shown = lines[max(0, end - body_h):end]
    where = "following" if offset == 0 else "%d lines up" % offset
    head = P(("Details", "b"), ("  " + home(m.log_path) + "  " + g["dot"] + "  " + where, "dim"))
    keys = P(("  %s %s  PgUp PgDn" % (g["up"], g["down"]), "b"), (" scroll  " + g["dot"] + "  ", "dim"), ("End", "b"), (" follow  " + g["dot"] + "  ", "dim"), ("q", "b"), (" back", "dim"))
    body = [P((t, "")) for t in shown]
    body += [""] * (body_h - len(body))
    return [head] + body + [keys], offset


# ── The terminal ──
KEYS = {b"\x1b[A": "up", b"\x1bOA": "up", b"\x1b[B": "down", b"\x1bOB": "down", b"\x1b[5~": "pgup", b"\x1b[6~": "pgdn",
        b"\x1b[H": "home", b"\x1b[1~": "home", b"\x1bOH": "home", b"\x1b[F": "end", b"\x1b[4~": "end", b"\x1bOF": "end",
        b"\x1b": "esc", b"k": "up", b"j": "down", b"g": "home", b"G": "end", b" ": "pgdn", b"b": "pgup",
        b"d": "d", b"D": "d", b"q": "q", b"Q": "q", b"\r": "enter", b"\n": "enter"}


def parse_keys(data):
    """Bytes read from the terminal: the keys in them"""
    out, i = [], 0
    while i < len(data):
        if data[i:i + 1] == b"\x1b":
            m = re.match(rb"\x1b(\[[0-9;]*[~A-Za-z]|O[A-Za-z])", data[i:])
            seq = m.group(0) if m else b"\x1b"
            out.append(KEYS.get(seq, "other"))
            i += len(seq)
        else:
            out.append(KEYS.get(data[i:i + 1], "other"))
            i += 1
    return out


class Screen:
    def __init__(self, out_fd, tty_fd, model, g, color):
        self.out, self.tty, self.m, self.g, self.color = out_fd, tty_fd, model, g, color
        self.saved = None
        self.drawn = 0
        self.view = "steps"
        self.offset = 0
        self.paused = False
        self.spin = 0
        self.resized = False
        self.last = 0.0

    def write(self, s):
        b = s.encode("utf-8", "replace")
        while b:
            try:
                n = os.write(self.out, b)
            except InterruptedError:
                continue
            except OSError:
                return
            b = b[n:]

    def size(self):
        for fd in (self.tty, self.out):
            try:
                c, r = os.get_terminal_size(fd)
                if c > 0 and r > 0:
                    return c, r
            except OSError:
                pass
        return 80, 24

    def grab(self):
        import termios
        try:
            self.saved = termios.tcgetattr(self.tty)
            new = termios.tcgetattr(self.tty)
            new[3] &= ~(termios.ICANON | termios.ECHO)
            new[6][termios.VMIN] = 1
            new[6][termios.VTIME] = 0
            termios.tcsetattr(self.tty, termios.TCSANOW, new)
        except (termios.error, OSError):
            self.saved = None
        self.write("\x1b[?25l")

    def release(self):
        import termios
        if self.view == "details":
            self.write("\x1b[?1049l")
            self.view = "steps"
        self.write("\x1b[?25h")
        if self.saved is not None:
            try:
                termios.tcsetattr(self.tty, termios.TCSANOW, self.saved)
            except (termios.error, OSError):
                pass
            self.saved = None

    def draw(self):
        cols, rows = self.size()
        if self.view == "details":
            lines, self.offset = render_details(self.m, cols, rows, self.offset, self.g, self.color)
            self.write("\x1b[H" + "\x1b[K\r\n".join(lines) + "\x1b[K\x1b[J")
        else:
            lines = render(self.m, cols, rows, self.spin, self.g, self.color)
            s = ""
            if self.resized:
                # (lines drawn before wrap differently now: start clean)
                s += "\x1b[2J\x1b[3J\x1b[H"
            elif self.drawn > 1:
                s += "\x1b[%dA" % (self.drawn - 1)
            self.write(s + "\r\x1b[J" + "\r\n".join(lines))
            self.drawn = len(lines)
        self.resized = False
        self.m.changed = False
        self.last = time.monotonic()

    def leave_details(self):
        if self.view == "details":
            self.write("\x1b[?1049l")
            self.view = "steps"

    def key(self, k):
        cols, rows = self.size()
        page = max(1, rows - 3)
        if self.view == "steps":
            if k == "d":
                self.view = "details"
                self.offset = 0
                self.write("\x1b[?1049h\x1b[H")
            else:
                return False
        else:
            if k in ("q", "esc", "d"):
                self.leave_details()
            elif k == "up":
                self.offset += 1
            elif k == "down":
                self.offset = max(0, self.offset - 1)
            elif k == "pgup":
                self.offset += page
            elif k == "pgdn":
                self.offset = max(0, self.offset - page)
            elif k == "home":
                self.offset = 10 ** 9
            elif k == "end":
                self.offset = 0
            else:
                return False
        return True


def ack(path):
    if not path:
        return
    try:
        fd = os.open(path, os.O_WRONLY | os.O_NONBLOCK)
        try:
            os.write(fd, b"ok\n")
        finally:
            os.close(fd)
    except OSError:
        pass


def serve(a):
    try:
        log = open(a.log, "ab")
    except OSError:
        log = None
    m = Model(a.log, log)
    try:
        tty = os.open("/dev/tty", os.O_RDWR | os.O_NOCTTY)
    except OSError:
        tty = None
    term = os.environ.get("TERM", "")
    sc = Screen(1, tty if tty is not None else 1, m, glyphs_for(term, os.environ), not os.environ.get("NO_COLOR"))
    # (Ctrl+C reaches install.sh too: it stops, the stream ends, and the last
    # frame says so)
    signal.signal(signal.SIGINT, signal.SIG_IGN)
    signal.signal(signal.SIGPIPE, signal.SIG_DFL)

    def winch(*_):
        sc.resized = True
        m.changed = True
    signal.signal(signal.SIGWINCH, winch)
    sc.grab()
    sc.draw()
    stdin = sys.stdin.fileno()
    try:
        while True:
            fds = [stdin] + ([tty] if tty is not None and not sc.paused else [])
            try:
                r, _, _ = select.select(fds, [], [], 0.1)
            except InterruptedError:
                r = []
            if stdin in r:
                try:
                    data = os.read(stdin, 65536)
                except InterruptedError:
                    continue
                if not data:
                    break
                for act in m.feed(data):
                    if act == "pause":
                        sc.leave_details()
                        sc.draw()
                        sc.write("\r\n\n")
                        sc.release()
                        sc.paused = True
                        sc.drawn = 0
                        ack(a.ack)
                    elif act == "resume":
                        sc.paused = False
                        sc.grab()
                        sc.write("\r\n")
                        sc.draw()
                    elif act == "quit":
                        if not sc.paused:
                            sc.leave_details()
                            sc.draw()
                            sc.write("\r\n")
                        sc.release()
                        ack(a.ack)
                        return 0
            if tty is not None and tty in r and not sc.paused:
                data = os.read(tty, 64)
                if data == b"\x1b":
                    # (Esc alone, or the start of an arrow's sequence)
                    more, _, _ = select.select([tty], [], [], 0.03)
                    if more:
                        data += os.read(tty, 64)
                for k in parse_keys(data):
                    if sc.key(k):
                        m.changed = True
            if not sc.paused:
                running = any(s["state"] == "run" for s in m.steps)
                due = time.monotonic() - sc.last >= (0.1 if running else 0.5)
                if (m.changed and time.monotonic() - sc.last >= 0.05) or (sc.view == "steps" and due):
                    if running and sc.view == "steps":
                        sc.spin += 1
                    sc.draw()
        # The stream ended without a quit: the installer stopped
        m.end()
        if not sc.paused:
            sc.leave_details()
            sc.draw()
            sc.write("\r\n")
        return 1
    finally:
        sc.release()
        if log:
            log.close()


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    mo = sub.add_parser("mode")
    mo.add_argument("--plain", action="store_true")
    mo.add_argument("--stdout-tty", choices=["0", "1"])
    se = sub.add_parser("serve")
    se.add_argument("--log", required=True)
    se.add_argument("--ack", default="")
    a = ap.parse_args(argv)
    if a.cmd == "mode":
        stdout_tty = a.stdout_tty == "1" if a.stdout_tty else sys.stdout.isatty()
        cols = rows = 0
        tty_ok = False
        try:
            fd = os.open("/dev/tty", os.O_RDWR | os.O_NOCTTY)
            try:
                tty_ok = True
                cols, rows = os.get_terminal_size(fd)
            finally:
                os.close(fd)
        except OSError:
            pass
        mode, why = choose_mode(a.plain, stdout_tty, os.environ.get("TERM", ""), tty_ok, cols, rows)
        print(mode)
        return 0
    return serve(a)


if __name__ == "__main__":
    sys.exit(main())
