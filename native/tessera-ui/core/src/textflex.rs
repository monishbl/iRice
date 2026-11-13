//! "Tessera Text Flex": Material's text face, drawn the way the QML shell's
//! Qt draws it.
//!
//! The QML loads Google Sans Flex ("Tessera Sans Flex") with a FontLoader,
//! and Qt (6.12) maps no `font.weight` onto an application font's `wght`
//! axis: every Text without `font.variableAxes` is drawn at the default
//! instance (wght 400), and a weight of Bold or more adds Qt's synthetic
//! bold (FreeType's embolden: the outline grown, the advances unchanged).
//! Medium and DemiBold look exactly like Regular there. Slint/parley map
//! `font-weight` straight onto `wght`, so the same text came out a weight
//! or two heavier and wider in the Rust shell.
//!
//! This makes a copy of the font, in memory, that behaves like Qt's:
//! - renamed "Tessera Text Flex" (same length, so the name table keeps its
//!   layout); "Tessera Sans Flex" stays the true variable font for what the
//!   QML drives through its axes (the lock and StandBy clocks, previews);
//! - an `avar` that maps every weight under 700 to the default (400) and
//!   every weight from 700 to the smallest step above it (1/16384: outlines
//!   and advances indistinguishable from 400) — a marker the renderer
//!   (vendored femtovg, text.rs `faux_bold`) reads as "embolden like Qt";
//! - a private `TSfb` table naming the `wght` axis, so femtovg knows the
//!   marker is ours and not a real weight.

const OLD: &[u8] = b"Tessera Sans Flex";
const NEW: &[u8] = b"Tessera Text Flex";
const OLD_PS: &[u8] = b"TesseraSansFlex";
const NEW_PS: &[u8] = b"TesseraTextFlex";

fn be16(d: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_be_bytes(d.get(o..o + 2)?.try_into().ok()?))
}
fn be32(d: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(o..o + 4)?.try_into().ok()?))
}

fn replace_all(buf: &mut [u8], from: &[u8], to: &[u8]) {
    debug_assert_eq!(from.len(), to.len());
    let mut i = 0;
    while i + from.len() <= buf.len() {
        if &buf[i..i + from.len()] == from {
            buf[i..i + from.len()].copy_from_slice(to);
            i += from.len();
        } else {
            i += 1;
        }
    }
}

fn utf16(s: &[u8]) -> Vec<u8> {
    s.iter().flat_map(|&c| [0u8, c]).collect()
}

fn checksum(t: &[u8]) -> u32 {
    t.chunks(4).fold(0u32, |s, c| {
        let mut w = [0u8; 4];
        w[..c.len()].copy_from_slice(c);
        s.wrapping_add(u32::from_be_bytes(w))
    })
}

/// The Qt-like copy of Google Sans Flex, or None if the file isn't the
/// variable font this expects (then Material text keeps the plain font).
pub fn derive(data: &[u8]) -> Option<Vec<u8>> {
    if be32(data, 0)? != 0x0001_0000 {
        return None;
    }
    let n = be16(data, 4)? as usize;
    let mut tables: Vec<([u8; 4], Vec<u8>)> = Vec::with_capacity(n + 2);
    for i in 0..n {
        let r = 12 + 16 * i;
        let tag: [u8; 4] = data.get(r..r + 4)?.try_into().ok()?;
        let (off, len) = (be32(data, r + 8)? as usize, be32(data, r + 12)? as usize);
        tables.push((tag, data.get(off..off + len)?.to_vec()));
    }
    let fvar = &tables.iter().find(|(t, _)| t == b"fvar")?.1;
    let (axes_off, axis_count, axis_size) = (be16(fvar, 4)? as usize, be16(fvar, 8)? as usize, be16(fvar, 10)? as usize);
    let wght = (0..axis_count).find(|&i| fvar.get(axes_off + i * axis_size..axes_off + i * axis_size + 4) == Some(b"wght"))?;

    // avar 1.0: identity for every axis but wght
    let mut avar = vec![0, 1, 0, 0, 0, 0];
    avar.extend((axis_count as u16).to_be_bytes());
    for i in 0..axis_count {
        let map: &[(i16, i16)] = if i == wght {
            // (-1 → -1 and +1 → +1 kept as the spec asks; a weight of 1 or
            // of 1000 exactly is never asked for by the shell)
            &[(-16384, -16384), (-16383, 0), (8191, 0), (8192, 1), (16383, 1), (16384, 16384)]
        } else {
            &[(-16384, -16384), (0, 0), (16384, 16384)]
        };
        avar.extend((map.len() as u16).to_be_bytes());
        for &(f, t) in map {
            avar.extend(f.to_be_bytes());
            avar.extend(t.to_be_bytes());
        }
    }
    tables.retain(|(t, _)| t != b"avar" && t != b"TSfb" && t != b"DSIG");
    tables.push((*b"avar", avar));
    tables.push((*b"TSfb", vec![wght as u8, 0, 0, 0]));

    let name = &mut tables.iter_mut().find(|(t, _)| t == b"name")?.1;
    replace_all(name, OLD, NEW);
    replace_all(name, OLD_PS, NEW_PS);
    replace_all(name, &utf16(OLD), &utf16(NEW));
    replace_all(name, &utf16(OLD_PS), &utf16(NEW_PS));

    tables.sort_by(|a, b| a.0.cmp(&b.0));
    let n = tables.len();
    let pow2 = 1usize << (usize::BITS - 1 - n.leading_zeros());
    let mut out = Vec::with_capacity(data.len() + 512);
    out.extend(0x0001_0000u32.to_be_bytes());
    out.extend((n as u16).to_be_bytes());
    out.extend(((pow2 * 16) as u16).to_be_bytes());
    out.extend((pow2.trailing_zeros() as u16).to_be_bytes());
    out.extend(((n * 16 - pow2 * 16) as u16).to_be_bytes());
    let mut off = 12 + 16 * n;
    for (tag, t) in &tables {
        out.extend(tag);
        out.extend(checksum(t).to_be_bytes());
        out.extend((off as u32).to_be_bytes());
        out.extend((t.len() as u32).to_be_bytes());
        off += (t.len() + 3) & !3;
    }
    for (_, t) in &tables {
        out.extend(t);
        out.resize((out.len() + 3) & !3, 0);
    }
    Some(out)
}
