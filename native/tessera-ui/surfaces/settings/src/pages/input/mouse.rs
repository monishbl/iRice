//! Mouse & Trackpad (pages/mouse.qml, with trackpad.qml as its part).
//! The shell's Mouse service applies the settings per device; its list of
//! mice and trackpads comes from RustBridgeInput.qml (`mice`, `touchpads`,
//! `boundaryAvailable`); three-finger drag is a gesture (Keymap: `keymap`).

use super::super::super::model::{Ctx, Page, Row};
use super::{feed, idx};
use serde_json::json;

pub fn mouse(cx: &mut Ctx, p: &mut Page) {
    let mice: Vec<String> = feed(cx)["mice"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();

    let speed = cx.f("mouse.speed");
    let sp = cx.num("mouse.speed", 0.05);
    // (one choice: off, or how it shows; "both" from before shows as Ring)
    let now = if !cx.b("findPointer.enabled") || cx.s("findPointer.style") == "off" {
        "off"
    } else if cx.s("findPointer.style") == "grow" {
        "grow"
    } else {
        "ring"
    };
    let values = ["off", "grow", "ring"];
    let fp = cx.on(move |cx, a| {
        let v = values[a.i.clamp(0, 2) as usize];
        cx.set("findPointer.enabled", v != "off");
        if v != "off" {
            cx.set("findPointer.style", v);
        }
    });
    p.group("")
        .footer(match mice.len() {
            0 => "No mouse is connected. These settings apply when one is.".to_string(),
            1 => format!("Applies to {}.", mice[0]),
            n => format!("Applies to {n} mice. Trackpads have their own settings."),
        })
        .row(Row::new("Tracking speed").slider(speed, -1.0, 1.0, 0.0, sp).words("Slow", "Fast").width(240.0))
        .row(Row::new("Shake to find the pointer")
            .detail("Move the mouse quickly back and forth: the pointer grows for a moment, or a ring closes in on it. Works with a trackpad too.")
            .segmented(&["Off", "Bigger", "Ring"], idx(&values, now), fp)
            .width(280.0));

    let natural = cx.b("mouse.naturalScroll");
    let nt = cx.toggle("mouse.naturalScroll");
    let scroll = cx.f("mouse.scrollSpeed");
    let ss = cx.num("mouse.scrollSpeed", 0.1);
    p.group("Scrolling")
        .row(Row::new("Natural scrolling").detail("Content moves the way the wheel turns").switch(natural, nt))
        .row(Row::new("Scroll speed").slider(scroll, 0.2, 3.0, 0.0, ss).words("Slow", "Fast").width(240.0));

    let left = cx.b("mouse.leftHanded");
    let lh = cx.on(|cx, a| cx.set("mouse.leftHanded", a.i == 1));
    p.group("Buttons").row(Row::new("Primary button").detail("Which button clicks; the other opens menus").segmented(&["Left", "Right"], left as i32, lh).width(160.0));

    let focus = cx.s("mouse.focus");
    let fv = ["hover", "click"];
    let fc = cx.pick("mouse.focus", &fv);
    p.group("Windows").footer("Also counts for trackpads.").row(Row::new("Focus")
        .detail(if focus == "click" { "A window takes the keyboard when you click it, as on a Mac" } else { "Workspace changes focus under the pointer. New windows take focus and bring the pointer if needed." })
        .segmented(&["Follows pointer", "On click"], idx(&fv, &focus), fc)
        .width(260.0));

    let restore = cx.on(|cx, _| {
        cx.set("mouse.speed", 0.0);
        cx.set("mouse.accelProfile", "adaptive");
        cx.set("mouse.naturalScroll", false);
        cx.set("mouse.scrollSpeed", 1.0);
        cx.set("mouse.leftHanded", false);
        cx.set("mouse.middleEmulation", false);
        cx.set("mouse.focus", "hover");
        cx.set("mouse.boundaryMode", "corners");
        cx.set("mouse.cornerSize", 24);
    });
    p.group("").row(Row::new("Restore defaults").button("Restore Defaults", restore));

    // ── More options ──
    p.more(cx);
    {
        let accel = cx.s("mouse.accelProfile");
        let av = ["adaptive", "flat"];
        let ac = cx.pick("mouse.accelProfile", &av);
        p.group("").row(Row::new("Pointer acceleration")
            .detail(if accel == "flat" { "The pointer moves the same distance however fast you move the mouse. Good for games and precise work." } else { "Faster movements carry the pointer further" })
            .segmented(&["On", "Off"], idx(&av, &accel), ac)
            .width(160.0));

        let mid = cx.b("mouse.middleEmulation");
        let mt = cx.toggle("mouse.middleEmulation");
        p.group("Buttons").row(Row::new("Middle click with both buttons").detail("Press left and right together, for mice without a wheel button").switch(mid, mt));

        let available = feed(cx)["boundaryAvailable"].as_bool().unwrap_or(false);
        let mode = cx.s("mouse.boundaryMode");
        let modes = ["corners", "resistance", "capture", "off"];
        let labels: Vec<String> = ["Protect shared corners", "Edge resistance / push again", "Only during screen selection", "Off"].iter().map(|s| s.to_string()).collect();
        let current = modes.iter().position(|m| *m == mode).map_or(-1, |i| i as i32);
        let bm = cx.pick("mouse.boundaryMode", &modes);
        let corner = cx.i("mouse.cornerSize");
        let cs = cx.num("mouse.cornerSize", 1.0);
        p.group("Between displays")
            .footer(if available {
                "Applies to mice and trackpads. Deliberate focus warps still cross displays. During region selection, the pointer stays on the starting screen."
            } else {
                "Pointer boundaries need the updated Tessera compositor plugin. Screenshot bounds are protected even without it."
            })
            .row(Row::new("Pointer boundaries").choice(&labels, current, bm).width(260.0))
            .row(Row::new("Corner reach")
                .visible(mode == "corners")
                .detail(format!("{corner} logical pixels from each corner"))
                .slider(corner as f32, 4.0, 100.0, 0.0, cs)
                .words("Small", "Large")
                .width(240.0));
    }
    p.end_more();

    p.part(cx, "Trackpad", trackpad);
}

/// The trackpad (trackpad.qml)
pub fn trackpad(cx: &mut Ctx, p: &mut Page) {
    let pads = feed(cx)["touchpads"].as_array().map_or(0, Vec::len);
    let speed = cx.f("trackpad.speed");
    let sp = cx.num("trackpad.speed", 0.05);
    let tap = cx.b("trackpad.tapToClick");
    let drag = cx.b("trackpad.tapAndDrag");
    // One choice: off, tap to click, or tap to click and drag
    let tc = cx.on(|cx, a| {
        if a.i > 0 {
            cx.set("trackpad.tapAndDrag", a.i == 2);
        }
        cx.set("trackpad.tapToClick", a.i > 0);
    });
    let right = cx.s("trackpad.rightClick");
    let rv = ["twoFingers", "corner"];
    let rc = cx.pick("trackpad.rightClick", &rv);
    p.group("Point and click")
        .footer(if pads == 0 { "No trackpad found. These settings apply when one is connected." } else { "" })
        .row(Row::new("Tracking speed").slider(speed, -1.0, 1.0, 0.0, sp).words("Slow", "Fast").width(240.0))
        .row(Row::new("Tap to click")
            .detail(if !tap {
                "Press the trackpad down to click"
            } else if drag {
                "Tap with one finger to click; tap, then keep the finger down and move it to drag"
            } else {
                "Tap with one finger to click"
            })
            .segmented(&["Off", "Click", "Click and drag"], if !tap { 0 } else if drag { 2 } else { 1 }, tc)
            .width(260.0))
        .row(Row::new("Secondary click")
            .detail(if right == "corner" { "Click in the bottom-right corner" } else { "Click or tap with two fingers" })
            .segmented(&["Two fingers", "Bottom right"], idx(&rv, &right), rc)
            .width(240.0));

    let natural = cx.b("trackpad.naturalScroll");
    let nt = cx.toggle("trackpad.naturalScroll");
    let scroll = cx.f("trackpad.scrollSpeed");
    let ss = cx.num("trackpad.scrollSpeed", 0.05);
    p.group("Scrolling")
        .row(Row::new("Natural scrolling").detail("Content follows your fingers").switch(natural, nt))
        .row(Row::new("Scroll speed").slider(scroll, 0.2, 3.0, 0.0, ss).words("Slow", "Fast").width(240.0));

    let three = feed(cx)["keymap"]["threeFingerDrag"].as_bool().unwrap_or(false);
    let td = cx.on(move |cx, _| cx.ask("input", "threeFingerDrag", json!([!three])));
    p.group("").row(Row::new("Three-finger drag")
        .detail(if three { "Three fingers move the window under them; swipe with four to change spaces" } else { "Three fingers swipe between spaces" })
        .switch(three, td));

    let link = cx.link("mouse/gestures", "");
    p.group("").row(Row::link("Gestures", "mouse/gestures").icon("hand-swipe-right").detail("Swipe and pinch with several fingers: spaces, All Windows, apps").id(link));

    // ── More options ──
    p.more(cx);
    {
        let lock = cx.b("trackpad.dragLock");
        let lt = cx.toggle("trackpad.dragLock");
        let mid = cx.b("trackpad.middleClick");
        let mt = cx.toggle("trackpad.middleClick");
        let typing = cx.b("trackpad.disableWhileTyping");
        let tt = cx.toggle("trackpad.disableWhileTyping");
        p.group("Point and click")
            .row(Row::new("Drag lock").detail("A tap-drag keeps holding when you lift your finger, until you tap again").switch(lock, lt).enabled(tap && drag))
            .row(Row::new("Middle click").detail(if right == "corner" { "Click both bottom corners together" } else { "Click or tap with three fingers" }).switch(mid, mt))
            .row(Row::new("Ignore the trackpad while typing").detail("Stops a resting palm from moving the pointer").switch(typing, tt));
    }
    p.end_more();
}
