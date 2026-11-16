#!/usr/bin/env bash
# tools/pac.sh's mirror-list check with made-up lists and mirrors: no sudo,
# no pacman, no network. The probes, the clock, sudo, reflector and
# cachyos-rate-mirrors are functions here. Run by tests/test_mirrors.py (or
# on its own: bash tests/pac_mirrors.test.sh).
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
export TESSERA_PACMAN_D="$T/pacman.d" TESSERA_PACMAN_CONF="$T/pacman.conf" TESSERA_OS_RELEASE="$T/os-release"
mkdir -p "$TESSERA_PACMAN_D"
# shellcheck source=../tools/pac.sh
source "$ROOT/tools/pac.sh"

fails=0
check() { # name expected actual
    if [[ "$2" == "$3" ]]; then echo "ok   $1"; else echo "FAIL $1: expected [$2], got [$3]"; fails=$((fails + 1)); fi
}
has() { # name needle haystack
    if [[ "$3" == *"$2"* ]]; then echo "ok   $1"; else echo "FAIL $1: [$2] not in [$3]"; fails=$((fails + 1)); fi
}

NOW=2000000000
_pac_now() { echo "$NOW"; }
# Each fake mirror's last sync: SYNC[host]; none: it doesn't answer
declare -A SYNC
REF=$NOW
_pac_probe() { # kind list server
    local host=${3#https://}
    host=${host%%/*}
    [[ -n "${SYNC[$host]:-}" ]] && echo "${SYNC[$host]}"
    return 0
}
_pac_reference() { [[ -n $REF ]] && echo "$REF"; return 0; }
CALLS="$T/calls"
sudo() { echo "sudo $*" >>"$CALLS"; "$@"; }
reflector() { echo "reflector $*" >>"$CALLS"; printf 'Server = https://fresh.example/$repo/os/$arch\n' >"${@: -1}"; }

arch_list() { # hosts…
    local h
    for h in "$@"; do echo "Server = https://$h/archlinux/\$repo/os/\$arch"; done >"$TESSERA_PACMAN_D/mirrorlist"
}
conf() { # lists…
    local l
    for l in "$@"; do printf '[repo-%s]\nInclude = /etc/pacman.d/%s\n' "$l" "$l"; done >"$TESSERA_PACMAN_CONF"
}
stale() { pac_mirrors_stale 2>"$T/notes" | cut -f1 | xargs -rn1 basename | paste -sd, -; }
fresh_hosts() { local h; for h in "$@"; do SYNC[$h]=$((NOW - 600)); done; }

echo 'ID=arch' >"$TESSERA_OS_RELEASE"
conf mirrorlist

# 1. Five fresh mirrors: left alone
fresh_hosts a b c d e
arch_list a b c d e
check "fresh list kept" "" "$(stale)"
has "fresh list said so" "up to date" "$(cat "$T/notes")"

# 2. Fewer than 5 servers: out of date
arch_list a b c
check "too few servers" "mirrorlist" "$(stale)"
has "too few: why" "only 3 mirrors" "$(pac_mirrors_stale 2>/dev/null)"

# 3. The first one 2 days behind the reference: out of date
SYNC[old]=$((NOW - 2 * 86400))
arch_list old a b c d
check "first behind" "mirrorlist" "$(stale)"

# 4. The first fresh, one of the next two dead: still fine
arch_list a dead b c d
check "one of three dead is fine" "" "$(stale)"

# 5. Two of the first three dead: out of date
arch_list a dead gone c d
check "two of three dead" "mirrorlist" "$(stale)"

# 6. No reference: Arch's lastsync more than a day old counts
REF=""
arch_list old a b c d
check "old without reference" "mirrorlist" "$(stale)"
arch_list a b c d e
check "fresh without reference" "" "$(stale)"

# 7. Offline: nothing answers, not even the reference: nothing decided
arch_list dead gone none nil zip
check "offline: left alone" "" "$(stale)"
has "offline: said so" "offline" "$(cat "$T/notes")"
REF=$NOW

# 8. Manjaro: left alone, with a note
echo 'ID=manjaro' >"$TESSERA_OS_RELEASE"
arch_list a b
check "manjaro skipped" "" "$(stale)"
has "manjaro note" "pacman-mirrors" "$(cat "$T/notes")"
echo 'ID=arch' >"$TESSERA_OS_RELEASE"

# 9. A layout it doesn't know ($branch: Manjaro's servers on another ID)
printf 'Server = https://m.example/stable/$repo/$arch\n%.0s' 1 2 3 4 5 >"$TESSERA_PACMAN_D/mirrorlist"
check "unknown layout skipped" "" "$(stale)"
has "unknown layout note" "layout" "$(cat "$T/notes")"

# 10. CachyOS's lists are checked too (those pacman.conf uses)
conf mirrorlist cachyos-v3-mirrorlist cachyos-mirrorlist
arch_list a b c d e
for l in cachyos-v3-mirrorlist cachyos-mirrorlist; do
    for h in cdn1 cdn2 cdn3 cdn4 cdn5; do echo "Server = https://$h/repo/\$arch_v3/\$repo"; done >"$TESSERA_PACMAN_D/$l"
done
fresh_hosts cdn1 cdn2 cdn3 cdn4 cdn5
check "cachyos fresh" "" "$(stale)"
SYNC[cdn1]=$((NOW - 3 * 86400))
check "cachyos first behind" "cachyos-mirrorlist,cachyos-v3-mirrorlist" "$(stale)"
for h in cdn2 cdn3 cdn4 cdn5; do echo "Server = https://$h/repo/\$arch/\$repo"; done >"$TESSERA_PACMAN_D/cachyos-mirrorlist"
check "cachyos list with too few" "cachyos-mirrorlist,cachyos-v3-mirrorlist" "$(stale)"  # (4 servers: too few)
# (a list pacman.conf doesn't use is never looked at)
conf mirrorlist
check "unused cachyos list ignored" "" "$(stale)"

# 11. --plan: says so, changes nothing
arch_list a b c
before=$(cat "$TESSERA_PACMAN_D/mirrorlist")
: >"$CALLS"
out=$(pac_mirrors_check --plan)
has "plan says it would" "Would re-rank" "$out"
check "plan changed nothing" "$before" "$(cat "$TESSERA_PACMAN_D/mirrorlist")"
check "plan ran nothing" "" "$(cat "$CALLS")"
check "plan made no backup" "no" "$([[ -e $TESSERA_PACMAN_D/mirrorlist.tessera-backup ]] && echo yes || echo no)"
out=$(DRY=1 pac_mirrors_check)
has "dry run says it would" "Would re-rank" "$out"

# 12. For real (stubbed): backed up once, reflector writes the new list
out=$(pac_mirrors_check)
has "refresh said so" "Re-ranking the mirrors" "$out"
check "backup is the old list" "$before" "$(cat "$TESSERA_PACMAN_D/mirrorlist.tessera-backup")"
has "reflector ran" "reflector --latest 20 --protocol https --sort rate" "$(cat "$CALLS")"
check "new list" 'Server = https://fresh.example/$repo/os/$arch' "$(cat "$TESSERA_PACMAN_D/mirrorlist")"
arch_list x y
pac_mirrors_check >/dev/null
check "backup kept once" "$before" "$(cat "$TESSERA_PACMAN_D/mirrorlist.tessera-backup")"

# 13. CachyOS: cachyos-rate-mirrors instead of reflector
cachyos-rate-mirrors() { echo "cachyos-rate-mirrors" >>"$CALLS"; }
: >"$CALLS"
arch_list a b
pac_mirrors_check >/dev/null
has "cachyos-rate-mirrors ran" "cachyos-rate-mirrors" "$(cat "$CALLS")"
check "reflector not run" "" "$(grep reflector "$CALLS" | grep -v cachyos)"
unset -f cachyos-rate-mirrors

# 14. A refresh that fails never stops the install
reflector() { return 1; }
arch_list a b
pac_mirrors_check >/dev/null
check "failed refresh is not fatal" "0" "$?"

# 15. The AUR down: Arch's GitHub mirror of it (a branch per package)
git init -q -b main "$T/aur-src"
git -C "$T/aur-src" checkout -q -b yay-bin
echo "pkgname=yay-bin" >"$T/aur-src/PKGBUILD"
git -C "$T/aur-src" -c user.name=t -c user.email=t@t add -A
git -C "$T/aur-src" -c user.name=t -c user.email=t@t commit -qm yay-bin
AUR_URL="file://$T/no-aur" AUR_MIRROR="$T/aur-src"
aur_reachable() { return 1; }
aur_clone yay-bin "$T/out" >/dev/null 2>&1
check "clone from the AUR's GitHub mirror" "pkgname=yay-bin" "$(cat "$T/out/PKGBUILD" 2>/dev/null)"
check "only that package's branch" "yay-bin" "$(git -C "$T/out" branch --show-current 2>/dev/null)"

echo
((fails == 0)) && echo "all passed" || echo "$fails failed"
((fails == 0))
