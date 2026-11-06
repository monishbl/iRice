//! shell/services/SystemInfo.qml: static facts about the machine, read once
//! (os-release, cpuinfo, meminfo, hostname, osrelease, glxinfo's renderer).
//! Published on the bus as "systeminfo": {osName, osLogo, hostname, kernel,
//! cpu, memory, gpu} (the QML property names).

use crate::util;
use serde_json::{json, Value};
use std::time::Duration;
use tessera_core::host::{Host, Service};

/// `KEY="value"` of an os-release style text (SystemInfo.field)
pub fn field(text: &str, key: &str) -> String {
    for l in text.lines() {
        if let Some(r) = l.strip_prefix(key).and_then(|r| r.strip_prefix('=')) {
            let r = r.strip_prefix('"').unwrap_or(r);
            return r.split(['"', '\n']).next().unwrap_or("").to_string();
        }
    }
    String::new()
}

/// The CPU model name, as the QML tidies it ((R)/(TM) dropped, spaces folded)
pub fn cpu_name(cpuinfo: &str) -> String {
    let m = cpuinfo.lines().find_map(|l| l.strip_prefix("model name").map(|r| r.trim_start().trim_start_matches(':').trim_start()));
    m.unwrap_or("").replace("(R)", "").replace("(TM)", "").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// "16 GB" from /proc/meminfo's MemTotal
pub fn memory(meminfo: &str) -> String {
    let kb: f64 = meminfo.lines().find_map(|l| l.strip_prefix("MemTotal:")).and_then(|r| r.split_whitespace().next()).and_then(|n| n.parse().ok()).unwrap_or(0.0);
    format!("{} GB", (kb / 1024.0 / 1024.0).round())
}

fn read() -> Value {
    let rd = |p: &str| std::fs::read_to_string(p).unwrap_or_default();
    let os = rd("/etc/os-release");
    let name = {
        let p = field(&os, "PRETTY_NAME");
        if !p.is_empty() {
            p
        } else {
            let n = field(&os, "NAME");
            if n.is_empty() {
                "Linux".into()
            } else {
                n
            }
        }
    };
    let gpu = util::sh("glxinfo -B 2>/dev/null | sed -n 's/^OpenGL renderer string: //p' | sed 's/ (.*//'", Duration::from_secs(10)).map(|(_, o)| o.trim().trim_start_matches("Mesa ").to_string()).unwrap_or_default();
    json!({
        "osName": name,
        "osLogo": field(&os, "LOGO"),
        "hostname": rd("/proc/sys/kernel/hostname").trim(),
        "kernel": rd("/proc/sys/kernel/osrelease").trim(),
        "cpu": cpu_name(&rd("/proc/cpuinfo")),
        "memory": memory(&rd("/proc/meminfo")),
        "gpu": gpu,
    })
}

#[derive(Default)]
pub struct SystemInfo;

struct Done(Value);

impl Service for SystemInfo {
    fn name(&self) -> &'static str {
        "SystemInfo"
    }
    fn calls(&self) -> &'static [&'static str] {
        &["systeminfo"]
    }
    fn start(&mut self, h: &mut Host) {
        // (the defaults until the thread has read: osName "Linux")
        crate::bus::put("systeminfo", json!({"osName": "Linux", "osLogo": "", "hostname": "", "kernel": "", "cpu": "", "memory": "", "gpu": ""}));
        let send = h.sender("SystemInfo");
        std::thread::spawn(move || send(Box::new(Done(read()))));
    }
    fn message(&mut self, _h: &mut Host, msg: Box<dyn std::any::Any + Send>) {
        if let Ok(d) = msg.downcast::<Done>() {
            crate::bus::put("systeminfo", d.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses() {
        let os = "NAME=\"Arch Linux\"\nPRETTY_NAME=\"Arch Linux\"\nLOGO=archlinux-logo\n";
        assert_eq!(field(os, "PRETTY_NAME"), "Arch Linux");
        assert_eq!(field(os, "LOGO"), "archlinux-logo");
        assert_eq!(field(os, "NOPE"), "");
        assert_eq!(cpu_name("model name\t: AMD Ryzen(TM) 7  5800X\n"), "AMD Ryzen 7 5800X");
        assert_eq!(memory("MemTotal:       16384000 kB\n"), "16 GB");
    }
}
