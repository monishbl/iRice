#!/usr/bin/env python3
"""The words in your screenshots, so Search finds a shot by its text
(services/LiveText.qml). Everything is read on this computer by tesseract;
nothing is uploaded.

  shot-index.py run FOLDER INDEX LANGS   reads the screenshots in FOLDER (not
                                         its subfolders) that INDEX doesn't
                                         have yet, newest first, with
                                         tesseract -l LANGS, and adds their
                                         text; shots that are gone leave it.
                                         Prints {"ok", "added", "total"}

The index is JSON {"v": 1, "shots": {path: {"m": modified, "t": text}}},
owner-only (0600), written whole to a new file and renamed into place every
few shots: stopping it (on battery, in power saver) keeps what was read, and
Search never sees half a file. The shell starts it at the lowest priority
(nice, ionice, one tesseract thread).
"""
import json
import os
import re
import subprocess
import sys
import time

IMAGES = (".png", ".jpg", ".jpeg", ".webp")
MAX_TEXT = 20000  # characters kept per shot
MAX_SIZE = 40 * 1024 * 1024  # bigger files aren't screenshots
SETTLE = 5  # seconds a new file is left alone (it may still be being written)
SAVE_EVERY = 5


def load(index):
    try:
        with open(index, encoding="utf-8") as f:
            d = json.load(f)
        shots = d.get("shots") if isinstance(d, dict) else None
        return shots if isinstance(shots, dict) else {}
    except (OSError, ValueError):
        return {}


def save(index, shots):
    """Atomically and owner-only (a new folder: yours alone too)."""
    folder = os.path.dirname(index) or "."
    os.makedirs(folder, mode=0o700, exist_ok=True)
    tmp = index + ".tmp"
    fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC | os.O_NOFOLLOW, 0o600)
    os.fchmod(fd, 0o600)  # (an older .tmp may have had other bits)
    with os.fdopen(fd, "w", encoding="utf-8") as f:
        json.dump({"v": 1, "shots": shots}, f, ensure_ascii=False, separators=(",", ":"))
    os.replace(tmp, index)


def screenshots(folder, now=None):
    """[(path, modified)] newest first: pictures, settled, not too big."""
    now = time.time() if now is None else now
    out = []
    try:
        entries = list(os.scandir(folder))
    except OSError:
        return []
    for e in entries:
        if not e.name.lower().endswith(IMAGES) or e.name.startswith("."):
            continue
        try:
            st = e.stat(follow_symlinks=False)
        except OSError:
            continue
        if not e.is_file(follow_symlinks=False) or st.st_size > MAX_SIZE or 0 <= now - st.st_mtime < SETTLE:
            continue
        out.append((e.path, int(st.st_mtime)))
    out.sort(key=lambda x: -x[1])
    return out


def clean(text):
    """Tesseract's lines, without the empty ones and runs of spaces."""
    lines = (re.sub(r"[ \t]+", " ", l).strip() for l in text.splitlines())
    return "\n".join(l for l in lines if l)[:MAX_TEXT]


def read(path, langs):
    try:
        p = subprocess.run(["tesseract", path, "stdout", "-l", langs], capture_output=True, timeout=120)
    except (OSError, subprocess.TimeoutExpired):
        return None
    return clean(p.stdout.decode("utf-8", "replace")) if p.returncode == 0 else None


def run(folder, index, langs):
    if not re.fullmatch(r"[a-z_]+(\+[a-z_]+)*", langs):
        langs = "eng"
    shots = load(index)
    present = screenshots(folder)
    keep = {p for p, _ in present}
    gone = [p for p in shots if p not in keep]
    for p in gone:
        del shots[p]
    added = changed = 0
    for path, mtime in present:
        e = shots.get(path)
        if isinstance(e, dict) and e.get("m") == mtime:
            continue
        text = read(path, langs)
        if text is None:
            continue  # (unreadable now: tried again next time)
        shots[path] = {"m": mtime, "t": text}
        added += 1
        changed += 1
        if changed >= SAVE_EVERY:
            save(index, shots)
            changed = 0
    if changed or gone or not os.path.exists(index):
        save(index, shots)
    return {"ok": True, "added": added, "total": len(shots)}


def main(argv):
    if len(argv) == 4 and argv[0] == "run":
        r = run(argv[1], argv[2], argv[3])
    else:
        r = {"ok": False, "error": "usage: shot-index.py run FOLDER INDEX LANGS"}
    print(json.dumps(r))
    return 0 if r.get("ok") else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
