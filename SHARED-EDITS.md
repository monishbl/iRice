# Shared edits for the greeter (tessera-ui --greeter), on the workspace layout

The greeter is the `tessera-greeter` crate (`native/tessera-ui/surfaces/greeter/`). It isn't a shell surface: it isn't in `KNOWN` or `make()`, and `--surfaces greeter` still answers "unknown". Below are the changes outside that folder, all applied on this branch.

| File | Change | Why |
|---|---|---|
| `native/tessera-ui/core/src/runloop.rs` (new) | `handle(state, msg)` and `register_fonts()`, **moved** from src/main.rs, not copied. `register_fonts` now finds the repo at `CARGO_MANIFEST_DIR/../../..` because it lives in core/. | The greeter runs its own event loop and needs the same message handling and fonts. |
| `native/tessera-ui/core/src/lib.rs` | `pub mod runloop;` | |
| `native/tessera-ui/core/Cargo.toml` | `memmap2.workspace = true` | `register_fonts` maps the font files. |
| `native/tessera-ui/src/main.rs` | Its own `handle` and `register_fonts` are removed, replaced by `use tessera_core::runloop::{handle, register_fonts};`. Before `surfaces::unknown_arg`: `if tessera_greeter::wanted(&args) { return tessera_greeter::run(&args, started); }`. One usage line in the top comment. | `--greeter` has to be handed over before the unknown-argument check rejects it. |
| `native/tessera-ui/Cargo.toml` | `tessera-greeter = { path = "surfaces/greeter" }` under the binary's `[dependencies]` | |
| `native/tessera-ui/core/src/wayland.rs` | In `key_text`, after `Keysym::Page_Down`: `Keysym::F1 => Key::F1` … `Keysym::F12 => Key::F12` | No function key reached any Slint window. The text login screen's F1/F2/F3 (shut down, restart, sleep) did nothing (found in the sandbox). Additive: keys no surface handled before. |
| `native/tessera-ui/Cargo.lock` | `cargo update --workspace`: adds tessera-greeter and nothing else | |

No platform change: the greeter uses the layer shell as every surface does (`Surface.layer`). If the lead's `Role` unification lands, the greeter touches `w.surface.layer.set_keyboard_interactivity(…)` and `.commit()` in one place (`set_primary` in lib.rs).

The greeter group's own non-Rust files:
- `shell/modules/greeter/GreeterSurface.qml`: "Touch ID or Enter Password" becomes "Use Fingerprint or Enter Password" (no product names). The Rust greeter says the same, and tests/test_rust_greeter.py checks both.
- New: `native/tessera-ui/tools/fake-greetd.py`, `native/tessera-ui/tools/greeter-shots.sh`, `tests/test_rust_greeter.py`.
