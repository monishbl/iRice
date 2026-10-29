"""iRice's Quick Share (Nearby Share) engine.

The protocol is Google's Nearby Connections with Nearby Sharing on top:
  transport.py   a byte stream carrying length-prefixed frames (TCP today;
                 Bluetooth LE and Wi-Fi Direct later plug in here)
  ukey2.py       the UKEY2 key exchange and the keys and PIN it yields
  secure.py      the encrypted channel (SecureMessage around D2D messages)
  endpoint.py    how a device names itself (mDNS service name, endpoint info)
  connection.py  Nearby Connections: connect, encrypt, payloads, keep-alive
  sharing.py     Nearby Sharing: introduce, accept, send and receive files
  avahi.py       finding and announcing devices on the network (mDNS)

The message formats come from Google's own .proto files (Apache-2.0), see
shell/assets/quickshare/proto/NOTICE; pb/ holds the code generated from them.
"""
import os
import sys

# The generated modules import each other as top-level packages ("from proto
# import …", "connections.…"). Their folder goes first on the path so a
# same-named installed package (proto-plus ships a "proto") can't stand in.
_PB = os.path.join(os.path.dirname(os.path.abspath(__file__)), "pb")
if _PB not in sys.path:
    sys.path.insert(0, _PB)

try:
    from connections.implementation.proto import offline_wire_formats_pb2 as offline  # noqa: E402
    from sharing.proto import wire_format_pb2 as sharing_pb  # noqa: E402
    from proto import sharing_enums_pb2 as sharing_enums  # noqa: E402
    import ukey_pb2 as ukey  # noqa: E402
    import securegcm_pb2 as securegcm  # noqa: E402
    import securemessage_pb2 as securemessage  # noqa: E402
    import device_to_device_messages_pb2 as d2d  # noqa: E402
except ImportError as e:  # an older protobuf runtime than the generated code needs
    raise ImportError(f"Quick Share's message formats couldn't load ({e}). "
                      "Update python-protobuf, or run tools/gen-quickshare-protos.sh.") from e

__all__ = ["offline", "sharing_pb", "sharing_enums", "ukey", "securegcm", "securemessage", "d2d"]
