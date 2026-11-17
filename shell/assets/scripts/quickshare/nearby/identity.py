"""This computer's Quick Share identity, made once and kept.

Phones list a device by what it advertises: its endpoint id (4 characters,
also in the mDNS name), the 16 bytes after the first byte of its endpoint
info, and its Bluetooth device token. Until 17 Nov all three were new each
time the helper started, and it starts each time visibility goes from No
one to Everyone: the phone then listed the computer once per start ("five
Monish-Work"). Now they are made the first time and kept in
$XDG_STATE_HOME/tessera/quickshare/identity.json (only you can read it);
a lost or damaged file is made again.
"""
import json
import os
import string

from . import endpoint

ALPHABET = string.ascii_uppercase + string.digits


def path():
    state = os.environ.get("XDG_STATE_HOME") or os.path.expanduser("~/.local/state")
    return os.path.join(state, "tessera", "quickshare", "identity.json")


def _valid(d):
    try:
        return (isinstance(d, dict) and isinstance(d.get("endpoint_id"), str) and len(d["endpoint_id"]) == 4
                and all(c in ALPHABET for c in d["endpoint_id"])
                and len(bytes.fromhex(d.get("salt", ""))) == 16 and len(bytes.fromhex(d.get("token", ""))) == 2)
    except (TypeError, ValueError):
        return False


def new():
    return {"endpoint_id": endpoint.new_endpoint_id(), "salt": os.urandom(16).hex(), "token": os.urandom(2).hex()}


def load(file=None):
    """(identity dict, "kept" | "new"): the one on disk, else a new one saved
    there (an unwritable state folder still gives one, for this run only)."""
    file = file or path()
    try:
        with open(file, encoding="utf-8") as f:
            d = json.load(f)
        if _valid(d):
            return d, "kept"
    except (OSError, ValueError):
        pass
    d = new()
    try:
        os.makedirs(os.path.dirname(file), mode=0o700, exist_ok=True)
        tmp = file + ".tmp"
        fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
        with os.fdopen(fd, "w", encoding="utf-8") as f:
            json.dump(d, f)
        os.replace(tmp, file)
    except OSError:
        return d, "new (not saved)"
    return d, "new"
