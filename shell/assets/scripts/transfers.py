#!/usr/bin/env python3
"""Copies in progress, for the island (services/Transfers.qml).

Run under watch.sh (through transfers.sh), for as long as the island shows
file copies. It looks for this user's cp, mv, rsync, dd, install, pv and
gcp, and for each one the largest file it's reading and how far in it is.
Each is reported with its start time, so cancelling one (process-identity.py)
can never hit another process that later got the same pid. Every 2 s while
something is copying, every 10 s when nothing is; a line of JSON only when
the list changed. It ends when the shell that started it is gone.
"""
import json
import os
from pathlib import Path
import sys
import time


def scan():
    result = []
    for proc in Path('/proc').iterdir():
        if not proc.name.isdigit(): continue
        try:
            if proc.stat().st_uid != os.getuid(): continue
            name = (proc / 'comm').read_text().strip()
            if name not in ('cp', 'mv', 'rsync', 'dd', 'install', 'pv', 'gcp'): continue
            start = (proc / 'stat').read_text().rsplit(') ', 1)[1].split()[19]
            best = None
            for fd in (proc / 'fd').iterdir():
                try:
                    path = os.readlink(fd)
                    if not os.path.isfile(path): continue
                    info = dict(line.split(':', 1) for line in (proc / 'fdinfo' / fd.name).read_text().splitlines())
                    if int(info['flags'].strip(), 8) & 3: continue
                    size = os.stat(fd).st_size
                    if best is None or size > best['size']:
                        best = dict(pid=int(proc.name), start=start, cmd=name, file=path, pos=int(info['pos']), size=size)
                except (OSError, ValueError, KeyError): pass
            if best and (proc / 'stat').read_text().rsplit(') ', 1)[1].split()[19] == start:
                result.append(best)
        except (OSError, ValueError, IndexError): pass
    return result


def main():
    parent, previous = os.getppid(), None
    while os.getppid() == parent:
        items = scan()
        data = json.dumps(items)
        if data != previous:
            print(data, flush=True)
            previous = data
        time.sleep(2 if items else 10)


if __name__ == '__main__':
    main()
