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

    def test_plain_webcam_is_not_offered(self):
        self.node("video0", "HD Webcam", "HD Webcam")
        self.assertIsNone(fc.ir_camera())

    def test_howdy_config(self):
        (self.root / "etc/howdy").mkdir(parents=True)
        (self.root / "etc/howdy/config.ini").write_text("[video]\ndevice_path = /dev/video2\n")
        self.assertEqual(fc.configured_device(), "/dev/video2")
        (self.root / "etc/howdy/config.ini").write_text("[video]\ndevice_path = none\n")
        self.assertEqual(fc.configured_device(), "")


if __name__ == "__main__":
    unittest.main()
