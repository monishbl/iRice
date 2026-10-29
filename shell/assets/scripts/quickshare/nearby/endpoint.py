"""How a Quick Share device names itself.

On the network a device that's visible advertises an mDNS service of type
_FC9F5ED42C8A._tcp (FC9F5ED42C8A is the start of SHA-256("NearbySharing")).
Its instance name is 10 bytes, base64url without padding:
  0x23 (the Nearby "PCP" for point-to-point), 4 ASCII characters of
  endpoint id, 3 bytes of service id hash (FC 9F 5E), 2 zero bytes
Its TXT record "n" (base64url) is the endpoint info, which the connection
request repeats:
  1 byte   version (3 bits) | hidden (1) | device type (3) | reserved (1)
  16 bytes random (contact-only devices put an encrypted key here)
  1 byte   name length, then the name in UTF-8
A device visible to everyone sends its name in the clear; one visible to
contacts only doesn't (its info is just the first 17 bytes), and stage 1
can't show those.
"""
import base64
import hashlib
import os
import random
import string

SERVICE_ID = "NearbySharing"
SERVICE_HASH = hashlib.sha256(SERVICE_ID.encode()).digest()[:3]  # FC 9F 5E
MDNS_TYPE = "_" + hashlib.sha256(SERVICE_ID.encode()).hexdigest()[:12].upper() + "._tcp"  # _FC9F5ED42C8A._tcp
PCP = 0x23

UNKNOWN, PHONE, TABLET, LAPTOP = 0, 1, 2, 3
TYPE_NAMES = {UNKNOWN: "unknown", PHONE: "phone", TABLET: "tablet", LAPTOP: "laptop"}
MAX_NAME = 255


def b64(data):
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode()


def unb64(text):
    if isinstance(text, bytes):
        text = text.decode("ascii", "strict")
    return base64.urlsafe_b64decode(text + "=" * (-len(text) % 4))


def new_endpoint_id():
    # Letters and digits, as Nearby Connections makes them
    alphabet = string.ascii_uppercase + string.digits
    return "".join(random.SystemRandom().choice(alphabet) for _ in range(4))


def service_name(endpoint_id):
    eid = endpoint_id.encode("ascii")
    if len(eid) != 4:
        raise ValueError("endpoint ids are 4 characters")
    return b64(bytes([PCP]) + eid + SERVICE_HASH + b"\x00\x00")


def parse_service_name(name):
    """The endpoint id of a Quick Share service name, or None if it isn't one."""
    try:
        raw = unb64(name)
    except (ValueError, UnicodeError):
        return None
    if len(raw) < 8 or raw[0] != PCP or raw[5:8] != SERVICE_HASH:
        return None
    try:
        return raw[1:5].decode("ascii")
    except UnicodeDecodeError:
        return None


def _clip(name):
    raw = name.encode("utf-8")[:MAX_NAME]
    return raw.decode("utf-8", "ignore").encode("utf-8")  # (not mid-character)


def endpoint_info(name, device_type=LAPTOP, visible=True):
    first = ((0 if visible else 1) << 4) | ((device_type & 7) << 1)
    raw = _clip(name)
    return bytes([first]) + os.urandom(16) + bytes([len(raw)]) + raw


def parse_endpoint_info(info):
    """{"type": "phone"…, "name": str or None, "hidden": bool}; None if unreadable."""
    if not info or len(info) < 17:
        return None
    first = info[0]
    out = {"type": TYPE_NAMES.get((first >> 1) & 7, "unknown"), "hidden": bool(first & 0x10), "name": None}
    if len(info) >= 18:
        n = info[17]
        raw = info[18:18 + n]
        if len(raw) == n and n > 0:
            name = raw.decode("utf-8", "replace")
            # Names are shown in the shell: no control characters
            out["name"] = "".join(ch for ch in name if ch.isprintable()).strip() or None
    return out
