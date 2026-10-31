"""dictate.py: whisper-cli's output to plain words, model downloads (checked
before they're kept), and a transcription run (with stand-ins for the network
and for whisper-cli)."""
import importlib.util
import io
import json
import os
import subprocess
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

SCRIPT = Path(__file__).resolve().parent.parent / "shell/assets/scripts/dictate.py"
spec = importlib.util.spec_from_file_location("dictate", SCRIPT)
dp = importlib.util.module_from_spec(spec)
spec.loader.exec_module(dp)


class Clean(unittest.TestCase):
    def test_segments_joined(self):
        self.assertEqual(dp.clean(" Hello there.\n How are you?\n"), "Hello there. How are you?")

    def test_noise_dropped(self):
        self.assertEqual(dp.clean("[BLANK_AUDIO]\n"), "")
        self.assertEqual(dp.clean(" (music) Okay, *laughs* send it. [ Silence ]"), "Okay, send it.")

    def test_spaces(self):
        self.assertEqual(dp.clean("a   b\n\n  c"), "a b c")


class Response(io.BytesIO):
    def __init__(self, data, length=None):
        super().__init__(data)
        self.headers = {"Content-Length": str(len(data) if length is None else length)}

    def __enter__(self):
        return self

    def __exit__(self, *a):
        return False


class Data(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.env = mock.patch.dict(os.environ, {"XDG_DATA_HOME": self.tmp.name})
        self.env.start()

    def tearDown(self):
        self.env.stop()
        self.tmp.cleanup()

    def run_download(self, data, length=None):
        out = io.StringIO()
        with redirect_stdout(out):
            code = dp.download("base", opener=lambda req, timeout: Response(data, length))
        return code, [json.loads(x) for x in out.getvalue().splitlines()]


class Download(Data):
    def test_kept_when_whole(self):
        code, lines = self.run_download(dp.MAGIC + b"x" * 5000)
        self.assertEqual(code, 0)
        self.assertEqual(lines[-1], {"ok": True, "error": ""})
        self.assertTrue(dp.status()["models"]["base"])
        self.assertFalse(dp.status()["models"]["tiny"])

    def test_not_a_model(self):
        code, lines = self.run_download(b"<html>not found</html>")
        self.assertEqual(code, 1)
        self.assertFalse(lines[-1]["ok"])
        self.assertFalse(os.path.exists(dp.model_path("base")))
        self.assertFalse(os.path.exists(dp.model_path("base") + ".part"))

    def test_cut_short(self):
        code, lines = self.run_download(dp.MAGIC + b"x" * 10, length=99999)
        self.assertEqual(code, 1)
        self.assertIn("didn't finish", lines[-1]["error"])
        self.assertFalse(os.path.exists(dp.model_path("base")))

    def test_network_error(self):
        def fail(req, timeout):
            raise OSError("no route")
        out = io.StringIO()
        with redirect_stdout(out):
            self.assertEqual(dp.download("base", opener=fail), 1)
        self.assertIn("internet", json.loads(out.getvalue().splitlines()[-1])["error"])

    def test_unknown_model(self):
        with redirect_stdout(io.StringIO()):
            self.assertEqual(dp.download("../../etc/passwd"), 1)

    def test_remove(self):
        self.run_download(dp.MAGIC + b"x")
        self.assertTrue(dp.remove("base")["ok"])
        self.assertFalse(os.path.exists(dp.model_path("base")))
        self.assertTrue(dp.remove("base")["ok"])  # (already gone)


class Transcribe(Data):
    def setUp(self):
        super().setUp()
        self.wav = os.path.join(self.tmp.name, "a.wav")
        Path(self.wav).write_bytes(b"RIFF")

    def model(self):
        os.makedirs(dp.model_dir(), exist_ok=True)
        Path(dp.model_path("base")).write_bytes(dp.MAGIC)

    def test_no_binary(self):
        with mock.patch.object(dp, "binary", return_value=None):
            r = dp.transcribe(self.wav, "base", "en")
        self.assertFalse(r["ok"])
        self.assertIn("whisper-cpp", r["error"])
        self.assertFalse(os.path.exists(self.wav))  # (never kept)

    def test_no_model(self):
        with mock.patch.object(dp, "binary", return_value="/usr/bin/whisper-cli"):
            r = dp.transcribe(self.wav, "base", "en")
        self.assertIn("model", r["error"])

    def test_words(self):
        self.model()
        seen = {}

        def run(cmd, **kw):
            seen["cmd"] = cmd
            return subprocess.CompletedProcess(cmd, 0, " Send the report today.\n", "")
        with mock.patch.object(dp, "binary", return_value="/usr/bin/whisper-cli"):
            r = dp.transcribe(self.wav, "base", "auto", run=run)
        self.assertEqual(r, {"ok": True, "text": "Send the report today.", "error": ""})
        self.assertEqual(seen["cmd"][:5], ["/usr/bin/whisper-cli", "-m", dp.model_path("base"), "-f", self.wav])
        self.assertIn("auto", seen["cmd"])
        self.assertFalse(os.path.exists(self.wav))

    def test_failure(self):
        self.model()
        run = lambda cmd, **kw: subprocess.CompletedProcess(cmd, 1, "", "loading\nfailed to read WAV file\n")
        with mock.patch.object(dp, "binary", return_value="/usr/bin/whisper-cli"):
            r = dp.transcribe(self.wav, "base", "en", run=run)
        self.assertEqual(r["error"], "failed to read WAV file")


if __name__ == "__main__":
    unittest.main()
