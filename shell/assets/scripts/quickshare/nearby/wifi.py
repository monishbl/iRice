"""Faster links for a connection that started over Bluetooth (stage 3).

Which one, like Android picks (connections/implementation/bwu_manager.cc and
the mediums' BWU handlers):
  WIFI_LAN     both on the same network: this computer's address there and a
               port (UPGRADE_PORT); nothing to set up
  WIFI_DIRECT  no shared network: one side hosts a Wi-Fi Direct group, the
               other joins it (192.168.49.x), then TCP to the owner's port
  WIFI_HOTSPOT the same with a plain access point, if Wi-Fi Direct can't
Receiving, this computer offers the path (it hosts); sending, the phone
offers (it hosts) and this computer joins. Hosting and joining need root
(wpa_supplicant's D-Bus API is root-only; NetworkManager can't host a
Wi-Fi Direct group): qs-wifi-helper.py runs under pkexec, started only
when such a link is needed and kept a few minutes for the next transfer.
Once an update installed iRice's root helper, it's the copy there
(/usr/lib/irice/irice-root wifi-helper); before, the one here. Either way
it asks for the password as it starts (it can make and join Wi-Fi
networks, more than iRice's silent jobs may), never during a transfer.
It ends — and takes every group, interface and DHCP server it made with it
— when its stdin closes, so nothing outlives the helper.
"""
import asyncio
import ipaddress
import json
import os
import shutil
import sys

from . import debuglog, offline
from .transport import StreamTransport

UPGRADE_PORT = 53319  # next to the Quick Share port (53318), for firewall rules
HELPER = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "qs-wifi-helper.py")
# iRice's root helper, once installed (IRICE_SYSTEM_ROOT: tests)
ROOT_HELPER = os.path.join(os.environ.get("IRICE_SYSTEM_ROOT", "/"), "usr/lib/irice/irice-root")
HELPER_IDLE = 600  # the helper (and its password) stays this long after a transfer
AUTH_TIMEOUT = 120  # for typing the password
PATH = offline.BandwidthUpgradeNegotiationFrame.UpgradePathInfo
CR = offline.ConnectionRequestFrame


class WifiUnavailable(Exception):
    """No way to make that link (the message says why, for the user)."""


def _log(what):
    debuglog.log("wifi", what)


async def _run(*cmd, timeout=5):
    if not shutil.which(cmd[0]):
        return ""
    try:
        p = await asyncio.create_subprocess_exec(*cmd, stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.DEVNULL)
        out, _ = await asyncio.wait_for(p.communicate(), timeout)
        return out.decode(errors="replace") if p.returncode == 0 else ""
    except (OSError, asyncio.TimeoutError):
        return ""


# ── This computer's network ──────────────────────────────────────────────
def parse_ip_addr(text):
    """`ip -j -4 addr` → [(ifname, IPv4Interface)] without loopback."""
    try:
        data = json.loads(text or "[]")
    except ValueError:
        return []
    out = []
    for link in data:
        for a in link.get("addr_info", []):
            if a.get("family") == "inet" and a.get("local") and not link.get("ifname", "").startswith("lo"):
                try:
                    out.append((link["ifname"], ipaddress.IPv4Interface(f"{a['local']}/{a.get('prefixlen', 32)}")))
                except ValueError:
                    pass
    return out


def same_network(ours, phone_ip):
    """Our address on the phone's network (as bytes), or None."""
    try:
        theirs = ipaddress.IPv4Address(bytes(phone_ip))
    except ValueError:
        return None
    for ifname, iface in ours:
        if ifname.startswith(("p2p-", "qs")):
            continue  # (our own Wi-Fi Direct links)
        if theirs in iface.network and theirs != iface.ip and iface.network.prefixlen < 32:
            return iface.ip.packed
    return None


def parse_iw_link(text):
    """`iw dev X link` → the station's frequency in MHz, or None."""
    for line in text.splitlines():
        line = line.strip()
        if line.startswith("freq:"):
            try:
                return int(float(line.split()[1]))
            except (IndexError, ValueError):
                return None
    return None


def parse_iw_list(text):
    """`iw list` → {"5ghz": bool, "p2p_go": bool}."""
    return {"5ghz": "Band 2:" in text or "* 5180 MHz" in text, "p2p_go": "P2P-GO" in text}


def wifi_interfaces():
    try:
        return sorted(n for n in os.listdir("/sys/class/net") if os.path.isdir(f"/sys/class/net/{n}/wireless")
                      and not n.startswith(("p2p-", "qs")))
    except OSError:
        return []


def nm_manages(nmcli_out, station):
    """`nmcli -t -f DEVICE,STATE device` → whether NetworkManager runs this
    Wi-Fi. It then ends every Wi-Fi Direct group it didn't start itself:
    told of the new group interface (P2PDevice.GroupStarted, on the
    station's own wpa_supplicant object), it wraps it, and unless its own
    P2P device is connecting nothing keeps that, so it's disposed with a
    RemoveInterface (NetworkManager's nm-supplicant-interface.c; seen on a
    real laptop: the group removed 2 ms after it started, on every channel)."""
    for line in nmcli_out.splitlines():
        dev, _, state = line.partition(":")
        if dev == station:
            return not state.startswith("unmanaged")
    return False


def wifi_driver(station):
    try:
        return os.path.basename(os.readlink(f"/sys/class/net/{station}/device/driver"))
    except OSError:
        return ""


async def network():
    """{"addresses": [(ifname, IPv4Interface)], "station": ifname or None,
    "frequency": MHz or None, "5ghz", "p2p_go", "nm": NetworkManager runs
    the Wi-Fi, "driver", "iwd": iwd is the Wi-Fi daemon (on its own, or
    under NetworkManager)}."""
    addresses = parse_ip_addr(await _run("ip", "-j", "-4", "addr", "show"))
    stations = wifi_interfaces()
    station = stations[0] if stations else None
    freq = parse_iw_link(await _run("iw", "dev", station, "link")) if station else None
    caps = parse_iw_list(await _run("iw", "list")) if station else {"5ghz": False, "p2p_go": False}
    nm = nm_manages(await _run("nmcli", "-t", "-f", "DEVICE,STATE", "device"), station) if station else False
    iwd = bool(await _run("busctl", "--system", "status", "net.connman.iwd"))
    return {"addresses": addresses, "station": station, "frequency": freq, "nm": nm, "iwd": iwd,
            "driver": wifi_driver(station) if station else "", **caps}


def direct_possible(net):
    """Whether a Wi-Fi Direct group we host can last (see nm_manages). The
    helper makes links through wpa_supplicant, which doesn't run next to iwd."""
    return net.get("p2p_go", True) and not net.get("nm", False) and not net.get("iwd", False)


def hotspot_possible(net):
    """Intel Wi-Fi doesn't run an access point on 5 GHz, and one next to a
    connection has to share its channel (qs-wifi-helper.py ap_freq). Not
    under iwd either (wpa_supplicant again)."""
    return not (net.get("driver") == "iwlwifi" and (net.get("frequency") or 0) >= 5000) and not net.get("iwd", False)


def no_direct_reason(net):
    """Why there's no Wi-Fi Direct or hotspot link here, for the person (a
    shared network and Bluetooth still work)."""
    if net.get("iwd"):
        return "iwd runs the Wi-Fi here, and Wi-Fi Direct needs wpa_supplicant instead"
    if net.get("nm") and net.get("p2p_go", True):
        return "NetworkManager ends Wi-Fi Direct links it didn't make"
    return "Wi-Fi Direct isn't available here"


def medium_metadata(net):
    """What the ConnectionRequest says about our Wi-Fi (a phone that hosts
    picks its group's channel from it: ours, on 5 GHz when we can)."""
    m = offline.MediumMetadata(supports_5_ghz=bool(net.get("5ghz")), supports_6_ghz=False, mobile_radio=False)
    m.ap_frequency = net["frequency"] if net.get("frequency") else -1
    for ifname, iface in net.get("addresses", []):
        if ifname == net.get("station"):
            m.ip_address = iface.ip.packed
            break
    return m


def helper_possible(net):
    # (under iwd there's no wpa_supplicant for the helper: no password asked for nothing)
    return bool(net.get("station")) and not net.get("iwd", False) and bool(shutil.which("pkexec")) and \
        (os.path.exists(HELPER) or os.access(ROOT_HELPER, os.X_OK))


def mediums_we_join(helper_ok, nm_ok=False):
    """For our ConnectionRequest: where we can follow a phone (its group or
    hotspot through the root helper, or NetworkManager)."""
    out = [CR.WIFI_LAN]
    if helper_ok or nm_ok:
        out += [CR.WIFI_DIRECT, CR.WIFI_HOTSPOT]
    return out


# ── The root helper ──────────────────────────────────────────────────────
class WifiHelper:
    """qs-wifi-helper.py under pkexec: one JSON request, one JSON answer."""

    def __init__(self, on_auth=None, helper=HELPER, root_helper=ROOT_HELPER):
        self.helper = helper
        self.root_helper = root_helper
        self.on_auth = on_auth or (lambda: None)
        self.proc = None
        self._lock = asyncio.Lock()
        self._idle = None

    async def _start(self):
        if self.proc and self.proc.returncode is None:
            return
        if not shutil.which("pkexec"):
            raise WifiUnavailable("Wi-Fi Direct needs pkexec (from polkit) to set up the link.")
        self.on_auth()  # (a password prompt is coming: say what for)
        if self.root_helper and os.access(self.root_helper, os.X_OK):
            cmd = ["pkexec", self.root_helper, "wifi-helper"]
        else:
            cmd = ["pkexec", sys.executable, "-I", self.helper]
        _log("starting the Wi-Fi helper (" + " ".join(cmd[:3]) + ")")
        self.proc = await asyncio.create_subprocess_exec(
            *cmd, stdin=asyncio.subprocess.PIPE,
            stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.DEVNULL)
        try:
            line = await asyncio.wait_for(self.proc.stdout.readline(), AUTH_TIMEOUT)
        except asyncio.TimeoutError:
            await self.close()
            raise WifiUnavailable("No password was given for the direct Wi-Fi link.") from None
        if not line:
            code = await self.proc.wait()
            self.proc = None
            raise WifiUnavailable("The direct Wi-Fi link wasn't allowed (the password prompt was dismissed)."
                                  if code in (126, 127) else "The Wi-Fi helper couldn't start.")
        try:
            hello = json.loads(line)
        except ValueError:
            hello = {}
        if not hello.get("ok"):
            await self.close()
            raise WifiUnavailable(hello.get("error") or "The Wi-Fi helper couldn't start.")

    async def request(self, cmd, timeout=40):
        async with self._lock:
            if self._idle:
                self._idle.cancel()
                self._idle = None
            await self._start()
            redacted = {k: ("…" if k == "passphrase" else v) for k, v in cmd.items()}
            _log(f"helper ← {redacted}")
            try:
                self.proc.stdin.write((json.dumps(cmd) + "\n").encode())
                await self.proc.stdin.drain()
                line = await asyncio.wait_for(self.proc.stdout.readline(), timeout)
            except (OSError, asyncio.TimeoutError) as e:
                await self.close()
                raise WifiUnavailable("The Wi-Fi helper stopped answering.") from e
            if not line:
                await self.close()
                raise WifiUnavailable("The Wi-Fi helper stopped.")
            reply = json.loads(line)
            _log(f"helper → {({k: ('…' if k == 'passphrase' else v) for k, v in reply.items()})}")
            if not reply.get("ok"):
                raise WifiUnavailable(reply.get("error") or "The direct Wi-Fi link couldn't be set up.")
            return reply

    async def release(self):
        """Tears the links down; the helper waits a while for the next one."""
        if not (self.proc and self.proc.returncode is None):
            return
        try:
            await self.request({"cmd": "stop"}, timeout=15)
        except WifiUnavailable:
            pass
        if self._idle is None:
            self._idle = asyncio.ensure_future(self._close_later())

    async def _close_later(self):
        await asyncio.sleep(HELPER_IDLE)
        self._idle = None
        await self.close()

    async def close(self):
        p, self.proc = self.proc, None
        if p is None or p.returncode is not None:
            return
        try:
            p.stdin.close()  # (the helper tears everything down on EOF)
            await asyncio.wait_for(p.wait(), 15)
        except (OSError, asyncio.TimeoutError):
            pass


# ── Through NetworkManager: no root, no password ─────────────────────────
NM_PROFILE = "irice-quickshare"
NM_MARKER = os.path.join(os.environ.get("XDG_RUNTIME_DIR") or "/tmp", "irice-quickshare-wifi.json")
NM_HOTSPOT_IP = "10.42.0.1"  # NetworkManager's "shared" network
NM_MIN_BYTES = 256 * 1024  # smaller shares stay on Bluetooth (no pause for a photo thumbnail or a link)
NM_JOIN_WAIT = 25
NM_RESTORE_TRIES, NM_RESTORE_PAUSE = 4, 1.5


def parse_iw_phy(text):
    """`iw phy` → {MHz: "ok" | "no-ir" | "radar" | "disabled"} (qs-wifi-helper.py's)."""
    out = {}
    for line in text.splitlines():
        line = line.strip()
        if not line.startswith("* ") or "MHz" not in line:
            continue
        try:
            freq = int(float(line.split()[1]))
        except (IndexError, ValueError):
            continue
        out[freq] = ("disabled" if "disabled" in line else "radar" if "radar detection" in line
                     else "no-ir" if "no IR" in line or "passive scanning" in line else "ok")
    return out


def nm_hotspot_band(chans):
    """("a" | "bg", channel) for a hotspot of our own, alone on the radio: a
    5 GHz channel a computer may start transmitting on (not "no IR", not
    radar), else 2.4 GHz (Intel chips mark all of 5 GHz "no IR")."""
    for f, ch in ((5180, 36), (5200, 40), (5220, 44), (5240, 48), (5745, 149), (5765, 153), (5785, 157), (5805, 161)):
        if chans.get(f) == "ok":
            return "a", ch
    for f, ch in ((2437, 6), (2412, 1), (2462, 11)):
        if chans.get(f, "ok") == "ok":
            return "bg", ch
    return "bg", 6


def parse_nm_permissions(text):
    """`nmcli -t -f PERMISSION,VALUE general permissions` → {name: value}."""
    out = {}
    for line in text.splitlines():
        name, _, value = line.partition(":")
        out[name.removeprefix("org.freedesktop.NetworkManager.")] = value.strip()
    return out


async def _nmcli(*args, timeout=40):
    """(returncode, stdout, stderr) of nmcli; 127 without it."""
    if not shutil.which("nmcli"):
        return 127, "", "no nmcli"
    try:
        p = await asyncio.create_subprocess_exec("nmcli", *args, stdout=asyncio.subprocess.PIPE,
                                                 stderr=asyncio.subprocess.PIPE)
        out, err = await asyncio.wait_for(p.communicate(), timeout)
        return p.returncode, out.decode(errors="replace"), err.decode(errors="replace")
    except asyncio.TimeoutError:
        return 124, "", "nmcli took too long"
    except OSError as e:
        return 126, "", str(e)


class NmWifi:
    """A Wi-Fi link through NetworkManager, using this computer's own Wi-Fi
    for the transfer: hosting a hotspot (receiving) or joining the phone's
    group (sending). No root and no password (NetworkManager lets the person
    at the computer share and switch Wi-Fi), and NetworkManager's own
    connection, so it doesn't end it the way it ends a Wi-Fi Direct group it
    didn't start. The price: the Wi-Fi's usual connection (and the internet
    over it) pauses during the transfer, then comes back. For computers whose
    chip can't run a second network next to the first (Intel: no hotspot on
    5 GHz, a group next to a connection ended by NetworkManager), which is
    where a shared network or Bluetooth were the only ways before.

    A marker (NM_MARKER) names the connection to bring back, so a helper that
    ended mid-transfer is cleaned up by the next one (recover())."""

    def __init__(self):
        self.station = None
        self.previous = None  # UUID of the connection to bring back
        self.active = False

    @staticmethod
    async def usable(net):
        """Whether NetworkManager runs the Wi-Fi and lets us switch it."""
        sta = net.get("station")
        if not sta or not net.get("nm") or not shutil.which("nmcli"):
            return False
        code, out, _ = await _nmcli("-t", "-f", "PERMISSION,VALUE", "general", "permissions", timeout=5)
        perms = parse_nm_permissions(out) if code == 0 else {}
        return perms.get("network-control") == "yes" and perms.get("wifi.share.protected") == "yes" and \
            perms.get("settings.modify.own") == "yes"

    @staticmethod
    async def can_host(net):
        if not await NmWifi.usable(net):
            return False
        code, out, _ = await _nmcli("-g", "WIFI-PROPERTIES.AP", "device", "show", net["station"], timeout=5)
        return code == 0 and out.strip() == "yes"

    async def _remember(self, sta):
        code, out, _ = await _nmcli("-t", "-f", "UUID,DEVICE", "connection", "show", "--active", timeout=5)
        prev = None
        for line in out.splitlines():
            uuid, _, dev = line.partition(":")
            if dev == sta:
                prev = uuid
        self.station, self.previous = sta, prev
        try:
            fd = os.open(NM_MARKER + ".tmp", os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
            with os.fdopen(fd, "w") as f:
                json.dump({"station": sta, "previous": prev}, f)
            os.replace(NM_MARKER + ".tmp", NM_MARKER)
        except OSError:
            pass

    def _profile_args(self, sta, ssid, passphrase, extra):
        user = os.environ.get("USER") or ""
        return (["connection", "add", "type", "wifi", "ifname", sta, "con-name", NM_PROFILE, "autoconnect", "no",
                 "save", "no", "ssid", ssid] + extra +
                ["ipv6.method", "disabled", "wifi-sec.key-mgmt", "wpa-psk", "wifi-sec.psk", passphrase,
                 "wifi-sec.proto", "rsn", "wifi-sec.pairwise", "ccmp", "wifi-sec.group", "ccmp"] +
                (["connection.permissions", f"user:{user}"] if user else []))

    async def host(self, net):
        """A hotspot of our own: {"ssid", "passphrase", "freq", "ip", "iface"}."""
        sta = net["station"]
        band, channel = nm_hotspot_band(parse_iw_phy(await _run("iw", "phy")))
        ssid = "DIRECT-iR-" + new_token(4)
        passphrase = new_token(12)
        await _nmcli("connection", "delete", NM_PROFILE, timeout=10)
        await self._remember(sta)
        code, _, err = await _nmcli(*self._profile_args(sta, ssid, passphrase, [
            "802-11-wireless.mode", "ap", "802-11-wireless.band", band, "802-11-wireless.channel", str(channel),
            "ipv4.method", "shared"]), timeout=10)
        if code != 0:
            await self.restore()
            raise WifiUnavailable(f"NetworkManager wouldn't make a hotspot ({err.strip() or code})")
        self.active = True
        _log(f"hotspot via NetworkManager on {sta}, band {band} channel {channel} (was {self.previous})")
        code, _, err = await _nmcli("-w", "30", "connection", "up", NM_PROFILE, timeout=40)
        if code != 0:
            await self.restore()
            raise WifiUnavailable(f"the hotspot didn't start ({err.strip() or code})")
        freq = parse_iw_link_or_info(await _run("iw", "dev", sta, "info")) or 0
        return {"ssid": ssid, "passphrase": passphrase, "freq": freq, "ip": NM_HOTSPOT_IP, "iface": sta}

    async def join(self, net, ssid, passphrase):
        """Joins a phone's group or hotspot: {"ip", "gateway", "iface"}."""
        sta = net["station"]
        await _nmcli("connection", "delete", NM_PROFILE, timeout=10)
        await self._remember(sta)
        code, _, err = await _nmcli(*self._profile_args(sta, ssid, passphrase, ["ipv4.method", "auto",
                                                                                "ipv4.never-default", "yes"]), timeout=10)
        if code != 0:
            await self.restore()
            raise WifiUnavailable(f"NetworkManager wouldn't join the phone's Wi-Fi ({err.strip() or code})")
        self.active = True
        _log(f"joining the phone's Wi-Fi via NetworkManager on {sta} (was {self.previous})")
        code, _, err = await _nmcli("-w", str(NM_JOIN_WAIT), "connection", "up", NM_PROFILE, timeout=NM_JOIN_WAIT + 10)
        if code != 0:
            await self.restore()
            raise WifiUnavailable(f"couldn't join the phone's Wi-Fi ({err.strip() or code})")
        addrs = [a for n, a in parse_ip_addr(await _run("ip", "-j", "-4", "addr", "show", sta)) if n == sta]
        if not addrs:
            await self.restore()
            raise WifiUnavailable("the phone's Wi-Fi gave this computer no address")
        code, out, _ = await _nmcli("-g", "IP4.GATEWAY,DHCP4.OPTION", "device", "show", sta, timeout=5)
        gateway = next((w for w in out.replace("|", " ").split() if w.count(".") == 3 and w[0].isdigit()), "")
        return {"ip": str(addrs[0].ip), "gateway": gateway, "iface": sta}

    async def restore(self):
        """Back to the connection the Wi-Fi had (or none)."""
        if not (self.active or self.station):
            return
        self.active = False
        await _nmcli("connection", "delete", NM_PROFILE, timeout=15)
        if self.previous:
            # (right after the link goes, the device can still be busy: a few tries)
            for attempt in range(NM_RESTORE_TRIES):
                code, _, err = await _nmcli("-w", "20", "connection", "up", "uuid", self.previous, timeout=30)
                _log(f"Wi-Fi back on {self.previous}: {'ok' if code == 0 else err.strip() or code}")
                if code == 0:
                    break
                await asyncio.sleep(NM_RESTORE_PAUSE)
            else:
                # (whatever NetworkManager would connect to by itself)
                await _nmcli("device", "connect", self.station, timeout=30)
        self.station = self.previous = None
        try:
            os.remove(NM_MARKER)
        except OSError:
            pass

    @staticmethod
    async def recover():
        """A marker from a run that ended mid-transfer: put the Wi-Fi back."""
        try:
            with open(NM_MARKER) as f:
                m = json.load(f)
        except (OSError, ValueError):
            return False
        w = NmWifi()
        w.station, w.previous, w.active = m.get("station"), m.get("previous"), True
        _log(f"putting the Wi-Fi back after an earlier run ({w.previous})")
        await w.restore()
        return True


def new_token(n):
    import secrets
    alphabet = "ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnpqrstuvwxyz23456789"
    return "".join(secrets.choice(alphabet) for _ in range(n))


def parse_iw_link_or_info(text):
    """The frequency from `iw dev X link` ("freq: 2437") or `iw dev X info`
    ("channel 6 (2437 MHz)")."""
    f = parse_iw_link(text)
    if f:
        return f
    for line in text.splitlines():
        if "MHz" in line and "channel" in line:
            try:
                return int(float(line.split("(")[1].split()[0]))
            except (IndexError, ValueError):
                pass
    return None


# ── Paths: hosting (we offer) ────────────────────────────────────────────
class Listener:
    """The TCP port a phone connects to after our offer."""

    def __init__(self, medium):
        self.medium = medium
        self.server = None
        self.port = 0
        self._arrived = asyncio.Queue()

    async def open(self, host="0.0.0.0", port=UPGRADE_PORT):
        async def incoming(reader, writer):
            self._arrived.put_nowait(StreamTransport(reader, writer, self.medium))
        try:
            self.server = await asyncio.start_server(incoming, host=host, port=port)
        except OSError:
            self.server = await asyncio.start_server(incoming, host=host, port=0)
        self.port = self.server.sockets[0].getsockname()[1]
        return self.port

    async def accept(self, timeout):
        try:
            return await asyncio.wait_for(self._arrived.get(), timeout)
        except asyncio.TimeoutError:
            return None

    async def close(self):
        if self.server:
            self.server.close()
            try:
                await asyncio.wait_for(self.server.wait_closed(), 2)
            except asyncio.TimeoutError:
                pass
            self.server = None
        while not self._arrived.empty():
            await self._arrived.get_nowait().close()


def lan_path(ip, port):
    info = PATH(medium=PATH.WIFI_LAN)
    info.wifi_lan_socket.ip_address = ip
    info.wifi_lan_socket.wifi_port = port
    return info


def direct_path(group, port):
    info = PATH(medium=PATH.WIFI_DIRECT)
    c = info.wifi_direct_credentials
    c.ssid, c.password, c.port, c.frequency = group["ssid"], group["passphrase"], port, int(group.get("freq") or 0)
    c.gateway = group["ip"]
    return info


def hotspot_path(group, port):
    info = PATH(medium=PATH.WIFI_HOTSPOT)
    c = info.wifi_hotspot_credentials
    c.ssid, c.password, c.port, c.frequency = group["ssid"], group["passphrase"], port, int(group.get("freq") or -1)
    c.gateway = group["ip"]
    return info


def host_paths(conn, net, helper_ok, reachable=True, nm_host=False):
    """The paths to offer a phone that connected over Bluetooth, best first:
    ("lan", our address) when it's on our network (its ConnectionRequest
    says where; `reachable`: it answers there — a phone can name an address
    on a network that only looks like ours), then ("direct", None) when the
    helper can make a group that lasts, then ("nm", None), a hotspot through
    NetworkManager (`nm_host`: NmWifi.can_host; no password, the Wi-Fi's own
    connection pauses), then the helper's ("hotspot", None). Empty: stay."""
    theirs = list(conn.remote_mediums)
    meta = conn.remote_metadata
    out = []
    if meta is not None and meta.ip_address and (not theirs or CR.WIFI_LAN in theirs) and reachable:
        mine = same_network(net.get("addresses", []), meta.ip_address)
        if mine:
            out.append(("lan", mine))
    if helper_ok and CR.WIFI_DIRECT in theirs and direct_possible(net):
        out.append(("direct", None))
    if nm_host and CR.WIFI_HOTSPOT in theirs:
        out.append(("nm", None))
    if helper_ok and CR.WIFI_HOTSPOT in theirs and hotspot_possible(net):
        out.append(("hotspot", None))
    return out


def phone_address(conn):
    meta = conn.remote_metadata
    if meta is None or len(meta.ip_address) != 4:
        return None
    return str(ipaddress.IPv4Address(bytes(meta.ip_address)))


async def reachable(ip):
    """Whether something answers at this address (one ping, 1 s); True when
    ping isn't there to ask."""
    if not ip:
        return False
    if not shutil.which("ping"):
        return True
    try:
        p = await asyncio.create_subprocess_exec("ping", "-c", "1", "-W", "1", "-n", "-q", ip,
                                                 stdout=asyncio.subprocess.DEVNULL, stderr=asyncio.subprocess.DEVNULL)
        return await asyncio.wait_for(p.wait(), 3) == 0
    except (OSError, asyncio.TimeoutError):
        return False


def group_frequency(net, meta):
    """A channel hint for the group: ours if we're on Wi-Fi (the radio stays
    on one channel), else the phone's (it can stay on its network)."""
    if net.get("frequency"):
        return net["frequency"]
    if meta is not None and meta.ap_frequency and meta.ap_frequency > 0:
        return meta.ap_frequency
    return None


# ── Paths: joining (the phone offers) ────────────────────────────────────
async def join(info, helper, net, helper_ok=True, timeout=15, nm=None):
    """A connected StreamTransport for a path the phone offers (`nm`: an
    NmWifi that joins it instead of the helper)."""
    if info.medium == PATH.WIFI_LAN:
        s = info.wifi_lan_socket
        try:
            host = str(ipaddress.IPv4Address(bytes(s.ip_address)))
        except ValueError as e:
            raise WifiUnavailable("the phone offered an address iRice can't use") from e
        if same_network(net.get("addresses", []), s.ip_address) is None:
            raise WifiUnavailable("the phone isn't on this computer's network")
        return await StreamTransport.connect(host, s.wifi_port, timeout=timeout, medium="wifi_lan")
    if info.medium in (PATH.WIFI_DIRECT, PATH.WIFI_HOTSPOT):
        c = info.wifi_direct_credentials if info.medium == PATH.WIFI_DIRECT else info.wifi_hotspot_credentials
        if not c.ssid or not c.password:
            raise WifiUnavailable("the phone offered a Wi-Fi Direct link without a password (not supported)")
        medium = "wifi_direct" if info.medium == PATH.WIFI_DIRECT else "wifi_hotspot"
        if nm is not None:
            link = await nm.join(net, c.ssid, c.password)
            gateway = c.gateway if c.gateway and c.gateway != "0.0.0.0" else link.get("gateway")
            try:
                if not gateway:
                    raise WifiUnavailable("the phone's Wi-Fi gave no address to connect to")
                return await StreamTransport.connect(gateway, c.port, timeout=timeout, medium=medium)
            except BaseException:
                await nm.restore()  # (the transfer stays on Bluetooth: the Wi-Fi comes back now)
                raise
        if net.get("iwd"):
            raise WifiUnavailable(no_direct_reason(net))
        if helper is None or not helper_ok:
            raise WifiUnavailable("joining the phone's Wi-Fi needs a Wi-Fi adapter and pkexec")
        link = await helper.request({"cmd": "join", "ssid": c.ssid, "passphrase": c.password,
                                     "freq": int(c.frequency) if c.frequency and c.frequency > 0 else 0}, timeout=60)
        gateway = c.gateway if c.gateway and c.gateway != "0.0.0.0" else link.get("gateway")
        if not gateway:
            raise WifiUnavailable("the phone's Wi-Fi gave no address to connect to")
        return await StreamTransport.connect(gateway, c.port, timeout=timeout, medium=medium,
                                             local_address=link.get("ip"))
    raise WifiUnavailable("the phone offered a link iRice doesn't support")
