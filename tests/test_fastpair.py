"""fastpair.py: Fast Pair service data (Google's public spec): a model ID in
pairing mode, and batteries (left, right, case) from the other adverts."""
import importlib.util
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "shell/assets/scripts/fastpair.py"
spec = importlib.util.spec_from_file_location("fastpair", SCRIPT)
fp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fp)


class Parse(unittest.TestCase):
    def test_pairing_mode(self):
        self.assertEqual(fp.parse(bytes([0x2C, 0xC1, 0x6A])), {"model": "2CC16A", "pairing": True})

    def test_batteries_after_the_key_filter(self):
        # flags, account key filter (len 4, type 0), salt (len 1, type 1), battery (len 3, type 3)
        data = bytes([0x00, 0x40, 1, 2, 3, 4, 0x11, 0x99, 0x33, 0x80 | 85, 70, 0x7F])
        r = fp.parse(data)
        self.assertFalse(r["pairing"])
        self.assertEqual(r["battery"], {"left": {"level": 85, "charging": True}, "right": {"level": 70, "charging": False}})
        self.assertTrue(r["showUi"])

    def test_hidden_battery(self):
        r = fp.parse(bytes([0x00, 0x34, 50, 50, 0x80 | 20]))
        self.assertFalse(r["showUi"])
        self.assertEqual(r["battery"]["case"], {"level": 20, "charging": True})

    def test_no_battery(self):
        self.assertEqual(fp.parse(bytes([0x00, 0x20, 9, 9])), {"pairing": False})

    def test_truncated(self):
        self.assertEqual(fp.parse(bytes([0x00, 0x00, 0x33, 60])), {"pairing": False, "battery": {"left": {"level": 60, "charging": False}}, "showUi": True})


class Signals(unittest.TestCase):
    """The bus is asked about a device only when an advert can't say."""

    class Params:
        def __init__(self, v):
            self.v = v

        def unpack(self):
            return self.v

    def setUp(self):
        self.w = object.__new__(fp.Watcher)
        self.w.devices, self.w.other, self.asked = {}, set(), []
        props = {"/d/buds": {"Address": "AA:BB", "Alias": "Buds", "RSSI": -50, "ServiceData": {fp.FAST_PAIR: bytes([0x2C, 0xC1, 0x6A])}},
                 "/d/tv": {"Address": "CC:DD", "RSSI": -40, "ServiceData": {}}}
        self.w.call = lambda path, *a, **k: (self.asked.append(path), (props[path],))[1]

    def changed(self, path, **props):
        self.w.on_signal(None, None, path, None, "PropertiesChanged", self.Params(("org.bluez.Device1", props, [])))

    def test_unknown_then_known(self):
        self.changed("/d/buds", RSSI=-50)
        self.assertEqual(self.asked, ["/d/buds"])
        self.assertTrue(self.w.devices["/d/buds"]["pairing"])
        self.changed("/d/buds", RSSI=-45)  # (known: no question)
        self.assertEqual(self.asked, ["/d/buds"])
        self.assertEqual(self.w.devices["/d/buds"]["rssi"], -45)

    def test_other_devices_are_asked_about_once(self):
        for rssi in (-40, -41, -42):
            self.changed("/d/tv", RSSI=rssi)
        self.assertEqual(self.asked, ["/d/tv"])
        self.changed("/d/tv", ServiceData={"0000180f-0000-1000-8000-00805f9b34fb": b"x"})
        self.assertEqual(self.asked, ["/d/tv"])
        self.assertEqual(self.w.devices, {})


if __name__ == "__main__":
    unittest.main()


class ClassicTwin(unittest.TestCase):
    """Connect pairs the earbuds' classic side: the LE advert has no name
    (BlueZ calls it by its address), and pairing with it is refused."""
    AUDIO = 0x240404

    def dev(self, address, name=None, paired=False, rssi=-50, cls=AUDIO):
        return ("/org/bluez/hci0/dev_" + address.replace(":", "_"),
                {"Address": address, "Alias": name or address.replace(":", "-"), "Name": name, "Paired": paired, "RSSI": rssi, "Class": cls})

    def test_an_address_is_not_a_name(self):
        self.assertEqual(fp.real_name("98-34-8C-03-82-DC"), "")
        self.assertEqual(fp.real_name("realme Buds Air 5"), "realme Buds Air 5")

    def test_by_name(self):
        devs = [self.dev("11:22:33:44:55:66", "Other buds"), self.dev("AA:BB:CC:00:00:01", "realme Buds Air 5")]
        self.assertEqual(fp.pick_twin(devs, "realme Buds Air 5", "98:34:8C:03:82:DC"), devs[1][0])

    def test_by_address_tail_when_nameless(self):
        # (the user's buds: LE 98:34:8C:03:82:DC, the classic side shares 03:82:DC)
        devs = [self.dev("11:22:33:44:55:66", "Other buds"), self.dev("29:46:8C:03:82:DC", "Buds")]
        self.assertEqual(fp.pick_twin(devs, "98-34-8C-03-82-DC", "98:34:8C:03:82:DC"), devs[1][0])

    def test_paired_and_non_audio_are_skipped(self):
        devs = [self.dev("29:46:8C:03:82:DC", "Buds", paired=True), self.dev("AA:46:8C:03:82:DC", "Phone", cls=0x5a020c)]
        self.assertIsNone(fp.pick_twin(devs, "", "98:34:8C:03:82:DC", fallback=True))

    def test_fallback_only_one_close_audio_device(self):
        one = [self.dev("11:22:33:44:55:66", "Buds X", rssi=-55), self.dev("11:22:33:44:55:77", "Far", rssi=-90)]
        self.assertIsNone(fp.pick_twin(one, "", "98:34:8C:03:82:DC"))  # (not before the scan is over)
        self.assertEqual(fp.pick_twin(one, "", "98:34:8C:03:82:DC", fallback=True), one[0][0])
        two = one + [self.dev("11:22:33:44:55:88", "Buds Y", rssi=-60)]
        self.assertIsNone(fp.pick_twin(two, "", "98:34:8C:03:82:DC", fallback=True))  # (which? don't guess)
