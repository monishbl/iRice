//! The click catchers (rust.surfaces: clickcatcher): a popover closes on a
//! press on any screen, not only its own (tessera_core::outside says why and
//! what is open).
//!
//! While a popover is open, every screen without one gets a transparent
//! layer over its apps (overlay, so over a fullscreen app too; exclusive zone
//! 0, so the bars' reserved strips stay the bars'; no keyboard, so typing
//! stays where it was). A press on any of the shell's surfaces on a screen
//! other than a popover's (one of these, a bar, a dock, the desktop) closes
//! that popover: Panels' through `panels close`, a part's own menu with
//! `outside::Elsewhere`. The catchers go the moment the press lands, before
//! the popover has even closed, so only that one press is taken.
//!
//! Made when first needed, unmapped (a null buffer) when nothing is open:
//! never on screen, and never taking input, while closed.

use super::*;
use slint::ComponentHandle;
use tessera_core::outside::{self, Elsewhere, PANELS_OWNER};
use tessera_core::services::bridge::{self, ShellState};
use tessera_core::shell::Part;

const NAME: &str = outside::CATCHER;
const NS: &str = "tessera:click-catcher";

struct Win {
    output: String,
    surface: Rc<Surface>,
    #[allow(dead_code)] // (kept: dropping the component would drop the window)
    ui: ui::Catcher,
    shown: bool,
}

#[derive(Default)]
pub struct ClickCatcher {
    wins: Vec<Win>,
    /// what was open when a press closed it: no catchers again until that changes
    /// (Panels hears the close a moment later)
    spent: Option<String>,
}

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

fn key(popovers: &[(&'static str, String)]) -> String {
    popovers.iter().map(|(w, o)| format!("{w}@{o}")).collect::<Vec<_>>().join(",")
}

impl ClickCatcher {
    fn sync(&mut self, st: &mut State) {
        if !super::on() {
            return;
        }
        let pops = popovers(st);
        let k = key(&pops);
        if self.spent.as_ref().is_some_and(|s| *s != k) {
            self.spent = None;
        }
        let outputs: Vec<String> = st.outputs_now().into_iter().map(|(_, n)| n).collect();
        let want = if self.spent.is_some() { Vec::new() } else { outside::catch_on(&pops, &outputs) };
        // (a screen that went: its catcher with it)
        for w in std::mem::take(&mut self.wins) {
            if outputs.contains(&w.output) {
                self.wins.push(w);
            } else {
                retire(st, &w.surface);
            }
        }
        for w in &mut self.wins {
            let on = want.contains(&w.output);
            if on != w.shown {
                w.shown = on;
                if on {
                    w.surface.show(0, 0);
                } else {
                    w.surface.hide();
                }
                if tessera_core::env::is_set("DEBUG") {
                    eprintln!("debug: click catcher on {}: {}", w.output, if on { "up" } else { "down" });
                }
            }
        }
        for o in want {
            if self.wins.iter().any(|w| w.output == o) {
                continue;
            }
            let Some(wl) = st.output_named(&o) else { continue };
            let surface = st.prepare_surface(Spec {
                layer: Layer::Overlay,
                namespace: NS,
                output: Some((&wl, &o)),
                anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
                size: (0, 0),
                // (0: inside the bars' reserved strips, which stay theirs; a
                // press on a bar closes the popover through pointer_pressed)
                exclusive: 0,
                keyboard: KeyboardInteractivity::None,
                margin_left: 0,
            });
            let ui = ui::Catcher::new().expect("Catcher");
            ui.show().expect("show click catcher");
            if tessera_core::env::is_set("DEBUG") {
                eprintln!("debug: click catcher on {o}: made, up");
            }
            self.wins.push(Win { output: o, surface, ui, shown: true });
        }
    }
}

impl Part for ClickCatcher {
    fn name(&self) -> &'static str {
        NAME
    }

    fn data(&mut self, st: &mut State, what: TypeId) {
        if what == TypeId::of::<ShellState>() {
            self.sync(st);
        }
    }

    fn urgent_data(&self, what: TypeId) -> bool {
        // (Panels' open panel: the catchers come and go with it, not after its animation)
        what == TypeId::of::<ShellState>()
    }

    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) {
        if msg.is::<outside::Changed>() {
            self.sync(st);
        }
    }

    fn output_added(&mut self, st: &mut State, _: &WlOutput, _: &str) {
        self.sync(st);
    }

    fn output_removed(&mut self, st: &mut State, _: &str) {
        self.sync(st);
    }

    /// A press on any of the shell's surfaces: the popovers on other screens close
    fn pointer_pressed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if !super::on() {
            return;
        }
        let pops = popovers(st);
        let owners = outside::closed_by(&pops, &s.output);
        if owners.is_empty() {
            return;
        }
        if tessera_core::env::is_set("DEBUG") {
            eprintln!("debug: click catcher: a press on {} ({}) closes {owners:?}", s.output, s.namespace);
        }
        for owner in owners {
            if owner == PANELS_OWNER {
                bridge::call(&["panels", "close"]);
            } else {
                outside::close(&st.tx, owner);
                let _ = st.tx.send(tessera_core::Msg::Part(owner, Box::new(Elsewhere)));
            }
        }
        // (down at once: the next press reaches the apps, even before the popover hears it)
        self.spent = Some(key(&popovers(st)));
        self.sync(st);
    }

    fn surface_closed(&mut self, st: &mut State, s: &Rc<Surface>) {
        if let Some(i) = self.wins.iter().position(|w| Rc::ptr_eq(&w.surface, s)) {
            let w = self.wins.remove(i);
            retire(st, &w.surface);
            self.sync(st);
        }
    }

    fn quit(&mut self, st: &mut State) {
        for w in std::mem::take(&mut self.wins) {
            retire(st, &w.surface);
        }
    }
}
