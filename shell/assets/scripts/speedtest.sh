#!/usr/bin/env bash
# Internet speed test for services/SpeedTest.qml. Prints one event per line:
#   phase latency|download|upload
#   meta <json>             Cloudflare's view of you: IP, ISP, test server
#   ping <ms>               one idle latency sample
#   live <bits/s>           the speed right now (download or upload phase)
#   loaded <ms>             a latency sample while the line is busy
#   done download|upload <bits/s> <bytes>
#   ookla <json>            Ookla's own progress and result lines, passed on
#   error <code> <message>  offline | curl | ookla | timeout
# Usage: speedtest.sh cloudflare | ookla | ookla-py
# Each phase stops after about 9 s or a data cap, so a test stays well under
# 400 MB even on gigabit.
set -uo pipefail
backend="${1:-cloudflare}"
CF="https://speed.cloudflare.com"
PHASE_SECS=${SPEEDTEST_SECS:-9}
DOWN_CAP=${SPEEDTEST_DOWN_CAP:-$((250 * 1000 * 1000))}
UP_CAP=${SPEEDTEST_UP_CAP:-$((120 * 1000 * 1000))}
STREAMS=4

kids=()
cleanup() { for p in "${kids[@]}"; do kill "$p" 2>/dev/null; done; kill $(jobs -p) 2>/dev/null; }
trap 'cleanup; exit 0' TERM INT
trap cleanup EXIT

now_ms() { echo $(( $(date +%s%N) / 1000000 )); }

case "$backend" in
ookla)
    command -v speedtest >/dev/null || { echo "error ookla speedtest isn't installed"; exit 1; }
    speedtest --format=jsonl --progress=yes --accept-license --accept-gdpr 2>/dev/null | while IFS= read -r l; do echo "ookla $l"; done
    exit 0 ;;
ookla-py)
    command -v speedtest-cli >/dev/null || { echo "error ookla speedtest-cli isn't installed"; exit 1; }
    echo "phase download"
    out=$(speedtest-cli --json --secure 2>&1) || { echo "error ookla ${out//$'\n'/ }"; exit 1; }
    echo "ookla {\"type\":\"py\",\"data\":$out}"
    exit 0 ;;
esac

command -v curl >/dev/null || { echo "error curl curl isn't installed"; exit 1; }

# The interface the internet goes through, for its byte counters
dev=$(ip route get 1.1.1.1 2>/dev/null | awk '{ for (i = 1; i < NF; i++) if ($i == "dev") { print $(i + 1); exit } }')
count() { cat "/sys/class/net/$dev/statistics/$1" 2>/dev/null || echo 0; }

# One TCP handshake to the test server: the round trip, without the server's
# own time or TLS in it
rtt() {
    curl -s -o /dev/null --connect-timeout 3 -m 4 -w '%{time_namelookup} %{time_connect}\n' "$CF/__down?bytes=0" |
        awk '$2 > 0 { printf "%.1f\n", ($2 - $1) * 1000 }'
}

echo "phase latency"
meta=$(curl -s -m 6 -H "Referer: $CF/" "$CF/meta") || meta=""
if [[ "$meta" != \{* ]]; then
    curl -s -o /dev/null -m 5 "$CF/__down?bytes=0" && echo "error timeout The test server didn't answer" || echo "error offline Can't reach the internet"
    exit 1
fi
echo "meta $meta"
for _ in $(seq 1 16); do
    r=$(rtt)
    [[ -n "$r" ]] && echo "ping $r"
done

# A busy phase: STREAMS parallel transfers on repeat, the speed read from the
# interface's counters every 250 ms, a latency sample every ~400 ms
phase() { # download|upload
    local kind=$1 counter cap start end t0 b0 tw bw last lastt
    [[ $kind == download ]] && { counter=rx_bytes; cap=$DOWN_CAP; } || { counter=tx_bytes; cap=$UP_CAP; }
    echo "phase $kind"
    local stop; stop=$(mktemp -u)
    for _ in $(seq 1 $STREAMS); do
        (
            c=0
            trap 'kill $c 2>/dev/null; exit 0' TERM
            while [[ ! -e $stop ]]; do
                if [[ $kind == download ]]; then
                    curl -s -o /dev/null -m 12 "$CF/__down?bytes=25000000" &
                else
                    head -c 10000000 /dev/zero | curl -s -o /dev/null -m 12 -H 'Content-Type: application/octet-stream' --data-binary @- "$CF/__up" &
                fi
                c=$!
                wait $c || sleep 0.3
            done
        ) &
        kids+=($!)
    done
    (
        while [[ ! -e $stop ]]; do
            r=$(rtt)
            [[ -n "$r" ]] && echo "loaded $r"
            sleep 0.4
        done
    ) &
    kids+=($!)
    start=$(now_ms); b0=$(count $counter); last=$b0; lastt=$start
    tw=0; bw=0 # where the measured window starts: after a 1 s ramp-up
    while true; do
        sleep 0.25
        local t b; t=$(now_ms); b=$(count $counter)
        (( t > lastt )) && echo "live $(( (b - last) * 8000 / (t - lastt) ))"
        last=$b; lastt=$t
        (( tw == 0 && t - start >= 1000 )) && { tw=$t; bw=$b; }
        (( t - start >= PHASE_SECS * 1000 || b - b0 >= cap )) && break
    done
    touch "$stop"
    for p in "${kids[@]}"; do kill "$p" 2>/dev/null; done # (each stops its own transfer)
    wait 2>/dev/null
    kids=()
    rm -f "$stop"
    end=$(now_ms)
    (( tw == 0 || lastt - tw < 500 )) && { tw=$start; bw=$b0; } # (stopped by the cap that early: count the ramp-up too)
    if (( end > tw && last > bw )); then
        echo "done $kind $(( (last - bw) * 8000 / (lastt - tw) )) $(( last - b0 ))"
    else
        echo "error timeout The $kind didn't start"
        exit 1
    fi
}
phase download
phase upload
