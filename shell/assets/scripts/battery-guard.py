#!/usr/bin/env python3
"""Puts the computer to sleep when the battery is about to run out.

Settings → Battery & Power → Critical battery. The shell warns at 10% and
5% (with Sleep now); this guard is what makes sure the computer sleeps at
3% on battery even if the shell has crashed or isn't running, and even with
Caffeine on (that only holds off idle sleep, not this). Sleep, never
hibernate.

It runs as a systemd user service (irice-battery-guard.service, installed by
battery-guard-install.sh) and only reacts: UPower tells it when the battery
changes (PropertiesChanged on its display device), so it costs nothing in
between. It asks logind to suspend once per discharge: after a wake on a
still-empty battery it waits for the next change before trying again, and
plugging in resets it.

  battery-guard.py [--sleep-at 3]
  battery-guard.py --check <percent> <state>    (the decision only, for tests)
"""
import argparse
import subprocess
import sys

UPOWER = "org.freedesktop.UPower"
DISPLAY = "/org/freedesktop/UPower/devices/DisplayDevice"
DEVICE = "org.freedesktop.UPower.Device"
DISCHARGING = 2  # UPower's state for "on battery, draining"
EMPTY = 5


def should_sleep(percent, state, sleep_at):
    """On battery (draining or empty) and at or under the level."""
    return state in (DISCHARGING, EMPTY) and 0 <= percent <= sleep_at


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--sleep-at", type=float, default=3)
    ap.add_argument("--check", nargs=2, type=float, metavar=("PERCENT", "STATE"))
    args = ap.parse_args()
    if args.check:
        print("sleep" if should_sleep(args.check[0], int(args.check[1]), args.sleep_at) else "wait")
        return 0

    try:
        import gi
        gi.require_version("Gio", "2.0")
        from gi.repository import Gio, GLib
    except (ImportError, ValueError, AttributeError) as e:
        print(f"battery-guard needs python-gobject ({e})", file=sys.stderr)
        return 1

    bus = Gio.bus_get_sync(Gio.BusType.SYSTEM)
    device = Gio.DBusProxy.new_sync(bus, Gio.DBusProxyFlags.NONE, None, UPOWER, DISPLAY, DEVICE)
    done = {"armed": True}

    def check(*_):
        percent = device.get_cached_property("Percentage")
        state = device.get_cached_property("State")
        present = device.get_cached_property("IsPresent")
        if percent is None or state is None or (present is not None and not present.unpack()):
            return
        p, s = percent.unpack(), state.unpack()
        if s not in (DISCHARGING, EMPTY):
            done["armed"] = True  # plugged in: ready for the next time
            return
        if done["armed"] and should_sleep(p, s, args.sleep_at):
            done["armed"] = False
            subprocess.run(["notify-send", "-u", "critical", "-a", "iRice", "Battery empty",
                            f"Sleeping at {p:.0f}% so you don't lose your work. Plug in to keep going."],
                           check=False, timeout=5)
            try:
                login = Gio.DBusProxy.new_sync(bus, Gio.DBusProxyFlags.NONE, None, "org.freedesktop.login1",
                                               "/org/freedesktop/login1", "org.freedesktop.login1.Manager")
                login.call_sync("Suspend", GLib.Variant("(b)", (False,)), Gio.DBusCallFlags.NONE, 10000, None)
            except GLib.Error as e:
                print(f"couldn't sleep: {e.message}", file=sys.stderr)
                done["armed"] = True

    device.connect("g-properties-changed", check)
    check()
    GLib.MainLoop().run()
    return 0


if __name__ == "__main__":
    sys.exit(main())
