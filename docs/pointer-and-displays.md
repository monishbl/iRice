# Pointer focus and display boundaries

## Focus

With **Settings → Mouse → Focus → Follows pointer**, workspace changes and window close/move/float changes recheck the window under the stationary pointer. New input-capable windows are the exception requested by the user: they take focus, and the pointer moves into them if they open elsewhere, including on another monitor. Windows already under the pointer do not need a cursor relocation.

Reliable stationary-pointer refocus uses the rebuilt compositor plugin to account for Hyprland’s movement-threshold check even after the pointer has been idle. Without the plugin, same-position hit-testing is best effort; new-window targeting still works.

Updates are event-driven and coalesced for 180 ms; there is no continuous focus polling. iRice delegates hit-testing to Hyprland instead of guessing which window is on top. Locking, open shell panels and region selection suppress this automatic focus work. On-click focus disables it.

## Boundaries

**Settings → Mouse → Between displays** offers:

- **Protect shared corners** (default): blocks crossing within 24 logical pixels of either end of an edge. The middle remains open. Corner reach is adjustable from 4–100 pixels.
- **Edge resistance / push again**: initially stops the pointer at an edge. Sustaining a push for 180 ms or pushing again after that delay releases it. An attempt expires after 900 ms without another outward push.
- **Only during screen selection**: normal monitor crossing remains free; region selection confines the pointer to its starting monitor.
- **Off**: no native boundary restrictions. Captured image coordinates still stay within their starting display.

All enabled modes confine the pointer during region selection. This has a 1.5-second lease refreshed while selection is active, so a shell crash cannot leave permanent confinement behind. Games/applications with pointer constraints take precedence. Relative mouse and touchpad movement are protected; deliberate pointer warps remain free, including the new-window focus behavior. Absolute tablet positioning is not modified.

The updated `irice-glass` compositor plugin is required for physical pointer barriers. Older/missing plugins retain normal pointer movement and Settings explains the dependency. Screenshot clipping does not require the plugin. The plugin advertises pointer support only when its motion hook installs successfully. New source files are included in its Makefile and normal plugin rebuilds.

## Workspace chips and screenshots

When showing other displays' spaces, every monitor's active workspace is clearly highlighted and remains visible when the workspace strip overflows. The local active workspace still controls the island's active-chip effects. With “other spaces” disabled, each bar continues showing its own monitor's workspace set.

Screenshot/recording/OCR/AI region selections clip drag and window bounds to the starting display. The capture service repeats the clipping defensively; rounding uses rectangle edges together to avoid a one-pixel bleed at fractional coordinates. Escape also cancels a delayed capture.

## Verification status

The native plugin compiles against the installed Hyprland 0.56.2 headers. Automated tests cover corner/resistance geometry, negative display coordinates, scaled logical bounds, focus actions and screenshot clipping. Isolated Quickshell tests cover focus event routing, lock suppression and boundary IPC. The active machine exposed only one monitor during implementation, so actual shared-edge feel, cross-monitor focus and workspace-chip appearance still require multi-monitor validation. The test plugin was built under `/tmp`; the live compositor was not hot-reloaded.
