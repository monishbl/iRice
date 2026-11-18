//! Popovers close on a press on any screen (rust.surfaces: clickcatcher;
//! tessera_core::outside says what is open). It maps nothing.
//!
//! Two ways a press elsewhere reaches the shell:
//! - Hyprland 0.56 keeps the pointer with a layer that holds the keyboard
//!   exclusively, as every open panel and menu does, even when it is over
//!   another screen: the press comes to the panel's own window at a position
//!   off it (core wayland.rs: `Part::pointer_pressed_outside`). Every open
//!   popover closes.
//! - A press on one of the shell's surfaces on another output (a bar, a
//!   dock, the desktop): the popovers that aren't on that output close.
//!
//! (18 Nov: transparent catcher layers mapped on the other screens while a
//! panel was open made Hyprland move its monitor focus, so the "display you
//! moved to" cue played at each open and close, and on 0.56 they never got
//! the press anyway.)

use super::*;
use tessera_core::outside::{self, Elsewhere, PANELS_OWNER};
use tessera_core::services::bridge::{self, ShellState};
use tessera_core::shell::Part;

const NAME: &str = outside::CATCHER;

#[derive(Default)]
pub struct ClickCatcher;

/// What is open now: the parts' menus and Panels' panel
fn popovers(st: &State) -> Vec<(&'static str, String)> {
    let mut v = outside::parts_open();
    if let Some(s) = st.shell.get::<ShellState>() {
        if let Some(p) = outside::panel_popover(s.str("open"), s.str("screen")) {
            v.push(p);
        }
    }
    v
}

fn close(st: &mut State, owners: Vec<&'static str>, why: &str) {
    if owners.is_empty() {
        return;
    }
    if tessera_core::env::is_set("DEBUG") {
        eprintln!("debug: outside press ({why}) closes {owners:?}");
    }
    for owner in owners {
        if owner == PANELS_OWNER {
            bridge::call(&["panels", "close"]);
        } else {
            outside::close(&st.tx, owner);
            let _ = st.tx.send(tessera_core::Msg::Part(owner, Box::new(Elsewhere)));
        }
    }
}

impl Part for ClickCatcher {
    fn name(&self) -> &'static str {
        NAME
    }

    /// A press on any of the shell's surfaces: the popovers on other screens close
    fn pointer_pressed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if !super::on() {
            return;
        }
        let owners = outside::closed_by(&popovers(st), &s.output);
        close(st, owners, &format!("on {} of {}", s.namespace, s.output));
    }

    /// A press off the window holding the pointer (another screen): every
    /// open popover closes
    fn pointer_pressed_outside(&mut self, st: &mut State, s: &Rc<Surface>) {
        if !super::on() {
            return;
        }
        let owners = outside::closed_off_window(&popovers(st));
        close(st, owners, &format!("off {}", s.namespace));
    }
}
