"""face.py: the infrared camera found by name, by its stable path (a
stand-in /sys and /dev), and Howdy's state."""
import importlib.util
import os
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "shell/assets/scripts/face.py"
spec = importlib.util.spec_from_file_location("face", SCRIPT)
fc = importlib.util.module_from_spec(spec)
spec.loader.exec_module(fc)
YUYV, MJPG = 0x56595559, 0x47504A4D


class Camera(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.old = fc.ROOT
        fc.ROOT = str(self.root)

    def tearDown(self):
        fc.ROOT = self.old
        self.tmp.cleanup()

    def node(self, dev, name, iface, index="0"):
        d = self.root / "sys/class/video4linux" / dev
        (d / "device").mkdir(parents=True)
        (d / "name").write_text(name + "\n")
        (d / "index").write_text(index + "\n")
        (d / "device/interface").write_text(iface + "\n")

    def link(self, kind, name, dev):
        d = self.root / "dev/v4l" / kind
        d.mkdir(parents=True, exist_ok=True)
        os.symlink("../../" + dev, d / name)

    def test_ir_camera_by_path(self):
        self.node("video0", "Integrated_Webcam_FHD", "Integrated Webcam")
        self.node("video2", "Integrated_Webcam_FHD", "Integrated IR Webcam")
        self.node("video3", "Integrated_Webcam_FHD", "Integrated IR Webcam", index="1")  # (metadata node)
        self.link("by-path", "pci-0000:00:14.0-usb-0:6:1.2-video-index0", "video2")
        self.assertEqual(fc.ir_camera(), {"path": "/dev/v4l/by-path/pci-0000:00:14.0-usb-0:6:1.2-video-index0", "name": "Integrated IR Webcam"})

    def test_ir_camera_without_ir_in_its_name(self):
        # (many name it like the colour camera: it only gives greyscale)
        self.node("video0", "Integrated Camera", "Integrated Camera")
        self.node("video2", "Integrated Camera", "Integrated Camera")
        fmts = lambda dev: [fc.GREY] if dev.endswith("video2") else [0x47504A4D, 0x56595559]
        cam = fc.ir_camera(fmts)
        self.assertEqual(cam["path"], "/dev/video2")
        self.assertIn("infrared", cam["name"])

    def test_plain_webcam_is_not_offered(self):
        self.node("video0", "HD Webcam", "HD Webcam")
        self.assertIsNone(fc.ir_camera(lambda dev: [0x56595559]))

    def test_both_cameras_infrared_first(self):
        self.node("video0", "Integrated_Webcam_FHD", "Integrated Webcam")
        self.node("video2", "Integrated_Webcam_FHD", "Integrated IR Webcam")
        self.link("by-id", "usb-Generic_Integrated_Webcam-video-index0", "video0")
        cams = fc.cameras(lambda dev: [YUYV, MJPG])
        self.assertEqual([c["kind"] for c in cams], ["ir", "rgb"])
        self.assertEqual(cams[1], {"path": "/dev/v4l/by-id/usb-Generic_Integrated_Webcam-video-index0", "name": "Integrated Webcam", "kind": "rgb"})
        # (what the QML offers is still the infrared one only)
        self.assertEqual(fc.ir_camera(lambda dev: [YUYV])["path"], "/dev/video2")

    def test_a_plain_webcam_is_a_regular_camera(self):
        self.node("video0", "HD Webcam", "HD Webcam")
        self.assertEqual(fc.cameras(lambda dev: [YUYV]), [{"path": "/dev/video0", "name": "HD Webcam", "kind": "rgb"}])
        # (one that couldn't be opened counts as regular: the blink is asked)
        self.assertEqual(fc.cameras(lambda dev: [])[0]["kind"], "rgb")

    def test_virtual_cameras_are_not_offered(self):
        # v4l2loopback (the phone camera, OBS): any app could show it a face
        virt = self.root / "sys/devices/virtual/video4linux/video42"
        virt.mkdir(parents=True)
        (virt / "name").write_text("Phone camera\n")
        (virt / "index").write_text("0\n")
        (self.root / "sys/class/video4linux").mkdir(parents=True, exist_ok=True)
        os.symlink(virt, self.root / "sys/class/video4linux/video42")
        self.assertEqual(fc.cameras(lambda dev: [YUYV]), [])

    def test_camera_kind_from_formats(self):
        self.assertEqual(fc.camera_kind("Integrated Camera", [fc.GREY]), "ir")
        self.assertEqual(fc.camera_kind("Integrated Camera", [fc.fourcc("Y16 "), fc.GREY]), "ir")
        self.assertEqual(fc.camera_kind("Integrated Camera", [YUYV, fc.GREY]), "rgb")  # (colour too: regular)
        self.assertEqual(fc.camera_kind("Integrated Camera", []), "rgb")
        self.assertEqual(fc.camera_kind("Integrated Infrared Camera", []), "ir")
        self.assertEqual(fc.camera_kind("Chicony USB2.0 IR", [YUYV]), "ir")
        self.assertEqual(fc.camera_kind("BIRCH camera", [YUYV]), "rgb")  # (IR as a word only)

    def test_v4l2ctl_output(self):
        ir = """ioctl: VIDIOC_ENUM_FMT
\tType: Video Capture

\t[0]: 'GREY' (8-bit Greyscale)
"""
        colour = """ioctl: VIDIOC_ENUM_FMT
\tType: Video Capture

\t[0]: 'MJPG' (Motion-JPEG, compressed)
\t[1]: 'YUYV' (YUYV 4:2:2)
"""
        y8 = "\t[0]: 'Y8  ' (8-bit Greyscale)\n\t[1]: 'Y16 ' (16-bit Greyscale)\n"
        self.assertEqual(fc.v4l2ctl_formats(ir), [fc.GREY])
        self.assertEqual(fc.v4l2ctl_formats(colour), [MJPG, YUYV])
        self.assertEqual(fc.camera_kind("Integrated Camera", fc.v4l2ctl_formats(ir)), "ir")
        self.assertEqual(fc.camera_kind("Integrated Camera", fc.v4l2ctl_formats(colour)), "rgb")
        self.assertEqual(fc.camera_kind("Integrated Camera", fc.v4l2ctl_formats(y8)), "ir")
        self.assertEqual(fc.v4l2ctl_formats(""), [])

    def test_status_lists_cameras(self):
        self.node("video0", "HD Webcam", "HD Webcam")
        st = fc.status(lambda dev: [YUYV])
        self.assertIsNone(st["camera"])
        self.assertEqual(st["cameras"][0]["kind"], "rgb")
        self.assertFalse(st["pamExec"])
        (self.root / "usr/lib/security").mkdir(parents=True)
        (self.root / "usr/lib/security/pam_exec.so").write_text("")
        self.assertTrue(fc.status(lambda dev: [YUYV])["pamExec"])
        self.assertIn("enrolled", st["live"])

    def test_howdy_config(self):
        (self.root / "etc/howdy").mkdir(parents=True)
        (self.root / "etc/howdy/config.ini").write_text("[video]\ndevice_path = /dev/video2\n")
        self.assertEqual(fc.configured_device(), "/dev/video2")
        (self.root / "etc/howdy/config.ini").write_text("[video]\ndevice_path = none\n")
        self.assertEqual(fc.configured_device(), "")

    def test_howdy_config_root_only(self):
        # (howdy-next makes /etc/howdy 0750: a user can't read the camera
        # it's set to, which isn't the same as not set up)
        (self.root / "etc/howdy").mkdir(parents=True)
        (self.root / "etc/howdy/config.ini").write_text("[video]\ndevice_path = /dev/video2\n")
        self.assertFalse(fc.config_private())
        if os.geteuid() != 0:
            os.chmod(self.root / "etc/howdy", 0o700)
            os.chmod(self.root / "etc/howdy/config.ini", 0o000)
            try:
                self.assertTrue(fc.config_private())
                self.assertEqual(fc.configured_device(), "")
            finally:
                os.chmod(self.root / "etc/howdy/config.ini", 0o644)
        (self.root / "etc/howdy/config.ini").unlink()
        self.assertFalse(fc.config_private())  # (missing isn't private)


if __name__ == "__main__":
    unittest.main()
