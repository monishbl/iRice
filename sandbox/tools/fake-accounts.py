#!/usr/bin/env python3
"""Sandbox only: stand-in accounts for the shell's Google and cloud helpers,
so a sandbox shows Google, iCloud and Microsoft 365 calendars and tasks and
takes changes to them without any real account (no network, no tokens).

The Rust shell runs this instead of google-account.py / cloud-accounts.py
when TESSERA_SANDBOX=1 and TESSERA_FAKE_ACCOUNTS=<folder> (host/system
accounts/common.rs fake_accounts):

    fake-accounts.py google|cloud <folder>     (one request on stdin)

It answers as the real helper would (Google: one JSON line with "data";
cloud: the state object). The folder keeps:
  google.json, cloud.json  the accounts' items (made on first use: events
                           around today, so Today and the calendar show them)
  offline                  while this file exists, changes wait in the
                           outbox ("offline") and go at the next flush/sync
  reject                   while this file exists, a change is turned away
                           as if it changed on another device
  requests.log             every request, one JSON line each (what the
                           shell sent back: the verification reads it)
"""
import importlib.util
import json
import os
import sys
import time
import uuid
from datetime import datetime, timedelta, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("cloud_accounts", ROOT / "shell/assets/scripts/cloud-accounts.py")
ca = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ca)


def at(days, hour, minute=0):
    """ms of today + days at hour:minute, local time"""
    d = datetime.now().replace(hour=hour, minute=minute, second=0, microsecond=0) + timedelta(days=days)
    return int(d.timestamp() * 1000)


def iso(ms):
    return datetime.fromtimestamp(ms / 1000, tz=timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def day(days):
    return (datetime.now() + timedelta(days=days)).strftime("%Y-%m-%d")


def ics_stamp(ms):
    return datetime.fromtimestamp(ms / 1000, tz=timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def seed_google():
    ev = lambda i, cal, title, s, e, **k: dict(dict(account="g1", calendarId=cal, id=i, title=title, start={"dateTime": iso(s)},  # noqa: E731
                                                     end={"dateTime": iso(e)}, location="", notes="", color="#039be5", calendar="Jane",
                                                     etag='"1"', series="", recurring=False, writable=True), **k)
    soon = (int(time.time()) // 60 + 8) * 60_000  # (its alert, 10 minutes before, is due as the shell starts)
    events = [ev("review", "primary", "Design review", at(0, 15), at(0, 16), location="Room 4", notes="Bring the mock-ups"),
              ev("sync", "primary", "Quick sync", soon, soon + 15 * 60_000, location="Huddle room")]
    for n in range(-1, 6):
        events.append(ev(f"standup_{n}", "primary", "Standup", at(n, 9, 30), at(n, 9, 45), series="standup", recurring=True))
    events.append(dict(account="g1", calendarId="holidays", id="holiday", title="Founders' Day", start={"date": day(1)}, end={"date": day(2)},
                       location="", notes="", color="#0b8043", calendar="Holidays", etag='"1"', series="", recurring=False, writable=False))
    return {"connected": True, "configured": True, "accounts": [{"id": "g1", "identity": "jane.fixture@gmail.com"}],
            "calendars": [{"account": "g1", "id": "primary", "title": "Jane", "color": "#039be5", "writable": True, "primary": True},
                          {"account": "g1", "id": "holidays", "title": "Holidays", "color": "#0b8043", "writable": False}],
            "taskLists": [{"account": "g1", "id": "L1", "title": "My Tasks"}],
            "events": events,
            "tasks": [{"account": "g1", "listId": "L1", "listName": "My Tasks", "id": "rent", "text": "Pay rent", "done": False, "date": day(0), "notes": "", "etag": '"1"'}],
            "outbox": 0, "syncedAt": int(time.time() * 1000)}


def seed_cloud():
    gym = ("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:gym\r\nDTSTAMP:20260101T000000Z\r\n"
           f"DTSTART:{ics_stamp(at(-3, 18))}\r\nDTEND:{ics_stamp(at(-3, 19))}\r\nRRULE:FREQ=DAILY\r\nSUMMARY:Gym\r\nLOCATION:Fitness First\r\n"
           "BEGIN:VALARM\r\nTRIGGER:-PT30M\r\nACTION:DISPLAY\r\nEND:VALARM\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n")
    dinner = ca.new_ics("dinner", {"title": "Dinner with Alex", "start": at(1, 19, 30), "end": at(1, 21), "location": "Olive Bistro"})
    home = "https://caldav.fixture/home/cal/"
    rem = "https://caldav.fixture/home/rem/"
    return {"accounts": [{"id": "icloud-fix", "provider": "icloud", "identity": "jane.fixture@icloud.com", "syncedAt": 0},
                         {"id": "ms-fix", "provider": "microsoft", "identity": "sam.fixture@outlook.com", "syncedAt": 0}],
            "calendars": [{"account": "icloud-fix", "id": home, "title": "Home", "color": "#ff2968", "writable": True},
                          {"account": "ms-fix", "id": "cal1", "title": "Calendar", "color": "#0078d4", "writable": True}],
            "events": [{"account": "icloud-fix", "calendarId": home, "href": home + "gym.ics", "etag": '"1"', "ics": gym, "uid": "gym", "writable": False},
                       {"account": "icloud-fix", "calendarId": home, "href": home + "dinner.ics", "etag": '"1"', "ics": dinner, "uid": "dinner", "writable": True},
                       {"account": "ms-fix", "calendarId": "cal1", "id": "one-on-one", "etag": 'W/"1"', "title": "1:1 with Sam",
                        "start": {"dateTime": iso(at(0, 11))}, "end": {"dateTime": iso(at(0, 11, 30))}, "location": "Teams", "notes": "Quarterly goals",
                        "series": "", "writable": True}],
            "taskLists": [{"account": "ms-fix", "id": "l1", "title": "Tasks"}, {"account": "icloud-fix", "id": rem, "title": "Errands"}],
            "tasks": [{"account": "ms-fix", "listId": "l1", "id": "milk", "etag": 'W/"1"', "text": "Buy milk", "done": False, "date": ""},
                      {"account": "icloud-fix", "listId": rem, "id": rem + "mum.ics", "etag": '"1"', "text": "Call mum", "done": False,
                       "date": day(0), "dueAt": at(0, 17)}],
            "contacts": [], "outbox": 0, "syncedAt": int(time.time() * 1000)}


class Store:
    def __init__(self, folder, which):
        self.dir = Path(folder)
        self.dir.mkdir(parents=True, exist_ok=True)
        self.path = self.dir / f"{which}.json"
        self.which = which
        try:
            self.data = json.loads(self.path.read_text())
        except (OSError, ValueError):
            self.data = seed_google() if which == "google" else seed_cloud()
            self.data["queue"] = []
            self.save()

    def save(self):
        self.data["outbox"] = len(self.data.get("queue", []))
        self.path.write_text(json.dumps(self.data, indent=1))

    def offline(self):
        return (self.dir / "offline").exists()

    def reject(self):
        return (self.dir / "reject").exists()

    def log(self, req):
        with open(self.dir / "requests.log", "a") as f:
            f.write(json.dumps(dict(req, helper=self.which)) + "\n")


# ── Google (google-account.py's protocol) ───────────────────────────────
def google_apply(d, r):
    cmd = r["cmd"]
    if cmd == "taskAdd":
        p = r.get("patch", {})
        d["tasks"].append({"account": r["account"], "listId": r["listId"], "listName": "My Tasks", "id": "t" + uuid.uuid4().hex[:8],
                           "text": p.get("text", ""), "done": bool(p.get("done")), "date": p.get("date", ""), "notes": "", "etag": '"1"'})
    elif cmd in ("taskUpdate", "taskDelete"):
        for t in list(d["tasks"]):
            if t["id"] == r["id"] and t["listId"] == r["listId"]:
                if cmd == "taskDelete":
                    d["tasks"].remove(t)
                else:
                    for k in ("text", "done", "date"):
                        if k in r.get("patch", {}):
                            t[k] = r["patch"][k]
                    t["etag"] = f'"{int(t["etag"].strip(chr(34)) or 0) + 1}"'
    elif cmd == "eventInsert":
        f = r.get("fields", {})
        when = (lambda ms: {"date": datetime.fromtimestamp(ms / 1000).strftime("%Y-%m-%d")}) if f.get("allDay") else (lambda ms: {"dateTime": iso(ms)})
        d["events"].append({"account": r["account"], "calendarId": r["calendarId"], "id": "e" + uuid.uuid4().hex[:8], "title": f.get("title", ""),
                            "start": when(f["start"]), "end": when(f.get("end") or f["start"]), "location": f.get("location", ""),
                            "notes": f.get("notes", ""), "color": "#039be5", "calendar": "Jane", "etag": '"1"', "series": "", "recurring": False, "writable": True})
    elif cmd in ("eventPatch", "eventDelete"):
        series = r.get("series")
        for e in list(d["events"]):
            if (series and e.get("series") == series) or (e["id"] == r["id"] and e["calendarId"] == r["calendarId"]):
                if cmd == "eventDelete":
                    d["events"].remove(e)
                    continue
                f = r.get("fields", {})
                for k in ("title", "location", "notes"):
                    if k in f:
                        e[k] = f[k]
                if "start" in f and not series:
                    all_day = f.get("allDay")
                    w = (lambda ms: {"date": datetime.fromtimestamp(ms / 1000).strftime("%Y-%m-%d")}) if all_day else (lambda ms: {"dateTime": iso(ms)})
                    e["start"], e["end"] = w(f["start"]), w(f.get("end") or f["start"])


def google(store, req):
    d, cmd = store.data, req.get("cmd", "status")
    out = {}
    if cmd in ("taskAdd", "taskUpdate", "taskDelete", "eventInsert", "eventPatch", "eventDelete"):
        if store.reject():
            out["error"] = "This changed in Google meanwhile. Google's version is kept."
        else:
            google_apply(d, req)  # (shown at once: the real helper's cache does so too)
            if store.offline():
                d["queue"].append(req)
                out.update(error="You're offline. The change is kept and sent when you're back online.", offline=True)
            out["mutationComplete"] = True
    elif cmd in ("flush", "sync"):
        if store.offline():
            out.update(error="Couldn't reach Google. Check your internet connection.", offline=True)
        else:
            d["queue"] = []
            d["syncedAt"] = int(time.time() * 1000)
    store.save()
    data = {k: v for k, v in d.items() if k != "queue"}
    print(json.dumps(dict(out, data=data)))


# ── iCloud and Microsoft 365 (cloud-accounts.py's protocol) ─────────────
def cloud_apply(d, r):
    cmd, f = r["cmd"], r.get("fields", {})
    if cmd == "eventInsert":
        if r["account"] == "icloud-fix":
            uid = uuid.uuid4().hex
            d["events"].append({"account": r["account"], "calendarId": r["calendarId"], "href": r["calendarId"] + uid + ".ics", "etag": '"1"',
                                "ics": ca.new_ics(uid, f), "uid": uid, "writable": True})
        else:
            d["events"].append({"account": r["account"], "calendarId": r["calendarId"], "id": uuid.uuid4().hex[:8], "etag": 'W/"1"', "title": f.get("title", ""),
                                "start": ca.when_of(f["start"], f.get("allDay")), "end": ca.when_of(f.get("end") or f["start"], f.get("allDay")),
                                "location": f.get("location", ""), "notes": f.get("notes", ""), "series": "", "writable": True})
    elif cmd in ("eventPatch", "eventDelete"):
        for e in list(d["events"]):
            if e.get("href", e.get("id")) == r["id"]:
                if cmd == "eventDelete":
                    d["events"].remove(e)
                elif "ics" in e:
                    e["ics"] = ca.patch_ics(e["ics"], f)
                else:
                    for k in ("title", "location", "notes"):
                        if k in f:
                            e[k] = f[k]
                    if "start" in f:
                        e["start"] = ca.when_of(f["start"], f.get("allDay"))
                        e["end"] = ca.when_of(f.get("end") or f["start"], f.get("allDay"))
    elif cmd == "taskInsert":
        tid = uuid.uuid4().hex[:8]
        d["tasks"].append({"account": r["account"], "listId": r["listId"], "id": (r["listId"] + tid + ".ics") if r["account"] == "icloud-fix" else tid,
                           "etag": '"1"', "text": f.get("text", ""), "done": bool(f.get("done")), "date": f.get("date", ""), "dueAt": f.get("dueAt", 0) or 0})
    elif cmd in ("taskPatch", "taskDelete"):
        for t in list(d["tasks"]):
            if t["id"] == r["id"]:
                if cmd == "taskDelete" or f.get("done"):
                    d["tasks"].remove(t)  # (done: the real helpers show open ones)
                else:
                    for k in ("text", "date", "dueAt"):
                        if k in f:
                            t[k] = f[k]


def cloud(store, req):
    d, cmd = store.data, req.get("cmd", "status")
    out = {}
    if cmd in ("eventInsert", "eventPatch", "eventDelete", "taskInsert", "taskPatch", "taskDelete"):
        if store.reject():
            out["error"] = "This changed on your other devices. It shows as it is there now."
        else:
            cloud_apply(d, req)
            if store.offline():
                d["queue"].append(req)
                where = "iCloud" if req.get("account") == "icloud-fix" else "Microsoft 365"
                out.update(error=f"You're offline. The change is saved on this computer and goes to {where} when you're back online.", offline=True)
    elif cmd in ("flush", "sync"):
        if store.offline():
            out.update(error="Couldn't reach the server. Check your internet connection.", offline=True)
        else:
            d["queue"] = []
            d["syncedAt"] = int(time.time() * 1000)
            for a in d["accounts"]:
                a["syncedAt"] = d["syncedAt"]
    elif cmd in ("connectICloud", "connectMicrosoft", "disconnect"):
        out["error"] = "This sandbox has stand-in accounts only."
    store.save()
    print(json.dumps(dict({k: v for k, v in d.items() if k != "queue"}, **out, cmd=cmd)))


def main():
    if len(sys.argv) != 3 or sys.argv[1] not in ("google", "cloud"):
        print(json.dumps({"error": "usage: fake-accounts.py google|cloud <folder>"}))
        return 2
    if os.environ.get("TESSERA_SANDBOX") != "1":
        print(json.dumps({"error": "fake-accounts.py runs in a sandbox only (TESSERA_SANDBOX=1)."}))
        return 2
    try:
        req = json.loads(sys.stdin.readline() or "{}")
    except ValueError:
        req = {}
    store = Store(sys.argv[2], sys.argv[1])
    store.log({k: v for k, v in req.items() if k != "password"})
    (google if sys.argv[1] == "google" else cloud)(store, req)
    return 0


if __name__ == "__main__":
    sys.exit(main())
