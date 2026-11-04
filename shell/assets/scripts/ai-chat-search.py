#!/usr/bin/env python3
"""Search the AI sidebar's past chats through their whole text.

    ai-chat-search.py <folder>     (the words to find on stdin)

Prints the id of each chat (ai-chats/<id>.json) whose title or messages hold
every word, ignoring case. The index's titles and previews are searched in
the shell as you type; this finds words further into a chat. The query comes
on stdin so it never shows in a process list.
"""
import json
from pathlib import Path
import re
import sys

ID = re.compile(r"^[a-z0-9-]{1,64}$")


def words(query):
    return [w for w in query.casefold().split() if w]


def text_of(chat):
    parts = [str(chat.get("title", ""))]
    for row in chat.get("rows", []):
        if isinstance(row, dict) and row.get("role") in ("user", "assistant"):
            parts.append(str(row.get("text", "")))
    return " ".join(parts).casefold()


def search(folder, query):
    want = words(query)
    if not want:
        return []
    found = []
    for path in sorted(Path(folder).glob("*.json")):
        if not ID.match(path.stem) or path.stem == "index":
            continue
        try:
            chat = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, ValueError):
            continue
        if isinstance(chat, dict):
            hay = text_of(chat)
            if all(w in hay for w in want):
                found.append(path.stem)
    return found


if __name__ == "__main__":
    for chat_id in search(sys.argv[1], sys.stdin.read()):
        print(chat_id)
