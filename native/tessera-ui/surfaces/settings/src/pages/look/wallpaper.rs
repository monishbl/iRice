//! Wallpaper (pages/wallpaper.qml): the picture showing, Choose Picture…,
//! the sets in the wallpaper folder, dynamic and live wallpapers, the login
//! screen's copy, the folder's pictures and the bundled ones.

use super::super::super::model::{Ctx, Opt, Page, Row};
use super::{feed, idx, place, wallpaper_path};
use serde_json::{json, Value};

const IMAGES: &str = "Images|*.jpg *.jpeg *.png *.webp";

/// A path row's action for a string setting (PathField.qml), its chooser
/// starting at `start` while the setting is empty
pub fn path_action(cx: &mut Ctx, key: &str, folder: bool, filter: &str, start: &str) -> String {
    let (key, filter, start) = (key.to_string(), filter.to_string(), start.to_string());
    cx.on(move |cx, a| {
        if a.i == 2 {
            let cur = cx.s(&key);
            let from = if cur.is_empty() { start.clone() } else if folder { cur } else { cur.rsplit_once('/').map_or(cur.clone(), |x| x.0.to_string()) };
            cx.choose_file(&a.id, folder, if folder { "Choose a folder" } else { "Choose a file" }, &from, &filter);
        } else if a.i == 1 {
            let home = std::env::var("HOME").unwrap_or_default();
            let t = a.s.trim();
            let p = match t.strip_prefix('~') {
                Some(rest) => format!("{home}{rest}"),
                None => t.to_string(),
            };
            if p != cx.s(&key) {
                cx.set(&key, p);
            }
        }
    })
}

/// The pictures in a folder, by name (FolderListModel: these extensions, no folders)
pub fn pictures_in(dir: &str, exts: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().map(|t| !t.is_dir()).unwrap_or(false))
        .filter_map(|e| {
            let p = e.path();
            let name = p.file_name()?.to_str()?.to_string();
            exts.iter().any(|x| name.ends_with(x)).then(|| p.to_string_lossy().into_owned())
        })
        .collect();
    out.sort();
    out
}

/// Whether a set is the one in use (Wallpaper.isCurrentSet)
pub fn is_current_set(s: &Value, dynamic: &str, light: &str, dark: &str, folder: &str) -> bool {
    if s["kind"] == "pair" {
        dynamic == "appearance" && s["light"].as_str() == Some(light) && s["dark"].as_str() == Some(dark)
    } else {
        dynamic == "time" && s["folder"].as_str() == Some(folder)
    }
}

pub fn wallpaper(cx: &mut Ctx, p: &mut Page) {
    let path = wallpaper_path(cx);
    if !path.is_empty() {
        if let Some(i) = cx.picture(&path, 840, 472) {
            p.custom("look", "wallpaper-preview").image(i);
        } else {
            p.custom("look", "wallpaper-preview");
        }
    }

    let busy = feed(cx)["pickerBusy"].as_bool().unwrap_or(false);
    let choose = cx.on(|cx, a| {
        if a.i == 1 {
            if !a.s.is_empty() {
                cx.ask("look", "setWallpaper", json!([a.s]));
            }
        } else {
            cx.choose_file(&a.id, false, "Choose a wallpaper", "~/Pictures", IMAGES);
        }
    });
    p.group("").row(Row::new("Picture").detail("Any image on your computer, or pick one below").button(if busy { "Choosing…" } else { "Choose Picture…" }, choose).enabled(!busy));

    // Light/dark pairs and time-of-day folders found in the wallpaper folder
    let sets: Vec<Value> = feed(cx)["sets"].as_array().cloned().unwrap_or_default();
    if !sets.is_empty() {
        let (dynamic, light, dark, folder) = (cx.s("wallpaper.dynamic"), cx.s("wallpaper.lightPath"), cx.s("wallpaper.darkPath"), cx.s("wallpaper.dynamicFolder"));
        let mut data = Vec::new();
        let mut darks = Vec::new();
        for s in &sets {
            let pair = s["kind"] == "pair";
            let mut o = Opt::new(s["name"].as_str().unwrap_or("")).detail(if pair { "Light & dark" } else { "Time of day" }).on(is_current_set(s, &dynamic, &light, &dark, &folder)).style(pair as i32);
            if let Some(i) = s["preview"].as_str().and_then(|p| cx.picture(p, 300, 180)) {
                o = o.image(i);
            }
            let mut d = Opt::new("");
            if pair {
                if let Some(i) = s["dark"].as_str().and_then(|p| cx.picture(p, 300, 180)) {
                    d = d.image(i);
                }
            }
            data.push(o);
            darks.push(d);
        }
        let use_set = cx.on(move |cx, a| {
            let Some(s) = sets.get(a.i.max(0) as usize) else { return };
            // (Wallpaper.useSet)
            cx.set("wallpaper.live", "");
            if s["kind"] == "pair" {
                cx.set("wallpaper.lightPath", s["light"].clone());
                cx.set("wallpaper.darkPath", s["dark"].clone());
                cx.set("wallpaper.dynamic", "appearance");
            } else {
                cx.set("wallpaper.dynamicFolder", s["folder"].clone());
                cx.set("wallpaper.dynamic", "time");
            }
        });
        let mut r = Row::custom("look", "sets").id(use_set).data(data).num(place(true, true));
        r.opts = darks;
        p.group("Wallpaper sets")
            .footer("Pairs named “Name Light” and “Name Dark” switch with light and dark mode; a sub-folder of pictures follows the time of day.")
            .row(r);
    }

    let dynamic = cx.s("wallpaper.dynamic");
    let dv = ["off", "time", "appearance"];
    let dp = cx.pick("wallpaper.dynamic", &dv);
    let folder = path_action(cx, "wallpaper.dynamicFolder", true, "", "~/Pictures");
    let light = path_action(cx, "wallpaper.lightPath", false, IMAGES, "~/Pictures");
    let dark = path_action(cx, "wallpaper.darkPath", false, IMAGES, "~/Pictures");
    p.group("Dynamic")
        .footer(match dynamic.as_str() {
            "time" => "Follows sunrise and sunset for your weather location. Choosing a picture below turns this off.",
            "appearance" => "Switches with light and dark mode (and Auto appearance).",
            _ => "",
        })
        .row(Row::new("Change automatically").segmented(&["Off", "Time of day", "Light & dark"], idx(&dv, &dynamic), dp).width(300.0))
        .row(Row::new("Pictures")
            .visible(dynamic == "time")
            .detail("A folder of pictures in name order: the first half spans the day, the rest the night. Empty uses the built-in set.")
            .path(cx.s("wallpaper.dynamicFolder"), "Built-in set", folder))
        .row(Row::new("Light picture").visible(dynamic == "appearance").path(cx.s("wallpaper.lightPath"), "Built-in day", light))
        .row(Row::new("Dark picture").visible(dynamic == "appearance").path(cx.s("wallpaper.darkPath"), "Built-in dusk", dark));

    let live = cx.s("wallpaper.live");
    let poster = feed(cx)["livePoster"].as_str().unwrap_or("").to_string();
    let lp = path_action(cx, "wallpaper.live", false, "Videos|*.mp4 *.webm *.mkv *.mov", "~/Videos");
    p.group("Live wallpaper")
        .footer("Plays muted and loops. It pauses while windows cover most of the screen, so it only costs power while you can see it.")
        .row(Row::new("Video")
            .detail(if !live.is_empty() && poster.is_empty() { "Couldn't read this video. Check the path, or install ffmpeg." } else { "An .mp4, .webm or .mkv file" })
            .path(live, "~/Videos/wallpaper.mp4", lp));

    // The login screen keeps its own copy (./install.sh --greeter); the
    // shell copies it (its root helper, else the password)
    let installed = std::path::Path::new("/usr/share/tessera").is_dir() || std::path::Path::new("/usr/share/irice").is_dir();
    let passwordless = feed(cx)["rootHelper"].as_bool().unwrap_or(false);
    let status = feed(cx)["loginStatus"].as_str().unwrap_or("").to_string();
    let copy = cx.on(|cx, _| cx.ask("look", "loginWallpaper", Value::Null));
    p.group("Login screen")
        .footer(if installed { status } else { "Install the Tessera login screen with ./install.sh --greeter.".into() })
        .row(Row::new("Use this wallpaper on the login screen")
            .visible(installed)
            .detail(if passwordless { "Copies it to the login screen." } else { "Asks for your password to update the login screen's copy." })
            .button("Use wallpaper", copy));

    // The folder's pictures, then the bundled ones
    let dir = feed(cx)["directory"].as_str().map(str::to_string).filter(|d| !d.is_empty()).unwrap_or_else(|| {
        let d = cx.s("wallpaper.directory");
        if d.is_empty() { format!("{}/Pictures/Wallpapers", std::env::var("HOME").unwrap_or_default()) } else { d }
    });
    let bundled = crate::util::asset("shell/assets/wallpapers").map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    let key = format!("look.pictures:{dir}");
    let d2 = dir.clone();
    let list: Vec<String> = cx
        .once(&key, move || {
            let mut v = pictures_in(&d2, &[".jpg", ".jpeg", ".png", ".webp", ".JPG", ".PNG"]);
            v.extend(pictures_in(&bundled, &[".jpg"]));
            Value::from(v)
        })
        .as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    if !list.is_empty() {
        let thumbs: Vec<Opt> = list
            .iter()
            .map(|f| {
                let o = Opt::new("").on(*f == path).id(f.clone());
                match cx.picture(f, 320, 180) {
                    Some(i) => o.image(i),
                    None => o,
                }
            })
            .collect();
        let set = cx.on(move |cx, a| {
            if let Some(f) = list.get(a.i.max(0) as usize) {
                cx.ask("look", "setWallpaper", json!([f]));
            }
        });
        p.custom("look", "wallpaper-grid").data(thumbs).id(set);
    }

    // ── More options ──
    p.more(cx);
    {
        let fa = path_action(cx, "wallpaper.directory", true, "", "~/Pictures");
        p.group("").row(Row::new("Folder").detail("Images in this folder appear below and in the wallpaper picker.").path(cx.s("wallpaper.directory"), "~/Pictures/Wallpapers", fa));
        let pause = cx.b("wallpaper.livePauseOnBattery");
        let pt = cx.toggle("wallpaper.livePauseOnBattery");
        p.group("Live wallpaper").row(Row::new("Pause on battery").switch(pause, pt));
    }
    p.end_more();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_set_in_use() {
        let pair = json!({"kind": "pair", "light": "/a", "dark": "/b"});
        assert!(is_current_set(&pair, "appearance", "/a", "/b", ""));
        assert!(!is_current_set(&pair, "time", "/a", "/b", ""));
        let time = json!({"kind": "time", "folder": "/f"});
        assert!(is_current_set(&time, "time", "", "", "/f"));
    }

    #[test]
    fn pictures_in_a_folder_by_name() {
        let d = std::env::temp_dir().join(format!("tessera-wp-{}", std::process::id()));
        std::fs::create_dir_all(d.join("sub")).unwrap();
        for f in ["b.png", "a.jpg", "c.txt"] {
            std::fs::write(d.join(f), b"x").unwrap();
        }
        let got = pictures_in(d.to_str().unwrap(), &[".jpg", ".png"]);
        assert_eq!(got.iter().map(|p| p.rsplit('/').next().unwrap()).collect::<Vec<_>>(), ["a.jpg", "b.png"]);
        let _ = std::fs::remove_dir_all(&d);
    }
}
