#!/usr/bin/env python3
"""The App Store (services/AppStore.qml, and Spotlight's "Get" rows): apps
from three places, searched together.

  - the Arch repos: their AppStream catalogue (the archlinux-appstream-data
    package): names, summaries, categories and icons for the desktop apps
    among the packages; read once and kept as JSON in the cache folder
  - the AUR: its search API (aur.archlinux.org/rpc), community packages
    built from scripts anyone can upload: marked, with their votes and
    popularity; the logo is the same app's from the catalogue or Flathub
    when the names match, else none (the shell draws a tile)
  - Flathub: its API (flathub.org/api/v2), sandboxed Flatpak apps

Installing and removing are the shell's (Software Update for packages,
flatpak for Flathub); this only finds things.

  appstore.py --serve     requests as JSON lines on stdin, answers on stdout:
      {"id", "op": "search", "q", "limit", "sources"}   {"id", "apps": [app, ...]}
          (sources: repo, flathub, aur; the repos and Flathub unless asked:
          the AUR only for "aur: name" or sources ["aur"])
      {"id", "op": "discover"}                    {"id", "sections": [{"title", "apps"}]}
      {"id", "op": "category", "category"}        {"id", "apps": [...]}
      {"id", "op": "installed"}                   {"id", "apps": [...]}
      {"id", "op": "status"}                      {"id", "status": {"catalogue", "aur", "flatpak", "helper"}}
  appstore.py search <words>   one search, printed (for trying it out)

An app: {"key": "<source>:<package or app id>", "source": "repo" | "aur" |
"flathub", "pkg", "name", "summary", "description", "icon" (a file or a URL;
"" for a tile), "iconName" (an icon theme name to try first), "categories",
"installed", "desktop" (its desktop file id, to open it), "homepage",
"developer", "verified" (Flathub), "votes", "popularity" (AUR), "installs"
(Flathub, last month), "version" (AUR), "repo" (core, extra, multilib)}
"""
import concurrent.futures
import glob
import gzip
import hashlib
import json
import os
import re
import select
import shutil
import subprocess
import sys
import time
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET

# (IRICE_APPSTREAM: another folder holding xml/ and icons/, for tests)
_AS = os.environ.get("IRICE_APPSTREAM")
CATALOGUE = (os.path.join(_AS, "xml"),) if _AS else ("/usr/share/swcatalog/xml", "/usr/share/app-info/xmls")
CATALOGUE_ICONS = (os.path.join(_AS, "icons"),) if _AS else ("/usr/share/swcatalog/icons", "/usr/share/app-info/icons")
AUR = "https://aur.archlinux.org/rpc/v5"
FLATHUB = "https://flathub.org/api/v2"
TIMEOUT = 4  # seconds a source gets before the others answer without it
LANG = "{http://www.w3.org/XML/1998/namespace}lang"

# The store's categories: freedesktop main categories (the catalogue's) and
# Flathub's
CATEGORIES = {
    "Productivity": ["Office"],
    "Development": ["Development"],
    "Graphics & Design": ["Graphics"],
    "Music & Video": ["AudioVideo"],
    "Games": ["Game"],
    "Internet": ["Network"],
    "Education & Science": ["Education", "Science"],
    "Utilities": ["Utility"],
    "System": ["System"],
}
# Well-known apps from the repos, for Discover (only the ones the catalogue has)
REPO_PICKS = ["firefox", "chromium", "thunderbird", "libreoffice-fresh", "gimp", "inkscape", "krita", "blender",
              "kdenlive", "obs-studio", "vlc", "mpv", "audacity", "steam", "telegram-desktop", "signal-desktop",
              "keepassxc", "qbittorrent", "code", "darktable", "shotcut", "handbrake", "foliate", "zed"]


def cache_dir():
    base = os.environ.get("XDG_CACHE_HOME") or os.path.expanduser("~/.cache")
    d = os.path.join(base, "irice", "appstore")
    os.makedirs(os.path.join(d, "icons"), exist_ok=True)
    return d


def fetch_json(url, data=None, timeout=TIMEOUT):
    req = urllib.request.Request(url, data=json.dumps(data).encode() if data is not None else None,
                                 headers={"User-Agent": "iRice Software", "Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read().decode())


# ── The repos' catalogue ─────────────────────────────────────────────────
def catalogue_files():
    out = []
    for d in CATALOGUE:
        out += sorted(glob.glob(os.path.join(d, "*.xml.gz")) + glob.glob(os.path.join(d, "*.xml")))
    return out


def text_of(el, tag):
    """The untranslated text of a child (<name>, <summary>)"""
    for c in el.findall(tag):
        if c.get(LANG) is None:
            return (c.text or "").strip()
    return ""


def parse_catalogue(stream, origin=""):
    """AppStream collection XML -> [app] (desktop apps only)"""
    apps = []
    repo = origin.rsplit("-", 1)[-1] if origin else ""
    for _, el in ET.iterparse(stream, events=("end",)):
        if el.tag == "components":
            repo = (el.get("origin") or origin).rsplit("-", 1)[-1]
        if el.tag != "component":
            continue
        if el.get("type") in ("desktop-application", "desktop") and el.findtext("pkgname"):
            desc = el.find("description")
            paras = [" ".join("".join(p.itertext()).split()) for p in (desc.findall("p") if desc is not None else [])]
            cached = sorted(((int(i.get("width") or 0), (i.text or "").strip()) for i in el.findall("icon") if i.get("type") == "cached"), reverse=True)
            stock = next(((i.text or "").strip() for i in el.findall("icon") if i.get("type") == "stock"), "")
            shots = []
            for s in el.findall("screenshots/screenshot"):
                img = next((i.text.strip() for i in s.findall("image") if i.text and i.get("type") == "source"), "")
                if img:
                    shots.append(img)
            apps.append({
                "key": "repo:" + el.findtext("pkgname").strip(),
                "source": "repo", "repo": repo, "pkg": el.findtext("pkgname").strip(),
                "id": (el.findtext("id") or "").strip(),
                "name": text_of(el, "name"), "summary": text_of(el, "summary"),
                "description": "\n\n".join(p for p in paras if p)[:1500],
                "iconName": stock, "cached": [c[1] for c in cached][:1],
                "categories": [(c.text or "").strip() for c in el.findall("categories/category")],
                "keywords": [(k.text or "").strip().lower() for k in el.findall("keywords/keyword") if k.get(LANG) is None],
                "desktop": next(((l.text or "").strip() for l in el.findall("launchable") if l.get("type") == "desktop-id"), ""),
                "homepage": next(((u.text or "").strip() for u in el.findall("url") if u.get("type") == "homepage"), ""),
                "developer": text_of(el, "developer_name") or text_of(el, "developer/name"),
                "screenshots": shots[:4],
            })
        el.clear()
    return apps


def load_catalogue():
    """Every repo app, from the cache when the catalogue hasn't changed"""
    files = catalogue_files()
    if not files:
        return []
    sig = hashlib.sha1("|".join(f"{f}:{os.path.getmtime(f)}" for f in files).encode()).hexdigest()[:12]
    path = os.path.join(cache_dir(), f"repo-{sig}.json")
    try:
        with open(path) as f:
            return json.load(f)
    except (OSError, ValueError):
        pass
    apps = []
    for f in files:
        origin = os.path.basename(f).split(".")[0]
        with (gzip.open(f) if f.endswith(".gz") else open(f, "rb")) as s:
            apps += parse_catalogue(s, origin)
    for old in glob.glob(os.path.join(cache_dir(), "repo-*.json")):
        os.remove(old)
    tmp = path + ".tmp"
    with open(tmp, "w") as f:
        json.dump(apps, f)
    os.replace(tmp, path)
    return apps


def catalogue_icon(app):
    """The catalogue's own icon as a PNG Qt can show (they're JPEG XL: djxl
    converts one once, into the cache), or ''"""
    for name in app.get("cached", []):
        for root in CATALOGUE_ICONS:
            for size in ("128x128", "64x64", "48x48"):
                for src in glob.glob(os.path.join(root, f"*-{app.get('repo', '*')}", size, name)) or glob.glob(os.path.join(root, "*", size, name)):
                    if not src.endswith(".jxl"):
                        return src
                    out = os.path.join(cache_dir(), "icons", os.path.basename(src)[:-4] + ".png")
                    if os.path.exists(out):
                        return out
                    if shutil.which("djxl") and subprocess.run(["djxl", src, out], capture_output=True).returncode == 0:
                        return out
    return ""


# ── What's installed ─────────────────────────────────────────────────────
def installed_packages():
    try:
        return set(subprocess.run(["pacman", "-Qq"], capture_output=True, text=True, timeout=10).stdout.split())
    except (OSError, subprocess.TimeoutExpired):
        return set()


def installed_flatpaks():
    if not shutil.which("flatpak"):
        return set()
    try:
        r = subprocess.run(["flatpak", "list", "--app", "--columns=application"], capture_output=True, text=True, timeout=10)
        return set(r.stdout.split())
    except (OSError, subprocess.TimeoutExpired):
        return set()


# ── Matching ─────────────────────────────────────────────────────────────
VARIANT = re.compile(r"-(bin|git|appimage|beta|nightly|wayland|electron|stable|latest|dev|edge|canary|desktop)$")


def base_name(pkg):
    """'obsidian-bin' -> 'obsidian' (the AUR's variants of one app)"""
    p = pkg.lower()
    while VARIANT.search(p):
        p = VARIANT.sub("", p)
    return p


def norm(s):
    return re.sub(r"[^a-z0-9]+", "", (s or "").lower())


def match_score(app, q):
    """How well an app matches what was typed: 0 none … 100 its name exactly"""
    q = q.strip().lower()
    if not q:
        return 0
    name = (app.get("name") or "").lower()
    pkg = (app.get("pkg") or "").lower()
    nq = norm(q)
    if norm(name) == nq or base_name(pkg) == q or norm(base_name(pkg)) == nq:
        return 100
    words_of = re.split(r"[\s\-_.]+", name) + re.split(r"[\s\-_.]+", base_name(pkg))
    if q in words_of:
        return 85  # ("chrome" in Google Chrome)
    if name.startswith(q) or pkg.startswith(q):
        return 80
    if any(w.startswith(q) for w in words_of):
        return 65
    keywords = [k.lower() for k in app.get("keywords") or []]
    if q in keywords or nq in (norm(k) for k in keywords):
        return 75  # ("vscode" is Visual Studio Code's keyword)
    if q in name or q in pkg:
        return 50
    words = q.split()
    if words and all(w in name + " " + pkg for w in words):
        return 45
    hay = " ".join([name, pkg, (app.get("summary") or "").lower(), " ".join(keywords)])
    if words and all(w in hay for w in words):
        return 30
    return 0


SOURCE_RANK = {"repo": 3, "flathub": 2, "aur": 1}


def quality(a):
    """How established it is, 0-15: the repos' apps are; Flathub's by their
    installs, the AUR's by popularity and votes (and a package hardly anyone
    uses goes down)"""
    if a["source"] == "repo":
        return 10
    if a["source"] == "flathub":
        return min(15, len(str(int(a.get("installs") or 0))) * 2.5)
    pop, votes = a.get("popularity", 0) or 0, a.get("votes", 0) or 0
    return min(15, pop * 3 + min(votes, 300) / 60) - (10 if pop < 0.1 and votes < 5 else 0)


def rank(apps, q):
    scored = [(match_score(a, q), a) for a in apps]
    scored = [(s + quality(a), a) for s, a in scored if s > 0]

    def key(x):
        s, a = x
        return (-s, -SOURCE_RANK.get(a["source"], 0), (a.get("name") or "").lower())
    ranked = [a for _, a in sorted(scored, key=key)]
    # One app in several places: the repos' copy first (native, signed by
    # Arch, updated with the system), where the best of them ranked
    out, placed = [], set()
    for a in ranked:
        k = norm(a.get("name"))
        if k in placed:
            continue
        group = [b for b in ranked if norm(b.get("name")) == k]
        group.sort(key=lambda b: -SOURCE_RANK.get(b["source"], 0))
        out += group
        placed.add(k)
    return out


# AUR packages that aren't apps: libraries for a language, themes, icons,
# fonts, plugins and add-ons, drivers, docs (the AUR doesn't say what's an app)
NOT_APP = re.compile(r"^(python|python2|perl|ruby|lua|r|haskell|nodejs|node|go|golang|rust|php|ocaml|vim|neovim|nvim|emacs|"
                     r"zsh|fish|bash|gnome-shell-extension|kodi-addon|obs|gimp-plugin|ttf|otf|nerd-fonts|lib)-"
                     r"|theme|icons?\b|icon-theme|cursors?\b|fonts?\b|plugin|addon|extension|kcm|branding|dkms|headers|firmware"
                     r"|-docs?$|-dev$|-devel$|-debug$|-bin-debug$|-data$|-common$|-l10n|-i18n|-lang$")


def pretty(pkg):
    """A name to show for an AUR package: 'google-chrome' -> 'Google Chrome'"""
    return " ".join(w[:1].upper() + w[1:] for w in base_name(pkg).replace("_", "-").split("-") if w) or pkg


def aur_apps(results, repo_pkgs):
    """AUR search results -> apps: one per app (its most popular variant),
    not ones the repos have, not libraries, themes, plugins and the like"""
    best = {}
    for r in results:
        name = r.get("Name", "")
        if NOT_APP.search(name.lower()) or base_name(name) in repo_pkgs or name in repo_pkgs:
            continue
        b = base_name(name)
        if b not in best or (r.get("Popularity", 0), r.get("NumVotes", 0)) > (best[b].get("Popularity", 0), best[b].get("NumVotes", 0)):
            best[b] = r
    return [{
        "key": "aur:" + r["Name"], "source": "aur", "pkg": r["Name"],
        "name": pretty(r["Name"]), "summary": r.get("Description") or "", "description": r.get("Description") or "",
        # (no theme icon by its name: themes fall back on dashes, so every
        # "spotify-something" would show Spotify's; borrow_icons sets one)
        "iconName": "", "icon": "", "categories": [],
        "homepage": r.get("URL") or "", "developer": r.get("Maintainer") or "",
        "votes": r.get("NumVotes", 0), "popularity": round(r.get("Popularity", 0), 2),
        "version": r.get("Version", ""), "outOfDate": bool(r.get("OutOfDate")),
    } for r in best.values()]


def flathub_app(h):
    return {
        "key": "flathub:" + h["app_id"], "source": "flathub", "pkg": h["app_id"],
        "name": h.get("name") or h["app_id"], "summary": h.get("summary") or "",
        "description": (h.get("description") or "")[:1500],
        "icon": h.get("icon") or "", "iconName": h["app_id"], "categories": [h["main_categories"]] if isinstance(h.get("main_categories"), str) else (h.get("main_categories") or []),
        "developer": h.get("developer_name") or "", "verified": bool(h.get("verification_verified")),
        "keywords": [k for k in (h.get("keywords") or []) if isinstance(k, str)],
        "installs": h.get("installs_last_month") or 0, "desktop": h["app_id"] + ".desktop",
    }


def twin_keys(a):
    """Names an app is known by: its name, its package's base, and for a
    Flathub app the parts of its id ("app.zen_browser.zen": zenbrowser, zen)"""
    keys = {norm(a.get("name")), norm(base_name(a.get("pkg", "")))}
    if a.get("source") == "flathub":
        parts = a["pkg"].split(".")
        keys |= {norm(p) for p in parts[1:]}
    return {k for k in keys if len(k) >= 3}


def borrow_icons(aur, others):
    """An AUR app gets the same app's logo (and proper name) from the repos or
    Flathub when it's that app: an established package (some popularity or
    votes) whose description or homepage names it. Else it stays a tile: a
    little-known package that only shares a word with a famous app doesn't
    get its logo."""
    by = {}
    for a in others:
        if a.get("icon") or a.get("iconName"):
            for k in twin_keys(a):
                by.setdefault(k, a)
    for a in aur:
        if a.get("popularity", 0) < 0.5 and a.get("votes", 0) < 10:
            continue
        mine = norm(base_name(a["pkg"]))
        twin = by.get(mine)
        if not twin:
            continue
        said = norm(a.get("summary")) + "|" + norm(a.get("homepage"))
        words = [norm(w) for w in re.split(r"[\s\-_.]+", twin.get("name") or "") if len(norm(w)) >= 3]
        if not (words and all(w in said for w in words)) and mine not in said:
            continue  # (its description and homepage don't name that app)
        a["icon"] = twin.get("icon", "")
        if twin.get("iconName"):
            a["iconName"] = twin["iconName"]
        a["name"] = twin.get("name") or a["name"]
    return aur


# ── The store ────────────────────────────────────────────────────────────
class Store:
    def __init__(self, fetch=fetch_json):
        self.fetch = fetch
        self.repo = load_catalogue()
        self.repo_pkgs = {a["pkg"] for a in self.repo}
        self.pkgs = installed_packages()
        self.flatpaks = installed_flatpaks()
        self.pool = concurrent.futures.ThreadPoolExecutor(max_workers=4)
        self.cache = {}  # (query, sources, limit) -> (when, apps)

    def refresh(self):
        self.pkgs = installed_packages()
        self.flatpaks = installed_flatpaks()

    def finish(self, apps):
        """installed flags, and the catalogue's icons for the ones shown"""
        for a in apps:
            a["installed"] = a["pkg"] in (self.flatpaks if a["source"] == "flathub" else self.pkgs)
            if a["source"] == "repo" and not a.get("icon"):
                a["icon"] = catalogue_icon(a)
            a.pop("cached", None)
            a.pop("keywords", None)
        return apps

    def remote(self, fn, default):
        try:
            return fn()
        except Exception:  # noqa: BLE001 (offline, or a source down: the others still answer)
            return default

    def search(self, q, limit=30, sources=None, partial=None):
        """The repos and Flathub; the AUR only when asked for ("aur: name",
        or sources=["aur"]: community packages aren't offered unasked).
        partial(apps): the repos' matches at once (they're on this computer),
        before the network answers. The same search again within ten
        minutes comes from memory."""
        q = " ".join(q.split())[:80]
        if q.lower().startswith("aur:"):
            q, sources = q[4:].strip(), ["aur"]
        sources = set(sources or ["repo", "flathub"])
        if len(q) < 2:
            return []
        key = (q.lower(), tuple(sorted(sources)), limit)
        hit = self.cache.get(key)
        if hit and time.monotonic() - hit[0] < 600:
            return self.finish([dict(a) for a in hit[1]])
        if partial and "repo" in sources:
            partial(self.finish(rank([dict(a) for a in self.repo if match_score(a, q) > 0], q)[:limit]))
        apps = self.search_all(q, limit, sources)
        self.cache[key] = (time.monotonic(), [dict(a) for a in apps])
        if len(self.cache) > 200:
            self.cache.pop(next(iter(self.cache)))
        return apps

    def search_all(self, q, limit, sources):
        aur = self.pool.submit(self.remote, lambda: self.fetch(f"{AUR}/search/{urllib.parse.quote(q)}?by=name-desc").get("results", []), []) if "aur" in sources else None
        flat = self.pool.submit(self.remote, lambda: self.fetch(f"{FLATHUB}/search", {"query": q}).get("hits", []), [])
        repo = [dict(a) for a in self.repo if match_score(a, q) > 0]
        hits = [h for h in flat.result() if h.get("type") in (None, "desktop-application", "desktop") and h.get("app_id")]
        flathub = [flathub_app(h) for h in hits]
        # (an AUR search still looks at the others: for the same app's logo)
        aur_found = borrow_icons(aur_apps(aur.result(), self.repo_pkgs), repo + flathub) if aur else []
        shown = (repo if "repo" in sources else []) + (flathub if "flathub" in sources else []) + aur_found
        return self.finish(rank(shown, q)[:limit])

    def category(self, name, limit=60):
        cats = CATEGORIES.get(name, [name])
        flat = []
        for c in cats:
            flat += self.remote(lambda c=c: self.fetch(f"{FLATHUB}/collection/category/{c}?page=1&per_page=24").get("hits", []), [])
        apps = [flathub_app(h) for h in flat if h.get("app_id")]
        repo = sorted((dict(a) for a in self.repo if set(a["categories"]) & set(cats)), key=lambda a: a["name"].lower())
        # (the repos' copy of an app first, then Flathub's popular ones)
        seen = {norm(a["name"]) for a in repo}
        picked = [a for a in repo if a["pkg"] in REPO_PICKS] + [a for a in apps if norm(a["name"]) not in seen] + [a for a in repo if a["pkg"] not in REPO_PICKS]
        return self.finish(picked[:limit])

    def discover(self):
        popular = self.remote(lambda: self.fetch(f"{FLATHUB}/collection/popular?page=1&per_page=18").get("hits", []), [])
        picks = [dict(a) for p in REPO_PICKS for a in self.repo if a["pkg"] == p][:12]
        sections = []
        if picks:
            sections.append({"title": "Essentials", "apps": self.finish(picks)})
        if popular:
            sections.append({"title": "Popular on Flathub", "apps": self.finish([flathub_app(h) for h in popular if h.get("app_id")])})
        return sections

    def installed(self):
        self.refresh()
        apps = [dict(a) for a in self.repo if a["pkg"] in self.pkgs]
        known = {a["pkg"] for a in apps}
        # AUR and other foreign packages with an app in them
        try:
            foreign = subprocess.run(["pacman", "-Qm"], capture_output=True, text=True, timeout=10).stdout.splitlines()
        except (OSError, subprocess.TimeoutExpired):
            foreign = []
        for line in foreign:
            pkg, _, version = line.partition(" ")
            if pkg in known:
                continue
            desktops = [d for d in glob.glob(f"/usr/share/applications/{pkg}*.desktop")]
            if desktops:
                apps.append({"key": "aur:" + pkg, "source": "aur", "pkg": pkg, "name": pkg, "summary": "", "icon": "",
                             "iconName": os.path.basename(desktops[0])[:-8], "categories": [], "version": version,
                             "desktop": os.path.basename(desktops[0])})
        for app_id in sorted(self.flatpaks):
            apps.append({"key": "flathub:" + app_id, "source": "flathub", "pkg": app_id, "name": app_id.split(".")[-1],
                         "summary": "", "icon": "", "iconName": app_id, "categories": [], "desktop": app_id + ".desktop"})
        return self.finish(sorted(apps, key=lambda a: (a.get("name") or a["pkg"]).lower()))

    def status(self):
        return {"catalogue": bool(self.repo), "flatpak": bool(shutil.which("flatpak")),
                "helper": shutil.which("yay") and "yay" or shutil.which("paru") and "paru" or ""}


class Requests:
    """Lines from stdin, all that are waiting at once (the first one waited
    for). Read raw: Python's own buffering would hide waiting lines from
    select."""
    def __init__(self, fd=0):
        self.fd, self.buf, self.ended = fd, b"", False

    def take(self):
        while b"\n" not in self.buf and not self.ended:
            chunk = os.read(self.fd, 65536)
            self.buf += chunk
            self.ended = not chunk
        while not self.ended and select.select([self.fd], [], [], 0)[0]:
            chunk = os.read(self.fd, 65536)
            self.buf += chunk
            self.ended = not chunk
        *lines, self.buf = self.buf.split(b"\n")
        if self.ended and self.buf:
            lines, self.buf = lines + [self.buf], b""
        return [l.decode(errors="replace") for l in lines if l.strip()], self.ended


def latest_only(reqs):
    """Searches typed past while one was being answered: only the newest is
    worth doing (the others are answered as stale, so nothing waits on them)"""
    searches = [r for r in reqs if isinstance(r, dict) and r.get("op") == "search"]
    stale = searches[:-1]
    return [r for r in reqs if r not in stale], stale


def serve():
    store = Store()

    def say(out):
        print(json.dumps(out), flush=True)

    def search(r):
        stream = r.get("stream")
        return {"apps": store.search(r.get("q", ""), int(r.get("limit", 30)), r.get("sources"),
                                     (lambda apps: say({"id": r.get("id"), "partial": True, "apps": apps})) if stream else None)}
    ops = {
        "search": search,
        "discover": lambda r: {"sections": store.discover()},
        "category": lambda r: {"apps": store.category(r.get("category", ""))},
        "installed": lambda r: {"apps": store.installed()},
        "status": lambda r: {"status": store.status()},
        "refresh": lambda r: (store.refresh(), {"ok": True})[1],
    }
    requests = Requests()
    while True:
        lines, ended = requests.take()
        reqs = []
        for line in lines:
            try:
                reqs.append(json.loads(line))
            except ValueError:
                pass
        todo, stale = latest_only(reqs)
        for r in stale:
            say({"id": r.get("id"), "stale": True})
        for req in todo:
            try:
                out = ops[req.get("op", "")](req)
            except Exception as e:  # noqa: BLE001
                out = {"error": str(e)}
            out["id"] = req.get("id") if isinstance(req, dict) else None
            say(out)
        if ended:
            return


if __name__ == "__main__":
    if sys.argv[1:2] == ["--serve"]:
        serve()
    elif sys.argv[1:2] == ["search"] and len(sys.argv) > 2:
        for a in Store().search(" ".join(sys.argv[2:]), 15):
            print(f"{a['source']:8} {a['name'][:28]:28} {a['pkg'][:30]:30} {'installed' if a['installed'] else ''} {a['icon'][:60]}")
    else:
        print(__doc__, file=sys.stderr)
        sys.exit(2)
