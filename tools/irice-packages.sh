#!/usr/bin/env bash
# Which of iRice's packages (tools/packages.sh) this machine is missing, and
# which ones iRice dropped are still installed. Read-only: installing and
# removing is done by the shell (sysupdate.py's root part, after iRice's
# password dialog), from Settings → Software Update and before an iRice update.
#
#   irice-packages.sh status [git ref]
#     prints one JSON line: {"missing":[…],"unavailable":[…],"dropped":[…],
#     "reasons":{"pkg":"why"}} (why: the comment above its group in packages.sh)
#     With a ref (e.g. origin/main), the list is the one in that version, so
#     an update's new packages can be installed before it's applied.
#   missing      in PACKAGES, not installed (nor provided by something
#                installed), and in the repositories
#   unavailable  in PACKAGES, not installed, not in the repositories (left
#                out, like install.sh does)
#   dropped      in DROPPED and still installed
set -uo pipefail
ROOT="$(cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/.." && pwd)"
cmd="${1:-status}" ref="${2:-}"
[[ "$cmd" == status ]] || { echo "usage: irice-packages.sh status [git ref]" >&2; exit 2; }
command -v pacman >/dev/null || { echo '{"missing":[],"unavailable":[],"dropped":[],"error":"pacman isn'"'"'t installed"}'; exit 0; }

list="$(mktemp)"
trap 'rm -f "$list"' EXIT
if [[ -n "$ref" ]] && git -C "$ROOT" show "$ref:tools/packages.sh" >"$list" 2>/dev/null; then :; else cp "$ROOT/tools/packages.sh" "$list"; fi
# The lists are read as text, never run: the file may come from a version
# that isn't installed yet (an update's), and running it would execute
# whatever it says before you've agreed to anything. A NAME=( … ) block, one
# or more names per line, # comments; only well-formed package names (a
# letter, digit or @_+ first, so nothing can pass as a pacman option).
names() { # array-name file
    awk -v want="$1" '
        !on && $0 ~ "^[[:space:]]*" want "=\\(" {
            on = 1
            sub("^[[:space:]]*" want "=\\(", "")
        }
        on {
            line = $0
            sub(/#.*/, "", line)
            done = (line ~ /\)/)
            sub(/\).*/, "", line)
            n = split(line, f, /[[:space:]]+/)
            for (i = 1; i <= n; i++) if (f[i] != "") print f[i]
            if (done) on = 0
        }' "$2" | grep -E '^[a-z0-9@_+][a-z0-9@._+-]*$'
}
mapfile -t PACKAGES < <(names PACKAGES "$list")
mapfile -t DROPPED < <(names DROPPED "$list")

json() { # name values...
    local out="" p
    for p in "${@:2}"; do out+="${out:+,}\"$p\""; done
    printf '"%s":[%s]' "$1" "$out"
}

# pacman -T: the ones not satisfied by anything installed (names or provides)
mapfile -t notmet < <([[ ${#PACKAGES[@]} -gt 0 ]] && pacman -T -- "${PACKAGES[@]}" 2>/dev/null)
missing=() unavailable=()
for p in "${notmet[@]}"; do
    [[ -z "$p" ]] && continue
    if pacman -Si -- "$p" >/dev/null 2>&1 || pacman -Sg -- "$p" >/dev/null 2>&1; then missing+=("$p"); else unavailable+=("$p"); fi
done
dropped=()
for p in "${DROPPED[@]}"; do
    pacman -Qq -- "$p" >/dev/null 2>&1 && dropped+=("$p")
done
# Why each missing one is needed: the "# comment" line above its group
reasons=""
if [[ ${#missing[@]} -gt 0 ]]; then
    want=" ${missing[*]} "
    while IFS=$'\t' read -r p why; do
        [[ "$want" == *" $p "* ]] && reasons+="${reasons:+,}\"$p\":\"${why//\"/\\\"}\""
    done < <(awk '/^PACKAGES=\(/ { on = 1; next } on && /^\)/ { on = 0 }
        on && /^[[:space:]]*#/ { c = $0; sub(/^[[:space:]]*#[[:space:]]*/, "", c); next }
        on { for (i = 1; i <= NF; i++) { if ($i ~ /^#/) break; printf "%s\t%s\n", $i, c } }' "$list")
fi
printf '{%s,%s,%s,"reasons":{%s}}\n' "$(json missing "${missing[@]}")" "$(json unavailable "${unavailable[@]}")" "$(json dropped "${dropped[@]}")" "$reasons"
