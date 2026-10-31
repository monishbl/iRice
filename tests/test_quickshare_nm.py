"""Quick Share's Wi-Fi links through NetworkManager (nearby/wifi.py NmWifi),
with stand-in nmcli and iw programs: the hotspot it makes, putting the
Wi-Fi's own connection back (also after a run that ended mid-transfer), and
the firewall check for the hotspot's DHCP (sharing-check.py)."""
import asyncio
import importlib.util
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "shell/assets/scripts/quickshare"))

from nearby import wifi  # noqa: E402

FAKE_NMCLI = r'''#!/usr/bin/env python3
import json, os, sys
d = os.path.dirname(os.path.abspath(__file__))
args = sys.argv[1:]
with open(os.path.join(d, "calls"), "a") as f:
    f.write(json.dumps(args) + "\n")
state = json.load(open(os.path.join(d, "state.json")))
if args[:2] == ["-t", "-f"] and "permissions" in args:
    print("\n".join(f"org.freedesktop.NetworkManager.{k}:{v}" for k, v in state["perms"].items()))
elif args[:2] == ["-g", "WIFI-PROPERTIES.AP"]:
    print(state.get("ap", "yes"))
elif "--active" in args:
    print(f"{state['active']}:wlan0")
elif args[:2] == ["connection", "add"] and state.get("refuse_add"):
    print("Error: not allowed", file=sys.stderr); sys.exit(4)
elif "up" in args and state.get("refuse_up_home") and "HOME-UUID" in args:
    print("Error: Connection activation failed: Unknown reason", file=sys.stderr); sys.exit(4)
elif "up" in args and state.get("refuse_up") and "irice-quickshare" in args:
    print("Error: no luck", file=sys.stderr); sys.exit(4)
'''

FAKE_IW = r'''#!/bin/sh
case "$1" in
phy) printf '\t\t\t* 2412.0 MHz [1] (22.0 dBm)\n\t\t\t* 2437.0 MHz [6] (22.0 dBm)\n\t\t\t* 5180.0 MHz [36] (22.0 dBm) (no IR)\n' ;;
dev) printf 'Interface wlan0\n\ttype AP\n\tchannel 6 (2437 MHz), width: 20 MHz\n' ;;
esac
'''


class NmLinks(unittest.IsolatedAsyncioTestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        d = Path(self.dir.name)
        for name, text in (("nmcli", FAKE_NMCLI), ("iw", FAKE_IW)):
            (d / name).write_text(text)
            (d / name).chmod(0o755)
        self.state = {"perms": {"network-control": "yes", "wifi.share.protected": "yes", "settings.modify.own": "yes"},
                      "active": "HOME-UUID"}
        self.save()
        self.path = os.environ["PATH"]
        os.environ["PATH"] = f"{d}:/usr/bin:/bin"
        self.marker = wifi.NM_MARKER
        wifi.NM_MARKER = str(d / "marker.json")

    def tearDown(self):
        os.environ["PATH"] = self.path
        wifi.NM_MARKER = self.marker
        self.dir.cleanup()

    def save(self):
        Path(self.dir.name, "state.json").write_text(json.dumps(self.state))

    def calls(self):
        f = Path(self.dir.name, "calls")
        return [json.loads(line) for line in f.read_text().splitlines()] if f.exists() else []

    NET = {"station": "wlan0", "nm": True}

    async def test_usable_needs_networkmanager_and_its_permissions(self):
        self.assertTrue(await wifi.NmWifi.usable(self.NET))
        self.assertTrue(await wifi.NmWifi.can_host(self.NET))
        self.assertFalse(await wifi.NmWifi.usable(dict(self.NET, nm=False)))
        self.state["perms"]["wifi.share.protected"] = "auth"  # (would ask for a password)
        self.save()
        self.assertFalse(await wifi.NmWifi.usable(self.NET))

    async def test_hotspot_then_the_old_connection_back(self):
        link = wifi.NmWifi()
        group = await link.host(self.NET)
        self.assertEqual((group["ip"], group["freq"], group["iface"]), ("10.42.0.1", 2437, "wlan0"))
        self.assertTrue(group["ssid"].startswith("DIRECT-iR-"))
        self.assertGreaterEqual(len(group["passphrase"]), 8)
        add = next(c for c in self.calls() if c[:2] == ["connection", "add"])
        opts = dict(zip(add[2::2], add[3::2]))
        self.assertEqual(opts["802-11-wireless.mode"], "ap")
        self.assertEqual(opts["802-11-wireless.band"], "bg")  # (5 GHz is "no IR" here)
        self.assertEqual(opts["ipv4.method"], "shared")
        self.assertEqual(opts["save"], "no")  # (never written to disk)
        self.assertEqual(opts["autoconnect"], "no")
        self.assertEqual(json.loads(Path(wifi.NM_MARKER).read_text())["previous"], "HOME-UUID")
        await link.restore()
        tail = self.calls()[-2:]
        self.assertEqual(tail[0], ["connection", "delete", wifi.NM_PROFILE])
        self.assertEqual(tail[1], ["-w", "20", "connection", "up", "uuid", "HOME-UUID"])
        self.assertFalse(Path(wifi.NM_MARKER).exists())
        n = len(self.calls())
        await link.restore()  # (twice is harmless: nothing more is done)
        self.assertEqual(len(self.calls()), n)

    async def test_a_hotspot_that_wont_start_puts_the_wifi_back(self):
        self.state["refuse_up"] = True
        self.save()
        with self.assertRaises(wifi.WifiUnavailable):
            await wifi.NmWifi().host(self.NET)
        self.assertEqual(self.calls()[-1], ["-w", "20", "connection", "up", "uuid", "HOME-UUID"])
        self.assertFalse(Path(wifi.NM_MARKER).exists())

    async def test_a_busy_device_is_asked_again_then_left_to_networkmanager(self):
        # (seen on a real laptop: "activation failed" right after the hotspot went)
        self.state["refuse_up_home"] = True
        self.save()
        old = wifi.NM_RESTORE_PAUSE
        wifi.NM_RESTORE_PAUSE = 0
        try:
            link = wifi.NmWifi()
            await link.host(self.NET)
            await link.restore()
        finally:
            wifi.NM_RESTORE_PAUSE = old
        ups = [c for c in self.calls() if c[-2:] == ["uuid", "HOME-UUID"]]
        self.assertEqual(len(ups), wifi.NM_RESTORE_TRIES)
        self.assertEqual(self.calls()[-1], ["device", "connect", "wlan0"])

    async def test_recovers_after_a_run_that_ended_mid_transfer(self):
        Path(wifi.NM_MARKER).write_text(json.dumps({"station": "wlan0", "previous": "HOME-UUID"}))
        self.assertTrue(await wifi.NmWifi.recover())
        self.assertEqual(self.calls()[-1], ["-w", "20", "connection", "up", "uuid", "HOME-UUID"])
        self.assertFalse(Path(wifi.NM_MARKER).exists())
        self.assertFalse(await wifi.NmWifi.recover())  # nothing left

    def test_band_choice(self):
        self.assertEqual(wifi.nm_hotspot_band({5180: "no-ir", 2437: "ok"}), ("bg", 6))
        self.assertEqual(wifi.nm_hotspot_band({5180: "ok", 2437: "ok"}), ("a", 36))
        self.assertEqual(wifi.nm_hotspot_band({5180: "radar", 5745: "ok"}), ("a", 149))
        self.assertEqual(wifi.nm_hotspot_band({2437: "no-ir", 2412: "ok"}), ("bg", 1))

    def test_parsers(self):
        self.assertEqual(wifi.parse_iw_link_or_info("\tchannel 6 (2437 MHz), width: 20 MHz"), 2437)
        self.assertEqual(wifi.parse_iw_link_or_info("\tfreq: 5180.0"), 5180)
        self.assertEqual(wifi.parse_nm_permissions("org.freedesktop.NetworkManager.network-control:yes\n"),
                         {"network-control": "yes"})
        self.assertEqual(wifi.mediums_we_join(False, nm_ok=True),
                         [wifi.CR.WIFI_LAN, wifi.CR.WIFI_DIRECT, wifi.CR.WIFI_HOTSPOT])
        self.assertEqual(wifi.mediums_we_join(False), [wifi.CR.WIFI_LAN])


class HotspotFirewall(unittest.TestCase):
    """sharing-check.py: whether DHCP gets through for the hotspot."""

    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.root = Path(self.dir.name)
        (self.root / "etc/ufw").mkdir(parents=True)
        (self.root / "etc/ufw/ufw.conf").write_text("ENABLED=yes\n")
        os.environ["IRICE_SYSTEM_ROOT"] = str(self.root)
        os.environ["XDG_STATE_HOME"] = str(self.root / "state")
        spec = importlib.util.spec_from_file_location("sharing_check", ROOT / "shell/assets/scripts/sharing-check.py")
        self.sc = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.sc)
        self.sc._active = lambda unit: unit == "ufw"

    def tearDown(self):
        os.environ.pop("IRICE_SYSTEM_ROOT", None)
        os.environ.pop("XDG_STATE_HOME", None)
        self.dir.cleanup()

    def rules(self, *lines):
        (self.root / "etc/ufw/user.rules").write_text("\n".join(lines) + "\n")

    def test_ufw_without_dhcp(self):
        self.rules("-A ufw-user-input -p tcp --dport 53317:53319 -j ACCEPT", "-A ufw-user-input -p udp --dport 5353 -j ACCEPT")
        fw = self.sc.firewall()
        self.assertEqual((fw["allowed"], fw["hotspot"]), (True, False))

    def test_ufw_with_dhcp(self):
        self.rules("-A ufw-user-input -p tcp --dport 53317:53319 -j ACCEPT", "-A ufw-user-input -p udp --dport 67 -j ACCEPT")
        self.assertTrue(self.sc.firewall()["hotspot"])

    def test_a_tcp_rule_doesnt_count_for_dhcp(self):
        self.rules("-A ufw-user-input -p tcp --dport 67 -j ACCEPT")
        self.assertFalse(self.sc.firewall()["hotspot"])

    def test_the_fixs_note_says_whether_it_opened_dhcp(self):
        self.sc.remember("ufw", "@@dhcp\n")
        self.assertTrue(self.sc.firewall()["hotspot"])
        self.sc.remember("ufw", "")  # (an older helper's fix)
        self.rules("-A ufw-user-input -p tcp --dport 53318 -j ACCEPT")
        self.assertFalse(self.sc.firewall()["hotspot"])

    def test_no_firewall_lets_it_through(self):
        (self.root / "etc/ufw/ufw.conf").write_text("ENABLED=no\n")
        self.assertTrue(self.sc.firewall()["hotspot"])

    def test_every_fix_opens_dhcp(self):
        script = self.sc.FIREWALL_SCRIPT
        self.assertIn("ufw allow 67/udp", script)
        self.assertIn("--add-service=dhcp", script)
        self.assertIn("{ 53317, 5353, 67 }", script)
        self.assertIn('"-p udp --dport 67"', script)
        self.assertEqual(script.count('echo "@@dhcp"'), 5)


if __name__ == "__main__":
    unittest.main()
