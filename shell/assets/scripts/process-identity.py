#!/usr/bin/env python3
"""Signal processes the shell started, and only those.

A pid alone isn't enough: once a process ends, its pid can be given to a new,
unrelated one. So every process is named by its pid and its start time (from
/proc), held by a pidfd once that's been checked, and only then signalled.
A pidfd keeps pointing at the same process even if the pid is reused.

  reap [--dry-run]       end watcher groups whose wrapper is gone
                         (reap-watchers.sh, at shell start)
  group <pgid> <start> [token]  end a watcher group (watch.sh, on its way out)
  interrupt <pid> <start>  SIGINT one process (cancelling a copy)
"""
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


def carries(pid, token):
    # The watcher's environment as it was started (children inherit it);
    # only readable for our own processes
    want = b'IRICE_WATCH_TOKEN=' + token.encode()
    return want in Path('/proc', str(pid), 'environ').read_bytes().split(b'\0')


def stop_group(group, start, token=None, dry=False):
    # The group's members, each held by a pidfd before anything is signalled.
    # While the recorded leader is alive, a member is ours if it's in its
    # group and started no earlier than it: the kernel never hands a group
    # id that's still in use to a new group.
    # A different process leading a group with this id: the old group had
    # emptied (only then is its id handed out again), so nothing here is ours
    try:
        leader = identity(group) == (str(start), int(group))
        if not leader and identity(group)[1] == int(group):
            return
    except OSError:
        leader = False
    # The leader has exited (as it does when its wrapper dies: pdeathsig),
    # which is exactly when helpers are left behind. Its group may still have
    # members of ours, but nothing proves the group is the recorded one any
    # more, so only members carrying the group's token count. A record
    # without one (written before tokens) ends nothing.
    if not leader and not (token and len(token) >= 16 and all(c in '0123456789abcdef' for c in token)):
        return
    handles = []
    try:
        for entry in Path('/proc').iterdir():
            if not entry.name.isdigit():
                continue
            try:
                born, pgid = identity(entry.name)
                if pgid == int(group) and int(born) >= int(start) and (leader or carries(entry.name, token)):
                    handles.append(handle(entry.name, born))
            except (OSError, ValueError):
                pass
        if not handles:
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
            wrapper, born, group, start, *token = record.read_text().split()
            if not all(x.isdigit() for x in (wrapper, born, group, start)) or len(token) > 1:
                continue
            try:
                alive = identity(wrapper)[0] == born
            except OSError:
                pass
            if not alive:
                stop_group(group, start, token[0] if token else None, dry)
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
            stop_group(args[1], args[2], args[3] if len(args) > 3 else None)
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
