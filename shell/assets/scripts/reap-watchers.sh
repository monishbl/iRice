#!/usr/bin/env bash
# Run once when the shell starts: ends iRice watchers left behind by an
# earlier shell that crashed or was killed outright, so its wrapper
# (watch.sh) never got to end them. Copies piling up like that once kept the
# laptop hot (25 of them at 250% CPU).
#
# watch.sh writes each watcher group down in $XDG_RUNTIME_DIR/irice-watchers
# (the wrapper and the group's leader, each with its start time, and the
# group's token). A record whose wrapper is gone is a leftover: its group's
# members are ended, even when the leader itself has already exited (it
# does, with its wrapper), as long as they started no earlier than that
# leader and, with the leader gone, carry the token. Processes are held by
# pidfd before being signalled, so a pid reused meanwhile is never touched;
# nothing without a record is (no guessing from names).
#   reap-watchers.sh [--dry-run]   (--dry-run: say what would be ended)
exec python3 -I "$(dirname -- "$0")/process-identity.py" reap "$@"
