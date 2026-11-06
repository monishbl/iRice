//! What About shows of this computer, read as services/SystemInfo.qml reads
//! it: /etc/os-release, /proc/cpuinfo, /proc/meminfo, the hostname, glxinfo's
//! renderer; and your name as the account has it (Today greets you by it).

use serde_json::{json, Value};

/// A `KEY="value"` line of an os-release file
pub fn field(text: &str, key: &str) -> String {
    text.lines()
        .find_map(|l| l.strip_prefix(key).and_then(|r| r.strip_prefix('=')))
        .map(|v| v.trim().trim_matches('"').to_string())
        .unwrap_or_default()
}

pub fn os_name(os_release: &str) -> String {
    let p = field(os_release, "PRETTY_NAME");
    if !p.is_empty() {
        return p;
    }
    let n = field(os_release, "NAME");
    if n.is_empty() { "Linux".into() } else { n }
}

/// The processor's model name, without (R) and (TM)
pub fn cpu(cpuinfo: &str) -> String {
    let m = cpuinfo.lines().find_map(|l| l.strip_prefix("model name").map(|r| r.trim_start().trim_start_matches(':').trim().to_string())).unwrap_or_default();
    m.replace("(R)", "").replace("(TM)", "").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The memory, in whole GB
pub fn memory(meminfo: &str) -> String {
    let kb: f64 = meminfo.lines().find_map(|l| l.strip_prefix("MemTotal:")).and_then(|r| r.split_whitespace().next()).and_then(|n| n.parse().ok()).unwrap_or(0.0);
    format!("{} GB", (kb / 1024.0 / 1024.0).round() as i64)
}

/// glxinfo -B's renderer, without the driver's note and "Mesa "
pub fn gpu(glxinfo: &str) -> String {
    let r = glxinfo.lines().find_map(|l| l.strip_prefix("OpenGL renderer string: ")).unwrap_or("");
    let r = r.split(" (").next().unwrap_or("").trim();
    r.strip_prefix("Mesa ").unwrap_or(r).to_string()
}

/// A passwd line's full name (the GECOS field's first part)
pub fn gecos_name(passwd: &str, user: &str) -> String {
    passwd
        .lines()
        .find(|l| l.split(':').next() == Some(user))
        .and_then(|l| l.split(':').nth(4))
        .map(|g| g.split(',').next().unwrap_or("").trim().to_string())
        .unwrap_or_default()
}

/// Everything, read now (blocking: off the UI thread)
pub fn read() -> Value {
    let r = |p: &str| std::fs::read_to_string(p).unwrap_or_default();
    let glx = std::process::Command::new("glxinfo").arg("-B").stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
    let user = std::env::var("USER").unwrap_or_default();
    json!({
        "os": os_name(&r("/etc/os-release")),
        "host": r("/proc/sys/kernel/hostname").trim(),
        "cpu": cpu(&r("/proc/cpuinfo")),
        "memory": memory(&r("/proc/meminfo")),
        "gpu": gpu(&glx),
        "name": gecos_name(&r("/etc/passwd"), &user),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_as_system_info_qml() {
        assert_eq!(os_name("NAME=\"Arch Linux\"\nPRETTY_NAME=\"Arch Linux\"\n"), "Arch Linux");
        assert_eq!(os_name("NAME=Foo\n"), "Foo");
        assert_eq!(os_name(""), "Linux");
        assert_eq!(cpu("processor\t: 0\nmodel name\t: Intel(R) Core(TM) i7-1165G7 @ 2.80GHz\n"), "Intel Core i7-1165G7 @ 2.80GHz");
        assert_eq!(memory("MemTotal:       16163084 kB\n"), "15 GB");
        assert_eq!(gpu("OpenGL renderer string: Mesa Intel(R) Xe Graphics (TGL GT2)\n"), "Intel(R) Xe Graphics");
        assert_eq!(gecos_name("root:x:0:0::/root:/bin/bash\nsam:x:1000:1000:Sam Doe,,,:/home/sam:/bin/fish\n", "sam"), "Sam Doe");
    }
}
