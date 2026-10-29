#!/usr/bin/env python3
"""Sends a test notification that offers an inline reply, like a messaging
app does, and prints what comes back (the reply text, actions, closing).

    tools/test-notify.py [--resident] [--placeholder TEXT] [--seconds N]

Run it inside an iRice session (or the dev sandbox) to try Reply in the
banner, Notification Center and the island. It waits up to N seconds
(default 120) for a reply, then exits.
"""
import argparse
import sys

import gi

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib  # noqa: E402

BUS = "org.freedesktop.Notifications"
PATH = "/org/freedesktop/Notifications"


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--resident", action="store_true", help="stay after a reply (the app updates it itself)")
    ap.add_argument("--placeholder", default="Reply to Sam", help="hint shown in the empty reply field")
    ap.add_argument("--seconds", type=int, default=120)
    args = ap.parse_args()

    bus = Gio.bus_get_sync(Gio.BusType.SESSION)
    caps = bus.call_sync(BUS, PATH, BUS, "GetCapabilities", None, GLib.VariantType("(as)"),
                         Gio.DBusCallFlags.NONE, 5000, None).unpack()[0]
    print("server capabilities:", ", ".join(caps))
    if "inline-reply" not in caps:
        print("The notification server doesn't offer inline replies.", file=sys.stderr)

    hints = {"x-kde-reply-placeholder-text": GLib.Variant("s", args.placeholder),
             "desktop-entry": GLib.Variant("s", "org.irice.test")}
    if args.resident:
        hints["resident"] = GLib.Variant("b", True)
    nid = bus.call_sync(BUS, PATH, BUS, "Notify",
                        GLib.Variant("(susssasa{sv}i)", ("Test chat", 0, "mail-message-new", "Sam",
                                                         "Are we still on for 7?",
                                                         ["default", "Open", "inline-reply", "Reply", "mute", "Mute"],
                                                         hints, -1)),
                        GLib.VariantType("(u)"), Gio.DBusCallFlags.NONE, 5000, None).unpack()[0]
    print("sent notification", nid)

    loop = GLib.MainLoop()

    def on_signal(_conn, _sender, _path, _iface, name, params):
        values = params.unpack()
        if values[0] != nid:
            return
        if name == "NotificationReplied":
            print("reply:", repr(values[1]))
        elif name == "ActionInvoked":
            print("action:", values[1])
        elif name == "NotificationClosed":
            print("closed, reason", values[1])
            loop.quit()

    for signal in ("NotificationReplied", "ActionInvoked", "NotificationClosed"):
        bus.signal_subscribe(None, BUS, signal, PATH, None, Gio.DBusSignalFlags.NONE, on_signal)
    GLib.timeout_add_seconds(args.seconds, loop.quit)
    loop.run()


if __name__ == "__main__":
    main()
