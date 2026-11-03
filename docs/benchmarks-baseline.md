# Baseline: resource use before any Rust rewrite

Measured 3 November 2026 on the real laptop (Intel Alder Lake, 16 GB), Material look, a normal desktop session, the shell up for ~23 minutes. Read-only from /proc (`ps` RSS and lifetime CPU), no screenshots.

| What | Processes | Resident memory | CPU (lifetime average) |
|---|---:|---:|---:|
| The shell (`irice-shell`, Quickshell/QML) | 1 | 462 MB | 2.7 % |
| Python helpers (Quick Share daemon, sharing, captions, etc.) | 6 | ~150 MB | ~0.1 % |
| Watchers and monitors (bash `watch.sh` wrappers, gdbus/udevadm/pactl monitors, nmcli, adb, systemd-inhibit) | 33 | ~150 MB | ~0 % |
| **Everything the shell runs** | **40** | **~765 MB** | |

RSS counts shared libraries in each process, so the total overstates real memory; it's still the fair number to compare against a rewrite measured the same way.

## How to measure again
```
P=<irice-shell pid in the session>
ps -o pid=,rss=,pcpu=,etimes=,comm= -p $P            # the shell
# its helpers, recursively: ps --ppid, then each child's children
```
Also worth recording next time: start-up time (launch to first frame), idle CPU over 60 s (`pidstat -p $P 1 60`), frame times while animating (QSG_RENDER_TIMING=1), and memory after an hour.

## Already visible before any rewrite
- 33 small processes are watchers: about ten `bash watch.sh` wrappers (4.4 MB each) only exist to make a monitor end with the shell; three separate `gdbus monitor`s. One supervisor process, or monitors run in-process, would remove most of them.
- The six Python helpers hold ~150 MB together; those are the natural first Rust targets (Quick Share, link sharing, the Wi-Fi helper that runs as root, the backup tool).
