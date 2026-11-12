# shell/modules/share/ShareFeed.qml → surfaces/share (src/lib.rs `Feed`, `Part::message`, `shell::ask` calls)

| # | Behaviour | QML | Rust | Status |
|---|-----------|-----|------|--------|
| 1 | Sends `{feed:{sheet,request,link,screen,motion}}` on every change, on start and when drawing switches | 671-698 | `Feed` (serde), `message` (applied only when it differs) | same |
| 2 | Sheet: paths, note, icon, devices with sending {done,total,waiting} / ended, available, portsError, errors; Chromecasts for one media file | 586-625 | `Sheet`, `Device` | same |
| 3 | Sharing.look while the sheet is open | 591-601 | (shell side, follows Panels) | same |
| 4 | Request: id, from, type, what, files, total(+Text), pin, state, message, paths, folder, item {done,total,waiting,transport,doneText,totalText} | 628-650 | `Request`, `Item` | same |
| 5 | Link: mode, stage, names, qr, error, busy, done, total, count, installing, updates | 653-669 | `Link` | same |
| 6 | Calls: send key, copy, reveal, qr, answer id bool, cancel, hide, drop, open, show, linkClose, installQr | 700-763 | `send`, `way`, `act_call`, `M::LinkDone`, `M::LinkInstall` | same |
| 7 | A feed that only moves one send's progress changes only that device on screen | (the QML Repeaters keep their delegates) | `devs.set_keyed`, `way_rows.set`, `btns.set` | **was** each feed handed Slint new device / way / button models; now changed in place |

## Test plan (tester)
Sandbox, arena 1920×1080.
1. `inbox 'ipc sharingdemo peers'`, open the sheet, hover device 4, then `inbox 'ipc sharingdemo sending 30'` five times with different values → only device 1's status and ring change; device 4 keeps its hover (ShareSheet step 2).
2. Click "Copy" on a file share → `wl-paste -l` lists `text/uri-list`; the sheet closes.
3. `inbox 'ipc sharingdemo ask'`, click Accept → the card goes to "receiving" (the answer reached the shell).
