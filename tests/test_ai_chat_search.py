"""ai-chat-search.py: the AI sidebar's search through whole past chats (the
index's titles and previews are searched in the shell; this finds words
further in). The words come on stdin, never in the arguments."""
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "shell/assets/scripts/ai-chat-search.py"


def chat(folder, chat_id, title, *texts, role="user"):
    rows = [{"role": role, "text": t} for t in texts]
    (Path(folder) / f"{chat_id}.json").write_text(json.dumps({"id": chat_id, "title": title, "rows": rows}))


def run(folder, query):
    out = subprocess.run([sys.executable, "-I", str(SCRIPT), str(folder)], input=query, capture_output=True, text=True, timeout=30, check=True)
    return out.stdout.split()


class Search(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        d = self.dir.name
        chat(d, "a1", "Fonts", "Which font for code?", "Try Geist Mono, it has good kerning")
        chat(d, "b2", "Trip", "Plan a weekend in Goa")
        chat(d, "c3", "Notes", "kerning is mentioned only in a note", role="note")
        (Path(d) / "index.json").write_text(json.dumps({"chats": [{"id": "x", "title": "kerning"}]}))
        (Path(d) / "broken.json").write_text("{not json")
        (Path(d) / "Bad_Name.json").write_text(json.dumps({"title": "kerning"}))

    def tearDown(self):
        self.dir.cleanup()

    def test_finds_words_deep_in_messages_ignoring_case(self):
        self.assertEqual(run(self.dir.name, "KERNING"), ["a1"])
        self.assertEqual(run(self.dir.name, "geist mono"), ["a1"])
        self.assertEqual(run(self.dir.name, "goa"), ["b2"])

    def test_every_word_must_be_there(self):
        self.assertEqual(run(self.dir.name, "goa kerning"), [])

    def test_titles_count_but_notes_index_and_odd_files_do_not(self):
        self.assertEqual(run(self.dir.name, "trip"), ["b2"])
        self.assertEqual(run(self.dir.name, "mentioned"), [])

    def test_an_empty_query_finds_nothing(self):
        self.assertEqual(run(self.dir.name, "   "), [])

    def test_a_missing_folder_finds_nothing(self):
        self.assertEqual(run(Path(self.dir.name) / "nope", "kerning"), [])


if __name__ == "__main__":
    unittest.main()
