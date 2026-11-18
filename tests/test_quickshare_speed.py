"""Quick Share's speed helpers (18 Nov: Wi-Fi Direct ran 8-9 MB/s, sending to
a phone averaged 1.6 MB/s): the channels told to a phone that hosts, the
stream buffers, progress paced for the shell, and the line that splits a
transfer's setup time from its data time."""
import asyncio
import ipaddress
import os
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import quickshare_peer  # noqa: E402,F401  (puts the engine on the path)

from nearby import transport, wifi  # noqa: E402
from nearby.sharing import Pace, Timeline  # noqa: E402

IW_PHY = """Wiphy phy0
	Band 1:
		Frequencies:
			* 2412.0 MHz [1] (22.0 dBm)
			* 2437.0 MHz [6] (22.0 dBm)
			* 2467.0 MHz [12] (disabled)
	Band 2:
		Frequencies:
			* 5180.0 MHz [36] (22.0 dBm)
			* 5260.0 MHz [52] (22.0 dBm) (no IR, radar detection)
			* 5745.0 MHz [149] (22.0 dBm) (no IR)
			* 5865.0 MHz [173] (disabled)
"""


class Clock:
    def __init__(self):
        self.t = 100.0

    def __call__(self):
        return self.t


class Channels(unittest.TestCase):
    def test_frequencies_as_channel_numbers(self):
        self.assertEqual([wifi.channel_of(f) for f in (2412, 2437, 2472, 2484, 5180, 5745, 5825)], [1, 6, 13, 14, 36, 149, 165])
        self.assertEqual([wifi.channel_of(f) for f in (2400, 5955, 0, 5182)], [None, None, None, None])

    def test_the_channels_a_phone_may_host_on_for_us(self):
        chans = wifi.parse_iw_phy(IW_PHY)
        # (5 GHz first; disabled ones never; "no IR" and radar ones joinable as a client)
        self.assertEqual(wifi.client_channels(chans), [36, 52, 149, 1, 6])
        self.assertEqual(wifi.client_channels(chans, has_5ghz=False), [1, 6])
        self.assertEqual(wifi.client_channels({}), [])

    def test_the_connection_request_carries_them(self):
        net = {"5ghz": True, "frequency": 2437, "station": "wlan0", "channels": wifi.parse_iw_phy(IW_PHY),
               "addresses": [("wlan0", ipaddress.IPv4Interface("192.168.1.20/24"))]}
        m = wifi.medium_metadata(net)
        self.assertTrue(m.supports_5_ghz)
        self.assertEqual(m.ap_frequency, 2437)
        self.assertEqual(list(m.wifi_direct_cli_usable_channels.channels), [36, 52, 149, 1, 6])
        self.assertEqual(list(m.wifi_hotspot_sta_usable_channels.channels), [36, 52, 149, 1, 6])
        # (none known: left out, as before)
        bare = wifi.medium_metadata({"5ghz": False})
        self.assertFalse(bare.HasField("wifi_direct_cli_usable_channels"))
        # (and the switch that leaves them out)
        os.environ["TESSERA_QS_NO_CHANNELS"] = "1"
        try:
            self.assertFalse(wifi.medium_metadata(net).HasField("wifi_direct_cli_usable_channels"))
        finally:
            del os.environ["TESSERA_QS_NO_CHANNELS"]


class Buffers(unittest.IsolatedAsyncioTestCase):
    async def test_a_whole_chunk_fits_and_the_next_is_made_while_one_goes_out(self):
        # (512 KiB of file plus its encryption and framing)
        self.assertGreater(transport.READ_LIMIT, 512 * 1024 + 4096)
        self.assertGreater(transport.WRITE_HIGH, 512 * 1024 + 4096)
        got = asyncio.get_running_loop().create_future()

        async def serve(reader, writer):
            got.set_result(await transport.StreamTransport(reader, writer).read_frame(5))
            writer.close()

        server = await asyncio.start_server(serve, "127.0.0.1", 0, limit=transport.READ_LIMIT)
        port = server.sockets[0].getsockname()[1]
        t = await transport.StreamTransport.connect("127.0.0.1", port)
        self.assertEqual(t._writer.transport.get_write_buffer_limits()[1], transport.WRITE_HIGH)
        self.assertEqual(t._reader._limit, transport.READ_LIMIT)
        frame = os.urandom(600 * 1024)
        await t.write_frame(frame)
        self.assertEqual(await asyncio.wait_for(got, 5), frame)
        await t.close()
        server.close()
        await server.wait_closed()


class Paced(unittest.TestCase):
    def test_progress_first_last_and_every_tenth_of_a_second(self):
        c = Clock()
        p = Pace(0.1, clock=c)
        self.assertTrue(p.due(1, 100))  # (the first)
        c.t += 0.02
        self.assertFalse(p.due(2, 100))
        c.t += 0.02
        self.assertFalse(p.due(3, 100))
        c.t += 0.07
        self.assertTrue(p.due(4, 100))
        c.t += 0.01
        self.assertTrue(p.due(100, 100))  # (the last, always)


class Timed(unittest.TestCase):
    def test_setup_and_data_split(self):
        c = Clock()
        t = Timeline(clock=c)
        for name, after in (("paired", 0.8), ("offered", 0.1), ("accepted", 2.0), ("first", 1.5), ("last", 10.0), ("done", 0.4)):
            c.t += after
            t.mark(name)
        c.t += 5
        t.mark("first")  # (the first mark of each step stands)
        line = t.summary(85_000_000, "wifi_direct")
        self.assertEqual(line, "setup 4.4 s (connect 0.8 s, decide 2.0 s, link 1.5 s), data 85.0 MB in 10.0 s = 8.5 MB/s, "
                               "end 0.4 s, over wifi_direct")

    def test_a_share_without_files(self):
        c = Clock()
        t = Timeline(clock=c)
        c.t += 1
        t.mark("paired")
        c.t += 1
        t.mark("done")
        self.assertEqual(t.summary(0), "setup 2.0 s (connect 1.0 s)")


if __name__ == "__main__":
    unittest.main()
