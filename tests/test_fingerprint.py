"""fingerprint.py: finding the reader (vendor drivers first, then systemd's
fingerprint hwdb), what it still needs, and where a finger is accepted (the
marked PAM lines), all against a stand-in system root."""
import importlib.util
import os
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "shell/assets/scripts/fingerprint.py"
spec = importlib.util.spec_from_file_location("fingerprint", SCRIPT)
fp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fp)


class Root(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.old = fp.ROOT
        fp.ROOT = str(self.root)

    def tearDown(self):
        fp.ROOT = self.old
        self.tmp.cleanup()

    def write(self, rel, text=""):
        p = self.root / rel.lstrip("/")
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text)

    def usb(self, name, vid, pid):
        self.write(f"/sys/bus/usb/devices/{name}/idVendor", vid + "\n")
        self.write(f"/sys/bus/usb/devices/{name}/idProduct", pid + "\n")

    def package(self, name):
        (self.root / f"var/lib/pacman/local/{name}-1.0-1").mkdir(parents=True)


class Reader(Root):
    def test_none(self):
        self.usb("1-1", "8087", "0033")
        self.assertIsNone(fp.find_reader())

    def test_hwdb_reader(self):
        self.write(fp.HWDB, "# comment\nusb:v27C6p5110*\n ID_AUTOSUSPEND=1\nusb:v06CBp00BD*\n")
        self.usb("1-1", "27c6", "5110")
        self.assertEqual(fp.find_reader(), {"id": "27c6:5110", "name": "Fingerprint reader"})

    def test_vendor_driver_first(self):
        self.write(fp.HWDB, "usb:v27C6p5110*\n")
        self.usb("1-1", "27c6", "5110")
        self.usb("1-2", "0a5c", "5843")
        self.assertEqual(fp.find_reader()["name"], "Broadcom ControlVault 3")


class Needs(Root):
    cv3 = {"id": "0a5c:5843", "name": "Broadcom ControlVault 3"}

    def test_no_reader(self):
        self.assertEqual(fp.needs(None), ([], []))

    def test_plain_reader_needs_fprintd(self):
        self.assertEqual(fp.needs({"id": "27c6:5110", "name": ""}), (["fprintd"], []))
        self.package("fprintd")
        self.assertEqual(fp.needs({"id": "27c6:5110", "name": ""}), ([], []))

    def test_vendor_reader_all_from_the_aur_helper(self):
        # (fprintd goes with the AUR packages, so the repo's libfprint never comes)
        self.assertEqual(fp.needs(self.cv3), ([], ["libfprint-tod", "libfprint-2-tod1-broadcom", "fprintd"]))

    def test_vendor_reader_only_whats_missing(self):
        self.package("fprintd")
        self.package("libfprint-tod")
        self.assertEqual(fp.needs(self.cv3), ([], ["libfprint-2-tod1-broadcom"]))

    def test_prefix_is_not_installed(self):
        self.package("libfprint-tod-git")  # (another package, not libfprint-tod)
        self.assertFalse(fp.installed("libfprint-tod"))
        self.assertTrue(fp.installed("libfprint-tod-git"))


class Pam(Root):
    def test_marked_lines(self):
        self.write("/etc/pam.d/sudo", "auth sufficient pam_fprintd.so " + fp.MARK + "\nauth include system-auth\n")
        self.write("/etc/pam.d/polkit-1", "auth include system-auth\n")
        self.write("/etc/pam.d/ly", "auth sufficient pam_fprintd.so " + fp.MARK + "\n")
        self.assertEqual(fp.pam_state(), {"polkit": False, "sudo": True, "login": True})

    def test_missing_files(self):
        self.assertEqual(fp.pam_state(), {"polkit": False, "sudo": False, "login": False})


class Cli(unittest.TestCase):
    def test_unknown_finger(self):
        import io
        from contextlib import redirect_stdout
        out = io.StringIO()
        with redirect_stdout(out):
            self.assertEqual(fp.enroll("nose"), 1)
        self.assertIn("unknown finger", out.getvalue())

    def test_usage(self):
        import io
        from contextlib import redirect_stdout
        with redirect_stdout(io.StringIO()):
            self.assertEqual(fp.main(["frobnicate"]), 2)

    def test_dbus_error_names(self):
        class E(Exception):
            message = "GDBus.Error:net.reactivated.Fprint.Error.NoEnrolledPrints: none"
        self.assertEqual(fp.dbus_error(E()), "NoEnrolledPrints")
        self.assertEqual(fp.dbus_error(ValueError("plain")), "plain")


if __name__ == "__main__":
    unittest.main()
