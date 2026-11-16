#!/bin/bash
# Installs the release publisher on the user's laptop from this checkout:
# ~/tessera-verify/publish-release.sh (and .py) and its systemd user timer.
#   tools/release-builder/install-publisher.sh             install or update, timer on
#   tools/release-builder/install-publisher.sh --no-timer  the script only
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
dir="$HOME/tessera-verify"
install -Dm755 "$here/publish-release" "$dir/publish-release.py"
cat > "$dir/publish-release.sh.tmp" <<'EOF'
#!/bin/bash
# Tessera's release publisher (tools/release-builder/publish-release in the repo)
exec python3 "$(dirname "$(readlink -f "$0")")/publish-release.py" "$@"
EOF
chmod 755 "$dir/publish-release.sh.tmp"
mv "$dir/publish-release.sh.tmp" "$dir/publish-release.sh"
[[ "${1:-}" == --no-timer ]] && exit 0
units="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
install -Dm644 "$here/tessera-publish.service" "$units/tessera-publish.service"
install -Dm644 "$here/tessera-publish.timer" "$units/tessera-publish.timer"
systemctl --user daemon-reload
systemctl --user enable --now tessera-publish.timer
systemctl --user list-timers tessera-publish.timer --no-pager
