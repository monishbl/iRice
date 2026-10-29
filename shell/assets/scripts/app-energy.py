#!/usr/bin/env python3
"""Which apps used the CPU over the last hour (Settings → Battery & Power).

Every 5 minutes the shell runs `app-energy.py sample`: it reads each
process's total CPU time from /proc, works out how much each one used since
the last look, adds that up by app and keeps an hour of these 5-minute
amounts in $XDG_STATE_HOME/irice/app-energy.json. While the page is open,
`app-energy.py report` adds the time since the last look (without saving)
and prints the apps that used the most, as JSON.

A process belongs to an app by, in order:
  1. its systemd scope (app-<desktop id>-<n>.scope, how launchers start apps)
  2. the app window it was started from (an ancestor that owns a window in
     `hyprctl clients`), e.g. a browser's helper processes
  3. otherwise the command name of its top ancestor below the session
A process is told apart from a later one that got its pid by its start time.
"""
import json
import os
import subprocess
import sys
import tempfile
import time

HOUR = 3600
TICK = os.sysconf("SC_CLK_TCK")


def state_path():
    base = os.environ.get("XDG_STATE_HOME") or os.path.expanduser("~/.local/state")
    return os.path.join(base, "irice", "app-energy.json")


def read_procs(root="/proc"):
    """{pid: (ppid, start, cpu_ticks, comm, cgroup)} for this user's processes."""
    uid = os.getuid()
    out = {}
    for name in os.listdir(root):
        if not name.isdigit():
            continue
        d = os.path.join(root, name)
        try:
            if os.stat(d).st_uid != uid:
                continue
            with open(d + "/stat") as f:
                raw = f.read()
            with open(d + "/cgroup") as f:
                cg = f.read().strip().rsplit("/", 1)[-1]
        except OSError:
            continue
        comm = raw[raw.find("(") + 1:raw.rfind(")")]
        f = raw[raw.rfind(")") + 2:].split()
        out[int(name)] = (int(f[1]), int(f[19]), int(f[11]) + int(f[12]), comm, cg)
    return out


def scope_app(cgroup):
    """Desktop id from app-<id>-<n>.scope / app-<id>@<n>.service, else None."""
    for prefix in ("app-flatpak-", "app-"):
        if cgroup.startswith(prefix) and (cgroup.endswith(".scope") or cgroup.endswith(".service")):
            core = cgroup[len(prefix):].rsplit(".", 1)[0]
            core = core.split("@", 1)[0]
            head, _, tail = core.rpartition("-")
            return head if tail.isdigit() and head else core
    return None


def window_pids():
    """{pid: class} of processes owning a window."""
    try:
        r = subprocess.run(["hyprctl", "-j", "clients"], capture_output=True, text=True, timeout=3)
        return {c["pid"]: c.get("class") or "" for c in json.loads(r.stdout or "[]") if c.get("pid", 0) > 1}
    except (OSError, ValueError, subprocess.SubprocessError):
        return {}


def group(procs, windows):
    """{pid: "app:<id>" | "class:<class>" | "proc:<comm>"}"""
    keys = {}

    def key_of(pid, seen=0):
        if pid in keys:
            return keys[pid]
        p = procs.get(pid)
        if not p or seen > 64:
            return None
        app = scope_app(p[4])
        if app:
            k = "app:" + app
        elif pid in windows and windows[pid]:
            k = "class:" + windows[pid]
        else:
            parent = procs.get(p[0])
            up = key_of(p[0], seen + 1) if parent else None
            # A parent that's only "the session" (a top ancestor by name) doesn't
            # own its children: each top process below it is its own group
            k = up if up and not up.startswith("proc:") else "proc:" + p[3]
        keys[pid] = k
        return k

    for pid in procs:
        key_of(pid)
    return keys


def deltas(prev, procs, keys, since):
    """CPU ticks per group since the previous look. A process seen before
    counts its growth; a new one (started after the last look) counts all of
    its time; one that reused a pid is a new one."""
    out = {}
    boot = time.time() - float(open("/proc/uptime").read().split()[0])
    for pid, (ppid, start, ticks, comm, cg) in procs.items():
        old = prev.get(str(pid))
        if old and old[0] == start:
            used = ticks - old[1]
        elif boot + start / TICK >= since:
            used = ticks
        else:
            continue  # was running before the first look: its past isn't this hour's
        if used > 0:
            k = keys.get(pid) or "proc:" + comm
            out[k] = out.get(k, 0) + used
    return out


def load():
    try:
        with open(state_path()) as f:
            s = json.load(f)
        return s if isinstance(s, dict) else {}
    except (OSError, ValueError):
        return {}


def save(s):
    path = state_path()
    os.makedirs(os.path.dirname(path), mode=0o700, exist_ok=True)
    fd, tmp = tempfile.mkstemp(dir=os.path.dirname(path), prefix=".app-energy.")
    with os.fdopen(fd, "w") as f:
        json.dump(s, f, separators=(",", ":"))
    os.replace(tmp, path)


def look(state, now):
    procs = read_procs()
    keys = group(procs, window_pids())
    d = deltas(state.get("pids", {}), procs, keys, state.get("at", now))
    pids = {str(pid): [p[1], p[2]] for pid, p in procs.items()}
    return d, pids


def top(ring, extra, now, n=5):
    total = {}
    for entry in ring + [{"t": now, "apps": extra}]:
        if now - entry["t"] <= HOUR:
            for k, v in entry["apps"].items():
                total[k] = total.get(k, 0) + v
    ranked = sorted(total.items(), key=lambda kv: -kv[1])
    return [{"key": k, "seconds": round(v / TICK, 1)} for k, v in ranked[:n] if v / TICK >= 1]


def main(argv):
    cmd = argv[1] if len(argv) > 1 else "report"
    now = time.time()
    state = load()
    first = "at" not in state
    d, pids = look(state, now)
    ring = [e for e in state.get("ring", []) if now - e.get("t", 0) <= HOUR]
    if cmd == "sample":
        if not first:
            ring.append({"t": now, "apps": d})
        save({"at": now, "pids": pids, "ring": ring})
    elif cmd == "report":
        span = min(HOUR, now - ring[0]["t"] + 300) if ring else (now - state.get("at", now))
        print(json.dumps({"apps": top(ring, {} if first else d, now), "minutes": round(span / 60)}))
    elif cmd == "clear":
        try:
            os.remove(state_path())
        except OSError:
            pass
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
