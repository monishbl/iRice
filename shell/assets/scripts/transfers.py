#!/usr/bin/env python3
"""Read-only copy progress with start identities and proper JSON encoding."""
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


def downloads(directory):
    result = []
    try:
        for path in Path(directory).iterdir():
            if path.suffix not in ('.part', '.crdownload', '.download') or not path.is_file():
                continue
            try:
                result.append({'path': str(path), 'bytes': path.stat().st_size})
            except OSError:
                pass
    except OSError:
        pass
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
    if len(sys.argv) == 3 and sys.argv[1] == '--downloads':
        print(json.dumps(downloads(sys.argv[2])))
    else:
        main()
