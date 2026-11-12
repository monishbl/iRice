# shell/modules/osd/MaterialDisplayConfirm.qml

Parity (12 Nov, written by the lead; not built or tested yet).

Rust: ConfirmMaterial in ui/display.slint.
- Card springs in (springDefault), wavy countdown bar, Go back / Keep settings dialog buttons: same.
- Fixed this round: dialog buttons swell while pressed (ElasticControl), ring only for keyboard focus (focus-on-click false), Return/Space without repeats, 0.1 px letter spacing.
