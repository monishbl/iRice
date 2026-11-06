//! A cloud drive as a folder (DriveGroup.qml; services/CloudDrives.qml,
//! rclone): Settings → Google and Settings → Microsoft 365 each show one.
//! The drives are the shell's (feed.about.drives); installing rclone,
//! connecting, mounting and disconnecting are asked of it (`drives.*`).
//!   `super::super::about::drive::group(cx, p, "gdrive")` (or "onedrive")

use super::super::super::model::{btn, Ctx, Page, Row};
use super::{b, feed, s};
use serde_json::{json, Value};

/// The folder's name in the home folder ("~/Google Drive")
pub fn folder_name(folder: &str) -> String {
    format!("~/{}", folder.rsplit('/').next().unwrap_or(folder))
}

pub fn group(cx: &mut Ctx, p: &mut Page, drive: &str) {
    let f = feed(cx);
    let dr = &f["drives"];
    let name = {
        let n = s(&dr["names"][drive]);
        if n.is_empty() { if drive == "gdrive" { "Google Drive".to_string() } else { "OneDrive".to_string() } } else { n }
    };
    let d = &dr[drive];
    let busy = s(&dr["busy"]);
    let working = busy.ends_with(&format!(" {drive}"));
    let error = s(&dr["errors"][drive]);
    let connected = b(&d["connected"]);
    let mounted = b(&d["mounted"]);
    let has_rclone = b(&dr["hasRclone"]);
    let installing = b(&dr["installing"]);
    let install_error = s(&dr["installError"]);
    let connecting = busy == format!("connect {drive}");

    let footer = if !error.is_empty() {
        error
    } else if connecting {
        "Sign in in your browser and allow access, then come back here…".to_string()
    } else if connected {
        "Files open on demand from the cloud; ones you use are kept on this computer (up to 2 GB) and changes upload in the background. It comes back at every login.".into()
    } else {
        format!("{name} as a folder in Files, file pickers and the terminal. It signs in separately from the account above (through rclone).")
    };

    let install = cx.on(|cx, _| cx.ask("about", "drives.install", Value::Null));
    let mut buttons = Vec::new();
    let mut acts: Vec<(&'static str, &'static str)> = Vec::new();
    if connected && mounted {
        buttons.push(btn("Open").icon("folder-open"));
        acts.push(("drives.open", ""));
    }
    if connected && !mounted {
        buttons.push(btn("Mount").on(working).off(!busy.is_empty()));
        acts.push(("drives.act", "mount"));
    }
    if connected {
        buttons.push(btn("Disconnect").danger().off(!busy.is_empty()));
        acts.push(("drives.act", "disconnect"));
    }
    if !connected {
        buttons.push(btn(if working { "Waiting for the browser…".to_string() } else { format!("Connect {name}") }).on(working).off(!busy.is_empty()));
        acts.push(("drives.act", "connect"));
    }
    if connecting {
        buttons.push(btn("Cancel"));
        acts.push(("drives.cancel", ""));
    }
    let dv = drive.to_string();
    let act = cx.on(move |cx, a| match acts.get(a.i as usize) {
        Some(("drives.open", _)) => cx.ask("about", "drives.open", json!([dv])),
        Some(("drives.act", what)) => cx.ask("about", "drives.act", json!([what, dv])),
        Some((w, _)) => cx.ask("about", w, Value::Null),
        None => {}
    });
    p.group(format!("{name} folder"))
        .footer(footer)
        .row(Row::new(format!("{name} as a folder"))
            .visible(!has_rclone)
            .tone(if install_error.is_empty() { 0 } else { 2 })
            .detail(if install_error.is_empty() { "Needs rclone. It asks for your password once.".to_string() } else { install_error })
            .buttons(vec![btn(if installing { "Installing…" } else { "Install" }).icon("download-simple").on(installing).off(b(&dr["updatesRunning"]))], install))
        .row(Row::new(if connected { folder_name(&s(&d["folder"])) } else { format!("{name} as a folder") })
            .visible(has_rclone)
            .detail(if working { "Working…" } else if connected { if mounted { "Connected" } else { "Connected · not mounted" } } else { "Not connected" })
            .buttons(buttons, act));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_folder_by_its_name() {
        assert_eq!(folder_name("/home/sam/Google Drive"), "~/Google Drive");
    }
}
