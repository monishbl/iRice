#!/usr/bin/env python3
"""Decode missing cliphist previews once and publish complete, stable paths."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile


def previews(db, directory, pairs):
    directory = Path(directory)
    directory.mkdir(mode=0o700, parents=True, exist_ok=True)
    os.chmod(directory, 0o700)
    for identifier, target in pairs:
        path = Path(target)
        if path.parent != directory or not identifier.isdigit():
            continue
        if not path.exists() or not path.stat().st_size:
            fd, tmp = tempfile.mkstemp(dir=directory)
            try:
                with os.fdopen(fd, 'wb') as output:
                    result = subprocess.run(['cliphist', '-db-path', db, 'decode', identifier],
                                            stdout=output, stderr=subprocess.DEVNULL, timeout=20)
                if result.returncode or not os.path.getsize(tmp):
                    continue
                os.replace(tmp, path)
            except (OSError, subprocess.TimeoutExpired):
                continue
            finally:
                if os.path.exists(tmp):
                    os.unlink(tmp)
        print(str(path), flush=True)


if __name__ == '__main__':
    previews(sys.argv[1], sys.argv[2], zip(sys.argv[3::2], sys.argv[4::2]))
