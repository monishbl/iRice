"""Sharing from file managers and the selection.

file-manager-share.py adds "Send to a nearby device…" to file managers'
menus and must leave everything else in them alone (Thunar keeps all custom
actions in one file). share-files.sh hands the chosen files to the shell
over IPC; names with spaces, quotes, % or even a newline must arrive intact.
Both run against a throwaway home, with a fake `qs` that records its call.
"""
import json
import os
import subprocess
import tempfile
import unittest
import urllib.parse
import xml.etree.ElementTree as ET
from pathlib import Path

SCRIPTS = Path(__file__).resolve().parent.parent / "shell/assets/scripts"


class Home:
    """A throwaway home with fake file managers and a fake qs on PATH."""

    def __init__(self, managers=("nautilus", "dolphin", "thunar", "nemo")):
        self.dir = tempfile.TemporaryDirectory()
        self.root = Path(self.dir.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        for m in managers:
            self.stub(m, "#!/bin/sh\nexit 0\n")
        # qs: write each argument, NUL-separated, so newlines inside survive
        self.stub("qs", '#!/bin/sh\nfor a in "$@"; do printf "%s\\0" "$a"; done > "$QS_CALL"\n')
        self.env = dict(os.environ, HOME=str(self.root), XDG_DATA_HOME=str(self.root / "data"),
                        XDG_CONFIG_HOME=str(self.root / "config"), QS_CALL=str(self.root / "qs-call"),
                        PATH=f"{self.bin}:/usr/bin:/bin")

    def stub(self, name, text):
        p = self.bin / name
        p.write_text(text)
        p.chmod(0o755)

    def fm(self, *args):
        r = subprocess.run(["python3", "-I", str(SCRIPTS / "file-manager-share.py"), *args], env=self.env,
                           capture_output=True, text=True, timeout=20)
        return r

    def share(self, *paths):
        subprocess.run(["bash", str(SCRIPTS / "share-files.sh"), *paths], env=self.env, check=True, timeout=20)
        return (self.root / "qs-call").read_bytes().split(b"\0")[:-1]

    def close(self):
        self.dir.cleanup()


class FileManagerMenus(unittest.TestCase):
    def setUp(self):
        self.h = Home()

    def tearDown(self):
        self.h.close()

    def test_login_session_puts_menus_where_the_apps_look(self):
        # (the shell's own data folder there isn't the apps'; menus added
        # there by an earlier version move over)
        old = self.h.root / "data/kio/servicemenus/irice-share.desktop"
        old.parent.mkdir(parents=True)
        old.write_text("[Desktop Entry]\n")
        self.h.env["IRICE_TTY"] = "1"
        r = self.h.fm("status")
        self.assertEqual(r.returncode, 0, r.stderr)
        new = self.h.root / ".local/share/kio/servicemenus/irice-share.desktop"
        self.assertTrue(new.exists())
        self.assertIn("Send to a nearby device", new.read_text())
        self.assertFalse(old.exists())

    def test_status_lists_the_file_managers_found(self):
        h = Home(managers=("dolphin",))
        try:
            st = {m["id"]: m for m in json.loads(h.fm("status").stdout)}
            self.assertTrue(st["dolphin"]["found"])
            self.assertFalse(st["thunar"]["found"])
            self.assertFalse(any(m["installed"] for m in st.values()))
        finally:
            h.close()

    def test_install_adds_an_entry_to_each_and_remove_takes_only_ours(self):
        uca = self.h.root / "config/Thunar/uca.xml"
        uca.parent.mkdir(parents=True)
        uca.write_text('<?xml version="1.0" encoding="UTF-8"?>\n<actions>\n<action><icon>utilities-terminal</icon>'
                       '<name>Open Terminal Here</name><unique-id>1234-1</unique-id><command>exo-open --launch TerminalEmulator</command>'
                       '<directories/></action>\n</actions>\n')
        r = self.h.fm("install")
        self.assertEqual(r.returncode, 0, r.stderr)
        st = {m["id"]: m for m in json.loads(r.stdout)}
        self.assertTrue(all(m["installed"] for m in st.values()), st)

        data = self.h.root / "data"
        dolphin = data / "kio/servicemenus/irice-share.desktop"
        self.assertTrue(os.access(dolphin, os.X_OK), "KDE 6 needs a local service menu to be executable")
        self.assertIn(str(SCRIPTS / "share-files.sh"), dolphin.read_text())
        self.assertIn("%U", dolphin.read_text())
        self.assertIn("%F", (data / "nemo/actions/irice-share.nemo_action").read_text())

        actions = ET.parse(uca).getroot().findall("action")
        self.assertEqual(sorted(a.findtext("unique-id") for a in actions), ["1234-1", "irice-share"])

        # Installing again doesn't add a second copy
        self.h.fm("install", "thunar")
        self.assertEqual(len(ET.parse(uca).getroot().findall("action")), 2)

        r = self.h.fm("remove")
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertFalse(dolphin.exists())
        left = ET.parse(uca).getroot().findall("action")
        self.assertEqual([a.findtext("name") for a in left], ["Open Terminal Here"])

    def test_nautilus_without_the_python_extension_gets_a_scripts_entry(self):
        self.h.fm("install", "nautilus")
        script = self.h.root / "data/nautilus/scripts/Send to a nearby device…"
        if script.exists():  # (on machines with nautilus-python, the extension is used instead)
            self.assertTrue(os.access(script, os.X_OK))
            self.assertIn("NAUTILUS_SCRIPT_SELECTED_URIS", script.read_text())
        else:
            self.assertTrue((self.h.root / "data/nautilus-python/extensions/irice-share.py").exists())

    def test_a_broken_thunar_file_is_reported_not_overwritten(self):
        uca = self.h.root / "config/Thunar/uca.xml"
        uca.parent.mkdir(parents=True)
        uca.write_text("<actions><action>")
        r = self.h.fm("install", "thunar")
        self.assertNotEqual(r.returncode, 0)
        self.assertIn("can't be read", r.stderr + r.stdout)
        self.assertEqual(uca.read_text(), "<actions><action>")

    def test_unknown_names_are_refused(self):
        r = self.h.fm("install", "explorer")
        self.assertEqual(r.returncode, 2)
        self.assertIn("Unknown file manager", r.stderr)


class ShareFiles(unittest.TestCase):
    def test_awkward_names_arrive_intact(self):
        h = Home()
        try:
            names = ['two words.txt', 'quote"and\'apostrophe.png', '100% done.pdf', 'new\nline.txt', 'ünïcødé ✓.md']
            paths = [str(h.root / n) for n in names]
            call = h.share(*paths, "file:///already/a%20uri.txt")
            self.assertEqual([c.decode() for c in call[:5]], ["-p", str(SCRIPTS.parent.parent), "ipc", "call", "panels"])
            self.assertEqual(call[5].decode(), "shareFiles")
            lines = call[6].decode().split("\n")
            # Every path is one line, a file:// URI that decodes back to it
            self.assertEqual(len(lines), len(paths) + 1)
            back = [urllib.parse.unquote(l[len("file://"):]) for l in lines]
            self.assertEqual(back[:-1], paths)
            self.assertEqual(back[-1], "/already/a uri.txt")
        finally:
            h.close()

    def test_nothing_selected_calls_nothing(self):
        h = Home()
        try:
            subprocess.run(["bash", str(SCRIPTS / "share-files.sh")], env=h.env, check=True, timeout=20)
            self.assertFalse((h.root / "qs-call").exists())
        finally:
            h.close()


if __name__ == "__main__":
    unittest.main()
