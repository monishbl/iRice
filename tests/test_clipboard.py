"""Clipboard image previews (clipboard-previews.py): decoded once, never
published half-written."""
import contextlib
import importlib.util
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1] / 'shell/assets/scripts'


def load(name):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / (name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


clipboard = load('clipboard-previews')


class ClipboardTests(unittest.TestCase):
    def test_cached_images_do_not_decode_again(self):
        with tempfile.TemporaryDirectory() as tmp:
            image = Path(tmp) / '1.png'
            image.write_bytes(b'image')
            with patch.object(clipboard.subprocess, 'run') as run, contextlib.redirect_stdout(io.StringIO()) as output:
                clipboard.previews('db', tmp, [('1', str(image))])
                clipboard.previews('db', tmp, [('1', str(image))])
                run.assert_not_called()
            self.assertEqual(output.getvalue().splitlines(), [str(image), str(image)])

    def test_failed_decode_does_not_publish_partial_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            image = Path(tmp) / '2.png'
            def decode(*args, **kwargs):
                kwargs['stdout'].write(b'partial')
                return type('Result', (), {'returncode': 1})()
            with patch.object(clipboard.subprocess, 'run', side_effect=decode), contextlib.redirect_stdout(io.StringIO()) as output:
                clipboard.previews('db', tmp, [('2', str(image))])
            self.assertFalse(image.exists())
            self.assertEqual(output.getvalue(), '')
            self.assertEqual(list(Path(tmp).iterdir()), [])

    def test_decodes_beyond_old_48_image_limit(self):
        with tempfile.TemporaryDirectory() as tmp:
            def decode(*args, **kwargs):
                kwargs['stdout'].write(b'image')
                return type('Result', (), {'returncode': 0})()
            pairs = [(str(i), str(Path(tmp) / (str(i) + '.png'))) for i in range(60)]
            with patch.object(clipboard.subprocess, 'run', side_effect=decode), contextlib.redirect_stdout(io.StringIO()) as output:
                clipboard.previews('db', tmp, pairs)
            self.assertEqual(len(output.getvalue().splitlines()), 60)


if __name__ == '__main__':
    unittest.main()
