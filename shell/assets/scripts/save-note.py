#!/usr/bin/env python3
"""Publish a complete note without replacing another note on a name collision."""
import os
from pathlib import Path
import sys
import tempfile


def save(path, content):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix='.note-', dir=path.parent)
    try:
        with os.fdopen(fd, 'w') as out:
            out.write(content)
            out.flush()
            os.fsync(out.fileno())
        os.link(temporary, path)  # atomic publication; fails if destination exists
    finally:
        os.unlink(temporary)


if __name__ == '__main__':
    save(sys.argv[1], sys.stdin.read())
