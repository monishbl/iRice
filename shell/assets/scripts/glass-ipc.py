#!/usr/bin/env python3
"""Sends the shell's requests to Hyprland's request socket, one per stdin line,
and prints each answer followed by a record separator (0x1E).

Why not a Socket in QML: Hyprland, once it accepts a connection, blocks its
main loop (up to 5 s) until the request arrives. The shell writes from its
event loop, so if it's meanwhile waiting on Hyprland itself (Qt does that when
it creates a popup or window), both wait on each other and everything freezes
for 5 seconds. Here connect, write and read happen back to back, in their own
process, so Hyprland never waits on the shell.

    glass-ipc.py <socket path>
Ends when stdin closes (the shell quit).
"""
import socket
import sys

SEP = "\x1e"


def request(path, cmd):
    data = cmd.encode()
    # Hyprland reads requests in 1023-byte chunks and waits for more when a
    # chunk is exactly full: never end a request on that edge
    if len(data) % 1023 == 0:
        data += b" "
    with socket.socket(socket.AF_UNIX, socket.SOCK_STREAM) as s:
        s.settimeout(3)
        s.connect(path)
        s.sendall(data)
        out = []
        while True:
            chunk = s.recv(8192)
            if not chunk:
                break
            out.append(chunk)
    return b"".join(out).decode(errors="replace")


def main():
    path = sys.argv[1]
    for line in sys.stdin:
        cmd = line.rstrip("\n")
        if not cmd:
            continue
        try:
            answer = request(path, cmd)
        except OSError as e:
            answer = "error: " + str(e)
        try:
            sys.stdout.write(answer.replace(SEP, " ") + SEP)
            sys.stdout.flush()
        except BrokenPipeError:
            return


if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        pass
