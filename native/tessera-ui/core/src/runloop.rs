//! What every event loop of tessera-ui does with its messages, and the
//! fonts it registers: the shell's loop (src/main.rs) and the login screen's
//! own (surfaces/greeter's `run`) share them.

use crate::wayland::State;
use crate::Msg;

/// One message for the loop (Slint's timers brought to now first)
pub fn handle(state: &mut State, msg: Msg) {
    if crate::wayland::wakes_on() {
        let what = match &msg {
            Msg::Run(_) => "run".to_string(),
            Msg::Data(_) => "data".into(),
            Msg::Part(n, _) => n.to_string(),
            Msg::Line(_) => "line".into(),
            _ => "other".into(),
        };
        crate::wayland::WAKE_MSGS.with(|m| *m.borrow_mut().entry(what).or_default() += 1);
    }
    if crate::perf::on() || crate::wayland::wakes_on() {
        let what = match &msg {
            Msg::Run(_) => "msg run".to_string(),
            Msg::Quit => "msg quit".into(),
            Msg::Data(_) => "msg data".into(),
            Msg::Part(n, _) => format!("msg {n}"),
            Msg::GlassPlugin(_) => "msg glass plugin".into(),
            Msg::ConfigChanged => "msg config changed".into(),
            Msg::Line(_) => "msg line".into(),
            Msg::A11yActivate(_) | Msg::A11yAction(..) => "msg a11y".into(),
        };
        return crate::perf::time(|| what, || handle_now(state, msg));
    }
    handle_now(state, msg)
}

fn handle_now(state: &mut State, msg: Msg) {
    // (Slint's animation clock moves only when the loop ticks it: on an idle
    // window it still reads its last frame's time, so an animation a message
    // starts would begin out of step. Bring it to now first.)
    crate::perf::time(|| "slint timers".into(), slint::platform::update_timers_and_animations);
    match msg {
        Msg::Run(f) => f(),
        Msg::Quit => state.exit = true,
        Msg::Data(v) => {
            // (the glass the shell said, its wallpaper tone and Cool-down: the look follows)
            if let Some(g) = v.downcast_ref::<crate::theme::ShellGlass>() {
                if crate::theme::set_shell_glass(g.clone()) {
                    state.config_changed();
                }
            }
            let what = state.shell.store(v);
            // (while something animates — a panel closing — the shell's
            // state and Hyprland's snapshot, which closing a panel changes,
            // would reach every part in this one turn, and the frame waits
            // for all of them: they go a few at a time between frames)
            if state.shell.defer_data && animating(state) {
                let names: Vec<(&'static str, bool)> = state.parts.iter().map(|p| (p.name(), p.urgent_data(what))).collect();
                for (n, urgent) in names {
                    if urgent {
                        // (Part::urgent_data: now, and any held copy is stale)
                        state.shell.deferred.retain(|d| *d != (n, what));
                        state.part(n, |p, st| p.data(st, what));
                    } else if !state.shell.deferred.contains(&(n, what)) {
                        state.shell.deferred.push_back((n, what));
                    }
                }
            } else {
                state.shell.deferred.retain(|(_, w)| *w != what);
                state.each_part(|p, st| p.data(st, what));
            }
        }
        Msg::Part(name, m) => {
            let mut m = Some(m);
            state.part(name, |p, st| p.message(st, m.take().unwrap()));
        }
        Msg::GlassPlugin(ok) => {
            eprintln!("tessera-ui: glass plugin {}", if ok { "found" } else { "missing (Hyprland's blur instead)" });
            state.shell.plugin = ok;
            state.each_part(|p, st| p.plugin_changed(st));
            state.redraw_everything();
        }
        Msg::ConfigChanged => state.config_changed(),
        // (the binary's loop handles the host's lines before this; elsewhere, none come)
        Msg::Line(_) => {}
        // (each part's own code: contain.rs)
        Msg::A11yActivate(id) => {
            for p in state.parts.iter_mut() {
                let n = p.name();
                crate::contain::guard(n, || {
                    for a in p.a11y() {
                        if a.id == id {
                            a.activate();
                        }
                    }
                });
            }
        }
        Msg::A11yAction(id, r) => {
            for p in state.parts.iter_mut() {
                let n = p.name();
                crate::contain::guard(n, || {
                    for a in p.a11y() {
                        if a.id == id {
                            a.action(r.clone());
                        }
                    }
                });
            }
        }
    }
}

/// Does any window run an animation now?
pub fn animating(state: &State) -> bool {
    state.surfaces.iter().any(|s| s.window().has_active_animations())
}

/// The readings held back while something animated (Shell::deferred): as
/// many as fit in `budget` this turn, all of them once nothing animates.
/// Each part still gets every kind of reading, the newest, in order.
pub fn drain_deferred(state: &mut State, budget: std::time::Duration) {
    if state.shell.deferred.is_empty() {
        return;
    }
    let all = !animating(state);
    let t = std::time::Instant::now();
    while let Some((name, what)) = state.shell.deferred.pop_front() {
        crate::perf::time(|| format!("deferred data > part {name}"), || state.part(name, |p, st| p.data(st, what)));
        if !all && t.elapsed() >= budget {
            break;
        }
    }
}

/// Fonts that aren't system fonts: Phosphor (icons) and Google Sans Flex
/// ("Tessera Sans Flex", Material's, and its Qt-like copy "Tessera Text
/// Flex", textflex.rs), mapped from the repo, registered with
/// Slint's font collection at run time (not compiled in)
pub fn register_fonts() {
    use slint::fontique_011::fontique;
    // (the checkout the shell runs from first: crate::env::root, 17 Nov)
    let roots = [crate::env::root(), std::path::PathBuf::from(crate::env::BUILT_ROOT)];
    let mut collection = slint::fontique_011::shared_collection();
    for f in ["shell/assets/fonts/Phosphor.ttf", "shell/assets/fonts/Phosphor-Fill.ttf", "shell/assets/fonts/Phosphor-Bold.ttf", "material/assets/fonts/GoogleSansFlex.ttf", "material/assets/fonts/MaterialSymbolsRounded.ttf", "material/assets/fonts/MaterialSymbolsRounded-Fill.ttf"] {
        let Some(file) = roots.iter().find_map(|r| std::fs::File::open(r.join(f)).ok()) else {
            eprintln!("tessera-ui: font {f} not found");
            continue;
        };
        if let Ok(map) = unsafe { memmap2::Mmap::map(&file) } {
            // (Material's text face, "Tessera Text Flex": its weights as the
            // QML's Qt draws them, textflex.rs; Theme.font in Material)
            if f.ends_with("GoogleSansFlex.ttf") {
                match crate::textflex::derive(&map) {
                    Some(text) => {
                        collection.register_fonts(fontique::Blob::new(std::sync::Arc::new(text)), None);
                    }
                    None => eprintln!("tessera-ui: {f}: no Tessera Text Flex (not the variable font expected)"),
                }
            }
            collection.register_fonts(fontique::Blob::new(std::sync::Arc::new(map)), None);
        }
    }
}
