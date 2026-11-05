#!/usr/bin/env python3
"""A fake greetd for trying the login screen in the sandbox (never the real
one): greetd's IPC (greetd-ipc(7): a 32-bit length in the machine's byte
order, then JSON) on a socket of your choosing. It knows two made-up
people, both with the password "tessera":
  ada    a password prompt
  grace  a fingerprint reader first: "Place your finger…", then (after
         --finger seconds) a miss, then the password prompt
Anyone else gets a prompt and is always refused. It never runs anything:
a start_session is written to --log (one JSON line) and answered.

  tools/fake-greetd.py SOCKET [--finger 8] [--log FILE]
Point the greeter at it with GREETD_SOCK=SOCKET (the QML one too)."""
import argparse
import json
import os
import select
import socket
import stat
import struct
import sys
import threading
import time

T0 = time.monotonic()

PEOPLE = {"ada": "tessera", "grace": "tessera"}


def frame(obj):
    body = json.dumps(obj).encode()
    return struct.pack("=I", len(body)) + body


def read_exact(conn, n):
    data = b""
    while len(data) < n:
        chunk = conn.recv(n - len(data))
        if not chunk:
            return None
        data += chunk
    return data


def read_req(conn):
    head = read_exact(conn, 4)
    if head is None:
        return None
    (n,) = struct.unpack("=I", head)
    if n > 1 << 16:
        return None
    body = read_exact(conn, n)
    return None if body is None else json.loads(body)


def auth(kind, msg):
    return {"type": "auth_message", "auth_message_type": kind, "auth_message": msg}


def serve(conn, finger_wait, log):
    who = None  # [user, step]
    with conn:
        while True:
            req = read_req(conn)
            if req is None:
                return
            t = req.get("type")
            # (what came, when: the type and, for a response, only whether it had one)
            print(f"fake-greetd: {time.monotonic() - T0:7.2f}s {t}" + (" (null)" if t == "post_auth_message_response" and req.get("response") is None else ""), file=sys.stderr, flush=True)
            if t == "create_session":
                if who is not None:
                    conn.sendall(frame({"type": "error", "error_type": "error", "description": "a session is already being configured"}))
                    continue
                user = str(req.get("username", ""))
                who = [user, 0]
                print(f"fake-greetd: create_session {user!r}", file=sys.stderr)
                conn.sendall(frame(auth("info", "Place your finger on the fingerprint reader") if user == "grace" else auth("secret", "Password: ")))
            elif t == "post_auth_message_response":
                if who is None:
                    conn.sendall(frame({"type": "error", "error_type": "error", "description": "no session"}))
                    continue
                user, step = who
                if user == "grace" and step == 0:
                    # (PAM waits for a finger: as greetd, the answer comes
                    # later; a request meanwhile gets this one answered first)
                    who[1] = 1
                    ready, _, _ = select.select([conn], [], [], finger_wait)
                    conn.sendall(frame(auth("error", "Failed to match fingerprint") if not ready else {"type": "error", "error_type": "error", "description": "interrupted"}))
                elif user == "grace" and step == 1:
                    who[1] = 2
                    conn.sendall(frame(auth("secret", "Password: ")))
                elif PEOPLE.get(user) is not None and req.get("response") == PEOPLE[user]:
                    print(f"fake-greetd: {user} authenticated", file=sys.stderr)
                    conn.sendall(frame({"type": "success"}))
                else:
                    print(f"fake-greetd: {user} refused", file=sys.stderr)
                    conn.sendall(frame({"type": "error", "error_type": "auth_error", "description": "pam_authenticate: AUTH_ERR"}))
            elif t == "start_session":
                entry = {"user": who[0] if who else None, "cmd": req.get("cmd"), "env": req.get("env")}
                print(f"fake-greetd: start_session {entry}", file=sys.stderr)
                if log:
                    with open(log, "a") as f:
                        f.write(json.dumps(entry) + "\n")
                conn.sendall(frame({"type": "success"}))
            elif t == "cancel_session":
                who = None
                conn.sendall(frame({"type": "success"}))
            else:
                conn.sendall(frame({"type": "error", "error_type": "error", "description": "unknown request"}))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("socket")
    ap.add_argument("--finger", type=float, default=8.0, help="seconds the reader listens before it misses")
    ap.add_argument("--log", default="")
    a = ap.parse_args()
    if os.path.lexists(a.socket):
        if not stat.S_ISSOCK(os.lstat(a.socket).st_mode):
            sys.exit(f"fake-greetd: {a.socket} exists and isn't a socket")
        os.unlink(a.socket)
    srv = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    srv.bind(a.socket)
    os.chmod(a.socket, 0o600)
    srv.listen(4)
    print(f"fake-greetd: listening on {a.socket}", file=sys.stderr, flush=True)
    try:
        while True:
            conn, _ = srv.accept()
            threading.Thread(target=serve, args=(conn, a.finger, a.log), daemon=True).start()
    except KeyboardInterrupt:
        pass
    finally:
        os.unlink(a.socket)


if __name__ == "__main__":
    main()
