# irice-events: one process instead of the watchers

Measured 4 November 2026 in sandbox arena 8 (the nested dev sandbox, not the real session), Glass look, the arena's own settings, the shell up for two minutes after a reload. Same arena, same settings, same wait for both. Read-only from /proc (`VmRSS` of every descendant of the `irice-shell` process). The real session will be measured separately.

| What | Before (main, 08e3a40) | After (irice-events) |
|---|---:|---:|
| Processes under the shell | 25 | 2 |
| Their resident memory | 111.9 MB | 7.7 MB |
| Of which: watchers and their wrappers | 24 processes, 108.1 MB | `irice-events`, 1 process, 4.1 MB (14 threads) |
| The glass request helper (`glass-ipc`, the C one in both) | 3.8 MB | 3.6 MB |

The shell itself was 496.5 MB before and 468.5 MB after. That difference is noise between runs, not irice-events.

## What went
Before, the arena ran 10 `bash watch.sh` wrappers (3.7–4.0 MB each) and what they wrapped: `pactl subscribe`, two `udevadm monitor`s (usb/block and input), `dbus-monitor` (badges), `nmcli monitor`, `avahi-browse`, `camera.sh`, `location.sh` and its `gdbus monitor` (with a second bash for the pipe), `screenshare.sh`, and `transfers.py` (12.1 MB of Python). There was also a `sleep` started every few seconds by camera.sh (no inotify-tools in the container) and screenshare.sh.

The arena doesn't run the real session's logind processes, which are off in the sandbox: the two `gdbus monitor`s (Lock with fingerprint/face unlock; Power while iRice handles the lid) and the `systemd-inhibit … sleep infinity` pairs (lid, idle). On the real session irice-events replaces them too, so the saving there is larger (the user counted 37 processes and ~200 MB before).

## Still running (and why)
- `glass-ipc`: the requests to Hyprland's socket for the glass plugin. It answers per frame, and a stuck request restarts it on its own. Folding it into irice-events would tie the glass to the event stream's restarts; left for a later step.
- Only while a feature is on or in use (none at idle in this arena): Phone (`phone.py` and `adb`), FastPair, Mirroring, Captions, AirPlay (`pipewire`), the speed test.
- `wl-paste --watch cliphist store` belongs to the Hyprland session, not the shell. The shell's own `wl-paste --watch` (Clipboard) is replaced by irice-events watching cliphist's database.

## How to measure again
```
IRICE_ARENA=8 ./dev.sh up                    # then set the look, reload, wait ~2 minutes
P=<irice-shell pid with XDG_RUNTIME_DIR=/tmp/irice-rt-8 in /proc/P/environ>
# every descendant of P (ppid walk over /proc/*/stat), summing VmRSS from /proc/*/status
```
Without the binary (`$XDG_DATA_HOME/irice/bin/irice-events` missing) the shell runs the watchers as before; that's the "before" column on any branch.
