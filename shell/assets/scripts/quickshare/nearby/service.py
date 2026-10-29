"""Quick Share as a whole: being visible, finding devices, the transfers.

  visible    a TCP listener on a free port, announced over mDNS, and (with
             Bluetooth on) a Bluetooth LE advertisement and GATT server, so
             phones without a shared network find it too; incoming
             connections are asked about (ask) before anything is saved
  discover   browse mDNS, and scan Bluetooth LE, for devices visible now
  send       connect to one (over the network when it's there, Bluetooth
             otherwise), introduce the files, stream them on accept
Which link a transfer uses, like Android: on the same network it stays
there; a connection that starts over Bluetooth moves to the same network, or
else to Wi-Fi Direct (wifi.py), after the receiver accepts; if that can't
happen it carries on over Bluetooth (slow) with a warning.
Everything the shell needs to show goes through emit(dict) (the daemon
prints it as a JSON line). Discovery is a pluggable object with publish /
unpublish / browse / stop_browsing (avahi.Avahi), Bluetooth one with the
bluez.Bluetooth methods, so tests use fakes.
"""
import asyncio
import os
import socket

from . import ble_format, debuglog, endpoint, wifi
from .connection import Connection, ProtocolError
from .sharing import Cancelled, Declined, Receive, Send
from .transport import StreamTransport, TransportClosed

PORT = 53318
MAX_INCOMING = 3  # at once; more are refused (a flood shouldn't fill the disk or the island)
UPGRADE_WAIT = 10  # sending over Bluetooth: how long the files wait for a faster link
MEDIUM_NAMES = {"wifi_lan": "Wi-Fi", "wifi_direct": "Wi-Fi Direct", "wifi_hotspot": "Wi-Fi Direct", "ble": "Bluetooth"}
SLOW = "Staying on Bluetooth, which is slow"


class QuickShareService:
    def __init__(self, emit, discovery=None, folder=None, name=None, ble=None, helper=None, net_probe=None):
        self.emit = emit
        self.discovery = discovery
        self.ble = ble
        self.helper = helper  # wifi.WifiHelper: root for Wi-Fi Direct, asked for on demand
        self.net_probe = net_probe or wifi.network
        self.folder = folder or os.path.expanduser("~/Downloads")
        self.name = name or socket.gethostname()
        self.endpoint_id = endpoint.new_endpoint_id()
        self.device_token = ble_format.new_device_token()
        self.info = None  # our endpoint info: one for mDNS and Bluetooth (a phone sees one device)
        self.visible = False
        self.looking = False
        self.server = None
        self.port = 0
        self.devices = {}  # id (mDNS name, or "ble-" + endpoint id) → {"id", "name", "type", "eid", host/port or ble}
        self.sessions = {}  # session id -> {"task", "answer" future, "session"}
        self._serial = 0
        self.preferred_port = PORT
        self.upgrade_port = wifi.UPGRADE_PORT
        self.port_tries = 6  # (½ s apart: a helper from before a restart lets go of the port)
        self.can_host = wifi.helper_possible  # (whether the root helper can make Wi-Fi links here)
        self.reachable = wifi.reachable  # (whether the phone answers at the address it gave)
        self.firewall = firewall_status  # (whether a firewall may keep phones from reaching us)
        if ble is not None:
            ble.on_session = lambda transport: asyncio.ensure_future(self._incoming_transport(transport))
            ble.on_state = self._bluetooth_changed

    def _log(self, what):
        debuglog.log("service", what)

    def _endpoint_info(self):
        if self.info is None:
            self.info = endpoint.endpoint_info(self.name, endpoint.LAPTOP, visible=True)
        return self.info

    # ── Visibility ───────────────────────────────────────────────────────
    async def configure(self, visible=None, name=None, folder=None):
        if folder:
            self.folder = os.path.expanduser(folder)
        renamed = name is not None and name.strip() and name.strip() != self.name
        if renamed:
            self.name = name.strip()
            self.info = None
        if visible is not None and visible != self.visible:
            await (self._show() if visible else self._hide())
        elif renamed and self.visible:
            await self._announce()  # the name is in the announcement
            await self._advertise()
        self.emit({"ev": "status", "visible": self.visible, "port": self.port, "name": self.name})
        self.emit_bluetooth()

    async def _show(self):
        # A fixed port when free, so a firewall rule can name it (next to
        # LocalSend's 53317); any port otherwise (phones learn it over mDNS)
        # (a helper from before a shell restart may still hold it a moment)
        why = None
        for attempt in range(self.port_tries) if self.preferred_port else ():
            try:
                self.server = await asyncio.start_server(self._incoming, host="0.0.0.0", port=self.preferred_port)
                break
            except OSError as e:
                why = e
                await asyncio.sleep(0.5)
        if self.server is None:
            self.server = await asyncio.start_server(self._incoming, host="0.0.0.0", port=0)
        self.port = self.server.sockets[0].getsockname()[1]
        if why is not None and self.port != self.preferred_port:
            self._log(f"port {self.preferred_port} unavailable ({why}): using {self.port}")
            self.emit({"ev": "error", "message": f"Quick Share's port {self.preferred_port} is taken ({why.strerror or why}), "
                                                 f"so it uses {self.port}: a firewall rule for {self.preferred_port} "
                                                 "won't cover it. Quit whatever holds it and turn visibility off and on."})
        self.visible = True
        await self._announce()
        await self._advertise()

    async def _announce(self):
        if self.discovery is None:
            return
        info = self._endpoint_info()
        try:
            await self.discovery.publish(endpoint.service_name(self.endpoint_id), endpoint.MDNS_TYPE + "",
                                         self.port, {"n": endpoint.b64(info)})
        except Exception as e:  # noqa: BLE001 (Avahi errors are GLib errors)
            self.emit({"ev": "error", "message": f"Couldn't announce this computer on the network ({e}). "
                                                 "Check that avahi-daemon is running."})

    async def _advertise(self):
        if self.ble is None or not self.visible or not self.ble.powered:
            return
        info = self._endpoint_info()
        if len(info) > ble_format.MAX_ENDPOINT_INFO:  # (a long name, cut for Bluetooth)
            name = info[18:][:ble_format.MAX_ENDPOINT_INFO - 18].decode("utf-8", "ignore").encode("utf-8")
            info = info[:17] + bytes([len(name)]) + name
        try:
            adv = ble_format.advertisement(self.endpoint_id, info, self.device_token)
            # The extended advertisement must fit the controller (BlueZ says
            # InvalidLength otherwise): a shorter name, or the header only
            longest = await self.ble.max_advertisement()
            extended = adv
            if longest is not None:
                extended = ble_format.fitting_advertisement(self.endpoint_id, info, self.device_token,
                                                            longest - ble_format.AD_OVERHEAD) or False
                self._log(f"controller takes {longest}-byte advertisements: extended "
                          f"{len(extended) if extended else 'skipped'} (full {len(adv)})")
            if not await self.ble.advertise(adv, extended):
                self.emit({"ev": "error", "message": "Bluetooth refused to advertise this computer, so phones "
                                                     "without Wi-Fi can't find it. Restart Bluetooth and try again."})
        except Exception as e:  # noqa: BLE001 (BlueZ errors: Bluetooth is optional)
            self._log(f"advertising failed: {e}")
            self.emit({"ev": "error", "message": f"Bluetooth couldn't advertise this computer ({e})."})

    async def _hide(self):
        self.visible = False
        if self.discovery is not None:
            try:
                await self.discovery.unpublish()
            except Exception:  # noqa: BLE001
                pass
        if self.ble is not None:
            try:
                await self.ble.unserve()
            except Exception:  # noqa: BLE001
                pass
        if self.server:
            self.server.close()
            await self.server.wait_closed()
            self.server = None
        self.port = 0

    # ── Bluetooth ────────────────────────────────────────────────────────
    def emit_bluetooth(self):
        if self.ble is None:
            self.emit({"ev": "bluetooth", "available": False, "powered": False})
            return
        s = self.ble.state()
        self.emit({"ev": "bluetooth", "available": s["available"], "powered": s["powered"]})

    def _bluetooth_changed(self, state):
        self.emit_bluetooth()
        if state["powered"]:
            asyncio.ensure_future(self._advertise())
            if self.looking:
                asyncio.ensure_future(self.ble.scan(self._ble_found, self._ble_lost))

    async def bluetooth_on(self):
        """The user's "Turn on Bluetooth" (never done otherwise)."""
        if self.ble is None:
            return
        try:
            await self.ble.power_on()
        except Exception as e:  # noqa: BLE001
            self.emit({"ev": "error", "message": f"Couldn't turn on Bluetooth ({e}). Check that it isn't "
                                                 "blocked (rfkill) and try again."})

    # ── Finding devices ──────────────────────────────────────────────────
    async def discover(self, on):
        self.looking = on
        if on:
            self.devices = {}
            self._devices_changed()
            if self.discovery is not None:
                await self.discovery.browse(endpoint.MDNS_TYPE, self._found, self._lost)
            if self.ble is not None and self.ble.powered:
                try:
                    await self.ble.scan(self._ble_found, self._ble_lost)
                except Exception as e:  # noqa: BLE001
                    self._log(f"Bluetooth scan failed: {e}")
        else:
            if self.discovery is not None:
                await self.discovery.stop_browsing()
            if self.ble is not None:
                try:
                    await self.ble.stop_scan()
                except Exception:  # noqa: BLE001
                    pass

    def _found(self, name, host, port, txt):
        eid = endpoint.parse_service_name(name)
        if eid is None or eid == self.endpoint_id:
            return
        raw = txt.get("n")
        try:
            info = endpoint.parse_endpoint_info(endpoint.unb64(raw)) if raw else None
        except ValueError:
            info = None
        if not info or info["hidden"] or not info["name"]:
            return  # visible to its owner's contacts only: no name to show
        self.devices[name] = {"id": name, "name": info["name"], "type": info["type"], "host": host, "port": port,
                              "eid": eid}
        self._devices_changed()

    def _lost(self, name):
        if self.devices.pop(name, None):
            self._devices_changed()

    def _ble_found(self, path, adv):
        eid = adv["endpoint_id"]
        if eid == self.endpoint_id:
            return
        info = endpoint.parse_endpoint_info(adv["endpoint_info"])
        if not info or info["hidden"] or not info["name"]:
            return
        key = "ble-" + eid
        for k, d in list(self.devices.items()):
            if d.get("ble") == path and k != key:
                del self.devices[k]  # (the same phone, a new endpoint id)
        self.devices[key] = {"id": key, "name": info["name"], "type": info["type"], "eid": eid, "ble": path}
        self._devices_changed()

    def _ble_lost(self, path):
        gone = [k for k, d in self.devices.items() if d.get("ble") == path and "host" not in d]
        for k in gone:
            del self.devices[k]
        if gone:
            self._devices_changed()

    def _devices_changed(self):
        on_lan = {d["eid"] for d in self.devices.values() if "host" in d}
        shown = [d for d in self.devices.values() if "host" in d or d["eid"] not in on_lan]
        self.emit({"ev": "devices", "devices": [{"id": d["id"], "name": d["name"], "type": d["type"],
                                                  "via": "wifi" if "host" in d else "bluetooth"} for d in shown]})

    def _new_session(self, prefix):
        self._serial += 1
        return f"{prefix}-{self._serial}"

    def _transport_event(self, sid, medium):
        self.emit({"ev": "transport", "session": sid, "medium": medium, "name": MEDIUM_NAMES.get(medium, "Wi-Fi")})

    # ── Receiving ────────────────────────────────────────────────────────
    async def _incoming(self, reader, writer):
        await self._incoming_transport(StreamTransport(reader, writer))

    async def _incoming_transport(self, transport):
        busy = sum(1 for s in self.sessions.values() if s.get("dir") == "in")
        if not self.visible or busy >= MAX_INCOMING:
            await transport.close()
            return
        sid = self._new_session("in")
        conn = Connection(transport, is_client=False, endpoint_id=self.endpoint_id,
                          endpoint_info=self._endpoint_info(), device_name=self.name)
        conn.tag = sid
        entry = {"dir": "in", "task": asyncio.current_task(), "answer": None, "session": None}
        self.sessions[sid] = entry
        sender = "A nearby device"
        try:
            await conn.handshake()
            sender = (conn.remote or {}).get("name") or sender
            self._transport_event(sid, conn.medium)
            conn.on_upgraded = lambda medium: self._transport_event(sid, medium)

            async def ask(offer):
                entry["answer"] = asyncio.get_running_loop().create_future()
                self.emit(dict({"ev": "incoming", "session": sid, "medium": conn.medium}, **offer))
                return await entry["answer"]

            def progress(done, total):
                self.emit({"ev": "progress", "session": sid, "dir": "in", "done": done, "total": total, "name": sender})

            receive = Receive(conn, self.folder, ask, progress)
            if conn.medium == "ble":
                receive.on_accepted = lambda: conn._spawn(self._upgrade_in(conn, sid, entry, sender))
            entry["session"] = receive
            result = await receive.run()
            self.emit(dict({"ev": "finished", "session": sid, "dir": "in", "from": sender}, **result))
        except Declined as e:
            self.emit({"ev": "ended", "session": sid, "dir": "in", "reason": str(e)})
        except Cancelled as e:
            self.emit({"ev": "failed", "session": sid, "dir": "in", "message": "Cancelled" if entry.get("cancelled") else f"{sender} cancelled"})
        except asyncio.CancelledError:
            raise
        except (ProtocolError, TransportClosed, OSError, ValueError, asyncio.TimeoutError) as e:
            self.emit({"ev": "failed", "session": sid, "dir": "in",
                       "message": f"The transfer from {sender} stopped: {e or 'the connection was lost'}."})
        finally:
            self.sessions.pop(sid, None)
            await conn.close()
            await self._release_links(entry)

    async def _upgrade_in(self, conn, sid, entry, sender):
        """Receiving over Bluetooth, accepted: offer the phone a faster link."""
        net = await self.net_probe()
        helper_ok = self.helper is not None and self.can_host(net)
        ip = wifi.phone_address(conn)
        answers = await self.reachable(ip) if ip and wifi.same_network(net.get("addresses", []),
                                                                         conn.remote_metadata.ip_address) else False
        tries = wifi.host_paths(conn, net, helper_ok, reachable=answers)
        self._log(f"{sid}: upgrade paths {[p for p, _ in tries]} (phone's address answers: {answers})")
        if not tries:
            self._warn(sid, f"{SLOW}: {sender} and this computer share no network and Wi-Fi Direct isn't available "
                            "here. For full speed, put both on the same Wi-Fi.")
            return
        reasons = []
        if tries[0][0] == "lan" and len(tries) > 1 and self.firewall().get("blocks"):
            # (a firewall that may drop the phone's connection costs ~10 s of
            # its waiting; a Wi-Fi Direct link opens its own way in)
            self._log(f"{sid}: skipping the shared network: the firewall may block it")
            tries = tries[1:]
        for p, mine in tries:
            if conn._closed:
                return
            medium = {"lan": "wifi_lan", "direct": "wifi_direct", "hotspot": "wifi_hotspot"}[p]
            listener = wifi.Listener(medium)
            try:
                if p == "lan":
                    port = await listener.open(port=self.upgrade_port)
                    info = wifi.lan_path(mine, port)
                else:
                    self.helper.on_auth = lambda: self.emit({
                        "ev": "auth", "session": sid,
                        "message": f"Enter your password to make a direct Wi-Fi link with {sender}. "
                                   "Without it, the transfer stays on Bluetooth (slow)."})
                    entry["links"] = True
                    group = await self.helper.request({"cmd": "host", "mode": "p2p" if p == "direct" else "ap",
                                                       "freq": wifi.group_frequency(net, conn.remote_metadata) or 0})
                    port = await listener.open(port=self.upgrade_port)
                    info = (wifi.direct_path if p == "direct" else wifi.hotspot_path)(group, port)
                if await conn.offer_upgrade(info, listener.accept, timeout=30):
                    return
                reason = f"{sender} couldn't join the {'shared network' if p == 'lan' else 'link'}"
                if p == "lan" and self.firewall().get("blocks"):
                    reason = (f"this computer's firewall ({self.firewall()['kind']}) keeps {sender} from connecting. "
                              "Allow it in Settings → Sharing")
                reasons.append(reason)
            except wifi.WifiUnavailable as e:
                reasons.append(str(e).rstrip("."))
            except (OSError, ValueError) as e:
                reasons.append(f"the link couldn't be set up ({e})")
            finally:
                await listener.close()
        if not conn._closed:
            self._warn(sid, f"{SLOW}: {'; '.join(dict.fromkeys(reasons)) or 'no faster link worked'}.")

    def _warn(self, sid, message):
        self._log(f"{sid}: {message}")
        self.emit({"ev": "warning", "session": sid, "message": message})

    async def _release_links(self, entry):
        if entry.get("links") and self.helper is not None:
            try:
                await self.helper.release()
            except Exception:  # noqa: BLE001
                pass

    # ── Sending ──────────────────────────────────────────────────────────
    async def send(self, device_id, paths, host=None, port=None):
        """Sends files to a found device (or host:port); returns the session id."""
        dev = self.devices.get(device_id) or ({"name": device_id, "host": host, "port": port} if host else None)
        sid = self._new_session("out")
        if dev is None:
            self.emit({"ev": "failed", "session": sid, "dir": "out",
                       "message": "That device is gone. Ask them to open Quick Share again, then try once more."})
            return sid
        files = [p for p in paths if os.path.isfile(p)]
        if not files:
            self.emit({"ev": "failed", "session": sid, "dir": "out",
                       "message": "Quick Share sends files; folders aren't supported yet. Choose the files inside."})
            return sid
        entry = {"dir": "out", "answer": None, "session": None}
        self.sessions[sid] = entry
        entry["task"] = asyncio.ensure_future(self._send(sid, entry, dev, files))
        return sid

    def _ble_twin(self, dev):
        """The same phone, found over Bluetooth (for when the network fails)."""
        if dev.get("ble"):
            return dev["ble"]
        twin = self.devices.get("ble-" + dev.get("eid", ""))
        return twin.get("ble") if twin else None

    async def _connect(self, dev):
        if dev.get("host"):
            try:
                return await StreamTransport.connect(dev["host"], dev["port"])
            except (OSError, asyncio.TimeoutError):
                if not (self.ble is not None and self._ble_twin(dev)):
                    raise
                self._log("couldn't connect over the network: trying Bluetooth")
        path = self._ble_twin(dev)
        if self.ble is None or not path:
            raise OSError("no way to reach that device")
        return await self.ble.connect(path)

    async def _send(self, sid, entry, dev, files):
        conn = None
        name = dev["name"]
        try:
            self.emit({"ev": "progress", "session": sid, "dir": "out", "done": 0,
                       "total": sum(os.path.getsize(p) for p in files), "name": name, "waiting": True})
            transport = await self._connect(dev)
            conn = Connection(transport, is_client=True, endpoint_id=self.endpoint_id,
                              endpoint_info=self._endpoint_info(), device_name=self.name)
            conn.tag = sid
            net = None
            if transport.medium == "ble":
                # Tell the phone where it can move us: it hosts, we follow
                net = await self.net_probe()
                conn.mediums = wifi.mediums_we_join(self.helper is not None and self.can_host(net))
                conn.medium_metadata = wifi.medium_metadata(net)
                conn.on_upgrade_offer = lambda info: self._join(sid, entry, info, net, name)
            conn.on_upgraded = lambda medium: self._transport_event(sid, medium)
            await conn.handshake()
            self._transport_event(sid, conn.medium)
            self.emit({"ev": "connected", "session": sid, "pin": conn.pin, "to": name, "medium": conn.medium})

            def progress(done, total):
                self.emit({"ev": "progress", "session": sid, "dir": "out", "done": done, "total": total, "name": name})

            send = Send(conn, files, progress)
            if conn.medium == "ble":
                async def wait_for_wifi():
                    # The phone offers its link after accepting; files wait a
                    # little for it rather than crawl over Bluetooth
                    try:
                        await asyncio.wait_for(conn._upgraded.wait(), UPGRADE_WAIT)
                    except asyncio.TimeoutError:
                        if not conn._upgrading:
                            self._warn(sid, f"{SLOW}: {name} didn't offer a faster link.")
                send.before_files = wait_for_wifi
            entry["session"] = send
            result = await send.run()
            self.emit(dict({"ev": "finished", "session": sid, "dir": "out", "to": name}, **result))
        except Declined as e:
            self.emit({"ev": "declined", "session": sid, "to": name, "reason": str(e)})
        except Cancelled:
            self.emit({"ev": "failed", "session": sid, "dir": "out", "message": "Cancelled" if entry.get("cancelled") else f"{name} cancelled"})
        except asyncio.CancelledError:
            raise
        except (OSError, asyncio.TimeoutError) as e:
            self.emit({"ev": "failed", "session": sid, "dir": "out",
                       "message": f"Couldn't reach {name}. Keep Quick Share open on it, nearby with Bluetooth on or "
                                  "on the same network, and try again."
                       if conn is None else f"Sending to {name} stopped: {e or 'the connection was lost'}."})
        except (ProtocolError, TransportClosed, ValueError) as e:
            self.emit({"ev": "failed", "session": sid, "dir": "out", "message": f"Sending to {name} stopped: {e}."})
        finally:
            self.sessions.pop(sid, None)
            if conn:
                await conn.close()
            await self._release_links(entry)

    async def _join(self, sid, entry, info, net, name):
        """The phone offers a link (sending over Bluetooth): follow it."""
        try:
            if info.medium != wifi.PATH.WIFI_LAN:
                if self.helper is None:
                    raise wifi.WifiUnavailable("joining the phone's Wi-Fi Direct link isn't available here")
                self.helper.on_auth = lambda: self.emit({
                    "ev": "auth", "session": sid,
                    "message": f"Enter your password to join {name}'s direct Wi-Fi link. "
                               "Without it, the files go over Bluetooth (slow)."})
                entry["links"] = True
            return await wifi.join(info, self.helper, net, helper_ok=self.can_host(net))
        except wifi.WifiUnavailable as e:
            self._warn(sid, f"{SLOW}: {str(e).rstrip('.')}.")
        except (OSError, asyncio.TimeoutError) as e:
            self._warn(sid, f"{SLOW}: couldn't connect over the faster link ({e}).")
        return None

    # ── Answers and cancelling ───────────────────────────────────────────
    def answer(self, sid, accept):
        entry = self.sessions.get(sid)
        if entry and entry.get("answer") is not None and not entry["answer"].done():
            entry["answer"].set_result(bool(accept))

    async def cancel(self, sid):
        entry = self.sessions.get(sid)
        if not entry:
            return
        entry["cancelled"] = True
        if entry.get("answer") is not None and not entry["answer"].done():
            entry["answer"].set_result(False)
            return
        if entry.get("session") is not None:
            await entry["session"].cancel()
            if entry["dir"] == "in" and entry.get("task"):
                entry["task"].cancel()  # stop waiting for payloads; partial files go
        elif entry.get("task"):
            entry["task"].cancel()

    async def close(self):
        for sid in list(self.sessions):
            await self.cancel(sid)
        await self._hide()
        if self.discovery is not None:
            try:
                await self.discovery.stop_browsing()
            except Exception:  # noqa: BLE001
                pass
        if self.ble is not None:
            try:
                await self.ble.close()
            except Exception:  # noqa: BLE001
                pass
        if self.helper is not None:
            await self.helper.close()


def firewall_status():
    """sharing-check.py's firewall() ({"kind", "allowed", "blocks"}), or
    nothing known."""
    import importlib.util
    path = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))),
                        "sharing-check.py")
    try:
        spec = importlib.util.spec_from_file_location("sharing_check", path)
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        return mod.firewall()
    except (OSError, ImportError, AttributeError):
        return {"kind": None, "allowed": None, "blocks": False}
