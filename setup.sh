#!/usr/bin/env bash
# iRice one-line setup, for a fresh or existing Arch Linux:
#
#   bash <(curl -fsSL https://raw.githubusercontent.com/monish-cosol/iRice/main/setup.sh)
#
# Downloads iRice to ~/.local/share/irice/repo (or updates it) and runs its
# installer, which installs everything iRice needs, builds it, sets it up and
# checks the result. Options are passed on to the installer (see
# ./install.sh --help), e.g. --yes for no questions.
set -euo pipefail

REPO="${IRICE_REPO:-https://github.com/monish-cosol/iRice.git}"
BRANCH="${IRICE_BRANCH:-main}"
DIR="${XDG_DATA_HOME:-$HOME/.local/share}/irice/repo"

bold() { printf '\033[1m%s\033[0m\n' "$*"; }
die() { printf '\033[1;31m%s\033[0m\n' "$*" >&2; exit 1; }

[[ $EUID == 0 ]] && die "Run this as your normal user (not root); it asks for sudo when it needs it."
command -v pacman >/dev/null || die "iRice installs on Arch Linux (and Arch-based systems with pacman)."

bold "iRice setup"
if ! command -v git >/dev/null; then
    echo "Installing git first…"
    sudo pacman -S --needed --noconfirm git
fi

if [[ -d "$DIR/.git" ]]; then
    echo "Updating iRice in $DIR"
    git -C "$DIR" fetch --depth 1 origin "$BRANCH"
    git -C "$DIR" reset --hard "origin/$BRANCH"
else
    echo "Downloading iRice to $DIR"
    mkdir -p "$(dirname "$DIR")"
    git clone --depth 1 --branch "$BRANCH" "$REPO" "$DIR"
fi

# The installer asks questions; give it the terminal even when this script
# itself came through a pipe
if [[ -t 0 ]]; then
    exec "$DIR/install.sh" "$@"
else
    exec "$DIR/install.sh" "$@" </dev/tty
fi
