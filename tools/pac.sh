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

# ── Mirror lists that are out of date, before pacman runs ───────────────
# pac_mirrors_check [--plan]: before the first install or update (install.sh,
# setup-tty.sh; Software Update asks pac_mirrors_stale and its root part does
# the refresh). Each mirror list pacman.conf uses (Arch's mirrorlist, and
# CachyOS's cachyos*-mirrorlist) is out of date when it has fewer than 5
# servers, or its first server, or 2 of its first 3, are behind or don't
# answer: Arch's by their lastsync against Arch's own mirror's (more than 12 h
# behind; 24 h old without it), CachyOS's by their repository database's date
# against mirror.cachyos.org's. The probes run side by side, 3 s each: well
# under 10 s. Out of date: the lists are re-ranked (cachyos-rate-mirrors on
# CachyOS; else the 20 fastest up-to-date HTTPS mirrors by reflector,
# installed first when missing), each old list kept once as
# <list>.tessera-backup. Never a reason to stop: on any failure the lists
# stay as they were. --plan (or DRY=1): says what it would do, changes
# nothing. Manjaro's mirrors (pacman-mirrors, a $branch in each server) are
# left alone, as is any list in a layout this doesn't know, with a note.
# Offline (nothing answers, not even the reference): nothing is decided.
#
# Tests: TESSERA_PACMAN_CONF, TESSERA_PACMAN_D (or TESSERA_SYSTEM_ROOT),
# TESSERA_OS_RELEASE, and the
# functions _pac_probe, _pac_reference, _pac_now, sudo, reflector,
# cachyos-rate-mirrors may be replaced.
PAC_MIRRORS_MIN=5
PAC_BEHIND=$((12 * 3600))
PAC_OLD=$((24 * 3600))

_pac_now() { date +%s; }

# A list's active servers, one a line
_pac_servers() { sed -nE 's/^\s*Server\s*=\s*(\S+).*/\1/p' "$1" 2>/dev/null; }

# arch | cachyos | "" (not one this knows) for a mirror list
_pac_kind() {
    case "$(basename "$1")" in
        cachyos*mirrorlist) echo cachyos ;;
        mirrorlist) _pac_servers "$1" | grep -q '\$repo/os/\$arch' && ! _pac_servers "$1" | grep -q '\$branch' && echo arch ;;
    esac
}

# The repository a CachyOS list serves, and its server URL with the
# variables filled in: cachyos-v3-mirrorlist -> cachyos-v3, x86_64_v3
_pac_cachyos_db() { # list server
    local repo arch
    repo=$(basename "$1" | sed -E 's/-mirrorlist$//')
    arch=x86_64
    [[ $repo == cachyos-v3 ]] && arch=x86_64_v3
    [[ $repo == cachyos-v4 ]] && arch=x86_64_v4
    local u=$2
    u=${u//'$arch_v3'/x86_64_v3}
    u=${u//'$arch_v4'/x86_64_v4}
    u=${u//'$arch'/$arch}
    u=${u//'$repo'/$repo}
    echo "$u/$repo.db"
}

# When a mirror last synced (seconds since 1970), or nothing when it doesn't
# answer: kind list server
_pac_probe() {
    local t
    if [[ $1 == arch ]]; then
        t=$(curl -fsL --connect-timeout 3 --max-time 3 "${3%%/\$repo/*}/lastsync" 2>/dev/null)
        [[ $t =~ ^[0-9]+$ ]] && echo "$t"
    else
        t=$(curl -fsIL --connect-timeout 3 --max-time 3 "$(_pac_cachyos_db "$2" "$3")" 2>/dev/null | sed -nE 's/^last-modified:\s*(.*)\r?$/\1/Ip' | tail -1)
        [[ -n $t ]] && date -d "$t" +%s 2>/dev/null
    fi
}

# The newest sync to compare with: Arch's own mirror, CachyOS's own
_pac_reference() { # kind list
    if [[ $1 == arch ]]; then
        _pac_probe arch "$2" 'https://geo.mirror.pkgbuild.com/$repo/os/$arch'
    else
        local own
        own=$(_pac_servers "$2" | grep -m1 '^https://mirror\.cachyos\.org/')
        [[ -n $own ]] || own='https://mirror.cachyos.org/repo/$arch/$repo'
        _pac_probe cachyos "$2" "$own"
    fi
}

# The mirror lists pacman.conf uses (Arch's and CachyOS's), one a line
_pac_lists() {
    local d=${TESSERA_PACMAN_D:-${TESSERA_SYSTEM_ROOT:-}/etc/pacman.d} conf=${TESSERA_PACMAN_CONF:-${TESSERA_SYSTEM_ROOT:-}/etc/pacman.conf} l
    {
        if [[ -r $conf ]]; then
            sed -nE 's/^\s*Include\s*=\s*(\S+).*/\1/p' "$conf" | sed "s#^/etc/pacman.d#$d#"
        else
            echo "$d/mirrorlist"
        fi
    } | sort -u | while read -r l; do
        case "$(basename "$l")" in mirrorlist | cachyos*mirrorlist) [[ -f $l ]] && echo "$l" ;; esac
    done
}

# Prints why a list is out of date (and returns 0), or why it's left as it
# is (returns 1); "offline" (returns 2) when nothing answered
_pac_list_stale() { # list
    local list=$1 kind n tmp i t ref now bad=0 answered=0 first_bad=0
    kind=$(_pac_kind "$list")
    [[ -n $kind ]] || { echo "not a layout Tessera knows: left as it is"; return 1; }
    n=$(_pac_servers "$list" | wc -l)
    ((n < PAC_MIRRORS_MIN)) && { echo "only $n mirror$( ((n == 1)) || echo s) in it"; return 0; }
    tmp=$(mktemp -d)
    _pac_reference "$kind" "$list" >"$tmp/ref" &
    i=0
    while read -r s; do
        _pac_probe "$kind" "$list" "$s" >"$tmp/$i" &
        i=$((i + 1))
    done < <(_pac_servers "$list" | head -3)
    wait
    ref=$(cat "$tmp/ref")
    now=$(_pac_now)
    for i in 0 1 2; do
        t=$(cat "$tmp/$i" 2>/dev/null)
        if [[ -z $t ]]; then
            bad=$((bad + 1))
            ((i == 0)) && first_bad=1
            continue
        fi
        answered=1
        if [[ -n $ref ]] && ((ref - t > PAC_BEHIND)) || { [[ -z $ref && $kind == arch ]] && ((now - t > PAC_OLD)); }; then
            bad=$((bad + 1))
            ((i == 0)) && first_bad=1
        fi
    done
    rm -rf "$tmp"
    if ((!answered)) && [[ -z $ref ]]; then
        echo "nothing answered (offline?): left as it is"
        return 2
    fi
    if ((first_bad || bad >= 2)); then
        echo "$( ((first_bad)) && echo "its first mirror" || echo "$bad of its first 3 mirrors") $( ((first_bad && bad > 1)) && echo "and others ")out of date or not answering"
        return 0
    fi
    echo "up to date"
    return 1
}

# Manjaro: its own tool (pacman-mirrors) looks after its lists
_pac_manjaro() {
    local f=${TESSERA_OS_RELEASE:-/etc/os-release}
    grep -qiE '^(ID|ID_LIKE)=.*manjaro' "$f" 2>/dev/null
}

# The lists out of date, one a line ("<list>\t<why>"); notes on stderr
pac_mirrors_stale() {
    local l why rc tmp i=0
    if _pac_manjaro; then
        echo "Manjaro's mirrors are looked after by pacman-mirrors: left as they are" >&2
        return 0
    fi
    tmp=$(mktemp -d)
    while read -r l; do
        { why=$(_pac_list_stale "$l"); rc=$?; printf '%s\t%s\t%s\n' "$rc" "$l" "$why" >"$tmp/$i"; } &
        i=$((i + 1))
    done < <(_pac_lists)
    wait
    cat "$tmp"/* 2>/dev/null | while IFS=$'\t' read -r rc l why; do
        if [[ $rc == 0 ]]; then printf '%s\t%s\n' "$l" "$why"; else printf '%s: %s\n' "$l" "$why" >&2; fi
    done
    rm -rf "$tmp"
}

# Re-ranks the mirror lists given (one a line), each kept once as
# <list>.tessera-backup. CachyOS: cachyos-rate-mirrors (it does Arch's list
# and its own); else reflector for Arch's list
pac_mirrors_refresh() {
    local lists=$1 l d=${TESSERA_PACMAN_D:-${TESSERA_SYSTEM_ROOT:-}/etc/pacman.d}
    while read -r l; do
        [[ -n $l ]] && sudo cp -n "$l" "$l.tessera-backup" 2>/dev/null
    done <<<"$lists"
    if command -v cachyos-rate-mirrors >/dev/null; then
        sudo cachyos-rate-mirrors >/dev/null 2>&1 && return 0
    elif grep -q 'cachyos' <<<"$lists" && ! grep -qx "$d/mirrorlist" <<<"$lists"; then
        _pac_say "  CachyOS's mirror lists are out of date; cachyos-rate-mirrors (sudo pacman -S cachyos-rate-mirrors) ranks them."
        return 1
    fi
    grep -qx "$d/mirrorlist" <<<"$lists" || return 1
    if ! command -v reflector >/dev/null; then
        sudo pacman -S --needed --noconfirm reflector >/dev/null 2>&1 || { _pac_say "  reflector couldn't be installed, so the mirror list stays as it is."; return 1; }
    fi
    sudo reflector --latest 20 --protocol https --sort rate --age 12 --save "$d/mirrorlist"
}

pac_mirrors_check() {
    local plan=0 stale notes lists l why
    [[ ${1:-} == --plan || ${DRY:-0} == 1 ]] && plan=1
    notes=$(mktemp)
    stale=$(pac_mirrors_stale 2>"$notes")
    while read -r l; do _pac_say "  Mirrors: $l"; done <"$notes"
    rm -f "$notes"
    [[ -z $stale ]] && return 0
    lists=$(cut -f1 <<<"$stale")
    while IFS=$'\t' read -r l why; do
        _pac_warn "$l: $why"
    done <<<"$stale"
    if ((plan)); then
        _pac_say "  Would re-rank the mirrors first ($(command -v cachyos-rate-mirrors >/dev/null && echo cachyos-rate-mirrors || echo "reflector: the 20 fastest up-to-date HTTPS mirrors"); the old lists kept as *.tessera-backup)."
        return 0
    fi
    _pac_say "  Re-ranking the mirrors first (the old lists are kept as *.tessera-backup)…"
    pac_mirrors_refresh "$lists" || _pac_warn "The mirrors couldn't be re-ranked: going on with the ones there are."
    return 0
}

# ── AUR sources: aur.archlinux.org, else Arch's GitHub mirror of it ─────
# Arch keeps every AUR package's git repository as a branch of
# github.com/archlinux/aur. aur_clone gets a package's PKGBUILD from the AUR,
# or from that mirror when the AUR doesn't answer within seconds;
# aur_build_from_mirror builds and installs packages that way (makepkg -si:
# their dependencies from the repositories) when the AUR is down and yay or
# paru can't work. (Tests: AUR_URL, AUR_MIRROR, aur_reachable may be replaced.)
AUR_URL=${AUR_URL:-https://aur.archlinux.org}
AUR_MIRROR=${AUR_MIRROR:-https://github.com/archlinux/aur.git}

aur_reachable() { curl -fsI --connect-timeout 5 --max-time 8 -o /dev/null "$AUR_URL/" 2>/dev/null; }

aur_clone() { # package dest
    if aur_reachable && GIT_TERMINAL_PROMPT=0 git clone -q --depth 1 "$AUR_URL/$1.git" "$2"; then
        return 0
    fi
    rm -rf "$2"
    _pac_warn "The AUR isn't answering: getting $1 from Arch's GitHub mirror of the AUR instead…"
    GIT_TERMINAL_PROMPT=0 git clone -q --depth 1 --branch "$1" --single-branch "$AUR_MIRROR" "$2"
}

aur_build_from_mirror() { # packages…
    local p tmp rc=0
    for p in "$@"; do
        if [[ ${DRY:-0} == 1 ]]; then
            printf '  %swould run:%s makepkg -si for %s, from %s (branch %s)\n' "${DIM:-}" "${R:-}" "$p" "$AUR_MIRROR" "$p"
            continue
        fi
        tmp=$(mktemp -d)
        if aur_clone "$p" "$tmp/$p"; then
            (cd "$tmp/$p" && makepkg -si --needed --noconfirm) || rc=1
        else
            rc=1
        fi
        rm -rf "$tmp"
    done
    return "$rc"
}

# Run directly: bash tools/pac.sh mirrors-check [--plan] | mirrors-stale
if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    case "${1:-}" in
        mirrors-check) shift; pac_mirrors_check "$@" ;;
        mirrors-stale) pac_mirrors_stale ;;
        aur-clone) shift; aur_clone "$@" ;;
        *) echo "usage: pac.sh mirrors-check [--plan] | mirrors-stale | aur-clone <package> <dest>" >&2; exit 2 ;;
    esac
fi
