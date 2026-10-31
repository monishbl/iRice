#!/usr/bin/env python3
# A fake MPRIS player for testing the island: plays a 3:30 "track",
# answers PlayPause/Next/Previous, reports position and volume.
# FAKEPLAYER_URL: what it says it plays (xesam:url), e.g. a file:// URL to
# test casting.
import os, sys, time
from gi.repository import Gio, GLib

ART = sys.argv[1] if len(sys.argv) > 1 else ""
TRACKS = [("Midnight City", "M83", "Hurry Up, We're Dreaming"), ("Blinding Lights", "The Weeknd", "After Hours"), ("Levitating", "Dua Lipa", "Future Nostalgia")]
XML = """<node>
<interface name="org.mpris.MediaPlayer2">
 <method name="Raise"/><method name="Quit"/>
 <property name="CanQuit" type="b" access="read"/><property name="CanRaise" type="b" access="read"/>
 <property name="HasTrackList" type="b" access="read"/><property name="Identity" type="s" access="read"/>
 <property name="DesktopEntry" type="s" access="read"/>
 <property name="SupportedUriSchemes" type="as" access="read"/><property name="SupportedMimeTypes" type="as" access="read"/>
</interface>
<interface name="org.mpris.MediaPlayer2.Player">
 <method name="Next"/><method name="Previous"/><method name="Pause"/><method name="PlayPause"/><method name="Stop"/><method name="Play"/>
 <method name="Seek"><arg direction="in" type="x"/></method>
 <method name="SetPosition"><arg direction="in" type="o"/><arg direction="in" type="x"/></method>
 <property name="PlaybackStatus" type="s" access="read"/><property name="Rate" type="d" access="readwrite"/>
 <property name="Metadata" type="a{sv}" access="read"/><property name="Volume" type="d" access="readwrite"/>
 <property name="Position" type="x" access="read"/><property name="MinimumRate" type="d" access="read"/><property name="MaximumRate" type="d" access="read"/>
 <property name="CanGoNext" type="b" access="read"/><property name="CanGoPrevious" type="b" access="read"/><property name="CanPlay" type="b" access="read"/>
 <property name="CanPause" type="b" access="read"/><property name="CanSeek" type="b" access="read"/><property name="CanControl" type="b" access="read"/>
</interface></node>"""

state = {"i": 0, "playing": True, "pos": 42.0, "vol": 0.7, "t": time.time()}
conn = None


def pos():
    p = state["pos"] + ((time.time() - state["t"]) if state["playing"] else 0)
    return min(p, 210.0)


def meta():
    t, a, al = TRACKS[state["i"]]
    m = {"mpris:trackid": GLib.Variant("o", "/irice/track/%d" % state["i"]), "xesam:title": GLib.Variant("s", t),
         "xesam:artist": GLib.Variant("as", [a]), "xesam:album": GLib.Variant("s", al), "mpris:length": GLib.Variant("x", 210 * 1000000)}
    if ART:
        m["mpris:artUrl"] = GLib.Variant("s", ART)
    if os.environ.get("FAKEPLAYER_URL"):
        m["xesam:url"] = GLib.Variant("s", os.environ["FAKEPLAYER_URL"])
    return m


def props(iface):
    if iface == "org.mpris.MediaPlayer2":
        return {"CanQuit": GLib.Variant("b", False), "CanRaise": GLib.Variant("b", False), "HasTrackList": GLib.Variant("b", False),
                "Identity": GLib.Variant("s", "Test Player"), "DesktopEntry": GLib.Variant("s", "foot"),
                "SupportedUriSchemes": GLib.Variant("as", []), "SupportedMimeTypes": GLib.Variant("as", [])}
    return {"PlaybackStatus": GLib.Variant("s", "Playing" if state["playing"] else "Paused"), "Rate": GLib.Variant("d", 1.0),
            "Metadata": GLib.Variant("a{sv}", meta()), "Volume": GLib.Variant("d", state["vol"]), "Position": GLib.Variant("x", int(pos() * 1e6)),
            "MinimumRate": GLib.Variant("d", 1.0), "MaximumRate": GLib.Variant("d", 1.0), "CanGoNext": GLib.Variant("b", True),
            "CanGoPrevious": GLib.Variant("b", True), "CanPlay": GLib.Variant("b", True), "CanPause": GLib.Variant("b", True),
            "CanSeek": GLib.Variant("b", True), "CanControl": GLib.Variant("b", True)}


def changed(keys):
    p = props("org.mpris.MediaPlayer2.Player")
    conn.emit_signal(None, "/org/mpris/MediaPlayer2", "org.freedesktop.DBus.Properties", "PropertiesChanged",
                     GLib.Variant("(sa{sv}as)", ("org.mpris.MediaPlayer2.Player", {k: p[k] for k in keys}, [])))


def call(c, sender, path, iface, method, params, inv):
    if method in ("PlayPause", "Play", "Pause"):
        state["pos"] = pos(); state["t"] = time.time()
        state["playing"] = (not state["playing"]) if method == "PlayPause" else method == "Play"
        changed(["PlaybackStatus"])
    elif method in ("Next", "Previous"):
        state["i"] = (state["i"] + (1 if method == "Next" else -1)) % len(TRACKS)
        state["pos"] = 0; state["t"] = time.time()
        changed(["Metadata"])
    elif method == "SetPosition":
        state["pos"] = params.unpack()[1] / 1e6; state["t"] = time.time()
    inv.return_value(None)


def get(c, sender, path, iface, name):
    return props(iface)[name]


def setp(c, sender, path, iface, name, value):
    if name == "Volume":
        state["vol"] = value.unpack()
        changed(["Volume"])
    return True


def acquired(c, name):
    global conn
    conn = c
    node = Gio.DBusNodeInfo.new_for_xml(XML)
    for i in node.interfaces:
        c.register_object("/org/mpris/MediaPlayer2", i, call, get, setp)


Gio.bus_own_name(Gio.BusType.SESSION, "org.mpris.MediaPlayer2.irice_test", Gio.BusNameOwnerFlags.NONE, acquired, None, None)
GLib.MainLoop().run()
