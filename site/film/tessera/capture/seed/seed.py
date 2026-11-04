#!/usr/bin/env python3
"""Seeds a sandbox arena's home for filming: the look, a calm wallpaper of
the project's own, a placeholder account picture, the canned AI model, and a
few past chats. Only ever touches the arena home given.
  seed.py <arena home> <glass|material> [wallpaper file]
Every change is to the arena's config and state, which `./dev.sh up` seeds
again from .sandbox/home when the arena is recreated.
"""
import json
import os
import shutil
import sys
import time

home, look = sys.argv[1], sys.argv[2]
here = os.path.dirname(os.path.abspath(__file__))
repo = os.path.abspath(os.path.join(here, "../../../../.."))
wall = sys.argv[3] if len(sys.argv) > 3 else os.path.join(here, "photos/wall-sunset-lake.jpg")
assert os.path.isdir(os.path.join(home, ".config")) and "/.sandbox/" in os.path.abspath(home), "not an arena home: " + home

cfg_path = os.path.join(home, ".config/irice/config.json")
c = json.load(open(cfg_path))
a = c.setdefault("appearance", {})
a.update(look=look, mode="dark", materialSeed="wallpaper", accent=os.environ.get("ACCENT", "wallpaper"), menuIcon="circle")  # (circle: the Badge, the new logo)
c.setdefault("desktop", {})["icons"] = False
# (the laptop runs warm while recording: no "Running hot" alert in the takes)
c.setdefault("island", {}).update(hotTemp=200, hotThrottle=False)
w = c.setdefault("wallpaper", {})
w.update(path=wall, dynamic="off", live="")
model = {"id": "atlas-8b", "name": "Atlas 8B", "provider": "custom", "endpoint": "http://127.0.0.1:47311/v1",
         "project": "", "location": "", "auth": "none", "thinking": False, "maxTokens": 0, "vision": True}
model["key"] = "|".join(["custom", "atlas-8b", model["endpoint"], "none", "", ""]).replace(":", "%3A").replace("/", "%2F")
ai = c.setdefault("ai", {})
ai.update(models=[json.dumps(model)], model=model["key"], keepHistory=True, historyDays=0)
json.dump(c, open(cfg_path, "w"), indent=4)

# The account picture (a placeholder: "M" for Maya)
shutil.copy(os.path.join(here, "avatar.png"), os.path.join(home, ".face"))

# Past chats, for the history view
chats = os.path.join(home, ".local/state/irice/ai-chats")
shutil.rmtree(chats, ignore_errors=True)
os.makedirs(chats, mode=0o700)
now = int(time.time() * 1000)
day = 86400000
past = [
    ("hyprland-keys", "Hyprland keybinds cheat sheet", 9 * day, True, "A cheat sheet of the keys you use most."),
    ("pipewire-fix", "Fix: pipewire crackles on Bluetooth", 1 * day, False, "Try a larger quantum for Bluetooth outputs."),
    ("haiku", "Write a haiku about the monsoon", 3 * 3600000, False, "Grey roofs drink the rain."),
    ("goa", "Trip plan: a weekend in Goa", 20 * day, False, "Saturday: the north beaches; Sunday: Old Goa."),
    ("font", "Which font for code?", 30 * day, False, "Pick one with clear 0/O and 1/l."),
]
index = []
for cid, title, ago, pinned, reply in past:
    t = now - ago
    rows = [{"role": "user", "text": title, "thinking": "", "status": "done", "model": "", "image": "", "search": ""},
            {"role": "assistant", "text": reply, "thinking": "", "status": "done", "model": "Atlas 8B", "image": "", "search": ""}]
    rec = {"id": cid, "title": title, "created": t, "updated": t, "model": model["key"], "modelName": "Atlas 8B",
           "pinned": pinned, "rows": rows, "history": [{"role": "user", "text": title}, {"role": "assistant", "text": reply}]}
    json.dump(rec, open(os.path.join(chats, cid + ".json"), "w"))
    index.append({k: rec[k] for k in ("id", "title", "created", "updated", "model", "modelName", "pinned")} | {"count": 2, "preview": reply})
json.dump({"version": 1, "current": "", "trash": [], "chats": index}, open(os.path.join(chats, "index.json"), "w"))
print("seeded", home, look, os.path.basename(wall))
