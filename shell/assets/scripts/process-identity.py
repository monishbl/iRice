#!/usr/bin/env python3
"""Signal only verified process incarnations, using pidfds (never reused PIDs)."""
import os
from pathlib import Path
import signal
import sys
import time


def identity(pid):
    fields = Path('/proc', str(pid), 'stat').read_text().rsplit(') ', 1)[1].split()
    return fields[19], int(fields[2])


def handle(pid, start):
    fd = os.pidfd_open(int(pid))
    try:
        if os.stat('/proc/' + str(pid)).st_uid != os.getuid() or identity(pid)[0] != str(start):
            raise ValueError('Process identity changed')
        return fd
    except BaseException:
        os.close(fd)
        raise


def stop_group(group, start, dry=False):
    # A missing/reused leader cannot establish ownership of the surviving group.
    if identity(group) != (str(start), int(group)):
        return
    handles = []
    try:
        for entry in Path('/proc').iterdir():
            if not entry.name.isdigit():
                continue
            try:
                born, pgid = identity(entry.name)
                if pgid == int(group):
                    handles.append(handle(entry.name, born))
            except (OSError, ValueError):
                pass
        if identity(group) != (str(start), int(group)):
            return
        if dry:
            print('would end verified watcher group', group, 'processes:', len(handles))
            return
        for sig in (signal.SIGTERM, signal.SIGKILL):
            for fd in handles:
                try:
                    signal.pidfd_send_signal(fd, sig)
                except ProcessLookupError:
                    pass
            if sig == signal.SIGTERM:
                time.sleep(.3)
    finally:
        for fd in handles:
            os.close(fd)


def reap(dry):
    directory = Path(os.environ.get('XDG_RUNTIME_DIR', '/run/user/' + str(os.getuid()))) / 'irice-watchers'
    for record in directory.glob('*'):
        alive = False
        try:
            wrapper, born, group, start = record.read_text().split()
            if not all(x.isdigit() for x in (wrapper, born, group, start)):
                continue
            try:
                alive = identity(wrapper)[0] == born
            except OSError:
                pass
            if not alive:
                stop_group(group, start, dry)
        except (OSError, ValueError):
            pass
        finally:
            if not dry and not alive:
                record.unlink(missing_ok=True)


def main(args):
    try:
        if args[0] == 'reap':
            reap('--dry-run' in args)
        elif args[0] == 'group':
            stop_group(args[1], args[2])
        elif args[0] == 'interrupt':
            fd = handle(args[1], args[2])
            try:
                signal.pidfd_send_signal(fd, signal.SIGINT)
            finally:
                os.close(fd)
    except (OSError, ValueError, IndexError):
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
