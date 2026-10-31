#!/usr/bin/env python3
"""iCloud and Microsoft 365 for iRice (services/CloudAccounts.qml): one
request per run, JSON on stdin, one JSON line out (the accounts' state, and
an "error" when something went wrong).

  iCloud: calendars over CalDAV (caldav.icloud.com) and contacts over CardDAV
    (contacts.icloud.com), Apple's own standard way in. Signing in takes the
    Apple ID and an app-specific password (made at account.apple.com, which
    needs two-factor authentication); the password goes to the keyring
    (secret-tool, the Secret Service), never to a file. Events go to the
    shell as their iCalendar text, which it reads and repeats itself (the
    same reader as subscribed calendars). Repeating events are shown, not
    changed: change those on the iPhone.
  Microsoft 365: Outlook calendars and Microsoft To Do over Microsoft Graph,
    signed in through GNOME Online Accounts (its Microsoft 365 account:
    calendars and tasks read/write), so iRice never holds a password or a
    refresh token. Graph repeats events itself (calendarView); occurrences
    of a series are shown, not changed.

Requests: {"cmd": "status"} | {"cmd": "sync"} |
  {"cmd": "connectICloud", "appleId", "password", "server"?, "cardServer"?} |
  {"cmd": "connectMicrosoft", "goa": "<GOA account path>", "identity"} |
  {"cmd": "disconnect", "account"} |
  {"cmd": "eventInsert", "account", "calendarId", "fields"} |
  {"cmd": "eventPatch", "account", "calendarId", "id", "etag", "fields"} |
  {"cmd": "eventDelete", "account", "calendarId", "id", "etag"} |
  {"cmd": "taskInsert" | "taskPatch" | "taskDelete", "account", "listId",
   "id"?, "fields"?}
  fields: {title, start, end (ms), allDay, location, notes} for events;
  {text, done, date ("yyyy-mm-dd" or "")} for tasks.

What's kept (0600 in a 0700 directory, $XDG_STATE_HOME/irice/cloud):
accounts.json (who, never secrets) and cache.json (the last sync).
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
    d = os.path.join(base, "irice", "cloud")
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
    """The Secret Service through secret-tool. (IRICE_CLOUD_SECRET_FILE, a
    JSON file instead, is for the tests only.)"""

    def __init__(self):
        self.file = os.environ.get("IRICE_CLOUD_SECRET_FILE")

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
        r = subprocess.run(["secret-tool", "store", "--label", f"iRice: iCloud ({account})", "service", "irice-icloud",
                            "account", account], input=secret, text=True, capture_output=True, timeout=60)
        if r.returncode != 0:
            raise Failed("Couldn't save the password in your keyring. Check that a keyring (GNOME Keyring or KWallet) is running.")

    def lookup(self, account):
        if self.file:
            return self._load().get(account)
        r = subprocess.run(["secret-tool", "lookup", "service", "irice-icloud", "account", account],
                           capture_output=True, text=True, timeout=60)
        return r.stdout if r.returncode == 0 and r.stdout else None

    def clear(self, account):
        if self.file:
            d = self._load()
            d.pop(account, None)
            with open(self.file, "w") as f:
                json.dump(d, f)
            return
        subprocess.run(["secret-tool", "clear", "service", "irice-icloud", "account", account],
                       capture_output=True, timeout=60)


# ── HTTP ────────────────────────────────────────────────────────────────
class _NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *a, **k):
        return None


def http(method, url, body=None, headers=None, auth=None, token=None):
    """(status, headers, text). Redirects are followed for every method
    (PROPFIND and REPORT too, which urllib wouldn't)."""
    for _ in range(5):
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
    lines = ["BEGIN:VCALENDAR", "VERSION:2.0", "PRODID:-//iRice//Calendar//EN", "BEGIN:VEVENT", "UID:" + uid,
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
    keys = {"title": "SUMMARY", "location": "LOCATION", "notes": "DESCRIPTION"}
    now = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    replaced = set()
    for line in lines:
        if not line:
            continue
        name = re.split(r"[;:]", line, 1)[0].upper()
        if line.upper() == "BEGIN:VEVENT" and not done:
            in_event = True
            out.append(line)
            continue
        if in_event and line.upper() == "END:VEVENT":
            # the fields it didn't have yet
            for k, prop in keys.items():
                if k in fields and prop not in replaced and fields[k]:
                    out.append(prop + ":" + ics_escape(fields[k]))
            if "start" in fields and "DTSTART" not in replaced:
                out.append("DTSTART" + ics_time(fields["start"], fields.get("allDay")))
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
            if comps and "VEVENT" not in comps:
                continue  # (reminders lists)
            privs = [p.tag.split("}")[1] for p in prop.findall("d:current-user-privilege-set/d:privilege/*", NS)]
            color = (prop.findtext("ical:calendar-color", "", NS) or "").strip()
            out.append({"id": href, "title": prop.findtext("d:displayname", "", NS) or "Calendar",
                        "color": color[:7] if color.startswith("#") else "",
                        "writable": not privs or bool({"write", "write-content", "all"} & set(privs))})
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
            raise Failed("Online Accounts can't give iRice access now. Sign in to Microsoft 365 again in Settings.") from None

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
        self.secrets = Secrets()

    def save(self):
        write("accounts.json", self.accounts)
        write("cache.json", self.cache)

    def account(self, aid):
        a = next((a for a in self.accounts if a["id"] == aid), None)
        if not a:
            raise Failed("That account isn't connected any more.")
        return a

    def dav(self, a, card=False):
        password = self.secrets.lookup(a["identity"])
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
        self.save()

    # Syncing: all of an account, or nothing of it (the last good copy stays)
    def sync(self, only=None):
        """Every account, or `only` that one (after a change to it: another
        account's trouble isn't this change's)."""
        errors = []
        for a in self.accounts:
            if only and a["id"] != only:
                continue
            try:
                got = self.sync_icloud(a) if a["provider"] == "icloud" else self.sync_microsoft(a)
            except Failed as e:
                errors.append((a, e))
                continue
            for k in ("calendars", "events", "taskLists", "tasks", "contacts"):
                self.cache[k] = [x for x in self.cache.get(k, []) if x.get("account") != a["id"]] + got.get(k, [])
            a["syncedAt"] = int(time.time() * 1000)
        self.cache["syncedAt"] = int(time.time() * 1000)
        self.save()
        if errors:
            a, e = errors[0]
            raise type(e)(str(e))

    def sync_icloud(self, a):
        d = self.dav(a)
        home = a.get("home") or d.home("calendar-home-set")
        a["home"] = home
        now = datetime.now(timezone.utc)
        cals = d.calendars(home)
        events = []
        for c in cals:
            for e in d.events(c["id"], now - BACK, now + AHEAD):
                p = ics_props(e["ics"])
                events.append(dict(e, account=a["id"], calendarId=c["id"], uid=p["uid"],
                                   writable=c["writable"] and not p["repeats"]))
        contacts = []
        try:
            cd = self.dav(a, card=True)
            contacts = cd.contacts(a.get("cardHome") or cd.home("addressbook-home-set"))
        except Failed:
            pass  # (contacts are a bonus: calendars still sync)
        return {"calendars": [dict(c, account=a["id"]) for c in cals], "events": events,
                "contacts": [dict(c, account=a["id"]) for c in contacts]}

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
    def event_insert(self, req):
        a, f = self.account(req["account"]), req.get("fields", {})
        if a["provider"] == "icloud":
            uid = str(uuid.uuid4()).upper()
            url = req["calendarId"].rstrip("/") + "/" + uid + ".ics"
            self.dav(a).put(url, new_ics(uid, f), create=True)
        else:
            self.graph(a).call("POST", f"/me/calendars/{urllib.parse.quote(req['calendarId'])}/events", graph_event_body(f))
        self.sync(only=a["id"])

    def event_patch(self, req):
        a, f = self.account(req["account"]), req.get("fields", {})
        if a["provider"] == "icloud":
            d = self.dav(a)
            text, etag = d.get(req["id"])
            if req.get("etag") and etag and etag != req["etag"]:
                self.sync(only=a["id"])
                raise Failed("This event changed on your other devices. It shows as it is there now.")
            d.put(req["id"], patch_ics(text, f), etag=etag or req.get("etag"))
        else:
            self.graph(a).call("PATCH", f"/me/events/{urllib.parse.quote(req['id'])}", graph_event_body(f), etag=req.get("etag"))
        self.sync(only=a["id"])

    def event_delete(self, req):
        a = self.account(req["account"])
        if a["provider"] == "icloud":
            self.dav(a).delete(req["id"], req.get("etag"))
        else:
            self.graph(a).call("DELETE", f"/me/events/{urllib.parse.quote(req['id'])}")
        self.sync(only=a["id"])

    def task(self, req):
        a = self.account(req["account"])
        if a["provider"] != "microsoft":
            raise Failed("To-dos sync with Microsoft To Do only.")
        g, f = self.graph(a), req.get("fields", {})
        base = f"/me/todo/lists/{urllib.parse.quote(req['listId'])}/tasks"
        body = {}
        if "text" in f:
            body["title"] = f["text"]
        if "done" in f:
            body["status"] = "completed" if f["done"] else "notStarted"
        if "date" in f:
            body["dueDateTime"] = {"dateTime": f["date"] + "T00:00:00", "timeZone": "UTC"} if f["date"] else None
        if req["cmd"] == "taskInsert":
            g.call("POST", base, body)
        elif req["cmd"] == "taskPatch":
            g.call("PATCH", base + "/" + urllib.parse.quote(req["id"]), body, etag=req.get("etag"))
        else:
            g.call("DELETE", base + "/" + urllib.parse.quote(req["id"]))
        self.sync(only=a["id"])

    def state(self):
        c = self.cache
        return {"accounts": [{k: a.get(k, "") for k in ("id", "provider", "identity", "syncedAt")} for a in self.accounts],
                "calendars": c.get("calendars", []), "events": c.get("events", []), "taskLists": c.get("taskLists", []),
                "tasks": c.get("tasks", []), "contacts": c.get("contacts", []), "syncedAt": c.get("syncedAt", 0)}


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
        elif cmd == "connectICloud":
            hub.connect_icloud(req)
        elif cmd == "connectMicrosoft":
            hub.connect_microsoft(req)
        elif cmd == "microsoftAccounts":
            out["goaAccounts"] = OnlineAccounts().accounts()
        elif cmd == "disconnect":
            hub.disconnect(req["account"])
        elif cmd == "eventInsert":
            hub.event_insert(req)
        elif cmd == "eventPatch":
            hub.event_patch(req)
        elif cmd == "eventDelete":
            hub.event_delete(req)
        elif cmd in ("taskInsert", "taskPatch", "taskDelete"):
            hub.task(req)
    except Offline as e:
        out.update(error=str(e), offline=True)
    except Failed as e:
        out["error"] = str(e)
    except (KeyError, TypeError) as e:
        out["error"] = f"iRice sent an incomplete request ({e})."
    except Exception as e:  # noqa: BLE001 (the shell always gets an answer)
        out["error"] = f"Something went wrong: {e}"
    out.update(hub.state(), cmd=cmd)
    print(json.dumps(out))
    return 0


if __name__ == "__main__":
    sys.exit(main())
