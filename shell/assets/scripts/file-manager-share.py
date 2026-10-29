#!/usr/bin/env python3
"""Adds "Send to a nearby device…" to file managers' right-click menus.

Settings → Sharing and install.sh run this. The menu entry runs
share-files.sh (next to this script) with the selected files, which opens
iRice's share sheet with them. Only our own entry is ever written or
removed: a file manager's other menu items, and anything you added
yourself, are left exactly as they were.

    file-manager-share.py status            JSON: each file manager, found? menu added?
    file-manager-share.py install [names]   add the entry (all found, or just these)
    file-manager-share.py remove [names]    take it out again

    Nautilus  ~/.local/share/nautilus-python/extensions/irice-share.py when
              nautilus-python is installed, else a Scripts entry
              (~/.local/share/nautilus/scripts/), which needs no extension
    Dolphin   ~/.local/share/kio/servicemenus/irice-share.desktop
    Thunar    an action in ~/.config/Thunar/uca.xml (merged, not replaced)
    Nemo      ~/.local/share/nemo/actions/irice-share.nemo_action
"""
import json
import os
import shutil
import stat
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

LABEL = "Send to a nearby device…"
SCRIPT = Path(__file__).resolve().parent / "share-files.sh"
UCA_ID = "irice-share"  # our action's unique id in Thunar's uca.xml


def data_home():
    return Path(os.environ.get("XDG_DATA_HOME") or Path.home() / ".local/share")


def config_home():
    return Path(os.environ.get("XDG_CONFIG_HOME") or Path.home() / ".config")


def quoted(path):
    # For Exec lines (desktop entries, Nemo, Thunar): a double-quoted path,
    # with the characters the spec reserves inside quotes escaped
    s = str(path)
    for c in ("\\", '"', "`", "$"):
        s = s.replace(c, "\\" + c)
    return '"' + s + '"'


def write(path, text, executable=False):
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(path.name + ".tmp")
    tmp.write_text(text)
    if executable:
        tmp.chmod(tmp.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    os.replace(tmp, path)


def nautilus_python():
    # The extension loader that nautilus-python (python-nautilus on Arch) installs
    for d in ("/usr/lib/nautilus/extensions-4", "/usr/lib64/nautilus/extensions-4", "/usr/lib/x86_64-linux-gnu/nautilus/extensions-4"):
        if Path(d, "libnautilus-python.so").exists():
            return True
    return False


# ── Each file manager: found, its files, how to add and remove ───────────

def nautilus_paths():
    return [data_home() / "nautilus-python/extensions/irice-share.py", data_home() / "nautilus/scripts" / LABEL]


def nautilus_install():
    ext, script = nautilus_paths()
    if nautilus_python():
        write(ext, f'''# iRice: "{LABEL}" in Nautilus (file-manager-share.py installed this)
import subprocess
from gi.repository import GObject, Nautilus

SCRIPT = {str(SCRIPT)!r}


class IriceShare(GObject.GObject, Nautilus.MenuProvider):
    def get_file_items(self, files):
        if not files:
            return []
        item = Nautilus.MenuItem(name="IriceShare::send", label={LABEL!r})
        item.connect("activate", lambda _item, fs=files: subprocess.Popen([SCRIPT] + [f.get_uri() for f in fs]))
        return [item]
''')
        if script.exists():
            script.unlink()
    else:
        # Scripts menu: Nautilus hands the selection over as URIs, one per line
        write(script, f'''#!/usr/bin/env bash
# iRice: send these to a nearby device (file-manager-share.py installed this)
mapfile -t uris <<< "$NAUTILUS_SCRIPT_SELECTED_URIS"
exec {quoted(SCRIPT)} "${{uris[@]}}"
''', executable=True)
        if ext.exists():
            ext.unlink()


def dolphin_paths():
    return [data_home() / "kio/servicemenus/irice-share.desktop"]


def dolphin_install():
    # (KDE 6 only loads a local service menu that's executable)
    write(dolphin_paths()[0], f'''[Desktop Entry]
Type=Service
X-KDE-ServiceTypes=KonqPopupMenu/Plugin
MimeType=all/all;
Actions=send;
X-KDE-Priority=TopLevel

[Desktop Action send]
Name={LABEL}
Icon=document-send
Exec={quoted(SCRIPT)} %U
''', executable=True)


def nemo_paths():
    return [data_home() / "nemo/actions/irice-share.nemo_action"]


def nemo_install():
    write(nemo_paths()[0], f'''[Nemo Action]
Name={LABEL}
Comment=Send with LocalSend or Quick Share
Exec={quoted(SCRIPT)} %F
Icon-Name=document-send
Selection=notnone
Extensions=any;
''')


def uca_path():
    return config_home() / "Thunar/uca.xml"


def uca_load():
    p = uca_path()
    if p.exists():
        try:
            return ET.parse(p)
        except ET.ParseError:
            raise SystemExit(f"Thunar's custom actions file ({p}) can't be read. Fix or move it, then try again.")
    return ET.ElementTree(ET.Element("actions"))


def uca_ours(tree):
    return [a for a in tree.getroot().findall("action") if (a.findtext("unique-id") or "") == UCA_ID]


def uca_save(tree):
    ET.indent(tree, space="\t")
    p = uca_path()
    p.parent.mkdir(parents=True, exist_ok=True)
    tmp = p.with_name(p.name + ".tmp")
    tree.write(tmp, encoding="UTF-8", xml_declaration=True)
    os.replace(tmp, p)


def thunar_install():
    tree = uca_load()
    for a in uca_ours(tree):
        tree.getroot().remove(a)
    a = ET.SubElement(tree.getroot(), "action")
    for tag, text in (("icon", "document-send"), ("name", LABEL), ("submenu", ""), ("unique-id", UCA_ID),
                      ("command", f"{quoted(SCRIPT)} %F"), ("description", "Send with LocalSend or Quick Share"),
                      ("range", "*"), ("patterns", "*")):
        ET.SubElement(a, tag).text = text
    for kind in ("directories", "audio-files", "image-files", "other-files", "text-files", "video-files"):
        ET.SubElement(a, kind)
    uca_save(tree)


def thunar_remove():
    if not uca_path().exists():
        return
    tree = uca_load()
    ours = uca_ours(tree)
    if ours:
        for a in ours:
            tree.getroot().remove(a)
        uca_save(tree)


MANAGERS = {
    "nautilus": {"name": "Files (Nautilus)", "bin": "nautilus", "paths": nautilus_paths, "install": nautilus_install},
    "dolphin": {"name": "Dolphin", "bin": "dolphin", "paths": dolphin_paths, "install": dolphin_install},
    "thunar": {"name": "Thunar", "bin": "thunar", "paths": lambda: [], "install": thunar_install, "remove": thunar_remove,
               "installed": lambda: uca_path().exists() and bool(uca_ours(uca_load()))},
    "nemo": {"name": "Nemo", "bin": "nemo", "paths": nemo_paths, "install": nemo_install},
}


def installed(key):
    m = MANAGERS[key]
    if "installed" in m:
        try:
            return m["installed"]()
        except SystemExit:
            return False
    return any(p.exists() for p in m["paths"]())


def remove(key):
    m = MANAGERS[key]
    if "remove" in m:
        m["remove"]()
    for p in m["paths"]():
        if p.exists():
            p.unlink()


def status():
    return [{"id": k, "name": m["name"], "found": shutil.which(m["bin"]) is not None, "installed": installed(k),
             "note": "" if k != "nautilus" or nautilus_python() else "As a Scripts entry (install nautilus-python for a top-level one)"}
            for k, m in MANAGERS.items()]


def main(argv):
    if not argv or argv[0] not in ("status", "install", "remove"):
        print(__doc__.strip(), file=sys.stderr)
        return 2
    cmd, names = argv[0], argv[1:]
    if cmd == "status":
        print(json.dumps(status()))
        return 0
    unknown = [n for n in names if n not in MANAGERS]
    if unknown:
        print(f"Unknown file manager: {', '.join(unknown)}. Choose from {', '.join(MANAGERS)}.", file=sys.stderr)
        return 2
    keys = names or [k for k, m in MANAGERS.items() if shutil.which(m["bin"]) or (cmd == "remove" and installed(k))]
    for k in keys:
        if cmd == "install":
            MANAGERS[k]["install"]()
        else:
            remove(k)
    print(json.dumps(status()))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
