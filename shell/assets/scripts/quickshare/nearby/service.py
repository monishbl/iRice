"""Quick Share as a whole: being visible, finding devices, the transfers.

  visible    a TCP listener on a free port, announced over mDNS, and (with
             Bluetooth on) a Bluetooth LE advertisement and GATT server, so
             phones without a shared network find it too; incoming
             connections are asked about (ask) before anything is saved
  discover   browse mDNS, and scan Bluetooth LE, for devices visible now
  send       connect to one (over the network when it's there, Bluetooth
             otherwise), introduce the files, stream them on accept
Which link a transfer uses: the fastest first, falling back only when it
fails. On the same network it stays there; a connection that starts over
Bluetooth moves, once the receiver accepts, to the same network, else to
Wi-Fi Direct or a hotspot (wifi.py); if none works it carries on over
Bluetooth (slow). The cheap links
get a second try before falling back, and links.LinkMemory remembers what
failed with which phone: a link that keeps failing is skipped for it for a
day. How it goes shows quietly in the transfer ("link" events), never as
alerts.
Everything the shell needs to show goes through emit(dict) (the daemon
prints it as a JSON line). Discovery is a pluggable object with publish /
unpublish / browse / stop_browsing (avahi.Avahi), Bluetooth one with the
bluez.Bluetooth methods, so tests use fakes.
"""
import asyncio
import os
import socket

from . import ble_format, debuglog, endpoint, links, wifi
from .connection import Connection, ProtocolError
from .sharing import Cancelled, Declined, Receive, Send
from .transport import StreamTransport, TransportClosed

PORT = 53318
MAX_INCOMING = 3  # at once; more are refused (a flood shouldn't fill the disk or the island)
UPGRADE_WAIT = 10  # sending over Bluetooth: how long the files wait for a faster link
MEDIUM_NAMES = {"wifi_lan": "Wi-Fi", "wifi_direct": "Wi-Fi Direct", "wifi_hotspot": "Wi-Fi hotspot", "ble": "Bluetooth"}
SLOW = "Staying on Bluetooth, which is slow"
RETRY = {"lan", "direct"}  # links worth a second try in the same transfer (nothing pauses for them)
RETRY_TIMEOUT = 12  # (the second try waits less)
SLOW_RTT = 25  # ms: a shared network slower than this goes after Wi-Fi Direct
SPEED_SAMPLE = 3  # seconds between speed checks on a shared network
SLOW_RATE = 3_000_000  # bytes/s: below this twice in a row, a big transfer asks for Wi-Fi Direct
WORTH_MOVING = 50_000_000  # bytes still to go for that to be worth it
TRUSTED = "/etc/irice/quickshare-trusted"  # (Settings: fast transfers without asking: no password mid-transfer)
TWIN_WAIT = 8  # a phone the network can't reach: how long to look for it over Bluetooth
PATH_MEDIUMS = {"lan": "wifi_lan", "direct": "wifi_direct", "hotspot": "wifi_hotspot", "nm": "wifi_hotspot"}
JOIN_KEYS = {wifi.CR.WIFI_LAN: "join-lan", wifi.CR.WIFI_DIRECT: "join-direct", wifi.CR.WIFI_HOTSPOT: "join-hotspot"}
OFFER_KEYS = {wifi.PATH.WIFI_LAN: "join-lan", wifi.PATH.WIFI_DIRECT: "join-direct", wifi.PATH.WIFI_HOTSPOT: "join-hotspot"}
ADV_REFRESH = 120  # seconds between fresh advertisements while visible



class Unreachable(OSError):
    """No way to the phone (`isolated`: found on the network, not reachable there)."""
    def __init__(self, isolated=False):
        super().__init__("no way to reach that device")
        self.isolated = isolated

class QuickShareService:
    def __init__(self, emit, discovery=None, folder=None, name=None, ble=None, helper=None, net_probe=None,
                 memory=None):
        self.emit = emit
        self.memory = memory if memory is not None else links.LinkMemory(path="")  # (the daemon's is kept on disk)
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
        self.latency = wifi.latency  # (and how quickly)
        self.firewall = firewall_status  # (whether a firewall may keep phones from reaching us)
        self.nm_can_host = wifi.NmWifi.can_host  # (a hotspot through NetworkManager: no password)
        self.nm_link = wifi.NmWifi
        self.nm_usable = wifi.NmWifi.usable
        self.trusted = TRUSTED  # (the file that says the helper runs without a password)
        self.pause_wifi = True  # (Settings: may the Wi-Fi pause for a fast link without a shared network)
        self._refresher = None
        if ble is not None:
            ble.on_session = lambda transport: asyncio.ensure_future(self._incoming_transport(transport))
            ble.on_state = self._bluetooth_changed
            ble.on_session_end = lambda: asyncio.ensure_future(self._readvertise("a phone disconnected"))

    def _log(self, what):
        debuglog.log("service", what)

    def _endpoint_info(self):
        if self.info is None:
            self.info = endpoint.endpoint_info(self.name, endpoint.LAPTOP, visible=True)
        return self.info

    # ── Visibility ───────────────────────────────────────────────────────
    async def configure(self, visible=None, name=None, folder=None, pause_wifi=None):
        if pause_wifi is not None:
            self.pause_wifi = bool(pause_wifi)
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
        if self.ble is not None and self._refresher is None:
            self._refresher = asyncio.ensure_future(self._refresh_loop())

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
            # Phones open an L2CAP channel when we name one (much faster than
            # GATT writes), and fall back to GATT if it fails
            psm = self.ble.l2cap_psm() if hasattr(self.ble, "l2cap_psm") else 0
            adv = ble_format.advertisement(self.endpoint_id, info, self.device_token, psm=psm)
            # The extended advertisement must fit the controller (BlueZ says
            # InvalidLength otherwise): a shorter name, or the header only
            longest = await self.ble.max_advertisement()
            extended = adv
            if longest is not None:
                extended = ble_format.fitting_advertisement(self.endpoint_id, info, self.device_token,
                                                            longest - ble_format.AD_OVERHEAD, psm=psm) or False
                self._log(f"controller takes {longest}-byte advertisements: extended "
                          f"{len(extended) if extended else 'skipped'} (full {len(adv)})")
            if not await self.ble.advertise(adv, extended):
                self.emit({"ev": "error", "message": "Bluetooth refused to advertise this computer, so phones "
                                                     "without Wi-Fi can't find it. Restart Bluetooth and try again."})
        except Exception as e:  # noqa: BLE001 (BlueZ errors: Bluetooth is optional)
            self._log(f"advertising failed: {e}")
            self.emit({"ev": "error", "message": f"Bluetooth couldn't advertise this computer ({e})."})

    async def _readvertise(self, why):
        """Advertises afresh: a controller stops a connectable advertisement
        when a phone connects to it, and doesn't always start it again (seen
        with phones that found a computer once and never again)."""
        if not self.visible or self.ble is None or not self.ble.powered or getattr(self.ble, "session", None):
            return
        self._log(f"advertising afresh ({why})")
        await self._advertise()

    async def _refresh_loop(self):
        try:
            while self.visible:
                await asyncio.sleep(ADV_REFRESH)
                await self._readvertise("every few minutes")
        except asyncio.CancelledError:
            pass

    async def _hide(self):
        self.visible = False
        if self._refresher is not None:
            self._refresher.cancel()
            self._refresher = None
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

    def _found(self, name, host, port, txt, ifname=""):
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
        # Every network it's seen on, the fastest kind first: Wi-Fi, then
        # wired, then a phone's own USB tether (slow, and it carries other
        # traffic)
        old = self.devices.get(name, {})
        hosts = [h for h in old.get("hosts", []) if h[1:] != (host, port)] + [(link_rank(ifname), host, port)]
        hosts.sort(key=lambda h: h[0])
        self.devices[name] = {"id": name, "name": info["name"], "type": info["type"], "host": hosts[0][1],
                              "port": hosts[0][2], "hosts": hosts, "eid": eid}
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

            def upgraded(medium):
                self._transport_event(sid, medium)
                if entry.get("offering"):  # (remembered the moment it moves: a short transfer may end first)
                    self.memory.worked(sender, entry["offering"])
            conn.on_upgraded = upgraded

            async def ask(offer):
                entry["answer"] = asyncio.get_running_loop().create_future()
                self.emit(dict({"ev": "incoming", "session": sid, "medium": conn.medium}, **offer))
                return await entry["answer"]

            def progress(done, total):
                entry["done"], entry["total"] = done, total
                self.emit({"ev": "progress", "session": sid, "dir": "in", "done": done, "total": total, "name": sender})

            receive = Receive(conn, self.folder, ask, progress)
            # A shared network that crawls (a busy 2.4 GHz band, a router
            # in between): Wi-Fi Direct next to it when the phone asks (it
            # knows its radio; moving on our own mid-flow can make it hop
            # channels and drop the old link)
            conn.on_upgrade_retry = lambda mediums, request: self._faster(conn, sid, entry, sender, "the phone asked for a faster link")

            if conn.medium == "ble":
                receive.on_accepted = lambda: conn._spawn(self._upgrade_in(conn, sid, entry, sender))
            elif conn.medium == "wifi_lan":
                # The phone chose the shared network: if that's a slow one,
                # offer Wi-Fi Direct as the files start (the same rule as
                # from Bluetooth)
                receive.on_accepted = lambda: conn._spawn(self._check_lan(conn, sid, entry, sender))
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
        """Receiving over Bluetooth, accepted: offer the phone a faster link,
        fastest first."""
        net = await self.net_probe()
        helper_ok = self.helper is not None and self.can_host(net)
        ip = wifi.phone_address(conn)
        answers = await self.reachable(ip) if ip and wifi.same_network(net.get("addresses", []),
                                                                         conn.remote_metadata.ip_address) else False
        # A hotspot through NetworkManager pauses this computer's own Wi-Fi:
        # only for shares worth it, and only if the phone can get an address
        # from it (the firewall lets DHCP through: it does while visible)
        total = getattr(entry.get("session"), "total", 0) or 0
        nm_host = fw_blocked = False
        if self.pause_wifi and total >= wifi.NM_MIN_BYTES and not answers and await self.nm_can_host(net):
            # (a firewall that keeps the phone from getting an address skips
            # this path: transfers never change the firewall for good; the
            # root helper's links open their own way in, for the link only)
            fw = self.firewall()
            nm_host = fw.get("hotspot") is not False
            fw_blocked = not nm_host
            if not nm_host:
                self._log(f"{sid}: no hotspot: the firewall ({fw.get('kind')}) would keep the phone from getting an address")
        tries = wifi.host_paths(conn, net, helper_ok, reachable=answers, nm_host=nm_host)
        if tries and tries[0][0] == "lan" and len(tries) > 1 and self.firewall().get("blocks"):
            # (a firewall that may drop the phone's connection costs ~10 s of
            # its waiting; a Wi-Fi Direct link opens its own way in)
            self._log(f"{sid}: skipping the shared network: the firewall may block it")
            tries = tries[1:]
        tries = self.memory.order(sender, tries, key=lambda t: t[0])
        # Fastest first means the measured kind of fast: a shared network on
        # 2.4 GHz, or one the phone answers slowly on (a busy band, a router
        # in between), goes after a Wi-Fi Direct link (~10 MB/s), when that
        # needs no password; decided before any data moves
        if len(tries) > 1 and tries[0][0] == "lan" and any(p == "direct" for p, _ in tries) \
                and os.path.exists(self.trusted):
            rtt = await self.latency(ip)
            band24 = 0 < (net.get("frequency") or 0) < 3000
            if band24 or (rtt is not None and rtt > SLOW_RTT):
                self._log(f"{sid}: the shared network looks slow ({'2.4 GHz' if band24 else f'{rtt:.0f} ms'}): Wi-Fi Direct first")
                tries = [t for t in tries if t[0] == "direct"] + [t for t in tries if t[0] != "direct"]
        self._log(f"{sid}: upgrade paths {[p for p, _ in tries]} (phone's address answers: {answers}, {total} bytes)")
        if not tries and fw_blocked:
            self._warn(sid, f"{SLOW}: this computer's firewall would keep {sender} from joining its hotspot. "
                            "Settings → Privacy & Security → Firewall says why.")
            return
        if not tries:
            why = wifi.no_direct_reason(net)
            self._warn(sid, f"{SLOW}: {sender} and this computer share no network, and {why}. "
                            "For full speed, put both on the same Wi-Fi.")
            return
        reasons = []
        for p, mine in tries:
            # Fastest first; the cheap links get a second try before falling back
            for n in range(2 if p in RETRY else 1):
                if conn._closed or conn._upgraded.is_set():
                    return
                entry["offering"] = p
                ok, reason = await self._offer(conn, sid, entry, sender, p, mine, net, RETRY_TIMEOUT if n else 30)
                if ok or conn._upgraded.is_set():
                    return
                reasons.append(reason)
                self._log(f"{sid}: {p} failed ({reason})")
            entry.pop("offering", None)
            self.memory.failed(sender, p)
        if not conn._closed and not conn._upgraded.is_set():
            self._warn(sid, f"{SLOW}: {'; '.join(dict.fromkeys(reasons)) or 'no faster link worked'}.")

    async def _watch_speed(self, conn, sid, entry, act):
        """Calls act(rate) once if a big transfer on a shared network stays
        slow (two checks in a row)."""
        last, slow = None, 0
        try:
            while not conn._closed:
                await asyncio.sleep(SPEED_SAMPLE)
                done, total = entry.get("done", 0), entry.get("total", 0)
                if last is not None and conn.medium == "wifi_lan" and not conn._upgrading and total - done > WORTH_MOVING:
                    rate = (done - last) / SPEED_SAMPLE
                    slow = slow + 1 if rate < SLOW_RATE else 0
                    if slow >= 2:
                        await act(rate)
                        return
                last = done
        except asyncio.CancelledError:
            pass

    async def _check_lan(self, conn, sid, entry, sender):
        total = getattr(entry.get("session"), "total", 0) or 0
        if total < wifi.NM_MIN_BYTES:
            return  # (a photo or a link: not worth a new link)
        net = await self.net_probe()
        band24 = 0 < (net.get("frequency") or 0) < 3000
        rtt = None if band24 else await self.latency(conn.transport.peer.rsplit(":", 1)[0])
        if band24 or (rtt is not None and rtt > SLOW_RTT):
            await self._faster(conn, sid, entry, sender, f"the shared network looks slow ({'2.4 GHz' if band24 else f'{rtt:.0f} ms'})")

    def _direct_beside(self, net, joining):
        """Whether a Wi-Fi Direct link can be made next to the Wi-Fi without
        a password now (mid-transfer nothing may pause or prompt): a group
        we host, or (`joining`) the phone's."""
        ok = wifi.join_beside_possible(net) if joining else wifi.direct_possible(net)
        return self.helper is not None and self.can_host(net) and ok and os.path.exists(self.trusted)

    async def _faster(self, conn, sid, entry, sender, why):
        """Receiving over a slow shared network, the phone asking: offer
        Wi-Fi Direct next to the Wi-Fi, once."""
        if entry.get("faster") or conn._closed or conn.medium != "wifi_lan" or conn._upgrading:
            return
        entry["faster"] = True
        net = await self.net_probe()
        if not self._direct_beside(net, joining=False) or self.memory.skip(sender, "direct"):
            self._log(f"{sid}: {why}, but no direct link can be made here now")
            return
        self._log(f"{sid}: {why}: offering Wi-Fi Direct")
        entry["offering"] = "direct"
        ok, reason = await self._offer(conn, sid, entry, sender, "direct", None, net, 20)
        if not ok and not conn._closed and conn.medium == "wifi_lan":
            self.memory.failed(sender, "direct")
            self._log(f"{sid}: staying on the shared network ({reason})")

    async def _offer(self, conn, sid, entry, sender, p, mine, net, timeout):
        """Offers the phone one link; (True, None) once it moved there, else
        (False, why)."""
        listener = wifi.Listener(PATH_MEDIUMS[p])
        nm = None
        try:
            if p == "lan":
                port = await listener.open(port=self.upgrade_port)
                info = wifi.lan_path(mine, port)
            elif p == "nm":
                nm = entry["nm"] = self.nm_link()
                self._note(sid, "This computer's Wi-Fi pauses for the transfer and comes back after")
                group = await nm.host(net)
                port = await listener.open(port=self.upgrade_port)
                info = wifi.hotspot_path(group, port)
            else:
                self.helper.on_auth = lambda: self._note(
                    sid, f"Enter your password to make a direct Wi-Fi link with {sender}", auth=True)
                entry["links"] = True
                group = await self.helper.request({"cmd": "host", "mode": "p2p" if p == "direct" else "ap",
                                                   "freq": wifi.group_frequency(net, conn.remote_metadata) or 0})
                port = await listener.open(port=self.upgrade_port)
                info = (wifi.direct_path if p == "direct" else wifi.hotspot_path)(group, port)
            if await conn.offer_upgrade(info, listener.accept, timeout=timeout):
                nm = None  # (in use: it goes when the transfer ends)
                return True, None
            if p == "lan" and self.firewall().get("blocks"):
                return False, (f"this computer's firewall ({self.firewall()['kind']}) keeps {sender} from connecting. "
                               "Settings → Privacy & Security → Firewall says why")
            return False, f"{sender} couldn't join the {'shared network' if p == 'lan' else 'hotspot' if p == 'nm' else 'link'}"
        except wifi.WifiUnavailable as e:
            return False, str(e).rstrip(".")
        except (OSError, ValueError) as e:
            return False, f"the link couldn't be set up ({e})"
        finally:
            await listener.close()
            if nm is not None:
                entry.pop("nm", None)
                await nm.restore()

    def _note(self, sid, message, auth=False):
        """How the transfer is going, for its live activity (no alert)."""
        self._log(f"{sid}: {message}")
        self.emit({"ev": "link", "session": sid, "message": message, "slow": False, "auth": auth})

    def _warn(self, sid, message):
        """It stays on slow Bluetooth: why and how to fix it, shown with the
        transfer (no alert)."""
        self._log(f"{sid}: {message}")
        self.emit({"ev": "link", "session": sid, "message": message, "slow": True, "auth": False})

    async def _release_links(self, entry):
        nm = entry.pop("nm", None)
        if nm is not None:
            await nm.restore()
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
        """The network first; Bluetooth when that fails, or straight away
        when the network keeps failing with this phone (and Bluetooth
        reaches it). A phone found on the network but not reachable there
        (a Wi-Fi that keeps devices apart) gets a few seconds to turn up over
        Bluetooth too."""
        name = dev.get("name")
        twin = self.ble is not None and self._ble_twin(dev)
        hosts = dev.get("hosts") or ([(1, dev["host"], dev["port"])] if dev.get("host") else [])
        # (a phone's USB tether only when Bluetooth can't reach it: Bluetooth
        # then Wi-Fi Direct is about ten times faster)
        tethers = [h for h in hosts if h[0] >= 2]
        if not (twin and self.memory.skip(name, "connect-lan")):
            t = await self._connect_lan(name, [h for h in hosts if h[0] < 2], twin)
            if t is not None:
                return t
            if hosts and not await self._find_twin(dev):
                t = await self._connect_lan(name, tethers, False)
                if t is not None:
                    return t
                raise Unreachable(isolated=True)
        path = self._ble_twin(dev)
        if self.ble is None or not path:
            t = await self._connect_lan(name, tethers, False)
            if t is not None:
                return t
            raise Unreachable(isolated=False)
        return await self.ble.connect(path)

    async def _connect_lan(self, name, hosts, twin):
        """A connection at the first of `hosts` that answers, else None."""
        for _, host, port in hosts:
            try:
                t = await StreamTransport.connect(host, port, timeout=6 if twin else 10)
                self.memory.worked(name, "connect-lan")
                return t
            except (OSError, asyncio.TimeoutError) as err:
                self._log(f"couldn't connect at {host} ({err})")
        if hosts:
            self.memory.failed(name, "connect-lan")
            self._log("couldn't connect over the network: trying Bluetooth")
        return None

    async def _find_twin(self, dev, wait=TWIN_WAIT):
        """The same phone over Bluetooth, scanning for it a moment if need be."""
        if self.ble is None or not self.ble.powered:
            return None
        if self._ble_twin(dev):
            return self._ble_twin(dev)
        scanning = self.looking
        try:
            if not scanning:
                await self.ble.scan(self._ble_found, self._ble_lost)
            for _ in range(int(wait / 0.25)):
                if self._ble_twin(dev):
                    return self._ble_twin(dev)
                await asyncio.sleep(0.25)
        except Exception as e:  # noqa: BLE001 (Bluetooth is optional)
            self._log(f"Bluetooth scan failed: {e}")
        finally:
            if not scanning and not self.looking:
                try:
                    await self.ble.stop_scan()
                except Exception:  # noqa: BLE001
                    pass
        return None

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
                nm_ok = self.pause_wifi and await self.nm_usable(net)
                # (the links that keep failing with this phone left out: it
                # offers the next one instead)
                conn.mediums = self.memory.order(
                    name, wifi.mediums_we_join(self.helper is not None and self.can_host(net), nm_ok),
                    key=lambda m: JOIN_KEYS.get(m, ""))
                conn.medium_metadata = wifi.medium_metadata(net)
                conn.on_upgrade_offer = lambda info: self._join(sid, entry, info, net, name)
            else:
                # On a shared network: the phone may move us to its Wi-Fi
                # Direct link when that's faster, joined next to the Wi-Fi
                # (nothing pauses mid-transfer, no password)
                net = await self.net_probe()
                if self._direct_beside(net, joining=True) and not self.memory.skip(name, "join-direct"):
                    conn.mediums = [wifi.CR.WIFI_LAN, wifi.CR.WIFI_DIRECT]
                    conn.medium_metadata = wifi.medium_metadata(net)
                    conn.on_upgrade_offer = lambda info: self._join(sid, entry, info, net, name, pause=False)
            conn.on_upgraded = lambda medium: self._transport_event(sid, medium)
            await conn.handshake()
            self._transport_event(sid, conn.medium)
            self.emit({"ev": "connected", "session": sid, "pin": conn.pin, "to": name, "medium": conn.medium})

            def progress(done, total):
                entry["done"], entry["total"] = done, total
                self.emit({"ev": "progress", "session": sid, "dir": "out", "done": done, "total": total, "name": name})

            send = Send(conn, files, progress)
            if wifi.CR.WIFI_DIRECT in (conn.mediums or []) and conn.medium == "wifi_lan":
                async def direct_first():
                    # A slow shared network (2.4 GHz, a slow answer): ask the
                    # phone for its Wi-Fi Direct link before the files go
                    # (nothing in flight yet, so moving is safe)
                    rtt = await self.latency(transport.peer.rsplit(":", 1)[0])
                    band24 = 0 < (net.get("frequency") or 0) < 3000
                    if band24 or (rtt is not None and rtt > SLOW_RTT):
                        await self._ask_faster(conn, sid, f"{'2.4 GHz' if band24 else f'{rtt:.0f} ms'}")
                        try:
                            await asyncio.wait_for(conn._upgraded.wait(), UPGRADE_WAIT)
                        except asyncio.TimeoutError:
                            self._log(f"{sid}: the phone stayed on the shared network")
                send.before_files = direct_first
                # (crawling later on: ask again, as Android asks us)
                conn._spawn(self._watch_speed(conn, sid, entry, lambda rate: self._ask_faster(conn, sid, f"{rate / 1e6:.1f} MB/s")))
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
        except Unreachable as e:
            bt = self.ble is not None and self.ble.powered
            self.emit({"ev": "failed", "session": sid, "dir": "out", "message": (
                f"Couldn't reach {name} over this Wi-Fi (it may keep devices apart)" if e.isolated else f"Couldn't reach {name}")
                + (". Keep Quick Share open on it, nearby with Bluetooth on, and try again." if bt else
                   ". Turn on Bluetooth so Quick Share can connect without the network, and try again." if self.ble is not None else
                   ". Put both on a Wi-Fi that lets devices reach each other (a phone's hotspot does), and try again.")})
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

    async def _ask_faster(self, conn, sid, why):
        self._log(f"{sid}: the shared network is slow ({why}): asking for Wi-Fi Direct")
        try:
            await conn.ask_faster([wifi.CR.WIFI_DIRECT])
        except (TransportClosed, OSError):
            pass

    async def _join(self, sid, entry, info, net, name, pause=True):
        key = OFFER_KEYS.get(info.medium)
        new = await self._join_path(sid, entry, info, net, name, pause)
        if key:
            (self.memory.worked if new is not None else self.memory.failed)(name, key)
        return new

    async def _join_path(self, sid, entry, info, net, name, pause=True):
        """The phone offers a link (sending over Bluetooth): follow it. Next
        to this computer's own Wi-Fi when the chip allows it (the root
        helper's Wi-Fi Direct client interface: nothing pauses; a password),
        else NetworkManager (no password; the Wi-Fi's own connection pauses
        meanwhile), else the root helper."""
        if info.medium != wifi.PATH.WIFI_LAN and self.helper is not None and self.can_host(net) \
                and wifi.join_beside_possible(net):
            self.helper.on_auth = lambda: self._note(
                sid, f"Enter your password to join {name}'s direct Wi-Fi link next to your Wi-Fi", auth=True)
            entry["links"] = True
            try:
                return await wifi.join(info, self.helper, net, helper_ok=True)
            except (wifi.WifiUnavailable, OSError, asyncio.TimeoutError) as e:
                self._log(f"{sid}: joining next to the Wi-Fi didn't work ({e}); trying the usual way")
        try:
            if info.medium != wifi.PATH.WIFI_LAN and pause and self.pause_wifi and await self.nm_usable(net):
                nm = entry["nm"] = self.nm_link()
                self._note(sid, "This computer's Wi-Fi pauses for the transfer and comes back after")
                return await wifi.join(info, None, net, nm=nm)
            if info.medium != wifi.PATH.WIFI_LAN:
                if self.helper is None:
                    raise wifi.WifiUnavailable("joining the phone's Wi-Fi Direct link isn't available here")
                self.helper.on_auth = lambda: self._note(
                    sid, f"Enter your password to join {name}'s direct Wi-Fi link", auth=True)
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


def link_rank(ifname, sys_net="/sys/class/net"):
    """0 Wi-Fi, 1 wired (or unknown), 2 a USB network (a phone's tether)."""
    if not ifname:
        return 1
    if os.path.exists(os.path.join(sys_net, ifname, "wireless")):
        return 0
    return 2 if "/usb" in os.path.realpath(os.path.join(sys_net, ifname, "device")) else 1


def _sharing_check():
    import importlib.util
    path = os.path.join(os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))),
                        "sharing-check.py")
    try:
        spec = importlib.util.spec_from_file_location("sharing_check", path)
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        return mod
    except (OSError, ImportError, AttributeError):
        return None


def firewall_status():
    """sharing-check.py's firewall() ({"kind", "allowed", "blocks",
    "hotspot"}), or nothing known."""
    mod = _sharing_check()
    try:
        return mod.firewall() if mod else {"kind": None, "allowed": None, "blocks": False, "hotspot": None}
    except (OSError, AttributeError):
        return {"kind": None, "allowed": None, "blocks": False, "hotspot": None}
