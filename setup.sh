#!/usr/bin/env bash
# Tessera one-line setup, for a fresh or existing Arch Linux:
#
#   bash <(curl -fsSL https://raw.githubusercontent.com/monishbl/tessera/main/setup.sh)
#
# or, when GitHub can't be reached, the same script from the GitLab mirror:
#
#   bash <(curl -fsSL https://gitlab.com/monishbl/tessera/-/raw/main/setup.sh)
#
# Downloads Tessera to ~/.local/share/tessera/repo (or updates it) and runs its
# installer, which installs everything Tessera needs, builds it, sets it up and
# checks the result. Options are passed on to the installer (see
# ./install.sh --help), e.g. --yes for no questions.
set -euo pipefail

# Where Tessera comes from, tried in order: GitHub, its GitLab mirror, then
# GitHub's tarball of the branch (no git history: a last resort). The same
# list as tools/sources.txt (this runs before that's downloaded);
# TESSERA_MIRRORS replaces it, TESSERA_REPO goes first.
SOURCES=(https://github.com/monishbl/tessera.git
         https://gitlab.com/monishbl/tessera.git
         "https://codeload.github.com/monishbl/tessera/tar.gz/refs/heads/{branch}")
[[ -n "${TESSERA_MIRRORS:-}" ]] && read -r -a SOURCES <<<"${TESSERA_MIRRORS//,/ }"
REPO="${TESSERA_REPO:-}"
[[ -n "$REPO" ]] && SOURCES=("$REPO" "${SOURCES[@]}")
BRANCH="${TESSERA_BRANCH:-main}"
DIR="${XDG_DATA_HOME:-$HOME/.local/share}/tessera/repo"

bold() { printf '\033[1m%s\033[0m\n' "$*"; }
die() { printf '\033[1;31m%s\033[0m\n' "$*" >&2; exit 1; }

# git never asks for a password here: a mirror that doesn't exist yet
# fails at once and the next one is tried
export GIT_TERMINAL_PROMPT=0 GIT_ASKPASS=true SSH_ASKPASS=true GCM_INTERACTIVE=never
# A quick look (5 s to connect) before a long git command: false when an
# https source doesn't answer or its repository isn't there
answers() {
    [[ $1 == https://* ]] || return 0
    command -v curl >/dev/null || return 0
    local u=${1%/}
    [[ $u == *.git ]] || u="$u.git"
    [[ "$(curl -s -L --proto =https --connect-timeout 5 --max-time 10 -o /dev/null -w '%{http_code}' "$u/info/refs?service=git-upload-pack")" == 200 ]]
}
git_sources() { local s; for s in "${SOURCES[@]}"; do [[ $s == *"{branch}"* ]] || echo "$s"; done; }
# The branch into $1 (a new folder) from any source; prints the source used
download() {
    local new=$1 src url
    for src in "${SOURCES[@]}"; do
        rm -rf "$new"
        if [[ $src == *"{branch}"* ]]; then
            # The last resort: the branch's files without git history
            url=${src//"{branch}"/$BRANCH}
            mkdir -p "$new"
            if curl -fsSL --proto =https --connect-timeout 10 --retry 2 "$url" | tar -xz --strip-components=1 -C "$new"; then
                echo "  Downloaded without git history: Software Update can't update this copy until you run this setup again." >&2
                echo "$src"
                return 0
            fi
        elif answers "$src" && git clone --depth 1 --branch "$BRANCH" "$src" "$new" >&2; then
            # (updates look at the first source, then the mirrors, whichever this came from)
            git -C "$new" remote set-url origin "$(git_sources | head -1)"
            echo "$src"
            return 0
        fi
        echo "  $src didn't work; trying the next source" >&2
    done
    rm -rf "$new"
    return 1
}
# The branch from origin, else from each source by URL (origin stays as it is)
update() {
    local src here
    here=$(git -C "$DIR" remote get-url origin 2>/dev/null || true)
    for src in origin $(git_sources); do
        if [[ $src != origin ]]; then
            [[ "$src" == "$here" ]] && continue
            answers "$src" || { echo "  $src didn't answer; trying the next source"; continue; }
        fi
        git -C "$DIR" fetch --depth 1 "$src" "+refs/heads/$BRANCH:refs/remotes/origin/$BRANCH" && { echo "  (from $src)"; return 0; }
        echo "  Couldn't download from $src; trying the next source"
    done
    return 1
}

[[ $EUID == 0 ]] && die "Run this as your normal user (not root); it asks for sudo when it needs it."
command -v pacman >/dev/null || die "Tessera installs on Arch Linux (and Arch-based systems with pacman)."

bold "Tessera setup"
if ! command -v git >/dev/null; then
    echo "Installing git first…"
    sudo pacman -S --needed --noconfirm git
fi

if [[ -d "$DIR/.git" ]]; then
    echo "Updating Tessera in $DIR"
    update || die "Couldn't download Tessera from any source (${SOURCES[*]}). Check the connection and run this again."
    git -C "$DIR" reset --hard "origin/$BRANCH"
else
    echo "Downloading Tessera to $DIR"
    mkdir -p "$(dirname "$DIR")"
    got=$(download "$DIR.incoming") || die "Couldn't download Tessera from any source (${SOURCES[*]}). Check the connection and run this again."
    [[ "$got" == "${SOURCES[0]}" ]] || echo "  (from $got)"
    # (a copy from the tarball, from before, makes way for this one)
    rm -rf "$DIR"
    mv "$DIR.incoming" "$DIR"
fi

# The installer asks questions; give it the terminal even when this script
# itself came through a pipe
if [[ -t 0 ]]; then
    exec "$DIR/install.sh" "$@"
else
    exec "$DIR/install.sh" "$@" </dev/tty
fi
