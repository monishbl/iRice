#!/usr/bin/env python3
"""The gate's recorder (tools/gate), run on the host beside one arena:

  sampler.py <runtime dir> <tessera-ui.log> <out dir>

- samples.tsv: every 0.5 s the arena's shell process (tessera-ui --standalone
  whose XDG_RUNTIME_DIR is this arena's): wall time, pid, CPU ticks
  (utime+stime, /proc/<pid>/stat), VmRSS and RssAnon kB (/proc/<pid>/status), and every
  3 s Pss/Rss kB from /proc/<pid>/smaps_rollup (that walks the page tables,
  so not every sample). A pid change is a restart.
- log.tsv: the shell's log, every line with the wall time it was read
  (polled every 0.1 s; follows launch-shell.sh's rotation to .1 by inode).
- every sample also has the machine's 1-minute load average (load1) and
  the tasks running at that moment (running: other builds are noise).

Exact PIDs only: it reads /proc, never signals anything. Stops on SIGTERM.
"""
import os
import signal
import sys
import time

rt, logpath, out = sys.argv[1:4]
os.makedirs(out, exist_ok=True)
stop = False


def on_term(*_):
    global stop
    stop = True


signal.signal(signal.SIGTERM, on_term)
signal.signal(signal.SIGINT, on_term)
want_env = ("XDG_RUNTIME_DIR=" + rt).encode()


def is_shell(pid):
    try:
        with open(f"/proc/{pid}/comm", "rb") as f:
            comm = f.read().strip()
        if comm not in (b"tessera-ui", b"tessera-ui.real"):
            return False
        with open(f"/proc/{pid}/cmdline", "rb") as f:
            if b"--standalone" not in f.read().split(b"\0"):
                return False
        with open(f"/proc/{pid}/environ", "rb") as f:
            return want_env in f.read().split(b"\0")
    except OSError:
        return False


def find_shell():
    for d in os.listdir("/proc"):
        if d.isdigit() and is_shell(int(d)):
            return int(d)
    return None


def ticks(pid):
    with open(f"/proc/{pid}/stat") as f:
        s = f.read()
    rest = s[s.rindex(")") + 2:].split()
    # (fields 14 and 15 of stat; rest[0] is field 3)
    return int(rest[11]) + int(rest[12])


def vmrss(pid):
    """(VmRSS, RssAnon) kB. RssAnon is what the budgets use: RssFile (the
    180 MB binary's pages, libraries, the GPU driver's mappings) differs by
    tens of MB between two runs of the same binary"""
    rss = anon = 0
    with open(f"/proc/{pid}/status") as f:
        for line in f:
            if line.startswith("VmRSS:"):
                rss = int(line.split()[1])
            elif line.startswith("RssAnon:"):
                anon = int(line.split()[1])
    return rss, anon


def rollup(pid):
    rss = pss = 0
    with open(f"/proc/{pid}/smaps_rollup") as f:
        for line in f:
            if line.startswith("Rss:"):
                rss = int(line.split()[1])
            elif line.startswith("Pss:"):
                pss = int(line.split()[1])
    return rss, pss


def load1():
    """the machine's 1-minute load average (other builds make a run unreliable)"""
    """…and the tasks running right now (loadavg's 4th field, less this
    reader; paused builds don't count): "load1<TAB>running" """
    try:
        with open("/proc/loadavg") as f:
            p = f.read().split()
        return f"{p[0]}\t{int(p[3].split('/')[0]) - 1}"
    except (OSError, IndexError, ValueError):
        return "\t"


class Follower:
    """Reads a log that launch-shell.sh rotates (mv log log.1; a new log)"""

    def __init__(self, path, sink):
        self.path, self.sink = path, sink
        self.f, self.ino, self.buf = None, None, b""

    def _open(self):
        try:
            f = open(self.path, "rb")
        except OSError:
            return False
        self.f, self.ino, self.buf = f, os.fstat(f.fileno()).st_ino, b""
        return True

    def _drain(self, now):
        data = self.f.read()
        if not data:
            return
        self.buf += data
        *lines, self.buf = self.buf.split(b"\n")
        for ln in lines:
            self.sink.write(f"{now:.3f}\t".encode() + ln.replace(b"\t", b" ") + b"\n")

    def poll(self, now):
        if self.f is None and not self._open():
            return
        self._drain(now)
        try:
            ino = os.stat(self.path).st_ino
        except OSError:
            return
        if ino != self.ino:
            # (rotated: the rest of the old one, then the new from its start)
            self._drain(now)
            if self.buf:
                self.sink.write(f"{now:.3f}\t".encode() + self.buf + b"\n")
            self.f.close()
            self.f = None
            if self._open():
                self._drain(now)


samples = open(os.path.join(out, "samples.tsv"), "w", buffering=1)
samples.write("t\tpid\tticks\tvmrss_kb\trss_kb\tpss_kb\tload1\trunning\tanon_kb\n")
logsink = open(os.path.join(out, "log.tsv"), "wb", buffering=0)
fol = Follower(logpath, logsink)
pid, next_sample, next_rollup, next_scan = None, 0.0, 0.0, 0.0
while not stop:
    now = time.time()
    fol.poll(now)
    if now >= next_sample:
        next_sample = now + 0.5
        if pid is not None and not os.path.exists(f"/proc/{pid}"):
            pid = None
        if pid is None or now >= next_scan:
            # (a restart: the old pid gone, or another shell beside it)
            next_scan = now + 5.0
            if pid is None or not is_shell(pid):
                pid = find_shell()
        if pid is not None:
            try:
                tk, (vr, an) = ticks(pid), vmrss(pid)
                rr = pp = ""
                if now >= next_rollup:
                    next_rollup = now + 3.0
                    rr, pp = rollup(pid)
                samples.write(f"{now:.3f}\t{pid}\t{tk}\t{vr}\t{rr}\t{pp}\t{load1()}\t{an}\n")
            except (OSError, ValueError, IndexError):
                pid = None
        else:
            samples.write(f"{now:.3f}\t\t\t\t\t\t{load1()}\t\n")
    time.sleep(0.1)
fol.poll(time.time())
