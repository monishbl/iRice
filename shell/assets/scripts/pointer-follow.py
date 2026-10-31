#!/usr/bin/env python3
"""The pointer's place, over and over, while a button is held (services/Shelf.qml
starts it after a moment's hold and stops it when the button comes up): one
"X, Y" line every 40 ms from Hyprland's socket, for at most a minute (or the
seconds given: pointer-follow.py 1.5, for the ring that finds the pointer)."""
import os
import socket
import sys
import time

INTERVAL = 0.04
LIMIT = 60


def main():
    path = os.path.join(os.environ.get("XDG_RUNTIME_DIR", ""), "hypr",
                        os.environ.get("HYPRLAND_INSTANCE_SIGNATURE", ""), ".socket.sock")
    try:
        limit = min(LIMIT, float(sys.argv[1])) if len(sys.argv) > 1 else LIMIT
    except ValueError:
        limit = LIMIT
    end = time.monotonic() + limit
    while time.monotonic() < end:
        try:
            with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
                s.connect(path)
                s.sendall(b"cursorpos")
                reply = s.recv(128).decode(errors="replace").strip()
            sys.stdout.write(reply + "\n")
            sys.stdout.flush()
        except (OSError, BrokenPipeError):
            return 1
        time.sleep(INTERVAL)
    return 0


if __name__ == "__main__":
    sys.exit(main())
