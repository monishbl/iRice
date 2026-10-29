#!/usr/bin/env bash
# Regenerates the Python modules for iRice's Quick Share engine from the
# vendored Google .proto files (shell/assets/quickshare/proto, see its NOTICE).
# The generated code is checked in, so this is only needed after updating the
# .proto files or when the installed protobuf runtime is older than the one
# they were generated for (the engine then says so and names this script).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="$ROOT/shell/assets/quickshare/proto"
OUT="$ROOT/shell/assets/scripts/quickshare/nearby/pb"
command -v protoc >/dev/null || { echo "protoc is missing: install protobuf (pacman -S protobuf)" >&2; exit 1; }
rm -rf "$OUT"
mkdir -p "$OUT"
# Nearby Connections and Sharing keep Google's directory layout (their
# imports use it); UKEY2's files import each other by bare name
(cd "$SRC/nearby" && protoc -I . --python_out="$OUT" \
    connections/implementation/proto/offline_wire_formats.proto \
    proto/sharing_enums.proto sharing/proto/wire_format.proto)
(cd "$SRC/ukey2" && protoc -I . --python_out="$OUT" \
    ukey.proto securegcm.proto securemessage.proto device_to_device_messages.proto)
# Packages, so the generated imports (from proto import …) resolve
find "$OUT" -type d -exec touch {}/__init__.py \;
echo "Generated into $OUT"
