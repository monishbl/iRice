#!/usr/bin/env python3
"""Tessera's features: the one list the installer's Features step, the
one-time "Set up the rest of Tessera" offer after Software Update and the
root helper's `features` all go by. Each feature is what it needs
(repository and AUR packages), what root does for it (tessera-root.py
features: PAM lines, Howdy's camera, bluetoothd, the login screen…), what
is done as you (the desktop look, the battery guard…), and how to tell it's
set up. Hardware that isn't here hides its feature.

  features.py list [--json]       every feature this computer can have; text:
                                  one a line, tab-separated: id, set up 1/0,
                                  group, title, detail, packages, AUR packages
  features.py missing             JSON {"items": [...]}: the ones the offer
                                  lists (not set up, not declined, offered)
  features.py plan ID...          JSON {"ids", "install", "aur", "root",
                                  "user", "rootArgs"}: what setting these up
                                  takes (only the ones not set up yet)
  features.py apply-user ID...    the parts done as you; a JSON line each:
                                  {"id", "ok", "error", "fix"} (--text: lines
                                  for people)
  features.py login-look          the login screen's look from your settings
                                  (JSON, as tessera-root login-switch takes it)
  features.py record [--chosen a,b] [--declined c,d] [--offer pending|done]
                                  what was chosen, into config.json
                                  (features.chosen, .declined, .offer)
  --facts FILE                    (first) facts from a JSON file instead of
                                  this computer's (tests)

Only reads, except apply-user and record. TESSERA_SYSTEM_ROOT moves /sys,
/etc, /usr and pacman's database somewhere else (tests).
"""
import glob
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.realpath(__file__))
ROOT = os.environ.get("TESSERA_SYSTEM_ROOT", "/")
GROUPS = ["Hardware", "Sharing", "Look and login", "Extras"]
# Other power managers: power-profiles-daemon would fight them
POWER_MANAGERS = ("tlp", "auto-cpufreq", "tuned-ppd", "tuned")
BT_CONF = "/etc/bluetooth/main.conf"
BT_DROPIN = "/etc/systemd/system/bluetooth.service.d/tessera-no-rap.conf"
QS_TRUST_MARK = "/etc/tessera/quickshare-trusted"
HOWDY = ("howdy-next", "howdy", "howdy-git", "howdy-beta-git")
# What root does for a feature (tessera-root.py features knows each by name)
ROOT_PARTS = {"fingerprint", "face", "bluetooth", "power", "quickshare", "login", "bootsplash"}
# What's done as you (apply_user)
USER_PARTS = {"face", "battery", "look", "captions"}
# How to try again by hand, said with a failure
FIX = {
    "fingerprint": "Settings → Privacy & Security → Fingerprint sets it up.",
    "face": "Settings → Privacy & Security → Face sets it up.",
    "bluetooth": "Settings → System health fixes Bluetooth's pairing prompts.",
    "battery": "Settings → Battery & Power turns the battery guard on.",
    "power": "sudo systemctl enable --now power-profiles-daemon",
    "quickshare": "Settings → Privacy & Security → Fast transfers without asking.",
    "cast": "sudo pacman -S yt-dlp",
    "mirroring": "Settings → Displays installs it when you mirror.",
    "phone": "Settings → Phone installs what it needs.",
    "login": "Settings → Lock & Login Screen switches to it.",
    "bootsplash": "Settings → Lock & Login Screen → Boot animation.",
    "look": "Settings → Appearance → Tessera's desktop look.",
    "glass": "Settings → Appearance → Build glass.",
    "recorder": "sudo pacman -S gpu-screen-recorder wf-recorder",
    "ocr": "Settings → Keyboard → Text capture installs your language.",
    "clouddrives": "Settings → Cloud drives installs rclone.",
    "clipboard": "sudo pacman -S cliphist",
    "captions": "Settings → Accessibility → Live captions sets them up.",
    "emoji": "Settings → Appearance → Emoji installs them.",
    "aur": "Install yay: git clone https://aur.archlinux.org/yay-bin.git && cd yay-bin && makepkg -si",
}
# A language's tesseract data (ISO 639-1 → tesseract's code), and its name
OCR = {"af": "afr", "ar": "ara", "az": "aze", "be": "bel", "bg": "bul", "bn": "ben", "bs": "bos", "ca": "cat", "cs": "ces",
       "cy": "cym", "da": "dan", "de": "deu", "el": "ell", "eo": "epo", "es": "spa", "et": "est", "eu": "eus", "fa": "fas",
       "fi": "fin", "fr": "fra", "ga": "gle", "gl": "glg", "gu": "guj", "he": "heb", "hi": "hin", "hr": "hrv", "hu": "hun",
       "hy": "hye", "id": "ind", "is": "isl", "it": "ita", "ja": "jpn", "ka": "kat", "kk": "kaz", "km": "khm", "kn": "kan",
       "ko": "kor", "lt": "lit", "lv": "lav", "mk": "mkd", "ml": "mal", "mn": "mon", "mr": "mar", "ms": "msa", "mt": "mlt",
       "my": "mya", "nb": "nor", "ne": "nep", "nl": "nld", "nn": "nor", "no": "nor", "pa": "pan", "pl": "pol", "pt": "por",
       "ro": "ron", "ru": "rus", "si": "sin", "sk": "slk", "sl": "slv", "sq": "sqi", "sr": "srp", "sv": "swe", "sw": "swa",
       "ta": "tam", "te": "tel", "th": "tha", "tr": "tur", "uk": "ukr", "ur": "urd", "uz": "uzb", "vi": "vie"}
LANGUAGES = {"de": "German", "fr": "French", "es": "Spanish", "it": "Italian", "pt": "Portuguese", "nl": "Dutch",
             "pl": "Polish", "ru": "Russian", "uk": "Ukrainian", "tr": "Turkish", "sv": "Swedish", "da": "Danish",
             "fi": "Finnish", "nb": "Norwegian", "no": "Norwegian", "cs": "Czech", "el": "Greek", "hu": "Hungarian",
             "ro": "Romanian", "ja": "Japanese", "ko": "Korean", "zh": "Chinese", "ar": "Arabic", "he": "Hebrew",
             "hi": "Hindi", "ta": "Tamil", "te": "Telugu", "bn": "Bengali", "ml": "Malayalam", "kn": "Kannada",
             "mr": "Marathi", "gu": "Gujarati", "vi": "Vietnamese", "th": "Thai", "id": "Indonesian"}


def path(p):
    return os.path.join(ROOT, p.lstrip("/"))


def read(p):
    try:
        with open(p, errors="replace") as f:
            return f.read()
    except OSError:
        return ""


def load(name):
    """One of the shell's scripts next to this one (fingerprint.py, face.py)."""
    spec = importlib.util.spec_from_file_location("tessera_f_" + name.replace("-", "_"), os.path.join(HERE, name + ".py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def config_path():
    return os.path.join(os.environ.get("XDG_CONFIG_HOME") or os.path.expanduser("~/.config"), "tessera", "config.json")


def read_config():
    try:
        with open(config_path()) as f:
            c = json.load(f)
        return c if isinstance(c, dict) else {}
    except (OSError, ValueError):
        return {}


def installed_packages():
    return sorted({re.sub(r"-[^-]+-[^-]+$", "", os.path.basename(d)) for d in glob.glob(path("/var/lib/pacman/local/*-*-*"))})


# ── What this computer has ──────────────────────────────────────────────
def locale_language(env=None):
    env = os.environ if env is None else env
    for k in ("LC_ALL", "LC_MESSAGES", "LANG"):
        v = env.get(k, "")
        m = re.match(r"^([a-z]{2,3})(?:_([A-Za-z]{2}))?", v)
        if v and m and not v.startswith(("C.", "POSIX")) and v != "C":
            if m[1] == "zh":
                return "zh", ("chi_tra" if (m[2] or "").upper() in ("TW", "HK", "MO") else "chi_sim")
            return m[1], OCR.get(m[1], "")
    return "", ""


def systemctl_enabled(unit, run=subprocess.run):
    if ROOT != "/":
        return os.path.exists(path("/etc/systemd/system/multi-user.target.wants/" + unit)) or \
            os.path.exists(path("/etc/systemd/system/graphical.target.wants/" + unit))
    try:
        return run(["systemctl", "is-enabled", "--quiet", unit], timeout=5).returncode == 0
    except (OSError, subprocess.SubprocessError):
        return False


def bt_experimental(text):
    """bluetoothd's [General] Experimental = true"""
    section = ""
    for line in text.splitlines():
        s = line.strip()
        m = re.match(r"^\[(.+)\]$", s)
        if m:
            section = m[1].strip()
        elif section == "General" and re.match(r"^Experimental\s*=\s*true\s*$", s, re.I):
            return True
    return False


def mkinitcpio_hooks():
    hooks = []
    for p in [path("/etc/mkinitcpio.conf")] + sorted(glob.glob(path("/etc/mkinitcpio.conf.d/*.conf"))):
        for line in read(p).splitlines():
            m = re.match(r"^\s*HOOKS(\+?)=\((.*)\)", line)
            if m:
                hooks = (hooks if m[1] else []) + m[2].split()
    return hooks


def plymouth_theme():
    for p in ("/etc/plymouth/plymouthd.conf", "/usr/share/plymouth/plymouthd.defaults"):
        m = re.search(r"^\s*Theme\s*=\s*(\S+)", read(path(p)), re.M)
        if m:
            return m[1]
    return ""


def display_manager():
    try:
        return os.path.basename(os.readlink(path("/etc/systemd/system/display-manager.service"))).removesuffix(".service")
    except OSError:
        return ""


def glass_built(data=None):
    data = data or os.path.join(os.environ.get("XDG_DATA_HOME") or os.path.expanduser("~/.local/share"), "tessera")
    if not os.path.isfile(os.path.join(data, "tessera-glass.so")):
        return False
    built = read(os.path.join(data, "tessera-glass.built-for")).strip()
    m = re.search(r"/hyprland-([0-9.]+)-", " ".join(glob.glob(path("/var/lib/pacman/local/hyprland-[0-9]*"))))
    return not m or built == m[1].rstrip(".")


def collect():
    """This computer's facts (what plan() needs), as JSON can say them."""
    have = installed_packages()
    reader = None
    try:
        fpm = load("fingerprint")
        r = fpm.find_reader()
        if r and not r.get("unsupported"):
            repo, aur = fpm.needs(r)
            reader = dict(r, repo=repo, aur=aur, pam=fpm.pam_state(), driver=r["id"] in fpm.DRIVERS)
    except Exception:  # noqa: BLE001 (an older script: no reader shown, nothing breaks)
        reader = None
    cams = []
    try:
        cams = load("face").cameras()
    except Exception:  # noqa: BLE001
        cams = []
    cfg = read_config()
    supplies = glob.glob(path("/sys/class/power_supply/*/type"))
    battery = any(read(t).strip() == "Battery" and read(os.path.join(os.path.dirname(t), "scope")).strip() != "Device" for t in supplies)
    lang, ocr = locale_language()
    dm = display_manager()
    greetd = read(path("/etc/greetd/config.toml"))
    return {
        "installed": have,
        "fingerprint": reader,
        "cameras": cams,
        "howdy_models": bool(glob.glob(path("/usr/share/howdy/models/*.onnx"))),
        "bluetooth": bool(glob.glob(path("/sys/class/bluetooth/hci*"))),
        "bt_experimental": bt_experimental(read(path(BT_CONF))),
        "bt_pairing": os.path.exists(path(BT_DROPIN)),
        "battery": battery,
        "battery_guard": os.path.exists(os.path.expanduser("~/.config/systemd/user/tessera-battery-guard.service")),
        "power_manager": next((p for p in POWER_MANAGERS if p in have), ""),
        "ppd_enabled": systemctl_enabled("power-profiles-daemon.service"),
        "qs_trusted": os.path.exists(path(QS_TRUST_MARK)),
        "display_manager": "" if dm == "greetd" else dm,
        "tessera_login": dm == "greetd" and "/usr/share/tessera" in greetd,
        "plymouth_hook": "plymouth" in mkinitcpio_hooks() and bool(shutil.which("mkinitcpio") or os.path.exists(path("/usr/bin/mkinitcpio"))),
        "plymouth_theme": plymouth_theme(),
        "desktop_look_off": isinstance(cfg.get("appearance"), dict) and cfg["appearance"].get("desktopLook") is False,
        "glass_built": glass_built(),
        "language": lang, "ocr": ocr,
        "aur_helper": bool(shutil.which("yay") or shutil.which("paru")),
        "whisper_model": bool(glob.glob(os.path.join(os.environ.get("XDG_DATA_HOME") or os.path.expanduser("~/.local/share"),
                                                     "tessera", "whisper", "ggml-tiny*.bin"))),
        "features": cfg.get("features") if isinstance(cfg.get("features"), dict) else {},
    }


# ── The list ────────────────────────────────────────────────────────────
def item(id, group, title, detail, set_up, packages=(), aur=(), offer=True, **extra):
    i = {"id": id, "group": group, "title": title, "detail": detail, "setUp": bool(set_up), "packages": list(packages),
         "aur": list(aur), "root": id in ROOT_PARTS, "user": id in USER_PARTS, "offer": offer, "fix": FIX.get(id, "")}
    i.update(extra)
    return i


def plan(f):
    """Every feature for these facts (collect()'s), in the groups' order."""
    have = set(f.get("installed", ()))
    miss = lambda pkgs: [p for p in pkgs if p not in have]
    G = GROUPS
    out = []

    # Hardware
    fp = f.get("fingerprint")
    if fp:
        need_repo, need_aur = list(fp.get("repo", [])), list(fp.get("aur", []))
        pam = fp.get("pam") or {}
        out.append(item("fingerprint", G[0], "Fingerprint",
                        fp["name"] + ": unlock and approve password prompts with a finger"
                        + (". Its driver comes from the AUR" if need_aur else ""),
                        not need_repo and not need_aur and pam.get("polkit") and pam.get("sudo"), need_repo, need_aur,
                        driver=bool(fp.get("driver"))))
    cams = f.get("cameras") or []
    if cams:
        cam = cams[0]  # (infrared first)
        rgb = cam.get("kind") == "rgb"
        howdy = bool(have & set(HOWDY))
        out.append(item("face", G[0], "Face unlock",
                        cam["name"] + (": unlock the lock screen by looking at it and blinking" if rgb else ": unlock the lock screen by looking at it"),
                        howdy and f.get("howdy_models") and (not rgb or "python-opencv" in have),
                        miss(["python-opencv"]) if rgb else [], [] if howdy else ["howdy-next"],
                        camera=cam["path"], kind=cam.get("kind", "ir")))
    if f.get("bluetooth"):
        out.append(item("bluetooth", G[0], "Bluetooth extras",
                        "Battery levels of headphones and phones, and no pairing prompts when a phone shares",
                        f.get("bt_experimental") and f.get("bt_pairing")))
    if f.get("battery"):
        out.append(item("battery", G[0], "Battery guard", "Sleeps before the battery runs out, even when the desktop isn't running",
                        f.get("battery_guard"), offer=False))
    if not f.get("power_manager"):
        out.append(item("power", G[0], "Energy modes", "Balanced, Power Saver and Performance in the menu bar",
                        "power-profiles-daemon" in have and f.get("ppd_enabled"), miss(["power-profiles-daemon"])))

    # Sharing
    out.append(item("quickshare", G[1], "Quick Share",
                    "Send files to and from phones nearby. Fast transfers set up their own Wi-Fi link without asking",
                    f.get("qs_trusted"), miss(["avahi", "python-cryptography", "python-protobuf"])))
    out.append(item("cast", G[1], "Cast web videos", "Play YouTube and other web videos on a Chromecast or Google TV",
                    "yt-dlp" in have, miss(["yt-dlp"])))
    out.append(item("mirroring", G[1], "Screen mirroring", "Show your screen on a TV that takes Miracast",
                    "gnome-network-displays" in have, miss(["gnome-network-displays"])))
    out.append(item("phone", G[1], "Phone link", "Show and control an Android phone on this screen",
                    not miss(["android-tools", "scrcpy"]), miss(["android-tools", "scrcpy"])))

    # Look and login
    dm = f.get("display_manager", "")
    out.append(item("login", G[2], "Tessera's login screen",
                    "Sign in on Tessera's own screen" + (". {} is turned off, and comes back by itself if Tessera's doesn't start".format(dm) if dm else ""),
                    f.get("tessera_login"), [] if "greetd" in have else ["greetd"], replaces=dm))
    if f.get("plymouth_hook"):
        out.append(item("bootsplash", G[2], "Boot animation", "Tessera's animation while the computer starts",
                        str(f.get("plymouth_theme", "")).startswith("tessera-")))
    out.append(item("look", G[2], "Tessera's desktop look", "Its font and terminal prompt, Darkly for Qt apps and Breeze Plus icons",
                    f.get("desktop_look_off") or not miss(["darkly-bin", "breeze-plus"]), [], miss(["darkly-bin", "breeze-plus"])))
    out.append(item("glass", G[2], "Glass plugin", "Real glass behind windows and panels, built for this Hyprland",
                    f.get("glass_built"), offer=False))

    # Extras
    out.append(item("recorder", G[3], "Screen recorder", "Record the screen with the graphics card",
                    not miss(["gpu-screen-recorder", "wf-recorder"]), miss(["gpu-screen-recorder", "wf-recorder"])))
    if f.get("ocr") and f.get("ocr") != "eng":
        name = LANGUAGES.get(f.get("language", ""), "")
        out.append(item("ocr", G[3], "Text recognition in " + name if name else "Text recognition in your language",
                        "Copy text from screenshots and photos in your language as well as English",
                        "tesseract-data-" + f["ocr"] in have, miss(["tesseract-data-" + f["ocr"]])))
    out.append(item("clouddrives", G[3], "Cloud drives", "Google Drive and OneDrive as folders in Files",
                    "rclone" in have, miss(["rclone", "fuse3"])))
    out.append(item("clipboard", G[3], "Clipboard history", "What you copy, a shortcut away", "cliphist" in have, miss(["cliphist"])))
    out.append(item("captions", G[3], "Live captions and dictation",
                    "Captions for what plays and typing by voice, made on this computer (a 75 MB model)",
                    "whisper-cpp" in have and f.get("whisper_model"), miss(["whisper-cpp"])))
    out.append(item("emoji", G[3], "Apple and Fluent emoji", "Two more emoji styles to pick from in Settings",
                    not miss(["ttf-apple-emoji", "ttf-fluent-emoji"]), [], miss(["ttf-apple-emoji", "ttf-fluent-emoji"])))
    if not f.get("aur_helper"):
        out.append(item("aur", G[3], "AUR helper (yay)", "Installs and updates the packages above that come from the AUR",
                        False, offer=False))
    return out


def missing(items, f):
    """What the offer lists: offered features not set up, not declined."""
    declined = set((f.get("features") or {}).get("declined") or [])
    return [i for i in items if i["offer"] and not i["setUp"] and i["id"] not in declined]


def plan_for(items, ids):
    """What setting these up takes: packages, AUR packages, root and user
    parts, and the root helper's arguments (only the ones not set up)."""
    by = {i["id"]: i for i in items}
    chosen = [by[i] for i in ids if i in by and not by[i]["setUp"]]
    out = {"ids": [i["id"] for i in chosen], "install": [], "aur": [], "root": [], "user": [], "rootArgs": []}
    for i in chosen:
        for p in i["packages"]:
            if p not in out["install"]:
                out["install"].append(p)
        for p in i["aur"]:
            if p not in out["aur"]:
                out["aur"].append(p)
        if i["root"]:
            out["root"].append(i["id"])
        if i["user"]:
            out["user"].append(i["id"])
    # (fprintd from the repositories needs the repo's libfprint, which the AUR's
    # libfprint-tod replaces: with an AUR driver, fprintd goes with the AUR's)
    if "libfprint-tod" in out["aur"] and "fprintd" in out["install"]:
        out["install"].remove("fprintd")
        out["aur"].append("fprintd")
    if out["root"]:
        out["rootArgs"] = [",".join(out["root"])]
        face = by.get("face")
        if "face" in out["root"] and face:
            out["rootArgs"] += ["--face-camera", face["camera"]]
        fp = by.get("fingerprint")
        if "fingerprint" in out["root"] and fp and fp.get("driver"):
            out["rootArgs"] += ["--fprintd-wait"]
    return out


# ── The login screen's look (install.sh's, tessera-root checks it) ──────
def login_look(c=None):
    c = read_config() if c is None else c
    a = c.get("appearance") if isinstance(c.get("appearance"), dict) else {}
    l = c.get("lock") if isinstance(c.get("lock"), dict) else {}
    pick = lambda v, ok, d: v if v in ok else d
    whole = lambda v, lo, hi, d: v if type(v) is int and lo <= v <= hi else d
    seed = a.get("materialSeed", "")
    return json.dumps({
        "clockFace": pick(l.get("clockFace"), ("dynamic", "stacked", "wide", "overlap", "analog", "weather"), "dynamic"),
        "clockRound": l.get("clockRound") if type(l.get("clockRound")) is bool else True,
        "clockWeight": whole(l.get("clockWeight"), 100, 1000, 640),
        "clockWidth": whole(l.get("clockWidth"), 25, 151, 112),
        "look": pick(a.get("look"), ("glass", "material"), "glass"),
        "materialStyle": pick(a.get("materialStyle"), ("tonalSpot", "vibrant", "expressive", "neutral", "content", "fidelity", "monochrome"), "tonalSpot"),
        "mode": "light" if a.get("mode") == "light" else "dark",
        "seed": seed if isinstance(seed, str) and len(seed) == 7 and seed.startswith("#") else "#6750A4",
        "style": pick(l.get("loginStyle"), ("look", "text", "plain"), "look"),
    }, sort_keys=True, separators=(",", ":"))


# ── Writing your settings ───────────────────────────────────────────────
def edit_config(change):
    """config.json changed by change(dict) and written whole (a temporary
    file, then a rename); left alone when it isn't Tessera's settings."""
    p = config_path()
    try:
        with open(p) as f:
            c = json.load(f)
    except FileNotFoundError:
        c = {"welcome": {"done": False}}
    except (OSError, ValueError):
        return False
    if not isinstance(c, dict):
        return False
    change(c)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    with open(p + ".tmp", "w") as f:
        json.dump(c, f, indent=4)
    os.replace(p + ".tmp", p)
    return True


def part(c, k):
    if not isinstance(c.get(k), dict):
        c[k] = {}
    return c[k]


def record(chosen=None, declined=None, offer=None):
    def change(c):
        f = part(c, "features")
        if chosen is not None:
            f["chosen"] = sorted(set(f.get("chosen") or []) | set(chosen))
            f["declined"] = sorted(set(f.get("declined") or []) - set(chosen))
        if declined is not None:
            f["declined"] = sorted(set(f.get("declined") or []) | set(declined))
        if offer is not None:
            f["offer"] = offer
    return edit_config(change)


# ── The parts done as you ───────────────────────────────────────────────
def run_quiet(cmd, timeout):
    try:
        p = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout, stdin=subprocess.DEVNULL)
    except (OSError, subprocess.SubprocessError) as e:
        return 1, str(e)
    last = ((p.stderr or "") + (p.stdout or "")).strip().splitlines()
    return p.returncode, last[-1] if last else ""


def apply_user(items, ids, dry=False):
    """Yields {"id", "ok", "error", "fix"} for each of ids with a user part."""
    by = {i["id"]: i for i in items}
    for id in ids:
        i = by.get(id)
        if not i or not i["user"]:
            continue
        ok, err = True, ""
        if dry:
            yield {"id": id, "ok": True, "error": "", "fix": "", "dry": True}
            continue
        if id == "face":
            # (Settings' Face goes by what Tessera set up: lock.faceDevice and its kind)
            cam, kind = i.get("camera", ""), i.get("kind", "ir")
            ok = record_face(cam, kind)
            err = "" if ok else "Your settings couldn't be written"
            if ok and kind == "rgb":
                code, last = run_quiet([sys.executable, "-I", os.path.join(HERE, "face_live.py"), "models"], 600)
                ok, err = code == 0, "" if code == 0 else "The eye model for the blink didn't download: " + last
        elif id == "battery":
            code, last = run_quiet(["bash", os.path.join(HERE, "battery-guard-install.sh"), "enable"], 60)
            ok, err = code == 0, "" if code == 0 else last or "The battery guard didn't start"
        elif id == "look":
            code, last = run_quiet([sys.executable, "-I", os.path.join(HERE, "desktop-look.py"), "install", "--text"], 600)
            ok, err = code == 0, "" if code == 0 else last or "The desktop look wasn't set up"
        elif id == "captions":
            code, last = run_quiet([sys.executable, "-I", os.path.join(HERE, "dictate.py"), "download", "tiny"], 1800)
            ok, err = code == 0, "" if code == 0 else "The speech model didn't download: " + (last or "check the internet connection")
        yield {"id": id, "ok": ok, "error": err, "fix": "" if ok else i["fix"]}


def record_face(cam, kind):
    def change(c):
        l = part(c, "lock")
        l["faceDevice"] = cam
        l["faceKind"] = "rgb" if kind == "rgb" else "ir"
    return bool(cam) and edit_config(change)


# ── Command line ────────────────────────────────────────────────────────
def main(argv):
    facts = None
    if argv[:1] == ["--facts"] and len(argv) > 1:
        with open(argv[1]) as f:
            facts = json.load(f)
        argv = argv[2:]
    cmd = argv[0] if argv else ""
    if cmd == "login-look":
        print(login_look())
        return 0
    if cmd == "record":
        vals = {}
        for k in ("--chosen", "--declined", "--offer"):
            if k in argv and argv.index(k) + 1 < len(argv):
                vals[k[2:]] = argv[argv.index(k) + 1]
        offer = vals.get("offer")
        if offer not in (None, "pending", "done"):
            print("--offer: pending or done", file=sys.stderr)
            return 2
        split = lambda v: None if v is None else [x for x in v.split(",") if re.fullmatch(r"[a-z]+", x)]
        return 0 if record(split(vals.get("chosen")), split(vals.get("declined")), offer) else 1
    if cmd not in ("list", "missing", "plan", "apply-user"):
        print(__doc__, file=sys.stderr)
        return 2
    f = collect() if facts is None else facts
    items = plan(f)
    if cmd == "list":
        if argv[1:2] == ["--json"]:
            print(json.dumps({"groups": GROUPS, "items": items}))
        else:
            for i in items:
                print("\t".join([i["id"], "1" if i["setUp"] else "0", i["group"], i["title"], i["detail"],
                                 " ".join(i["packages"]), " ".join(i["aur"])]))
        return 0
    if cmd == "missing":
        print(json.dumps({"groups": GROUPS, "items": missing(items, f)}))
        return 0
    ids = [x for a in argv[1:] if not a.startswith("--") for x in a.split(",") if x]
    if cmd == "plan":
        out = plan_for(items, ids)
        if "login" in out["root"]:
            # (the login screen's look from your settings, and this shell's
            # folder for its copy: tessera-root checks both)
            out["rootArgs"] += ["--login-look", login_look(), "--shell", os.path.dirname(os.path.dirname(HERE))]
        print(json.dumps(out))
        return 0
    text = "--text" in argv
    failed = 0
    titles = {i["id"]: i["title"] for i in items}
    for r in apply_user(items, ids, dry="--dry-run" in argv):
        failed += 0 if r["ok"] else 1
        if text:
            print(("would set up " if r.get("dry") else "✓ " if r["ok"] else "! ") + titles[r["id"]]
                  + ("" if r["ok"] else ": " + r["error"] + ". " + r["fix"]), flush=True)
        else:
            print(json.dumps(r), flush=True)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
