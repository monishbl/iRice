# Parity: shell/modules/greeter/GreeterNameLabel.qml

The chosen person's name: on hover a soft pill and a pencil; a click types
another name.
Rust: `surfaces/greeter/ui/greeter.slint` `NameLabel`.

| Behaviour | QML line | Rust place | Status |
|---|---|---|---|
| Size: text + 2 × (12 + 20) wide, text + 12 high, pill radius | 166-168 | `width`, `height`, `border-radius` | same |
| Hover fill, 150 ms colour animation | 169-170 | `background` by `touch.has-hover`, `animate background 150ms` | same |
| Pencil (pencil-simple, 14 px) 6 px after the name, opacity 0.8 on hover, 150 ms | 175-182 | `PIcon` pencil, `animate opacity 150ms` | same |
| I-beam cursor; click → editName | 183-189 | `TouchArea` (`MouseCursor.text`, `clicked`) | same |
| Disabled while the Material pill is closed (its opacity follows openT) | (Material GreeterSurface 128-129) | `enabled: root.ot > 0.5` | same |
| Accessible: button, "<name>, type another username" | 190-192 | `accessible-*` | same |

## Test plan (arena N, Glass and Plain, `--demo users`)

1. `pointer glide` onto the name: the pill fades in (~150 ms) and the pencil appears to the right of the name; glide off: both fade out. Record and check ≈150 ms with anim-timing.
2. Click the name: the username field takes its place with the name selected.
3. Material (`--look material`): before the pill opens the name above it is invisible and can't be clicked; move the pointer (opens the pill) → the name fades in above, hover/click work.

## Test results

(testers fill in)
