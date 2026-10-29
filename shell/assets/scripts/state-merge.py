#!/usr/bin/env python3
"""Merge the shell's UI state journals into state.json (common/UiState.qml).

Each change the shell makes is first written to its own small journal file
under pending/, so a crash or a quit can't lose it. This folds them into the
shared state.json, under a lock (two shells can share the file: the dev
sandbox and a tty session), field by field, keeping the newest revision of
each, so neither shell overwrites the other's unrelated changes.

A state.json that can't be read as an object is set aside as
state.json.corrupt-<time> and the merge starts from empty: one bad file must
not stop the shell from loading and saving its state ever again.
"""
import fcntl
import json
import os
from pathlib import Path
import sys
import tempfile
import time


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
        except (ValueError, UnicodeDecodeError):
            state = None
        if not isinstance(state, dict):
            os.replace(path, path.with_name('state.json.corrupt-' + time.strftime('%Y%m%d-%H%M%S')))
            state = {}
        if not isinstance(state.get('_irice_revisions'), dict):
            state['_irice_revisions'] = {}
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
