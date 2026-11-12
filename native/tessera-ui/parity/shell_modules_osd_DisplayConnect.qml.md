# shell/modules/osd/DisplayConnect.qml

Parity (12 Nov, written by the lead; not built or tested yet).

Rust: surfaces/screenfx/src/connect.rs + ui/connect.slint (rust.surfaces: displayconnect).
- Extend / liquid / warp from the shared edge, neighbours light at the edge, timings 1100/1050/950/1000/800 ms × motion, 220 ms fade out, reduce motion fades only: ported earlier (see connect.rs header). Not changed this round.
