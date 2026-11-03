"""Shared test setup."""

import os
import tempfile

# Tests never write into the real session: the Quick Share debug log (and
# anything else keyed to $XDG_RUNTIME_DIR) goes to a throwaway directory, and
# a debug flag the user set for a real phone test doesn't turn tests' traffic
# into lines in their log.
_runtime = tempfile.mkdtemp(prefix="irice-tests-run-")
os.environ["XDG_RUNTIME_DIR"] = _runtime
os.environ.pop("IRICE_QS_DEBUG", None)
# Nor the real system's configuration (firewall, BlueZ): the sharing checks
# read /etc and /usr under this, empty unless a test fills it
os.environ["IRICE_SYSTEM_ROOT"] = tempfile.mkdtemp(prefix="irice-tests-root-")

# Nor the real screen: a shell or preview a test starts (on purpose, or by a
# launcher that found the real Quickshell) must never draw on the person's
# display. Seen once: test shells left running over the desktop. Offscreen,
# with no display or compositor to find
for _var in ("WAYLAND_DISPLAY", "WAYLAND_SOCKET", "DISPLAY", "HYPRLAND_INSTANCE_SIGNATURE"):
    os.environ.pop(_var, None)
os.environ["QT_QPA_PLATFORM"] = "offscreen"

_started = None


def pytest_sessionstart(session):
    global _started
    import time
    _started = time.time()


def pytest_sessionfinish(session, exitstatus):
    """Stops shells a test started and left running (anything named
    irice-shell or qs, started during the run, whose runtime folder is a
    temporary one: never the person's own shell in /run/user)."""
    import signal
    import time
    tmp = tempfile.gettempdir().rstrip("/") + "/"
    boot = time.time() - float(open("/proc/uptime").read().split()[0])
    hz = os.sysconf("SC_CLK_TCK")
    for pid in os.listdir("/proc"):
        if not pid.isdigit():
            continue
        try:
            with open(f"/proc/{pid}/comm") as f:
                comm = f.read().strip()
            if comm not in ("irice-shell", "qs", "quickshell"):
                continue
            with open(f"/proc/{pid}/environ", "rb") as f:
                env = dict(e.split(b"=", 1) for e in f.read().split(b"\0") if b"=" in e)
            runtime = env.get(b"XDG_RUNTIME_DIR", b"").decode(errors="replace")
            with open(f"/proc/{pid}/stat") as f:
                started = boot + int(f.read().rsplit(")", 1)[1].split()[19]) / hz
            if runtime.startswith(tmp) and not runtime.startswith("/tmp/irice-rt") and _started and started >= _started - 1:
                os.kill(int(pid), signal.SIGTERM)
        except (OSError, ValueError, IndexError):
            continue
