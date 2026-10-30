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


def mediums_we_join(helper_ok):
    """For our ConnectionRequest: where we can follow a phone."""
    out = [CR.WIFI_LAN]
    if helper_ok:
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


def host_paths(conn, net, helper_ok, reachable=True):
    """The paths to offer a phone that connected over Bluetooth, best first:
    ("lan", our address) when it's on our network (its ConnectionRequest
    says where; `reachable`: it answers there — a phone can name an address
    on a network that only looks like ours), then ("direct", None) and
    ("hotspot", None) when the helper can make them. Empty: stay."""
    theirs = list(conn.remote_mediums)
    meta = conn.remote_metadata
    out = []
    if meta is not None and meta.ip_address and (not theirs or CR.WIFI_LAN in theirs) and reachable:
        mine = same_network(net.get("addresses", []), meta.ip_address)
        if mine:
            out.append(("lan", mine))
    if helper_ok:
        if CR.WIFI_DIRECT in theirs and direct_possible(net):
            out.append(("direct", None))
        if CR.WIFI_HOTSPOT in theirs and hotspot_possible(net):
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
async def join(info, helper, net, helper_ok=True, timeout=15):
    """A connected StreamTransport for a path the phone offers."""
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
        if net.get("iwd"):
            raise WifiUnavailable(no_direct_reason(net))
        if helper is None or not helper_ok:
            raise WifiUnavailable("joining the phone's Wi-Fi needs a Wi-Fi adapter and pkexec")
        link = await helper.request({"cmd": "join", "ssid": c.ssid, "passphrase": c.password,
                                     "freq": int(c.frequency) if c.frequency and c.frequency > 0 else 0}, timeout=60)
        gateway = c.gateway if c.gateway and c.gateway != "0.0.0.0" else link.get("gateway")
        if not gateway:
            raise WifiUnavailable("the phone's Wi-Fi gave no address to connect to")
        medium = "wifi_direct" if info.medium == PATH.WIFI_DIRECT else "wifi_hotspot"
        return await StreamTransport.connect(gateway, c.port, timeout=timeout, medium=medium,
                                             local_address=link.get("ip"))
    raise WifiUnavailable("the phone offered a link iRice doesn't support")
