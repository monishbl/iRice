#!/usr/bin/env python3
"""iCloud and Microsoft 365 for Tessera (services/CloudAccounts.qml): one
request per run, JSON on stdin, one JSON line out (the accounts' state, and
an "error" when something went wrong).

  iCloud: calendars over CalDAV (caldav.icloud.com) and contacts over CardDAV
    (contacts.icloud.com), Apple's own standard way in. Signing in takes the
    Apple ID and an app-specific password (made at account.apple.com, which
    needs two-factor authentication); the password goes to the keyring
    (secret-tool, the Secret Service), never to a file. Events go to the
    shell as their iCalendar text, which it reads and repeats itself (the
    same reader as subscribed calendars). Repeating events are shown, not
    changed: change those on the iPhone. Reminders lists (VTODO
    collections) sync as task lists: open reminders, with a due date or a
    due time; added, ticked, renamed, moved and deleted here. (Reminders
    upgraded to Apple's newer format, iOS 13 and later, are no longer
    shared over CalDAV, so those lists don't appear.)
  Microsoft 365: Outlook calendars and Microsoft To Do over Microsoft Graph,
    signed in through GNOME Online Accounts (its Microsoft 365 account:
    calendars and tasks read/write), so Tessera never holds a password or a
    refresh token. Graph repeats events itself (calendarView); occurrences
    of a series are shown, not changed.

Offline: a change that can't reach the server is kept in outbox.json and
shown as made (the cache changes here); "flush", and every sync, send them
in order. One the server turns away then is dropped and said ("dropped").

Requests: {"cmd": "status"} | {"cmd": "sync"} | {"cmd": "flush"} |
  {"cmd": "connectICloud", "appleId", "password", "server"?, "cardServer"?} |
  {"cmd": "connectMicrosoft", "goa": "<GOA account path>", "identity"} |
  {"cmd": "disconnect", "account"} |
  {"cmd": "eventInsert", "account", "calendarId", "fields"} |
  {"cmd": "eventPatch", "account", "calendarId", "id", "etag", "fields"} |
  {"cmd": "eventDelete", "account", "calendarId", "id", "etag"} |
  {"cmd": "taskInsert" | "taskPatch" | "taskDelete", "account", "listId",
   "id"?, "fields"?}
  fields: {title, start, end (ms), allDay, location, notes} for events;
  {text, done, date ("yyyy-mm-dd" or ""), dueAt (ms, iCloud's due time; 0
  none)} for tasks.

What's kept (0600 in a 0700 directory, $XDG_STATE_HOME/tessera/cloud):
accounts.json (who, never secrets), cache.json (the last sync) and outbox.json
(changes made offline).
No third-party dependencies.
"""
import base64
import hashlib
import json
import os
import re
import secrets as pysecrets
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
import xml.etree.ElementTree as ET
from datetime import datetime, timedelta, timezone


def app_dir(base):
    """Tessera's folder under base."""
    return os.path.join(base, "tessera")

def _old_names():
    """Tessera's old name (migrate-names.py: the one place that knows it)."""
    import importlib.util
    spec = importlib.util.spec_from_file_location("migrate_names", os.path.join(os.path.dirname(os.path.realpath(__file__)), "migrate-names.py"))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


OLD = _old_names()
ICLOUD = "https://caldav.icloud.com/"
ICLOUD_CARD = "https://contacts.icloud.com/"
GRAPH = "https://graph.microsoft.com/v1.0"
BACK, AHEAD = timedelta(days=90), timedelta(days=366)  # events kept around now
NS = {"d": "DAV:", "c": "urn:ietf:params:xml:ns:caldav", "card": "urn:ietf:params:xml:ns:carddav",
      "cs": "http://calendarserver.org/ns/", "ical": "http://apple.com/ns/ical/"}


class Failed(RuntimeError):
    """Something the person should read: the message says what to do."""


class Offline(Failed):
    pass


def state_dir():
    base = os.environ.get("XDG_STATE_HOME") or os.path.expanduser("~/.local/state")
    d = os.path.join(app_dir(base), "cloud")
    os.makedirs(d, mode=0o700, exist_ok=True)
    return d


def read(name, default):
    try:
        with open(os.path.join(state_dir(), name)) as f:
            return json.load(f)
    except (OSError, ValueError):
        return default


def write(name, value):
    path = os.path.join(state_dir(), name)
    tmp = path + ".tmp"
    fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(fd, "w") as f:
        json.dump(value, f)
    os.replace(tmp, path)


# ── Secrets: the keyring (secret-tool) ──────────────────────────────────
class Secrets:
    """The Secret Service through secret-tool. (TESSERA_CLOUD_SECRET_FILE, a
    JSON file instead, is for the tests only.)"""

    def __init__(self):
        self.file = os.environ.get("TESSERA_CLOUD_SECRET_FILE")

    def _load(self):
        try:
            with open(self.file) as f:
                return json.load(f)
        except (OSError, ValueError):
            return {}

    def store(self, account, secret):
        if self.file:
            d = self._load()
            d[account] = secret
            with open(self.file, "w") as f:
                json.dump(d, f)
            return
        r = subprocess.run(["secret-tool", "store", "--label", f"Tessera: iCloud ({account})", "service", "tessera-icloud",
                            "account", account], input=secret, text=True, capture_output=True, timeout=60)
        if r.returncode != 0:
            raise Failed("Couldn't save the password in your keyring. Check that a keyring (GNOME Keyring or KWallet) is running.")

    def lookup(self, account):
        if self.file:
            return self._load().get(account)
        # (or kept under the old name)
        for service in ("tessera-icloud", OLD.OLD_ICLOUD_SERVICE):
            r = subprocess.run(["secret-tool", "lookup", "service", service, "account", account],
                               capture_output=True, text=True, timeout=60)
            if r.returncode == 0 and r.stdout:
                return r.stdout
        return None

    def clear(self, account):
        if self.file:
            d = self._load()
            d.pop(account, None)
            with open(self.file, "w") as f:
                json.dump(d, f)
            return
        for service in ("tessera-icloud", OLD.OLD_ICLOUD_SERVICE):
            subprocess.run(["secret-tool", "clear", "service", service, "account", account],
                           capture_output=True, timeout=60)


# ── HTTP ────────────────────────────────────────────────────────────────
def _site(url):
    """The registered domain, near enough: p42-caldav.icloud.com -> icloud.com."""
    host = (urllib.parse.urlsplit(url).hostname or "").lower()
    return ".".join(host.split(".")[-2:])


class _NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *a, **k):
        return None


def http(method, url, body=None, headers=None, auth=None, token=None):
    """(status, headers, text). Redirects are followed for every method
    (PROPFIND and REPORT too, which urllib wouldn't), but the password or
    token only goes over https to the same service (iCloud sends you on to
    pNN-caldav.icloud.com)."""
    home, secure = _site(url), urllib.parse.urlsplit(url).scheme == "https"
    for _ in range(5):
        if _site(url) != home or (secure and urllib.parse.urlsplit(url).scheme != "https"):
            auth = token = None
        h = dict(headers or {})
        if auth:
            h["Authorization"] = "Basic " + base64.b64encode(f"{auth[0]}:{auth[1]}".encode()).decode()
        if token:
            h["Authorization"] = "Bearer " + token
        data = body.encode() if isinstance(body, str) else body
        req = urllib.request.Request(url, data=data, headers=h, method=method)
        opener = urllib.request.build_opener(_NoRedirect)
        try:
            with opener.open(req, timeout=30) as r:
                return r.status, dict(r.headers), r.read().decode("utf-8", "replace")
        except urllib.error.HTTPError as e:
            with e:
                if e.code in (301, 302, 307, 308) and e.headers.get("Location"):
                    url = urllib.parse.urljoin(url, e.headers["Location"])
                    continue
                return e.code, dict(e.headers or {}), (e.read() or b"").decode("utf-8", "replace")
        except (urllib.error.URLError, OSError) as e:
            raise Offline("Couldn't reach the server. Check your internet connection.") from e
    raise Failed("Too many redirects.")


# ── iCalendar ───────────────────────────────────────────────────────────
def ics_time(ms, all_day):
    if all_day:
        d = datetime.fromtimestamp(ms / 1000)  # (local: an all-day event is a local date)
        return ";VALUE=DATE:" + d.strftime("%Y%m%d")
    return ":" + datetime.fromtimestamp(ms / 1000, tz=timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def ics_escape(s):
    return str(s).replace("\\", "\\\\").replace(";", "\\;").replace(",", "\\,").replace("\n", "\\n")


def fold(line):
    """Lines over 75 octets go on as continuation lines (RFC 5545)."""
    out, b = [], line.encode()
    while len(b) > 75:
        cut = 75
        while cut > 0 and (b[cut] & 0xC0) == 0x80:  # (not inside a UTF-8 character)
            cut -= 1
        out.append(b[:cut].decode())
        b = b" " + b[cut:]
    out.append(b.decode())
    return "\r\n".join(out)


def new_ics(uid, fields):
    now = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    end = fields.get("end") or fields["start"]
    if fields.get("allDay") and end <= fields["start"]:
        end = fields["start"] + 86400000
    lines = ["BEGIN:VCALENDAR", "VERSION:2.0", "PRODID:-//Tessera//Calendar//EN", "BEGIN:VEVENT", "UID:" + uid,
             "DTSTAMP:" + now, "CREATED:" + now, "LAST-MODIFIED:" + now,
             "DTSTART" + ics_time(fields["start"], fields.get("allDay")), "DTEND" + ics_time(end, fields.get("allDay")),
             "SUMMARY:" + ics_escape(fields.get("title", ""))]
    if fields.get("location"):
        lines.append("LOCATION:" + ics_escape(fields["location"]))
    if fields.get("notes"):
        lines.append("DESCRIPTION:" + ics_escape(fields["notes"]))
    lines += ["END:VEVENT", "END:VCALENDAR"]
    return "\r\n".join(fold(l) for l in lines) + "\r\n"


def unfold(text):
    return re.sub(r"\r?\n[ \t]", "", text).replace("\r\n", "\n").split("\n")


def patch_ics(text, fields):
    """The event's iCalendar text with `fields` changed and everything else
    (alarms, attendees, the app's own properties) left as it was."""
    lines = unfold(text)
    out, in_event, done = [], False, False
    depth = 0  # (inside the event's own parts: an alarm has a DESCRIPTION too)
    keys = {"title": "SUMMARY", "location": "LOCATION", "notes": "DESCRIPTION"}
    now = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    replaced = set()
    for line in lines:
        if not line:
            continue
        name = re.split(r"[;:]", line, maxsplit=1)[0].upper()
        if line.upper() == "BEGIN:VEVENT" and not done:
            in_event = True
            out.append(line)
            continue
        if in_event and name == "BEGIN":
            depth += 1
        if in_event and depth > 0:
            if name == "END":
                depth -= 1
            out.append(line)
            continue
        if in_event and line.upper() == "END:VEVENT":
            # the fields it didn't have yet
            for k, prop in keys.items():
                if k in fields and prop not in replaced and fields[k]:
                    out.append(prop + ":" + ics_escape(fields[k]))
            if "start" in fields and "DTSTART" not in replaced:
                out.append("DTSTART" + ics_time(fields["start"], fields.get("allDay")))
                end = fields.get("end") or fields["start"]
                if fields.get("allDay") and end <= fields["start"]:
                    end = fields["start"] + 86400000
                out.append("DTEND" + ics_time(end, fields.get("allDay")))
            in_event, done = False, True
            out.append(line)
            continue
        if in_event:
            if name in ("SUMMARY", "LOCATION", "DESCRIPTION"):
                k = {v: k for k, v in keys.items()}[name]
                if k in fields:
                    replaced.add(name)
                    if fields[k]:
                        out.append(name + ":" + ics_escape(fields[k]))
                    continue
            if name in ("DTSTART", "DTEND", "DURATION") and "start" in fields:
                if name == "DTSTART":
                    replaced.add("DTSTART")
                    out.append("DTSTART" + ics_time(fields["start"], fields.get("allDay")))
                    end = fields.get("end") or fields["start"]
                    if fields.get("allDay") and end <= fields["start"]:
                        end = fields["start"] + 86400000
                    out.append("DTEND" + ics_time(end, fields.get("allDay")))
                continue  # (DTEND/DURATION: written with DTSTART)
            if name in ("DTSTAMP", "LAST-MODIFIED"):
                out.append(name + ":" + now)
                continue
            if name == "SEQUENCE":
                try:
                    out.append("SEQUENCE:" + str(int(line.split(":", 1)[1]) + 1))
                except ValueError:
                    out.append(line)
                continue
        out.append(line)
    return "\r\n".join(fold(l) for l in out) + "\r\n"


def ics_props(text):
    """UID and RRULE of the event (to know whether it repeats)."""
    lines = unfold(text)
    return {"uid": next((l.split(":", 1)[1] for l in lines if l.upper().startswith("UID")), ""),
            "repeats": any(l.upper().startswith(("RRULE", "RDATE")) for l in lines)}


def when_of(ms, all_day):
    """{date} (local) or {dateTime} (UTC), as the shell reads Microsoft's events."""
    if all_day:
        return {"date": datetime.fromtimestamp(ms / 1000).strftime("%Y-%m-%d")}
    return {"dateTime": datetime.fromtimestamp(ms / 1000, tz=timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")}


def ics_unescape(s):
    out, i = [], 0
    while i < len(s):
        c = s[i]
        if c == "\\" and i + 1 < len(s):
            n = s[i + 1]
            out.append("\n" if n in "nN" else n)
            i += 2
            continue
        out.append(c)
        i += 1
    return "".join(out)


def ics_due(value, params):
    """A reminder's DUE -> (date "yyyy-mm-dd", dueAt ms or 0 for a date).
    A time with a TZID or none is read as local time (as the calendar does)."""
    v = value.strip()
    try:
        if len(v) == 8 or "VALUE=DATE" in params.upper() and "DATE-TIME" not in params.upper():
            d = datetime.strptime(v[:8], "%Y%m%d")
            return d.strftime("%Y-%m-%d"), 0
        if v.endswith("Z"):
            d = datetime.strptime(v[:15], "%Y%m%dT%H%M%S").replace(tzinfo=timezone.utc)
        else:
            d = datetime.strptime(v[:15], "%Y%m%dT%H%M%S")
    except ValueError:
        return "", 0
    at = int(d.timestamp() * 1000)
    return datetime.fromtimestamp(at / 1000).strftime("%Y-%m-%d"), at


def _vtodo_lines(text):
    """(before, the VTODO's own lines, its nested parts by position, after)"""
    lines = [l for l in unfold(text) if l]
    try:
        start = next(i for i, l in enumerate(lines) if l.upper() == "BEGIN:VTODO")
        end = next(i for i in range(len(lines) - 1, start, -1) if lines[i].upper() == "END:VTODO")
    except StopIteration:
        raise Failed("That reminder isn't one Tessera can read. Change it on your iPhone or at icloud.com.") from None
    return lines[:start + 1], lines[start + 1:end], lines[end:]


def vtodo_props(text):
    """A reminder: {uid, text, done, date, dueAt}"""
    p = {"uid": "", "text": "", "done": False, "date": "", "dueAt": 0}
    try:
        _, body, _ = _vtodo_lines(text)
    except Failed:
        return p
    depth = 0
    for line in body:
        up = line.upper()
        if up.startswith("BEGIN:"):
            depth += 1
            continue
        if up.startswith("END:"):
            depth = max(0, depth - 1)
            continue
        if depth or ":" not in line:
            continue
        head, value = line.split(":", 1)
        name = head.split(";")[0].upper()
        if name == "UID":
            p["uid"] = value.strip()
        elif name == "SUMMARY":
            p["text"] = ics_unescape(value)
        elif name == "STATUS":
            p["done"] = p["done"] or value.strip().upper() == "COMPLETED"
        elif name == "COMPLETED":
            p["done"] = True
        elif name == "DUE":
            p["date"], p["dueAt"] = ics_due(value, head)
    return p


def _due_line(fields):
    if fields.get("dueAt"):
        return "DUE:" + datetime.fromtimestamp(fields["dueAt"] / 1000, tz=timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    if fields.get("date"):
        return "DUE;VALUE=DATE:" + fields["date"].replace("-", "")
    return None


def _status_lines(done, now):
    return ["STATUS:COMPLETED", "COMPLETED:" + now, "PERCENT-COMPLETE:100"] if done else ["STATUS:NEEDS-ACTION"]


def new_vtodo(uid, fields):
    now = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    lines = ["BEGIN:VCALENDAR", "VERSION:2.0", "PRODID:-//Tessera//Reminders//EN", "BEGIN:VTODO", "UID:" + uid,
             "DTSTAMP:" + now, "CREATED:" + now, "LAST-MODIFIED:" + now, "SUMMARY:" + ics_escape(fields.get("text", ""))]
    due = _due_line(fields)
    if due:
        lines.append(due)
    lines += _status_lines(bool(fields.get("done")), now)
    lines += ["END:VTODO", "END:VCALENDAR"]
    return "\r\n".join(fold(l) for l in lines) + "\r\n"


def patch_vtodo(text, fields):
    """The reminder's iCalendar text with `fields` ({text, done, date,
    dueAt}) changed and the rest (alarms, notes, the app's own properties)
    kept. A new due time moves an alarm set for the old one."""
    before, body, after = _vtodo_lines(text)
    now = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    moving = "date" in fields or "dueAt" in fields
    new_due = _due_line(fields) if moving else None
    out, depth = [], 0
    for line in body:
        up = line.upper()
        name = re.split(r"[;:]", line, maxsplit=1)[0].upper()
        if up.startswith("BEGIN:"):
            depth += 1
        elif up.startswith("END:"):
            depth = max(0, depth - 1)
        elif depth:
            # (an alarm at the old due time follows it)
            if name == "TRIGGER" and moving and "VALUE=DATE-TIME" in up and fields.get("dueAt"):
                line = "TRIGGER;VALUE=DATE-TIME:" + datetime.fromtimestamp(fields["dueAt"] / 1000, tz=timezone.utc).strftime("%Y%m%dT%H%M%SZ")
        else:
            if name == "SUMMARY" and "text" in fields:
                continue
            if name in ("STATUS", "COMPLETED", "PERCENT-COMPLETE") and "done" in fields:
                continue
            if name in ("DUE", "DTSTART") and moving:
                continue  # (a start after the new due date would be wrong)
            if name in ("DTSTAMP", "LAST-MODIFIED"):
                line = name + ":" + now
            elif name == "SEQUENCE":
                try:
                    line = "SEQUENCE:" + str(int(line.split(":", 1)[1]) + 1)
                except ValueError:
                    pass
        out.append(line)
    # (the changed ones before the nested parts)
    head = []
    if "text" in fields:
        head.append("SUMMARY:" + ics_escape(fields["text"]))
    if "done" in fields:
        head += _status_lines(bool(fields["done"]), now)
    if new_due:
        head.append(new_due)
    at = next((i for i, l in enumerate(out) if l.upper().startswith("BEGIN:")), len(out))
    out[at:at] = head
    return "\r\n".join(fold(l) for l in before + out + after) + "\r\n"


def vcards(text):
    """Contacts from vCards: [{name, emails, phones}]."""
    out = []
    for card in re.findall(r"BEGIN:VCARD(.*?)END:VCARD", "\n".join(unfold(text)), re.S | re.I):
        name, emails, phones, n = "", [], [], ""
        for line in card.split("\n"):
            if ":" not in line:
                continue
            key, value = line.split(":", 1)
            prop = key.split(";")[0].split(".")[-1].upper()
            value = value.replace("\\,", ",").replace("\\;", ";").replace("\\n", " ").strip()
            if prop == "FN":
                name = value
            elif prop == "N" and not n:
                parts = value.split(";")
                n = " ".join(p for p in (parts[1] if len(parts) > 1 else "", parts[0]) if p)
            elif prop == "EMAIL" and value:
                emails.append(value)
            elif prop == "TEL" and value:
                phones.append(value)
        name = name or n
        if name and (emails or phones):
            out.append({"name": name, "emails": emails, "phones": phones})
    return out


# ── CalDAV / CardDAV ────────────────────────────────────────────────────
def _xml(text):
    try:
        return ET.fromstring(text)
    except ET.ParseError:
        raise Failed("The server sent something unexpected. Try again later.") from None


def _check(status, what):
    if status == 401:
        raise Failed("Your Apple ID or app-specific password was refused. Make a new app-specific password at account.apple.com and sign in again.")
    if status >= 400:
        raise Failed(f"The server refused {what} ({status}).")


class Dav:
    def __init__(self, server, user, password):
        self.server, self.auth = server, (user, password)

    def propfind(self, url, body, depth="0"):
        st, _, text = http("PROPFIND", url, body, {"Depth": depth, "Content-Type": "application/xml; charset=utf-8"}, auth=self.auth)
        _check(st, "the request")
        return _xml(text)

    def href(self, base, node):
        return urllib.parse.urljoin(base, node.text.strip()) if node is not None and node.text else None

    def home(self, prop):
        """The home set (calendar-home-set or addressbook-home-set)."""
        root = self.propfind(self.server, '<d:propfind xmlns:d="DAV:"><d:prop><d:current-user-principal/></d:prop></d:propfind>')
        principal = self.href(self.server, root.find(".//d:current-user-principal/d:href", NS))
        if not principal:
            raise Failed("The server didn't say who you are. Check the Apple ID.")
        ns = "c" if prop == "calendar-home-set" else "card"
        uri = NS[ns]
        root = self.propfind(principal, f'<d:propfind xmlns:d="DAV:" xmlns:x="{uri}"><d:prop><x:{prop}/></d:prop></d:propfind>')
        home = self.href(principal, root.find(f".//{ns}:{prop}/d:href", NS))
        if not home:
            raise Failed("The server has no calendars for this account.")
        return home

    def calendars(self, home):
        body = ('<d:propfind xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav" xmlns:i="http://apple.com/ns/ical/">'
                '<d:prop><d:resourcetype/><d:displayname/><i:calendar-color/><c:supported-calendar-component-set/>'
                '<d:current-user-privilege-set/></d:prop></d:propfind>')
        root = self.propfind(home, body, depth="1")
        out = []
        for r in root.findall("d:response", NS):
            href = self.href(home, r.find("d:href", NS))
            prop = r.find(".//d:prop", NS)
            if prop is None or prop.find("d:resourcetype/c:calendar", NS) is None:
                continue
            comps = [c.get("name") for c in prop.findall("c:supported-calendar-component-set/c:comp", NS)]
            # (kind: "events", or "todos" for a Reminders list; other collections aren't shown)
            if not comps or "VEVENT" in comps:
                kind = "events"
            elif "VTODO" in comps:
                kind = "todos"
            else:
                continue
            privs = [p.tag.split("}")[1] for p in prop.findall("d:current-user-privilege-set/d:privilege/*", NS)]
            color = (prop.findtext("ical:calendar-color", "", NS) or "").strip()
            out.append({"id": href, "title": prop.findtext("d:displayname", "", NS) or ("Calendar" if kind == "events" else "Reminders"),
                        "color": color[:7] if color.startswith("#") else "", "kind": kind,
                        "writable": not privs or bool({"write", "write-content", "all"} & set(privs))})
        return out

    def todos(self, lst):
        """A Reminders list's reminders: [{href, etag, ics}]"""
        body = ('<c:calendar-query xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:prop><d:getetag/><c:calendar-data/></d:prop>'
                '<c:filter><c:comp-filter name="VCALENDAR"><c:comp-filter name="VTODO"/></c:comp-filter></c:filter></c:calendar-query>')
        st, _, text = http("REPORT", lst, body, {"Depth": "1", "Content-Type": "application/xml; charset=utf-8"}, auth=self.auth)
        _check(st, "the reminders")
        out = []
        for r in _xml(text).findall("d:response", NS):
            ics = r.findtext(".//c:calendar-data", "", NS)
            if ics:
                out.append({"href": self.href(lst, r.find("d:href", NS)), "etag": r.findtext(".//d:getetag", "", NS), "ics": ics})
        return out

    def events(self, cal, start, end):
        fmt = "%Y%m%dT%H%M%SZ"
        body = ('<c:calendar-query xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:prop><d:getetag/><c:calendar-data/></d:prop>'
                '<c:filter><c:comp-filter name="VCALENDAR"><c:comp-filter name="VEVENT">'
                f'<c:time-range start="{start.strftime(fmt)}" end="{end.strftime(fmt)}"/>'
                '</c:comp-filter></c:comp-filter></c:filter></c:calendar-query>')
        st, _, text = http("REPORT", cal, body, {"Depth": "1", "Content-Type": "application/xml; charset=utf-8"}, auth=self.auth)
        _check(st, "the calendar")
        out = []
        for r in _xml(text).findall("d:response", NS):
            ics = r.findtext(".//c:calendar-data", "", NS)
            if not ics:
                continue
            out.append({"href": self.href(cal, r.find("d:href", NS)), "etag": r.findtext(".//d:getetag", "", NS), "ics": ics})
        return out

    def contacts(self, home):
        root = self.propfind(home, '<d:propfind xmlns:d="DAV:"><d:prop><d:resourcetype/></d:prop></d:propfind>', depth="1")
        books = [self.href(home, r.find("d:href", NS)) for r in root.findall("d:response", NS)
                 if r.find(".//d:resourcetype/card:addressbook", NS) is not None]
        out = []
        for book in books:
            body = ('<card:addressbook-query xmlns:d="DAV:" xmlns:card="urn:ietf:params:xml:ns:carddav">'
                    '<d:prop><card:address-data/></d:prop></card:addressbook-query>')
            st, _, text = http("REPORT", book, body, {"Depth": "1", "Content-Type": "application/xml; charset=utf-8"}, auth=self.auth)
            if st >= 400:
                continue
            for r in _xml(text).findall("d:response", NS):
                out += vcards(r.findtext(".//card:address-data", "", NS))
        return out

    def put(self, url, ics, etag=None, create=False):
        h = {"Content-Type": "text/calendar; charset=utf-8"}
        if create:
            h["If-None-Match"] = "*"
        elif etag:
            h["If-Match"] = etag
        st, headers, _ = http("PUT", url, ics, h, auth=self.auth)
        if st == 412:
            raise Failed("This event changed on your other devices. It shows as it is there now.")
        _check(st, "the change")
        return headers.get("ETag") or headers.get("Etag") or ""

    def get(self, url):
        st, headers, text = http("GET", url, None, {}, auth=self.auth)
        _check(st, "the event")
        return text, headers.get("ETag") or headers.get("Etag") or ""

    def delete(self, url, etag=None):
        st, _, _ = http("DELETE", url, None, {"If-Match": etag} if etag else {}, auth=self.auth)
        if st == 412:
            raise Failed("This event changed on your other devices. It shows as it is there now.")
        if st != 404:
            _check(st, "the deletion")


# ── Microsoft Graph ─────────────────────────────────────────────────────
class OnlineAccounts:
    """GNOME Online Accounts' Microsoft 365 accounts and their tokens."""
    name = "org.gnome.OnlineAccounts"

    def call(self, path, interface, method):
        try:
            from gi.repository import Gio
            bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
            return bus.call_sync(self.name, path, interface, method, None, None, Gio.DBusCallFlags.NONE, 30000, None).unpack()
        except Exception:
            raise Failed("Online Accounts can't give Tessera access now. Sign in to Microsoft 365 again in Settings.") from None

    def access(self, path):
        self.call(path, self.name + ".Account", "EnsureCredentials")
        token, _ = self.call(path, self.name + ".OAuth2Based", "GetAccessToken")
        return token

    def accounts(self):
        try:
            objects = self.call("/org/gnome/OnlineAccounts", "org.freedesktop.DBus.ObjectManager", "GetManagedObjects")[0]
        except Failed:
            return []
        return [{"goa": p, "identity": i[self.name + ".Account"].get("PresentationIdentity", "")}
                for p, i in objects.items() if i.get(self.name + ".Account", {}).get("ProviderType") == "ms_graph"]


class Graph:
    def __init__(self, base, token):
        self.base, self.token = base.rstrip("/"), token

    def call(self, method, path, body=None, etag=None):
        url = path if path.startswith("http") else self.base + path
        h = {"Accept": "application/json", "Prefer": 'outlook.timezone="UTC"'}
        if body is not None:
            h["Content-Type"] = "application/json"
        if etag:
            h["If-Match"] = etag
        st, _, text = http(method, url, json.dumps(body) if body is not None else None, h, token=self.token)
        if st == 401:
            raise Failed("Microsoft asked to sign in again. Sign in to Microsoft 365 in Settings.")
        if st == 403:
            raise Failed("Your Microsoft account doesn't allow this (a work or school account may need its admin's approval).")
        if st == 412:
            raise Failed("This changed on your other devices. It shows as it is there now.")
        if st == 404 and method == "DELETE":
            return {}
        if st >= 400:
            raise Failed(f"Microsoft refused it ({st}).")
        return json.loads(text) if text.strip() else {}

    def pages(self, path):
        out, url = [], path
        while url:
            r = self.call("GET", url)
            out += r.get("value", [])
            url = r.get("@odata.nextLink")
        return out


def graph_time(ms, all_day):
    d = datetime.fromtimestamp(ms / 1000, tz=None if all_day else timezone.utc)
    return {"dateTime": d.strftime("%Y-%m-%dT00:00:00" if all_day else "%Y-%m-%dT%H:%M:%S"), "timeZone": "UTC"}


def graph_event_body(fields):
    b = {}
    if "title" in fields:
        b["subject"] = fields["title"]
    if "start" in fields:
        all_day = bool(fields.get("allDay"))
        end = fields.get("end") or fields["start"]
        if all_day and end <= fields["start"]:
            end = fields["start"] + 86400000
        b.update(isAllDay=all_day, start=graph_time(fields["start"], all_day), end=graph_time(end, all_day))
    if "location" in fields:
        b["location"] = {"displayName": fields["location"] or ""}
    if "notes" in fields:
        b["body"] = {"contentType": "text", "content": fields["notes"] or ""}
    return b


def graph_when(x, all_day):
    v = (x or {}).get("dateTime", "")
    return {"date": v[:10]} if all_day else {"dateTime": v[:19] + "Z"}


# ── The accounts ────────────────────────────────────────────────────────
class Hub:
    def __init__(self):
        self.accounts = read("accounts.json", [])
        self.cache = read("cache.json", {})
        self.outbox = read("outbox.json", [])
        self.dropped = []
        self.secrets = Secrets()
        self.passwords = {}

    def save(self):
        write("accounts.json", self.accounts)
        write("cache.json", self.cache)
        write("outbox.json", self.outbox)

    def account(self, aid):
        a = next((a for a in self.accounts if a["id"] == aid), None)
        if not a:
            raise Failed("That account isn't connected any more.")
        return a

    def dav(self, a, card=False):
        # (looked up once a run: calendars and contacts both need it)
        if a["identity"] not in self.passwords:
            self.passwords[a["identity"]] = self.secrets.lookup(a["identity"])
        password = self.passwords[a["identity"]]
        if not password:
            raise Failed("The app-specific password isn't in your keyring any more. Sign in to iCloud again.")
        return Dav(a.get("cardServer" if card else "server") or (ICLOUD_CARD if card else ICLOUD), a["identity"], password)

    def graph(self, a):
        token = a.get("testToken") or OnlineAccounts().access(a["goa"])
        return Graph(a.get("graph") or GRAPH, token)

    # Connecting
    def connect_icloud(self, req):
        apple_id = (req.get("appleId") or "").strip()
        password = (req.get("password") or "").strip()  # (xxxx-xxxx-xxxx-xxxx)
        if not apple_id or not password:
            raise Failed("Type your Apple ID and an app-specific password.")
        a = {"id": "icloud-" + hashlib.sha1(apple_id.lower().encode()).hexdigest()[:10], "provider": "icloud",
             "identity": apple_id, "server": req.get("server") or ICLOUD, "cardServer": req.get("cardServer") or ICLOUD_CARD}
        d = Dav(a["server"], apple_id, password)
        a["home"] = d.home("calendar-home-set")  # (checks the password first)
        self.secrets.store(apple_id, password)
        self.accounts = [x for x in self.accounts if x["id"] != a["id"]] + [a]
        self.sync(only=a["id"])

    def connect_microsoft(self, req):
        if not req.get("goa") and not req.get("testToken"):
            raise Failed("Sign in to Microsoft first.")
        ident = req.get("identity") or ""
        a = {"id": "ms-" + hashlib.sha1((req.get("goa") or ident).encode()).hexdigest()[:10], "provider": "microsoft",
             "identity": ident, "goa": req.get("goa", "")}
        for k in ("graph", "testToken"):
            if req.get(k):
                a[k] = req[k]
        self.graph(a).call("GET", "/me")  # (checks access)
        self.accounts = [x for x in self.accounts if x["id"] != a["id"]] + [a]
        self.sync(only=a["id"])

    def disconnect(self, aid):
        a = self.account(aid)
        if a["provider"] == "icloud":
            self.secrets.clear(a["identity"])
        self.accounts = [x for x in self.accounts if x["id"] != aid]
        for k in ("calendars", "events", "taskLists", "tasks", "contacts"):
            self.cache[k] = [x for x in self.cache.get(k, []) if x.get("account") != aid]
        self.outbox = [r for r in self.outbox if r.get("account") != aid]
        self.save()

    # Syncing: all of an account, or nothing of it (the last good copy stays)
    def sync(self, only=None, contacts=True):
        """Every account, or `only` that one (after a change to it: another
        account's trouble isn't this change's). contacts=False: the contacts
        are kept as they were (a change to an event doesn't touch them).
        Changes made offline go first; those still waiting stay shown."""
        if self.outbox and not only:
            self.flush(refresh=False)
        errors = []
        for a in self.accounts:
            if only and a["id"] != only:
                continue
            try:
                got = self.sync_icloud(a, contacts) if a["provider"] == "icloud" else self.sync_microsoft(a)
            except Failed as e:
                errors.append((a, e))
                continue
            for k in ("calendars", "events", "taskLists", "tasks", "contacts"):
                if k == "contacts" and k not in got:
                    continue
                self.cache[k] = [x for x in self.cache.get(k, []) if x.get("account") != a["id"]] + got.get(k, [])
            a["syncedAt"] = int(time.time() * 1000)
        for r in self.outbox:
            self.apply_local(r)
        self.cache["syncedAt"] = int(time.time() * 1000)
        self.save()
        if errors:
            a, e = errors[0]
            raise type(e)(str(e))

    def refreshed(self, a):
        """After a change the server took: a refresh that then fails (the
        network dropped, a 5xx) doesn't make the change a failure; the next
        sync picks it up."""
        try:
            self.sync(only=a["id"], contacts=False)
        except Failed:
            pass

    def sync_icloud(self, a, with_contacts=True):
        d = self.dav(a)
        home = a.get("home") or d.home("calendar-home-set")
        a["home"] = home
        now = datetime.now(timezone.utc)
        found = d.calendars(home)
        cals = [c for c in found if c["kind"] == "events"]
        events = []
        for c in cals:
            for e in d.events(c["id"], now - BACK, now + AHEAD):
                p = ics_props(e["ics"])
                events.append(dict(e, account=a["id"], calendarId=c["id"], uid=p["uid"],
                                   writable=c["writable"] and not p["repeats"]))
        # Reminders: the lists iCloud still shares over CalDAV (open ones)
        lists, tasks = [], []
        for l in (c for c in found if c["kind"] == "todos"):
            try:
                items = d.todos(l["id"])
            except Offline:
                raise
            except Failed:
                continue  # (a list it won't give: the calendars still sync)
            lists.append({"account": a["id"], "id": l["id"], "title": l["title"]})
            for it in items:
                p = vtodo_props(it["ics"])
                if p["uid"] and not p["done"]:
                    tasks.append({"account": a["id"], "listId": l["id"], "id": it["href"], "etag": it["etag"], "text": p["text"],
                                  "done": False, "date": p["date"], "dueAt": p["dueAt"], "writable": l["writable"]})
        strip = lambda c: {k: v for k, v in c.items() if k != "kind"}  # noqa: E731
        out = {"calendars": [dict(strip(c), account=a["id"]) for c in cals], "events": events, "taskLists": lists, "tasks": tasks}
        if not with_contacts:
            return out
        contacts = []
        try:
            cd = self.dav(a, card=True)
            a["cardHome"] = a.get("cardHome") or cd.home("addressbook-home-set")  # (kept: found once)
            contacts = cd.contacts(a["cardHome"])
        except Failed:
            pass  # (contacts are a bonus: calendars still sync)
        out["contacts"] = [dict(c, account=a["id"]) for c in contacts]
        return out

    def sync_microsoft(self, a):
        g = self.graph(a)
        now = datetime.now(timezone.utc)
        rng = f"startDateTime={(now - BACK).strftime('%Y-%m-%dT%H:%M:%SZ')}&endDateTime={(now + AHEAD).strftime('%Y-%m-%dT%H:%M:%SZ')}"
        cals, events = [], []
        for c in g.pages("/me/calendars?$top=100"):
            cals.append({"account": a["id"], "id": c["id"], "title": c.get("name", "Calendar"),
                         "color": c.get("hexColor") or "", "writable": bool(c.get("canEdit", True))})
            q = "&$select=id,subject,start,end,isAllDay,location,bodyPreview,type,seriesMasterId&$top=200"
            for e in g.pages(f"/me/calendars/{urllib.parse.quote(c['id'])}/calendarView?{rng}{q}"):
                all_day = bool(e.get("isAllDay"))
                events.append({"account": a["id"], "calendarId": c["id"], "id": e["id"], "etag": e.get("@odata.etag", ""),
                               "title": e.get("subject") or "(No title)", "start": graph_when(e.get("start"), all_day),
                               "end": graph_when(e.get("end"), all_day), "location": (e.get("location") or {}).get("displayName", ""),
                               "notes": e.get("bodyPreview", ""), "series": e.get("seriesMasterId") or "",
                               "writable": bool(c.get("canEdit", True)) and e.get("type", "singleInstance") == "singleInstance"})
        lists, tasks = [], []
        for l in g.pages("/me/todo/lists"):
            lists.append({"account": a["id"], "id": l["id"], "title": l.get("displayName", "Tasks")})
            for t in g.pages(f"/me/todo/lists/{urllib.parse.quote(l['id'])}/tasks?$filter=" + urllib.parse.quote("status ne 'completed'") + "&$top=100"):
                due = ((t.get("dueDateTime") or {}).get("dateTime") or "")[:10]
                tasks.append({"account": a["id"], "listId": l["id"], "id": t["id"], "etag": t.get("@odata.etag", ""),
                              "text": t.get("title", ""), "done": t.get("status") == "completed", "date": due})
        return {"calendars": cals, "events": events, "taskLists": lists, "tasks": tasks}

    # Changes
    def event_insert(self, req, refresh=True):
        """Returns the new event's id (Microsoft's; iCloud's is its link)."""
        a, f = self.account(req["account"]), req.get("fields", {})
        if a["provider"] == "icloud":
            uid = req.get("uid") or str(uuid.uuid4()).upper()
            url = req.get("href") or req["calendarId"].rstrip("/") + "/" + uid + ".ics"
            self.dav(a).put(url, new_ics(uid, f), create=True)
            new = url
        else:
            new = self.graph(a).call("POST", f"/me/calendars/{urllib.parse.quote(req['calendarId'])}/events", graph_event_body(f)).get("id", "")
        if refresh:
            self.refreshed(a)
        return new

    def event_patch(self, req, refresh=True):
        a, f = self.account(req["account"]), req.get("fields", {})
        if a["provider"] == "icloud":
            d = self.dav(a)
            text, etag = d.get(req["id"])
            if req.get("etag") and etag and etag != req["etag"]:
                self.refreshed(a)
                raise Failed("This event changed on your other devices. It shows as it is there now.")
            d.put(req["id"], patch_ics(text, f), etag=etag or req.get("etag"))
        else:
            self.graph(a).call("PATCH", f"/me/events/{urllib.parse.quote(req['id'])}", graph_event_body(f), etag=req.get("etag"))
        if refresh:
            self.refreshed(a)

    def event_delete(self, req, refresh=True):
        a = self.account(req["account"])
        if a["provider"] == "icloud":
            self.dav(a).delete(req["id"], req.get("etag"))
        else:
            self.graph(a).call("DELETE", f"/me/events/{urllib.parse.quote(req['id'])}")
        if refresh:
            self.refreshed(a)

    def task(self, req, refresh=True):
        """Microsoft To Do over Graph; iCloud Reminders over CalDAV (a
        reminder is a VTODO in its list). Returns a new task's id."""
        a = self.account(req["account"])
        f = req.get("fields", {})
        new = ""
        if a["provider"] == "icloud":
            d = self.dav(a)
            if req["cmd"] == "taskInsert":
                uid = req.get("uid") or str(uuid.uuid4()).upper()
                new = req.get("href") or req["listId"].rstrip("/") + "/" + uid + ".ics"
                d.put(new, new_vtodo(uid, f), create=True)
            elif req["cmd"] == "taskPatch":
                text, etag = d.get(req["id"])
                if req.get("etag") and etag and etag != req["etag"]:
                    self.refreshed(a)
                    raise Failed("This reminder changed on your other devices. It shows as it is there now.")
                d.put(req["id"], patch_vtodo(text, f), etag=etag or req.get("etag"))
            else:
                d.delete(req["id"], req.get("etag"))
            if refresh:
                self.refreshed(a)
            return new
        g = self.graph(a)
        base = f"/me/todo/lists/{urllib.parse.quote(req['listId'])}/tasks"
        body = {}
        if "text" in f:
            body["title"] = f["text"]
        if "done" in f:
            body["status"] = "completed" if f["done"] else "notStarted"
        if "date" in f:
            body["dueDateTime"] = {"dateTime": f["date"] + "T00:00:00", "timeZone": "UTC"} if f["date"] else None
        if req["cmd"] == "taskInsert":
            new = g.call("POST", base, body).get("id", "")
        elif req["cmd"] == "taskPatch":
            g.call("PATCH", base + "/" + urllib.parse.quote(req["id"]), body, etag=req.get("etag"))
        else:
            g.call("DELETE", base + "/" + urllib.parse.quote(req["id"]))
        if refresh:
            self.refreshed(a)
        return new

    def change(self, req, refresh=True):
        cmd = req["cmd"]
        if cmd == "eventInsert":
            return self.event_insert(req, refresh)
        if cmd == "eventPatch":
            return self.event_patch(req, refresh)
        if cmd == "eventDelete":
            return self.event_delete(req, refresh)
        return self.task(req, refresh)

    # ── Offline: the outbox ────────────────────────────────────────────
    # A change that couldn't reach the server is kept (outbox.json, 0600),
    # shown as made (the cache changed here), and sent first at the next
    # sync or "flush". A new item gets its id now: iCloud's link is chosen
    # here anyway; Microsoft's is "pending-…" until the server gives one.
    def keep(self, req):
        req = dict(req)
        a = self.account(req["account"])
        if req["cmd"] in ("eventInsert", "taskInsert"):
            uid = str(uuid.uuid4()).upper()
            if a["provider"] == "icloud":
                req["uid"] = uid
                req["href"] = req["calendarId" if req["cmd"] == "eventInsert" else "listId"].rstrip("/") + "/" + uid + ".ics"
            else:
                req["pending"] = "pending-" + uid
        elif req["cmd"] in ("eventDelete", "taskDelete"):
            # (deleting what never left this computer: neither goes)
            made = next((r for r in self.outbox if r.get("href", r.get("pending")) == req.get("id")), None)
            if made:
                self.outbox = [r for r in self.outbox if r is not made and r.get("id") != req.get("id")]
                self.apply_local(req)
                self.save()
                return
        self.outbox.append(req)
        self.apply_local(req)
        self.save()

    def apply_local(self, r):
        """Shows a change in the cache as if the server had it."""
        cmd, f = r["cmd"], r.get("fields", {})
        if cmd.startswith("event"):
            events = self.cache.setdefault("events", [])
            if cmd == "eventInsert":
                new = r.get("href") or r.get("pending")
                if any(e.get("href", e.get("id")) == new for e in events):
                    return
                if r.get("href"):
                    events.append({"account": r["account"], "calendarId": r["calendarId"], "href": new, "etag": "",
                                   "ics": new_ics(r["uid"], f), "uid": r["uid"], "writable": True})
                else:
                    events.append({"account": r["account"], "calendarId": r["calendarId"], "id": new, "etag": "", "title": f.get("title", ""),
                                   "start": when_of(f["start"], f.get("allDay")), "end": when_of(f.get("end") or f["start"], f.get("allDay")),
                                   "location": f.get("location", ""), "notes": f.get("notes", ""), "series": "", "writable": True})
                return
            for e in events:
                if e.get("account") == r["account"] and e.get("href", e.get("id")) == r.get("id"):
                    if cmd == "eventDelete":
                        events.remove(e)
                    elif "ics" in e:
                        e["ics"] = patch_ics(e["ics"], f)
                    else:
                        for k in ("title", "location", "notes"):
                            if k in f:
                                e[k] = f[k]
                        if "start" in f:
                            e["start"] = when_of(f["start"], f.get("allDay"))
                            e["end"] = when_of(f.get("end") or f["start"], f.get("allDay"))
                    return
            return
        tasks = self.cache.setdefault("tasks", [])
        if cmd == "taskInsert":
            new = r.get("href") or r.get("pending")
            if not any(t.get("id") == new for t in tasks):
                tasks.append({"account": r["account"], "listId": r["listId"], "id": new, "etag": "", "text": f.get("text", ""),
                              "done": bool(f.get("done")), "date": f.get("date", ""), "dueAt": f.get("dueAt", 0) or 0})
            return
        for t in tasks:
            if t.get("account") == r["account"] and t.get("listId") == r["listId"] and t.get("id") == r.get("id"):
                if cmd == "taskDelete":
                    tasks.remove(t)
                else:
                    for k in ("text", "done", "date", "dueAt"):
                        if k in f:
                            t[k] = f[k]
                return

    def flush(self, refresh=True):
        """Sends the outbox in order until the network goes again. A change
        the server turns away is dropped and said ("dropped"). True when
        the network went again with changes still waiting."""
        ids, touched, offline = {}, set(), False
        while self.outbox:
            r = dict(self.outbox[0])
            pid = str(r.get("id", ""))
            if pid.startswith("pending-"):
                if pid not in ids:
                    self.outbox.pop(0)  # (its new item never got made)
                    continue
                r["id"] = ids[pid]
            try:
                new = self.change(r, refresh=False)
                if r.get("pending") and new:
                    ids[r["pending"]] = new
                touched.add(r["account"])
            except Offline:
                offline = True
                break
            except Failed as e:
                what = "A change to " + ("an event" if r["cmd"].startswith("event") else "a task")
                self.dropped.append(f"{what} made while you were offline wasn't saved: {e}")
                touched.add(r.get("account"))
            except KeyError:
                pass  # (its account went away)
            self.outbox.pop(0)
            self.save()
        if refresh:
            for aid in touched:
                a = next((x for x in self.accounts if x["id"] == aid), None)
                if a:
                    self.refreshed(a)
        self.save()
        return offline

    def state(self):
        c = self.cache
        out = {"accounts": [{k: a.get(k, "") for k in ("id", "provider", "identity", "syncedAt")} for a in self.accounts],
               "calendars": c.get("calendars", []), "events": c.get("events", []), "taskLists": c.get("taskLists", []),
               "tasks": c.get("tasks", []), "contacts": c.get("contacts", []), "syncedAt": c.get("syncedAt", 0),
               "outbox": len(self.outbox)}
        if self.dropped:
            out["dropped"] = self.dropped
        return out


CHANGES = ("eventInsert", "eventPatch", "eventDelete", "taskInsert", "taskPatch", "taskDelete")


def main():
    try:
        req = json.loads(sys.stdin.readline() or "{}")
    except ValueError:
        req = {}
    hub = Hub()
    cmd = req.get("cmd", "status")
    out = {}
    try:
        if cmd == "sync":
            hub.sync()
        elif cmd == "flush":
            if hub.flush():
                raise Offline("You're offline. Changes made here go to your accounts when you're back online.")
        elif cmd == "connectICloud":
            hub.connect_icloud(req)
        elif cmd == "connectMicrosoft":
            hub.connect_microsoft(req)
        elif cmd == "microsoftAccounts":
            out["goaAccounts"] = OnlineAccounts().accounts()
        elif cmd == "disconnect":
            hub.disconnect(req["account"])
        elif cmd in CHANGES and any(r.get("href", r.get("pending")) == req.get("id") for r in hub.outbox):
            # (a change to something made offline goes after it)
            hub.keep(req)
            if hub.flush():
                raise Offline("You're offline. The change is saved on this computer and goes to your account when you're back online.")
        elif cmd in CHANGES:
            try:
                hub.change(req)
            except Offline:
                # (kept here and sent later: the shell shows it as made)
                where = "iCloud" if hub.account(req["account"])["provider"] == "icloud" else "Microsoft 365"
                hub.keep(req)
                raise Offline(f"You're offline. The change is saved on this computer and goes to {where} when you're back online.") from None
    except Offline as e:
        out.update(error=str(e), offline=True)
    except Failed as e:
        out["error"] = str(e)
    except (KeyError, TypeError) as e:
        out["error"] = f"Tessera sent an incomplete request ({e})."
    except Exception as e:  # noqa: BLE001 (the shell always gets an answer)
        out["error"] = f"Something went wrong: {e}"
    out.update(hub.state(), cmd=cmd)
    print(json.dumps(out))
    return 0


if __name__ == "__main__":
    sys.exit(main())
