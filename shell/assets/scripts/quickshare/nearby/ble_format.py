"""What a Quick Share device puts in its Bluetooth LE advertisement.

Phones look for advertisements carrying service data under the 16-bit UUID
0xFEF3. Nearby Connections nests two layers there (google/nearby,
Apache-2.0; the files are named next to each function):

  medium advertisement   connections/implementation/mediums/ble/ble_advertisement.cc
    1 byte   version 2 (3 bits) | socket version 2 (3) | fast (1) | 2nd profile (1)
    3 bytes  service id hash, SHA-256("NearbySharing")[:3] = FC 9F 5E
    4 bytes  size of the data (big-endian), then the data:
  connections advertisement   connections/implementation/ble_advertisement.cc
    1 byte   version 1 (3 bits) | PCP (5): 0x23, point to point
    3 bytes  service id hash
    4 bytes  endpoint id (the one the mDNS name carries too)
    1 byte   endpoint info size, then the endpoint info (endpoint.py)
    6 bytes  Bluetooth Classic address (zeros: iRice doesn't do Classic)
    1 byte   UWB address size (0), 1 byte extra field (WebRTC: 0)
  … back in the medium advertisement:
    2 bytes  device token (random), then the extra fields: a mask byte,
             bit 0 → a 2-byte L2CAP PSM follows (mediums/ble/ble_advertisement.cc
             BleExtraFields)

That is ~70 bytes with a name: too big for a legacy advertisement (31), so it
goes out as an extended one. Phones that only scan legacy advertisements see
a 17-byte "header" instead (mediums/ble/ble_advertisement_header.cc), and
read the full advertisement from a GATT characteristic ("slot 0"):
    1 byte   version 2 (3 bits) | extended advertised too (1) | slots (4)
    10 bytes Bloom filter of the service ids on offer (bloom_filter.cc:
             MurmurHash3_x64_128, 5 hashes over 80 bits)
    4 bytes  advertisement hash: SHA-256 chained over the slots (ble.cc
             CreateAdvertisementHeader)
    2 bytes  PSM (big-endian), 0 for none
"""
import hashlib
import os
import struct

from .endpoint import PCP, SERVICE_HASH, SERVICE_ID

COPRESENCE_UUID = "0000fef3-0000-1000-8000-00805f9b34fb"
# GATT characteristics (mediums/ble/ble_utils.cc GenerateAdvertisementUuid;
# the weave pair from the Apple implementation's GNSUtils.m)
SLOT_UUID = "00000000-0000-3000-8000-0000000000{:02x}"
WEAVE_WRITE_UUID = "00000100-0004-1000-8000-001a11000101"   # the central writes here
WEAVE_NOTIFY_UUID = "00000100-0004-1000-8000-001a11000102"  # the peripheral notifies here

MEDIUM_VERSION = 2
SOCKET_VERSION = 2
CONNECTIONS_VERSION = 1
HEADER_VERSION = 2
MAX_ENDPOINT_INFO = 131  # connections/implementation/ble_advertisement.h
MAX_FAST_ENDPOINT_INFO = 17
PSM_BIT = 0x01
RX_INSTANT_BIT = 0x02
DUMMY_SERVICE_ID_LENGTH = 128  # ble.cc kDummyServiceIdLength
BLOOM_BYTES = 10
BLOOM_HASHES = 5


class AdvertisementError(ValueError):
    """Bytes that aren't a Nearby advertisement (or one iRice can't read)."""


# ── Building ─────────────────────────────────────────────────────────────
def connections_advertisement(endpoint_id, endpoint_info, bt_mac=b"\x00" * 6):
    eid = endpoint_id.encode("ascii")
    if len(eid) != 4:
        raise ValueError("endpoint ids are 4 characters")
    if not endpoint_info or len(endpoint_info) > MAX_ENDPOINT_INFO:
        raise ValueError("endpoint info must be 1-131 bytes")
    first = ((CONNECTIONS_VERSION << 5) & 0xE0) | (PCP & 0x1F)
    return (bytes([first]) + SERVICE_HASH + eid + bytes([len(endpoint_info)]) + endpoint_info
            + bytes(bt_mac) + b"\x00" + b"\x00")  # no UWB address, not WebRTC-connectable


def extra_fields(psm=0):
    """Nothing when there's nothing to say (BleExtraFields::operator ByteArray)."""
    if not psm:
        return b""
    return bytes([PSM_BIT]) + struct.pack(">H", psm)


def new_device_token():
    # (bleutils::GenerateDeviceToken: a hash of a random number)
    return hashlib.sha256(str(int.from_bytes(os.urandom(4), "big")).encode()).digest()[:2]


def medium_advertisement(data, device_token, psm=0):
    """The regular (not fast) advertisement, with its extra fields
    (ByteArrayWithExtraField: what both the extended advertisement and the
    GATT slot carry)."""
    if len(device_token) != 2:
        raise ValueError("device tokens are 2 bytes")
    first = ((MEDIUM_VERSION << 5) & 0xE0) | ((SOCKET_VERSION << 2) & 0x1C)
    return bytes([first]) + SERVICE_HASH + struct.pack(">I", len(data)) + data + device_token + extra_fields(psm)


def advertisement(endpoint_id, endpoint_info, device_token, psm=0):
    """The 0xFEF3 service data of a visible iRice."""
    return medium_advertisement(connections_advertisement(endpoint_id, endpoint_info), device_token, psm)


def header(slots, extended, psm=0, dummy_service_id=None):
    """The legacy advertisement's service data. `slots` are (service id,
    GATT advertisement) pairs in slot order."""
    dummy = dummy_service_id if dummy_service_id is not None else os.urandom(DUMMY_SERVICE_ID_LENGTH)
    bloom = BloomFilter()
    bloom.add(dummy)
    chain = hashlib.sha256(dummy).digest()[:4]
    for service_id, body in slots:
        bloom.add(service_id.encode() if isinstance(service_id, str) else service_id)
        chain = hashlib.sha256(chain + body).digest()[:4]
    first = ((HEADER_VERSION << 5) & 0xE0) | ((1 if extended else 0) << 4) | (len(slots) & 0x0F)
    return bytes([first]) + bloom.to_bytes() + chain + struct.pack(">H", psm)


# ── Reading (a phone's advertisement) ────────────────────────────────────
def parse_medium(raw):
    """{"version", "socket_version", "fast", "service_hash", "data",
    "device_token", "psm"} of a medium advertisement."""
    if not raw:
        raise AdvertisementError("empty")
    first = raw[0]
    out = {"version": (first & 0xE0) >> 5, "socket_version": (first & 0x1C) >> 2,
           "fast": bool(first & 0x02), "service_hash": b"", "device_token": b"", "psm": 0}
    if out["version"] not in (1, 2) or out["socket_version"] not in (1, 2):
        raise AdvertisementError(f"unsupported advertisement version {out['version']}/{out['socket_version']}")
    i = 1
    if out["fast"]:
        if len(raw) < 2:
            raise AdvertisementError("short fast advertisement")
        size = raw[1]
        i = 2
    else:
        if len(raw) < 8:
            raise AdvertisementError("short advertisement")
        out["service_hash"] = raw[1:4]
        (size,) = struct.unpack(">I", raw[4:8])
        i = 8
    if len(raw) < i + size:
        raise AdvertisementError("advertisement data cut short")
    out["data"] = raw[i:i + size]
    i += size
    if len(raw) >= i + 2:
        out["device_token"] = raw[i:i + 2]
        i += 2
    if len(raw) > i:
        mask = raw[i]
        i += 1
        if mask & PSM_BIT and len(raw) >= i + 2:
            (out["psm"],) = struct.unpack(">H", raw[i:i + 2])
    return out


def parse_connections(raw, fast=False):
    """{"pcp", "service_hash", "endpoint_id", "endpoint_info", "bt_mac"}."""
    minimum = 1 + 4 + 1 if fast else 1 + 3 + 4 + 1 + 6
    if len(raw) < minimum:
        raise AdvertisementError("short connections advertisement")
    first = raw[0]
    if (first & 0xE0) >> 5 != CONNECTIONS_VERSION:
        raise AdvertisementError("unsupported connections advertisement version")
    out = {"pcp": first & 0x1F, "service_hash": b"", "bt_mac": b""}
    if out["pcp"] not in (1, 2, 3):  # cluster, star, point to point
        raise AdvertisementError("unsupported PCP")
    i = 1
    if not fast:
        out["service_hash"] = raw[1:4]
        i = 4
    try:
        out["endpoint_id"] = raw[i:i + 4].decode("ascii")
    except UnicodeDecodeError as e:
        raise AdvertisementError("endpoint id isn't text") from e
    i += 4
    n = raw[i]
    i += 1
    info = raw[i:i + n]
    if not info or len(info) != n or n > (MAX_FAST_ENDPOINT_INFO if fast else MAX_ENDPOINT_INFO):
        raise AdvertisementError("endpoint info cut short")
    out["endpoint_info"] = info
    i += n
    if not fast:
        out["bt_mac"] = raw[i:i + 6]
    return out


def parse_service_data(raw):
    """A Quick Share device's advertisement → {"endpoint_id", "endpoint_info",
    "psm", "fast"}; raises AdvertisementError for anything else (a header
    only, another service, Nearby Presence beacons…)."""
    medium = parse_medium(raw)
    if not medium["fast"] and medium["service_hash"] != SERVICE_HASH:
        raise AdvertisementError("another Nearby service")
    conn = parse_connections(medium["data"], fast=medium["fast"])
    if not medium["fast"] and conn["service_hash"] != SERVICE_HASH:
        raise AdvertisementError("another Nearby service")
    return {"endpoint_id": conn["endpoint_id"], "endpoint_info": conn["endpoint_info"],
            "psm": medium["psm"], "fast": medium["fast"]}


def is_header(raw):
    return len(raw) == 17 and (raw[0] & 0xE0) >> 5 == HEADER_VERSION


def parse_header(raw):
    if not is_header(raw):
        raise AdvertisementError("not an advertisement header")
    return {"extended": bool(raw[0] & 0x10), "slots": raw[0] & 0x0F, "bloom": raw[1:11],
            "hash": raw[11:15], "psm": struct.unpack(">H", raw[15:17])[0]}


def header_offers(raw, service_id=SERVICE_ID):
    """Whether a header's Bloom filter may hold this service (no false negatives)."""
    return BloomFilter(parse_header(raw)["bloom"]).may_contain(service_id.encode())


# ── Bloom filter (mediums/ble/bloom_filter.cc) ───────────────────────────
class BloomFilter:
    """80 bits; bit n is bit (n % 8) of byte n // 8."""

    def __init__(self, raw=None):
        self.bits = bytearray(raw if raw is not None else BLOOM_BYTES)
        if len(self.bits) != BLOOM_BYTES:
            raise ValueError("the filter is 10 bytes")

    @staticmethod
    def _positions(data):
        h1, _ = murmur3_x64_128(data)
        lo = h1 & 0xFFFFFFFF
        hi = (h1 >> 32) & 0xFFFFFFFF
        for i in range(1, BLOOM_HASHES + 1):
            combined = (lo + i * hi) & 0xFFFFFFFF  # int32 arithmetic, wrapping
            if combined & 0x80000000:
                combined = ~(combined - (1 << 32))  # ~x of a negative int32 is non-negative
            yield combined % (BLOOM_BYTES * 8)

    def add(self, data):
        for p in self._positions(data):
            self.bits[p // 8] |= 1 << (p % 8)

    def may_contain(self, data):
        return all(self.bits[p // 8] & (1 << (p % 8)) for p in self._positions(data))

    def to_bytes(self):
        return bytes(self.bits)


def _rotl(x, r):
    return ((x << r) | (x >> (64 - r))) & 0xFFFFFFFFFFFFFFFF


def _fmix(k):
    k ^= k >> 33
    k = (k * 0xFF51AFD7ED558CCD) & 0xFFFFFFFFFFFFFFFF
    k ^= k >> 33
    k = (k * 0xC4CEB9FE1A85EC53) & 0xFFFFFFFFFFFFFFFF
    k ^= k >> 33
    return k


def murmur3_x64_128(data, seed=0):
    """(h1, h2) of Austin Appleby's MurmurHash3_x64_128 (little-endian blocks)."""
    mask = 0xFFFFFFFFFFFFFFFF
    c1, c2 = 0x87C37B91114253D5, 0x4CF5AD432745937F
    h1 = h2 = seed
    n = len(data)
    nblocks = n // 16
    for b in range(nblocks):
        k1, k2 = struct.unpack_from("<QQ", data, b * 16)
        k1 = (k1 * c1) & mask
        k1 = _rotl(k1, 31)
        k1 = (k1 * c2) & mask
        h1 ^= k1
        h1 = _rotl(h1, 27)
        h1 = (h1 + h2) & mask
        h1 = (h1 * 5 + 0x52DCE729) & mask
        k2 = (k2 * c2) & mask
        k2 = _rotl(k2, 33)
        k2 = (k2 * c1) & mask
        h2 ^= k2
        h2 = _rotl(h2, 31)
        h2 = (h2 + h1) & mask
        h2 = (h2 * 5 + 0x38495AB5) & mask
    tail = data[nblocks * 16:]
    k1 = k2 = 0
    for i in range(len(tail) - 1, 7, -1):
        k2 ^= tail[i] << ((i - 8) * 8)
    if len(tail) > 8:
        k2 = (k2 * c2) & mask
        k2 = _rotl(k2, 33)
        k2 = (k2 * c1) & mask
        h2 ^= k2
    for i in range(min(len(tail), 8) - 1, -1, -1):
        k1 ^= tail[i] << (i * 8)
    if tail:
        k1 = (k1 * c1) & mask
        k1 = _rotl(k1, 31)
        k1 = (k1 * c2) & mask
        h1 ^= k1
    h1 ^= n
    h2 ^= n
    h1 = (h1 + h2) & mask
    h2 = (h2 + h1) & mask
    h1 = _fmix(h1)
    h2 = _fmix(h2)
    h1 = (h1 + h2) & mask
    h2 = (h2 + h1) & mask
    return h1, h2
