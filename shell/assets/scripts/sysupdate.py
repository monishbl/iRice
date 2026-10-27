#!/usr/bin/env python3
"""The system part of Software Update's one Update button, without a
terminal: a snapshot, pacman -Syu, iRice's missing packages, removing the
ones it dropped, then AUR updates with yay or paru. The iRice part follows,
started by the shell once this is done (services/SoftwareUpdate.qml).

    sysupdate.py run [--fake] [--system] [--snapshot snapper|timeshift]
                     [--ignore a,b] [--install a,b] [--remove a,b] [--aur]
    sysupdate.py cancel         stop it while it's still waiting for the password

Everything that needs root is one pkexec (one password): an inline script
(argv, never a file that could be swapped) with the steps in order:
snapshot → pacman -Syu [--ignore held] → pacman -S --needed <missing> →
pacman -Rns <dropped>. It stops at the first failure. AUR updates run after
it as you (yay/paru --sudo pkexec: a second prompt only when there are any).

Everything goes to $XDG_RUNTIME_DIR/irice-sysupdate/:
    status.json  what's happening, rewritten whole on each change (the shell
                 reads it: Updates.qml); survives shell restarts
    log          the full output, as a terminal would show it
    lock         held (flock) for the whole run: one update at a time; a
                 status that says "running" with the lock free was interrupted
    auth.pid     the pkexec waiting for the password (what Cancel stops)

Root comes from pkexec, so the password is asked by iRice's own dialog (its
polkit agent). The job must run in the login session (setsid from the shell,
not systemd-run --user): polkit finds the agent through the session.
Cancel only works while pkexec waits for the password; once pacman runs as
root it can't be signalled by us, and stopping a transaction midway would
break the system anyway.

--fake (or IRICE_FAKE_SYSUPDATE=1, and always in the dev sandbox) replays a
canned pacman run with delays: the whole UI, nothing installed.
"""
import fcntl
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import time

RUN = os.environ.get("XDG_RUNTIME_DIR") or f"/run/user/{os.getuid()}"
DIR = os.path.join(RUN, "irice-sysupdate")
STATUS = os.path.join(DIR, "status.json")
LOG = os.path.join(DIR, "log")
LOCK = os.path.join(DIR, "lock")
AUTH = os.path.join(DIR, "auth.pid")
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
}
log = None
cancel_asked = False


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


WEIGHT = {"snapshot": 6, "system": 64, "packages": 12, "remove": 4, "aur": 14}


def begin(step):
    """A step starts: its share of the percentage, and its name."""
    global span
    steps = state["steps"] or [step]
    total = sum(WEIGHT.get(x, 10) for x in steps)
    before = sum(WEIGHT.get(x, 10) for x in steps[:steps.index(step)]) if step in steps else 0
    span = (100 * before / total, 100 * (before + WEIGHT.get(step, 10)) / total)
    text = {"snapshot": "Taking a snapshot", "system": "Checking for updates", "packages": "Installing iRice's packages",
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


def stream(cmd, aur=False, env=None):
    """Runs cmd, logging and parsing each line. Returns the exit code."""
    out("$ " + " ".join(cmd))
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL,
                         env=env, text=True, errors="replace", bufsize=1)
    if cmd[0] == "pkexec" or "pkexec" in cmd:
        with open(AUTH, "w") as f:
            f.write(str(p.pid))
    first = True
    for line in p.stdout:
        line = line.rstrip("\n")
        # \r progress bars (a tty-less pacman shouldn't print them; just in case)
        line = line.split("\r")[-1]
        if first and state["phase"] == "auth":
            set_(phase="sync", text="Checking for updates")
        first = False
        out(line)
        parse(line, aur)
    code = p.wait()
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
    set_(phase="auth", text="Waiting for your password")
    time.sleep(1.5)
    if cancel_asked:
        return finish("cancelled", "Update cancelled")
    out("(simulated: nothing is installed)")
    for step in plan["steps"]:
        out(f"@@step {step}")
        parse(f"@@step {step}")
        for delay, line in (FAKE if step == "system" else FAKE_STEP.get(step, [])):
            time.sleep(delay)
            out(line)
            parse(line)
    finish("done", "Updated")


# ── The real run ────────────────────────────────────────────────────────
# The root part, as one inline script: $1 snapshot tool, $2 held packages,
# $3 packages to install, $4 packages to remove, $5 1 = pacman -Syu.
# Exit codes say which step failed (20 snapshot, 21 system, 22 install,
# 23 remove); "@@step" lines mark where it is.
ROOT_SCRIPT = r"""
step() { echo "@@step $1"; }
if [ "$1" = snapper ]; then
    step snapshot
    snapper -c root create -d "Before iRice update" -c number || exit 20
elif [ "$1" = timeshift ]; then
    step snapshot
    timeshift --create --comments "Before iRice update" --scripted || exit 20
fi
if [ "$5" = 1 ]; then
    step system
    if [ -n "$2" ]; then pacman -Syu --noconfirm --ignore "$2" || exit 21
    else pacman -Syu --noconfirm || exit 21; fi
fi
if [ -n "$3" ]; then
    step packages
    pacman -S --needed --noconfirm $(echo "$3" | tr , ' ') || exit 22
fi
if [ -n "$4" ]; then
    step remove
    pacman -Rns --noconfirm $(echo "$4" | tr , ' ') || exit 23
fi
exit 0
"""
NAME = re.compile(r"^[a-z0-9@._+-]+$")


def real_run(plan):
    if os.path.exists(DBLOCK):
        busy = subprocess.run(["pgrep", "-x", "pacman|yay|paru|pamac|octopi"], capture_output=True).returncode == 0
        return finish("error", "Another update is running",
                      "Another package manager is using the package database. Wait until it's done, then try again." if busy
                      else f"pacman's lock file is left over from an interrupted update. Remove it with: sudo rm {DBLOCK}")
    if not shutil.which("pkexec"):
        return finish("error", "Can't ask for your password", "pkexec isn't installed (it comes with the polkit package).")
    env = dict(os.environ, LANG="C", LC_ALL="C")
    root_steps = [x for x in plan["steps"] if x != "aur"]
    if root_steps:
        set_(phase="auth", text="Waiting for your password")
        code = stream(["pkexec", "/bin/sh", "-c", ROOT_SCRIPT, "sh", plan["snapshot"], ",".join(plan["ignore"]),
                       ",".join(plan["install"]), ",".join(plan["remove"]), "1" if plan["system"] else "0"], env=env)
        if code in (126, 127):
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
    if "aur" in plan["steps"]:
        helper = aur_helper()
        if helper:
            out("@@step aur")
            parse("@@step aur")
            cmd = [helper, "-Sua", "--noconfirm", "--sudo", "pkexec"]
            cmd += ["--answerdiff", "None", "--answerclean", "None", "--answeredit", "None"] if helper == "yay" else ["--skipreview"]
            if plan["ignore"]:
                cmd += ["--ignore", ",".join(plan["ignore"])]
            code = stream(cmd, aur=True, env=env)
            if code != 0:
                return finish("error", "The AUR updates didn't finish",
                              "Updates from your distribution are installed. " + tail())
    finish("done", "Updated")


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
    for f in (AUTH, os.path.join(DIR, "cancel")):
        try:
            os.unlink(f)
        except FileNotFoundError:
            pass
    state["steps"] = plan["steps"]
    state["ignore"] = plan["ignore"]
    state["then"] = plan["then"]  # what the shell does after (the iRice part)
    save()

    def on_term(*_):
        global cancel_asked
        cancel_asked = True
        if state["phase"] in ("starting", "auth"):
            finish("cancelled", "Update cancelled")
            sys.exit(0)
    signal.signal(signal.SIGTERM, on_term)
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
    # pkexec waiting for the password still runs as us: stopping it ends that
    # step (exit 126/127), and the job reports "cancelled"
    try:
        with open(AUTH) as f:
            os.kill(int(f.read().strip()), signal.SIGTERM)
    except (OSError, ValueError):
        pass
    try:
        os.kill(int(st.get("pid", 0)), signal.SIGUSR1)
    except (OSError, ValueError):
        pass
    return 0


def plan_of(a):
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
         "ignore": names("--ignore"), "install": names("--install"), "remove": names("--remove")}
    then = a[a.index("--then") + 1] if "--then" in a and a.index("--then") + 1 < len(a) else "null"
    try:
        p["then"] = json.loads(then)
    except ValueError:
        p["then"] = None
    p["steps"] = ([("snapshot")] if p["snapshot"] else []) + (["system"] if p["system"] else []) + \
        (["packages"] if p["install"] else []) + (["remove"] if p["remove"] else []) + (["aur"] if "--aur" in a else [])
    return p


if __name__ == "__main__":
    a = sys.argv[1:]
    if not a or a[0] not in ("run", "cancel"):
        print(__doc__, file=sys.stderr)
        sys.exit(2)
    if a[0] == "cancel":
        sys.exit(cancel())
    fake = "--fake" in a or os.environ.get("IRICE_FAKE_SYSUPDATE") == "1" or os.environ.get("IRICE_SANDBOX") == "1"
    sys.exit(run(fake, plan_of(a)))
