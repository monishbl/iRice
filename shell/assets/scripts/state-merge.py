#!/usr/bin/env python3
"""Merge durable per-field UI journals under a lock, preserving unrelated edits."""
import fcntl
import json
import os
from pathlib import Path
import sys
import tempfile


def merge(directory):
    directory = Path(directory)
    directory.mkdir(parents=True, exist_ok=True, mode=0o700)
    os.chmod(directory, 0o700)
    pending = directory / 'pending'
    pending.mkdir(exist_ok=True, mode=0o700)
    os.chmod(pending, 0o700)
    with open(directory / '.state.lock', 'a') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        path = directory / 'state.json'
        try:
            state = json.loads(path.read_text())
        except FileNotFoundError:
            state = {}
        if not isinstance(state, dict):
            raise ValueError('State is not an object')
        revisions = state.setdefault('_irice_revisions', {})
        consumed = []
        for journal in pending.glob('*.json'):
            try:
                update = json.loads(journal.read_text())
                for key, change in update.items():
                    if key.startswith('_'):
                        continue
                    if change['revision'] > revisions.get(key, [0, '', 0]):
                        state[key] = change['value']
                        revisions[key] = change['revision']
                consumed.append(journal)
            except (ValueError, KeyError, TypeError):
                continue # Never erase a malformed/unreadable recovery journal.
        fd, temp = tempfile.mkstemp(dir=directory)
        try:
            with os.fdopen(fd, 'w') as out:
                json.dump(state, out, ensure_ascii=False)
                out.flush()
                os.fsync(out.fileno())
            os.replace(temp, path)
        finally:
            if os.path.exists(temp): os.unlink(temp)
        for journal in consumed:
            journal.unlink(missing_ok=True)


if __name__ == '__main__':
    merge(sys.argv[1])
