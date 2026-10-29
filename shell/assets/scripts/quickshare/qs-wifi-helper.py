#!/usr/bin/env python3
"""Quick Share's Wi-Fi links, as root (started by nearby/wifi.py under pkexec).

One JSON request per line on stdin, one JSON answer per line on stdout:
  {"cmd": "host", "mode": "p2p" | "ap", "freq": MHz or 0}
      a Wi-Fi Direct group (we are its owner) or an access point, with
      DHCP for the phone: {"ok", "ssid", "passphrase", "freq", "ip", "iface"}
  {"cmd": "join", "ssid", "passphrase", "freq"}
      joins a phone's group on a second interface next to the normal Wi-Fi
      connection: {"ok", "ip", "gateway", "iface"}
  {"cmd": "stop"}   tears every link down: {"ok"}
It starts with {"ok": true, "ready": true}. When stdin closes (the Quick
Share helper ended, however it ended) or on SIGTERM, everything it made goes:
groups, interfaces, addresses, DHCP servers. Nothing is left configured.

Everything goes through wpa_supplicant's D-Bus API (fi.w1.wpa_supplicant1,
next to NetworkManager, which keeps the normal connection): P2PDevice.GroupAdd
for a group; a virtual interface (iw) with its own wpa_supplicant interface
for an access point or for joining, kept away from NetworkManager. The group's
channel: the station's when a group owner may use it (not DFS / no-IR, per
`iw phy`), else the first usable of 5180…5240, 5745…5805, 2412/2437/2462.
Requests are checked strictly (SSID, passphrase, channel): nothing from
stdin reaches a shell or a command line unchecked.
"""
import json
import os
import random
import re
import select
import signal
import socket
import struct
import subprocess
import sys
import threading
import time

WPA = "fi.w1.wpa_supplicant1"
WPA_PATH = "/fi/w1/wpa_supplicant1"
IFACE = WPA + ".Interface"
P2P = IFACE + ".P2PDevice"
GROUP = WPA + ".Group"
GO_IP = "192.168.49.1"  # what Android's own groups use
AP_IP = "192.168.50.1"
PREFERRED = [5180, 5200, 5220, 5240, 5745, 5765, 5785, 5805, 2412, 2437, 2462]
JOIN_IFACE = "qsc0"
AP_IFACE = "qsap0"


def answer(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()


class Refused(Exception):
    """A request that can't be done; the message goes back to the user."""


# ── Checks (pure: tested in tests/test_quickshare_ble.py) ────────────────
def check_ssid(ssid):
    if not isinstance(ssid, str) or not 1 <= len(ssid.encode()) <= 32 or any(ord(c) < 32 for c in ssid):
        raise Refused("unusable network name")
    return ssid


def check_passphrase(p):
    if not isinstance(p, str) or not 8 <= len(p) <= 63 or not all(32 <= ord(c) < 127 for c in p):
        raise Refused("unusable Wi-Fi password")
    return p


def check_freq(f):
    if f in (None, 0, "", -1):
        return 0
    if not isinstance(f, int) or not (2400 <= f <= 2500 or 4900 <= f <= 5900 or 5925 <= f <= 7125):
        raise Refused("unusable channel")
    return f


def check_ifname(name):
    if not re.fullmatch(r"[A-Za-z0-9_.-]{1,15}", name or ""):
        raise Refused("unusable interface name")
    return name


def parse_iw_phy(text):
    """`iw phy` → {freq: usable as group owner} (not disabled, no radar
    detection, no "no IR")."""
    out = {}
    for line in text.splitlines():
        m = re.match(r"\s*\*\s+(\d+)(?:\.\d+)?\s+MHz\s+\[\d+\](.*)", line)
        if m:
            freq, rest = int(m.group(1)), m.group(2)
            out[freq] = not any(w in rest for w in ("disabled", "radar detection", "no IR", "passive scanning"))
    return out


def pick_freq(wanted, usable):
    """The group's channel: the wanted one when usable, else a safe one."""
    if wanted and usable.get(wanted):
        return wanted
    for f in PREFERRED:
        if usable.get(f):
            return f
    return 2437 if not usable else next((f for f, ok in sorted(usable.items()) if ok), 2437)


def group_add_args(freq, wide=True):
    """P2PDevice.GroupAdd's dictionary; wide asks for 40/80 MHz (VHT), which
    older wpa_supplicant versions don't know (then retried without)."""
    args = {"persistent": ("b", False)}
    if freq:
        args["frequency"] = ("i", freq)
    if wide:
        args["ht40"] = ("b", True)
        args["vht"] = ("b", True)
    return args


def ap_network(ssid, passphrase, freq):
    return {"ssid": ("s", ssid), "psk": ("s", passphrase), "mode": ("u", 2), "frequency": ("i", freq),
            "key_mgmt": ("s", "WPA-PSK"), "proto": ("s", "RSN"), "pairwise": ("s", "CCMP"), "group": ("s", "CCMP")}


def join_network(ssid, passphrase, freq):
    n = {"ssid": ("s", ssid), "psk": ("s", passphrase), "key_mgmt": ("s", "WPA-PSK"), "scan_ssid": ("u", 1)}
    if freq:
        n["scan_freq"] = ("s", str(freq))
        n["freq_list"] = ("s", str(freq))
    return n


def new_passphrase():
    alphabet = "abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789"
    return "".join(random.SystemRandom().choice(alphabet) for _ in range(12))


def random_mac():
    b = bytearray(os.urandom(6))
    b[0] = (b[0] & 0xFC) | 0x02  # locally administered, unicast
    return ":".join(f"{x:02x}" for x in b)


# ── DHCP (dnsmasq when installed, else these) ────────────────────────────
MAGIC = b"\x63\x82\x53\x63"


def dhcp_packet(op, xid, mac, ciaddr="0.0.0.0", yiaddr="0.0.0.0", siaddr="0.0.0.0", flags=0x8000, options=()):
    head = struct.pack("!BBBBIHH4s4s4s4s16s64s128s", op, 1, 6, 0, xid, 0, flags, socket.inet_aton(ciaddr),
                       socket.inet_aton(yiaddr), socket.inet_aton(siaddr), b"\0" * 4, mac.ljust(16, b"\0"), b"", b"")
    opts = b"".join(bytes([code, len(v)]) + v for code, v in options)
    return head + MAGIC + opts + b"\xff"


def parse_dhcp(raw):
    if len(raw) < 240 or raw[236:240] != MAGIC:
        return None
    op, xid, flags = raw[0], struct.unpack("!I", raw[4:8])[0], struct.unpack("!H", raw[10:12])[0]
    opts, i = {}, 240
    while i < len(raw) and raw[i] != 255:
        if raw[i] == 0:
            i += 1
            continue
        if i + 1 >= len(raw):
            break
        code, n = raw[i], raw[i + 1]
        opts[code] = raw[i + 2:i + 2 + n]
        i += 2 + n
    return {"op": op, "xid": xid, "flags": flags, "chaddr": raw[28:34], "ciaddr": socket.inet_ntoa(raw[12:16]),
            "yiaddr": socket.inet_ntoa(raw[16:20]), "type": opts.get(53, b"\0")[0], "options": opts}


def _dhcp_socket(iface, port):
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    s.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)
    s.setsockopt(socket.SOL_SOCKET, socket.SO_BINDTODEVICE, iface.encode())
    s.bind(("0.0.0.0", port))
    return s


class DhcpServer(threading.Thread):
    """Leases .10–.99 of our /24 to whoever asks on one interface."""

    def __init__(self, iface, ip):
        super().__init__(daemon=True)
        self.iface, self.ip = iface, ip
        self.prefix = ip.rsplit(".", 1)[0]
        self.leases = {}
        self.sock = _dhcp_socket(iface, 67)
        self.stopped = False

    def run(self):
        while not self.stopped:
            r, _, _ = select.select([self.sock], [], [], 0.5)
            if not r:
                continue
            try:
                raw = self.sock.recv(2048)
            except OSError:
                return
            p = parse_dhcp(raw)
            if not p or p["op"] != 1 or p["type"] not in (1, 3):
                continue
            mac = bytes(p["chaddr"])
            addr = self.leases.setdefault(mac, f"{self.prefix}.{10 + len(self.leases) % 90}")
            kind = 2 if p["type"] == 1 else 5  # DISCOVER → OFFER, REQUEST → ACK
            if p["type"] == 3:
                wanted = p["options"].get(50)
                if wanted and socket.inet_ntoa(wanted) != addr:
                    kind = 6  # NAK
            opts = [(53, bytes([kind])), (54, socket.inet_aton(self.ip))]
            if kind != 6:
                opts += [(51, struct.pack("!I", 3600)), (1, socket.inet_aton("255.255.255.0"))]
            reply = dhcp_packet(2, p["xid"], mac, yiaddr=addr if kind != 6 else "0.0.0.0", siaddr=self.ip, options=opts)
            try:
                self.sock.sendto(reply, ("255.255.255.255", 68))
            except OSError:
                pass

    def stop(self):
        self.stopped = True
        try:
            self.sock.close()
        except OSError:
            pass


def dhcp_client(iface, mac_text, timeout=6):
    """(address, mask, gateway) from the network's DHCP server, or None."""
    mac = bytes(int(x, 16) for x in mac_text.split(":"))
    xid = random.getrandbits(32)
    try:
        s = _dhcp_socket(iface, 68)
    except OSError:
        return None
    try:
        def exchange(kind, extra, want):
            pkt = dhcp_packet(1, xid, mac, options=[(53, bytes([kind])), (55, bytes([1, 3, 54]))] + extra)
            end = time.monotonic() + timeout / 2
            s.sendto(pkt, ("255.255.255.255", 67))
            while time.monotonic() < end:
                r, _, _ = select.select([s], [], [], 0.5)
                if not r:
                    s.sendto(pkt, ("255.255.255.255", 67))
                    continue
                p = parse_dhcp(s.recv(2048))
                if p and p["op"] == 2 and p["xid"] == xid and p["type"] == want:
                    return p
            return None
        offer = exchange(1, [], 2)
        if not offer:
            return None
        server = offer["options"].get(54, b"")
        ack = exchange(3, [(50, socket.inet_aton(offer["yiaddr"]))] + ([(54, server)] if server else []), 5)
        if not ack:
            return None
        mask = socket.inet_ntoa(ack["options"].get(1, socket.inet_aton("255.255.255.0")))
        gw = ack["options"].get(3) or server
        return ack["yiaddr"], mask, socket.inet_ntoa(gw[:4]) if gw else None
    except OSError:
        return None
    finally:
        s.close()


def prefix_len(mask):
    return sum(bin(int(x)).count("1") for x in mask.split("."))


# ── The links ────────────────────────────────────────────────────────────
def run(*cmd, check=True):
    r = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, timeout=20)
    if check and r.returncode != 0:
        raise Refused(f"{cmd[0]} failed: {r.stderr.strip()[:200]}")
    return r.stdout


def have(cmd):
    return any(os.access(os.path.join(d, cmd), os.X_OK) for d in os.environ.get("PATH", "/usr/bin").split(":"))


class Wpa:
    """wpa_supplicant over D-Bus (blocking calls; this helper is single-threaded)."""

    def __init__(self):
        import gi
        gi.require_version("Gio", "2.0")
        from gi.repository import Gio, GLib
        self.Gio, self.GLib = Gio, GLib
        self.bus = Gio.bus_get_sync(Gio.BusType.SYSTEM, None)

    def _v(self, sig, value):
        if sig == "a{sv}":
            return self.GLib.Variant("a{sv}", {k: self.GLib.Variant(s, v) for k, (s, v) in value.items()})
        return self.GLib.Variant(sig, value)

    def call(self, path, iface, method, sig=None, args=()):
        params = None
        if sig:
            types = re.findall(r"a\{sv\}|a?.", sig)
            params = self.GLib.Variant.new_tuple(*[self._v(t, a) for t, a in zip(types, args)])
        try:
            r = self.bus.call_sync(WPA, path, iface, method, params, None, self.Gio.DBusCallFlags.NONE, 15000, None)
        except self.GLib.Error as e:
            raise Refused(f"wpa_supplicant: {e.message}") from None
        return r.unpack() if r is not None else ()

    def get(self, path, iface, prop):
        (v,) = self.call(path, "org.freedesktop.DBus.Properties", "Get", "ss", (iface, prop))
        return v

    def interface(self, ifname):
        (p,) = self.call(WPA_PATH, WPA, "GetInterface", "s", (ifname,))
        return p

    def create(self, ifname):
        (p,) = self.call(WPA_PATH, WPA, "CreateInterface", "a{sv}",
                         ({"Ifname": ("s", ifname), "Driver": ("s", "nl80211")},))
        return p

    def remove(self, path):
        try:
            self.call(WPA_PATH, WPA, "RemoveInterface", "o", (path,))
        except Refused:
            pass


class Links:
    def __init__(self):
        self.wpa = None
        self.undo = []  # callables, last first

    def _wpa(self):
        if self.wpa is None:
            try:
                self.wpa = Wpa()
            except Exception as e:  # noqa: BLE001
                raise Refused(f"wpa_supplicant isn't reachable ({e})") from None
        return self.wpa

    @staticmethod
    def station():
        names = sorted(n for n in os.listdir("/sys/class/net")
                       if os.path.isdir(f"/sys/class/net/{n}/wireless") and not n.startswith(("p2p-", "qs")))
        if not names:
            raise Refused("no Wi-Fi adapter")
        return check_ifname(names[0])

    @staticmethod
    def station_freq(sta):
        m = re.search(r"freq:\s*(\d+)", run("iw", "dev", sta, "link", check=False))
        return int(m.group(1)) if m else 0

    @staticmethod
    def usable():
        return parse_iw_phy(run("iw", "phy", check=False)) if have("iw") else {}

    def _address(self, iface, ip):
        run("ip", "link", "set", iface, "up")
        run("ip", "addr", "add", f"{ip}/24", "dev", iface)
        self.undo.append(lambda: run("ip", "addr", "flush", "dev", iface, check=False))

    def _dhcp_server(self, iface, ip):
        if have("dnsmasq"):
            prefix = ip.rsplit(".", 1)[0]
            p = subprocess.Popen(["dnsmasq", "--keep-in-foreground", "--conf-file=/dev/null", f"--interface={iface}",
                                  "--bind-interfaces", "--except-interface=lo", "--port=0", "--no-resolv",
                                  "--no-hosts", f"--dhcp-range={prefix}.10,{prefix}.99,255.255.255.0,1h",
                                  "--dhcp-option=3", "--dhcp-option=6", "--dhcp-leasefile=/dev/null",
                                  "--pid-file="], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            self.undo.append(lambda: (p.terminate(), p.wait(5)))
        else:
            d = DhcpServer(iface, ip)
            d.start()
            self.undo.append(d.stop)

    def _unmanaged(self, iface):
        if have("nmcli"):
            run("nmcli", "device", "set", iface, "managed", "no", check=False)

    def _virtual(self, sta, name, kind):
        run("iw", "dev", sta, "interface", "add", name, "type", kind)
        self.undo.append(lambda: run("iw", "dev", name, "del", check=False))
        self._unmanaged(name)
        mac = random_mac()
        run("ip", "link", "set", name, "address", mac, check=False)
        return mac

    def host_p2p(self, freq):
        wpa = self._wpa()
        sta = self.station()
        usable = self.usable()
        freq = pick_freq(freq or self.station_freq(sta), usable) if usable else (freq or self.station_freq(sta))
        dev = wpa.interface(sta)
        before = set(wpa.get(WPA_PATH, WPA, "Interfaces"))
        try:
            wpa.call(dev, P2P, "GroupAdd", "a{sv}", (group_add_args(freq, wide=True),))
        except Refused:
            wpa.call(dev, P2P, "GroupAdd", "a{sv}", (group_add_args(freq, wide=False),))
        group_iface = group = None
        end = time.monotonic() + 10
        while time.monotonic() < end and group is None:
            time.sleep(0.1)
            for path in set(wpa.get(WPA_PATH, WPA, "Interfaces")) - before:
                try:
                    g = wpa.get(path, P2P, "Group")
                except Refused:
                    continue
                if g and g != "/":
                    group_iface, group = path, g
                    break
        if group is None:
            raise Refused("the Wi-Fi Direct group didn't start")
        self.undo.append(lambda: wpa.call(group_iface, P2P, "Disconnect"))
        ifname = check_ifname(wpa.get(group_iface, IFACE, "Ifname"))
        ssid = bytes(wpa.get(group, GROUP, "SSID")).decode("utf-8", "replace")
        passphrase = wpa.get(group, GROUP, "Passphrase")
        real = int(wpa.get(group, GROUP, "Frequency") or freq or 0)
        self._address(ifname, GO_IP)
        self._dhcp_server(ifname, GO_IP)
        return {"ssid": ssid, "passphrase": passphrase, "freq": real, "ip": GO_IP, "iface": ifname}

    def host_ap(self, freq):
        wpa = self._wpa()
        sta = self.station()
        here = self.station_freq(sta)
        usable = self.usable()
        # (an access point next to a station has to share its channel)
        freq = here or pick_freq(freq, usable)
        if usable and not usable.get(freq):
            raise Refused("this Wi-Fi channel doesn't allow a hotspot")
        self._virtual(sta, AP_IFACE, "__ap")
        path = wpa.create(AP_IFACE)
        self.undo.append(lambda: wpa.remove(path))
        ssid = "DIRECT-iR-" + "".join(random.SystemRandom().choice("ABCDEFGHJKLMNPQRSTUVWXYZ") for _ in range(4))
        passphrase = new_passphrase()
        (net,) = wpa.call(path, IFACE, "AddNetwork", "a{sv}", (ap_network(ssid, passphrase, freq),))
        wpa.call(path, IFACE, "SelectNetwork", "o", (net,))
        end = time.monotonic() + 10
        while wpa.get(path, IFACE, "State") != "completed":
            if time.monotonic() > end:
                raise Refused("the hotspot didn't start")
            time.sleep(0.1)
        self._address(AP_IFACE, AP_IP)
        self._dhcp_server(AP_IFACE, AP_IP)
        return {"ssid": ssid, "passphrase": passphrase, "freq": freq, "ip": AP_IP, "iface": AP_IFACE}

    def join(self, ssid, passphrase, freq):
        wpa = self._wpa()
        sta = self.station()
        mac = self._virtual(sta, JOIN_IFACE, "managed")
        run("ip", "link", "set", JOIN_IFACE, "up")
        path = wpa.create(JOIN_IFACE)
        self.undo.append(lambda: wpa.remove(path))
        (net,) = wpa.call(path, IFACE, "AddNetwork", "a{sv}", (join_network(ssid, passphrase, freq),))
        wpa.call(path, IFACE, "SelectNetwork", "o", (net,))
        end = time.monotonic() + 20
        while wpa.get(path, IFACE, "State") != "completed":
            if time.monotonic() > end:
                raise Refused("couldn't join the phone's Wi-Fi")
            time.sleep(0.2)
        lease = dhcp_client(JOIN_IFACE, mac)
        if lease:
            ip, mask, gateway = lease
            run("ip", "addr", "add", f"{ip}/{prefix_len(mask)}", "dev", JOIN_IFACE)
        else:
            # (no DHCP answer: an address the phone's group won't hand out soon)
            gateway = GO_IP
            ip = f"192.168.49.{random.randint(200, 250)}"
            run("ip", "addr", "add", f"{ip}/24", "dev", JOIN_IFACE)
        self.undo.append(lambda: run("ip", "addr", "flush", "dev", JOIN_IFACE, check=False))
        return {"ip": ip, "gateway": gateway, "iface": JOIN_IFACE}

    def teardown(self):
        while self.undo:
            step = self.undo.pop()
            try:
                step()
            except Exception:  # noqa: BLE001 (keep tearing down)
                pass


def main():
    links = Links()

    def bye(*_):
        links.teardown()
        os._exit(0)
    signal.signal(signal.SIGTERM, bye)
    signal.signal(signal.SIGHUP, bye)
    if os.geteuid() != 0:
        answer({"ok": False, "error": "The Wi-Fi helper needs root (pkexec)."})
        return
    answer({"ok": True, "ready": True})
    try:
        for line in sys.stdin:
            try:
                req = json.loads(line)
                cmd = req.get("cmd")
                if cmd == "host":
                    links.teardown()
                    freq = check_freq(req.get("freq"))
                    out = links.host_ap(freq) if req.get("mode") == "ap" else links.host_p2p(freq)
                elif cmd == "join":
                    links.teardown()
                    out = links.join(check_ssid(req.get("ssid")), check_passphrase(req.get("passphrase")),
                                     check_freq(req.get("freq")))
                elif cmd == "stop":
                    links.teardown()
                    out = {}
                else:
                    raise Refused("unknown request")
                answer(dict({"ok": True}, **out))
            except Refused as e:
                links.teardown()
                answer({"ok": False, "error": str(e)})
            except (ValueError, AttributeError, OSError, subprocess.SubprocessError) as e:
                links.teardown()
                answer({"ok": False, "error": f"the Wi-Fi link failed ({e})"})
    finally:
        links.teardown()


if __name__ == "__main__":
    main()
