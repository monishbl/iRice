#!/usr/bin/env python3
"""Icon index for iRice: every icon name in a theme (and the themes it
inherits, then hicolor and /usr/share/pixmaps) mapped to the file to draw,
preferring scalable and large icons over small ones, and full-colour icons
over symbolic ones. Qt's own theme lookup depends on the platform theme and
desktop settings and can come back empty; real file paths always load.

    icon-index.py <theme> <cache file>
writes one "name<TAB>path" line per icon to the cache file, unless it's
already up to date (newer than the themes, and less than a week old).
"""
import os
import sys
import time


def theme_dirs():
    home = os.path.expanduser("~")
    dirs = [os.path.join(os.environ.get("XDG_DATA_HOME") or os.path.join(home, ".local/share"), "icons"), os.path.join(home, ".icons")]
    for d in (os.environ.get("XDG_DATA_DIRS") or "/usr/local/share:/usr/share").split(":"):
        if d:
            dirs.append(os.path.join(d, "icons"))
    seen, out = set(), []
    for d in dirs:
        if d not in seen and os.path.isdir(d):
            seen.add(d)
            out.append(d)
    return out


ROOTS = theme_dirs()


def theme_paths(name):
    return [os.path.join(r, name) for r in ROOTS if os.path.isfile(os.path.join(r, name, "index.theme"))]


def inherits(name):
    for p in theme_paths(name):
        try:
            for line in open(os.path.join(p, "index.theme"), encoding="utf-8", errors="ignore"):
                if line.startswith("Inherits="):
                    return [t.strip() for t in line.split("=", 1)[1].split(",") if t.strip()]
        except OSError:
            pass
    return []


def chain(name):
    order, todo = [], [name]
    while todo:
        t = todo.pop(0)
        if t in order or not theme_paths(t):
            continue
        order.append(t)
        todo.extend(inherits(t))
    if "hicolor" not in order:
        order.append("hicolor")
    return order


def score(path):
    """Higher is better: scalable > big > small; symbolic last."""
    low = path.lower()
    s = 0
    if "symbolic" in low:
        s -= 1000
    if "scalable" in low:
        s += 500
    for part in low.split("/"):
        digits = part.split("x")[0].split("@")[0]
        if digits.isdigit():
            s += min(int(digits), 256)
            break
    if "@2x" in low:
        s += 50
    if low.endswith(".svg"):
        s += 10
    return s


def walk_theme(base):
    # Themes commonly symlink subdirectories; visit targets once, avoiding loops.
    seen = set()
    for directory, dirs, files in os.walk(base, followlinks=True):
        identity = os.stat(directory)
        key = (identity.st_dev, identity.st_ino)
        if key in seen:
            dirs[:] = []
            continue
        seen.add(key)
        yield directory, dirs, files


def fresh(cache, themes):
    try:
        made = os.path.getmtime(cache)
    except OSError:
        return False
    if time.time() - made > 7 * 86400:
        return False
    for t in themes:
        for p in theme_paths(t):
            # Nested additions change directory mtimes; replacements change file
            # mtimes. Checking only the theme root misses both cases.
            try:
                for directory, dirs, files in walk_theme(p):
                    if os.path.getmtime(directory) > made:
                        return False
                    for name in files:
                        if os.path.getmtime(os.path.join(directory, name)) > made:
                            return False
            except OSError:
                return False
    try:
        for directory, dirs, files in walk_theme('/usr/share/pixmaps'):
            if os.path.getmtime(directory) > made or any(os.path.getmtime(os.path.join(directory, f)) > made for f in files):
                return False
    except OSError:
        return False
    return True


def main():
    theme = sys.argv[1] if len(sys.argv) > 1 else "hicolor"
    cache = sys.argv[2] if len(sys.argv) > 2 else ""
    if cache and fresh(cache, chain(theme)):
        return
    best = {}  # name -> (theme rank, -score, path): the first theme in the chain wins, then the best file
    for rank, t in enumerate(chain(theme)):
        for base in theme_paths(t):
            for dirpath, _dirs, files in walk_theme(base):
                for f in files:
                    if not f.endswith((".svg", ".png")):
                        continue
                    name = f[:-4]
                    path = os.path.join(dirpath, f)
                    key = (rank, -score(path))
                    cur = best.get(name)
                    if cur is None or key < cur[0]:
                        best[name] = (key, path)
    for d in ("/usr/share/pixmaps",):
        if os.path.isdir(d):
            for f in os.listdir(d):
                if f.endswith((".svg", ".png")) and f[:-4] not in best:
                    best[f[:-4]] = ((99, 0), os.path.join(d, f))
    lines = "".join(name + "\t" + path + "\n" for name, (_k, path) in best.items() if "\t" not in path and "\n" not in path)
    if not cache:
        sys.stdout.write(lines)
        return
    os.makedirs(os.path.dirname(cache), exist_ok=True)
    tmp = cache + ".new"
    with open(tmp, "w") as f:
        f.write(lines)
    os.replace(tmp, cache)


if __name__ == "__main__":
    main()
