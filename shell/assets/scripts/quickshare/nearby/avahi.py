"""Announcing this computer and finding Quick Share devices, through Avahi.

mDNS on a Linux desktop belongs to the Avahi daemon (it already answers on
port 5353); a second responder inside this process would fight it. So
publishing and browsing go through Avahi's D-Bus API, with Gio from
python-gobject. Gio's signals need a GLib main loop: that loop runs in its
own thread, and what it finds is handed to asyncio's loop.

Phones only announce themselves while they can receive (Quick Share open
or visible to everyone, screen on), so a phone appears in the list then.
"""
import asyncio
import threading

AVAHI = "org.freedesktop.Avahi"
IF_UNSPEC = -1
PROTO_INET = 0  # IPv4: link-local IPv6 would need a scope id to connect
LOOKUP_RESULT_OUR_OWN = 16


class AvahiUnavailable(Exception):
    """Avahi isn't running (or python-gobject is missing)."""


class Avahi:
    """publish(name, type, port, txt) / unpublish(); browse(type, found, lost)."""

    def __init__(self, loop):
        try:
            import gi
            gi.require_version("Gio", "2.0")
            from gi.repository import Gio, GLib
        except (ImportError, ValueError) as e:
            raise AvahiUnavailable("python-gobject is missing") from e
        self.Gio, self.GLib = Gio, GLib
        self.loop = loop
        self.ctx = GLib.MainContext.new()
        self.gloop = GLib.MainLoop.new(self.ctx, False)
        self.bus = None
        self.group = None
        self.browser = None
        self.subscriptions = []
        ready = threading.Event()
        error = []

        def run():
            self.ctx.push_thread_default()
            try:
                self.bus = Gio.bus_get_sync(Gio.BusType.SYSTEM, None)
                self.bus.call_sync(AVAHI, "/", "org.freedesktop.Avahi.Server", "GetVersionString",
                                   None, None, Gio.DBusCallFlags.NONE, 3000, None)
            except Exception as e:  # GLib.Error: no system bus, or no Avahi on it
                error.append(e)
                ready.set()
                return
            ready.set()
            self.gloop.run()

        self.thread = threading.Thread(target=run, name="avahi", daemon=True)
        self.thread.start()
        ready.wait(5)
        if error or self.bus is None:
            raise AvahiUnavailable(str(error[0]) if error else "no answer from Avahi")

    # Everything that touches GLib runs in its thread, through this
    def _in_glib(self, fn):
        fut = self.loop.create_future()

        def call():
            try:
                result = fn()
                self.loop.call_soon_threadsafe(lambda: fut.done() or fut.set_result(result))
            except Exception as e:  # noqa: BLE001 (handed to the awaiting side)
                self.loop.call_soon_threadsafe(lambda e=e: fut.done() or fut.set_exception(e))
            return False

        src = self.GLib.idle_source_new()
        src.set_callback(lambda *a: call())
        src.attach(self.ctx)
        return fut

    def _call(self, path, iface, method, args, reply=None):
        v = self.GLib.Variant(args[0], args[1]) if args else None
        r = self.bus.call_sync(AVAHI, path, iface, method, v,
                               self.GLib.VariantType(reply) if reply else None,
                               self.Gio.DBusCallFlags.NONE, 5000, None)
        return r.unpack() if r is not None else None

    # ── Publishing ───────────────────────────────────────────────────────
    async def publish(self, name, kind, port, txt):
        """Announces (or re-announces) one service; replaces the last one."""
        def go():
            if self.group is None:
                (self.group,) = self._call("/", "org.freedesktop.Avahi.Server", "EntryGroupNew", None, "(o)")
            else:
                self._call(self.group, "org.freedesktop.Avahi.EntryGroup", "Reset", None)
            records = [f"{k}={v}".encode() for k, v in txt.items()]
            self._call(self.group, "org.freedesktop.Avahi.EntryGroup", "AddService",
                       ("(iiussssqaay)", (IF_UNSPEC, PROTO_INET, 0, name, kind, "", "", port, records)))
            self._call(self.group, "org.freedesktop.Avahi.EntryGroup", "Commit", None)
        await self._in_glib(go)

    async def unpublish(self):
        def go():
            if self.group is not None:
                try:
                    self._call(self.group, "org.freedesktop.Avahi.EntryGroup", "Free", None)
                except Exception:  # noqa: BLE001 (Avahi restarted: nothing to free)
                    pass
                self.group = None
        await self._in_glib(go)

    # ── Browsing ─────────────────────────────────────────────────────────
    async def browse(self, kind, found, lost):
        """found(name, host, port, txt dict) / lost(name), called on asyncio's loop."""
        await self.stop_browsing()

        def on_signal(conn, sender, path, iface, signal, params):
            if path != self.browser:
                return
            iface_, proto, name, stype, domain, flags = params.unpack()
            if signal == "ItemRemove":
                self.loop.call_soon_threadsafe(lost, name)
            elif signal == "ItemNew" and not flags & LOOKUP_RESULT_OUR_OWN:
                self._resolve(iface_, proto, name, stype, domain, found)

        def go():
            # Subscribed before the browser exists, so no early result is missed
            for sig in ("ItemNew", "ItemRemove"):
                self.subscriptions.append(self.bus.signal_subscribe(
                    None, "org.freedesktop.Avahi.ServiceBrowser", sig, None, None,
                    self.Gio.DBusSignalFlags.NONE, on_signal))
            (self.browser,) = self._call("/", "org.freedesktop.Avahi.Server", "ServiceBrowserNew",
                                         ("(iissu)", (IF_UNSPEC, PROTO_INET, kind, "", 0)), "(o)")
        await self._in_glib(go)

    def _resolve(self, iface, proto, name, stype, domain, found):
        def done(bus, result):
            try:
                r = bus.call_finish(result).unpack()
            except Exception:  # noqa: BLE001 (gone before it could be resolved)
                return
            host, address, port, txt = r[5], r[7], r[8], r[9]
            fields = {}
            for rec in txt:
                k, _, v = bytes(rec).partition(b"=")
                fields[k.decode("ascii", "replace")] = v
            self.loop.call_soon_threadsafe(found, name, address or host, port, fields)

        self.bus.call(AVAHI, "/", "org.freedesktop.Avahi.Server", "ResolveService",
                      self.GLib.Variant("(iisssiu)", (iface, proto, name, stype, domain, PROTO_INET, 0)),
                      None, self.Gio.DBusCallFlags.NONE, 5000, None, done)

    async def stop_browsing(self):
        def go():
            for s in self.subscriptions:
                self.bus.signal_unsubscribe(s)
            self.subscriptions = []
            if self.browser:
                try:
                    self._call(self.browser, "org.freedesktop.Avahi.ServiceBrowser", "Free", None)
                except Exception:  # noqa: BLE001
                    pass
                self.browser = None
        await self._in_glib(go)

    async def close(self):
        await self.stop_browsing()
        await self.unpublish()
        await self._in_glib(self.gloop.quit)
        self.thread.join(2)
