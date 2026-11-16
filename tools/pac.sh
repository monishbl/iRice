# shellcheck shell=bash
# pacman with second chances, shared by install.sh and tools/setup-tty.sh
# (source it; it defines pac). Downloads fail for reasons that aren't yours:
# a mirror that's behind (its package list names files it doesn't have yet,
# or has a package but not its .sig: CachyOS's mirrors, 16 Nov), a stalled
# download, an old signing keyring.
#
#   pac -S|-Syu [pacman options…] packages…
#
#  1. as asked
#  2. a download failed: the same, without the mirrors that failed (a copy of
#     pacman.conf and its mirror lists in a temporary folder, those servers
#     left out; nothing in /etc changes), the package lists refreshed and the
#     system brought up to them (-Syyu: never a partial upgrade)
#  3. still failing: the mirrors re-ranked (CachyOS: cachyos-rate-mirrors,
#     which does Arch's list and its own; else reflector), the old lists kept
#     as *.tessera-backup, and once more
#  A signature from an unknown key: archlinux-keyring (and cachyos-keyring
#  where there is one) updated, then again. A locked database: says so.
#
# Uses, when the caller has them: DRY (1: only say), warn, say, DIM, R.

_pac_warn() { if declare -F warn >/dev/null; then warn "$@"; else printf '  ! %s\n' "$*"; fi; }
_pac_say() { if declare -F say >/dev/null; then say "$@"; else printf '%s\n' "$*"; fi; }

# The mirrors a pacman run's output says failed (hosts)
_pac_bad_hosts() {
    grep -oiE "failed retrieving file '[^']+' from [^ ]+|too many errors from [^ ,]+" "$1" |
        sed -E 's/.* from //' | sort -u
}

# A pacman.conf in $2 whose mirror lists leave out the hosts in $1 (one a line)
_pac_conf_without() {
    local hosts=$1 dir=$2 inc f
    cp /etc/pacman.conf "$dir/pacman.conf" || return 1
    while read -r inc; do
        [[ -f "$inc" ]] || continue
        f="$dir/$(basename "$inc")"
        # (that host exactly: mirror.example.org, not mirror5.example.org)
        grep -vF -f <(sed 's#.*#//&/#' "$hosts") "$inc" >"$f"
        # (a list that had only bad mirrors keeps them: no list, no repo)
        grep -qE '^\s*Server' "$f" || cp "$inc" "$f"
        sed -i "s#^\(\s*Include\s*=\s*\)$inc\s*\$#\1$f#" "$dir/pacman.conf"
    done < <(sed -nE 's/^\s*Include\s*=\s*(\S+).*/\1/p' /etc/pacman.conf | sort -u)
}

_pac_failed_download() { grep -qiE "failed retrieving file|failed to retrieve some files|error 404|Operation too slow|Could not resolve host|Connection timed out|Connection reset" "$1"; }

pac() {
    if [[ ${DRY:-0} == 1 ]]; then printf '  %swould run:%s sudo pacman %s\n' "${DIM:-}" "${R:-}" "$*"; return 0; fi
    local out rc tmp rest=() a
    out=$(mktemp)
    for a in "$@"; do [[ $a == -S* ]] || rest+=("$a"); done
    sudo pacman "$@" 2>&1 | tee "$out"; rc=${PIPESTATUS[0]}

    if ((rc != 0)) && _pac_failed_download "$out"; then
        tmp=$(mktemp -d)
        _pac_bad_hosts "$out" >"$tmp/hosts"
        if [[ -s "$tmp/hosts" ]] && _pac_conf_without "$tmp/hosts" "$tmp"; then
            _pac_warn "Some packages didn't download from $(paste -sd, "$tmp/hosts" | sed 's/,/, /g') (a mirror that's behind): trying again without it…"
            sudo pacman --config "$tmp/pacman.conf" -Syyu "${rest[@]}" 2>&1 | tee "$out"; rc=${PIPESTATUS[0]}
        else
            _pac_warn "Some packages didn't download (a mirror that's behind, or a stalled download): refreshing the package lists and trying again…"
            sudo pacman -Syyu "${rest[@]}" 2>&1 | tee "$out"; rc=${PIPESTATUS[0]}
        fi
        rm -rf "$tmp"
        if ((rc != 0)) && _pac_failed_download "$out"; then
            local ranked=0 l
            for l in /etc/pacman.d/mirrorlist /etc/pacman.d/cachyos*-mirrorlist; do
                [[ -f $l ]] && sudo cp -n "$l" "$l.tessera-backup" 2>/dev/null
            done
            if command -v cachyos-rate-mirrors >/dev/null; then
                _pac_warn "Still failing: ranking the mirrors again (cachyos-rate-mirrors; your lists are kept as *.tessera-backup)…"
                sudo cachyos-rate-mirrors >/dev/null 2>&1 && ranked=1
            elif command -v reflector >/dev/null; then
                _pac_warn "Still failing: switching to the fastest up-to-date mirrors (your list is kept as mirrorlist.tessera-backup)…"
                sudo reflector --latest 20 --protocol https --sort rate --age 12 --save /etc/pacman.d/mirrorlist && ranked=1
            fi
            if ((ranked)); then
                sudo pacman -Syyu "${rest[@]}" 2>&1 | tee "$out"; rc=${PIPESTATUS[0]}
            fi
            if ((rc != 0)); then
                _pac_say "  The mirrors are out of date or unreachable. Pick fresh ones, then run this again:"
                if command -v cachyos-rate-mirrors >/dev/null; then
                    _pac_say "    sudo cachyos-rate-mirrors"
                else
                    _pac_say "    sudo pacman -S reflector && sudo reflector --latest 20 --protocol https --sort rate --save /etc/pacman.d/mirrorlist"
                fi
            fi
        fi
    elif ((rc != 0)) && grep -qiE "unable to lock database" "$out"; then
        _pac_say "  Another package manager is running (pacman's database is locked). Let it finish, then try again."
    elif ((rc != 0)) && grep -qiE "invalid or corrupted package|signature .* is (unknown trust|invalid)|key .* could not be looked up|unknown public key" "$out"; then
        _pac_warn "A package's signature didn't check out (often an old keyring): updating the keyring and trying again…"
        local keys=(archlinux-keyring)
        pacman -Si cachyos-keyring >/dev/null 2>&1 && keys+=(cachyos-keyring)
        sudo pacman -Sy --needed --noconfirm "${keys[@]}" && { sudo pacman "$@" 2>&1 | tee "$out"; rc=${PIPESTATUS[0]}; }
    fi
    rm -f "$out"
    return "$rc"
}
