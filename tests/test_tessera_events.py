"""tessera-events (native/tessera-events), the shell's one event process, run for
real on a private D-Bus (dbus-run-session, standing in for both the session
and the system bus): never the real buses. Skipped when it isn't built
(tools/build-native.sh, or cargo build in native/tessera-events), or when
dbus-run-session or gdbus is missing.

The driver half of this file runs inside dbus-run-session: it starts
tessera-events, plays a fake logind (with Gio) where one is needed, sends it
signals, and prints what tessera-events wrote as one JSON object."""
import json
import os
import queue
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SELF = Path(__file__).resolve()


def binary():
    for p in (os.environ.get("TESSERA_EVENTS_BIN"), ROOT / "native/tessera-events/target/release/tessera-events",
              ROOT / "native/tessera-events/target/debug/tessera-events"):
        if p and os.access(p, os.X_OK):
            return str(p)
    return None


def has_gi():
    try:
        import gi
        gi.require_version("Gio", "2.0")
        from gi.repository import Gio  # noqa: F401
        return True
    except (ImportError, ValueError):
        return False


BIN = binary()
TOOLS = shutil.which("dbus-run-session") and shutil.which("gdbus")


def drive(scenario):
    env = {k: v for k, v in os.environ.items() if not k.startswith("DBUS_")}
    env.pop("TESSERA_SANDBOX", None)
    env["XDG_RUNTIME_DIR"] = tempfile.mkdtemp(prefix="tessera-events-test-")
    env["PULSE_SERVER"] = "unix:" + env["XDG_RUNTIME_DIR"] + "/no-pulse"
    r = subprocess.run(["dbus-run-session", "--", sys.executable, "-I", str(SELF), "--driver", BIN, scenario],
                       env=env, capture_output=True, text=True, timeout=60)
    if r.returncode != 0:
        raise AssertionError(r.stderr[-2000:])
    return json.loads(r.stdout.strip().splitlines()[-1])


@unittest.skipUnless(BIN, "tessera-events isn't built (tools/build-native.sh)")
@unittest.skipUnless(TOOLS, "needs dbus-run-session and gdbus")
class TesseraEvents(unittest.TestCase):
    def test_help_and_version(self):
        r = subprocess.run([BIN, "--help"], capture_output=True, text=True, timeout=10)
        self.assertEqual(r.returncode, 0)
        self.assertIn("Ends when stdin closes", r.stdout)
        self.assertIn("protocol 1", subprocess.run([BIN, "--version"], capture_output=True, text=True, timeout=10).stdout)

    def test_ends_when_stdin_closes(self):
        p = subprocess.Popen([BIN], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True,
                             env=dict(os.environ, DBUS_SESSION_BUS_ADDRESS="unix:path=/nonexistent"))
        ready = json.loads(p.stdout.readline())
        self.assertEqual((ready["src"], ready["kind"], ready["version"]), ("events", "ready", 1))
        self.assertIn("unity", ready["sources"])
        p.stdin.close()
        self.assertEqual(p.wait(timeout=5), 0)

    def test_launcher_entry_badge(self):
        out = drive("unity")
        self.assertEqual(out["ready"]["kind"], "ready")
        self.assertEqual(out["events"], [
            {"src": "unity", "kind": "Update", "args": ["application://org.telegram.desktop.desktop", {"count": 3, "count-visible": True}]},
        ])
        # turned off: nothing more
        self.assertEqual(out["after_off"], [])

    @unittest.skipUnless(has_gi(), "needs python-gobject for the fake logind")
    def test_login1_and_inhibitors(self):
        out = drive("login1")
        # a signal on logind's path from someone else isn't logind's
        self.assertEqual(out["spoofed"], [])
        self.assertEqual(out["sleep"], [{"src": "login1", "kind": "PrepareForSleep", "args": [True]}])
        lid = out["lid"]
        self.assertEqual(len(lid), 1)
        self.assertEqual(lid[0]["kind"], "PropertiesChanged")
        self.assertEqual(lid[0]["args"][0], "org.freedesktop.login1.Manager")
        self.assertEqual(lid[0]["args"][1], {"LidClosed": True})
        # the inhibitor: asked for as Tessera, held while on, let go when off
        self.assertEqual(out["inhibit_call"], ["handle-lid-switch", "Tessera", "Lid settings", "block"])
        self.assertEqual(out["inhibit_on"], [{"src": "inhibit", "what": "handle-lid-switch", "on": True}])
        self.assertTrue(out["held_while_on"])
        self.assertEqual(out["inhibit_off"], [{"src": "inhibit", "what": "handle-lid-switch", "on": False}])
        self.assertFalse(out["held_after_off"])

    @unittest.skipUnless(has_gi(), "needs python-gobject for the fake Avahi")
    def test_cast_devices_through_avahi(self):
        out = drive("avahi")
        self.assertEqual(out["found"], [{"src": "avahi", "kind": "new", "key": "2;0;Chromecast-abc", "name": "Chromecast-abc",
                                         "host": "192.168.1.20", "port": 8009,
                                         "txt": {"id": "abc", "fn": "Living Room TV", "md": "Chromecast"}}])
        self.assertEqual(out["lost"], [{"src": "avahi", "kind": "remove", "key": "2;0;Chromecast-abc"}])
        # prepared, started, resolved; freed when turned off
        self.assertEqual(out["calls"], ["ServiceBrowserPrepare", "Start", "ResolveService", "Free"])


# ── The driver (inside dbus-run-session) ────────────────────────────────
class Events:
    def __init__(self, binary):
        env = dict(os.environ, DBUS_SYSTEM_BUS_ADDRESS=os.environ["DBUS_SESSION_BUS_ADDRESS"])
        self.p = subprocess.Popen([binary], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, env=env)
        self.q = queue.Queue()
        threading.Thread(target=self._read, daemon=True).start()
        self.ready = self.next(5)

    def _read(self):
        for line in self.p.stdout:
            self.q.put(json.loads(line))

    def send(self, **cmd):
        self.p.stdin.write(json.dumps(cmd) + "\n")
        self.p.stdin.flush()

    def next(self, timeout):
        try:
            return self.q.get(timeout=timeout)
        except queue.Empty:
            return None

    def gather(self, seconds):
        out, end = [], time.time() + seconds
        while (left := end - time.time()) > 0:
            e = self.next(left)
            if e is not None:
                out.append(e)
        return out

    def close(self):
        self.p.stdin.close()
        self.p.wait(timeout=5)


def emit(path, signal, *args, bus="--session"):
    subprocess.run(["gdbus", "emit", bus, "--object-path", path, "--signal", signal, *args], check=True, capture_output=True)


def scenario_unity(binary):
    ev = Events(binary)
    ev.send(cmd="watch", src="unity", on=True)
    time.sleep(0.3)
    emit("/com/canonical/unity/launcherentry/1", "com.canonical.Unity.LauncherEntry.Update",
         "application://org.telegram.desktop.desktop", "{'count': <int64 3>, 'count-visible': <true>}")
    events = ev.gather(1.0)
    ev.send(cmd="watch", src="unity", on=False)
    time.sleep(0.3)
    emit("/com/canonical/unity/launcherentry/1", "com.canonical.Unity.LauncherEntry.Update",
         "application://x.desktop", "{'count': <int64 1>}")
    after = ev.gather(0.7)
    ev.close()
    return {"ready": ev.ready, "events": events, "after_off": after}


LOGIND_XML = """<node><interface name="org.freedesktop.login1.Manager">
  <method name="Inhibit"><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="s" direction="in"/>
    <arg type="s" direction="in"/><arg type="h" direction="out"/></method>
  <signal name="PrepareForSleep"><arg type="b"/></signal>
  <property name="LidClosed" type="b" access="read"/>
</interface></node>"""


def scenario_login1(binary):
    import gi
    gi.require_version("Gio", "2.0")
    from gi.repository import Gio, GLib

    conn = Gio.bus_get_sync(Gio.BusType.SESSION)
    state = {"call": None, "w": None}

    def on_call(conn, sender, path, iface, method, params, inv):
        r, w = os.pipe()
        state["call"], state["w"] = list(params.unpack()), w
        fds = Gio.UnixFDList.new()
        fds.append(r)
        os.close(r)
        inv.return_value_with_unix_fd_list(GLib.Variant("(h)", (0,)), fds)

    node = Gio.DBusNodeInfo.new_for_xml(LOGIND_XML)
    conn.register_object("/org/freedesktop/login1", node.interfaces[0], on_call, None, None)
    Gio.bus_own_name_on_connection(conn, "org.freedesktop.login1", Gio.BusNameOwnerFlags.NONE, None, None)
    loop = GLib.MainLoop()
    threading.Thread(target=loop.run, daemon=True).start()
    time.sleep(0.3)

    def held():
        try:
            os.write(state["w"], b"x")
            return True
        except BrokenPipeError:
            return False

    ev = Events(binary)
    ev.send(cmd="watch", src="login1", on=True)
    time.sleep(0.3)
    # from another connection, on logind's path: must not count
    emit("/org/freedesktop/login1", "org.freedesktop.login1.Manager.PrepareForSleep", "true")
    spoofed = ev.gather(0.5)
    conn.emit_signal(None, "/org/freedesktop/login1", "org.freedesktop.login1.Manager", "PrepareForSleep", GLib.Variant("(b)", (True,)))
    sleep = ev.gather(0.5)
    conn.emit_signal(None, "/org/freedesktop/login1", "org.freedesktop.DBus.Properties", "PropertiesChanged",
                     GLib.Variant("(sa{sv}as)", ("org.freedesktop.login1.Manager", {"LidClosed": GLib.Variant("b", True)}, [])))
    lid = ev.gather(0.5)
    ev.send(cmd="inhibit", what="handle-lid-switch", why="Lid settings", on=True)
    inhibit_on = ev.gather(0.7)
    held_on = held()
    ev.send(cmd="inhibit", what="handle-lid-switch", on=False)
    inhibit_off = ev.gather(0.5)
    held_off = held()
    ev.close()
    loop.quit()
    return {"spoofed": spoofed, "sleep": sleep, "lid": lid, "inhibit_call": state["call"], "inhibit_on": inhibit_on,
            "held_while_on": held_on, "inhibit_off": inhibit_off, "held_after_off": held_off}


AVAHI_XML = """<node>
<interface name="org.freedesktop.Avahi.Server2">
  <method name="ServiceBrowserPrepare"><arg type="i" direction="in"/><arg type="i" direction="in"/><arg type="s" direction="in"/>
    <arg type="s" direction="in"/><arg type="u" direction="in"/><arg type="o" direction="out"/></method>
</interface>
<interface name="org.freedesktop.Avahi.Server">
  <method name="ResolveService"><arg type="i" direction="in"/><arg type="i" direction="in"/><arg type="s" direction="in"/>
    <arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="i" direction="in"/><arg type="u" direction="in"/>
    <arg type="i" direction="out"/><arg type="i" direction="out"/><arg type="s" direction="out"/><arg type="s" direction="out"/>
    <arg type="s" direction="out"/><arg type="s" direction="out"/><arg type="i" direction="out"/><arg type="s" direction="out"/>
    <arg type="q" direction="out"/><arg type="aay" direction="out"/><arg type="u" direction="out"/></method>
</interface>
<interface name="org.freedesktop.Avahi.ServiceBrowser">
  <method name="Start"/><method name="Free"/>
  <signal name="ItemNew"><arg type="i"/><arg type="i"/><arg type="s"/><arg type="s"/><arg type="s"/><arg type="u"/></signal>
  <signal name="ItemRemove"><arg type="i"/><arg type="i"/><arg type="s"/><arg type="s"/><arg type="s"/><arg type="u"/></signal>
</interface></node>"""


def scenario_avahi(binary):
    import gi
    gi.require_version("Gio", "2.0")
    from gi.repository import Gio, GLib

    conn = Gio.bus_get_sync(Gio.BusType.SESSION)
    browser = "/Client1/ServiceBrowser1"
    calls = []
    item = (2, 0, "Chromecast-abc", "_googlecast._tcp", "local", 0)

    def on_call(conn, sender, path, iface, method, params, inv):
        calls.append(method)
        if method == "ServiceBrowserPrepare":
            inv.return_value(GLib.Variant("(o)", (browser,)))
        elif method == "Start":
            inv.return_value(None)
            conn.emit_signal(sender, browser, "org.freedesktop.Avahi.ServiceBrowser", "ItemNew", GLib.Variant("(iisssu)", item))
        elif method == "ResolveService":
            txt = [b"id=abc", b"fn=Living Room TV", b"md=Chromecast"]
            inv.return_value(GLib.Variant("(iissssisqaayu)", (2, 0, "Chromecast-abc", "_googlecast._tcp", "local", "tv.local", 0,
                                                              "192.168.1.20", 8009, txt, 0)))
        else:
            inv.return_value(None)

    node = Gio.DBusNodeInfo.new_for_xml(AVAHI_XML)
    for i in node.interfaces:
        conn.register_object(browser if i.name.endswith("Browser") else "/", i, on_call, None, None)
    Gio.bus_own_name_on_connection(conn, "org.freedesktop.Avahi", Gio.BusNameOwnerFlags.NONE, None, None)
    loop = GLib.MainLoop()
    threading.Thread(target=loop.run, daemon=True).start()
    time.sleep(0.3)

    ev = Events(binary)
    ev.send(cmd="watch", src="avahi", on=True)
    found = ev.gather(1.0)
    conn.emit_signal(None, browser, "org.freedesktop.Avahi.ServiceBrowser", "ItemRemove", GLib.Variant("(iisssu)", item))
    lost = ev.gather(0.5)
    ev.send(cmd="watch", src="avahi", on=False)
    time.sleep(0.3)
    ev.close()
    loop.quit()
    return {"found": found, "lost": lost, "calls": calls}


if __name__ == "__main__":
    if sys.argv[1:2] == ["--driver"]:
        scenarios = {"unity": scenario_unity, "login1": scenario_login1, "avahi": scenario_avahi}
        print(json.dumps(scenarios[sys.argv[3]](sys.argv[2])))
    else:
        unittest.main()
