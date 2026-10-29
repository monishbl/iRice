.pragma library
// Scrolling over the island to move between its live activities
// (Island.scroll). A mouse wheel notch is one step; a touchpad sends many
// small deltas per swipe, which used to count as a step each and spun
// through the activities. Deltas are added up until they make a notch, then
// it waits for the switch to finish before taking another; a touchpad swipe
// (small deltas, then its momentum) is one step however long it runs. Up or
// right is back, down or left is next; whichever axis moved more decides.
// Pure logic, tested in Node (tests/island_cycle.test.cjs).

var NOTCH = 120;   // angleDelta of one wheel notch
var LOCK_MS = 320; // one switch at a time (its animation)
var IDLE_MS = 400; // a pause starts a new gesture

// state: { acc, at, lockedUntil, swiped }; dx, dy: angleDelta; now: ms
// -> { state, step: -1 | 0 | 1 }
function wheel(state, dx, dy, now) {
    var s = state ? { acc: state.acc, at: state.at, lockedUntil: state.lockedUntil, swiped: state.swiped } : { acc: 0, at: 0, lockedUntil: 0, swiped: false };
    // Down (negative y) or left (negative x) moves on
    var d = Math.abs(dx) > Math.abs(dy) ? dx : dy;
    if (d === 0)
        return { state: s, step: 0 };
    if (now - s.at > IDLE_MS) {
        s.acc = 0;
        s.swiped = false;
    } else if (s.acc !== 0 && (d > 0) !== (s.acc > 0)) {
        s.acc = 0;
    }
    s.at = now;
    // A swipe that already switched: the rest of it (and its momentum)
    // doesn't switch again; a wheel notch is whole and may, after the lock
    if (now < s.lockedUntil || (s.swiped && Math.abs(d) < NOTCH))
        return { state: s, step: 0 };
    s.acc += d;
    if (Math.abs(s.acc) < NOTCH)
        return { state: s, step: 0 };
    var step = s.acc < 0 ? 1 : -1;
    s.swiped = Math.abs(d) < NOTCH;
    s.acc = 0;
    s.lockedUntil = now + LOCK_MS;
    return { state: s, step: step };
}
