//! Draw on the GPU the compositor draws on. On a laptop with two (an Intel or
//! AMD one driving the screen, NVIDIA beside it), glvnd tries NVIDIA's EGL
//! first, and it accepts any Wayland display: the shell then drew every frame
//! on the NVIDIA card and the compositor copied it to the other one, at 4K and
//! 120 Hz (server1, 16 Nov: stutter, and the NVIDIA card kept awake). The
//! compositor names its GPU in linux-dmabuf's default feedback (main_device);
//! when that's a Mesa one and Mesa's EGL is installed, libEGL is told to load
//! only Mesa's, for this process's EGL start, and the setting is taken away
//! again before anything is started from here (apps keep their own choice).
//! With more than one GPU, Settings can pick another (performance.gpu).

use smithay_client_toolkit::reexports::client::{
    globals::{registry_queue_init, GlobalListContents},
    protocol::wl_registry,
    Connection, Dispatch, QueueHandle,
};
use smithay_client_toolkit::reexports::protocols::wp::linux_dmabuf::zv1::client::{
    zwp_linux_dmabuf_feedback_v1::{self, ZwpLinuxDmabufFeedbackV1},
    zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1,
};

const VENDOR_VAR: &str = "__EGL_VENDOR_LIBRARY_FILENAMES";

#[derive(Default)]
struct Feedback {
    main: Option<u64>,
    done: bool,
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Feedback {
    fn event(_: &mut Self, _: &wl_registry::WlRegistry, _: wl_registry::Event, _: &GlobalListContents, _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<ZwpLinuxDmabufV1, ()> for Feedback {
    fn event(_: &mut Self, _: &ZwpLinuxDmabufV1, _: <ZwpLinuxDmabufV1 as smithay_client_toolkit::reexports::client::Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<ZwpLinuxDmabufFeedbackV1, ()> for Feedback {
    fn event(st: &mut Self, _: &ZwpLinuxDmabufFeedbackV1, ev: zwp_linux_dmabuf_feedback_v1::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match ev {
            zwp_linux_dmabuf_feedback_v1::Event::MainDevice { device } => {
                if let Ok(b) = <[u8; 8]>::try_from(&device[..device.len().min(8)]) {
                    st.main = Some(u64::from_ne_bytes(b));
                }
            }
            zwp_linux_dmabuf_feedback_v1::Event::Done => st.done = true,
            _ => {}
        }
    }
}

/// The kernel driver of the compositor's GPU ("i915", "xe", "amdgpu",
/// "nvidia"…), None when it doesn't say (no linux-dmabuf v4)
pub fn compositor_driver(conn: &Connection) -> Option<String> {
    let (globals, mut queue) = registry_queue_init::<Feedback>(conn).ok()?;
    let qh = queue.handle();
    let dmabuf: ZwpLinuxDmabufV1 = globals.bind(&qh, 4..=5, ()).ok()?;
    let feedback = dmabuf.get_default_feedback(&qh, ());
    let mut st = Feedback::default();
    for _ in 0..4 {
        if queue.roundtrip(&mut st).is_err() || st.done {
            break;
        }
    }
    feedback.destroy();
    dmabuf.destroy();
    driver_of(st.main?)
}

/// dev_t's major and minor (glibc's gnu_dev_major/minor)
fn major_minor(dev: u64) -> (u64, u64) {
    (((dev >> 8) & 0xfff) | ((dev >> 32) & !0xfff), (dev & 0xff) | ((dev >> 12) & !0xff))
}

fn driver_of(dev: u64) -> Option<String> {
    let (maj, min) = major_minor(dev);
    let link = std::fs::read_link(format!("/sys/dev/char/{maj}:{min}/device/driver")).ok()?;
    Some(link.file_name()?.to_string_lossy().into_owned())
}

/// Mesa's glvnd vendor file, when installed
fn mesa_vendor() -> Option<String> {
    ["/usr/share/glvnd/egl_vendor.d/50_mesa.json", "/etc/glvnd/egl_vendor.d/50_mesa.json"]
        .into_iter()
        .find(|p| std::path::Path::new(p).is_file())
        .map(String::from)
}

fn nvidia_vendor() -> Option<String> {
    ["/usr/share/glvnd/egl_vendor.d/10_nvidia.json", "/etc/glvnd/egl_vendor.d/10_nvidia.json"]
        .into_iter()
        .find(|p| std::path::Path::new(p).is_file())
        .map(String::from)
}

/// A GPU's family by its kernel driver: "intel", "amd", "nvidia" (the
/// proprietary driver; nouveau is drawn through Mesa, so it counts as "nouveau")
pub fn family(driver: &str) -> &'static str {
    match driver {
        "i915" | "xe" => "intel",
        "amdgpu" | "radeon" => "amd",
        d if d.starts_with("nvidia") => "nvidia",
        "nouveau" => "nouveau",
        _ => "other",
    }
}

/// What people call it
pub fn family_label(family: &str) -> &'static str {
    match family {
        "intel" => "Intel",
        "amd" => "AMD",
        "nvidia" => "NVIDIA",
        "nouveau" => "NVIDIA (nouveau)",
        _ => "Other",
    }
}

/// This computer's GPUs: (family, PCI address), one each, in /dev/dri order
pub fn gpus() -> Vec<(&'static str, String)> {
    let mut out: Vec<(&'static str, String)> = Vec::new();
    let Ok(rd) = std::fs::read_dir("/sys/class/drm") else { return out };
    let mut cards: Vec<String> = rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.starts_with("card") && !n.contains('-')).collect();
    cards.sort();
    for c in cards {
        let dev = format!("/sys/class/drm/{c}/device");
        let Some(driver) = std::fs::read_link(format!("{dev}/driver")).ok().and_then(|l| l.file_name().map(|f| f.to_string_lossy().into_owned())) else { continue };
        let pci = std::fs::canonicalize(&dev).ok().and_then(|p| p.file_name().map(|f| f.to_string_lossy().into_owned())).unwrap_or_default();
        if !out.iter().any(|(_, p)| *p == pci) {
            out.push((family(&driver), pci));
        }
    }
    out
}

/// Settings' choice (performance.gpu): "auto" (the compositor's), or a family
fn chosen() -> String {
    let path = crate::util::xdg("XDG_CONFIG_HOME", ".config").join("tessera/config.json");
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v["performance"]["gpu"].as_str().map(String::from))
        .unwrap_or_else(|| "auto".into())
}

/// Before EGL starts: which GPU's EGL. Settings → Battery → Graphics
/// processor (performance.gpu): the compositor's by default (no copying of
/// every frame between two GPUs), or the one picked. Nothing is changed when
/// the environment already says (glvnd's variables, TESSERA_GPU=any) or
/// there's only one GPU. Returns the variables it set (`restore` takes them
/// away once EGL is up, so what the shell starts chooses for itself).
pub fn prefer_compositor_gpu(conn: &Connection) -> Vec<&'static str> {
    if std::env::var_os(VENDOR_VAR).is_some() || std::env::var_os("__EGL_VENDOR_LIBRARY_DIRS").is_some() {
        return vec![];
    }
    if crate::env::var("GPU").as_deref() == Some("any") {
        return vec![];
    }
    let all = gpus();
    if all.len() < 2 {
        return vec![];
    }
    let want = chosen();
    let target = if want == "auto" || !all.iter().any(|(f, _)| *f == want) {
        let Some(driver) = compositor_driver(conn) else { return vec![] };
        family(&driver)
    } else {
        all.iter().find(|(f, _)| *f == want).map(|(f, _)| *f).unwrap_or("other")
    };
    let mut set = vec![];
    if target == "nvidia" {
        let Some(nv) = nvidia_vendor() else { return vec![] };
        std::env::set_var(VENDOR_VAR, nv);
        set.push(VENDOR_VAR);
    } else {
        let Some(mesa) = mesa_vendor() else { return vec![] };
        std::env::set_var(VENDOR_VAR, mesa);
        set.push(VENDOR_VAR);
        // (two Mesa GPUs, Intel and AMD: Mesa's own pick, by PCI address,
        // when it isn't the compositor's)
        if want != "auto" {
            if let Some((_, pci)) = all.iter().find(|(f, _)| *f == target) {
                std::env::set_var("DRI_PRIME", format!("pci-{}", pci.replace([':', '.'], "_")));
                set.push("DRI_PRIME");
            }
        }
    }
    eprintln!("tessera-ui: drawing on the {} GPU ({})", family_label(target), if want == "auto" { "the desktop's" } else { "chosen in Settings" });
    set
}

/// After EGL started: the shell's children choose their own GPU again
pub fn restore(set: Vec<&'static str>) {
    for v in set {
        std::env::remove_var(v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn families_by_driver() {
        assert_eq!(family("i915"), "intel");
        assert_eq!(family("xe"), "intel");
        assert_eq!(family("amdgpu"), "amd");
        assert_eq!(family("nvidia"), "nvidia");
        assert_eq!(family("nouveau"), "nouveau");
    }

    #[test]
    fn device_numbers_split_like_glibc() {
        // /dev/dri/card2 is 226:2, renderD128 226:128
        assert_eq!(major_minor((226 << 8) | 2), (226, 2));
        assert_eq!(major_minor((226 << 8) | 128), (226, 128));
        // a large minor uses the high bits
        let dev = (226u64 << 8) | (0x123 << 12 & !0xff) | 0x45;
        assert_eq!(major_minor(dev).0, 226);
    }
}
