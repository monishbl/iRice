//! Default Apps (pages/defaultapps.qml): what the shortcuts and the shell
//! open. Each list offers the ones installed (Tools.has: the shell says,
//! feed.accounts.tools; else this process looks on the PATH).

use super::super::super::model::{Ctx, Page, Row};
use super::feed;
use serde_json::Value;

/// Whether a program is on the PATH
fn on_path(name: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(name).is_file()))
}

/// The choices a list offers: Automatic, then the installed ones (a
/// command's last word is the program: "kitty -e btop" needs btop)
pub fn choices(auto: &str, all: &[&str], has: impl Fn(&str) -> bool) -> Vec<(String, String)> {
    let mut out = vec![(String::new(), auto.to_string())];
    for c in all {
        if has(c.split(' ').next_back().unwrap_or(c)) {
            out.push((c.to_string(), c.to_string()));
        }
    }
    out
}

fn row(cx: &mut Ctx, label: &str, keys: &str, key: &str, auto: &str, all: &[&str]) -> Row {
    let f = feed(cx);
    let told: Option<Vec<String>> = f["tools"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect());
    let list = choices(auto, all, |p| match &told {
        Some(t) => t.iter().any(|x| x == p),
        None => on_path(p),
    });
    let path = format!("defaultApps.{key}");
    let cur = cx.s(&path);
    let index = list.iter().position(|(v, _)| *v == cur).map_or(-1, |i| i as i32);
    let labels: Vec<String> = list.iter().map(|c| c.1.clone()).collect();
    let pick = cx.pick_values(&path, list.into_iter().map(|c| Value::from(c.0)).collect());
    // (a value no longer installed shows as itself, as Dropdown's label does)
    let mut r = Row::new(label).detail(keys).choice(&labels, index, pick).width(220.0);
    if index < 0 {
        r = r.placeholder(cur);
    }
    r
}

pub fn page(cx: &mut Ctx, p: &mut Page) {
    let rows = vec![
        row(cx, "Terminal", "Super+Return", "terminal", "Automatic", &["kitty", "foot", "alacritty", "wezterm", "ghostty", "konsole", "gnome-terminal", "kgx", "ptyxis", "xfce4-terminal", "xterm"]),
        row(cx, "Browser", "Super+W", "browser", "Automatic (system default)", &["zen-browser", "firefox", "google-chrome-stable", "chromium", "brave"]),
        row(cx, "Code editor", "Super+C", "code", "Automatic", &["code", "codium", "zed", "cursor", "windsurf"]),
        row(cx, "Text editor", "Super+X", "text", "Automatic", &["gnome-text-editor", "kate", "mousepad"]),
    ];
    let g = p.group("Default apps");
    g.footer("Used by keyboard shortcuts and the shell. Automatic picks the first one installed. The Files app is set in Desktop & Widgets.");
    for r in rows {
        g.row(r);
    }
    p.more(cx);
    let rows = vec![
        row(cx, "Volume mixer", "Ctrl+Super+V", "mixer", "Automatic", &["pavucontrol", "pavucontrol-qt"]),
        row(cx, "Task manager", "Ctrl+Shift+Escape", "tasks", "Automatic", &["gnome-system-monitor", "plasma-systemmonitor", "kitty -e btop", "kitty -e htop"]),
    ];
    let g = p.group("Default apps");
    for r in rows {
        g.row(r);
    }
    p.end_more();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_installed_ones_are_offered() {
        let c = choices("Automatic", &["kitty", "foot", "kitty -e btop"], |p| p == "kitty" || p == "btop");
        assert_eq!(c.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), ["", "kitty", "kitty -e btop"]);
        assert_eq!(c[0].1, "Automatic");
    }
}
