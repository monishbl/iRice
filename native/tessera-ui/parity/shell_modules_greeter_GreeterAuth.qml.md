# Parity: shell/modules/greeter/GreeterAuth.qml

The greeter's login logic: people from /etc/passwd, sessions from
wayland-sessions, the greetd conversation (opened as soon as a person is
chosen so a fingerprint reader listens at once), the preview mock, typed
usernames, remembering the last person and session.
Rust: `surfaces/greeter/src/model.rs` (`Model`: pure logic, `Effect`s),
`greetd.rs` (greetd's JSON protocol on $GREETD_SOCK), `system.rs` (passwd,
sessions, hostname, cache file, Exec parsing), `lib.rs` (does the effects:
timers, requests, quit).

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Mock (preview) without greetd or with TESSERA_GREETER_MOCK=1; password "tessera" after 600 ms | 21, 56-59, 177-187 | `Greeter::new` (`mock`), `Effect::MockCheck` → 600 ms → `mock_result` | same |
| login(): busy guard; "Type your username first."; respond if PAM asked, else hold the password and open the conversation | 47-68 | `Model::login` | same |
| begin(): one conversation per chosen user (CreateSession) | 69-77 | `Model::begin` | same |
| restart() on a person / typed name change: cancel, clear message & finger, reopen 300 ms later | 78-91, 112-115 | `Model::restart`, `Effect::BeginLater` (300 ms `begin_timer`) | same |
| editName / pickTyped / cancelName | 92-111 | `auth_edit_name`, `pick_typed`, `auth_cancel_name` | same |
| launch(): parse Exec (session-exec.js), unsupported → "This session's launch command isn't supported. Choose another with Tab."; remember {user, session}; StartSession | 117-135 | `Model::launch`, `system::parse_exec`, `Effect::Remember` | same |
| greetd messages: secret/visible prompt → answer the held password or wait; finger/swipe/touch words → finger waiting/miss + message; other error → error line; info → message | 139-157 | `Model::heard` AuthMessage arms (info/error answered with an empty response, as greetd requires) | same |
| Auth failure → "Incorrect password. Try again.", restart (the reader listens again) | 158-163 | `heard` AuthError → `failed` | same |
| Ready to launch → launch | 164-168 | `heard` Success | same |
| Other error → its text or "Login failed. Try again." | 169-174 | `heard` Error | same |
| Answers for a cancelled conversation ignored | (Quickshell's Greetd) | `queue` with `gen` | same |
| greetd gone | — | `Model::lost` → "The login service isn't answering. Try again in a moment." | Rust extra (fine) |
| Preview: quits 1.2 s after a "login", Esc quits | 188-197 | `Effect::PreviewDone` (1200 ms `quit_timer`), `cancel` → Quit | same |
| Users: uid 1000..59999 with a login shell; none → typed username | 199-209 | `system::users`, `Model::new` (`other` when empty) | same |
| Sessions: Tessera first, then Hyprland, then by name; fallback Hyprland | 211-226 | `system::sessions`, `sort_sessions` | same |
| Hostname (text style's title) | 228-233 | `system::hostname` | same |
| Picture: AccountsService's icon only | 234-238 | `want_picture` (decoded off the UI thread, once per name; Material cut to cookie9) | same |
| Restore last person / typed name and session from ~/.cache/tessera-greeter.json | 240-258 | `system::remembered`, `Model::restore` | same |

## Test plan (arena N; the greeter runs as a client of the arena's compositor)

Run: `IRICE_ARENA=N ./dev.sh run env TESSERA_GREETER_MOCK=1 <bin> --greeter --style look --look glass` (preview: the password is **tessera**; Esc quits). For the greetd path: `python3 native/tessera-ui/tools/fake-greetd.py` in the arena (see its header for its socket), then `GREETD_SOCK=<sock> <bin> --greeter --demo users`.
1. Type `nope` + Return → field clears, shakes, "Incorrect password. Try again."; ~600 ms "Logging in…" first.
2. Type `tessera` + Return → "logs in": the window quits ~1.2 s later; stderr says "would launch".
3. Esc with an empty field and no power choice → quits (preview).
4. With fake-greetd: choose a person (Down) → a CreateSession appears in fake-greetd's log ~300 ms later (before Enter); the fingerprint demo (`--demo finger`) shows the glyph and "Place your finger…".
5. Remember: after step 2, run again → the same person and session are preselected (cache file in the arena's ~/.cache).

## Test results

(testers fill in)
