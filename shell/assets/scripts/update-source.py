#!/usr/bin/env python3
"""Locate the source checkout for both copied and linked shell installations."""
import os
from pathlib import Path
import sys


def resolve(shell):
    candidates = [os.environ.get('IRICE_UPDATE_REPO', ''), str(Path(shell).resolve().parent)]
    for base in [os.environ.get('XDG_DATA_HOME', str(Path.home() / '.local/share')), str(Path.home() / '.local/share')]:
        try:
            candidates.append((Path(base) / 'irice/source').read_text().strip())
        except OSError:
            pass
    for candidate in candidates:
        if candidate and (Path(candidate) / 'tools/irice-update.py').is_file() and (Path(candidate) / '.git').exists():
            return str(Path(candidate).resolve())
    return ''


if __name__ == '__main__':
    print(resolve(sys.argv[1]))
