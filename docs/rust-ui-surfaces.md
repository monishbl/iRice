# Porting a surface to Rust

How one of the shell's surfaces (the dock, the OSD, notifications, …) moves from QML to `tessera-ui` (native/tessera-ui, Slint). The plan and the order are in [rust-ui-plan.md](rust-ui-plan.md); the toolkit's measurements in [rust-ui-spike-slint.md](rust-ui-spike-slint.md). The menu bar (`src/surfaces/menubar/`) is the worked example.

## The rule

A surface moves when its Rust version does what the QML one does, **in both looks** (Glass and Material), at the user's settings. Until then it ships as a preview: its name is accepted in Settings' `rust.surfaces` (empty by default), so nothing changes for anyone who hasn't listed it.

## The pieces, and where each goes

| Piece | Where | Notes |
|---|---|---|
| The part (behaviour) | `native/tessera-ui/src/surfaces/<name>/mod.rs` | a struct implementing `shell::Part` (see `src/shell.rs`) |
| Its registration | `src/surfaces/mod.rs` | one `pub mod <name>;`, one name in `KNOWN`, one line in `make()` |
| Its Slint markup | `native/tessera-ui/ui/<name>.slint` | `import { Theme, Icons, Icon, … } from "theme.slint";` |
| Its export | `ui/main.slint` | one `export { … } from "<name>.slint";` line |
| Data it needs from the system | `src/services/<service>.rs` | a thread that sends `Msg::Data(Box::new(value))`; registered in `services::start` by name; a part asks with `st.shell.want("<service>")` and reads `st.shell.get::<T>()` |
| The QML twin stepping aside | the QML surface's model or `visible` | `RustUi.drawing("<name>")` (e.g. MenuBar.qml's model is `[]` then) |
| Tests | the part's `#[cfg(test)] mod tests`, and `tests/test_rust_ui.py` checks the twin | |

Shared, so ask before changing their meaning (add, don't rename): `src/shell.rs`, `src/theme.rs` + `ui/theme.slint` (tokens follow `shell/common/Theme.qml`; add a token there when a surface needs one the QML Theme has), `ui/widgets.slint` (search field, icon button, key caps, spinner, Material state shapes, text button), `src/wayland.rs` (the platform), `src/a11y.rs`.

## Panels: opened by the shell, drawn here

Panels (the emoji picker, the clipboard, later Control Center, Search…) open through the QML shell's `Panels.show/toggle/close`, whatever asked (a shortcut, `qs ipc call …`, Spotlight, Control Center). They keep doing so: services/RustUi.qml forwards every open and close of a panel tessera-ui draws as a JSON line on its stdin, and tessera-ui answers on stdout (the protocol is in `src/shell.rs`'s top comment):

| Direction | Line | Meaning |
|---|---|---|
| shell → tessera-ui | `{"cmd":"open","surface":"emoji","screen":"DP-1"}` | Panels opened it on that output (sent again if it moves to another) |
| shell → tessera-ui | `{"cmd":"close","surface":"emoji"}` | Panels closed it (its shortcut again, another panel opened) |
| tessera-ui → shell | `{"src":"ui","kind":"closed","surface":"emoji"}` | it closed itself (Esc, a click outside, a pick): `Panels.close()` |
| tessera-ui → shell | `{"src":"ui","kind":"call","call":"clipboard.pin","args":[…]}` | something only the shell does (its state files, Settings, opening a link): RustUi.qml's `call()` lists them |

To move a panel: its part implements `Part::command` ("open" on `cmd.screen`, "close"; any other `cmd` reaches it too with the whole object in `Command::args`), calls `shell::closed(name)` when it closes itself, and `shell::ask(call, args)` for the shell's own things (add the call to RustUi.qml's `call()`; tests/test_rust_ui.py checks every asked call is answered). Its QML twin's PanelLoader steps aside with `elsewhere: RustUi.drawing("<name>")`: the QML window then never loads, and if tessera-ui ends, the QML one opens again. A panel is a full-screen transparent Overlay layer (`Anchor` all sides, size 0×0, `exclusive: -1`) with `KeyboardInteractivity::Exclusive` while open and `None` while it animates out, so a click outside reaches it (and closes it) as the QML panels' click catcher does; `keyboard_left` closes it too. Keep the window after closing (hide the surface): Slint keeps its drawn glyphs.

Keys a panel drives from its search field: the field's `key` callback returns at once whether it's one the panel handles (Slint wants the answer synchronously) and posts the key to the part as a message; the part does the rest.

## A part

```rust
#[derive(Default)]
pub struct Dock { /* its windows, per output */ }

impl Part for Dock {
    fn name(&self) -> &'static str { "dock" }
    fn start(&mut self, st: &mut State) { st.shell.want("hypr"); }
    fn output_added(&mut self, st: &mut State, output: &WlOutput, name: &str) {
        let surface = st.prepare_surface(Spec { layer: Layer::Top, namespace: "tessera:dock", .. });
        let ui = ui::Dock::new().unwrap();          // made right after prepare_surface: it becomes its window
        st.shell.apply_theme(&ui.global::<ui::Theme>());
        ui.show().unwrap();
        // keep (surface, ui, a11y) per output
    }
    fn config_changed(&mut self, st: &mut State, look: bool) { /* settings; re-apply the theme if `look` */ }
    fn data(&mut self, st: &mut State, what: TypeId) { /* a service's new reading */ }
    fn message(&mut self, st: &mut State, msg: Box<dyn Any + Send>) { /* its own, from callbacks: st.shell.sender("dock") */ }
    fn after_frame(&mut self, st: &mut State) { /* glass shapes (when they change), a11y.update */ }
    fn quit(&mut self, st: &mut State) { /* clear its glass: Glass::clear_now */ }
}
```

- **Layer surfaces:** `st.prepare_surface(Spec)` then create the Slint component at once (it becomes that surface's window). One per output for per-screen surfaces (follow the QML model's `screens` setting with `surfaces::on_screen`). `surface.show(w, h)` / `surface.hide()` map and unmap; `st.drop_surface` when it goes.
- **Glass:** in Glass, panels sit on the plugin's glass: `theme::glass_shape(...)` then `st.shell.glass.set(output, namespace, &shapes)`, only when the shapes change; `Theme.panel` is the tint over it. Material draws its own solid tonal surfaces (no glass).
- **Keyboard:** `KeyboardInteractivity::OnDemand` for things you type into; `keyboard_left` is the moment to close (another window was clicked).
- **Accessibility:** `crate::a11y::A11y::new(&surface, st.tx.clone())`, `a.update(false, focused)` in `after_frame`, and return them from `a11y()`. Give every control an `accessible-role` and `accessible-label` (see menubar.slint); icon glyphs `accessible-role: none` (wrap them: `Icon`).
- **Settings:** read from `st.shell.cfg` (config.json as JSON) with the QML Config.qml defaults when a key is missing; small pure functions with tests (see `surfaces/menubar/settings.rs`).
- **Not ready yet:** a part that needs something before it can take over (notifications: the session bus's name) calls `st.shell.surfaces.hold(name)` in `start`, then `release(name)` and `crate::surfaces::announce(...)` once it has it; the QML twin keeps drawing until then.
- **Talking with the QML shell, beyond panels:** a part's own lines on stdout (`crate::shell::tell(kind, …)`, `{"src":"ui","kind":…}`) that RustUi.qml doesn't handle itself reach QML as the `RustUi.message(e)` signal, for the service that owns the data; QML's `RustUi.send({to: "<name>", …})` reaches that part's `message` as a `serde_json::Value`. The notifications part is the example (its and panel.rs's top comments list their lines): a panel whose name differs from its surface's (Notification Center, "notificationCenter", in "notifications") is forwarded by its own service with `cmd` lines, not by RustUi's `follow`.
- **Actions** the QML surface does through Hyprland: `services::hypr::request("dispatch …")` on a thread (never block the UI thread). Things the QML shell does itself (open a panel, run its IPC): `shell/assets/scripts/shell-ipc.sh call …` from a thread, until that piece is in Rust too.

## Fidelity: what "matches" means

Read the QML surface and everything it imports (its module folder, `shell/common/*` it uses, the services it reads, and `material/` overrides). Match: layout and sizes (the QML numbers, in logical px), the look's tokens (`Theme`), motion (durations and springs: `Theme.motion`; Slint `animate … { easing: spring(…) }`), every state it shows (empty, hover, pressed, focus, disabled), what each click, scroll, key and drag does, and its settings. Where Slint can't do something QML does (custom shaders, `ShaderEffect`), say so in the part's top comment and pick the closest built-in.

## Testing

1. `cargo test` in native/tessera-ui: pure logic (layout maths, settings, models built from data) as plain functions with tests. Keep tests fast; no display needed.
2. `python3 -m pytest -q tests/test_rust_ui.py` (the QML twin steps aside; nothing moves by default) and the whole suite before merging.
3. **The hidden sandbox only, never the user's screen:** `IRICE_ARENA=<your arena> ./dev.sh …` (arenas are hidden and silent; never set `IRICE_VISIBLE` or `IRICE_HOST_AUDIO`). site/film/tessera/capture/lib.sh has the helpers (`setup glass|material <wallpaper> 1`, `inbox` to run inside, `dev shot` for screenshots). Install the release build into the arena's data dir (`.sandbox/arena-N/home/.local/share/tessera/bin/tessera-ui`), set `rust.surfaces` in the arena's config, screenshot the QML surface and the Rust one in the same state, both looks, and compare. Kill only processes whose `/proc/PID/environ` names your arena; never `pkill`/`killall` (PIDs are shared with the user's session). Bring the arena down when done (the laptop runs hot).
4. Build release once at the end (`cargo build --release --locked --bin tessera-ui`, about 1–2 minutes incremental).
