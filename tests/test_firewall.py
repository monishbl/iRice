"""Sharing through a firewall: what irice-root.py's sharing-ports job does
with ufw, firewalld, nftables and iptables (against fake ones on PATH: the
calls are recorded; its /run and /etc are a temporary folder), how
sharing-check.py tells the firewalls apart and notes what's open (for this
boot, or through a restart with ufw), System health's row for it, and Quick
Share's Wi-Fi links under nftables (qs-wifi-helper.py). Nothing real is
asked or changed."""
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path
from unittest import mock

ROOT = Path(__file__).resolve().parent.parent
SCRIPTS = ROOT / "shell/assets/scripts"


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    m = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(m)
    return m


R = load("irice_root_ports", SCRIPTS / "irice-root.py")

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

# `ufw status numbered` after the permanent fix before sharing-ports, with
# rules of the person's own (one of them on mDNS's port, one called "mDNS")
UFW = """\
Status: active

     To                         Action      From
     --                         ------      ----
[ 1] 22/tcp                     ALLOW IN    Anywhere
[ 2] 53317:53320/tcp            ALLOW IN    Anywhere                   # iRice sharing
[ 3] 53317/udp                  ALLOW IN    Anywhere                   # iRice sharing (LocalSend)
[ 4] 5353/udp                   ALLOW IN    Anywhere                   # mDNS
[ 5] 67/udp                     ALLOW IN    Anywhere                   # iRice Quick Share hotspot (DHCP)
[ 6] 5000/udp                   ALLOW IN    Anywhere                   # mDNS
[ 7] 22/tcp (v6)                ALLOW IN    Anywhere (v6)
[ 8] 53317:53320/tcp (v6)       ALLOW IN    Anywhere (v6)              # iRice sharing
[ 9] 5353/udp (v6)              ALLOW IN    Anywhere (v6)              # mDNS
"""

# An input chain with an old rule of ours (the fix before: 53317-53320), one
# still wanted while visible (mDNS), and the person's ssh
NFT_CHAIN = """\
table inet filter {
	chain input {
		type filter hook input priority filter; policy drop;
		udp dport 5353 accept comment "irice sharing" # handle 9
		tcp dport 53317-53320 accept comment "irice sharing" # handle 7
		tcp dport 22 accept # handle 3
	}
}
"""

# `iptables -L INPUT -n --line-numbers` (iptables 1.8.9+ names protocols by number)
IPT = """\
Chain INPUT (policy DROP)
num  target     prot opt source               destination
1    ACCEPT     6    --  0.0.0.0/0            0.0.0.0/0            tcp dpts:53317:53320 /* irice sharing */
2    ACCEPT     6    --  0.0.0.0/0            0.0.0.0/0            tcp dpt:22
3    ACCEPT     17   --  0.0.0.0/0            0.0.0.0/0            udp dpt:5353 /* irice sharing */
"""

# One stand-in for nft, iptables, ip6tables, systemctl, ufw, firewall-cmd and
# pkexec. FAKE_UNITS: the services on; FAKE_FWD_HAS: ports firewalld has
# already; FAKE_FWD_OLD: the permanent fix's ports are in firewalld's
# permanent set; FAKE_PKEXEC_EXIT: pkexec's exit code
FAKE = r'''#!/usr/bin/env python3
import os, sys
name = os.path.basename(sys.argv[0]); args = " ".join(sys.argv[1:])
open(os.environ["FAKE_LOG"], "a").write(name + " " + args + "\n")
units = os.environ.get("FAKE_UNITS", "").split(",")
if name == "systemctl":
    on = sys.argv[-1] in units
    if "--quiet" not in sys.argv:
        print("active" if on else "inactive")
    sys.exit(0 if on else 3)
if name == "ufw":
    if "ufw" not in units:
        print("Status: inactive")
    elif args in ("status", "status numbered"):
        sys.stdout.write(open(os.environ["FAKE_UFW"]).read())
if name == "firewall-cmd":
    if "firewalld" not in units:
        sys.exit(252)
    if args == "--get-default-zone":
        print("public")
    elif args.startswith("--permanent --query-port="):
        sys.exit(0 if os.environ.get("FAKE_FWD_OLD") else 1)
    elif args.startswith("--query-port="):
        sys.exit(0 if args.split("=", 1)[1] in os.environ.get("FAKE_FWD_HAS", "").split(",") else 1)
if name == "nft":
    if args == "list chains":
        sys.stdout.write(open(os.environ["FAKE_CHAINS"]).read())
    elif args.startswith("-a list chain"):
        sys.stdout.write(open(os.environ["FAKE_NFT"]).read())
    elif args.startswith("--echo --handle insert"):
        print("insert rule " + " ".join(sys.argv[4:]) + " # handle 42")
if name in ("iptables", "ip6tables") and args == "-L INPUT -n --line-numbers":
    sys.stdout.write(open(os.environ["FAKE_IPT"]).read())
if name == "pkexec":
    sys.exit(int(os.environ.get("FAKE_PKEXEC_EXIT", "0")))
'''


class Fakes(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        d = self.d = Path(self.tmp.name)
        self.bin = d / "bin"
        self.bin.mkdir()
        (self.bin / "fake").write_text(FAKE)
        (self.bin / "fake").chmod(0o755)
        for c in ("nft", "iptables", "ip6tables", "systemctl", "ufw", "firewall-cmd", "pkexec"):
            (self.bin / c).symlink_to(self.bin / "fake")
        self.prefix = d / "prefix"  # (the helper's /: /run, /etc, /usr)
        self.prefix.mkdir()
        self.sysroot = d / "root"
        self.home = d / "home"
        (self.home / ".local/state").mkdir(parents=True)
        self.log = d / "calls"
        self.log.write_text("")
        for name, text in (("chains", CHAINS), ("ufw", UFW), ("nft", NFT_CHAIN), ("ipt", IPT)):
            (d / name).write_text(text)
        self.env = dict(os.environ, PATH=f"{self.bin}:/usr/bin:/bin", HOME=str(self.home),
                        XDG_STATE_HOME=str(self.home / ".local/state"), IRICE_SYSTEM_ROOT=str(self.sysroot),
                        FAKE_LOG=str(self.log), FAKE_CHAINS=str(d / "chains"), FAKE_UFW=str(d / "ufw"),
                        FAKE_NFT=str(d / "nft"), FAKE_IPT=str(d / "ipt"), FAKE_UNITS="")

    def tearDown(self):
        self.tmp.cleanup()

    def calls(self):
        return self.log.read_text().splitlines()


class SharingPortsJob(Fakes):
    """irice-root sharing-ports open <sets> | close: exactly the sets asked
    for, runtime rules where the firewall has them, only ours taken out, and
    the permanent fix before it migrated away."""

    def ports(self, sets, **env):
        self.log.write_text("")
        with mock.patch.dict(os.environ, dict(self.env, **env)):
            done, failed = R.sharing_ports(sets, prefix=str(self.prefix))
        self.assertEqual(failed, [])
        return done, self.calls()

    def changes(self, calls, *tools):
        return [c for c in calls if c.split()[0] in tools and any(w in c for w in ("allow", "delete", "-D", "-I", "insert", "--add", "--remove"))]

    def test_ufw_opens_what_is_asked_and_takes_out_only_ours(self):
        _, calls = self.ports(["quickshare", "mdns"], FAKE_UNITS="ufw")
        # the fix before's 53317-53320 and LocalSend's 53317, last first;
        # mDNS and DHCP stay (wanted); the person's rules are never touched
        self.assertEqual(self.changes(calls, "ufw"), ["ufw --force delete 8", "ufw --force delete 3", "ufw --force delete 2",
                                                      "ufw allow 53318:53319/tcp comment iRice sharing"])
        _, calls = self.ports([], FAKE_UNITS="ufw")
        self.assertEqual(self.changes(calls, "ufw"), [f"ufw --force delete {n}" for n in (9, 8, 5, 4, 3, 2)])

    def test_ufw_leaves_a_port_someone_allowed_to_them(self):
        (self.d / "ufw").write_text("Status: active\n\n[ 1] 53317/tcp                  ALLOW IN    Anywhere\n")
        _, calls = self.ports(["localsend"], FAKE_UNITS="ufw")
        self.assertEqual(self.changes(calls, "ufw"), ["ufw allow 53317/udp comment iRice sharing"])
        _, calls = self.ports([], FAKE_UNITS="ufw")
        self.assertEqual(self.changes(calls, "ufw"), [])

    def test_firewalld_runtime_only_and_the_permanent_fix_goes(self):
        _, calls = self.ports(["quickshare", "mdns"], FAKE_UNITS="firewalld", FAKE_FWD_OLD="1", FAKE_FWD_HAS="5353/udp")
        old = "--remove-port=53317-53320/tcp --remove-port=53317/udp --remove-service=mdns --remove-service=dhcp"
        self.assertIn(f"firewall-cmd --permanent {old}", calls)
        self.assertIn(f"firewall-cmd {old}", calls)  # (and now, without a reload)
        self.assertEqual([c for c in calls if "--add" in c], ["firewall-cmd --add-port=53318-53319/tcp", "firewall-cmd --add-port=67/udp"])
        self.assertFalse([c for c in calls if "--reload" in c or ("--permanent" in c and "--add" in c)], calls)
        # (5353/udp was there already, ours or not: left alone, now and at close)
        added = self.prefix / "run/irice/sharing-ports"
        self.assertEqual(added.read_text().split(), ["53318-53319/tcp", "67/udp"])
        _, calls = self.ports([], FAKE_UNITS="firewalld")
        self.assertEqual(self.changes(calls, "firewall-cmd"), ["firewall-cmd --remove-port=53318-53319/tcp", "firewall-cmd --remove-port=67/udp"])
        self.assertFalse(added.exists())

    def test_firewalld_keeps_the_zones_own_mdns(self):
        zones = self.prefix / "usr/lib/firewalld/zones"
        zones.mkdir(parents=True)
        (zones / "public.xml").write_text('<zone><service name="ssh"/><service name="mdns"/></zone>')
        _, calls = self.ports([], FAKE_UNITS="firewalld", FAKE_FWD_OLD="1")
        self.assertIn("firewall-cmd --permanent --remove-port=53317-53320/tcp --remove-port=53317/udp --remove-service=dhcp", calls)

    def test_nftables_rules_go_first_in_each_input_chain_and_nothing_is_kept(self):
        drop = self.prefix / "etc/nftables.d"
        drop.mkdir(parents=True)
        (drop / "irice.nft").write_text("insert rule inet filter input tcp dport 53317-53320 accept\n")
        done, calls = self.ports(["quickshare", "mdns"], FAKE_UNITS="nftables")
        self.assertEqual(self.changes(calls, "nft"), [
            "nft delete rule inet filter input handle 7",
            'nft insert rule inet filter input tcp dport 53318-53319 accept comment "irice sharing"',
            'nft insert rule inet filter input udp dport 67 accept comment "irice sharing"'])
        self.assertFalse((drop / "irice.nft").exists())  # (the fix before's, loaded at every boot)
        self.assertIn("removed /etc/nftables.d/irice.nft", done)
        self.assertFalse([c for c in calls if " -f " in c or "add table" in c], calls)
        _, calls = self.ports([], FAKE_UNITS="nftables")
        self.assertEqual(self.changes(calls, "nft"), ["nft delete rule inet filter input handle 9", "nft delete rule inet filter input handle 7"])

    def test_iptables_runtime_rules_tagged_ours(self):
        _, calls = self.ports(["localsend"], FAKE_UNITS="iptables")
        self.assertEqual(self.changes(calls, "iptables", "ip6tables"), [
            "iptables -D INPUT 3", "iptables -D INPUT 1",
            "iptables -I INPUT -p tcp --dport 53317 -m comment --comment irice sharing -j ACCEPT",
            "iptables -I INPUT -p udp --dport 53317 -m comment --comment irice sharing -j ACCEPT"])  # (ip6tables.service isn't on)
        self.assertFalse([c for c in calls if "save" in c], calls)

    def test_no_firewall_changes_nothing(self):
        done, calls = self.ports(["quickshare"])
        self.assertEqual(done, ["no firewall is turned on: nothing to open or close"])
        self.assertEqual(self.changes(calls, "ufw", "firewall-cmd", "nft", "iptables"), [])

    def test_only_the_fixed_sets(self):
        for bad in (["ssh"], ["53317"], ["quickshare;reboot"], [""], ["QUICKSHARE"]):
            with self.assertRaises(R.Refused):
                R.sharing_ports(bad, run=lambda *a, **k: None, prefix=str(self.prefix))


class Detection(Fakes):
    def sharing(self, *args, code=0, **env):
        p = subprocess.run([sys.executable, "-I", str(SCRIPTS / "sharing-check.py"), *args], env=dict(self.env, **env),
                           capture_output=True, text=True, timeout=20)
        self.assertEqual(p.returncode, code, p.stdout + p.stderr)
        return json.loads(p.stdout)

    def status(self):
        return self.sharing("status")["firewall"]

    def tool(self, name):
        (self.sysroot / "usr/bin").mkdir(parents=True, exist_ok=True)
        (self.sysroot / "usr/bin" / name).write_text("")

    def helper(self):
        h = self.sysroot / "usr/lib/irice/irice-root"
        h.parent.mkdir(parents=True, exist_ok=True)
        h.write_text("#!/bin/sh\n")
        h.chmod(0o755)
        return h

    def test_an_active_nftables_or_iptables_service_may_block(self):
        self.assertEqual(self.status()["kind"], None)
        self.env["FAKE_UNITS"] = "nftables"
        self.assertEqual(self.status()["kind"], None)  # (no nft installed)
        self.tool("nft")
        f = self.status()
        self.assertEqual((f["kind"], f["allowed"], f["blocks"], f["open"]), ("nftables", None, True, []))
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

    def pkexecs(self):
        return [c for c in self.calls() if c.startswith("pkexec ")]

    def test_open_while_visible_for_this_boot(self):
        self.tool("nft")
        self.env["FAKE_UNITS"] = "nftables"
        helper = self.helper()
        r = self.sharing("ports", "quickshare,mdns", "visible to Everyone")
        self.assertEqual((r["ok"], r["kind"]), (True, "nftables"))
        self.assertEqual(self.pkexecs(), [f"pkexec {helper} sharing-ports open mdns quickshare"])
        f = self.status()
        self.assertEqual((f["open"], f["blocks"], f["hotspot"]), (["mdns", "quickshare"], False, True))
        self.assertEqual(self.health_row()["detail"], "nftables: open now (visible to Everyone)")
        # Asked again unchanged: nothing asked
        self.sharing("ports", "mdns,quickshare", "visible to Everyone")
        self.assertEqual(len(self.pkexecs()), 1)
        # After a restart nftables starts closed (its rules were runtime only)
        note = self.home / ".local/state/irice/sharing-ports.json"
        n = json.loads(note.read_text())
        note.write_text(json.dumps(dict(n, boot="another-boot")))
        f = self.status()
        self.assertEqual((f["open"], f["blocks"]), ([], True))
        row = self.health_row()
        self.assertEqual((row["status"], row["detail"]), ("ok", "nftables: opened while you're visible to others; closed now"))
        # ufw keeps them through a restart
        note.write_text(json.dumps(dict(n, kind="ufw", boot="another-boot")))
        (self.sysroot / "etc/ufw").mkdir(parents=True)
        (self.sysroot / "etc/ufw/ufw.conf").write_text("ENABLED=yes\n")
        self.env["FAKE_UNITS"] = "ufw"
        self.assertEqual(self.status()["open"], ["mdns", "quickshare"])
        # Visible no more: closed
        self.sharing("ports", "none")
        self.assertEqual(self.pkexecs()[-1], f"pkexec {helper} sharing-ports close")
        self.assertEqual(self.status()["open"], [])

    def test_no_firewall_asks_nothing(self):
        self.helper()
        r = self.sharing("ports", "quickshare,mdns,localsend,links", "visible to Everyone")
        self.assertEqual((r["ok"], r["kind"]), (True, None))
        self.assertEqual(self.pkexecs(), [])
        self.assertEqual(self.health_row()["detail"], "No firewall turned on")

    def test_an_older_or_missing_helper_says_run_software_update(self):
        self.tool("nft")
        self.env["FAKE_UNITS"] = "nftables"
        r = self.sharing("ports", "quickshare", "visible to Everyone", code=64)  # (no helper installed: nothing asked)
        self.assertIn("Run Software Update once", r["error"])
        self.assertEqual(self.pkexecs(), [])
        self.helper()
        r = self.sharing("ports", "quickshare", "visible to Everyone", code=64, FAKE_PKEXEC_EXIT="64")
        self.assertIn("Run Software Update once", r["error"])
        row = self.health_row()
        self.assertEqual((row["status"], row["canFix"], row["fix"]), ("warn", False, "Run Software Update once"))
        self.assertIn("didn't open", row["detail"])

    def test_cancelled_then_system_healths_fix_asks_again(self):
        self.tool("nft")
        self.env["FAKE_UNITS"] = "nftables"
        helper = self.helper()
        r = self.sharing("ports", "localsend", "visible to Everyone", code=126, FAKE_PKEXEC_EXIT="126")
        self.assertEqual(r["error"], "Cancelled: no password was given.")
        self.assertEqual(self.status()["open"], [])
        row = self.health_row()
        self.assertEqual((row["status"], row["canFix"]), ("warn", True))
        p = subprocess.run([sys.executable, "-I", str(SCRIPTS / "health-check.py"), "fix", "firewall"], env=self.env,
                           capture_output=True, text=True, timeout=20)
        self.assertTrue(json.loads(p.stdout)["ok"], p.stdout + p.stderr)
        self.assertEqual(self.pkexecs()[-1], f"pkexec {helper} sharing-ports open localsend")
        self.assertEqual(self.status()["open"], ["localsend"])
        self.assertEqual(self.health_row()["status"], "ok")

    def test_only_the_fixed_sets_are_passed_on(self):
        self.helper()
        for bad in ("ssh", "quickshare,ssh", "", "quickshare;reboot"):
            self.sharing("ports", bad, code=2)
        self.assertEqual(self.pkexecs(), [])


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


class TransferFirewall(unittest.TestCase):
    """The Wi-Fi helper opens the firewall only for a transfer: on the link's
    own interface, only DHCP and the transfer port, removed with the link
    (the user's rule: never the permanent sharing rules mid-transfer)."""

    def links(self, kind):
        h = load("qs_wifi_helper_fw", SCRIPTS / "quickshare/qs-wifi-helper.py")
        calls = []
        h.have = lambda name: name == kind or (kind == "nft" and name == "nft")
        h.run = lambda *cmd, check=True: (calls.append(list(cmd)), "# handle 7")[1]

        class Done:
            def __init__(self, code):
                self.returncode = code
        real = h.subprocess.run
        h.subprocess.run = lambda cmd, **kw: Done(0 if (kind == "nft" and cmd[-1] == "nftables") else 1)
        h.nft_input_chains = lambda text: [("inet", "filter", "input")]
        try:
            links = h.Links()
            links._open_firewall("p2p-wlan0-3")
            opened = list(calls)
            calls.clear()
            for undo in reversed(links.undo):
                undo()
            return opened, calls
        finally:
            h.subprocess.run = real

    def test_iptables_only_the_two_ports_on_the_link(self):
        opened, closed = self.links("iptables")
        self.assertEqual(len(opened), 2)
        for c in opened:
            self.assertIn("p2p-wlan0-3", c)
            self.assertIn("--dport", c)
        self.assertEqual(sorted(c[c.index("--dport") + 1] for c in opened), ["53319", "67"])
        self.assertEqual(len(closed), 2)
        self.assertTrue(all("-D" in c for c in closed))

    def test_nftables_only_the_two_ports_on_the_link(self):
        opened, closed = self.links("nft")
        inserts = [c for c in opened if "insert" in c]
        self.assertEqual(len(inserts), 2)
        for c in inserts:
            self.assertIn("p2p-wlan0-3", c)
            self.assertIn("dport", c)
        self.assertTrue(all("delete" in c for c in closed) and len(closed) == 2)
