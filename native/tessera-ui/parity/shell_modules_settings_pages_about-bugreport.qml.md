# shell/modules/settings/pages/about-bugreport.qml ↔ surfaces/settings (src/pages/about/bugreport.rs `page`, ui/settings-about.slint boxes/shot/log/btnswitch)

_Re-checked 17 Nov (B5, parity-settings) against main 18628a5 + the B5 ports: statuses below say what's there now; [12 Nov: …] is the old status._

Report a Bug (page `about/bugreport`, title "Report a Bug"). The report is
the shell's BugReport (feed.about.bug; asks `bug.*`).

## Behaviours

| # | Behaviour (QML line) | Rust place | Status |
|---|---|---|---|
| 1 | Title; on open gather() when the log is empty and not gathering (13, 258) | bugreport.rs 311; RustBridgeAbout.qml 32–34 | same |
| 2 | Group "What happened" + footer; "Title" TextField 300, placeholder "e.g. Dock disappears after waking", every edit → BugReport.title (73–86) | 317, 327–329 | same |
| 3 | Three Boxes (inset Material 20/16, Glass 12/12, 16 apart): label (body) over an 88 px box; Glass fill + 1 px separator, accent border on focus; Material FilledField (r 20, **hover tint**, 2 px primary outline on focus); hint as placeholder (tertiary); edits → happened/steps/expected (17–62, 88–116) | 318–330; slint TextBox 114–159, boxes 431–443 | **same (fixed before 17 Nov: TextBox Material hover tint, focus outline)** [12 Nov: different (small): Material box has no hover tint] |
| 4 | Box text area inside a **ScrollView** (margins 8): long text scrolls inside the 88 px box (46–60) | TextBox: clipped TextInput at 12/10, no scrolling | **same (fixed before 17 Nov: TextBox scrolls inside, following the caret)** [12 Nov: **different**: past ~4 lines the text and cursor run out of view] |
| 5 | Box follows the value when it changes from outside (a cleared form clears the box) (24–25) | TextBox `text: root.value` (Slint drops the binding once typed in) | check: after a send that clears the form, Rust boxes may keep the old text |
| 6 | Group "Attached" + footer (scrubbing) (119–122) | 363–364 | same |
| 7 | "Screenshot": detail none-taken / take text; "Mark up…" (pencil-simple, only with a shot) → editShot; switch checked includeShot && hasShot, enabled hasShot (123–140) | 365–372 (btnswitch) | same |
| 8 | Screenshot image (has & include): 12 in, width min(page−24, 480), aspect kept, reloaded on shotVersion (141–157) | 360, 373–375; slint shot 446–458 | same |
| 9 | "Tessera log": detail Reading… / "The last 500 lines, scrubbed"; Show/Hide + switch includeLog; preview LogView 200 ("Reading the log…" / log / "The log is empty.") (158–183) | 361, 376–379 | same |
| 10 | "System info": Show/Hide + switch includeInfo; preview 140 ("Reading…" / key: value lines) (184–210) | 362, 380–383 | same |
| 11 | Send row: label Sending… / Report sent. Thank you! / error / "You can send another in N s" / Ready to send; secondary, red on error; detail issue URL when sent; buttons Open issue (sent), Copy report + Open on GitHub (error), "Send report" (paper-plane-tilt, primary, busy sending, disabled sending / waiting / gathering); footer (213–254) | 386–413, send_label 291–299 | same |

## Test plan

Setups as in pages_menubar. `open about/bugreport`; read y from a shot.
**Never press "Send report", "Open on GitHub" or "Mark up…".** Type only
into the boxes and clear them afterwards.

BR1 `shot bug-top`, scroll, `shot bug-end`. PASS: same groups, rows, texts,
"Send report" state.

BR2 Box scrolling: click "What happened", type 8 lines
(`run 'key -t "line one" -k Return …'` ×8), `shot bug-box`. PASS (QML): the
box scrolls, the cursor's line is visible; Rust: write what shows.

BR3 Material hover: move the pointer over a box (not focused), `shot`.
PASS (QML): the box tints; Rust: note.

BR4 Previews: Tessera log › Show → 200 px dark box with the log; Hide.
System info › Show → 140 px box; Hide. Switch the log's switch off and on.

BR5 Clean up: select all in each box (`key -M ctrl -k a -m ctrl`) and Delete;
clear the Title.

## Fixed in batch 3 (fixer)

- The boxes scroll and follow the caret; Material's hover tint; a cleared form clears them.

## Test results

(tester)
