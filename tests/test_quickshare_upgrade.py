"""Quick Share without a shared network: whole transfers over an in-memory
Bluetooth (weave) link, and the bandwidth upgrade off it.

The stand-in phone (quickshare_peer.py + quickshare_ble_peer.py, written
from the protocol side) talks weave to iRice's service. Upgrades go to a
second TCP port on 127.0.0.1, standing in for the network or a Wi-Fi
Direct group (the root helper is a fake that hands out 127.0.0.1): the
offer, the plain-text introduction and ack, LAST_WRITE / SAFE_TO_CLOSE on
the old link while frames still flow there, the plain-text goodbye, and the
rest of the transfer on the new socket with the same keys and counters.
"""
import asyncio
import ipaddress
import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import quickshare_ble_peer as bp  # noqa: E402
import quickshare_peer as qp  # noqa: E402

from nearby import offline, sharing_pb, weave  # noqa: E402
from nearby.service import QuickShareService  # noqa: E402

SF = sharing_pb.V1Frame
FILE = qp.PT.PayloadHeader.FILE
CR = offline.ConnectionRequestFrame
PATH = offline.BandwidthUpgradeNegotiationFrame.UpgradePathInfo
BWU = offline.BandwidthUpgradeNegotiationFrame


class Events:
    def __init__(self):
        self.items = []
        self.changed = asyncio.Event()

    def __call__(self, ev):
        self.items.append(ev)
        self.changed.set()

    def of(self, kind):
        return [e for e in self.items if e.get("ev") == kind]

    async def wait(self, kind, timeout=10, **match):
        async def find():
            while True:
                for ev in self.items:
                    if ev.get("ev") == kind and all(ev.get(k) == v for k, v in match.items()):
                        return ev
                self.changed.clear()
                await self.changed.wait()
        return await asyncio.wait_for(find(), timeout)


class FakeHelper:
    """The root Wi-Fi helper, minus root: a "group" on 127.0.0.1."""

    def __init__(self, fail=False):
        self.requests = []
        self.released = False
        self.fail = fail
        self.on_auth = lambda: None

    async def request(self, cmd, timeout=40):
        self.requests.append(cmd)
        self.on_auth()
        if self.fail:
            from nearby.wifi import WifiUnavailable
            raise WifiUnavailable("No password was given for the direct Wi-Fi link.")
        if cmd["cmd"] == "host":
            return {"ok": True, "ssid": "DIRECT-iR-TEST", "passphrase": "s3cretpass", "freq": 5180,
                    "ip": "127.0.0.1", "iface": "p2p-wlan0-0"}
        return {"ok": True, "ip": "127.0.0.1", "gateway": "127.0.0.1", "iface": "qsc0"}

    async def release(self):
        self.released = True

    async def close(self):
        pass


def net(shared=False):
    addrs = [("eth0", ipaddress.IPv4Interface("127.0.0.1/8"))] if shared else []
    return {"addresses": addrs, "station": "wlan0", "frequency": 5180, "5ghz": True, "p2p_go": True}


async def read_until_offer(p):
    """Frames until the upgrade offer; the sharing frames on the way."""
    sharing, buf = [], {}
    while True:
        f = await asyncio.wait_for(p.recv(), 10)
        if f.v1.type == qp.V1.BANDWIDTH_UPGRADE_NEGOTIATION:
            return sharing, f
        if f.v1.type == qp.V1.PAYLOAD_TRANSFER:
            pt = f.v1.payload_transfer
            buf[pt.payload_header.id] = buf.get(pt.payload_header.id, b"") + pt.payload_chunk.body
            if pt.payload_chunk.flags & 1:
                s = sharing_pb.Frame()
                s.ParseFromString(buf.pop(pt.payload_header.id))
                sharing.append(s.v1)


class ReceivingOverBluetooth(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.events = Events()
        self.helper = FakeHelper()
        self.net = net(shared=False)

        async def probe():
            return self.net
        self.svc = QuickShareService(self.events, None, folder=self.dir.name, name="Test laptop",
                                     helper=self.helper, net_probe=probe)
        self.svc.preferred_port = self.svc.upgrade_port = 0  # (any free ports)
        self.svc.can_host = lambda n: True
        self.answers = True

        async def reachable(ip):
            self.pinged = ip
            return self.answers
        self.svc.reachable = reachable
        await self.svc.configure(visible=True)
        # The link: the phone is the central, iRice the peripheral (GATT server)
        self.to_phone, self.to_engine, self.phone = bp.pair(engine_central=False, packet_size=185)
        sock = weave.WeaveSocket(self.to_phone, is_client=False, max_packet=244)
        self.to_engine.connect(sock.feed)
        self.task = asyncio.ensure_future(self.svc._incoming_transport(weave.BleSocketTransport(sock, is_client=False)))
        await self.phone.connect()
        self.p = qp.Peer(self.phone.reader, self.phone.writer, "Pixel 9")

    async def asyncTearDown(self):
        await self.svc.close()
        self.to_phone.stop()
        self.to_engine.stop()
        self.dir.cleanup()

    async def offer_file(self, data, mediums, ip=None):
        meta = offline.MediumMetadata(supports_5_ghz=True)
        if ip:
            meta.ip_address = ipaddress.IPv4Address(ip).packed
        await self.p.connect_as_sender(mediums=mediums, metadata=meta)
        await self.p.paired_keys()
        intro = sharing_pb.IntroductionFrame()
        intro.file_metadata.append(sharing_pb.FileMetadata(name="photo.jpg", size=len(data), payload_id=21,
                                                           mime_type="image/jpeg"))
        await self.p.send_sharing(SF.INTRODUCTION, introduction=intro)
        offer = await self.events.wait("incoming")
        self.assertEqual(offer["medium"], "ble")
        self.assertEqual(offer["pin"], self.p.pin)  # the same PIN over Bluetooth
        self.svc.answer(offer["session"], True)
        return offer["session"]

    async def finish(self, data, start=0):
        step = 10000
        for off in range(start, len(data), step):
            await self.p.chunk(21, FILE, len(data), off, data[off:off + step], False, "photo.jpg")
        await self.p.chunk(21, FILE, len(data), len(data), b"", True, "photo.jpg")
        done = await self.events.wait("finished")
        self.assertEqual(Path(done["paths"][0]).read_bytes(), data)
        await self.p.disconnect()
        await asyncio.wait_for(self.task, 10)
        self.assertEqual(self.phone.errors, [])

    async def test_upgrades_to_the_shared_network(self):
        self.net = net(shared=True)
        data = os.urandom(120_000)
        sid = await self.offer_file(data, [CR.WIFI_LAN, CR.WIFI_DIRECT], ip="127.0.0.2")
        sharing, offer = await read_until_offer(self.p)
        self.assertEqual(sharing[-1].connection_response.status, sharing_pb.ConnectionResponseFrame.ACCEPT)
        info = offer.v1.bandwidth_upgrade_negotiation.upgrade_path_info
        self.assertEqual(info.medium, PATH.WIFI_LAN)
        self.assertEqual(bytes(info.wifi_lan_socket.ip_address), bytes([127, 0, 0, 1]))
        self.assertTrue(info.supports_client_introduction_ack)
        self.assertFalse(info.supports_disabling_encryption)
        # Some of the file over Bluetooth, while the upgrade starts
        await self.p.chunk(21, FILE, len(data), 0, data[:30000], False, "photo.jpg")
        seen = []
        await bp.respond_to_offer(self.p, offer, seen)
        self.assertIsInstance(self.p.w, asyncio.StreamWriter)  # the phone is on TCP now
        await self.finish(data, start=30000)
        self.assertEqual([e["medium"] for e in self.events.of("transport") if e["session"] == sid], ["ble", "wifi_lan"])
        self.assertEqual(self.events.of("warning"), [])
        self.assertEqual(self.helper.requests, [])  # (no root needed on a shared network)

    async def test_a_lookalike_network_isnt_offered(self):
        # (a phone on another network with the same numbering: no answer there)
        self.net = net(shared=True)
        self.answers = False
        data = os.urandom(3000)
        await self.offer_file(data, [CR.WIFI_LAN, CR.WIFI_DIRECT], ip="127.0.0.2")
        _, offer = await read_until_offer(self.p)
        self.assertEqual(self.pinged, "127.0.0.2")
        self.assertEqual(offer.v1.bandwidth_upgrade_negotiation.upgrade_path_info.medium, PATH.WIFI_DIRECT)
        await bp.respond_to_offer(self.p, offer, [])
        await self.finish(data)

    async def test_network_refused_then_wifi_direct(self):
        self.net = net(shared=True)
        data = os.urandom(3000)
        sid = await self.offer_file(data, [CR.WIFI_LAN, CR.WIFI_DIRECT], ip="127.0.0.2")
        _, offer = await read_until_offer(self.p)
        self.assertEqual(offer.v1.bandwidth_upgrade_negotiation.upgrade_path_info.medium, PATH.WIFI_LAN)
        await self.p.send(bp.bwu(BWU.UPGRADE_FAILURE, upgrade_path_info=BWU.UpgradePathInfo(medium=PATH.WIFI_LAN)))
        _, offer = await read_until_offer(self.p)  # the next path, at once
        self.assertEqual(offer.v1.bandwidth_upgrade_negotiation.upgrade_path_info.medium, PATH.WIFI_DIRECT)
        await bp.respond_to_offer(self.p, offer, [])
        await self.finish(data)
        self.assertEqual([e["medium"] for e in self.events.of("transport") if e["session"] == sid], ["ble", "wifi_direct"])

    async def test_upgrades_to_a_wifi_direct_group_it_hosts(self):
        data = os.urandom(50_000)
        sid = await self.offer_file(data, [CR.WIFI_DIRECT, CR.WIFI_HOTSPOT, CR.BLE])
        _, offer = await read_until_offer(self.p)
        info = offer.v1.bandwidth_upgrade_negotiation.upgrade_path_info
        self.assertEqual(info.medium, PATH.WIFI_DIRECT)
        c = info.wifi_direct_credentials
        self.assertEqual((c.ssid, c.password, c.gateway, c.frequency), ("DIRECT-iR-TEST", "s3cretpass", "127.0.0.1", 5180))
        self.assertEqual(self.helper.requests[0], {"cmd": "host", "mode": "p2p", "freq": 5180})
        self.assertTrue(self.events.of("auth"))  # the password prompt was announced
        await bp.respond_to_offer(self.p, offer, [])
        await self.finish(data)
        self.assertEqual([e["medium"] for e in self.events.of("transport") if e["session"] == sid], ["ble", "wifi_direct"])
        self.assertTrue(self.helper.released)  # the group goes with the transfer

    async def test_debug_log_names_frames_but_no_secrets(self):
        from nearby import debuglog
        log = Path(self.dir.name, "irice-quickshare.log")
        debuglog._path = str(log)
        try:
            data = os.urandom(5000)
            await self.offer_file(data, [CR.WIFI_DIRECT])
            _, offer = await read_until_offer(self.p)
            await bp.respond_to_offer(self.p, offer, [])
            await self.finish(data)
        finally:
            debuglog._path = None
        text = log.read_text()
        for expected in ("CONNECTION_REQUEST mediums [WIFI_DIRECT]", "UPGRADE_PATH_AVAILABLE WIFI_DIRECT",
                         "← CLIENT_INTRODUCTION [wifi_direct]", "upgraded to wifi_direct", "PAYLOAD_TRANSFER FILE"):
            self.assertIn(expected, text)
        for secret in ("s3cretpass", self.p.pin, "photo.jpg"):
            self.assertNotIn(secret, text)
        self.assertEqual(log.stat().st_mode & 0o777, 0o600)

    async def test_a_failed_upgrade_carries_on_over_bluetooth(self):
        data = os.urandom(20_000)
        await self.offer_file(data, [CR.WIFI_DIRECT])
        _, offer = await read_until_offer(self.p)
        failure = BWU.UpgradePathInfo(medium=offer.v1.bandwidth_upgrade_negotiation.upgrade_path_info.medium)
        await self.p.send(bp.bwu(BWU.UPGRADE_FAILURE, upgrade_path_info=failure))
        warning = await self.events.wait("warning")
        self.assertIn("Bluetooth", warning["message"])
        await self.finish(data)  # all of it over the weave link

    async def test_no_way_up_says_how_to_fix_it(self):
        self.svc.can_host = lambda n: False
        data = b"small"
        await self.offer_file(data, [CR.WIFI_LAN])
        warning = await self.events.wait("warning")
        self.assertIn("same Wi-Fi", warning["message"])
        await self.p.recv_sharing()  # the accept
        await self.finish(data)

    async def test_password_refused_stays_on_bluetooth(self):
        self.helper.fail = True
        data = os.urandom(5000)
        await self.offer_file(data, [CR.WIFI_DIRECT])
        warning = await self.events.wait("warning")
        self.assertIn("password", warning["message"])
        await self.finish(data)


class FakeBluetooth:
    """What the service needs of bluez.Bluetooth to send: a link to a phone."""

    def __init__(self, transport):
        self.transport = transport
        self.powered = True
        self.on_session = self.on_state = None

    def state(self):
        return {"available": True, "powered": True}

    async def connect(self, path):
        return self.transport

    async def unserve(self):
        pass

    async def stop_scan(self):
        pass

    async def close(self):
        pass


class SendingOverBluetooth(unittest.IsolatedAsyncioTestCase):
    async def test_joins_the_phones_wifi_direct_group(self):
        events = Events()
        helper = FakeHelper()
        # iRice is the central now: it connects to the phone's GATT server
        to_phone, to_engine, phone = bp.pair(engine_central=True, packet_size=20)
        sock = weave.WeaveSocket(to_phone, is_client=True, max_packet=20)
        to_engine.connect(sock.feed)
        transport = weave.BleSocketTransport(sock, is_client=True)

        async def probe():
            return net(shared=False)
        svc = QuickShareService(events, None, name="Test laptop", ble=FakeBluetooth(transport), helper=helper,
                                net_probe=probe)
        svc.can_host = lambda n: True
        svc.devices["ble-PH0N"] = {"id": "ble-PH0N", "name": "Pixel 9", "type": "phone", "eid": "PH0N",
                                   "ble": "/org/bluez/hci0/dev_X"}
        with tempfile.TemporaryDirectory() as d:
            path = Path(d, "clip.mp4")
            data = os.urandom(90_000)
            path.write_bytes(data)
            await sock.connect()
            await transport.start()
            sid = await svc.send("ble-PH0N", [str(path)])
            p = qp.Peer(phone.reader, phone.writer, "Pixel 9")
            await p.accept_as_receiver()
            req = p.remote_request
            self.assertEqual(list(req.mediums), [CR.WIFI_LAN, CR.WIFI_DIRECT, CR.WIFI_HOTSPOT])
            self.assertTrue(req.medium_metadata.supports_5_ghz)
            self.assertEqual(req.medium_metadata.ap_frequency, 5180)
            await p.paired_keys()
            v1 = await p.recv_sharing()
            self.assertEqual(v1.type, SF.INTRODUCTION)
            meta = v1.introduction.file_metadata[0]
            await p.send_sharing(SF.RESPONSE, connection_response=sharing_pb.ConnectionResponseFrame(
                status=sharing_pb.ConnectionResponseFrame.ACCEPT))
            # The phone hosts a group (here: a port on 127.0.0.1) and offers it
            arrived = asyncio.Queue()
            server = await asyncio.start_server(lambda r, w: arrived.put_nowait((r, w)), "127.0.0.1", 0)
            info = PATH(medium=PATH.WIFI_DIRECT)
            c = info.wifi_direct_credentials
            c.ssid, c.password, c.frequency, c.gateway = "DIRECT-Ab-Pixel", "phonepass", 5180, "127.0.0.1"
            c.port = server.sockets[0].getsockname()[1]
            who = await bp.offer_and_switch(p, info, arrived, [])
            self.assertEqual(who, svc.endpoint_id)
            self.assertEqual(helper.requests, [{"cmd": "join", "ssid": "DIRECT-Ab-Pixel", "passphrase": "phonepass",
                                                "freq": 5180}])
            # The file comes over the new socket
            got = b""
            while True:
                kind, pt = await asyncio.wait_for(p.recv_sharing(), 10)
                self.assertEqual(kind, "file")
                self.assertEqual(pt.payload_header.id, meta.payload_id)
                got += pt.payload_chunk.body
                if pt.payload_chunk.flags & 1:
                    break
            self.assertEqual(got, data)
            await p.disconnect()
            await events.wait("finished", session=sid)
            self.assertEqual([e["medium"] for e in events.of("transport")], ["ble", "wifi_direct"])
            self.assertEqual(phone.errors, [])
            for _ in range(50):  # (released as the session ends, just after "finished")
                if helper.released:
                    break
                await asyncio.sleep(0.02)
            self.assertTrue(helper.released)
            server.close()
        await svc.close()
        to_phone.stop()
        to_engine.stop()


if __name__ == "__main__":
    unittest.main()
