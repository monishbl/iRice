#!/usr/bin/env python3
"""The system part of Software Update's one Update button, without a
terminal: a snapshot, pacman -Syu, iRice's missing packages, removing the
ones it dropped, then AUR updates with yay or paru. The iRice part follows,
started by the shell once this is done (services/SoftwareUpdate.qml).

    sysupdate.py run [--fake] [--system] [--snapshot snapper|timeshift]
                     [--ignore a,b] [--install a,b] [--remove a,b] [--aur]
                     [--aur-install a,b]
--aur-install: these packages from the AUR (and their dependencies, from the
repositories too), in the AUR step instead of updates: drivers Settings
offers (a fingerprint reader's). Same single password.
    sysupdate.py cancel         stop it while it's still waiting for the password
    sysupdate.py helper-status  exit 0 when iRice's root helper is installed
                                and the same as this folder's

Everything that needs root is one pkexec (one password, asked at the
start, never in the middle): iRice's root helper (irice-root.py update),
with the steps in order: snapshot → its own files when they changed
(setup: /usr/lib/irice, the polkit action and rule) → pacman -Syu [--ignore
held] → pacman -S --needed <missing> → pacman -Rns <dropped>. It stops at the
first failure. The installed copy (/usr/lib/irice/irice-root) runs when it's
up to date; otherwise this folder's, which installs itself (setup).
AUR updates run after it as you, while the root part waits: yay/paru get
--sudo irice-sudo, which hands each pacman command to this job over a
socket only it can use (sudo.sock, 0600, with a secret for this run), and
this job to the root part over its stdin (a socket pair). The root part
runs pacman alone, its arguments checked, and ends when this job closes
its end. No second password.

Everything goes to $XDG_RUNTIME_DIR/irice-sysupdate/:
    status.json  what's happening, rewritten whole on each change (the shell
                 reads it: Updates.qml); survives shell restarts
    log          the full output, as a terminal would show it
    root.log     what the root part writes (a file, not a pipe: pacman never
                 depends on this job being alive, so it can't be cut off by a
                 broken pipe mid-transaction); this job follows it into log
    lock         held (flock) for the whole run: one update at a time; a
                 status that says "running" with the lock free was interrupted
    auth.pid     the pkexec waiting for the password
    sudo.sock    irice-sudo's way in, during the AUR step only

Root comes from pkexec, so the password is asked by iRice's own dialog (its
polkit agent). The job must run in the login session (setsid from the shell,
not systemd-run --user): polkit finds the agent through the session.
Cancel only works while pkexec waits for the password. The root part's first
line is a marker ("@@authenticated"); from then on Cancel is ignored: pacman
runs as root, can't be signalled by us, and stopping a transaction midway
would break the system anyway. Cancel signals only the job holding the lock,
checked by its start time (a stale pid may belong to anything by now).

--fake (or IRICE_FAKE_SYSUPDATE=1, and always in the dev sandbox) replays a
canned pacman run with delays: the whole UI, nothing installed.
"""
import fcntl
import hmac
import importlib.util
import json
import os
import re
import secrets
import shutil
import signal
import socket
import struct
import subprocess
import sys
import threading
import time

RUN = os.environ.get("XDG_RUNTIME_DIR") or f"/run/user/{os.getuid()}"
DIR = os.path.join(RUN, "irice-sysupdate")
STATUS = os.path.join(DIR, "status.json")
LOG = os.path.join(DIR, "log")
LOCK = os.path.join(DIR, "lock")
AUTH = os.path.join(DIR, "auth.pid")
ROOTLOG = os.path.join(DIR, "root.log")
SUDO_SOCKET = os.path.join(DIR, "sudo.sock")
HERE = os.path.dirname(os.path.abspath(__file__))
SHELL = os.path.dirname(os.path.dirname(HERE))  # (where the root helper's setup reads from)
SHIM = os.path.join(HERE, "irice-sudo")
SOURCE = os.path.join(HERE, "irice-root.py")  # the root helper, as installed by setup
SYSTEM = os.environ.get("IRICE_SYSTEM_ROOT", "/")  # (tests)
INSTALLED = os.path.join(SYSTEM, "usr/lib/irice/irice-root")
DBLOCK = "/var/lib/pacman/db.lck"
# Updated packages that want a restart to take effect
RESTART = re.compile(r"^(linux(-[a-z0-9]+)?|linux-firmware.*|systemd|systemd-libs|glibc|mesa|lib32-mesa|nvidia.*|amd-ucode|intel-ucode|dbus|dbus-broker.*)$")

state = {
    "state": "running",   # running | done | error | cancelled
    "phase": "starting",  # starting | auth | sync | download | install | hooks | aur | build | done
    "steps": [],          # the planned steps: snapshot, system, packages, remove, aur
    "step": "",           # the one running
    "text": "Starting",
    "done": 0, "total": 0, "percent": 0,
    "package": "",
    "upgraded": [],
    "restart": False,
    "aur": False,
    "error": "",
    "started": time.time(), "finished": 0,
    "pid": os.getpid(),
    "pidStart": "",  # the job's start time (/proc/<pid>/stat field 22): cancel checks it
}
log = None
cancel_asked = False
authed = False   # the root part has started: no more cancelling
asking = None    # the pkexec waiting for the password
root_done = False  # the root part's pacman steps finished (it waits for the AUR step)
root_proc = None   # the root part, kept until the AUR step is over


def start_time(pid):
    """A process's start time (clock ticks since boot), "" if it's gone."""
    try:
        with open(f"/proc/{pid}/stat") as f:
            return f.read().rsplit(")", 1)[1].split()[19]
    except (OSError, IndexError):
        return ""


def save():
    tmp = STATUS + ".new"
    with open(tmp, "w") as f:
        json.dump(state, f)
    os.replace(tmp, STATUS)


def set_(**kw):
    state.update(kw)
    save()


def out(line):
    log.write(line + "\n")
    log.flush()


# ── Reading pacman (and yay/paru) output ────────────────────────────────
# The run is split into weighted parts so one percentage covers it all:
# pacman 0–90 (sync 0–5, download 5–45, install 45–85, hooks 85–90), AUR 90–100
# (or pacman 0–100 when there's nothing from the AUR).
span = (0, 100)


def at(lo, hi, frac):
    a, b = span
    p = a + (b - a) * (lo + (hi - lo) * max(0.0, min(1.0, frac))) / 100
    return int(max(state["percent"], min(99, p)))


WEIGHT = {"snapshot": 6, "setup": 1, "system": 64, "packages": 12, "remove": 4, "aur": 14}


def begin(step):
    """A step starts: its share of the percentage, and its name."""
    global span
    steps = state["steps"] or [step]
    total = sum(WEIGHT.get(x, 10) for x in steps)
    before = sum(WEIGHT.get(x, 10) for x in steps[:steps.index(step)]) if step in steps else 0
    span = (100 * before / total, 100 * (before + WEIGHT.get(step, 10)) / total)
    text = {"snapshot": "Taking a snapshot", "setup": "Setting up iRice's system helper", "system": "Checking for updates", "packages": "Installing iRice's packages",
            "remove": "Removing unused packages", "aur": "Updating from the AUR"}.get(step, step)
    set_(step=step, phase=step if step != "system" else "sync", text=text, done=0, total=0, percent=at(0, 0, 0))


def parse(line, aur=False):
    s = line.strip()
    if not s:
        return
    if s.startswith("@@step "):
        begin(s[7:].strip())
        return
    if s.startswith(":: Synchronizing package databases"):
        set_(phase="sync", text="Checking for updates", percent=at(0, 5, 0))
        return
    m = re.match(r"^Packages \((\d+)\)\s*(.*)$", s)
    if m:
        set_(total=int(m.group(1)), done=0)
        return
    if s.startswith(":: Retrieving packages"):
        set_(phase="download", text="Downloading", done=0, percent=at(5, 45, 0))
        return
    # non-tty pacman: "<pkg> downloading..." per file
    m = re.match(r"^(\S+) downloading\.\.\.$", s)
    if m and state["phase"] == "download":
        d = state["done"] + 1
        t = max(state["total"], d)
        set_(done=d, package=m.group(1), text=f"Downloading {d} of {t}", percent=at(5, 45, d / t))
        return
    if s.startswith(":: Processing package changes") or s.startswith("(1/") and "checking keys" in s:
        set_(phase="install", text="Installing", done=0, percent=at(45, 85, 0))
        return
    m = re.match(r"^\(\s*(\d+)/(\d+)\)\s+(upgrading|installing|reinstalling|downgrading|removing)\s+(\S+)", s)
    if m:
        d, t, verb, pkg = int(m.group(1)), int(m.group(2)), m.group(3), m.group(4)
        up = state["upgraded"] + ([pkg] if verb != "removing" and pkg not in state["upgraded"] else [])
        word = "Removing" if verb == "removing" else "Installing"
        set_(phase="install", done=d, total=t, package=pkg, upgraded=up,
             restart=state["restart"] or bool(RESTART.match(pkg)),
             text=f"{word} {d} of {t}", percent=at(45, 85, d / t))
        return
    if s.startswith(":: Running post-transaction hooks"):
        set_(phase="hooks", text="Finishing up", percent=at(85, 100, 0))
        return
    # yay / paru building an AUR package
    m = re.match(r"^==> Making package: (\S+)", s)
    if m:
        set_(phase="build", package=m.group(1), text=f"Building {m.group(1)}", percent=at(10, 80, 0.2))
        return


def feed(line, aur):
    global authed, root_done
    line = line.rstrip("\n").split("\r")[-1]  # (\r progress bars, just in case)
    if line == "@@authenticated":
        authed = True
        if state["phase"] == "auth":
            set_(phase="sync", text="Checking for updates")
        return
    if line == "@@root-done":
        root_done = True
        return
    out(line)
    parse(line, aur)


def stream(cmd, aur=False, env=None):
    """Runs cmd as you (the AUR helper), logging and parsing each line.
    Returns the exit code."""
    out("$ " + " ".join(cmd))
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL,
                         env=env, text=True, errors="replace", bufsize=1)
    for line in p.stdout:
        feed(line, aur)
    return p.wait()


def stream_root(cmd, env=None, stdin=subprocess.DEVNULL):
    """Runs the pkexec'd root part. Its output goes to a file this job opened
    (handed over as its stdout, so nothing is opened by path as root), and
    this job follows the file: if the job dies, root carries on unharmed.
    Returns its exit code, or 0 once it says "@@root-done" (it then waits
    for the AUR step's pacman commands, on stdin: root_proc)."""
    global asking, root_proc
    out("$ " + " ".join(cmd[:3]) + " …")
    with open(ROOTLOG, "w") as w:
        asking = subprocess.Popen(cmd, stdout=w, stderr=subprocess.STDOUT, stdin=stdin, env=env)
    root_proc = asking
    with open(AUTH, "w") as f:
        f.write(f"{asking.pid} {start_time(asking.pid)}")
    set_(phase="auth", text="Waiting for your password")
    pos = 0
    rest = ""
    with open(ROOTLOG, errors="replace") as r:
        while True:
            done = asking.poll() is not None or root_done
            r.seek(pos)
            chunk = r.read()
            pos = r.tell()
            rest += chunk
            *lines, rest = rest.split("\n")
            for line in lines:
                feed(line, False)
            if done:
                if rest and not root_done:
                    feed(rest, False)
                break
            time.sleep(0.2)
    code = 0 if root_done else asking.returncode
    asking = None
    try:
        os.unlink(AUTH)
    except FileNotFoundError:
        pass
    return code


def finish(st, text, error=""):
    set_(state=st, phase="done", text=text, error=error, finished=time.time(),
         percent=100 if st == "done" else state["percent"])


def tail(n=6):
    try:
        with open(LOG, errors="replace") as f:
            lines = [l.rstrip() for l in f if l.strip() and not l.startswith("$ ")]
        return "\n".join(lines[-n:])
    except OSError:
        return ""


def aur_helper():
    for h in ("yay", "paru"):
        if shutil.which(h):
            return h
    return ""


# ── The fake run (sandbox / testing) ────────────────────────────────────
FAKE = [
    (0.6, ":: Synchronizing package databases..."),
    (0.4, " core downloading..."), (0.3, " extra downloading..."), (0.3, " multilib downloading..."),
    (0.5, ":: Starting full system upgrade..."),
    (0.3, "resolving dependencies..."), (0.2, "looking for conflicting packages..."),
    (0.3, ""), (0.1, "Packages (5) firefox-133.0-1  linux-6.12.1.arch1-1  mesa-1:24.3.1-1  python-3.13.1-1  vim-9.1.0900-1"),
    (0.2, ""), (0.2, "Total Download Size:   312.40 MiB"), (0.1, "Total Installed Size:  980.12 MiB"),
    (0.1, "Net Upgrade Size:       12.30 MiB"), (0.4, ":: Proceed with installation? [Y/n] "),
    (0.2, ":: Retrieving packages..."),
    (1.0, " firefox-133.0-1-x86_64 downloading..."), (1.4, " linux-6.12.1.arch1-1-x86_64 downloading..."),
    (0.8, " mesa-1:24.3.1-1-x86_64 downloading..."), (0.5, " python-3.13.1-1-x86_64 downloading..."),
    (0.3, " vim-9.1.0900-1-x86_64 downloading..."),
    (0.5, "(5/5) checking keys in keyring"), (0.4, "(5/5) checking package integrity"),
    (0.3, "(5/5) loading package files"), (0.3, "(5/5) checking for file conflicts"),
    (0.3, "(5/5) checking available disk space"), (0.3, ":: Processing package changes..."),
    (0.9, "(1/5) upgrading firefox"), (1.2, "(2/5) upgrading linux"), (0.7, "(3/5) upgrading mesa"),
    (0.6, "(4/5) upgrading python"), (0.3, "(5/5) upgrading vim"),
    (0.3, ":: Running post-transaction hooks..."),
    (0.6, "(1/3) Updating module dependencies..."), (0.9, "(2/3) Updating linux initcpios..."),
    (0.3, "(3/3) Arming ConditionNeedsUpdate..."),
]


FAKE_STEP = {
    "snapshot": [(0.8, "Creating snapshot: Before iRice update"), (0.6, "Snapshot 412 created")],
    "packages": [(0.4, "resolving dependencies..."), (0.3, ":: Processing package changes..."),
                 (0.8, "(1/1) installing gpu-screen-recorder")],
    "remove": [(0.4, "checking dependencies..."), (0.6, "(1/1) removing mactahoe-gtk-theme")],
    "aur": [(0.6, ":: Searching AUR for updates..."), (0.8, "==> Making package: apple_cursor 2.0.1-1"),
            (1.5, "==> Finished making: apple_cursor 2.0.1-1"), (0.6, "(1/1) upgrading apple_cursor")],
}


def fake_run(plan):
    global authed
    set_(phase="auth", text="Waiting for your password")
    for _ in range(15):
        time.sleep(0.1)
        if cancel_asked:
            return finish("cancelled", "Update cancelled")
    authed = True  # (as if "@@authenticated" arrived: no cancelling from here)
    set_(phase="sync", text="Checking for updates")
    out("(simulated: nothing is installed)")
    for step in plan["steps"]:
        out(f"@@step {step}")
        parse(f"@@step {step}")
        for delay, line in (FAKE if step == "system" else FAKE_STEP.get(step, [])):
            time.sleep(delay)
            out(line)
            parse(line)
    finish("done", "Updated")


# ── The root helper ─────────────────────────────────────────────────────
def root_helper():
    spec = importlib.util.spec_from_file_location("irice_root", SOURCE)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def helper_current():
    """Whether everything the root helper's setup installs is there and the
    same as in this folder (else the update's root part installs it)."""
    for rel, dest, mode in root_helper().FILES:
        try:
            with open(os.path.join(SHELL, rel), "rb") as a, open(os.path.join(SYSTEM, dest.lstrip("/")), "rb") as b:
                if a.read() != b.read():
                    return False
        except OSError:
            return False
    return True


def root_command(plan, broker):
    """pkexec's arguments for the root part: the installed helper, or this
    folder's (by path, run by Python) when it's missing or out of date."""
    cmd = [INSTALLED] if not plan["setup"] and os.access(INSTALLED, os.X_OK) else [sys.executable, "-I", SOURCE]
    cmd.append("update")
    if plan["snapshot"]:
        cmd += ["--snapshot", plan["snapshot"]]
    if plan["system"]:
        cmd.append("--system")
    for k in ("ignore", "install", "remove"):
        if plan[k]:
            cmd += ["--" + k, ",".join(plan[k])]
    if plan["setup"]:
        cmd += ["--setup", SHELL]
    if broker:
        cmd.append("--broker")
    return cmd


# ── The AUR step's pacman commands ──────────────────────────────────────
class Broker:
    """sudo.sock, for irice-sudo (started by yay/paru): each request, with
    this run's secret and from this user, goes to the root part over chan;
    its answer comes back the same way. One at a time."""

    def __init__(self, chan):
        self.chan = chan
        self.answers = chan.makefile("rb")
        self.token = secrets.token_hex(16)
        self.lock = threading.Lock()
        self.stop = threading.Event()
        try:
            os.unlink(SUDO_SOCKET)
        except FileNotFoundError:
            pass
        self.srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        old = os.umask(0o177)
        try:
            self.srv.bind(SUDO_SOCKET)
        finally:
            os.umask(old)
        os.chmod(SUDO_SOCKET, 0o600)
        self.srv.listen(4)
        self.srv.settimeout(0.5)
        self.thread = threading.Thread(target=self.serve, daemon=True)
        self.thread.start()

    def env(self, env):
        return dict(env, IRICE_SUDO_SOCKET=SUDO_SOCKET, IRICE_SUDO_TOKEN=self.token)

    def serve(self):
        while not self.stop.is_set():
            try:
                conn, _ = self.srv.accept()
            except socket.timeout:
                continue
            except OSError:
                return
            threading.Thread(target=self.one, args=(conn,), daemon=True).start()

    def one(self, conn):
        def send(obj):
            try:
                conn.sendall((json.dumps(obj) + "\n").encode())
            except OSError:
                pass
        with conn:
            conn.settimeout(None)
            try:
                uid = struct.unpack("3i", conn.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12))[1]
                req = json.loads(conn.makefile("rb").readline() or b"{}")
            except (OSError, ValueError):
                return
            if uid != os.getuid() or not hmac.compare_digest(str(req.get("token", "")), self.token):
                send({"out": "irice-sudo: not this update's\n"})
                send({"exit": 1})
                return
            with self.lock:
                try:
                    self.chan.sendall((json.dumps({"argv": req.get("argv"), "cwd": req.get("cwd")}) + "\n").encode())
                except OSError:
                    send({"out": "irice-sudo: the update's root part has stopped\n"})
                    send({"exit": 1})
                    return
                # Everything up to its exit code, even if irice-sudo went away
                # (so the next answer is the next request's)
                for line in self.answers:
                    try:
                        conn.sendall(line)
                    except OSError:
                        pass
                    try:
                        if "exit" in json.loads(line):
                            return
                    except ValueError:
                        pass
                send({"out": "irice-sudo: the update's root part has stopped\n"})
                send({"exit": 1})

    def close(self):
        self.stop.set()
        self.srv.close()
        try:
            os.unlink(SUDO_SOCKET)
        except FileNotFoundError:
            pass


def end_root(chan):
    """Closes the root part's channel (it ends) and waits for it."""
    global root_proc
    if chan is not None:
        try:
            chan.shutdown(socket.SHUT_RDWR)
        except OSError:
            pass
        chan.close()
    if root_proc is not None:
        root_proc.wait()
        root_proc = None


# ── The real run ────────────────────────────────────────────────────────
# The root part: irice-root.py update. Exit codes say which step failed
# (20 snapshot, 21 system, 22 install, 23 remove); "@@step" lines mark
# where it is, "@@root-done" that it's waiting for the AUR step.
# A package name: never starting with "-" (it would pass as a pacman option)
NAME = re.compile(r"^[a-z0-9@_+][a-z0-9@._+-]*$")


def aur_command(helper, plan):
    if plan.get("aur_install"):
        cmd = [helper, "-S", "--needed", "--noconfirm", "--sudo", SHIM]
        cmd += ["--answerdiff", "None", "--answerclean", "None", "--answeredit", "None"] if helper == "yay" else ["--skipreview"]
        return cmd + ["--"] + plan["aur_install"]
    cmd = [helper, "-Sua", "--noconfirm", "--sudo", SHIM]
    cmd += ["--answerdiff", "None", "--answerclean", "None", "--answeredit", "None"] if helper == "yay" else ["--skipreview"]
    if plan["ignore"]:
        cmd += ["--ignore", ",".join(plan["ignore"])]
    return cmd


def real_run(plan):
    if os.path.exists(DBLOCK):
        busy = subprocess.run(["pgrep", "-x", "pacman|yay|paru|pamac|octopi"], capture_output=True).returncode == 0
        return finish("error", "Another update is running",
                      "Another package manager is using the package database. Wait until it's done, then try again." if busy
                      else f"pacman's lock file is left over from an interrupted update. Remove it with: sudo rm {DBLOCK}")
    if not shutil.which("pkexec"):
        return finish("error", "Can't ask for your password", "pkexec isn't installed (it comes with the polkit package).")
    env = dict(os.environ, LANG="C", LC_ALL="C")
    helper = aur_helper() if "aur" in plan["steps"] else ""
    if not helper and plan.get("aur_install"):
        return finish("error", "Needs an AUR helper", "These come from the AUR (" + ", ".join(plan["aur_install"]) +
                      "), which needs yay or paru. Install one (sudo pacman -S yay, or from the AUR), then try again.")
    if not helper and all(x == "aur" for x in plan["steps"]):
        return finish("done", "Updated")  # (nothing for root to do)
    if cancel_asked:
        return finish("cancelled", "Update cancelled")
    # The root part, even for the AUR alone: its pacman commands go through it
    chan, theirs = socket.socketpair()
    try:
        code = stream_root(["pkexec"] + root_command(plan, bool(helper)), env=env, stdin=theirs)
        theirs.close()
        if code in (126, 127) and not authed:
            # 126: the dialog was dismissed (or Cancel stopped it); 127: not authorised
            return finish("cancelled", "Update cancelled") if (code == 126 or cancel_asked) else \
                finish("error", "Not allowed", "Your account isn't allowed to install updates (polkit refused).")
        if code == 20:
            return finish("error", "The snapshot didn't work",
                          "Nothing was updated. " + tail(3) + "\nFix the snapshot tool, or try again.")
        if code == 21:
            text = open(LOG, errors="replace").read()
            if plan["ignore"] and re.search(r"could not satisfy dependencies|unable to satisfy|breaks dependency", text):
                return finish("error", "Keeping Hyprland back clashes with other updates",
                              "Other updates need the newer Hyprland libraries. Choose Update anyway, or update later. " + tail(3))
            return finish("error", "The system update didn't finish", tail())
        if code == 22:
            return finish("error", "iRice's packages didn't install", "Your system is updated. " + tail())
        if code == 23:
            return finish("error", "Unused packages weren't removed",
                          "Everything else is done. Something else may still need them: " + tail(3))
        if code != 0:
            return finish("error", "The update stopped", tail())
        if helper:
            out("@@step aur")
            parse("@@step aur")
            broker = Broker(chan)
            try:
                code = stream(aur_command(helper, plan), aur=True, env=broker.env(env))
            finally:
                broker.close()
            if code != 0:
                return finish("error", "The AUR updates didn't finish",
                              "Updates from your distribution are installed. " + tail())
    finally:
        theirs.close()
        end_root(chan)
    finish("done", "Updated")


def background():
    """The desktop comes first: this job and everything it starts (pacman,
    AUR builds compiling for minutes, even as root: the priority carries
    through pkexec) get CPU and disk only when the shell and apps don't need
    them, so animations don't stutter while an update runs."""
    try:
        os.nice(15)
    except OSError:
        pass
    if shutil.which("ionice"):
        subprocess.run(["ionice", "-c", "3", "-p", str(os.getpid())], capture_output=True)


def run(fake, plan):
    global log
    os.makedirs(DIR, exist_ok=True)
    lockf = open(LOCK, "w")
    try:
        fcntl.flock(lockf, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        print("already running", file=sys.stderr)
        return 1
    log = open(LOG, "w")
    background()
    for f in (AUTH, os.path.join(DIR, "cancel")):
        try:
            os.unlink(f)
        except FileNotFoundError:
            pass
    state["steps"] = plan["steps"]
    state["ignore"] = plan["ignore"]
    state["then"] = plan["then"]  # what the shell does after (the iRice part)
    state["pidStart"] = start_time(os.getpid())
    save()

    def on_term(*_):
        # Cancel: before the password, stop asking for it (pkexec, still ours,
        # exits 126 and the run reports "cancelled"). Once the root part has
        # started, it's ignored: the update finishes and says so. This job
        # never exits from here while pkexec runs.
        global cancel_asked
        if authed:
            return
        cancel_asked = True
        p = asking
        if p is not None and p.poll() is None:
            try:
                os.kill(p.pid, signal.SIGTERM)  # (if pkexec just became root, this is refused: harmless)
            except OSError:
                pass
        elif p is None and state["phase"] in ("starting",):
            finish("cancelled", "Update cancelled")
            sys.exit(0)
    signal.signal(signal.SIGTERM, signal.SIG_IGN)  # only Cancel (SIGUSR1) stops it, and only before the password
    signal.signal(signal.SIGUSR1, on_term)  # (cancel)
    signal.signal(signal.SIGHUP, signal.SIG_IGN)  # the shell going away is no reason to stop
    try:
        fake_run(plan) if fake else real_run(plan)
    except Exception as e:  # anything unexpected still ends in a readable state
        out(f"sysupdate: {e!r}")
        finish("error", "The update stopped", str(e))
    finally:
        log.close()
    return 0


def cancel():
    try:
        with open(STATUS) as f:
            st = json.load(f)
    except (OSError, ValueError):
        return 1
    if st.get("state") != "running" or st.get("phase") not in ("starting", "auth"):
        print("can't cancel now", file=sys.stderr)
        return 1
    # Only a job that's really running: it holds the lock, and the pid in the
    # status is still that job (not a reused pid)
    try:
        with open(LOCK, "a") as lf:
            try:
                fcntl.flock(lf, fcntl.LOCK_EX | fcntl.LOCK_NB)
                fcntl.flock(lf, fcntl.LOCK_UN)
                print("no update is running", file=sys.stderr)
                return 1
            except BlockingIOError:
                pass  # held: a job is running
    except OSError:
        return 1
    pid = int(st.get("pid", 0) or 0)
    if pid <= 1 or not st.get("pidStart") or start_time(pid) != st.get("pidStart"):
        print("the update job isn't running any more", file=sys.stderr)
        return 1
    # The job stops the password prompt itself (and ignores this once root runs)
    try:
        os.kill(pid, signal.SIGUSR1)
    except OSError:
        return 1
    return 0


AUTOSNAP = {"snapper": "snap-pac", "timeshift": "timeshift-autosnap"}


def installed(package):
    try:
        return subprocess.run(["pacman", "-Q", "--", package], capture_output=True, timeout=10).returncode == 0
    except (OSError, subprocess.TimeoutExpired):
        return False


def other_power_manager():
    for name in ("tlp", "auto-cpufreq", "tuned-ppd", "tuned"):
        if installed(name):
            return name
        try:
            if subprocess.run(["systemctl", "is-active", "--quiet", name], timeout=10).returncode == 0:
                return name
        except (OSError, subprocess.TimeoutExpired):
            pass
    return ""


def plan_of(a, fake=False):
    """The steps from the arguments, package names checked."""
    def names(flag):
        v = a[a.index(flag) + 1] if flag in a and a.index(flag) + 1 < len(a) else ""
        out = [n for n in v.split(",") if n]
        bad = [n for n in out if not NAME.match(n)]
        if bad:
            raise SystemExit("not a package name: " + ", ".join(bad))
        return out
    snap = a[a.index("--snapshot") + 1] if "--snapshot" in a else ""
    p = {"snapshot": snap if snap in ("snapper", "timeshift") else "", "system": "--system" in a,
         "ignore": names("--ignore"), "install": names("--install"), "remove": names("--remove"),
         "aur_install": names("--aur-install")}
    # A snapshot tool's pacman hook already takes one around each pacman run
    # (snap-pac for snapper, timeshift-autosnap): a second would only take space
    if p["snapshot"] and not fake and installed(AUTOSNAP[p["snapshot"]]):
        p["snapshot"] = ""
    # Not power-profiles-daemon next to another power manager (they'd fight;
    # TLP's package conflicts with it): tools/irice-packages.sh leaves it out too
    if "power-profiles-daemon" in p["install"] and not fake and other_power_manager():
        p["install"].remove("power-profiles-daemon")
    then = a[a.index("--then") + 1] if "--then" in a and a.index("--then") + 1 < len(a) else "null"
    try:
        p["then"] = json.loads(then)
    except ValueError:
        p["then"] = None
    # The root helper's own files, when they changed (never in the sandbox)
    p["setup"] = not fake and not helper_current()
    p["steps"] = ([("snapshot")] if p["snapshot"] else []) + (["setup"] if p["setup"] else []) + (["system"] if p["system"] else []) + \
        (["packages"] if p["install"] else []) + (["remove"] if p["remove"] else []) + (["aur"] if "--aur" in a or p["aur_install"] else [])
    return p


if __name__ == "__main__":
    a = sys.argv[1:]
    if not a or a[0] not in ("run", "cancel", "helper-status"):
        print(__doc__, file=sys.stderr)
        sys.exit(2)
    if a[0] == "cancel":
        sys.exit(cancel())
    if a[0] == "helper-status":
        current = helper_current()
        print(json.dumps({"current": current}))
        sys.exit(0 if current else 1)
    fake = "--fake" in a or os.environ.get("IRICE_FAKE_SYSUPDATE") == "1" or os.environ.get("IRICE_SANDBOX") == "1"
    sys.exit(run(fake, plan_of(a, fake)))
