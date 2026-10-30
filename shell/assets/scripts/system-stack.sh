#!/usr/bin/env bash
# What this computer uses for networking, energy modes and Bluetooth, for
# services/Backends.qml: one key=value per line. Read-only and quick (a few
# busctl calls); systemd is asked over D-Bus, which also works from the dev
# sandbox's container, where systemctl can't reach it.

acquired=$(busctl --system list --acquired --no-legend --no-pager 2>/dev/null | awk '{ print $1 }')
activatable=$(busctl --system list --activatable --no-legend --no-pager 2>/dev/null | awk '{ print $1 }')
owned() { grep -qxF "$1" <<<"$acquired"; }
startable() { grep -qxF "$1" <<<"$activatable"; }

# systemd's object path for a unit: everything but [A-Za-z0-9] as _xx
esc() {
    local s="$1" o="" c i
    for ((i = 0; i < ${#s}; i++)); do
        c="${s:i:1}"
        if [[ "$c" =~ [A-Za-z0-9] ]]; then o+="$c"; else o+=$(printf '_%02x' "'$c"); fi
    done
    printf '%s' "$o"
}
# "<ActiveState> <LoadState>" of a system unit, e.g. "active loaded",
# "inactive not-found", "inactive masked"
unit() {
    local out
    out=$(busctl --system get-property org.freedesktop.systemd1 "/org/freedesktop/systemd1/unit/$(esc "$1")" \
        org.freedesktop.systemd1.Unit ActiveState LoadState 2>/dev/null | awk '{ gsub(/"/, "", $2); print $2 }' | paste -sd' ')
    if [[ -z "$out" ]] && command -v systemctl >/dev/null; then
        out="$(systemctl is-active "$1" 2>/dev/null | head -1) $(systemctl show -p LoadState --value "$1" 2>/dev/null)"
    fi
    printf '%s' "${out:-unknown unknown}"
}
active() { [[ "$(unit "$1")" == "active "* ]]; }
installed() { local s; s=$(unit "$1"); [[ "$s" != *" not-found" && "$s" != *" unknown" ]]; }
first_cmd() { for c in "$@"; do command -v "$c" >/dev/null 2>&1 && { echo "$c"; return; }; done; }

# ── Network ─────────────────────────────────────────────────────────────
net=none
if owned org.freedesktop.NetworkManager; then net=networkmanager
elif owned net.connman; then net=connman
elif owned net.connman.iwd; then net=iwd
elif owned fi.w1.wpa_supplicant1; then net=wpa_supplicant
elif owned org.freedesktop.network1; then net=systemd-networkd
fi
echo "net=$net"
installed NetworkManager.service && echo "nm_installed=1"
# An app for the Wi-Fi of whatever manages it (a terminal one runs in a terminal)
app=""; term=0
case "$net" in
    iwd) app=$(first_cmd iwgtk iwgui); [[ -z "$app" ]] && { app=$(first_cmd impala iwctl); term=1; } ;;
    connman) app=$(first_cmd cmst connman-gtk); [[ -z "$app" ]] && { app=$(first_cmd connmanctl); term=1; } ;;
    wpa_supplicant | systemd-networkd) app=$(first_cmd wpa_gui); [[ -z "$app" ]] && { app=$(first_cmd impala iwctl wpa_cli); term=1; } ;;
esac
[[ -z "$app" ]] && term=0
echo "wifi_app=$app"
echo "wifi_app_terminal=$term"
for d in /sys/class/net/*; do
    [[ -d "$d/wireless" || -e "$d/phy80211" ]] || continue
    echo "wifi_iface=${d##*/}"
    [[ "$(cat "$d/operstate" 2>/dev/null)" == up ]] && echo "wifi_up=1"
    break
done

# ── Energy modes ────────────────────────────────────────────────────────
powerd=""
if active tlp.service; then powerd=tlp
elif active auto-cpufreq.service; then powerd=auto-cpufreq
elif owned com.system76.PowerDaemon; then powerd=system76-power
elif owned com.redhat.tuned || active tuned.service; then powerd=tuned
fi
echo "powerd=$powerd"
ppd=0
if owned net.hadess.PowerProfiles || owned org.freedesktop.UPower.PowerProfiles; then
    ppd=1
elif [[ -z "$powerd" ]] && { startable net.hadess.PowerProfiles || startable org.freedesktop.UPower.PowerProfiles; } &&
    [[ "$(unit power-profiles-daemon.service)" != *" masked" ]]; then
    ppd=1 # stopped but D-Bus starts it on first use
fi
echo "ppd=$ppd"
installed power-profiles-daemon.service && echo "ppd_installed=1"

# ── Bluetooth ───────────────────────────────────────────────────────────
owned org.bluez && echo "bt_running=1"
if installed bluetooth.service || command -v bluetoothd >/dev/null 2>&1 || [[ -x /usr/lib/bluetooth/bluetoothd ]]; then
    echo "bt_installed=1"
fi
ls /sys/class/bluetooth 2>/dev/null | grep -q '^hci' && echo "bt_hardware=1"
for r in /sys/class/rfkill/rfkill*; do
    [[ "$(cat "$r/type" 2>/dev/null)" == bluetooth ]] || continue
    echo "bt_hardware=1"
    [[ "$(cat "$r/soft" 2>/dev/null)" == 1 ]] && echo "bt_soft=1"
    [[ "$(cat "$r/hard" 2>/dev/null)" == 1 ]] && echo "bt_hard=1"
done
exit 0
