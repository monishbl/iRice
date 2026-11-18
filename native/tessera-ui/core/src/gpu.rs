//! Which GPU draws the shell. By default the low-power one: on a laptop with
//! two (an Intel or AMD one driving the screen, NVIDIA beside it) the
//! integrated GPU, which is normally also the compositor's, so frames go to
//! the screen without a copy between GPUs and the big GPU can sleep. Should
//! the compositor itself draw on the other GPU, the shell draws there too
//! (a frame copied across every refresh is worse than either), and says so.
//!
//! Why it has to be chosen at all: glvnd tries NVIDIA's EGL first, and it
//! accepts any Wayland display, so the shell drew every frame on the NVIDIA
//! card and the compositor copied it to the Intel one, at 4K and 120 Hz
//! (server1, 16 Nov: stutter, and the NVIDIA card kept awake). The
//! compositor names its GPU in linux-dmabuf's default feedback (main_device);
//! libEGL is told which vendor's EGL to load (and Mesa which device), for
//! this process's EGL start only: the setting is taken away again before
//! anything is started from here (apps keep their own choice).
//!
//! Settings → Battery → Cool-down and performance → Graphics processor
//! (performance.gpu): "low" (the default), "auto" (the compositor's), or a
//! family ("intel", "amd", "nvidia"). Drawing on the low-power GPU, frames that
//! keep missing the refresh (pacing.rs's slow watch) ask once whether to
//! switch to the other one (`slow`); "Not now" waits a week.

use smithay_client_toolkit::reexports::client::{
    globals::{registry_queue_init, GlobalListContents},
    protocol::wl_registry,
    Connection, Dispatch, QueueHandle,
};
use smithay_client_toolkit::reexports::protocols::wp::linux_dmabuf::zv1::client::{
    zwp_linux_dmabuf_feedback_v1::{self, ZwpLinuxDmabufFeedbackV1},
    zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1,
};
use std::ffi::OsString;
use std::sync::{Mutex, OnceLock};

const VENDOR_VAR: &str = "__EGL_VENDOR_LIBRARY_FILENAMES";
/// Set only on the one restart after a GPU didn't start ("nvidia>intel":
/// the one that failed, the one it's back on): that start never falls back
/// again, and says so in a notification. Taken out of the environment at once.
const FELL_BACK_VAR: &str = "TESSERA_GPU_FELL_BACK";
/// state.json's key for the last check of a GPU other than the desktop's
/// (`probe`): {"to", "desktop", "ok", "t"}
const PROBE_KEY: &str = "gpuProbe";
/// state.json's key for the fall back's notice not yet shown ("nvidia>intel")
const NOTICE_KEY: &str = "gpuFellBack";
/// A check is trusted this long (a driver update may change it)
const PROBE_TTL_MS: i64 = 30 * 24 * 3600 * 1000;

/// The environment as this process got it (remember_env), and the restart
/// marker it carried
static ORIGINAL_ENV: OnceLock<Vec<(OsString, OsString)>> = OnceLock::new();
static FELL_BACK: OnceLock<Option<String>> = OnceLock::new();
/// The faster GPU's check this run: Some(false) once it failed
static PROBED: Mutex<Option<bool>> = Mutex::new(None);

/// First thing in main, before anything changes the environment: kept so a
/// restart (fall_back) and the GPU check (probe) run with it exactly. The
/// restart's marker is read and taken out, so nothing started from here sees it.
pub fn remember_env() {
    let marker = std::env::var(FELL_BACK_VAR).ok();
    if marker.is_some() {
        std::env::remove_var(FELL_BACK_VAR);
    }
    let _ = FELL_BACK.set(marker);
    let _ = ORIGINAL_ENV.set(std::env::vars_os().collect());
}

fn original_env() -> Vec<(OsString, OsString)> {
    ORIGINAL_ENV.get().cloned().unwrap_or_else(|| std::env::vars_os().filter(|(k, _)| k != FELL_BACK_VAR).collect())
}

/// This start is the restart after a GPU didn't start
fn fell_back_already() -> bool {
    FELL_BACK.get().is_some_and(|m| m.is_some())
}

/// The restart's environment: the original one exactly, plus the marker
fn with_marker(original: &[(OsString, OsString)], marker: &str) -> Vec<(OsString, OsString)> {
    let mut env: Vec<(OsString, OsString)> = original.iter().filter(|(k, _)| k != FELL_BACK_VAR).cloned().collect();
    env.push((FELL_BACK_VAR.into(), marker.into()));
    env
}

/// "nvidia>intel" → ("nvidia", "intel")
fn parse_marker(m: &str) -> Option<(&str, &str)> {
    m.split_once('>').filter(|(a, b)| !a.is_empty() && !b.is_empty())
}

/// "Not now": asked again after this long
const SNOOZE_MS: i64 = 7 * 24 * 3600 * 1000;
/// state.json's key for it (ms since the epoch, until when)
const SNOOZE_KEY: &str = "gpuSlowAskAfter";
/// state.json's key for a GPU whose EGL didn't start here (fall_back)
const CANT_KEY: &str = "gpuCantDraw";

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

/// The compositor's GPU, by PCI address, None when it doesn't say (no
/// linux-dmabuf v4)
fn compositor_pci(conn: &Connection) -> Option<String> {
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
    let (maj, min) = major_minor(st.main?);
    pci_of(&format!("/sys/dev/char/{maj}:{min}/device"))
}

/// dev_t's major and minor (glibc's gnu_dev_major/minor)
fn major_minor(dev: u64) -> (u64, u64) {
    (((dev >> 8) & 0xfff) | ((dev >> 32) & !0xfff), (dev & 0xff) | ((dev >> 12) & !0xff))
}

fn pci_of(device: &str) -> Option<String> {
    std::fs::canonicalize(device).ok().and_then(|p| p.file_name().map(|f| f.to_string_lossy().into_owned()))
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

/// One GPU, as /sys/class/drm describes it
#[derive(Clone, Debug, PartialEq)]
pub struct Gpu {
    pub family: &'static str,
    pub pci: String,
    /// built into the processor (an Intel one on the root bus, an AMD APU):
    /// the low-power one
    pub integrated: bool,
    /// its own memory, bytes (amdgpu says; 0 unknown)
    pub vram: u64,
}

/// This computer's GPUs, one each, in /dev/dri order
pub fn list() -> Vec<Gpu> {
    let mut out: Vec<Gpu> = Vec::new();
    let Ok(rd) = std::fs::read_dir("/sys/class/drm") else { return out };
    let mut cards: Vec<String> = rd.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.starts_with("card") && !n.contains('-')).collect();
    cards.sort_by_key(|c| c[4..].parse::<u32>().unwrap_or(u32::MAX));
    for c in cards {
        let dev = format!("/sys/class/drm/{c}/device");
        let Some(driver) = std::fs::read_link(format!("{dev}/driver")).ok().and_then(|l| l.file_name().map(|f| f.to_string_lossy().into_owned())) else { continue };
        let pci = pci_of(&dev).unwrap_or_default();
        if out.iter().any(|g| g.pci == pci) {
            continue;
        }
        let read = |f: &str| std::fs::read_to_string(format!("{dev}/{f}")).map(|s| s.trim().to_string()).unwrap_or_default();
        let vram = read("mem_info_vram_total").parse().unwrap_or(0);
        let fam = family(&driver);
        out.push(Gpu { family: fam, integrated: integrated(fam, &pci, vram), pci, vram });
    }
    out
}

/// An integrated GPU: Intel's on the processor's own PCI bus (00:02.0; an
/// Arc card sits behind a bridge, on a bus of its own), an AMD one with
/// little memory of its own (an APU's carve-out: 512 MB to 4 GB)
fn integrated(family: &str, pci: &str, vram: u64) -> bool {
    let bus = pci.split(':').nth(1).unwrap_or("");
    match family {
        "intel" => bus == "00",
        "amd" => vram > 0 && vram <= 4 << 30,
        _ => false,
    }
}

/// This computer's GPUs: (family, PCI address), one each (Settings' list)
pub fn gpus() -> Vec<(&'static str, String)> {
    list().into_iter().map(|g| (g.family, g.pci)).collect()
}

/// The low-power GPU of several: an integrated one, else the one with the
/// least memory of its own, never NVIDIA's while there's another
pub fn low_power(all: &[Gpu]) -> Option<&Gpu> {
    all.iter().min_by_key(|g| {
        let nv = matches!(g.family, "nvidia" | "nouveau");
        (!g.integrated, nv, if g.vram == 0 { u64::MAX } else { g.vram })
    })
}

/// A GPU that draws faster than `low`: the other one (NVIDIA's first)
pub fn performance<'a>(all: &'a [Gpu], low: &Gpu) -> Option<&'a Gpu> {
    all.iter().filter(|g| g.pci != low.pci && g.family != "other").max_by_key(|g| (matches!(g.family, "nvidia"), !g.integrated, g.vram))
}

/// Settings' choice (performance.gpu): "low" (the default), "auto" (the
/// compositor's), or a family
fn chosen() -> String {
    let path = crate::util::xdg("XDG_CONFIG_HOME", ".config").join("tessera/config.json");
    std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v["performance"]["gpu"].as_str().map(String::from))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "low".into())
}

/// What the shell draws on, once decided (prefer_gpu)
struct Drawing {
    on: Gpu,
    /// the low-power policy chose it, and it's the low-power one
    low_power: bool,
    /// the faster one there is to offer
    faster: Option<Gpu>,
    /// "Not now" was said less than a week ago
    snoozed: bool,
    /// the compositor's GPU's family, when it said
    desktop: Option<&'static str>,
    /// the compositor's GPU
    theirs: Option<Gpu>,
    /// the low-power GPU
    low: Option<Gpu>,
}

static DRAWING: OnceLock<Drawing> = OnceLock::new();

/// The GPU the policy settles on, and why (for the log): `want` is Settings'
/// choice, `comp` the compositor's GPU (PCI address) if it said
fn decide<'a>(all: &'a [Gpu], want: &str, comp: Option<&str>) -> Option<(&'a Gpu, &'static str)> {
    let theirs = comp.and_then(|c| all.iter().find(|g| g.pci == c));
    match want {
        "auto" => theirs.map(|g| (g, "the desktop's")),
        "low" | "" => {
            let low = low_power(all)?;
            match theirs {
                // (the compositor draws on the other one: a copy of every frame
                // between GPUs would cost more than the low-power one saves)
                Some(t) if t.pci != low.pci => Some((t, "the desktop's: the desktop doesn't draw on the low-power GPU")),
                _ => Some((low, "low power")),
            }
        }
        f => match (all.iter().find(|g| g.family == f), theirs) {
            // (frames Mesa draws on another GPU never show on NVIDIA's
            // driver: the sandbox's desktop on NVIDIA, Intel chosen, stayed
            // black; drawn on the desktop's instead)
            (Some(g), Some(t)) if g.pci != t.pci && t.family == "nvidia" && g.family != "nvidia" => Some((t, "the desktop's: frames drawn on another GPU don't show on NVIDIA's")),
            (Some(g), _) => Some((g, "chosen in Settings")),
            (None, t) => t.map(|g| (g, "the desktop's")),
        },
    }
}

/// libEGL told to load `target`'s vendor (and Mesa which device, when it
/// isn't the compositor's): the variables set
fn select(target: &Gpu, comp: Option<&str>) -> Vec<&'static str> {
    let mut set = vec![];
    if target.family == "nvidia" {
        let Some(nv) = nvidia_vendor() else { return set };
        std::env::set_var(VENDOR_VAR, nv);
        set.push(VENDOR_VAR);
    } else {
        let Some(mesa) = mesa_vendor() else { return set };
        std::env::set_var(VENDOR_VAR, mesa);
        set.push(VENDOR_VAR);
        // (two Mesa GPUs, Intel and AMD: Mesa's own pick, by PCI address,
        // when it isn't the compositor's)
        if comp != Some(target.pci.as_str()) {
            std::env::set_var("DRI_PRIME", format!("pci-{}", target.pci.replace([':', '.'], "_")));
            set.push("DRI_PRIME");
        }
    }
    set
}

/// Before EGL starts: which GPU's EGL (the policy above). Nothing is changed
/// when the environment already says (glvnd's variables, TESSERA_GPU=any) or
/// there's only one GPU. Returns the variables it set (`restore` takes them
/// away once EGL is up, so what the shell starts chooses for itself).
pub fn prefer_gpu(conn: &Connection) -> Vec<&'static str> {
    if std::env::var_os(VENDOR_VAR).is_some() || std::env::var_os("__EGL_VENDOR_LIBRARY_DIRS").is_some() {
        return vec![];
    }
    if crate::env::var("GPU").as_deref() == Some("any") {
        return vec![];
    }
    let all = list();
    if all.len() < 2 {
        return vec![];
    }
    let mut want = chosen();
    let comp = compositor_pci(conn);
    // (the notice of a fall back, kept until a start stays up long enough to send it)
    if let Some((from, to)) = crate::cfgfile::state_get(NOTICE_KEY).as_str().and_then(|m| parse_marker(m).map(|(a, b)| (family_named(a), family_named(b)))) {
        tell_fell_back(from, to);
    }
    // (a GPU whose EGL didn't start for this desktop: the desktop's instead)
    let cant = crate::cfgfile::state_get(CANT_KEY);
    if let Some(m) = FELL_BACK.get().and_then(|m| m.clone()) {
        // (the restart after one didn't start: the desktop's, whatever the
        // settings say, should they not have been saved; the low-power one
        // when the desktop doesn't say which is its)
        want = if comp.is_some() { "auto".into() } else { "low".into() };
        if let Some((from, _)) = parse_marker(&m) {
            eprintln!("tessera-ui: started again after the {} GPU couldn't draw here", family_label(family_named(from)));
        }
    } else if cant.as_str() == Some(want.as_str()) {
        eprintln!("tessera-ui: the {} GPU couldn't draw here before: drawing on the desktop's", family_label(&want));
        want = "auto".into();
    }
    let Some((target, why)) = decide(&all, &want, comp.as_deref()) else { return vec![] };
    let target = target.clone();
    let set = select(&target, comp.as_deref());
    if set.is_empty() {
        return set;
    }
    let low = low_power(&all).cloned();
    let low_power = matches!(want.as_str(), "low" | "") && low.as_ref().is_some_and(|l| l.pci == target.pci);
    // (one that couldn't start here before, or whose check failed, isn't offered again)
    let probe = crate::cfgfile::state_get(PROBE_KEY);
    let faster = low
        .as_ref()
        .and_then(|l| performance(&all, l))
        .filter(|f| cant.as_str() != Some(f.family))
        .filter(|f| cached_probe(&probe, &f.pci, comp.as_deref(), crate::util::now_ms()) != Some(false))
        .cloned();
    let snoozed = crate::cfgfile::state_get(SNOOZE_KEY).as_i64().is_some_and(|t| crate::util::now_ms() < t);
    eprintln!(
        "tessera-ui: drawing on the {} GPU ({why}){}",
        family_label(target.family),
        if comp.is_some() && comp.as_deref() != Some(target.pci.as_str()) { "; the desktop draws on another: frames are copied across" } else { "" }
    );
    let theirs = comp.as_deref().and_then(|c| all.iter().find(|g| g.pci == c)).cloned();
    let desktop = theirs.as_ref().map(|g| g.family);
    let _ = DRAWING.set(Drawing { on: target, low_power, faster, snoozed, desktop, theirs, low });
    set
}

/// The GPU to start again on when `on` didn't start: the desktop's, else the
/// low-power one; none when that's the one that failed, or this start is
/// already the restart (once per start, never a loop)
fn fall_back_to<'a>(on: &Gpu, theirs: Option<&'a Gpu>, low: Option<&'a Gpu>, already: bool) -> Option<&'a Gpu> {
    if already {
        return None;
    }
    theirs.or(low).filter(|g| g.pci != on.pci)
}

/// Settings' choice back to "Same as the desktop" (a GPU that failed)
fn reset_choice(m: &mut serde_json::Map<String, serde_json::Value>) -> bool {
    crate::cfgfile::put_path(m, "performance.gpu", serde_json::json!("auto"))
}

/// EGL didn't start on the GPU chosen (NVIDIA's EGL refuses a desktop on
/// Intel: "no EGL display for Wayland"; libEGL keeps the vendor it loaded
/// first, so only a new process can load another). Some(environment): start
/// again with it, the original environment exactly plus the once-only
/// marker, after the choice was set back to the desktop's and the GPU noted
/// as one that can't draw here, both written to disk now.
///
/// (18 Nov, server1: these were queued for the files thread, which the exec
/// dropped, so every restart chose NVIDIA again: a loop at 96% CPU, black
/// screen, until the setting was changed by hand)
pub fn fall_back() -> Option<Vec<(OsString, OsString)>> {
    let d = DRAWING.get()?;
    let back = fall_back_to(&d.on, d.theirs.as_ref(), d.low.as_ref(), fell_back_already())?;
    eprintln!("tessera-ui: the {} GPU can't draw for this desktop: starting again on the {} one, and Graphics processor set to Same as the desktop", family_label(d.on.family), family_label(back.family));
    crate::cfgfile::flush();
    if let Err(e) = crate::cfgfile::edit_config(reset_choice) {
        eprintln!("tessera-ui: can't save the setting performance.gpu: {e}");
    }
    let on = d.on.family;
    let marker = format!("{}>{}", d.on.family, back.family);
    let notice = marker.clone();
    if let Err(e) = crate::cfgfile::edit_state(move |m| {
        m.insert(CANT_KEY.into(), serde_json::json!(on));
        m.insert(NOTICE_KEY.into(), serde_json::json!(notice));
        true
    }) {
        eprintln!("tessera-ui: can't save {CANT_KEY} in state.json: {e}");
    }
    crate::health::event_now("gpu-fallback", serde_json::json!({"from": family_label(d.on.family), "to": family_label(back.family)}));
    Some(with_marker(&original_env(), &marker))
}

/// A family's &'static name (from the restart's marker)
fn family_named(f: &str) -> &'static str {
    ["intel", "amd", "nvidia", "nouveau"].into_iter().find(|x| *x == f).unwrap_or("other")
}

/// After the restart: "Couldn't draw on NVIDIA: back on Intel", once the
/// shell (and its notification server, standalone) is up
fn tell_fell_back(from: &'static str, to: &'static str) {
    let title = format!("Couldn't draw on {}: back on {}", family_label(from), family_label(to));
    let body = format!(
        "Tessera couldn't start on the {} GPU with this desktop, so it's drawing on the {} GPU, and Graphics processor is set to Same as the desktop. You can change it in Settings → Battery & Power → Cool-down and performance.",
        family_label(from),
        family_label(to)
    );
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(8));
        notify(&title, &body);
        if let Err(e) = crate::cfgfile::edit_state(|m| m.remove(NOTICE_KEY).is_some()) {
            eprintln!("tessera-ui: can't save {NOTICE_KEY} in state.json: {e}");
        }
    });
}

/// After EGL started: the shell's children choose their own GPU again (each
/// variable back as this process got it: removed, or its old value)
pub fn restore(set: Vec<&'static str>) {
    let original = original_env();
    for v in set {
        match original.iter().find(|(k, _)| k == v) {
            Some((_, val)) => std::env::set_var(v, val),
            None => std::env::remove_var(v),
        }
    }
}

// ── Whether a GPU other than the desktop's can draw for it ────────────────

/// A check's answer from state.json, when it's about this GPU and this
/// desktop's, and recent
fn cached_probe(v: &serde_json::Value, to: &str, desktop: Option<&str>, now: i64) -> Option<bool> {
    (v["to"].as_str() == Some(to) && v["desktop"].as_str() == desktop && v["t"].as_i64().is_some_and(|t| now - t < PROBE_TTL_MS)).then(|| v["ok"].as_bool()).flatten()
}

/// The rule for offering `to`: the desktop's own GPU always; another only
/// once a check drew a frame on it for this desktop
fn offer_ok(to: &Gpu, desktop: Option<&Gpu>, checked: Option<bool>) -> bool {
    desktop.is_some_and(|d| d.pci == to.pci) || checked == Some(true)
}

/// Whether `to` can draw for this desktop: its own GPU, else a check in a
/// process of its own (`tessera-ui --gpu-probe PCI`, with the original
/// environment, 15 s at most), remembered for a month. Blocks: not on the UI thread.
fn verified(to: &Gpu) -> bool {
    let Some(d) = DRAWING.get() else { return false };
    let desktop = d.theirs.as_ref().map(|t| t.pci.clone());
    let mut checked = cached_probe(&crate::cfgfile::state_get(PROBE_KEY), &to.pci, desktop.as_deref(), crate::util::now_ms());
    if !offer_ok(to, d.theirs.as_ref(), checked) && checked.is_none() {
        let ok = run_probe(&to.pci);
        eprintln!("tessera-ui: checked the {} GPU for this desktop: {}", family_label(to.family), if ok { "it draws" } else { "it can't draw here" });
        crate::cfgfile::set_state(PROBE_KEY, serde_json::json!({"to": to.pci, "desktop": desktop, "ok": ok, "t": crate::util::now_ms()}));
        if !ok {
            crate::cfgfile::set_state(CANT_KEY, serde_json::json!(to.family));
        }
        checked = Some(ok);
    }
    if let Ok(mut p) = PROBED.lock() {
        *p = checked;
    }
    offer_ok(to, d.theirs.as_ref(), checked)
}

fn run_probe(pci: &str) -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    std::process::Command::new("timeout")
        .args(["-k", "2", "15"])
        .arg(exe)
        .args(["--gpu-probe", pci])
        .env_clear()
        .envs(original_env())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// `tessera-ui --gpu-probe PCI`: 0 when that GPU's EGL starts for this
/// desktop and a frame drawn on it is taken by the compositor (on a surface
/// with no role: never shown)
pub fn probe_main(pci: Option<&String>) -> i32 {
    match pci.ok_or_else(|| "no GPU named".to_string()).and_then(|p| probe(p)) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("tessera-ui: gpu probe: {e}");
            1
        }
    }
}

fn probe(pci: &str) -> Result<(), String> {
    use slint::platform::femtovg_renderer::OpenGLInterface;
    use smithay_client_toolkit::reexports::client::Proxy;
    let conn = Connection::connect_to_env().map_err(|e| format!("no Wayland display: {e}"))?;
    let all = list();
    let target = all.iter().find(|g| g.pci == pci).ok_or("no such GPU")?;
    let comp = compositor_pci(&conn);
    std::env::remove_var(VENDOR_VAR);
    std::env::remove_var("__EGL_VENDOR_LIBRARY_DIRS");
    std::env::remove_var("DRI_PRIME");
    if select(target, comp.as_deref()).is_empty() {
        return Err("no EGL vendor file for it".into());
    }
    let egl = crate::egl::Egl::new(conn.backend().display_ptr() as *mut _).map_err(|e| e.to_string())?;
    let (globals, mut queue) = registry_queue_init::<Feedback>(&conn).map_err(|e| e.to_string())?;
    let qh = queue.handle();
    let compositor: WlCompositor = globals.bind(&qh, 1..=4, ()).map_err(|e| e.to_string())?;
    let surface = compositor.create_surface(&qh, ());
    let gl = crate::egl::GlSurface::new(egl, surface.id()).map_err(|e| e.to_string())?;
    gl.ensure_current().map_err(|e| e.to_string())?;
    gl.swap_buffers().map_err(|e| e.to_string())?;
    let mut st = Feedback::default();
    queue.roundtrip(&mut st).map_err(|e| format!("the desktop refused the frame: {e}"))?;
    drop(gl);
    surface.destroy();
    let _ = queue.roundtrip(&mut st);
    Ok(())
}

impl Dispatch<WlCompositor, ()> for Feedback {
    fn event(_: &mut Self, _: &WlCompositor, _: <WlCompositor as smithay_client_toolkit::reexports::client::Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlSurface, ()> for Feedback {
    fn event(_: &mut Self, _: &WlSurface, _: <WlSurface as smithay_client_toolkit::reexports::client::Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
use smithay_client_toolkit::reexports::client::protocol::{wl_compositor::WlCompositor, wl_surface::WlSurface};

/// The family of the GPU the desktop (the compositor) draws on, when known
pub fn desktop_family() -> Option<&'static str> {
    DRAWING.get().and_then(|d| d.desktop)
}

/// The family of the GPU the shell draws on, when it chose one (two GPUs)
pub fn drawing_family() -> Option<&'static str> {
    DRAWING.get().map(|d| d.on.family)
}

/// The faster GPU there is to switch to (Settings → Health's "Use the
/// other GPU"), when the shell draws on the low-power one
pub fn faster_family() -> Option<&'static str> {
    if PROBED.lock().is_ok_and(|p| *p == Some(false)) {
        return None;
    }
    DRAWING.get().filter(|d| d.low_power).and_then(|d| d.faster.as_ref()).map(|g| g.family)
}

/// Draw on the faster GPU from now (the setting, and the shell starts
/// again), once it's checked that it can draw for this desktop (in the
/// background; a notification when it can't)
pub fn use_faster() -> bool {
    let Some(faster) = DRAWING.get().filter(|d| d.low_power).and_then(|d| d.faster.clone()) else { return false };
    std::thread::spawn(move || {
        if verified(&faster) {
            answered("switch", faster.family);
        } else {
            let on = DRAWING.get().map_or("other", |d| d.on.family);
            notify(
                &format!("Couldn't switch to the {} GPU", family_label(faster.family)),
                &format!("It can't draw for this desktop, which runs on the {} GPU, so Tessera stays there.", family_label(on)),
            );
        }
    });
    true
}

fn notify(title: &str, body: &str) {
    let _ = std::process::Command::new("timeout")
        .args(["30", "notify-send", "-a", "Tessera", "-i", "video-display", "--", title, body])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

/// The slow-GPU watch runs: drawing on the low-power GPU by its policy, a
/// faster one exists, and "Not now" wasn't said this week
pub fn watching() -> bool {
    DRAWING.get().is_some_and(|d| d.low_power && d.faster.is_some() && !d.snoozed)
}

/// Frames were too slow on the low-power GPU (pacing.rs, `share` of them
/// missed): ask once whether to draw on the faster one. Switch sets
/// performance.gpu and starts the shell again; Not now waits a week.
pub fn slow(share: f64) {
    let Some(d) = DRAWING.get() else { return };
    let Some(faster) = d.faster.clone() else { return };
    let (on, to) = (family_label(d.on.family), family_label(faster.family));
    eprintln!("tessera-ui: {:.0}% of animation frames missed on the {on} GPU: asking about the {to} GPU", share * 100.0);
    crate::health::event("gpu-slow", serde_json::json!({"share": (share * 1000.0).round() / 1000.0, "on": on, "to": to}));
    let title = format!("Tessera is running slowly on the {on} graphics — use the {to} GPU?");
    let body = format!("Animations missed {:.0}% of their frames. The {to} GPU is faster but uses more power. You can change this in Settings → Battery & Power → Cool-down and performance.", share * 100.0);
    std::thread::spawn(move || {
        // (offered only when it can draw for this desktop: 18 Nov, server1's
        // NVIDIA beside a desktop on Intel was offered, and couldn't start)
        if !verified(&faster) {
            eprintln!("tessera-ui: the {to} GPU isn't offered: it can't draw for this desktop");
            crate::cfgfile::set_state(SNOOZE_KEY, serde_json::json!(crate::util::now_ms() + SNOOZE_MS));
            return;
        }
        let out = std::process::Command::new("timeout")
            .args(["86400", "notify-send", "-a", "Tessera", "-i", "video-display", "-A", "switch=Switch", "-A", "later=Not now", "--", &title, &body])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output();
        let answer = out.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
        answered(&answer, faster.family);
    });
}

/// The question's answer: "switch", "later", or nothing (closed: as Not now)
fn answered(answer: &str, family: &str) {
    if answer == "switch" {
        eprintln!("tessera-ui: switching to the {} GPU: the shell starts again", family_label(family));
        if crate::cfgfile::set_config("performance.gpu", serde_json::json!(family)) {
            restart_shell();
        }
    } else {
        crate::cfgfile::set_state(SNOOZE_KEY, serde_json::json!(crate::util::now_ms() + SNOOZE_MS));
    }
}

/// The shell starts again (repair-shell.sh: it outlives this process,
/// replaces it, and apps keep running)
fn restart_shell() {
    let script = crate::env::root().join("shell/assets/scripts/repair-shell.sh");
    crate::services::bridge::run(&["setsid", "-f", &script.to_string_lossy()]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(family: &'static str, pci: &str, vram: u64) -> Gpu {
        Gpu { family, pci: pci.into(), integrated: integrated(family, pci, vram), vram }
    }

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
        // a minor over 255 continues above the major's 12 bits
        assert_eq!(major_minor((226 << 8) | 0x45 | (0x123 << 20)), (226, 0x12345));
    }

    #[test]
    fn the_low_power_gpu_is_the_integrated_one() {
        // server1: NVIDIA first in /dev/dri, Intel driving the screen
        let all = [g("nvidia", "0000:01:00.0", 0), g("intel", "0000:00:02.0", 0)];
        assert_eq!(low_power(&all).unwrap().family, "intel");
        assert_eq!(performance(&all, low_power(&all).unwrap()).unwrap().family, "nvidia");
        // an AMD APU beside an AMD card: the one with little memory of its own
        let all = [g("amd", "0000:03:00.0", 16 << 30), g("amd", "0000:c4:00.0", 512 << 20)];
        assert_eq!(low_power(&all).unwrap().pci, "0000:c4:00.0");
        // Intel Arc (its own bus) beside the processor's Intel
        let all = [g("intel", "0000:03:00.0", 0), g("intel", "0000:00:02.0", 0)];
        assert_eq!(low_power(&all).unwrap().pci, "0000:00:02.0");
        // nouveau is never the low-power one beside another
        let all = [g("nouveau", "0000:01:00.0", 0), g("amd", "0000:05:00.0", 0)];
        assert_eq!(low_power(&all).unwrap().family, "amd");
    }

    #[test]
    fn the_policy_follows_the_desktop_when_it_must() {
        let all = [g("nvidia", "0000:01:00.0", 0), g("intel", "0000:00:02.0", 0)];
        // the default: the low-power GPU, which the desktop draws on too
        assert_eq!(decide(&all, "low", Some("0000:00:02.0")).unwrap().0.family, "intel");
        // the desktop doesn't say: still the low-power GPU (glvnd would pick NVIDIA)
        assert_eq!(decide(&all, "low", None).unwrap().0.family, "intel");
        // the desktop draws on NVIDIA: so does the shell (no copy of every frame)
        let (gpu, why) = decide(&all, "low", Some("0000:01:00.0")).unwrap();
        assert_eq!(gpu.family, "nvidia");
        assert!(why.contains("desktop"));
        // Same as the desktop
        assert_eq!(decide(&all, "auto", Some("0000:01:00.0")).unwrap().0.family, "nvidia");
        assert!(decide(&all, "auto", None).is_none());
        // a family picked in Settings; one not there falls back to the desktop's
        assert_eq!(decide(&all, "nvidia", Some("0000:00:02.0")).unwrap().0.family, "nvidia");
        assert_eq!(decide(&all, "amd", Some("0000:00:02.0")).unwrap().0.family, "intel");
        // Intel picked while the desktop draws on NVIDIA: its frames would never show
        let (gpu, why) = decide(&all, "intel", Some("0000:01:00.0")).unwrap();
        assert_eq!(gpu.family, "nvidia");
        assert!(why.contains("don't show"));
    }

    #[test]
    fn only_a_gpu_that_can_draw_for_the_desktop_is_offered() {
        // server1, 18 Nov: the desktop on Intel, NVIDIA beside it
        let (nv, intel) = (g("nvidia", "0000:01:00.0", 0), g("intel", "0000:00:02.0", 0));
        // not checked, or the check failed: not offered
        assert!(!offer_ok(&nv, Some(&intel), None));
        assert!(!offer_ok(&nv, Some(&intel), Some(false)));
        // a check that drew a frame for this desktop: offered
        assert!(offer_ok(&nv, Some(&intel), Some(true)));
        // the desktop's own GPU needs no check
        assert!(offer_ok(&nv, Some(&nv), None));
        // the desktop doesn't say: only a passed check counts
        assert!(!offer_ok(&nv, None, None));
        assert!(offer_ok(&nv, None, Some(true)));
    }

    #[test]
    fn a_check_counts_for_its_gpu_and_desktop_for_a_month() {
        let now = 100 * PROBE_TTL_MS;
        let v = serde_json::json!({"to": "0000:01:00.0", "desktop": "0000:00:02.0", "ok": false, "t": now - 1000});
        assert_eq!(cached_probe(&v, "0000:01:00.0", Some("0000:00:02.0"), now), Some(false));
        // another desktop's GPU, another GPU: not this check
        assert_eq!(cached_probe(&v, "0000:01:00.0", Some("0000:01:00.0"), now), None);
        assert_eq!(cached_probe(&v, "0000:03:00.0", Some("0000:00:02.0"), now), None);
        // older than a month: checked again
        assert_eq!(cached_probe(&v, "0000:01:00.0", Some("0000:00:02.0"), now + PROBE_TTL_MS), None);
        assert_eq!(cached_probe(&serde_json::Value::Null, "0000:01:00.0", None, now), None);
    }

    #[test]
    fn a_failed_gpu_starts_again_once_on_the_desktops() {
        let (nv, intel) = (g("nvidia", "0000:01:00.0", 0), g("intel", "0000:00:02.0", 0));
        // NVIDIA chosen, the desktop on Intel: again on Intel
        assert_eq!(fall_back_to(&nv, Some(&intel), Some(&intel), false).unwrap().family, "intel");
        // the desktop doesn't say: the low-power one
        assert_eq!(fall_back_to(&nv, None, Some(&intel), false).unwrap().family, "intel");
        // this start is already the restart: never again (no loop)
        assert!(fall_back_to(&nv, Some(&intel), Some(&intel), true).is_none());
        // the desktop's own GPU failed: nothing else to start on
        assert!(fall_back_to(&intel, Some(&intel), Some(&intel), false).is_none());
    }

    #[test]
    fn the_restart_gets_the_original_environment_and_the_marker() {
        let orig: Vec<(OsString, OsString)> = vec![("WAYLAND_DISPLAY".into(), "wayland-1".into()), ("XDG_RUNTIME_DIR".into(), "/run/user/1000".into()), (FELL_BACK_VAR.into(), "stale".into())];
        let env = with_marker(&orig, "nvidia>intel");
        assert_eq!(env[0], ("WAYLAND_DISPLAY".into(), "wayland-1".into()));
        assert_eq!(env[1], ("XDG_RUNTIME_DIR".into(), "/run/user/1000".into()));
        // one marker, the new one; no EGL vendor variable from the failed try
        assert_eq!(env.iter().filter(|(k, _)| k == FELL_BACK_VAR).count(), 1);
        assert_eq!(env.last().unwrap().1, OsString::from("nvidia>intel"));
        assert!(!env.iter().any(|(k, _)| k == VENDOR_VAR || k == "DRI_PRIME"));
        assert_eq!(parse_marker("nvidia>intel"), Some(("nvidia", "intel")));
        assert_eq!(parse_marker("garbage"), None);
        assert_eq!(family_named("nvidia"), "nvidia");
        assert_eq!(family_named("x"), "other");
    }

    #[test]
    fn restore_puts_back_what_the_process_got() {
        // (this test's own variables: DRI_PRIME was set before, the vendor one wasn't)
        remember_env();
        let had = std::env::var_os("DRI_PRIME");
        std::env::set_var("DRI_PRIME", "pci-0000_01_00_0");
        restore(vec!["DRI_PRIME"]);
        assert_eq!(std::env::var_os("DRI_PRIME"), had);
    }

    #[test]
    fn a_failed_choice_goes_back_to_the_desktops_on_disk() {
        let dir = std::env::temp_dir().join(format!("tessera-gpu-reset-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        std::fs::write(&path, r#"{"performance": {"gpu": "nvidia", "keep": true}, "dock": {"iconSize": 48}}"#).unwrap();
        assert!(crate::cfgfile::edit_config_at(&path, reset_choice).unwrap());
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["performance"]["gpu"], "auto");
        assert_eq!(v["performance"]["keep"], true);
        assert_eq!(v["dock"]["iconSize"], 48);
        // already so: nothing written
        assert!(!crate::cfgfile::edit_config_at(&path, reset_choice).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
