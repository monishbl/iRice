"""The Quick Share helper's stdin/stdout interface (what the shell uses):
commands in, JSON events out, a real transfer in between, and it ends when
stdin closes."""
import asyncio
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import quickshare_peer as qp  # noqa: E402

from nearby import sharing_pb  # noqa: E402

DAEMON = qp.ENGINE / "qs-daemon.py"


class Daemon(unittest.IsolatedAsyncioTestCase):
    async def test_receive_through_the_json_interface_then_exit(self):
        with tempfile.TemporaryDirectory() as d:
            env = dict(os.environ, IRICE_QS_NO_MDNS="1", IRICE_QS_NO_BLE="1")
            proc = await asyncio.create_subprocess_exec(sys.executable, "-I", str(DAEMON), env=env,
                                                        stdin=asyncio.subprocess.PIPE, stdout=asyncio.subprocess.PIPE)

            async def say(obj):
                proc.stdin.write((json.dumps(obj) + "\n").encode())
                await proc.stdin.drain()

            async def until(kind):
                while True:
                    line = await asyncio.wait_for(proc.stdout.readline(), 10)
                    self.assertTrue(line, "the helper ended early")
                    ev = json.loads(line)
                    if ev.get("ev") == kind:
                        return ev

            await say({"cmd": "config", "visible": True, "name": "Desk", "folder": d})
            status = await until("status")
            self.assertTrue(status["visible"])
            p = await qp.open_peer(status["port"])
            await p.connect_as_sender()
            await p.paired_keys()
            intro = sharing_pb.IntroductionFrame()
            intro.file_metadata.add(name="hello.txt", size=5, payload_id=3)
            await p.send_sharing(qp.SF.INTRODUCTION, introduction=intro)
            offer = await until("incoming")
            self.assertEqual(offer["pin"], p.pin)
            await say({"cmd": "answer", "session": offer["session"], "accept": True})
            await p.recv_sharing()
            await p.chunk(3, qp.PT.PayloadHeader.FILE, 5, 0, b"hello", False, "hello.txt")
            await p.chunk(3, qp.PT.PayloadHeader.FILE, 5, 5, b"", True, "hello.txt")
            done = await until("finished")
            self.assertEqual(Path(done["paths"][0]).read_bytes(), b"hello")
            await p.disconnect()
            proc.stdin.close()  # the shell went away
            self.assertEqual(await asyncio.wait_for(proc.wait(), 10), 0)


if __name__ == "__main__":
    unittest.main()
