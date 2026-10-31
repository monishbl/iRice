"""assets/pipewire/irice-airplay.conf: the AirPlay client iRice runs
(services/AirPlay.qml). Only speakers on the network: never this computer's
own (raop.discover-local stays off), and nothing besides discovery."""
import re
import unittest
from pathlib import Path

CONF = Path(__file__).resolve().parent.parent / "shell/assets/pipewire/irice-airplay.conf"


class AirPlayConf(unittest.TestCase):
    def setUp(self):
        self.text = "\n".join(l for l in CONF.read_text().splitlines() if not l.lstrip().startswith("#"))

    def test_discovery_module(self):
        self.assertIn("libpipewire-module-raop-discover", self.text)

    def test_not_local(self):
        self.assertNotRegex(self.text, r"discover-local\s*=\s*true")

    def test_client_only(self):
        # a client of the user's PipeWire: no audio devices or servers of its own
        mods = re.findall(r"name\s*=\s*(libpipewire-module-[\w-]+)", self.text)
        self.assertEqual(sorted(mods), sorted(["libpipewire-module-rt", "libpipewire-module-protocol-native",
                                               "libpipewire-module-client-node", "libpipewire-module-adapter",
                                               "libpipewire-module-raop-discover"]))

    def test_balanced(self):
        self.assertEqual(self.text.count("{"), self.text.count("}"))
        self.assertEqual(self.text.count("["), self.text.count("]"))


if __name__ == "__main__":
    unittest.main()
