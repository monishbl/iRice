"""Quick Share transfers end to end, over real sockets on 127.0.0.1.

A stand-in phone (quickshare_peer.py, written from the protocol side)
sends to iRice's receiver and receives from iRice's sender: accepting,
declining, cancelling halfway, hostile file names, name clashes and a file
large enough to span many chunks.
"""
import asyncio
import hashlib
import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import quickshare_peer as qp  # noqa: E402

from nearby import sharing_pb  # noqa: E402
from nearby.service import QuickShareService  # noqa: E402

SF = sharing_pb.V1Frame
FILE = qp.PT.PayloadHeader.FILE


class Events:
    def __init__(self):
        self.items = []
        self.changed = asyncio.Event()

    def __call__(self, ev):
        self.items.append(ev)
        self.changed.set()

    async def wait(self, kind, timeout=10, **match):
        async def find():
            while True:
                for ev in self.items:
                    if ev.get("ev") == kind and all(ev.get(k) == v for k, v in match.items()):
                        return ev
                self.changed.clear()
                await self.changed.wait()
        return await asyncio.wait_for(find(), timeout)


def intro_file(name, size, pid):
    m = sharing_pb.FileMetadata(name=name, size=size, payload_id=pid, mime_type="application/octet-stream")
    return m


class FixedPort(unittest.IsolatedAsyncioTestCase):
    async def test_a_taken_port_is_waited_for_then_reported(self):
        import socket
        holder = socket.socket()
        holder.bind(("0.0.0.0", 0))
        holder.listen()
        taken = holder.getsockname()[1]
        events = Events()
        svc = QuickShareService(events, None, name="Test laptop")
        svc.preferred_port, svc.port_tries = taken, 2
        try:
            await svc.configure(visible=True)
            self.assertNotEqual(svc.port, taken)
            err = await events.wait("error")
            self.assertIn(f"port {taken} is taken", err["message"])
        finally:
            await svc.close()
            holder.close()

    async def test_the_port_frees_up_in_time(self):
        import socket
        holder = socket.socket()
        holder.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        holder.bind(("0.0.0.0", 0))
        holder.listen()
        port = holder.getsockname()[1]
        asyncio.get_running_loop().call_later(0.2, holder.close)  # (the old helper exits)
        svc = QuickShareService(Events(), None, name="Test laptop")
        svc.preferred_port = port
        try:
            await svc.configure(visible=True)
            self.assertEqual(svc.port, port)
        finally:
            await svc.close()


class Receiving(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.folder = self.dir.name
        self.events = Events()
        self.svc = QuickShareService(self.events, None, folder=self.folder, name="Test laptop")
        self.svc.preferred_port = 0  # (any free port: tests mustn't clash with a running helper)
        await self.svc.configure(visible=True)
        self.port = self.svc.port

    async def asyncTearDown(self):
        await self.svc.close()
        self.dir.cleanup()

    def leftovers(self):
        return [n for n in os.listdir(self.folder) if n.endswith(".quickshare-part")]

    async def phone(self):
        p = await qp.open_peer(self.port, "Pixel 8")
        await p.connect_as_sender()
        await p.paired_keys()
        return p

    async def test_accepted_files_text_and_wifi_arrive_safely(self):
        Path(self.folder, "photo.jpg").write_bytes(b"already here")
        p = await self.phone()
        a = os.urandom(70000)
        b = b"evil"
        intro = sharing_pb.IntroductionFrame()
        intro.file_metadata.append(intro_file("photo.jpg", len(a), 11))
        intro.file_metadata.append(intro_file("../../.bashrc", len(b), 12))
        t = intro.text_metadata.add()
        t.type, t.payload_id, t.text_title, t.size = sharing_pb.TextMetadata.URL, 21, "Link", 22
        w = intro.wifi_credentials_metadata.add()
        w.ssid, w.payload_id = "Home", 31
        await p.send_sharing(SF.INTRODUCTION, introduction=intro)

        offer = await self.events.wait("incoming")
        self.assertEqual(offer["from"], "Pixel 8")
        self.assertEqual(offer["pin"], p.pin)  # both screens show the same code
        self.assertEqual([f["name"] for f in offer["files"]], ["photo.jpg", "bashrc"])
        self.assertEqual(offer["texts"], [{"kind": "url", "title": "Link"}])
        self.assertEqual(offer["wifi"], ["Home"])
        self.svc.answer(offer["session"], True)

        resp = await p.recv_sharing()
        self.assertEqual(resp.type, SF.RESPONSE)
        self.assertEqual(resp.connection_response.status, sharing_pb.ConnectionResponseFrame.ACCEPT)
        for off in range(0, len(a), 30000):
            await p.chunk(11, FILE, len(a), off, a[off:off + 30000], False, "photo.jpg")
        await p.chunk(11, FILE, len(a), len(a), b"", True, "photo.jpg")
        await p.chunk(12, FILE, len(b), 0, b, False, "../../.bashrc")
        await p.chunk(12, FILE, len(b), len(b), b"", True, "../../.bashrc")
        await p.send_bytes(b"https://example.com/x", pid=21)
        await p.send_bytes(sharing_pb.WifiCredentials(password="hunter22").SerializeToString(), pid=31)

        done = await self.events.wait("finished", dir="in")
        names = sorted(os.path.basename(x) for x in done["paths"])
        self.assertEqual(names, ["bashrc", "photo (2).jpg"])
        self.assertEqual(Path(self.folder, "photo.jpg").read_bytes(), b"already here")  # never replaced
        self.assertEqual(Path(self.folder, "photo (2).jpg").read_bytes(), a)
        self.assertFalse(Path(self.folder).parent.joinpath(".bashrc").exists())
        self.assertEqual(done["texts"], [{"kind": "url", "text": "https://example.com/x"}])
        self.assertEqual(done["wifi"], [{"ssid": "Home", "password": "hunter22", "hidden": False}])
        self.assertEqual(self.leftovers(), [])
        await p.disconnect()

    async def test_declining_saves_nothing(self):
        p = await self.phone()
        intro = sharing_pb.IntroductionFrame()
        intro.file_metadata.append(intro_file("a.bin", 5, 1))
        await p.send_sharing(SF.INTRODUCTION, introduction=intro)
        offer = await self.events.wait("incoming")
        self.svc.answer(offer["session"], False)
        resp = await p.recv_sharing()
        self.assertEqual(resp.connection_response.status, sharing_pb.ConnectionResponseFrame.REJECT)
        await self.events.wait("ended", reason="declined")
        self.assertEqual(os.listdir(self.folder), [])

    async def test_a_cancel_halfway_leaves_no_partial_file(self):
        p = await self.phone()
        intro = sharing_pb.IntroductionFrame()
        intro.file_metadata.append(intro_file("big.bin", 100000, 7))
        await p.send_sharing(SF.INTRODUCTION, introduction=intro)
        offer = await self.events.wait("incoming")
        self.svc.answer(offer["session"], True)
        await p.recv_sharing()
        await p.chunk(7, FILE, 100000, 0, os.urandom(40000), False, "big.bin")
        await self.events.wait("progress", dir="in", done=40000)
        await p.send_sharing(SF.CANCEL)
        await self.events.wait("failed", dir="in")
        self.assertEqual(os.listdir(self.folder), [])

    async def test_cancelling_here_tells_the_phone_and_cleans_up(self):
        p = await self.phone()
        intro = sharing_pb.IntroductionFrame()
        intro.file_metadata.append(intro_file("big.bin", 100000, 7))
        await p.send_sharing(SF.INTRODUCTION, introduction=intro)
        offer = await self.events.wait("incoming")
        self.svc.answer(offer["session"], True)
        await p.recv_sharing()
        await p.chunk(7, FILE, 100000, 0, os.urandom(40000), False, "big.bin")
        await self.events.wait("progress", dir="in", done=40000)
        await self.svc.cancel(offer["session"])
        told = await asyncio.wait_for(p.recv_sharing(), 5)
        self.assertEqual(told.type, SF.CANCEL)
        await asyncio.sleep(0.2)
        self.assertEqual(os.listdir(self.folder), [])

    async def test_an_unannounced_file_is_refused(self):
        p = await self.phone()
        intro = sharing_pb.IntroductionFrame()
        intro.file_metadata.append(intro_file("a.bin", 4, 1))
        await p.send_sharing(SF.INTRODUCTION, introduction=intro)
        offer = await self.events.wait("incoming")
        self.svc.answer(offer["session"], True)
        await p.recv_sharing()
        await p.chunk(999, FILE, 4, 0, b"sneak", False, "x.sh")  # not the announced payload
        await self.events.wait("failed", dir="in")
        self.assertEqual(os.listdir(self.folder), [])

    async def test_more_data_than_announced_is_refused(self):
        p = await self.phone()
        intro = sharing_pb.IntroductionFrame()
        intro.file_metadata.append(intro_file("a.bin", 4, 1))
        await p.send_sharing(SF.INTRODUCTION, introduction=intro)
        offer = await self.events.wait("incoming")
        self.svc.answer(offer["session"], True)
        await p.recv_sharing()
        await p.chunk(1, FILE, 4, 0, b"too long", False, "a.bin")
        await self.events.wait("failed", dir="in")
        self.assertEqual(os.listdir(self.folder), [])

    async def test_invisible_means_no_connection(self):
        await self.svc.configure(visible=False)
        with self.assertRaises((ConnectionError, OSError)):
            r, w = await asyncio.open_connection("127.0.0.1", self.port)
            w.write(b"\x00\x00\x00\x01x")
            await w.drain()
            await asyncio.wait_for(r.readexactly(1), 2)


class Sending(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.events = Events()
        self.svc = QuickShareService(self.events, None, folder=self.dir.name, name="Test laptop")
        self.received = asyncio.get_running_loop().create_future()

    async def asyncTearDown(self):
        await self.svc.close()
        self.server.close()
        self.dir.cleanup()

    async def phone_receiver(self, answer):
        """A phone that's visible: accepts the connection, answers `answer`."""
        async def handle(r, w):
            p = qp.Peer(r, w, "Galaxy S24")
            try:
                await p.accept_as_receiver()
                await p.paired_keys()
                intro = await p.recv_sharing()
                assert intro.type == SF.INTRODUCTION
                self.pin_seen = p.pin
                await p.send_sharing(SF.RESPONSE, connection_response=sharing_pb.ConnectionResponseFrame(status=answer))
                files = {}
                if answer == sharing_pb.ConnectionResponseFrame.ACCEPT:
                    want = {m.payload_id: m for m in intro.introduction.file_metadata}
                    while len(files) < len(want) or any(len(files[k]) < want[k].size for k in files):
                        item = await p.recv_sharing()
                        if item is None:
                            break
                        _, pt = item
                        files.setdefault(pt.payload_header.id, b"")
                        assert pt.payload_chunk.offset == len(files[pt.payload_header.id])
                        files[pt.payload_header.id] += pt.payload_chunk.body
                    files = {want[k].name: v for k, v in files.items()}
                await p.disconnect()
                self.received.set_result(files)
            except Exception as e:  # noqa: BLE001
                if not self.received.done():
                    self.received.set_exception(e)

        self.server = await asyncio.start_server(handle, "127.0.0.1", 0)
        return self.server.sockets[0].getsockname()[1]

    async def test_files_arrive_whole_and_the_pins_match(self):
        port = await self.phone_receiver(sharing_pb.ConnectionResponseFrame.ACCEPT)
        big = os.urandom(3 * 1024 * 1024 + 123)  # several 512 KiB chunks
        a = Path(self.dir.name, "holiday.mp4")
        a.write_bytes(big)
        b = Path(self.dir.name, "note.txt")
        b.write_bytes(b"hi")
        await self.svc.send("phone", [str(a), str(b)], host="127.0.0.1", port=port)
        connected = await self.events.wait("connected")
        files = await asyncio.wait_for(self.received, 20)
        self.assertEqual(hashlib.sha256(files["holiday.mp4"]).digest(), hashlib.sha256(big).digest())
        self.assertEqual(files["note.txt"], b"hi")
        self.assertEqual(connected["pin"], self.pin_seen)
        await self.events.wait("finished", dir="out", timeout=20)
        progress = [e for e in self.events.items if e.get("ev") == "progress" and e.get("dir") == "out"]
        self.assertEqual(progress[-1]["done"], len(big) + 2)

    async def test_cancelling_a_send_stops_it(self):
        stalled = asyncio.Event()

        async def handle(r, w):
            p = qp.Peer(r, w, "Galaxy S24")
            await p.accept_as_receiver()
            await p.paired_keys()
            await p.recv_sharing()
            await p.send_sharing(SF.RESPONSE, connection_response=sharing_pb.ConnectionResponseFrame(
                status=sharing_pb.ConnectionResponseFrame.ACCEPT))
            got = []
            while True:
                item = await p.recv_sharing()
                if item is None:
                    break
                if isinstance(item, tuple):
                    got.append(item[1])
                    stalled.set()
                    if item[1].packet_type == qp.PT.CONTROL:
                        break
                elif item.type == SF.CANCEL:
                    break
            self.received.set_result(got)

        self.server = await asyncio.start_server(handle, "127.0.0.1", 0)
        port = self.server.sockets[0].getsockname()[1]
        f = Path(self.dir.name, "huge.bin")
        f.write_bytes(os.urandom(40 * 1024 * 1024))
        sid = await self.svc.send("phone", [str(f)], host="127.0.0.1", port=port)
        await asyncio.wait_for(stalled.wait(), 10)
        await self.svc.cancel(sid)
        got = await asyncio.wait_for(self.received, 10)
        self.assertLess(sum(len(x.payload_chunk.body) for x in got), 40 * 1024 * 1024)  # stopped early
        await self.events.wait("failed", dir="out", message="Cancelled")

    async def test_a_decline_is_reported(self):
        port = await self.phone_receiver(sharing_pb.ConnectionResponseFrame.REJECT)
        f = Path(self.dir.name, "a.txt")
        f.write_bytes(b"x")
        await self.svc.send("phone", [str(f)], host="127.0.0.1", port=port)
        ev = await self.events.wait("declined")
        self.assertEqual(ev["reason"], "declined")

    async def test_unreachable_device_says_what_to_do(self):
        self.server = await asyncio.start_server(lambda r, w: None, "127.0.0.1", 0)
        port = self.server.sockets[0].getsockname()[1]
        self.server.close()
        await self.server.wait_closed()
        f = Path(self.dir.name, "a.txt")
        f.write_bytes(b"x")
        await self.svc.send("phone", [str(f)], host="127.0.0.1", port=port)
        ev = await self.events.wait("failed", dir="out")
        self.assertIn("Keep Quick Share open", ev["message"])


if __name__ == "__main__":
    unittest.main()
