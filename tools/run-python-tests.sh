#!/usr/bin/env bash
# All Python tests, each file on its own, in a throwaway home: no test can
# reach a real install (17 Nov: one ran native-install for real and started a
# full release build into server1's own ~/.local/share/tessera/bin).
#   tools/run-python-tests.sh [tests/test_x.py …]
set -uo pipefail
cd "$(dirname "$0")/.."
H=$(mktemp -d "${TMPDIR:-/tmp}/tessera-pytests.XXXXXX")
trap 'rm -rf "$H"' EXIT
mkdir -p "$H/.config" "$H/.local/share" "$H/.local/state" "$H/.cache"
files=("$@"); [[ ${#files[@]} -gt 0 ]] || files=(tests/test_*.py)
fail=0 ran=0
for f in "${files[@]}"; do
    out=$(env HOME="$H" XDG_CONFIG_HOME="$H/.config" XDG_DATA_HOME="$H/.local/share" XDG_STATE_HOME="$H/.local/state" \
        XDG_CACHE_HOME="$H/.cache" TESSERA_NATIVE_BUILDER=/bin/false timeout 900 python3 -m unittest "$f" 2>&1)
    n=$(grep -oE '^Ran [0-9]+' <<<"$out" | grep -oE '[0-9]+'); ran=$((ran + ${n:-0}))
    # (unittest's verdict line; warnings may print after it)
    if grep -q '^FAILED' <<<"$out" || ! grep -qE '^OK( \(.*\))?$' <<<"$out"; then fail=$((fail + 1)); echo "FAIL $f"; grep -E '^(FAIL|ERROR):' <<<"$out" | sed 's/^/  /'; fi
done
[[ -z "$(ls -A "$H/.local/share/tessera/bin" 2>/dev/null)" ]] || echo "note: a test installed into the throwaway home's tessera/bin"
echo "python tests: $ran ran, $fail file(s) failing"
exit $(( fail > 0 ))
