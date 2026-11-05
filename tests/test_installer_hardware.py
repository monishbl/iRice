"""The installer's hardware check (tools/installer/hardware.py): what it reads
from lspci, lsusb, /proc/cpuinfo, EDID and /sys fixtures, and the plan it
makes: only what's missing, NVIDIA and biometrics only when asked. Then
install.sh --plan with those choices, against fake pacman, systemctl and sudo
on PATH: it shows the plan and changes nothing. Nothing real is read for the
plan or installed."""
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
HW = ROOT / "tools/installer/hardware.py"


def load(system_root):
    """hardware.py with /sys, /proc and pacman's database under system_root."""
    old = os.environ.get("TESSERA_SYSTEM_ROOT")
    os.environ["TESSERA_SYSTEM_ROOT"] = str(system_root)
    try:
        spec = importlib.util.spec_from_file_location("tessera_hardware_test", HW)
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        return mod
    finally:
        if old is None:
            os.environ.pop("TESSERA_SYSTEM_ROOT", None)
        else:
            os.environ["TESSERA_SYSTEM_ROOT"] = old


LSPCI = textwrap.dedent("""\
    00:02.0 VGA compatible controller [0300]: Intel Corporation Alder Lake-UP3 GT2 [Iris Xe Graphics] [8086:46a8] (rev 0c)
    \tSubsystem: Dell Device [1028:0b19]
    \tKernel driver in use: i915
    \tKernel modules: i915, xe
    00:1f.3 Multimedia audio controller [0401]: Intel Corporation Alder Lake PCH-P High Definition Audio Controller [8086:51c8] (rev 01)
    \tSubsystem: Dell Device [1028:0b19]
    \tKernel driver in use: sof-audio-pci-intel-tgl
    \tKernel modules: snd_hda_intel, snd_sof_pci_intel_tgl
    01:00.0 3D controller [0302]: NVIDIA Corporation GA107M [GeForce RTX 3050 Mobile] [10de:25a2] (rev a1)
    \tKernel driver in use: nouveau
    \tKernel modules: nouveau
    02:00.0 Network controller [0280]: Broadcom Inc. and subsidiaries BCM4360 802.11ac Dual Band Wireless Network Adapter [14e4:43a0] (rev 03)
    \tSubsystem: Apple Inc. Device [106b:0134]
    \tKernel modules: bcma
""")
LSUSB = textwrap.dedent("""\
    Bus 003 Device 004: ID 0a5c:5843 Broadcom Corp. 58200
    Bus 003 Device 002: ID 0c45:6739 Microdia Integrated_Webcam_HD
""")
CPUINFO = "processor\t: 0\nvendor_id\t: GenuineIntel\ncpu family\t: 6\n"


def edid(w_cm, h_cm):
    return b"\x00\xff\xff\xff\xff\xff\xff\x00" + bytes(13) + bytes([w_cm, h_cm]) + bytes(105)


class Parsing(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.hw = load(self.tmp.name)

    def tearDown(self):
        self.tmp.cleanup()

    def test_lspci(self):
        devs = self.hw.parse_lspci(LSPCI)
        self.assertEqual([(d["slot"], d["cls"], d["vendor"], d["device"]) for d in devs],
                         [("00:02.0", "0300", "8086", "46a8"), ("00:1f.3", "0401", "8086", "51c8"),
                          ("01:00.0", "0302", "10de", "25a2"), ("02:00.0", "0280", "14e4", "43a0")])
        self.assertEqual(devs[1]["driver"], "sof-audio-pci-intel-tgl")
        self.assertEqual(devs[1]["modules"], ["snd_hda_intel", "snd_sof_pci_intel_tgl"])
        self.assertEqual(devs[3]["driver"], "")
        self.assertIn("GeForce RTX 3050", devs[2]["name"])

    def test_cpu_usb_and_the_fingerprint_reader(self):
        self.assertEqual(self.hw.parse_cpu(CPUINFO), "intel")
        self.assertEqual(self.hw.parse_cpu("vendor_id : AuthenticAMD\n"), "amd")
        self.assertEqual(self.hw.parse_cpu(""), "")
        ids = self.hw.parse_lsusb(LSUSB)
        self.assertEqual(ids, {"0a5c:5843", "0c45:6739"})
        # (the shell's own fingerprint.py knows the readers: Dell's Broadcom one needs the AUR's driver)
        fp = self.hw.load("fingerprint")
        reader = fp.find_reader(ids, known=set())
        self.assertEqual(reader["name"], "Broadcom ControlVault 3")
        repo, aur = fp.needs(reader)
        self.assertIn("libfprint-2-tod1-broadcom", aur)

    def test_edid_and_scale(self):
        self.assertEqual(self.hw.edid_size(edid(31, 17)), (310, 170))
        self.assertIsNone(self.hw.edid_size(edid(0, 0)))  # (a projector)
        self.assertIsNone(self.hw.edid_size(b"junk"))
        # a 14" 1080p laptop screen, a 13" 3K one, a 27" 1440p monitor, a 27" 4K one
        self.assertEqual(self.hw.suggest_scale(1920, 310, True), 1.25)
        self.assertEqual(self.hw.suggest_scale(2880, 290, True), 2.0)
        self.assertEqual(self.hw.suggest_scale(2560, 600, False), 1.0)
        self.assertEqual(self.hw.suggest_scale(3840, 600, False), 1.25)
        self.assertEqual(self.hw.suggest_scale(1920, None, True), 1.0)

    def test_kernel_headers(self):
        self.assertEqual(self.hw.kernel_headers({"linux", "linux-zen", "linux-firmware", "linux-headers", "linux-api-headers"}),
                         ["linux-headers", "linux-zen-headers"])


def facts(**over):
    f = {"cpu": "intel", "pci": [], "laptop": True, "installed": ["linux", "linux-firmware"], "power_manager": "",
         "fingerprint": None, "ir_camera": None, "screens": [], "monitors_set": False, "hyprland": "0.56.2",
         "pulseaudio": False, "network": "", "display_manager": ""}
    f.update(over)
    return f


class Plan(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.hw = load(self.tmp.name)
        self.pci = self.hw.parse_lspci(LSPCI)

    def tearDown(self):
        self.tmp.cleanup()

    def items(self, **over):
        return {i["id"]: i for i in self.hw.plan(facts(**over))}

    def test_only_whats_missing(self):
        got = self.items(pci=self.pci)
        self.assertEqual(got["sof"]["packages"], ["sof-firmware", "alsa-ucm-conf"])
        self.assertEqual(got["gpu-intel"]["packages"], ["intel-media-driver", "vulkan-intel"])
        self.assertEqual(got["broadcom-wl"]["packages"], ["broadcom-wl-dkms", "linux-headers"])
        self.assertNotIn("firmware", got)  # (linux-firmware is installed)
        have = ["linux", "linux-firmware", "sof-firmware", "alsa-ucm-conf", "intel-media-driver", "vulkan-intel", "power-profiles-daemon"]
        got = self.items(pci=self.pci, installed=have)
        for gone in ("sof", "gpu-intel", "power"):
            self.assertNotIn(gone, got)
        self.assertIn("firmware", self.items(installed=["linux"]))

    def test_nvidia_only_when_asked(self):
        nv = self.items(pci=self.pci)["gpu-nvidia"]
        self.assertTrue(nv["ask"])
        self.assertFalse(nv["ticked"])
        self.assertIn("nvidia-open-dkms", nv["packages"])
        self.assertIn("linux-headers", nv["packages"])
        # On NVIDIA's driver already: just its video decoding
        pci = [dict(d, driver="nvidia") if d["vendor"] == "10de" else d for d in self.pci]
        got = self.items(pci=pci)
        self.assertNotIn("gpu-nvidia", got)
        self.assertEqual(got["gpu-nvidia-video"]["packages"], ["libva-nvidia-driver"])
        self.assertFalse(got["gpu-nvidia-video"]["ask"])

    def test_biometrics_each_asked(self):
        got = self.items(fingerprint={"id": "0a5c:5843", "name": "Broadcom ControlVault 3", "repo": [],
                                      "aur": ["libfprint-tod", "libfprint-2-tod1-broadcom", "fprintd"]},
                         ir_camera={"path": "/dev/v4l/by-path/x", "name": "Integrated IR Webcam"})
        self.assertTrue(got["fingerprint"]["ask"])
        self.assertEqual(got["fingerprint"]["aur"], ["libfprint-tod", "libfprint-2-tod1-broadcom", "fprintd"])
        self.assertTrue(got["face"]["ask"])
        self.assertFalse(got["face"]["ticked"])
        self.assertEqual(got["face"]["aur"], ["howdy-next"])
        # An unsupported reader, or Howdy already there: nothing
        got = self.items(fingerprint={"id": "27c6:55a4", "name": "Goodix fingerprint reader", "unsupported": True, "repo": [], "aur": []},
                         ir_camera={"path": "/dev/video2", "name": "IR"}, installed=["linux-firmware", "howdy-next"])
        self.assertNotIn("fingerprint", got)
        self.assertNotIn("face", got)

    def test_build_items_are_always_done(self):
        got = self.items()
        for i in ("build-plugin", "build-native", "build-shaders"):
            self.assertTrue(got[i]["locked"])
        self.assertIn("0.56.2", got["build-plugin"]["title"])

    def test_tuning(self):
        self.assertIn("power", self.items())
        self.assertNotIn("power", self.items(power_manager="tlp"))
        self.assertNotIn("power", self.items(laptop=False))
        self.assertFalse(self.items()["thermald"]["ticked"])
        self.assertNotIn("thermald", self.items(cpu="amd"))
        screen = {"name": "eDP-1", "width": 2880, "height": 1800, "mm": (290, 180), "internal": True, "inches": 13.4, "scale": 2.0}
        got = self.items(screens=[screen])
        self.assertEqual(got["scale-eDP-1"]["scale"], 2.0)
        self.assertNotIn("scale-eDP-1", self.items(screens=[screen], monitors_set=True))  # (Settings → Displays has its own)
        self.assertNotIn("scale-eDP-1", self.items(screens=[dict(screen, scale=1.0)]))

    def test_apply_only_the_chosen(self):
        screen = {"name": "eDP-1", "width": 2880, "height": 1800, "mm": (290, 180), "internal": True, "inches": 13.4, "scale": 2.0}
        items = self.hw.plan(facts(pci=self.pci, screens=[screen]))
        # (the always-done ones are done by their own steps: builds, packages)
        self.assertEqual(self.hw.apply_lines(items, {"sof", "power", "build-plugin", "scale-eDP-1", "thermald"}),
                         ["pkg sof-firmware", "pkg alsa-ucm-conf", "pkg thermald", "svc thermald", "scale eDP-1 2"])
        self.assertEqual(self.hw.apply_lines(items, {"nope"}), [])


class Collect(unittest.TestCase):
    """This computer's facts, from a fake /sys and pacman database."""

    def test_sysfs(self):
        with tempfile.TemporaryDirectory() as d:
            r = Path(d)

            def put(p, text):
                f = r / p.lstrip("/")
                f.parent.mkdir(parents=True, exist_ok=True)
                f.write_bytes(text if isinstance(text, bytes) else text.encode())
            put("/proc/cpuinfo", CPUINFO)
            for slot, cls, ven, dev in (("0000:00:02.0", "0x030000", "0x8086", "0x46a8"), ("0000:00:1f.3", "0x040380", "0x8086", "0x51c8")):
                put(f"/sys/bus/pci/devices/{slot}/class", cls + "\n")
                put(f"/sys/bus/pci/devices/{slot}/vendor", ven + "\n")
                put(f"/sys/bus/pci/devices/{slot}/device", dev + "\n")
            drv = r / "sys/bus/pci/drivers/sof-audio-pci-intel-tgl"
            drv.mkdir(parents=True)
            os.symlink(drv, r / "sys/bus/pci/devices/0000:00:1f.3/driver")
            put("/sys/class/power_supply/BAT0/type", "Battery\n")
            put("/sys/class/power_supply/hidpp_battery_0/type", "Battery\n")
            put("/sys/class/power_supply/hidpp_battery_0/scope", "Device\n")
            put("/sys/class/drm/card1-eDP-1/status", "connected\n")
            put("/sys/class/drm/card1-eDP-1/edid", edid(29, 18))
            put("/sys/class/drm/card1-eDP-1/modes", "2880x1800\n1920x1200\n")
            put("/sys/class/drm/card1-HDMI-A-1/status", "disconnected\n")
            for pkg in ("linux-6.17.1.arch1-1", "linux-firmware-20261001-1", "hyprland-0.56.2-1"):
                (r / "var/lib/pacman/local" / pkg).mkdir(parents=True)
            hw = load(d)
            f = hw.collect(run=lambda *a, **k: (_ for _ in ()).throw(OSError("no commands in tests")))
        self.assertEqual(f["cpu"], "intel")
        self.assertTrue(f["laptop"])
        self.assertEqual(sorted(f["installed"]), ["hyprland", "linux", "linux-firmware"])
        self.assertEqual(f["hyprland"], "0.56.2")
        self.assertEqual([(d["cls"], d["vendor"], d["driver"]) for d in f["pci"]],
                         [("0300", "8086", ""), ("0403", "8086", "sof-audio-pci-intel-tgl")])
        self.assertEqual(f["screens"][0]["name"], "eDP-1")
        self.assertEqual(f["screens"][0]["scale"], 2.0)
        ids = {i["id"] for i in hw.plan(f)}
        self.assertTrue({"sof", "gpu-intel", "power", "scale-eDP-1"} <= ids)

    def test_a_desktop_with_a_wireless_mouse(self):
        with tempfile.TemporaryDirectory() as d:
            p = Path(d) / "sys/class/power_supply/hidpp_battery_0"
            p.mkdir(parents=True)
            (p / "type").write_text("Battery\n")
            (p / "scope").write_text("Device\n")
            f = load(d).collect(run=lambda *a, **k: (_ for _ in ()).throw(OSError()))
        self.assertFalse(f["laptop"])


class CommandLine(unittest.TestCase):
    def test_plan_and_apply(self):
        with tempfile.TemporaryDirectory() as d:
            ff = Path(d) / "facts.json"
            ff.write_text(json.dumps(facts(installed=["linux"])))
            run = lambda *a: subprocess.run([sys.executable, "-I", str(HW), "--facts", str(ff), *a],
                                            capture_output=True, text=True, timeout=30)
            rows = [line.split("\t") for line in run("plan").stdout.splitlines()]
            self.assertEqual(rows[0][:4], ["firmware", "1", "0", "0"])
            self.assertEqual(rows[0][7], "linux-firmware")
            self.assertTrue(all(len(r) == 9 for r in rows))
            self.assertEqual(run("apply", "firmware", "build-plugin").stdout.split("\n")[0], "pkg linux-firmware")
            self.assertIn("items", json.loads(run("plan", "--json").stdout))


class InstallPlan(unittest.TestCase):
    """install.sh --plan: the plan for the choices, nothing done."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        d = Path(self.tmp.name)
        self.bin, self.home, self.marks = d / "bin", d / "home", d / "marks"
        for p in (self.bin, self.home, self.marks):
            p.mkdir()
        self.root = d / "root"
        (self.root / "var/lib/pacman/local").mkdir(parents=True)

    def tearDown(self):
        self.tmp.cleanup()

    def fake(self, name, script):
        f = self.bin / name
        f.write_text("#!/bin/sh\n" + textwrap.dedent(script))
        f.chmod(0o755)

    def plan(self, *args, installed=""):
        # Anything that would change the system leaves a mark instead
        for cmd in ("sudo", "yay", "paru", "makepkg", "snapper", "timeshift"):
            self.fake(cmd, f'echo "$0 $*" >> {self.marks}/changed\nexit 1\n')
        self.fake("pacman", f'''
            case "$1" in
                -Qq) shift; [ "$1" = -- ] && shift; ok=1; for p; do case " {installed} " in *" $p "*) echo "$p" ;; *) ok=0 ;; esac; done; [ $ok = 1 ] ;;
                -T) shift; [ "$1" = -- ] && shift; for p; do case " {installed} " in *" $p "*) ;; *) echo "$p" ;; esac; done ;;
                -Si) exit 0 ;;
                *) echo "pacman $*" >> {self.marks}/changed; exit 1 ;;
            esac
        ''')
        self.fake("systemctl", 'case "$1" in is-active|is-enabled|list-unit-files) exit 3 ;; esac\necho "systemctl $*" >> ' + str(self.marks) + '/changed\nexit 1\n')
        self.fake("systemd-detect-virt", "echo none\nexit 1\n")
        self.fake("lspci", "exit 0\n")
        env = dict(os.environ, PATH=f"{self.bin}:/usr/bin:/bin", HOME=str(self.home), XDG_CONFIG_HOME=str(self.home / ".config"),
                   XDG_DATA_HOME=str(self.home / ".local/share"), XDG_CACHE_HOME=str(self.home / ".cache"),
                   TESSERA_SYSTEM_ROOT=str(self.root), COLUMNS="300")
        p = subprocess.run([str(ROOT / "install.sh"), "--plan", *args], env=env, capture_output=True, text=True,
                           timeout=60, stdin=subprocess.DEVNULL)
        self.assertFalse((self.marks / "changed").exists(), (self.marks / "changed").read_text() if (self.marks / "changed").exists() else "")
        self.assertEqual(list(self.home.iterdir()), [], "--plan wrote into your home")
        return p

    def test_the_whole_plan_and_nothing_else(self):
        p = self.plan("--icons=papirus,colloid", "--hardware=firmware,nope", "--look=material", "--mode=dark",
                      "--login=text", "--extras=none", installed="python")
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        out = p.stdout
        self.assertIn("The plan", out)
        self.assertIn("papirus-icon-theme colloid-icon-theme-git", out)
        self.assertIn("Linux firmware: linux-firmware", out)
        self.assertIn("nope: nothing to do on this computer", out)
        self.assertIn("look material, dark appearance, login style text", out)
        self.assertIn("the Tessera login screen (greetd)", out)
        self.assertIn("hyprland", out)  # (the packages it would install)
        self.assertNotIn("Start?", out)

    def test_nothing_of_yours_removed_unless_chosen(self):
        p = self.plan("--yes", installed="pulseaudio")
        self.assertIn("keeps PulseAudio", p.stdout)
        self.assertIn("none: the apps' own icons", p.stdout)
        p = self.plan("--replace-pulseaudio", installed="pulseaudio")
        self.assertIn("removes pulseaudio", p.stdout)

    def test_rust_programs_ready_made_unless_asked(self):
        # (no question about it: ready-made, with the advanced flags named)
        p = self.plan("--yes")
        self.assertIn("tessera-ui and tessera-events, ready-made and signed", p.stdout)
        self.assertIn("--ui-source=build or buildTuned", p.stdout)
        p = self.plan("--yes", "--ui-source=buildTuned")
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)
        self.assertIn("built here, tuned for this processor", p.stdout)
        self.assertIn("Rust programs buildTuned", p.stdout)  # (kept in Settings)

    def test_unknown_icon_theme(self):
        p = self.plan("--icons=macos")
        self.assertNotEqual(p.returncode, 0)
        self.assertIn("No icon theme called macos", p.stdout)


if __name__ == "__main__":
    unittest.main()
