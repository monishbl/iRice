# Porting a surface to Rust

How one of the shell's surfaces (the dock, the OSD, notifications, …) moves from QML to `tessera-ui` (native/tessera-ui, Slint). The plan and the order are in [rust-ui-plan.md](rust-ui-plan.md); the toolkit's measurements in [rust-ui-spike-slint.md](rust-ui-spike-slint.md). The menu bar (`surfaces/menubar/`) is the worked example. Paths below are inside native/tessera-ui, a Cargo workspace: "The workspace" below.

## The rule

A surface moves when its Rust version does what the QML one does, **in both looks** (Glass and Material), at the user's settings. Until then it ships as a preview: its name is accepted in Settings' `rust.surfaces` (empty by default), so nothing changes for anyone who hasn't listed it.

## The pieces, and where each goes

| Piece | Where | Notes |
|---|---|---|
| Its crate | `surfaces/<name>/` | `Cargo.toml` (`tessera-<name>`), `build.rs`, `src/`, `ui/`: copy the dock's and rename ("The workspace") |
| The part (behaviour) | `surfaces/<name>/src/lib.rs` (and its other files beside it) | a struct implementing `shell::Part` (see `core/src/shell.rs`) |
| Its registration | the workspace's `Cargo.toml` and `src/surfaces.rs` | one `tessera-<name> = { path = "surfaces/<name>" }` line under the binary's `[dependencies]`; one name in `KNOWN`, one line in `make()` |
| Its Slint markup | `surfaces/<name>/ui/<name>.slint` | `import { Theme, Icons, Icon, … } from "@tessera/theme.slint";` (the shared files are the `@tessera` library: core/ui/) |
| Its export | `surfaces/<name>/ui/main.slint` | `export { Theme, Icons } from "@tessera/theme.slint";` and one `export { … } from "<name>.slint";` line |
| Data it needs from the system | `core/src/services/<service>.rs` | a thread that sends `Msg::Data(Box::new(value))`; registered in `services::start` by name; a part asks with `st.shell.want("<service>")` and reads `st.shell.get::<T>()` |
| The QML twin stepping aside | the QML surface's model or `visible` | `RustUi.drawing("<name>")` (e.g. MenuBar.qml's model is `[]` then) |
| Hearing from the shell | the QML twin, then the part's `message` | `RustUi.tell("<name>", {…})` writes `{"to": "<name>", …}` on tessera-ui's stdin (only while it draws that surface); the part's `message` gets it as a `serde_json::Value`. E.g. Background.qml tells the background the day's sunrise and sunset and the lock's settle (LockFx's steps). Panels' open and close come as commands instead ("Panels" below) |
| Tests | the part's `#[cfg(test)] mod tests` (`cargo test -p tessera-<name>`), and `tests/test_rust_ui.py` checks the twin | |

Shared, so ask before changing their meaning (add, don't rename), all in tessera-core (`core/`): `src/shell.rs`, `src/theme.rs` + `ui/theme.slint` (tokens follow `shell/common/Theme.qml`; add a token there when a surface needs one the QML Theme has: "Theme tokens" below), `ui/widgets.slint` (search field, icon button, key caps, spinner, Material state shapes, text button), `ui/symbols.slint` (`SymIcon`: a Phosphor glyph or Material's symbol for it), `src/wayland.rs` (the platform), `src/a11y.rs`. A change to anything in core/ recompiles every surface; a change inside `surfaces/<name>/` only that crate and the binary.

## The workspace

native/tessera-ui is a Cargo workspace (one crate until main b19b699, 5 Nov 2026), so a change to one surface recompiles only that surface's crate and the final binary, and each rustc needs less memory (measurements below).

| Crate | Folder | What's in it |
|---|---|---|
| `tessera-ui`, the binary | `src/main.rs`, `src/surfaces.rs`, `src/bin/fake-ime.rs` | the event loop and stdin (main.rs); `KNOWN` and `make()`, the list of surfaces (surfaces.rs). `target/release/tessera-ui` and `--bin tessera-ui` are where they were |
| `tessera-core` | `core/src/` | the platform (wayland.rs, egl.rs, a11y.rs, stats.rs); shell.rs (`Part`, `Shell`, `Command`, `ask`/`closed`/`tell`); `Msg` (lib.rs); theme.rs (`Look`, `tokens`, `glass_shape`, `theme_global!`); services/; surfaces/ (`Surfaces`, `announce`, `tell`, `on_screen`, `shapes`); helpers more than one surface uses: util.rs (`on_path`, `encode`…), env.rs, seed.rs, appicon.rs, icons.rs, icon_theme.rs, picture.rs (`decode`, `Pool`, `second_frame`), wallpaper.rs (which picture shows; the look's seed reads it), settings.rs (`island_hud`, `emoji_font`); services/notify/model.rs and markup.rs (the notifications' model and body markup, which the server fills) |
| tessera-core's Slint | `core/ui/` | theme.slint (`Theme`, `Icons`, `Icon`), widgets.slint, symbols.slint: the `@tessera` Slint library every surface's build.rs passes to slint-build (`@tessera/theme.slint`) |
| `tessera-<name>`, one per surface | `surfaces/<name>/` | `src/lib.rs` (the part: what was `src/surfaces/<name>/mod.rs`) and its other files, `ui/main.slint` (its exports), `ui/<name>.slint`, `build.rs` |

The surface crates: menubar, dock, emoji, clipboard, osd, wallpaper (both the `background` and `wallpaper` parts: `src/background/`, `src/wallpaper/`), today, desktop, notifications, spotlight, controlcenter, sidebar.

**Rules.** A surface crate depends on tessera-core, never on another surface's crate (that would recompile it with the other). Something two surfaces use goes into core/ (as `on_path`, `encode`, `island_hud`, `emoji_font`, `picture`, `SymIcon` did) and the owner re-exports it where its code used it (`pub use tessera_core::settings::island_hud;` in osd's settings.rs), so the old paths still read. `[workspace.dependencies]` in the root Cargo.toml has every version once; a crate names what it uses (`image.workspace = true`).

**A surface crate's lib.rs** starts, after its `//!` comment, with:

```rust
pub mod ui {
    slint::include_modules!();
}
tessera_core::theme_global!(ui);

// tessera-core's modules (shell, wayland, theme, services, util…) under the
// names they had when everything was one crate: `crate::shell::Part`
#[allow(unused_imports)]
use tessera_core::*;
```

The glob keeps `crate::shell::Part`, `crate::wayland::State`, `crate::services::hypr::Glass`, `crate::util::now_ms`, `crate::Msg`, `crate::surfaces::shapes`… working in every file of the crate unchanged; the crate's own modules win over it (a surface's own `settings` or `icons`). In lib.rs itself, name core directly in its `use` lines (`use tessera_core::shell::Part;`), and don't `use crate::ui;` there (it's defined right there); the other files keep `use crate::…`.

**Theme tokens.** Each surface crate compiles its own Slint, so each has its own generated `ui::Theme` type. `theme_global!(ui)` implements `tessera_core::theme::ThemeGlobal` for it, from the one token list (`__theme_tokens!` in core/src/theme.rs, each `field / setter: type`); `st.shell.apply_theme(&ui.global::<ui::Theme>())` fills it exactly as before (`theme::tokens(look, plugin)` gives the values; a token a look doesn't set is left as it was). A new token: its property in core/ui/theme.slint, its line in the list, its value in `tokens()`; a token missing from the list or from theme.slint doesn't compile.

**Adding a surface:** copy `surfaces/dock/`'s Cargo.toml, build.rs and ui/main.slint into `surfaces/<name>/`, rename (`tessera-<name>`, its own dependencies and exports), write `src/lib.rs` with the header above; then one line in the root Cargo.toml (`tessera-<name> = { path = "surfaces/<name>" }`; `members` already takes `surfaces/*`), its name in `KNOWN` and its line in `make()` (src/surfaces.rs). Then `cargo update --workspace` adds the new crate to Cargo.lock and nothing else (`--locked` builds refuse until it's there); commit it. A surface begun before the split: "Moving a branch from before the split" below.

**Commands** (in native/tessera-ui, through the build gate):

| What | Command |
|---|---|
| your surface's tests only (fast: core is built once and reused) | `cargo test --locked -p tessera-<name>` |
| every test (the binary's, core's, every surface's) | `cargo test --locked --workspace` |
| the release binary | `cargo build --release --locked --bin tessera-ui` |

**Build times** (12 cores, the gate's sandbox settings: no LTO, 16 codegen units, incremental, lld; `SLOT_JOBS=10`; dependencies already built in both, so "clean" is this workspace's own crates; peak memory is the largest single rustc's VmHWM, polled every 0.5 s, and the sum over every rustc running at once):

| Build | One crate (main b19b699) | The workspace | Largest rustc, before → after | Every rustc at once, after |
|---|---|---|---|---|
| clean release (`--bin tessera-ui`) | 158 s | 78 s | 6.8 GB → 1.4 GB (desktop) | 9.0 GB |
| clean test build (`--no-run`) | 122 s | 130 s | 7.8 GB → 1.7 GB (desktop's tests) | 9.8 GB |
| release, one line in the dock's Rust | 23 s | 7 s | 4.3 GB → 0.4 GB | 0.4 GB |
| tests, the same | 27 s | 4 s | 4.6 GB → 0.5 GB | 0.6 GB |
| release, a value in dock.slint | 25 s | 3 s | 4.3 GB → 0.35 GB | 0.35 GB |
| tests, the same | 32 s | 7 s | 4.4 GB → 0.5 GB | 0.7 GB |
| release, a comment in dock.slint | 24 s | 1 s | 4.2 GB → 0.35 GB | 0.3 GB |
| tests, the same | 30 s | 4 s | 4.4 GB → 0.5 GB | 0.8 GB |
| release, a line in core/ui/theme.slint (every surface) | 24 s | 12 s | 4.2 GB → 0.9 GB | 5.0 GB |
| tests, the same | 31 s | 25 s | 4.3 GB → 1.0 GB | 6.6 GB |
| release, a line in core/src/shell.rs (everything) | 26 s | 13 s | 4.3 GB → 0.9 GB | 4.5 GB |
| tests, the same (`--workspace`) | 33 s | 71 s | 4.6 GB → 1.0 GB | 5.9 GB |

A change inside one surface now costs seconds and under 1 GB. A change in core/ recompiles every surface: the release build is still twice as fast, but `cargo test --workspace` is slower than before (each surface's tests are their own binary to compile and link), so while you work run only your crate's (`-p tessera-<name>`). With many jobs, a clean build runs a dozen rustc processes side by side (9–10 GB together at `SLOT_JOBS=10`); with the gate's usual 2–3 jobs it should stay near two or three crates' worth (not measured).

## Panels: opened by the shell, drawn here

Panels (the emoji picker, the clipboard, later Control Center, Search…) open through the QML shell's `Panels.show/toggle/close`, whatever asked (a shortcut, `qs ipc call …`, Spotlight, Control Center). They keep doing so: services/RustUi.qml forwards every open and close of a panel tessera-ui draws as a JSON line on its stdin, and tessera-ui answers on stdout (the protocol is in `core/src/shell.rs`'s top comment):

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

- **Layer surfaces:** `st.prepare_surface(Spec)` then create the Slint component at once (it becomes that surface's window). One per output for per-screen surfaces (follow the QML model's `screens` setting with `surfaces::on_screen`). `surface.show(w, h)` / `surface.hide()` map and unmap; `st.drop_surface` when it goes, after `ui.hide()` and `surface.hide()` (Slint keeps a shown component, and with it the surface, alive: a dropped strip's exclusive zone stayed).
- **Regular windows** (the App Store; later Settings): `st.prepare_window(WindowSpec { title, app_id, namespace, size, min_size })`, then the component, as for a layer. It maps on its first frame and resizes as the compositor says; a close request (Super+Q) reaches `Part::surface_closed`. Never touch `surface.layer` on one (it has no layer).
- **Drag and drop** (core/src/dnd.rs): `st.accept_drops(&surface, true)` and the part's `drag` hears Enter, Motion, Leave and Drop (file URIs and text, read for it); `st.refuse_drop()` during the call says "not here". `st.start_drag(&surface, offers)` drags data out from the latest press (the shelf's rows).
- **Glass:** in Glass, panels sit on the plugin's glass: `theme::glass_shape(...)` then `st.shell.glass.set(output, namespace, &shapes)`, only when the shapes change; `Theme.panel` is the tint over it. Material draws its own solid tonal surfaces (no glass).
- **Keyboard:** `KeyboardInteractivity::OnDemand` for things you type into; `keyboard_left` is the moment to close (another window was clicked).
- **Accessibility:** `crate::a11y::A11y::new(&surface, st.tx.clone())`, `a.update(false, focused)` in `after_frame`, and return them from `a11y()`. Give every control an `accessible-role` and `accessible-label` (see menubar.slint); icon glyphs `accessible-role: none` (wrap them: `Icon`).
- **Settings:** read from `st.shell.cfg` (config.json as JSON) with the QML Config.qml defaults when a key is missing; small pure functions with tests (see `surfaces/menubar/src/settings.rs`); settings two surfaces read go in `core/src/settings.rs`.
- **Not ready yet:** a part that needs something before it can take over (notifications: the session bus's name) calls `st.shell.surfaces.hold(name)` in `start`, then `release(name)` and `crate::surfaces::announce(...)` once it has it; the QML twin keeps drawing until then.
- **Talking with the QML shell, beyond panels:** a part's own lines on stdout (`crate::shell::tell(kind, …)`, `{"src":"ui","kind":…}`) that RustUi.qml doesn't handle itself reach QML as the `RustUi.message(e)` signal, for the service that owns the data; QML's `RustUi.send({to: "<name>", …})` reaches that part's `message` as a `serde_json::Value`. The notifications part is the example (its and panel.rs's top comments list their lines): a panel whose name differs from its surface's (Notification Center, "notificationCenter", in "notifications") is forwarded by its own service with `cmd` lines, not by RustUi's `follow`.
- **Pictures:** decode off the UI thread and hand Slint a buffer no larger than it shows (`core/src/picture.rs`: `cover`, `decode`, a small `Pool`). A still surface needs a second frame: the first is drawn while the EGL window is still 1×1 and the compositor stretches it (`picture::second_frame`).
- **Actions** the QML surface does through Hyprland: `services::hypr::request("dispatch …")` on a thread (never block the UI thread). Things the QML shell does itself (open a panel, run its IPC): `shell/assets/scripts/shell-ipc.sh call …` from a thread, until that piece is in Rust too.

## A panel's own commands (the sidebar)

Beyond `open` and `close`, RustUi.qml can send a panel commands of its own, `RustUi.send({cmd, surface, …})` (any other `cmd` reaches `Part::command` with the whole object in `Command::args`). The sidebar's: `tab` (Panels.sidebarTab, sent before it opens and whenever it changes), `attach` (a screenshot or circled picture's file for the chat, `lens: true` when circled: ScreenTools.qml) and `continue` (Spotlight's question and answer: Ai.qml). It asks the shell for `sidebar.tab` (its tab row), `ai.reloadHistory` (it wrote the chat history: the QML Ai service reads it again), `screen.ask` (its screenshot menu), `launch.openLink` and `panels.openSettings`. tests/test_rust_ui.py checks that what RustUi sends is handled.

## Fidelity: what "matches" means

Read the QML surface and everything it imports (its module folder, `shell/common/*` it uses, the services it reads, and `material/` overrides). Match: layout and sizes (the QML numbers, in logical px), the look's tokens (`Theme`), motion (durations and springs: `Theme.motion`; Slint `animate … { easing: spring(…) }`), every state it shows (empty, hover, pressed, focus, disabled), what each click, scroll, key and drag does, and its settings. Where Slint can't do something QML does (custom shaders, `ShaderEffect`), say so in the part's top comment and pick the closest built-in.

## Testing

1. `cargo test --locked -p tessera-<name>` in native/tessera-ui while you work, `cargo test --locked --workspace` before you hand back: pure logic (layout maths, settings, models built from data) as plain functions with tests. Keep tests fast; no display needed.
2. `python3 -m pytest -q tests/test_rust_ui.py` (the QML twin steps aside; nothing moves by default) and the whole suite before merging.
3. **The hidden sandbox only, never the user's screen:** `IRICE_ARENA=<your arena> ./dev.sh …` (arenas are hidden and silent; never set `IRICE_VISIBLE` or `IRICE_HOST_AUDIO`). site/film/tessera/capture/lib.sh has the helpers (`setup glass|material <wallpaper> 1`, `inbox` to run inside, `dev shot` for screenshots). Install the release build into the arena's data dir (`.sandbox/arena-N/home/.local/share/tessera/bin/tessera-ui`), set `rust.surfaces` in the arena's config, screenshot the QML surface and the Rust one in the same state, both looks, and compare. Kill only processes whose `/proc/PID/environ` names your arena; never `pkill`/`killall` (PIDs are shared with the user's session). Bring the arena down when done (the laptop runs hot).
4. Build release once at the end (`cargo build --release --locked --bin tessera-ui`, about 1–2 minutes incremental).
5. **Never run a binary of this workspace with the live display.** Run `cargo test`, any test binary and `tessera-ui` itself with `env -u WAYLAND_DISPLAY -u WAYLAND_SOCKET -u DISPLAY …`, or inside an arena. Don't run the files in `target/*/deps/` by hand: the programs there (`tessera_ui-…`, `fake_ime-…`) sit beside the test binaries, and a program started on the real session draws on it. As a guard, `tessera-ui` refuses any argument it doesn't know (`--list`…) before it opens a display, and `fake-ime` runs only inside an arena (`TESSERA_ARENA`/`IRICE_ARENA` set), with no arguments.

## Moving a branch from before the split

This is for the porting branches started on main b19b699, when native/tessera-ui was one crate. Each of them adds `src/surfaces/<name>/` and `ui/<name>.slint`, registers the surface in `src/surfaces/mod.rs` and `ui/main.slint`, and edits shared files. Here is how such a branch moves onto the workspace. I tested it on a branch built the same way: a new surface with its own .slint, a new service file, a new Theme token, a new helper in shell.rs, and calls to dock's `on_path` and osd's `island_hud`. After these steps it built and passed `cargo test --locked --workspace` with no hand edits.

**Where everything went:**

| Before (b19b699) | Now |
|---|---|
| `src/surfaces/<name>/mod.rs`, its other files | `surfaces/<name>/src/lib.rs` (with the header from "The workspace"), the others beside it |
| `ui/<name>.slint` | `surfaces/<name>/ui/<name>.slint`; `"theme.slint"` and `"widgets.slint"` imports become `"@tessera/theme.slint"` and `"@tessera/widgets.slint"` |
| your line in `ui/main.slint` | `surfaces/<name>/ui/main.slint`, after `export { Theme, Icons } from "@tessera/theme.slint";` |
| `pub mod <name>;` in `src/surfaces/mod.rs` | `tessera-<name> = { path = "surfaces/<name>" }` in the root Cargo.toml |
| `KNOWN`, `make()`: `<name>::Part::default()` | `src/surfaces.rs`: `tessera_<name>::Part::default()` |
| `src/shell.rs`, `theme.rs`, `wayland.rs`, `a11y.rs`, `egl.rs`, `stats.rs`, `util.rs`, `env.rs`, `icons.rs`, `appicon.rs`, `icon_theme.rs`, `seed.rs` | `core/src/` (same names) |
| `src/services/…` (`notify.rs`) | `core/src/services/…` (`notify/mod.rs`) |
| `src/surfaces/mod.rs`: `Surfaces`, `announce`, `tell`, `on_screen` | `core/src/surfaces/mod.rs` (`crate::surfaces::…` still works); `Surfaces::from_args` takes the list: `surfaces::from_args(&args)` in the binary |
| `src/surfaces/shapes.rs` | `core/src/surfaces/shapes.rs` (`crate::surfaces::shapes` still works) |
| `ui/theme.slint`, `ui/widgets.slint`; `SymIcon`, `Symbols` in `ui/island.slint` | `core/ui/theme.slint`, `widgets.slint`, `symbols.slint` |
| `src/surfaces/background/{picture,wallpaper}.rs`; `background::second_frame` | `core/src/picture.rs`, `core/src/wallpaper.rs`; `crate::picture::second_frame` |
| `src/surfaces/background/mod.rs`, `src/surfaces/wallpaper/` | `surfaces/wallpaper/src/background/`, `surfaces/wallpaper/src/wallpaper/` |
| `src/surfaces/notifications/{model,markup}.rs` | `core/src/services/notify/{model,markup}.rs` (notifications re-exports both) |
| `crate::surfaces::dock::on_path`, `emoji::klipy::encode` | `crate::util::on_path`, `crate::util::encode` |
| `crate::surfaces::emoji::emoji_font`, `osd::settings::island_hud` | `tessera_core::settings::emoji_font`, `tessera_core::settings::island_hud` |
| `env!("CARGO_MANIFEST_DIR")` + `"/../.."` (the repo) | `"/../../../.."` in a surface crate, `"/../../.."` in core (`include_str!` paths relative to a file don't change for a surface's files: same depth) |
| your new external crate in `[dependencies]` | its line in `[workspace.dependencies]`, and `<crate>.workspace = true` in your crate's Cargo.toml |

**The steps**, in your worktree, with your work committed:

```sh
git merge main
cd native/tessera-ui
python3 tools/adopt-surface.py <name>    # moves it, writes its crate, registers it, adds its Theme tokens
git rm -q src/surfaces/mod.rs ui/main.slint
cargo update --workspace                 # Cargo.lock gains your crate, nothing else changes
git add -A . && git status               # anything still unmerged: below
# then, through the gate, with the display removed:
#   cargo test --locked -p tessera-<name>; cargo test --locked --workspace;
#   python3 -m pytest -q tests (from the repo root); cargo build --release --locked --bin tessera-ui
git commit
```

**What `git merge main` does, and what you do:**

- **Your edits to shared files** (shell.rs, theme.rs, theme.slint, widgets.slint, wayland.rs, a11y.rs, services/…) follow the renames into `core/` by themselves. So do edits you made to another surface's files: they follow into that surface's crate. Only lines that clash with main's own changes conflict, as usual.
- `CONFLICT (file location)` for a file you **added** in `src/services/`: git has already put it in `core/src/services/`. `git add -A` takes it there.
- `CONFLICT (modify/delete)` for `src/surfaces/mod.rs` and `ui/main.slint`: your copies stay in the tree, and `adopt-surface.py` reads your `make()` line and your export lines from them. Then `git rm` them. If you changed anything else in `src/surfaces/mod.rs` (`Surfaces`, `announce`, `on_screen`), make that change again in `core/src/surfaces/mod.rs`.
- `CONFLICT (directory rename split)` for `ui/` is only a notice: your `ui/<name>.slint` stays in `ui/` for the tool to move. Your `src/surfaces/<name>/` stays where it was too.
- `src/main.rs` no longer has the `mod …;` lines. A new shared module goes in `core/src/` with a `pub mod x;` line in `core/src/lib.rs`. A new service needs only its line in `core/src/services/mod.rs`, as before.
- `Cargo.toml`: if you added an external crate, resolve the conflict by putting your line under `[workspace.dependencies]` and adding `<crate>.workspace = true` to `surfaces/<name>/Cargo.toml`.
- `tests/test_rust_ui.py`: conflicts there are ordinary text conflicts. Paths in your new checks change: `CRATE / "src/surfaces/<n>/mod.rs"` becomes `surface("<n>") / "src/lib.rs"`, `CRATE / "src/services/x.rs"` becomes `CORE / "src/services/x.rs"`, and `(CRATE / "src").rglob("*.rs")` becomes `rust_sources()`.

**What `adopt-surface.py` does:**

- It takes the part's type from your old `make()` line (or `--part`), and your exports from your old `ui/main.slint` line (or `--export`). If your surface has more than one .slint file, name them all with `--slint a.slint b.slint`.
- It writes Cargo.toml, build.rs and ui/main.slint (copied from the dock's), and lists your external crates by what your code uses. Check Cargo.toml afterwards.
- It writes lib.rs's header. It turns lib.rs's own `use crate::shell…` lines into `use tessera_core::shell…` and drops its `use crate::ui;`. Your other files keep their `use crate::…` lines.
- It rewrites the borrowed helpers in the table above, the `CARGO_MANIFEST_DIR` paths, and the Slint imports (`SymIcon`/`Symbols` from island.slint become `@tessera/symbols.slint`).
- It adds every token `theme::tokens` sets that the token list lacks, typed from core/ui/theme.slint. `tools/adopt-surface.py --tokens` does only that, for a branch that adds tokens but no surface.
- It prints what it can't do: a `crate::surfaces::<other surface>::…` it doesn't know, or an import from another surface's .slint. For those, move the item into core (`core/src/util.rs`, `core/src/settings.rs`, a module of its own, or `core/ui/`). Then re-export it from where its owner used it (`pub use tessera_core::…;`), so the owner's code doesn't change.

**Commands that changed:** `cargo test --locked --bin tessera-ui` now runs only the binary's own tests (the surface list). Use `-p tessera-<name>` while you work and `--workspace` before you hand back. Everything else (`--release --bin tessera-ui`, `target/release/tessera-ui`, installing it into an arena) is unchanged.
