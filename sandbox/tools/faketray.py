#!/usr/bin/env python3
# A fake tray app (StatusNotifierItem) for testing the menu bar's app icons:
# registers N icons (faketray.py 3) with the shell's watcher, each named for
# a themed icon, and prints what's clicked.
import sys
from gi.repository import Gio, GLib

ICONS = ["applications-internet", "audio-headphones", "folder-download", "weather-clear", "mail-unread"]
XML = """<node><interface name="org.kde.StatusNotifierItem">
 <method name="Activate"><arg direction="in" type="i"/><arg direction="in" type="i"/></method>
 <method name="SecondaryActivate"><arg direction="in" type="i"/><arg direction="in" type="i"/></method>
 <method name="ContextMenu"><arg direction="in" type="i"/><arg direction="in" type="i"/></method>
 <method name="Scroll"><arg direction="in" type="i"/><arg direction="in" type="s"/></method>
 <property name="Category" type="s" access="read"/><property name="Id" type="s" access="read"/>
 <property name="Title" type="s" access="read"/><property name="Status" type="s" access="read"/>
 <property name="IconName" type="s" access="read"/><property name="ItemIsMenu" type="b" access="read"/>
</interface></node>"""


def main():
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 3
    node = Gio.DBusNodeInfo.new_for_xml(XML)
    conn = Gio.bus_get_sync(Gio.BusType.SESSION)
    for i in range(n):
        icon = ICONS[i % len(ICONS)]

        def call(c, sender, path, iface, method, params, inv, icon=icon):
            print(method, icon, flush=True)
            inv.return_value(None)

        def prop(c, sender, path, iface, name, icon=icon, i=i):
            return {"Category": GLib.Variant("s", "ApplicationStatus"), "Id": GLib.Variant("s", "faketray%d" % i),
                    "Title": GLib.Variant("s", "Fake tray %d" % (i + 1)), "Status": GLib.Variant("s", "Active"),
                    "IconName": GLib.Variant("s", icon), "ItemIsMenu": GLib.Variant("b", False)}[name]

        # (one connection, an object path each: the watcher takes a path and
        # pairs it with the sender)
        path = "/StatusNotifierItem%d" % (i + 1)
        conn.register_object(path, node.interfaces[0], call, prop, None)
        conn.call_sync("org.kde.StatusNotifierWatcher", "/StatusNotifierWatcher", "org.kde.StatusNotifierWatcher",
                       "RegisterStatusNotifierItem", GLib.Variant("(s)", (path,)), None, Gio.DBusCallFlags.NONE, -1, None)
    GLib.MainLoop().run()


if __name__ == "__main__":
    main()
