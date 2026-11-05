//! The periodic table, read from the QML's own data
//! (shell/modules/cheatsheet/elements.js, compiled in): each element's
//! number, symbol, name, mass, category, row and column (lanthanides and
//! actinides on rows 9 and 10, shown below the main table).

const JS: &str = include_str!("../../../../../shell/modules/cheatsheet/elements.js");

#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    pub number: u32,
    pub symbol: String,
    pub name: String,
    pub mass: String,
    pub category: String,
    pub row: u32,
    pub col: u32,
}

/// (key, name, tint index) in elements.js's order
pub fn categories() -> Vec<(String, String, usize)> {
    let Some(start) = JS.find("var categories") else { return Vec::new() };
    let body = &JS[start..];
    let body = &body[body.find('{').map_or(0, |i| i + 1)..body.find('}').unwrap_or(body.len())];
    let mut out = Vec::new();
    for part in body.split(']') {
        let Some((key, rest)) = part.split_once(':') else { continue };
        let key = key.trim().trim_start_matches(',').trim();
        let Some((name, idx)) = rest.trim().trim_start_matches('[').rsplit_once(',') else { continue };
        let name = name.trim().trim_matches('"').to_string();
        if let Ok(i) = idx.trim().parse::<usize>() {
            out.push((key.to_string(), name, i));
        }
    }
    out
}

pub fn elements() -> Vec<Element> {
    let Some(start) = JS.find("var elements") else { return Vec::new() };
    let body = &JS[start..];
    let end = body.find("];").unwrap_or(body.len());
    let body = &body[body.find('[').map_or(0, |i| i + 1)..end];
    let mut out = Vec::new();
    for entry in body.split(']') {
        let Some(i) = entry.find('[') else { continue };
        let f: Vec<&str> = entry[i + 1..].split(',').map(|s| s.trim().trim_matches('"')).collect();
        if f.len() < 7 {
            continue;
        }
        let (Ok(number), Ok(row), Ok(col)) = (f[0].parse(), f[5].parse(), f[6].parse()) else { continue };
        out.push(Element { number, symbol: f[1].into(), name: f[2].into(), mass: f[3].into(), category: f[4].into(), row, col });
    }
    out
}

/// Where an element's cell is, in cells (Cheatsheet.qml: the f-block two
/// rows down, under a gap)
pub fn cell_xy(e: &Element) -> (f32, f32) {
    let x = e.col as f32 - 1.0;
    let y = if e.row <= 7 { e.row as f32 - 1.0 } else { e.row as f32 - 2.6 };
    (x, y)
}

/// The category tints (chart palette, quiet), dark then light
pub const TINTS_DARK: [&str; 10] = ["#F5886C", "#E7963D", "#93A3FC", "#6EC272", "#25C2C2", "#BF93F1", "#F3829F", "#E087CE", "#8FB8FF", "#C6A26A"];
pub const TINTS_LIGHT: [&str; 10] = ["#D05F43", "#BE7100", "#6D7CDF", "#409D48", "#1D9999", "#9B6BCE", "#CE597A", "#BB5FAA", "#4F7FD8", "#8F6F3A"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_whole_table() {
        let e = elements();
        assert_eq!(e.len(), 118);
        assert_eq!(e[0], Element { number: 1, symbol: "H".into(), name: "Hydrogen".into(), mass: "1.008".into(), category: "nm".into(), row: 1, col: 1 });
        let og = e.iter().find(|x| x.number == 118).unwrap();
        assert_eq!((og.symbol.as_str(), og.row, og.col), ("Og", 7, 18));
        let la = e.iter().find(|x| x.symbol == "La").unwrap();
        assert_eq!(cell_xy(la), (2.0, 9.0 - 2.6));
        let c = categories();
        assert_eq!(c.len(), 10);
        assert_eq!(c[0], ("am".into(), "Alkali metal".into(), 0));
        assert_eq!(c[9], ("ac".into(), "Actinide".into(), 9));
        // every element's category is one of them
        assert!(e.iter().all(|x| c.iter().any(|k| k.0 == x.category)));
    }
}
