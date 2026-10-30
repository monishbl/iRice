"""audio-profiles.py: outputs that need their sound card switched over (a
monitor's speakers on the laptop's sound chip), listed and switched with a
fake pactl."""
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "shell/assets/scripts/audio-profiles.py"


def card(active, hdmi_available="available"):
    return {
        "name": "alsa_card.pci-0000_00_1f.3",
        "active_profile": active,
        "profiles": {p: {"available": True} for p in (
            "output:analog-stereo", "output:analog-stereo+input:analog-stereo",
            "output:hdmi-stereo", "output:hdmi-stereo+input:analog-stereo", "output:hdmi-surround")},
        "ports": {
            "analog-output-speaker": {"availability": "availability unknown", "description": "Speakers",
                                      "profiles": ["output:analog-stereo", "output:analog-stereo+input:analog-stereo"]},
            "analog-output-headphones": {"availability": "not available", "description": "Headphones",
                                         "profiles": ["output:analog-stereo", "output:analog-stereo+input:analog-stereo"]},
            "hdmi-output-0": {"availability": hdmi_available, "description": "HDMI / DisplayPort",
                              "properties": {"device.product.name": "BenQ GW2786TC"},
                              "profiles": ["output:hdmi-stereo", "output:hdmi-stereo+input:analog-stereo", "output:hdmi-surround"]},
            "hdmi-output-1": {"availability": "not available", "description": "HDMI / DisplayPort 2",
                              "profiles": ["output:hdmi-stereo-extra1"]},
            "analog-input-mic": {"availability": "available", "description": "Microphone",
                                 "profiles": ["output:analog-stereo+input:analog-stereo"]},
        },
    }


class AudioProfiles(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.root = Path(self.dir.name)
        self.state = self.root / "state.json"
        self.log = self.root / "log"
        (self.root / "pactl").write_text(f"""#!/usr/bin/env python3
import json, sys
st = json.load(open({str(self.state)!r}))
a = sys.argv[1:]
open({str(self.log)!r}, "a").write(" ".join(a) + "\\n")
if a[:4] == ["-f", "json", "list", "cards"]:
    print(json.dumps(st["cards"]))
elif a[:1] == ["set-card-profile"]:
    st["cards"][0]["active_profile"] = a[2]
    json.dump(st, open({str(self.state)!r}, "w"))
elif a[:3] == ["list", "short", "sinks"]:
    out = st["cards"][0]["active_profile"].split("+")[0].removeprefix("output:")
    print("56\\talsa_output.pci-0000_00_1f.3." + out + "\\tPipeWire\\ts32le 2ch 48000Hz\\tSUSPENDED")
""")
        (self.root / "pactl").chmod(0o755)
        self.env = dict(os.environ, PATH=f"{self.root}:/usr/bin:/bin")

    def tearDown(self):
        self.dir.cleanup()

    def run_script(self, *args):
        r = subprocess.run(["python3", "-I", str(SCRIPT), *args], env=self.env, capture_output=True, text=True, timeout=20)
        return r.returncode, json.loads(r.stdout or "null")

    def set_cards(self, *cards):
        self.state.write_text(json.dumps({"cards": list(cards)}))

    def test_monitor_speakers_offered_keeping_the_microphone(self):
        self.set_cards(card("output:analog-stereo+input:analog-stereo"))
        code, rows = self.run_script("list")
        self.assertEqual(code, 0)
        self.assertEqual(rows, [{"card": "alsa_card.pci-0000_00_1f.3", "profile": "output:hdmi-stereo+input:analog-stereo",
                                 "name": "BenQ GW2786TC speakers", "kind": "display"}])

    def test_nothing_offered_without_a_monitor(self):
        self.set_cards(card("output:analog-stereo+input:analog-stereo", hdmi_available="not available"))
        self.assertEqual(self.run_script("list")[1], [])

    def test_switch_over_and_back(self):
        self.set_cards(card("output:analog-stereo+input:analog-stereo"))
        code, r = self.run_script("switch", "alsa_card.pci-0000_00_1f.3", "output:hdmi-stereo+input:analog-stereo")
        self.assertEqual((code, r["ok"], r["sink"]), (0, True, "alsa_output.pci-0000_00_1f.3.hdmi-stereo"))
        self.assertIn("set-default-sink alsa_output.pci-0000_00_1f.3.hdmi-stereo", self.log.read_text())
        # now the laptop's speakers are the one to switch back to
        rows = self.run_script("list")[1]
        self.assertEqual([(x["name"], x["profile"]) for x in rows],
                         [("Built-in speakers", "output:analog-stereo+input:analog-stereo")])

    def test_a_profile_not_offered_is_refused(self):
        self.set_cards(card("output:analog-stereo+input:analog-stereo"))
        code, r = self.run_script("switch", "alsa_card.pci-0000_00_1f.3", "pro-audio")
        self.assertEqual((code, r["ok"]), (1, False))
        self.assertNotIn("set-card-profile", self.log.read_text())


if __name__ == "__main__":
    unittest.main()
