"""The system D-Bus through Gio, for BlueZ (bluez.py).

Gio's signals and exported objects need a GLib main loop: it runs in its own
thread, as for Avahi (avahi.py). Values cross over as plain Python: a call
takes (signature, value) and returns the unpacked reply; exported objects
describe their properties as {name: (signature, value)} and answer methods
from asyncio coroutines. bluez.py only sees this small interface, so its
tests run against a fake bus without D-Bus.
"""
import asyncio
import threading


class BusUnavailable(Exception):
    """No system bus, or python-gobject missing."""


class DBusError(Exception):
    """A D-Bus call failed; .name is the error name (org.bluez.Error.…)."""

    def __init__(self, name, message):
        super().__init__(message)
        self.name = name


def _intro_xml(interfaces):
    """Introspection XML for exported interfaces: {iface: {"methods": {name:
    (in signature, out signature)}, "properties": {name: signature},
    "signals": {name: signature}}}."""
    out = ["<node>"]
    for iface, spec in interfaces.items():
        out.append(f'<interface name="{iface}">')
        for name, (sig_in, sig_out) in spec.get("methods", {}).items():
            out.append(f'<method name="{name}">')
            for i, t in enumerate(_split(sig_in)):
                out.append(f'<arg name="a{i}" type="{t}" direction="in"/>')
            for i, t in enumerate(_split(sig_out)):
                out.append(f'<arg name="r{i}" type="{t}" direction="out"/>')
            out.append("</method>")
        for name, t in spec.get("properties", {}).items():
            out.append(f'<property name="{name}" type="{t}" access="read"/>')
        for name, sig in spec.get("signals", {}).items():
            out.append(f'<signal name="{name}">')
            for i, t in enumerate(_split(sig)):
                out.append(f'<arg name="a{i}" type="{t}"/>')
            out.append("</signal>")
        out.append("</interface>")
    out.append("</node>")
    return "".join(out)


def _split(sig):
    """'a{sv}ay' → ['a{sv}', 'ay'] (single complete types)."""
    out, i = [], 0
    while i < len(sig):
        j = _end(sig, i)
        out.append(sig[i:j])
        i = j
    return out


def _end(sig, i):
    c = sig[i]
    if c == "a":
        return _end(sig, i + 1)
    if c in "({":
        close = ")" if c == "(" else "}"
        j = i + 1
        while sig[j] != close:
            j = _end(sig, j)
        return j + 1
    return i + 1


class GioBus:
    """call / subscribe / export / emit on the system bus."""

    def __init__(self, loop, address=None):
        try:
            import gi
            gi.require_version("Gio", "2.0")
            from gi.repository import Gio, GLib
        except (ImportError, ValueError) as e:
            raise BusUnavailable("python-gobject is missing") from e
        self.Gio, self.GLib = Gio, GLib
        self.loop = loop
        self.ctx = GLib.MainContext.new()
        self.gloop = GLib.MainLoop.new(self.ctx, False)
        self.conn = None
        self._exports = {}
        ready = threading.Event()
        error = []

        def run():
            self.ctx.push_thread_default()
            try:
                if address:  # (a private bus: the tests)
                    flags = (Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT
                             | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION)
                    self.conn = Gio.DBusConnection.new_for_address_sync(address, flags, None, None)
                else:
                    self.conn = Gio.bus_get_sync(Gio.BusType.SYSTEM, None)
            except Exception as e:  # noqa: BLE001 (GLib.Error)
                error.append(e)
                ready.set()
                return
            ready.set()
            self.gloop.run()

        self.thread = threading.Thread(target=run, name="gbus", daemon=True)
        self.thread.start()
        ready.wait(5)
        if error or self.conn is None:
            raise BusUnavailable(str(error[0]) if error else "no system bus")

    # ── Plumbing ─────────────────────────────────────────────────────────
    def _in_glib(self, fn):
        fut = self.loop.create_future()

        def call(*_):
            try:
                result = fn()
                self.loop.call_soon_threadsafe(lambda: fut.done() or fut.set_result(result))
            except Exception as e:  # noqa: BLE001 (handed to the awaiting side)
                self.loop.call_soon_threadsafe(lambda e=e: fut.done() or fut.set_exception(e))
            return False

        src = self.GLib.idle_source_new()
        src.set_callback(call)
        src.attach(self.ctx)
        return fut

    def _variant(self, sig, value):
        """Python → GLib.Variant, with {name: (sig, value)} for a{sv} and
        nested dicts of those."""
        GLib = self.GLib
        if sig == "v":
            s, v = value
            return GLib.Variant("v", self._variant(s, v))
        if sig.startswith("a{s") and sig.endswith("v}"):
            return GLib.Variant(sig, {k: self._variant(s, v) for k, (s, v) in value.items()})
        if sig == "a{oa{sa{sv}}}":
            return GLib.Variant(sig, {o: {i: {k: self._variant(s, v) for k, (s, v) in props.items()}
                                          for i, props in ifaces.items()} for o, ifaces in value.items()})
        if sig == "a{sa{sv}}":
            return GLib.Variant(sig, {i: {k: self._variant(s, v) for k, (s, v) in props.items()}
                                      for i, props in value.items()})
        if sig.startswith("("):
            parts = _split(sig[1:-1])
            return GLib.Variant.new_tuple(*[self._variant(t, v) for t, v in zip(parts, value)])
        return GLib.Variant(sig, value)

    def _error(self, e):
        name = ""
        try:
            name = self.Gio.DBusError.get_remote_error(e) or ""
        except Exception:  # noqa: BLE001
            pass
        msg = getattr(e, "message", str(e))
        return DBusError(name, msg)

    # ── Calls ────────────────────────────────────────────────────────────
    async def call(self, service, path, iface, method, sig=None, args=(), timeout=25000):
        """The unpacked reply (a tuple). Raises DBusError. Never blocks the
        GLib thread: BlueZ calls back into our exported objects (GetManagedObjects,
        the advertisement's properties) while it handles our Register… call."""
        return await self.call_async(service, path, iface, method, sig, args, timeout)

    async def call_async(self, service, path, iface, method, sig=None, args=(), timeout=30000):
        fut = self.loop.create_future()

        def done(conn, result):
            try:
                r = conn.call_finish(result)
                value = r.unpack() if r is not None else ()
                self.loop.call_soon_threadsafe(lambda: fut.done() or fut.set_result(value))
            except self.GLib.Error as e:
                err = self._error(e)
                self.loop.call_soon_threadsafe(lambda: fut.done() or fut.set_exception(err))

        def go():
            v = self._variant("(" + sig + ")", args) if sig else None
            self.conn.call(service, path, iface, method, v, None, self.Gio.DBusCallFlags.NONE, timeout, None, done)
        await self._in_glib(go)
        return await fut

    async def get_property(self, service, path, iface, name):
        (v,) = await self.call(service, path, "org.freedesktop.DBus.Properties", "Get", "ss", (iface, name))
        return v

    async def set_property(self, service, path, iface, name, sig, value):
        await self.call(service, path, "org.freedesktop.DBus.Properties", "Set", "ssv", (iface, name, (sig, value)))

    def subscribe(self, sender, iface, signal, path, callback, arg0=None):
        """callback(path, signal, args) on asyncio's loop; returns a handle."""
        def on_signal(conn, sender_, path_, iface_, signal_, params):
            args = params.unpack()
            self.loop.call_soon_threadsafe(callback, path_, signal_, args)

        box = {}

        def go():
            box["id"] = self.conn.signal_subscribe(sender, iface, signal, path, arg0,
                                                   self.Gio.DBusSignalFlags.NONE, on_signal)
        src = self.GLib.idle_source_new()
        src.set_callback(lambda *a: (go(), False)[1])
        src.attach(self.ctx)
        return box

    def unsubscribe(self, handle):
        def go():
            if "id" in handle:
                self.conn.signal_unsubscribe(handle.pop("id"))
        src = self.GLib.idle_source_new()
        src.set_callback(lambda *a: (go(), False)[1])
        src.attach(self.ctx)

    # ── Exported objects ─────────────────────────────────────────────────
    async def export(self, path, interfaces, props, methods):
        """interfaces: see _intro_xml; props(iface) → {name: (sig, value)};
        methods[(iface, name)](args, sender) → awaitable of a tuple
        (matching the out signature) or raises DBusError."""
        if path in self._exports:
            await self.unexport(path)  # (exported again: the new one replaces it)
        info = self.Gio.DBusNodeInfo.new_for_xml(_intro_xml(interfaces))
        ids = []

        def method_call(conn, sender, path_, iface, name, params, invocation):
            handler = methods.get((iface, name))
            if handler is None:
                invocation.return_dbus_error("org.freedesktop.DBus.Error.UnknownMethod", name)
                return
            args = params.unpack()
            out_sig = interfaces[iface]["methods"][name][1]

            async def run():
                try:
                    result = await handler(args, sender)
                    reply = self._variant("(" + out_sig + ")", result or ()) if out_sig else None
                    self._reply(invocation, reply)
                except DBusError as e:
                    self._reply_error(invocation, e.name or "org.bluez.Error.Failed", str(e))
                except Exception as e:  # noqa: BLE001 (answered, never left hanging)
                    self._reply_error(invocation, "org.bluez.Error.Failed", str(e))
            asyncio.run_coroutine_threadsafe(run(), self.loop)

        def get_property(conn, sender, path_, iface, name):
            p = props(iface).get(name)
            return self._variant(*p) if p else None

        def go():
            for iface_info in info.interfaces:
                ids.append(self.conn.register_object(path, iface_info, method_call, get_property, None))
        await self._in_glib(go)
        self._exports[path] = ids

    def _reply(self, invocation, value):
        src = self.GLib.idle_source_new()
        src.set_callback(lambda *a: (invocation.return_value(value), False)[1])
        src.attach(self.ctx)

    def _reply_error(self, invocation, name, message):
        src = self.GLib.idle_source_new()
        src.set_callback(lambda *a: (invocation.return_dbus_error(name, message), False)[1])
        src.attach(self.ctx)

    async def unexport(self, path):
        ids = self._exports.pop(path, [])

        def go():
            for i in ids:
                self.conn.unregister_object(i)
        await self._in_glib(go)

    def emit(self, path, iface, signal, sig, args):
        def go():
            self.conn.emit_signal(None, path, iface, signal, self._variant("(" + sig + ")", args))
        src = self.GLib.idle_source_new()
        src.set_callback(lambda *a: (go(), False)[1])
        src.attach(self.ctx)

    async def close(self):
        for path in list(self._exports):
            await self.unexport(path)
        await self._in_glib(self.gloop.quit)
        self.thread.join(2)
