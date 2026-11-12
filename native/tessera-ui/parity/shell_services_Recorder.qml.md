# shell/services/Recorder.qml → host/desktop/src/screen/recorder.rs (stop and the end of a recording)

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Stop: during the countdown cancels it; otherwise SIGINT to the recorder (gpu-screen-recorder paused: SIGUSR2 first so the end is written) | 232-240 | `stop` | same |
| The recorder hears the signal | 224 (Process starts it with default signals) | **tessera-ui blocks its signals for its signalfd and, started in the background by a non-interactive shell, has SIGINT ignored; an ignored signal stays ignored across exec, so wf-recorder never heard the stop** → `pre_exec`: every signal unblocked, INT/TERM/HUP/USR1/USR2/PIPE at default | fixed |
| A stop that isn't heard still ends it | — (Qt: none) | `rec_escalate`: SIGTERM 4 s after SIGINT, SIGKILL 3 s after that, only for the same run | added (so the timer can never run on) |
| Ending after a stop is a save, also when it ended by a signal (130/143/137) and the file has data | — | `rec_exited`: a signal end after a stop with a non-empty file counts as saved | fixed (was "The recorder stopped (code -1)") |
| Saved: island "Recording saved" with Open / Show in Folder (island on with the recording event), else a notification with both actions | `saved` | `saved` | same |
| Discard: stop, file removed after 8 s unless Undo | 252-270 | `discard`, `drop_now` | same |
| The process is the recorder itself (setpriv and sh exec it: one PID) | 224 | `rec_pid` = the child's PID | same |

## Test plan

Arena with the user's settings, standalone (`--bin` this worktree's binary). wf-recorder installed (and gpu-screen-recorder if the arena has it: run both paths if so; set `recorder` to use wf-recorder by uninstalling/hiding gsr from PATH for the second run if needed).

1. Start a screen recording: `ipc recorder screen` (or Super+Shift+R in the arena: ALT is the mod there). PASS: the recording pill/island shows a counting timer; `pgrep -a wf-recorder` (inside the arena) shows one process; `grep SigIgn /proc/<pid>/status` shows bit 2 (SIGINT) NOT set (`SigIgn: 0000000000000000` or at least `& 0x2 == 0`), and `SigBlk: 0000000000000000`.
2. Move the pointer or open foot for 5 s so frames change.
3. Stop with the island's stop button (hover the island, click the red stop). PASS: within ~1 s wf-recorder exits, the timer disappears, the island shows "Recording saved" with the file name and Open / Show in Folder; `~/Videos/Screen Recording <stamp>.mp4` (or the configured folder) exists and `ffprobe` reads its duration ≈ the recorded time.
4. Repeat with the pill's stop button, then with `ipc recorder stop`. Same PASS each time.
5. Static screen: start a recording, touch nothing (no pointer, no windows) for 10 s, then `ipc recorder stop`. PASS: it ends within 8 s at the latest (SIGINT, then SIGTERM at 4 s, SIGKILL at 7 s; the log says which); the timer stops; "Recording saved" if the file has data. FAIL if the timer keeps counting or wf-recorder is still in `pgrep` after 8 s.
6. Discard: start, record 3 s, press Discard on the island. PASS: it stops, the island offers Undo for 8 s; without Undo the file is gone after 8 s; with Undo it stays.
7. QML pass for reference (steps 1, 3, 4): same timings and messages.
