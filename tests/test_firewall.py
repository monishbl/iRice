"""Sharing through nftables and iptables firewalls: how sharing-check.py
tells them apart, what irice-root.py's fixed firewall script does there
(against a fake nft, iptables and systemctl; the script's /etc paths point
at a temporary folder), how long a fix counts for, and Quick Share's Wi-Fi
links under nftables (qs-wifi-helper.py). Nothing real is asked or changed."""
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPTS = ROOT / "shell/assets/scripts"


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


# What `nft list chains` prints on Arch's default nftables.conf, with an
# iptables-nft table next to it (docker's, say)
CHAINS = """\
table inet filter {
	chain input {
		type filter hook input priority filter; policy drop;
	}
	chain forward {
		type filter hook forward priority filter; policy drop;
	}
}
table ip filter {
	chain INPUT {
		type filter hook input priority filter; policy accept;
	}
}
table bridge br {
	chain in {
		type filter hook input priority filter; policy accept;
	}
}
"""

# One stand-in for nft, iptables, ip6tables, systemctl, ufw and firewall-cmd
FAKE = r'''#!/usr/bin/env python3
import os, sys
name = os.path.basename(sys.argv[0]); args = " ".join(sys.argv[1:])
open(os.environ["FAKE_LOG"], "a").write(name + " " + args + "\n")
if name == "systemctl":
    on = sys.argv[-1] in os.environ.get("FAKE_UNITS", "").split(",")
    if "--quiet" not in sys.argv:
        print("active" if on else "inactive")
    sys.exit(0 if on else 3)
if name in ("ufw", "firewall-cmd"):
    sys.exit(1)
if name == "nft":
    if args == "list chains":
        sys.stdout.write(open(os.environ["FAKE_CHAINS"]).read())
    elif args.startswith("-a list chain"):
        print('\t\ttcp dport 53317-53319 accept comment "irice sharing" # handle 7')
        print('\t\ttcp dport 22 accept # handle 3')
    elif args.startswith("-c -f"):
        sys.exit(int(os.environ.get("FAKE_CHECK", "0")))
    elif args.startswith("--echo --handle insert"):
        print("insert rule " + " ".join(sys.argv[4:]) + " # handle 42")
if name in ("iptables", "ip6tables") and "-C" in sys.argv:
    sys.exit(1)  # (not there yet)
'''


class Fakes(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        d = Path(self.tmp.name)
        self.bin = d / "bin"
        self.bin.mkdir()
        (self.bin / "fake").write_text(FAKE)
        (self.bin / "fake").chmod(0o755)
        for c in ("nft", "iptables", "ip6tables", "systemctl", "ufw", "firewall-cmd", "pkexec"):
            (self.bin / c).symlink_to(self.bin / "fake")
        self.etc = d / "etc"
        self.etc.mkdir()
        self.sysroot = d / "root"
        self.home = d / "home"
        (self.home / ".local/state").mkdir(parents=True)
        self.log = d / "calls"
        self.log.write_text("")
        (d / "chains").write_text(CHAINS)
        self.env = dict(os.environ, PATH=f"{self.bin}:/usr/bin:/bin", HOME=str(self.home),
                        XDG_STATE_HOME=str(self.home / ".local/state"), IRICE_SYSTEM_ROOT=str(self.sysroot),
                        FAKE_LOG=str(self.log), FAKE_CHAINS=str(d / "chains"), FAKE_UNITS="")

    def tearDown(self):
        self.tmp.cleanup()

    def calls(self):
        return self.log.read_text().splitlines()


class FixScript(Fakes):
    def fix(self, **env):
        root = load("irice_root", SCRIPTS / "irice-root.py")
        script = root.FIREWALL_SCRIPT.replace("/etc/", str(self.etc) + "/")
        return subprocess.run(["sh", "-c", script], env=dict(self.env, **env), capture_output=True, text=True, timeout=20)

    def test_nftables_rules_go_first_in_each_input_chain_until_a_restart(self):
        p = self.fix(FAKE_UNITS="nftables")
        self.assertEqual(p.returncode, 0, p.stderr)
        calls = self.calls()
        # the earlier rule of ours goes, then the new ones, only in inet filter input
        self.assertIn("nft delete rule inet filter input handle 7", calls)
        self.assertIn('nft insert rule inet filter input tcp dport 53317-53319 accept comment "irice sharing"', calls)
        self.assertIn('nft insert rule inet filter input udp dport { 53317, 5353 } accept comment "irice sharing"', calls)
        self.assertFalse([c for c in calls if "insert" in c and (" ip filter " in c or " bridge " in c)], calls)
        self.assertFalse([c for c in calls if "add table" in c], calls)
        # Arch's nftables.conf includes nothing: runtime only, and it says so
        self.assertIn("@@runtime", p.stdout.splitlines())
        self.assertIn("until the next restart", p.stdout)
        self.assertFalse((self.etc / "nftables.d").exists())

    def test_nftables_with_a_drop_in_folder_keeps_them(self):
        (self.etc / "nftables.d").mkdir()
        # (the include as the script sees it: its /etc is the temporary one)
        (self.etc / "nftables.conf").write_text(f'table inet filter {{ }}\ninclude "{self.etc}/nftables.d/*.nft"\n')
        p = self.fix(FAKE_UNITS="nftables")
        self.assertEqual(p.returncode, 0, p.stderr)
        self.assertNotIn("@@runtime", p.stdout)
        drop = (self.etc / "nftables.d/irice.nft").read_text()
        self.assertIn('insert rule inet filter input tcp dport 53317-53319 accept comment "irice sharing"', drop)
        self.assertIn("nft -c -f " + str(self.etc) + "/nftables.conf", self.calls())
        # nftables couldn't load the file at boot: it's taken out again, runtime only
        p = self.fix(FAKE_UNITS="nftables", FAKE_CHECK="1")
        self.assertEqual(p.returncode, 0, p.stderr)
        self.assertIn("@@runtime", p.stdout)
        self.assertFalse((self.etc / "nftables.d/irice.nft").exists())

    def test_iptables_rules_are_inserted_and_it_says_how_to_keep_them(self):
        p = self.fix(FAKE_UNITS="iptables")
        self.assertEqual(p.returncode, 0, p.stderr)
        calls = self.calls()
        self.assertIn('iptables -I INPUT -p tcp --dport 53317:53319 -m comment --comment irice sharing -j ACCEPT', calls)
        self.assertIn('iptables -I INPUT -p udp --dport 5353 -m comment --comment irice sharing -j ACCEPT', calls)
        self.assertFalse([c for c in calls if c.startswith("ip6tables")])  # (ip6tables.service isn't on)
        self.assertIn("@@runtime", p.stdout.splitlines())
        self.assertIn("iptables-save", p.stdout)

    def test_no_firewall_says_so(self):
        p = self.fix()
        self.assertEqual(p.returncode, 1)
        self.assertIn("nftables or iptables", p.stderr)


class Detection(Fakes):
    def status(self):
        p = subprocess.run([sys.executable, "-I", str(SCRIPTS / "sharing-check.py"), "status"], env=self.env,
                           capture_output=True, text=True, timeout=20)
        self.assertEqual(p.returncode, 0, p.stderr)
        return json.loads(p.stdout)["firewall"]

    def tool(self, name):
        (self.sysroot / "usr/bin").mkdir(parents=True, exist_ok=True)
        (self.sysroot / "usr/bin" / name).write_text("")

    def test_an_active_nftables_or_iptables_service_may_block(self):
        self.assertEqual(self.status()["kind"], None)
        self.env["FAKE_UNITS"] = "nftables"
        self.assertEqual(self.status()["kind"], None)  # (no nft installed)
        self.tool("nft")
        f = self.status()
        self.assertEqual((f["kind"], f["allowed"], f["blocks"]), ("nftables", None, True))
        self.tool("iptables")
        self.env["FAKE_UNITS"] = "iptables"
        self.assertEqual(self.status()["kind"], "iptables")
        # Arch's iptables.service ruleset accepts everything: nothing blocks
        (self.sysroot / "etc/iptables").mkdir(parents=True)
        (self.sysroot / "etc/iptables/iptables.rules").write_text("*filter\n:INPUT ACCEPT [0:0]\nCOMMIT\n")
        self.assertFalse(self.status()["blocks"])
        (self.sysroot / "etc/iptables/iptables.rules").write_text("*filter\n:INPUT DROP [0:0]\nCOMMIT\n")
        self.assertTrue(self.status()["blocks"])

    def health_row(self):
        """health-check.py's firewall row alone (the other checks would ask real services)."""
        code = ("import importlib.util, json, sys; s = importlib.util.spec_from_file_location('h', sys.argv[1]); "
                "h = importlib.util.module_from_spec(s); s.loader.exec_module(h); print(json.dumps(h.check_firewall(None)))")
        p = subprocess.run([sys.executable, "-I", "-c", code, str(SCRIPTS / "health-check.py")], env=self.env,
                           capture_output=True, text=True, timeout=20)
        self.assertEqual(p.returncode, 0, p.stderr)
        return json.loads(p.stdout)

    def test_a_runtime_fix_counts_only_for_this_boot(self):
        self.tool("nft")
        self.env["FAKE_UNITS"] = "nftables"
        (self.bin / "pkexec").unlink()
        (self.bin / "pkexec").write_text("#!/bin/sh\necho '@@runtime'\necho 'Allowed until the next restart.'\n")
        (self.bin / "pkexec").chmod(0o755)
        p = subprocess.run([sys.executable, "-I", str(SCRIPTS / "sharing-check.py"), "fix", "firewall"], env=self.env,
                           capture_output=True, text=True, timeout=20)
        r = json.loads(p.stdout)
        self.assertTrue(r["ok"], p.stdout)
        self.assertEqual(r["note"], "Allowed until the next restart.")
        f = self.status()
        self.assertEqual((f["blocks"], f["runtime"]), (False, True))
        marker = self.home / ".local/state/irice/sharing-firewall"
        self.assertIn("nftables boot=", marker.read_text())
        marker.write_text("nftables boot=another-boot\n")  # (after a restart)
        self.assertTrue(self.status()["blocks"])
        # the health row says how long, and shows the nftables command
        row = self.health_row()
        self.assertEqual(row["status"], "warn")
        self.assertIn("nftables is on and may block", row["detail"])
        self.assertIn("nft insert rule", row["fixCommand"])
        marker.write_text("nftables boot=" + Path("/proc/sys/kernel/random/boot_id").read_text().strip() + "\n")
        row = self.health_row()
        self.assertEqual(row["status"], "ok")
        self.assertIn("until the next restart", row["detail"])


class WifiLinks(unittest.TestCase):
    def test_input_chains(self):
        helper = load("qs_wifi_helper", SCRIPTS / "quickshare/qs-wifi-helper.py")
        self.assertEqual(helper.nft_input_chains(CHAINS), [("inet", "filter", "input")])
        self.assertEqual(helper.nft_input_chains(textwrap.dedent("""\
            table ip6 mine {
                chain in6 {
                    type filter hook input priority -10; policy drop;
                }
            }""")), [("ip6", "mine", "in6")])
        self.assertEqual(helper.nft_input_chains(""), [])


if __name__ == "__main__":
    unittest.main()
