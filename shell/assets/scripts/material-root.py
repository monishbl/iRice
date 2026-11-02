#!/usr/bin/env python3
"""The Material look's Quickshell root, assembled from links:
  material-root.py <shell dir> <material dir> <root>
Every file of the Glass shell (shell/) is linked into <root> at the same
place, except where the Material look has its own file (material/, the same
layout), which is linked instead; files only Material has are linked too.
Quickshell resolves `qs.common`, `qs.services`… inside the root it runs, so
a Glass file used unchanged in the Material look gets the Material Theme and
controls. Built fresh at every start (a new file in either tree is never
missed), into a new folder that then replaces the old one. Prints <root>.
"""
import os
import shutil
import sys

SKIP_DIRS = {".git", "__pycache__", "build"}  # (native/build: launch-shell.sh adds it by path)
SKIP_EXT = (".pyc",)


def files(top):
    out = {}
    for d, dirs, names in os.walk(top):
        dirs[:] = [x for x in dirs if x not in SKIP_DIRS]
        for n in names:
            if n.endswith(SKIP_EXT):
                continue
            p = os.path.join(d, n)
            out[os.path.relpath(p, top)] = os.path.realpath(p)
    return out


def build(shell, material, root):
    shell, material = os.path.realpath(shell), os.path.realpath(material)
    if not os.path.isfile(os.path.join(shell, "shell.qml")) or not os.path.isfile(os.path.join(material, "shell.qml")):
        raise SystemExit("material-root: no shell.qml in " + shell + " or " + material)
    links = files(shell)
    links.update(files(material))  # (Material's own files win)
    parent = os.path.dirname(os.path.abspath(root))
    os.makedirs(parent, exist_ok=True)
    tmp = root + ".new"
    shutil.rmtree(tmp, ignore_errors=True)
    for rel, target in links.items():
        dest = os.path.join(tmp, rel)
        os.makedirs(os.path.dirname(dest), exist_ok=True)
        os.symlink(target, dest)
    old = root + ".old"
    shutil.rmtree(old, ignore_errors=True)
    if os.path.lexists(root):
        os.rename(root, old)
    os.rename(tmp, root)
    shutil.rmtree(old, ignore_errors=True)
    return root


if __name__ == "__main__":
    if len(sys.argv) != 4:
        raise SystemExit(__doc__)
    print(build(*sys.argv[1:]))
