//! What every event loop of tessera-ui does with its messages, and the
//! fonts it registers: the shell's loop (src/main.rs) and the login screen's
//! own (surfaces/greeter's `run`) share them.

use crate::wayland::State;
use crate::Msg;

/// One message for the loop (Slint's timers brought to now first)
pub fn handle(state: &mut State, msg: Msg) {
    // (Slint's animation clock moves only when the loop ticks it: on an idle
    // window it still reads its last frame's time, so an animation a message
    // starts would begin out of step. Bring it to now first.)
    slint::platform::update_timers_and_animations();
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
            state.each_part(|p, st| p.data(st, what));
        }
        Msg::Part(name, m) => {
            let mut m = Some(m);
            state.part(name, |p, st| p.message(st, m.take().unwrap()));
        }
        Msg::GlassPlugin(ok) => {
            eprintln!("tessera-ui: glass plugin {}", if ok { "found" } else { "missing (Hyprland's blur instead)" });
            state.shell.plugin = ok;
            state.each_part(|p, st| p.plugin_changed(st));
        }
        Msg::ConfigChanged => state.config_changed(),
        Msg::A11yActivate(id) => {
            for p in state.parts.iter_mut() {
                for a in p.a11y() {
                    if a.id == id {
                        a.activate();
                    }
                }
            }
        }
        Msg::A11yAction(id, r) => {
            for p in state.parts.iter_mut() {
                for a in p.a11y() {
                    if a.id == id {
                        a.action(r.clone());
                    }
                }
            }
        }
    }
}

/// Fonts that aren't system fonts: Phosphor (icons) and Google Sans Flex
/// ("Tessera Sans Flex", Material's), mapped from the repo, registered with
/// Slint's font collection at run time (not compiled in)
pub fn register_fonts() {
    use slint::fontique_011::fontique;
    let roots = [
        std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../..")),
        std::path::PathBuf::from(crate::env::var("ROOT").unwrap_or_default()),
    ];
    let mut collection = slint::fontique_011::shared_collection();
    for f in ["shell/assets/fonts/Phosphor.ttf", "shell/assets/fonts/Phosphor-Fill.ttf", "shell/assets/fonts/Phosphor-Bold.ttf", "material/assets/fonts/GoogleSansFlex.ttf", "material/assets/fonts/MaterialSymbolsRounded.ttf"] {
        let Some(file) = roots.iter().find_map(|r| std::fs::File::open(r.join(f)).ok()) else {
            eprintln!("tessera-ui: font {f} not found");
            continue;
        };
        if let Ok(map) = unsafe { memmap2::Mmap::map(&file) } {
            collection.register_fonts(fontique::Blob::new(std::sync::Arc::new(map)), None);
        }
    }
}
