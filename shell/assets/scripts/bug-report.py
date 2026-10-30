#!/usr/bin/env python3
"""A bug report for iRice (Settings → Report a Bug, services/BugReport.qml).

What leaves the computer is only what the person saw in the preview and left
ticked, and it's scrubbed first: the log and system info go through scrub(),
which takes out home paths, the user and computer names, email addresses,
anything that looks like a key or token, IP and MAC addresses, and the names
of Wi-Fi networks and Bluetooth devices this computer knows (those are
personal: a home network's name, "Sam's AirPods").

  bug-report.py gather <shell dir>   the scrubbed log and system info, as JSON
  bug-report.py send                 a report (JSON on stdin) to the server
  bug-report.py text                 the same report as plain text (stdin JSON)
  bug-report.py github-url           a prefilled GitHub issue link (stdin JSON)

Sending is HTTPS only, at most 2 MB with the screenshot (downscaled to 1600
px on its longest side and re-encoded), and gives up after 30 s.
"""
import base64
import ipaddress
import json
import os
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import urllib.error
import urllib.parse
import urllib.request

ENDPOINT = "https://irice-bugs.monishbl.in/api/bug-reports"
REPO = "monishbl/iRice"
MAX_BYTES = 2 * 1024 * 1024
MAX_SIDE = 1600
LOG_LINES = 500
TIMEOUT = 30


def run(cmd, timeout=5):
    try:
        out = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
        return out.stdout if out.returncode == 0 else ""
    except (OSError, subprocess.SubprocessError):
        return ""


# ── Scrubbing ───────────────────────────────────────────────────────────

def personal_names():
    """Wi-Fi networks and Bluetooth devices this computer knows by name."""
    names = set()
    for line in run(["nmcli", "-t", "-f", "NAME,TYPE", "connection", "show"]).splitlines():
        name, _, kind = line.rpartition(":")
        if "wireless" in kind and name:
            names.add(name.replace("\\:", ":"))
    for line in run(["nmcli", "-t", "-f", "SSID", "device", "wifi", "list", "--rescan", "no"]).splitlines():
        if line.strip():
            names.add(line.strip().replace("\\:", ":"))
    # iwd (without NetworkManager, or under it): its known and nearby networks
    names |= iwd_names(run(["busctl", "--system", "--json=short", "call", "net.connman.iwd", "/",
                            "org.freedesktop.DBus.ObjectManager", "GetManagedObjects"]))
    for line in run(["bluetoothctl", "devices"]).splitlines():
        parts = line.split(" ", 2)  # Device <MAC> <name>
        if len(parts) == 3 and parts[2].strip():
            names.add(parts[2].strip())
    # Too short to tell apart from ordinary words
    return {n for n in names if len(n) >= 3}


def iwd_names(text):
    """The Name of every KnownNetwork and Network in iwd's GetManagedObjects
    answer (busctl --json=short)."""
    try:
        objects = json.loads(text)["data"][0]
    except (ValueError, KeyError, IndexError, TypeError):
        return set()
    names = set()
    for ifaces in objects.values() if isinstance(objects, dict) else []:
        for iface in ("net.connman.iwd.KnownNetwork", "net.connman.iwd.Network"):
            name = ((ifaces or {}).get(iface) or {}).get("Name", {})
            if isinstance(name, dict) and isinstance(name.get("data"), str) and name["data"]:
                names.add(name["data"])
    return names


EMAIL = re.compile(r"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}")
MAC = re.compile(r"\b(?:[0-9A-Fa-f]{2}[:-]){5}[0-9A-Fa-f]{2}\b")
IPV4 = re.compile(r"\b(?:\d{1,3}\.){3}\d{1,3}\b")
# (any run of hex digits and colons with two colons or more; ipaddress decides)
IPV6 = re.compile(r"(?<![\w:.])[0-9A-Fa-f]{0,4}(?::[0-9A-Fa-f]{0,4}){2,7}(?![\w:])")
BEARER = re.compile(r"(?i)\b(bearer|token|key|secret|password|passwd|authorization)\b(\s*[:=]\s*|\s+)(\S+)")
JWT = re.compile(r"\beyJ[\w-]+\.[\w-]+\.[\w-]+")
LONG_HEX = re.compile(r"\b[0-9a-fA-F]{32,}\b")
# (no "/" in it: a long path is not a key)
LONG_B64 = re.compile(r"(?<![\w+=/-])[A-Za-z0-9+_-]{32,}={0,2}(?![\w+=-])")
ANSI = re.compile(r"\x1b\[[0-9;]*[A-Za-z]")


def scrub(text, names=None, home=None, user=None, host=None):
    """Personal details out of `text`, each replaced by what it was (<email>)."""
    home = home if home is not None else os.path.expanduser("~")
    user = user if user is not None else os.environ.get("USER", "")
    host = host if host is not None else socket.gethostname()
    text = ANSI.sub("", text)
    if home and home not in ("/", ""):
        text = text.replace(home, "~")
    text = EMAIL.sub("<email>", text)
    text = JWT.sub("<token>", text)
    text = BEARER.sub(lambda m: m.group(1) + m.group(2) + "<redacted>", text)
    text = MAC.sub("<mac>", text)

    def ip(m):
        try:
            a = ipaddress.ip_address(m.group(0))
        except ValueError:
            return m.group(0)
        return m.group(0) if a.is_loopback or a.is_unspecified else "<ip>"
    text = IPV4.sub(ip, text)
    text = IPV6.sub(ip, text)
    text = LONG_HEX.sub("<hex>", text)
    # Long runs of letters and digits mixed: keys, not words or paths
    text = LONG_B64.sub(lambda m: "<token>" if re.search(r"\d", m.group(0)) and re.search(r"[A-Za-z]", m.group(0)) else m.group(0), text)
    for n in sorted(names or (), key=len, reverse=True):
        text = text.replace(n, "<name>")
    # The account and computer names, as whole words (not inside other words)
    for word, label in ((user, "<user>"), (host, "<host>")):
        if word and len(word) >= 3:
            text = re.sub(r"(?<![\w-])" + re.escape(word) + r"(?![\w-])", label, text)
    return text


# ── Gathering ───────────────────────────────────────────────────────────

def qs_binary():
    """The Quickshell this shell runs on (its log is read with the same one)."""
    try:
        exe = os.readlink(f"/proc/{os.getppid()}/exe")
        if os.path.basename(exe) in ("qs", "quickshell"):
            return exe
    except OSError:
        pass
    return shutil.which("qs") or shutil.which("quickshell") or "qs"


def read_log(shell_dir):
    out = run([qs_binary(), "log", "-p", shell_dir], timeout=10)
    return "\n".join(out.splitlines()[-LOG_LINES:])


def sysinfo(shell_dir):
    info = {}
    repo = os.path.dirname(os.path.abspath(shell_dir.rstrip("/")))
    commit = run(["git", "-C", repo, "rev-parse", "--short", "HEAD"]).strip()
    date = run(["git", "-C", repo, "log", "-1", "--format=%cs"]).strip()
    info["iRice"] = f"{commit} ({date})" if commit else "unknown"
    try:
        hv = json.loads(run(["hyprctl", "version", "-j"]) or "{}")
        info["Hyprland"] = hv.get("tag") or hv.get("version") or "unknown"
    except ValueError:
        info["Hyprland"] = "unknown"
    info["Quickshell"] = (run([qs_binary(), "--version"]).strip().splitlines() or ["unknown"])[0]
    gpus = [l.split(": ", 1)[-1] for l in run(["lspci"]).splitlines() if re.search(r"VGA|3D|Display", l)]
    info["GPU"] = "; ".join(gpus) or "unknown"
    try:
        mons = json.loads(run(["hyprctl", "monitors", "-j"]) or "[]")
        info["Displays"] = ", ".join(f"{m['width']}×{m['height']}@{round(m.get('refreshRate', 0))}Hz×{m.get('scale', 1)}" for m in mons) or "unknown"
    except (ValueError, KeyError, TypeError):
        info["Displays"] = "unknown"
    distro = "unknown"
    try:
        for line in open("/etc/os-release", encoding="utf-8"):
            if line.startswith("PRETTY_NAME="):
                distro = line.split("=", 1)[1].strip().strip('"')
    except OSError:
        pass
    info["System"] = distro
    info["Kernel"] = os.uname().release
    info["Health"] = health_summary()
    return info


def health_summary():
    """What System health finds wrong (health-check.py), in one line; part of
    System info, so it's shown before sending and can be left out with it.
    (The password prompt check is left out: only the shell knows that.)"""
    script = os.path.join(os.path.dirname(os.path.abspath(__file__)), "health-check.py")
    try:
        rows = json.loads(run([sys.executable, "-I", script, "--polkit", "off", "--shell-pid", str(os.getppid())], timeout=5) or "[]")
    except ValueError:
        return "unknown"
    bad = [f"{r['name']}: {r['status']} ({r['detail']})" for r in rows if r.get("status") in ("fail", "warn")]
    return "; ".join(bad) or "all good"


def gather(shell_dir):
    names = personal_names()
    info = {k: scrub(v, names) for k, v in sysinfo(shell_dir).items()}
    return {"log": scrub(read_log(shell_dir), names), "sysinfo": info}


# ── The report ──────────────────────────────────────────────────────────

FIELDS = (("happened", "What happened"), ("steps", "Steps to reproduce"), ("expected", "What you expected"))


def check(report):
    title = (report.get("title") or "").strip()
    if not title:
        raise ValueError("Add a title, then send again.")
    if not (report.get("happened") or "").strip():
        raise ValueError("Say what happened, then send again.")
    return title[:120]


def as_text(report):
    lines = [report.get("title", "").strip(), ""]
    for key, label in FIELDS:
        if (report.get(key) or "").strip():
            lines += [label, report[key].strip(), ""]
    if report.get("includeInfo") and report.get("sysinfo"):
        lines += ["System"] + [f"  {k}: {v}" for k, v in report["sysinfo"].items()] + [""]
    if report.get("includeLog") and report.get("log"):
        lines += ["Log", report["log"], ""]
    return "\n".join(lines).rstrip() + "\n"


def github_url(report):
    body = []
    for key, label in FIELDS:
        if (report.get(key) or "").strip():
            body += [f"### {label}", report[key].strip(), ""]
    if report.get("includeInfo") and report.get("sysinfo"):
        body += ["### System", "| | |", "|---|---|"] + [f"| {k} | {v} |" for k, v in report["sysinfo"].items()]
    q = urllib.parse.urlencode({"title": report.get("title", ""), "labels": "bug-report", "body": "\n".join(body)[:6000]})
    return f"https://github.com/{REPO}/issues/new?{q}"


def screenshot(path, budget):
    """The screenshot, at most MAX_SIDE px and `budget` bytes, as (mime, base64)."""
    if not path or not os.path.isfile(path):
        return None
    magick = shutil.which("magick") or shutil.which("convert")
    data, mime = None, "image/png"
    if magick:
        with tempfile.TemporaryDirectory() as d:
            for fmt, quality in (("webp", "82"), ("webp", "65"), ("jpg", "70"), ("jpg", "50")):
                out = os.path.join(d, "shot." + fmt)
                if subprocess.run([magick, path, "-resize", f"{MAX_SIDE}x{MAX_SIDE}>", "-quality", quality, out],
                                  capture_output=True, timeout=30).returncode != 0:
                    continue
                with open(out, "rb") as f:
                    data = f.read()
                mime = "image/webp" if fmt == "webp" else "image/jpeg"
                if len(data) * 4 // 3 <= budget:
                    break
    else:
        with open(path, "rb") as f:
            data = f.read()
    if data is None or len(data) * 4 // 3 > budget:
        return None
    return mime, base64.b64encode(data).decode("ascii")


def payload(report):
    title = check(report)
    body = {"title": title}
    for key, _ in FIELDS:
        body[key] = (report.get(key) or "").strip()[:20000]
    body["version"] = (report.get("sysinfo") or {}).get("iRice", "")
    if report.get("includeInfo"):
        body["sysinfo"] = report.get("sysinfo") or {}
    if report.get("includeLog"):
        body["log"] = (report.get("log") or "")[-200000:]
    used = len(json.dumps(body).encode())
    if used > MAX_BYTES:
        raise ValueError("The report is too large. Shorten the description or untick the log, then send again.")
    if report.get("includeShot"):
        shot = screenshot(report.get("shot"), MAX_BYTES - used - 1024)
        if shot is None:
            raise ValueError("The screenshot couldn't be made small enough. Crop it, or untick it, then send again.")
        body["screenshot"] = {"type": shot[0], "data": shot[1]}
    return body


def send(report, endpoint=None):
    endpoint = endpoint or os.environ.get("IRICE_BUG_ENDPOINT") or ENDPOINT
    if urllib.parse.urlparse(endpoint).scheme != "https":
        raise ValueError("Reports are only sent over https.")
    data = json.dumps(payload(report)).encode()
    req = urllib.request.Request(endpoint, data=data, method="POST",
                                 headers={"Content-Type": "application/json", "User-Agent": "iRice-bug-report"})
    try:
        with urllib.request.urlopen(req, timeout=TIMEOUT) as r:
            return json.loads(r.read().decode() or "{}")
    except urllib.error.HTTPError as e:
        try:
            msg = json.loads(e.read().decode()).get("error")
        except ValueError:
            msg = None
        if e.code == 429:
            raise ValueError("Too many reports from this network. Wait an hour, or open it on GitHub instead.")
        raise ValueError(msg or f"The report server answered {e.code}. Try again later, or open it on GitHub instead.")
    except (urllib.error.URLError, TimeoutError, OSError):
        raise ValueError("Couldn't reach the report server. Check you're online, then try again, or open it on GitHub instead.")


def main(argv):
    if not argv:
        print(__doc__, file=sys.stderr)
        return 2
    cmd = argv[0]
    try:
        if cmd == "gather":
            print(json.dumps(gather(argv[1] if len(argv) > 1 else os.getcwd())))
            return 0
        report = json.load(sys.stdin)
        if cmd == "send":
            print(json.dumps({"ok": True, **send(report)}))
        elif cmd == "text":
            sys.stdout.write(as_text(report))
        elif cmd == "github-url":
            print(github_url(report))
        else:
            return 2
    except ValueError as e:
        print(json.dumps({"ok": False, "error": str(e)}))
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
