//! Hyprland's event socket (socket2), for the glass plugin's own lines that
//! services::hypr and services::clients don't follow: tessera-drag (a
//! window dragged to an edge: the snap preview) and tessera-controls (a rest
//! on a window's green button: the tile grid). A thread reads the lines and
//! turns the ones a part wants into its messages (`each` runs on that
//! thread, so it may ask Hyprland for more without holding the UI).

use crate::Msg;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::any::Any;
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::time::Duration;

pub fn follow(part: &'static str, tx: Sender<Msg>, mut each: impl FnMut(&str) -> Option<Box<dyn Any + Send>> + Send + 'static) {
    std::thread::Builder::new()
        .name(format!("tessera-{part}-events"))
        .spawn(move || loop {
            let Some(path) = crate::services::hypr::dir().map(|d| d.join(".socket2.sock")) else { return };
            let Ok(stream) = UnixStream::connect(path) else {
                std::thread::sleep(Duration::from_secs(1));
                continue;
            };
            let mut r = BufReader::new(stream);
            let mut line = String::new();
            loop {
                line.clear();
                match r.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
                if let Some(m) = each(line.trim_end()) {
                    if tx.send(Msg::Part(part, m)).is_err() {
                        return;
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(500));
        })
        .expect("events thread");
}
