#pragma once

// When focus that follows the pointer should bring the pointer along.
// Pure, so it's tested without a compositor (tests/test_pointer_policy.py).
//
// With focus following the pointer (input.follow_mouse = 1), a window that
// got focus some other way is lost again on the next nudge of the mouse: the
// window under the pointer takes focus back. That's what made apps asked to
// come forward (a link opened in the browser, "Open in VS Code", a sign-in
// finishing) never come into view. Such a focus change moves the pointer
// onto the window, but only when the pointer is over another window (over
// the menu bar, the dock or empty desktop nothing would take focus back,
// and a pointer jumping out from under a click on the dock would be odd).
namespace FocusPolicy {

// Hyprland 0.56's Desktop::eFocusReason, by value (FocusState.hpp)
enum Reason : int {
    UNKNOWN = 0,
    FFM = 1, // focus follows mouse
    KEYBIND = 2,
    DISPATCH_FOCUSWINDOW = 3,
    DISPATCH_MOVEWINDOWINTOGROUP = 4,
    CLICK = 5,
    OTHER = 6,
    DESKTOP_STATE_CHANGE = 7,
    SWITCH_TO_WINDOW_SOFT = 8,
    SWITCH_TO_WINDOW_HARD = 9,
    GROUP_CURRENT_WINDOW_CHANGE = 10,
    WORKSPACE_CHANGE = 11,
    TOGGLE_SPECIAL_WORKSPACE = 12,
    UNMAP_WINDOW_TILING = 13,
    UNMAP_WINDOW_FLOATING = 14,
    UNMAP_GROUPED_WINDOW = 15,
    NEW_WINDOW = 16,
    GHOSTS = 17,
};

// Focus that didn't come from the pointer, the keyboard's own focus moves
// (Hyprland warps for those itself), a workspace switch (the shell refocuses
// what's under the pointer), a window closing or opening (the shell handles
// new windows), but from an app or a dispatcher bringing a window forward
inline bool broughtForward(int reason) {
    switch (reason) {
    case UNKNOWN:
    case DISPATCH_FOCUSWINDOW:
    case OTHER:
    case SWITCH_TO_WINDOW_SOFT:
    case SWITCH_TO_WINDOW_HARD:
        return true;
    default:
        return false;
    }
}

inline bool shouldWarp(int reason, bool pointerInWindow, bool pointerOverOtherWindow) {
    return broughtForward(reason) && !pointerInWindow && pointerOverOtherWindow;
}

} // namespace FocusPolicy
