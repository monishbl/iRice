#!/usr/bin/env python3
"""How far a build is: Tessera's builds (cargo for tessera-ui and
tessera-events, make for the glass plugin) run through this, so the
installer, the console setup and the updater can show progress, the step,
the time and the log instead of minutes of silence.

  build-progress.py cargo --label NAME [--title TEXT] [--final CRATE] -- cargo build …
      runs cargo with --message-format=json-render-diagnostics (--quiet is
      dropped) and counts the units it builds
  build-progress.py make --label NAME [--title TEXT] -- make -C …
      any other command: its phase and log only (no count)
  build-progress.py show [--json]
      the last progress written (what Settings and the update screen read)

What it writes:
  $XDG_STATE_HOME/tessera/build-progress.json   rewritten as it goes:
      {"phase": prepare|download|compile|link|done|failed, "label", "title"
       (how it's named: "the shell"), "kind", "step" (what's happening, in
       words: "Compiling slint (212 of 431)"), "fraction" (0…1, null when
       there's nothing honest to measure against),
       "done", "total" (null: not known), "exact" (the total is last
       build's count, not an estimate), "crate" (what's compiling),
       "elapsed_s", "eta_s" (null: no honest guess), "log", "started_at",
       "updated_at", "pid", "warnings", "error" (the first error line),
       "tail" (on failure: the last lines of the compiler's output)}
  $XDG_STATE_HOME/tessera/build.log             the whole output (the one
      before is kept as build.log.1, .2, .3: a new log starts when the last
      one is more than 15 minutes old)
  <target dir>/tessera-build-stats.json          per label: the last
      build's unit count (for an exact total next time) and timings (ETA)

How it shows (on stderr):
  a terminal: one line that updates (bar, count, crate, time); warnings and
      errors scroll above it
  TESSERA_PROGRESS_TTY=/dev/tty: the same line on that terminal, with only
      the result on stderr (install.sh: its output goes through tee to the
      install log, which shouldn't fill with redraws)
  anything else: cargo's own lines as they come, and a timestamped summary
      every 30 s and at each new phase
  TESSERA_PROGRESS=lines: also "BUILD {json}" lines (the full-screen
      installer reads them; the same fields as the file)

The total, honestly: after one build of a target folder its unit count is
known exactly (the same Cargo.lock and flags give the same units). Before
that it's estimated from `cargo metadata` (packages for this platform,
build scripts, and those built twice: once for the build machine, for build
scripts and macros, once for the program), which comes out up to a fifth
high (features the workspace turns on that this program doesn't use): it's
shown as "212 of about 800" with no percent (fraction null: the bars sweep).
With an exact total the bar is by units, and never passes 99 % before cargo
says it's done; while the program links (its last unit: minutes of work)
it sweeps, unless there's an ETA. The ETA comes only from a previous build
of the same folder: for a full build, how long the last full build took to
get as far; for a partial one, only the final link (the last one's
length). With an ETA the bar is by time, as the ETA is. With no history
there's no ETA, only the time so far.
"""
import argparse
import collections
import hashlib
import json
import os
import re
import select
import shutil
import signal
import subprocess
import sys
import time

TAIL = 40           # lines of compiler output shown when a build fails
PLAIN_EVERY = 30    # s between summaries when not on a terminal
WRITE_EVERY = 0.5   # s between rewrites of the progress file
NEW_LOG_AFTER = 15 * 60
KEEP_LOGS = 3
STATS = "tessera-build-stats.json"
GCC_ERROR = re.compile(r"^\S+:\d+(:\d+)?: (fatal )?error: ")
STATUS = re.compile(r"^\s*(Compiling|Downloading|Downloaded|Fresh|Checking|Building|Updating|Locking|Adding|Blocking|Waiting|Finished)\s+(\S+)(?:\s+(v\S+))?")


def state_dir():
    return os.path.join(os.environ.get("XDG_STATE_HOME") or os.path.expanduser("~/.local/state"), "tessera")


def progress_path():
    return os.path.join(state_dir(), "build-progress.json")


def log_path():
    return os.path.join(state_dir(), "build.log")


def write_json(path, obj):
    tmp = path + ".tmp." + str(os.getpid())
    try:
        with open(tmp, "w") as f:
            json.dump(obj, f)
        os.replace(tmp, path)
    except OSError:
        try:
            os.unlink(tmp)
        except OSError:
            pass


def read_json(path, default=None):
    try:
        with open(path) as f:
            return json.load(f)
    except (OSError, ValueError):
        return default


def open_log(path, now=None):
    """build.log for appending; the old one moves to build.log.1 when it's
    from an earlier session (or big)."""
    now = time.time() if now is None else now
    os.makedirs(os.path.dirname(path), exist_ok=True)
    try:
        st = os.stat(path)
        if now - st.st_mtime > NEW_LOG_AFTER or st.st_size > 16 * 1024 * 1024:
            for i in range(KEEP_LOGS - 1, 0, -1):
                if os.path.exists(f"{path}.{i}"):
                    os.replace(f"{path}.{i}", f"{path}.{i + 1}")
            os.replace(path, path + ".1")
    except OSError:
        pass
    return open(path, "a", buffering=1, errors="replace")


def fmt_time(s):
    s = int(max(0, s))
    return "{}:{:02d}:{:02d}".format(s // 3600, s // 60 % 60, s % 60) if s >= 3600 else "{}:{:02d}".format(s // 60, s % 60)


def cap(t):
    """A sentence's start: "the shell" -> "The shell"; a program's name
    (tessera-events) stays as it's spelled"""
    return t if "-" in t.split(" ")[0] else t[:1].upper() + t[1:]


def fmt_left(s):
    if s is None:
        return ""
    if s < 60:
        return "less than a minute left"
    m = round(s / 60)
    return "about {} min left".format(m)


# ── The total: last build's count, else an estimate ────────────────────────

def host_triple():
    try:
        out = subprocess.run(["rustc", "-vV"], capture_output=True, text=True, timeout=30).stdout
    except (OSError, subprocess.SubprocessError):
        return ""
    m = re.search(r"^host: (\S+)", out, re.M)
    return m[1] if m else ""


def estimate_units(meta, final=None):
    """The units cargo will build for the root package (or FINAL), from
    `cargo metadata`'s resolve graph: each package once for the program, and
    again for the build machine when a build script or a macro needs it
    (release builds those with their own profile, so they aren't shared),
    plus one per build script. An estimate: features can change what's in."""
    pkgs = {p["id"]: p for p in meta.get("packages", [])}
    nodes = {n["id"]: n for n in (meta.get("resolve") or {}).get("nodes", [])}
    root = (meta.get("resolve") or {}).get("root")
    if final:
        root = next((i for i, p in pkgs.items() if p["name"] == final and i in nodes), root)
    if not root or root not in nodes:
        return None

    def kinds(p):
        return {k for t in p.get("targets", []) for k in t.get("kind", [])}

    seen = set()
    todo = [(root, False)]
    while todo:
        pid, host = todo.pop()
        if (pid, host) in seen:
            continue
        seen.add((pid, host))
        for d in nodes.get(pid, {}).get("deps", []):
            dk = [k.get("kind") for k in d.get("dep_kinds", [])] or [None]
            dp = pkgs.get(d["pkg"], {})
            macro = "proc-macro" in kinds(dp)
            for k in dk:
                if k == "dev":
                    continue
                todo.append((d["pkg"], host or k == "build" or macro))
    units = 0
    scripts = set()
    for pid, host in seen:
        k = kinds(pkgs.get(pid, {}))
        if k & {"lib", "rlib", "proc-macro", "cdylib", "dylib", "staticlib"}:
            units += 1
        if "custom-build" in k:
            scripts.add(pid)
    units += len(scripts)
    units += 1  # the program itself
    return units


def metadata(cargo, manifest, env):
    cmd = [cargo, "metadata", "--format-version", "1", "--locked"]
    if manifest:
        cmd += ["--manifest-path", manifest]
    t = host_triple()
    if t:
        cmd += ["--filter-platform", t]
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=180, env=env, stdin=subprocess.DEVNULL)
        return json.loads(r.stdout) if r.returncode == 0 else None
    except (OSError, subprocess.SubprocessError, ValueError):
        return None


def build_key(args, manifest):
    """What makes two builds' units the same: Cargo.lock, the flags."""
    h = hashlib.sha256()
    lock = os.path.join(os.path.dirname(os.path.abspath(manifest)) if manifest else os.getcwd(), "Cargo.lock")
    try:
        with open(lock, "rb") as f:
            h.update(f.read())
    except OSError:
        pass
    keep = [a for a in args if a not in ("--quiet", "-q") and not a.startswith("--message-format")]
    h.update(json.dumps([keep, os.environ.get("RUSTFLAGS", "")]).encode())
    return h.hexdigest()[:16]


def target_dir(args, manifest):
    for i, a in enumerate(args):
        if a == "--target-dir" and i + 1 < len(args):
            return args[i + 1]
        if a.startswith("--target-dir="):
            return a.split("=", 1)[1]
    if os.environ.get("CARGO_TARGET_DIR"):
        return os.environ["CARGO_TARGET_DIR"]
    return os.path.join(os.path.dirname(os.path.abspath(manifest)) if manifest else os.getcwd(), "target")


def manifest_of(args):
    for i, a in enumerate(args):
        if a == "--manifest-path" and i + 1 < len(args):
            return args[i + 1]
        if a.startswith("--manifest-path="):
            return a.split("=", 1)[1]
    return os.path.join(os.getcwd(), "Cargo.toml") if os.path.exists("Cargo.toml") else None


# ── The progress itself ───────────────────────────────────────────────────

class Progress:
    """Counts what cargo (or make) says; knows the phase, the crate, the
    time and the ETA. No I/O of its own: feed it, then read snapshot()."""

    def __init__(self, label, kind="cargo", final=None, total=None, exact=False, history=None, now=None, title=None):
        self.label = label
        self.title = title or label
        self.kind = kind
        self.final = final
        self.total = total
        self.exact = exact
        self.history = history or {}
        self.started = time.time() if now is None else now
        self.phase = "prepare" if kind == "cargo" else "compile"
        self.done = 0
        self.fresh = 0
        self.built = 0
        self.flying = collections.OrderedDict()  # crate -> units started, not finished
        self.crate = ""
        self.compile_at = None   # when the first unit started (the profile's 0)
        self.link_at = None
        self.times = []          # (units done, s since compile_at): this build's profile
        self.warnings = 0
        self.errors = []         # the first error line(s)
        self.output = collections.deque(maxlen=400)  # the compiler's own lines (not "Compiling x")
        self.error_lines = []    # the errors' whole messages (not the warnings around them)
        self.block = None        # the message being read: warning | error
        self.downloaded = 0
        self.ended = None
        self.code = None

    # cargo's stderr: status lines, warnings and errors
    def feed_line(self, line, now=None):
        now = time.time() if now is None else now
        m = STATUS.match(line)
        if m:
            what, name = m[1], m[2]
            if what in ("Downloading", "Downloaded", "Updating", "Locking", "Adding", "Blocking", "Waiting") and self.phase in ("prepare", "download"):
                self.phase = "download"
                if what == "Downloaded":
                    self.downloaded += 1
            elif what in ("Compiling", "Checking"):
                self.flying[name] = self.flying.get(name, 0) + 1
                self.flying.move_to_end(name)
                if self.compile_at is None:
                    self.compile_at = now
                if self.phase in ("prepare", "download"):
                    self.phase = "compile"
                self._phase(now)
            self.block = None
            return
        if line.strip():
            self.output.append(line.rstrip("\n"))
        s = line.lstrip()
        if s.startswith("warning"):
            self.block = "warning"
        elif s.startswith("error") or s.startswith("For more information about this error"):
            self.block = "error"
        if self.block == "error" and len(self.error_lines) < 400:
            self.error_lines.append(line.rstrip("\n"))
        if s.startswith("warning:") and not re.match(r"warning: `[^`]+` \(.*\) generated \d+ warning", s):
            self.warnings += 1
        elif re.match(r"error(\[E\d+\])?:", s) and len(self.errors) < 3 and not s.startswith("error: could not compile"):
            self.errors.append(s.strip())
        elif s.startswith("error: could not compile") and not self.errors:
            self.errors.append(s.strip())
        elif self.kind != "cargo" and GCC_ERROR.match(s) and len(self.errors) < 3:
            # (a C++ compiler's "main.cpp:12:3: error: …", for make)
            self.errors.append(s.strip())

    # cargo's stdout: one JSON message per line
    def feed_json(self, msg, now=None):
        now = time.time() if now is None else now
        r = msg.get("reason")
        if r == "compiler-artifact":
            self.done += 1
            if msg.get("fresh"):
                self.fresh += 1
            else:
                self.built += 1
            if self.compile_at is None:
                self.compile_at = now
            if self.phase in ("prepare", "download"):
                self.phase = "compile"
            name = (msg.get("target") or {}).get("name", "").replace("_", "-")
            pname = self._package(msg.get("package_id", "")) or name
            kinds = (msg.get("target") or {}).get("kind", [])
            if "custom-build" not in kinds:
                # (the program's package is done only with the program itself)
                if pname != self.final or "bin" in kinds:
                    left = self.flying.get(pname, 0) - 1
                    if left > 0:
                        self.flying[pname] = left
                    else:
                        self.flying.pop(pname, None)
            if self.compile_at is not None:
                self.times.append((self.done, now - self.compile_at))
            self._phase(now)
        elif r == "build-finished":
            pass

    @staticmethod
    def _package(pid):
        # "registry+https://…#slint@1.9.0", "path+file:///…/tessera-ui#0.1.0" (named by its folder),
        # or the older "slint 1.9.0 (registry+…)"
        if " (" in pid:
            return pid.split(" ", 1)[0]
        frag = pid.rsplit("#", 1)[-1]
        if "@" in frag:
            return frag.split("@", 1)[0]
        base = pid.rsplit("#", 1)[0].rstrip("/").rsplit("/", 1)[-1]
        return base

    def _phase(self, now):
        if self.phase not in ("compile", "link") or self.kind != "cargo":
            return
        others = [c for c, n in self.flying.items() if n > 0 and c != self.final]
        if self.final and self.final in self.flying and not others:
            if self.phase != "link":
                self.phase = "link"
                self.link_at = now
        elif self.phase == "link" and others:
            self.phase = "compile"
            self.link_at = None
        live = [c for c, n in self.flying.items() if n > 0]
        self.crate = live[-1] if live else self.crate

    def finish(self, code, now=None):
        self.ended = time.time() if now is None else now
        self.code = code
        self.phase = "done" if code == 0 else "failed"
        if code == 0 and self.kind == "cargo":
            self.total, self.exact = self.done, True

    def elapsed(self, now=None):
        return ((self.ended or (time.time() if now is None else now)) - self.started)

    def full_build(self):
        """Most of it is being built (not taken as it was): the last full
        build's timings fit"""
        return self.done > 0 and self.fresh <= 0.1 * self.done

    def eta(self, now=None):
        now = time.time() if now is None else now
        h = self.history
        if self.kind != "cargo":
            # (make: how long the last good build took; nothing to count)
            if self.phase != "compile" or not h.get("make_s"):
                return None
            left = h["make_s"] - self.elapsed(now)
            return max(0.0, left) if left > -30 else None
        if self.phase == "link" and h.get("link_s") and self.link_at is not None:
            left = h["link_s"] - (now - self.link_at)
            return max(0.0, left) if left > -30 else None
        if self.phase != "compile" or not self.exact or not self.total or self.compile_at is None:
            return None
        prof = h.get("profile") or {}
        dur = h.get("compile_s")
        if not prof or not dur or not self.full_build():
            # (a partial build: mostly the final link, when that's next)
            return None
        f = 100 * self.done / self.total
        keys = sorted(int(k) for k in prof)
        lo = max([k for k in keys if k <= f], default=None)
        hi = min([k for k in keys if k > f], default=None)
        if lo is None:
            return None
        t_lo = prof[str(lo)] if str(lo) in prof else prof[lo]
        t_f = t_lo
        if hi is not None:
            t_hi = prof[str(hi)] if str(hi) in prof else prof[hi]
            t_f = t_lo + (t_hi - t_lo) * (f - lo) / (hi - lo)
        if t_f < 5:
            return None
        ratio = min(3.0, max(0.33, (now - self.compile_at) / t_f))
        return max(0.0, (dur - t_f) * ratio)

    def fraction(self, now=None, eta=None):
        """0…1, or None when there's nothing honest to measure against.
        With an ETA (a previous build's timings) it's by time, as the ETA
        is; else by units, but not while linking: the last unit is minutes
        of work, so the bar sweeps then instead of sitting at 85 %."""
        if self.phase == "done":
            return 1.0
        if self.phase in ("prepare", "download", "failed"):
            return None
        if self.kind != "cargo" and eta is None:
            return None
        if eta is not None:
            el = self.elapsed(now)
            return min(0.99, el / max(1.0, el + eta))
        if not self.total or not self.exact or self.phase == "link":
            return None
        return min(0.99, self.done / self.total)

    def step_text(self):
        """"Compiling slint (212 of 431)", "Linking the shell (the longest step, a few minutes)" …"""
        if self.phase == "prepare":
            return "Getting ready"
        if self.phase == "download":
            return "Downloading crates" + (" ({})".format(self.downloaded) if self.downloaded else "")
        if self.phase == "link":
            return "Linking {} (the longest step, a few minutes)".format(self.title)
        if self.phase == "done":
            return "Built {}".format(self.title)
        if self.phase == "failed":
            return "{} didn't build".format(cap(self.title))
        if self.kind != "cargo":
            return "Building {}".format(self.title)
        count = "{} of {}{}".format(self.done, "" if self.exact else "about ", self.total) if self.total else "{} built".format(self.done)
        return "Compiling {} ({})".format(self.crate, count) if self.crate else "Compiling ({})".format(count)

    def snapshot(self, log="", now=None):
        now = time.time() if now is None else now
        eta = self.eta(now)
        snap = {
            "phase": self.phase, "label": self.label, "title": self.title, "kind": self.kind,
            "done": self.done, "total": self.total, "exact": bool(self.exact and self.total),
            "fraction": self.fraction(now, eta), "crate": self.crate, "step": self.step_text(),
            "elapsed_s": round(self.elapsed(now), 1), "eta_s": None if eta is None else round(eta),
            "log": log, "started_at": round(self.started, 3), "updated_at": round(now, 3), "pid": os.getpid(),
            "warnings": self.warnings, "error": self.errors[0] if self.errors else "",
        }
        if self.phase == "failed":
            snap["tail"] = self.tail()
        return snap

    def tail(self):
        """What a failure shows: the errors' messages (cargo goes on
        finishing other crates after one, so the last lines are often only
        their warnings), else the last lines of output"""
        if self.error_lines:
            return self.error_lines[:TAIL]
        return list(self.output)[-TAIL:]

    def history_after(self):
        """The stats to keep for next time (None: nothing worth keeping)"""
        if self.code != 0:
            return None
        if self.kind != "cargo":
            return {"make_s": round(self.elapsed(), 1)} if self.ended else None
        h = dict(self.history)
        h["units"] = self.done
        if self.link_at is not None and self.ended:
            h["link_s"] = round(self.ended - self.link_at, 1)
        if self.full_build() and self.compile_at is not None and self.ended:
            h["compile_s"] = round(self.ended - self.compile_at, 1)
            prof = {}
            for done, t in self.times:
                pct = int(100 * done / max(1, self.done)) // 5 * 5
                prof.setdefault(str(pct), round(t, 1))
            h["profile"] = prof
        return h


def summary_line(snap, now=None):
    """A plain line: "[12:03:04] tessera-ui: Compiling slint (212 of 431) · 3:12 · about 4 min left" """
    parts = [snap["step"], fmt_time(snap["elapsed_s"])]
    if snap.get("eta_s") is not None:
        parts.append(fmt_left(snap["eta_s"]))
    return "[{}] {}".format(time.strftime("%H:%M:%S", time.localtime(now)), " · ".join(parts))


def live_line(snap, width, spin=0):
    """The terminal's one updating line"""
    frac = snap.get("fraction")
    right = fmt_time(snap["elapsed_s"]) + ("  " + fmt_left(snap["eta_s"]) if snap.get("eta_s") is not None else "")
    bar_w = 20 if width >= 90 else 10
    if frac is not None:
        fill = int(round(bar_w * frac))
        bar = "█" * fill + "░" * (bar_w - fill)
    else:
        pos = spin % (bar_w + 4)
        bar = "".join("█" if pos - 4 <= i < pos else "░" for i in range(bar_w))
    text = "  {} {}".format(bar, snap["step"])
    room = width - len(right) - 3
    if len(text) > room:
        text = text[:max(0, room - 1)] + "…"
    return text + " " * max(1, width - len(text) - len(right) - 1) + right


# ── Running it ─────────────────────────────────────────────────────────────

class Out:
    """Where it shows: a terminal line, plain lines, BUILD lines"""

    def __init__(self, err=None):
        self.err = err or sys.stderr
        self.tty = None
        path = os.environ.get("TESSERA_PROGRESS_TTY", "")
        self.separate = False
        if path:
            try:
                self.tty = open(path, "w", buffering=1)
                self.separate = True
            except OSError:
                self.tty = None
        if self.tty is None and self.err.isatty() and os.environ.get("TERM") != "dumb":
            self.tty = self.err
        self.lines = os.environ.get("TESSERA_PROGRESS") == "lines"
        self.shown = False
        self.last_plain = 0.0
        self.last_phase = None
        self.last_lines = 0.0
        self.spin = 0
        self.block = None  # the diagnostic being printed: warning | error

    def width(self):
        try:
            return max(40, min(160, os.get_terminal_size(self.tty.fileno()).columns))
        except (OSError, AttributeError, ValueError):
            return 100

    def clear(self):
        if self.tty and self.shown:
            self.tty.write("\r\x1b[K")
            self.tty.flush()
            self.shown = False

    def text(self, line):
        """A line of the compiler's (a warning, an error) or cargo's. On a
        terminal only errors scroll above the progress line: warnings
        (Tessera's own code has some) are counted, and are in the log"""
        if self.tty:
            s = line.lstrip()
            if STATUS.match(line):
                self.block = None
                return
            if s.startswith("warning"):
                self.block = "warning"
            elif s.startswith("error"):
                self.block = "error"
            if self.block == "warning" or not line.strip() and self.block is None:
                return
            self.clear()
            self.tty.write(line + "\n")
        else:
            self.err.write(line + "\n")
            self.err.flush()

    def progress(self, snap, now, force=False):
        if self.tty:
            self.spin += 1
            self.tty.write("\r\x1b[K" + live_line(snap, self.width(), self.spin))
            self.tty.flush()
            self.shown = True
        elif force or snap["phase"] != self.last_phase or now - self.last_plain >= PLAIN_EVERY:
            self.err.write(summary_line(snap, now) + "\n")
            self.err.flush()
            self.last_plain = now
        self.last_phase = snap["phase"]
        if self.lines and (force or now - self.last_lines >= 0.5):
            self.err.write("BUILD " + json.dumps(snap, separators=(",", ":")) + "\n")
            self.err.flush()
            self.last_lines = now

    def result(self, text):
        """The end: on stderr always (it's in the install log too)"""
        self.clear()
        self.err.write(text + "\n")
        self.err.flush()


def run(kind, label, cmd, final=None, out=None, progress_file=None, log_file=None, title=None):
    out = out or Out()
    progress_file = progress_file or progress_path()
    log_file = log_file or log_path()
    env = dict(os.environ, CARGO_TERM_COLOR="never", CARGO_TERM_PROGRESS_WHEN="never")
    env.pop("TESSERA_PROGRESS_TTY", None)
    total, exact, history, stats_path, key = None, False, {}, None, None
    if kind != "cargo":
        # (make: the last good build's length, for an ETA and a percent),
        # kept beside the progress file
        stats_path = os.path.join(os.path.dirname(progress_file), STATS)
        stats = read_json(stats_path, {}) or {}
        history = stats.get(label) if isinstance(stats.get(label), dict) else {}
    if kind == "cargo":
        cmd = [a for a in cmd if a not in ("--quiet", "-q") and not a.startswith("--message-format")]
        cmd.append("--message-format=json-render-diagnostics")
        manifest = manifest_of(cmd[1:])
        key = build_key(cmd[1:], manifest)
        stats_path = os.path.join(target_dir(cmd[1:], manifest), STATS)
        stats = read_json(stats_path, {}) or {}
        history = stats.get(label) if isinstance(stats.get(label), dict) else {}
        if history.get("key") == key and history.get("units"):
            total, exact = history["units"], True
        else:
            # (another Cargo.lock or other flags: timings may still help, the count not)
            history = {k: v for k, v in history.items() if k in ("link_s",)}
            meta = metadata(cmd[0], manifest, env)
            total = estimate_units(meta, final) if meta else None
    os.makedirs(os.path.dirname(progress_file), exist_ok=True)
    log = open_log(log_file)
    log.write("== {} {} ({})\n$ {}\n".format(time.strftime("%F %T"), label, kind, " ".join(cmd)))
    title = title or label
    p = Progress(label, kind, final, total, exact, history, title=title)
    last_write = 0.0

    def save(force=False):
        nonlocal last_write
        now = time.time()
        if force or now - last_write >= WRITE_EVERY:
            write_json(progress_file, p.snapshot(log_file, now))
            last_write = now
        out.progress(p.snapshot(log_file, now), now, force)

    save(True)
    try:
        proc = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, stdin=subprocess.DEVNULL, env=env)
    except OSError as e:
        p.errors.append(str(e))
        p.finish(127)
        save(True)
        out.result("✗ {} didn't build: {}".format(title, e))
        return 127
    # (Ctrl+C: cargo gets it too, as part of the terminal's group: it stops,
    # and the failure is written down like any other)
    old = signal.signal(signal.SIGINT, signal.SIG_IGN)
    bufs = {proc.stdout.fileno(): b"", proc.stderr.fileno(): b""}
    open_fds = set(bufs)
    try:
        while open_fds:
            r, _, _ = select.select(list(open_fds), [], [], 0.25)
            for fd in r:
                chunk = os.read(fd, 65536)
                if not chunk:
                    open_fds.discard(fd)
                    continue
                bufs[fd] += chunk
                *lines, bufs[fd] = bufs[fd].split(b"\n")
                for raw in lines:
                    handle(raw.decode("utf-8", "replace"), fd == proc.stdout.fileno() and kind == "cargo", p, out, log)
            save()
        for fd, rest in bufs.items():
            if rest:
                handle(rest.decode("utf-8", "replace"), fd == proc.stdout.fileno() and kind == "cargo", p, out, log)
        code = proc.wait()
    finally:
        signal.signal(signal.SIGINT, old)
    p.finish(code)
    snap = p.snapshot(log_file)
    write_json(progress_file, snap)
    log.write("== {} {} after {} (exit {})\n".format("built" if code == 0 else "failed:", title, fmt_time(p.elapsed()), code))
    log.close()
    if code == 0:
        h = p.history_after()
        if h is not None and stats_path:
            h["key"] = key
            stats = read_json(stats_path, {}) or {}
            stats[label] = h
            if os.path.isdir(os.path.dirname(stats_path)):
                write_json(stats_path, stats)
        out.result("✓ Built {} in {}{}{}".format(title, fmt_time(p.elapsed()), " ({} crates)".format(p.done) if kind == "cargo" else "",
                                                 ", {} warning{}".format(p.warnings, "" if p.warnings == 1 else "s") if p.warnings else ""))
    else:
        if out.lines:
            out.err.write("BUILD " + json.dumps(snap, separators=(",", ":")) + "\n")
        tail = p.tail()
        out.result("✗ {} didn't build (exit {}, after {}).{}".format(
            cap(title), code, fmt_time(p.elapsed()), (" The compiler's errors:" if p.error_lines else " The last lines of the compiler's output:") if tail else ""))
        for line in tail:
            out.err.write("    " + line + "\n")
        out.err.write("  The whole log: {}\n".format(log_file))
        out.err.flush()
    return code


def handle(line, is_json, p, out, log):
    if is_json:
        try:
            msg = json.loads(line)
        except ValueError:
            # (a build script's own println, or anything that isn't a message)
            if line.strip():
                log.write(line + "\n")
            return
        p.feed_json(msg)
        if msg.get("reason") == "compiler-message":
            # (with json-render-diagnostics these are cargo's own: rendered below if any)
            rendered = (msg.get("message") or {}).get("rendered")
            if rendered:
                for l in rendered.rstrip("\n").split("\n"):
                    log.write(l + "\n")
                    p.feed_line(l)
                    out.text(l)
        return
    log.write(line + "\n")
    p.feed_line(line)
    out.text(line)


def show(as_json):
    snap = read_json(progress_path())
    if not snap:
        print("No build yet")
        return 1
    if as_json:
        print(json.dumps(snap, indent=1))
    else:
        print(summary_line(snap, snap.get("updated_at")))
    return 0


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    if argv[:1] == ["show"]:
        return show("--json" in argv)
    if "--" not in argv:
        print(__doc__.split("\n\n")[1], file=sys.stderr)
        return 2
    i = argv.index("--")
    ap = argparse.ArgumentParser()
    ap.add_argument("kind", choices=["cargo", "make"])
    ap.add_argument("--label", required=True, help="what it is (tessera-ui): its stats are kept under it")
    ap.add_argument("--title", help="how it's named to people (the shell); the label otherwise")
    ap.add_argument("--final", help="cargo: the program's package (its last unit is the link)")
    a = ap.parse_args(argv[:i])
    cmd = argv[i + 1:]
    if not cmd or (a.kind == "cargo" and not shutil.which(cmd[0]) and not os.path.exists(cmd[0])):
        print("{} not found".format(cmd[0] if cmd else "the command"), file=sys.stderr)
        return 127
    return run(a.kind, a.label, cmd, a.final, title=a.title)


if __name__ == "__main__":
    sys.exit(main())
