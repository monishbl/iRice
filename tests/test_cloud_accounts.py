"""cloud-accounts.py: iCloud (CalDAV/CardDAV) and Microsoft 365 (Graph).

iCalendar edits keep what iRice doesn't change (alarms, attendees). The
CalDAV side runs against a real CalDAV server, Radicale, when one is there
(IRICE_TEST_RADICALE: a Python with radicale; skipped otherwise): connect,
add, edit, delete, repeating events left alone, contacts, a refused password.
The Graph side runs against a stand-in Microsoft Graph."""
import http.server
import importlib.util
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time
import unittest
import urllib.parse
from pathlib import Path

SCRIPT = Path(__file__).resolve().parent.parent / "shell/assets/scripts/cloud-accounts.py"
spec = importlib.util.spec_from_file_location("cloud_accounts", SCRIPT)
ca = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ca)


def run(req, env):
    r = subprocess.run([sys.executable, "-I", str(SCRIPT)], input=json.dumps(req) + "\n", capture_output=True,
                       text=True, env=env, timeout=60)
    return json.loads(r.stdout.strip().splitlines()[-1])


def free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    p = s.getsockname()[1]
    s.close()
    return p


class Ics(unittest.TestCase):
    EVENT = ("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:abc\r\nDTSTAMP:20260101T000000Z\r\n"
             "DTSTART:20261101T090000Z\r\nDTEND:20261101T100000Z\r\nSUMMARY:Dentist\r\nSEQUENCE:2\r\n"
             "X-APPLE-TRAVEL-ADVISORY-BEHAVIOR:AUTOMATIC\r\nBEGIN:VALARM\r\nTRIGGER:-PT15M\r\nACTION:DISPLAY\r\n"
             "END:VALARM\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n")

    def test_patch_keeps_the_rest(self):
        out = ca.patch_ics(self.EVENT, {"title": "Dentist, again", "location": "Main St"})
        self.assertIn("SUMMARY:Dentist\\, again", out)
        self.assertIn("LOCATION:Main St", out)
        self.assertIn("TRIGGER:-PT15M", out)
        self.assertIn("X-APPLE-TRAVEL-ADVISORY-BEHAVIOR:AUTOMATIC", out)
        self.assertIn("SEQUENCE:3", out)
        self.assertIn("DTSTART:20261101T090000Z", out)

    def test_patch_times(self):
        start = int(ca.datetime(2026, 11, 1, 6, 40, tzinfo=ca.timezone.utc).timestamp() * 1000)
        out = ca.patch_ics(self.EVENT, {"start": start, "end": start + 3600000, "allDay": False})
        self.assertIn("DTSTART:20261101T064000Z", out)
        self.assertIn("DTEND:20261101T074000Z", out)
        self.assertEqual(out.count("DTEND"), 1)

    def test_new_and_fold(self):
        out = ca.new_ics("u1", {"title": "x" * 200, "start": 1793520000000, "allDay": True, "notes": "a\nb"})
        self.assertIn("DTSTART;VALUE=DATE:", out)
        self.assertIn("DESCRIPTION:a\\nb", out)
        self.assertTrue(all(len(l.encode()) <= 75 for l in out.split("\r\n")))
        self.assertIn("x" * 50, "".join(ca.unfold(out)))

    def test_repeats(self):
        self.assertFalse(ca.ics_props(self.EVENT)["repeats"])
        self.assertTrue(ca.ics_props(self.EVENT.replace("SEQUENCE:2", "RRULE:FREQ=WEEKLY"))["repeats"])

    def test_vcards(self):
        text = ("BEGIN:VCARD\nVERSION:3.0\nN:Doe;Jane;;;\nFN:Jane Doe\nitem1.EMAIL;type=INTERNET:jane@example.com\n"
                "TEL;type=CELL:+1 555 0100\nEND:VCARD\nBEGIN:VCARD\nVERSION:3.0\nFN:No Way To Reach\nEND:VCARD\n")
        self.assertEqual(ca.vcards(text), [{"name": "Jane Doe", "emails": ["jane@example.com"], "phones": ["+1 555 0100"]}])


# ── Against a real CalDAV server ────────────────────────────────────────
RADICALE = os.environ.get("IRICE_TEST_RADICALE", "")


@unittest.skipUnless(RADICALE and os.path.exists(RADICALE), "set IRICE_TEST_RADICALE to a Python with radicale")
class CalDav(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        t = cls.tmp.name
        Path(t, "users").write_text("jane@icloud.com:abcd-efgh-ijkl-mnop\n")
        cls.port = free_port()
        cls.server = subprocess.Popen([RADICALE, "-m", "radicale", "--server-hosts", f"127.0.0.1:{cls.port}",
                                       "--storage-filesystem-folder", os.path.join(t, "store"), "--auth-type", "htpasswd",
                                       "--auth-htpasswd-filename", os.path.join(t, "users"), "--auth-htpasswd-encryption", "plain",
                                       "--rights-type", "owner_only", "--logging-level", "error"],
                                      stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        base = f"http://127.0.0.1:{cls.port}/"
        auth = ("jane@icloud.com", "abcd-efgh-ijkl-mnop")
        for _ in range(50):
            try:
                ca.http("GET", base, auth=auth)
                break
            except ca.Offline:
                time.sleep(0.1)
        cal = ('<?xml version="1.0"?><c:mkcalendar xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:set><d:prop>'
               '<d:displayname>Home</d:displayname><c:supported-calendar-component-set><c:comp name="VEVENT"/>'
               '</c:supported-calendar-component-set></d:prop></d:set></c:mkcalendar>')
        cls.cal = base + "jane@icloud.com/home/"
        st, _, _ = ca.http("MKCALENDAR", cls.cal, cal, {"Content-Type": "application/xml"}, auth=auth)
        assert st in (200, 201), st
        book = ('<?xml version="1.0"?><d:mkcol xmlns:d="DAV:" xmlns:card="urn:ietf:params:xml:ns:carddav"><d:set><d:prop>'
                '<d:resourcetype><d:collection/><card:addressbook/></d:resourcetype></d:prop></d:set></d:mkcol>')
        st, _, _ = ca.http("MKCOL", base + "jane@icloud.com/contacts/", book, {"Content-Type": "application/xml"}, auth=auth)
        assert st in (200, 201), st
        ca.http("PUT", base + "jane@icloud.com/contacts/jane.vcf",
                "BEGIN:VCARD\r\nVERSION:3.0\r\nUID:jane\r\nFN:Jane Doe\r\nN:Doe;Jane;;;\r\nEMAIL:jane@example.com\r\nEND:VCARD\r\n",
                {"Content-Type": "text/vcard"}, auth=auth)
        cls.base, cls.auth = base, auth

    @classmethod
    def tearDownClass(cls):
        cls.server.terminate()
        cls.server.wait()
        cls.tmp.cleanup()

    def setUp(self):
        self.state = tempfile.TemporaryDirectory()
        self.env = dict(os.environ, XDG_STATE_HOME=self.state.name,
                        IRICE_CLOUD_SECRET_FILE=os.path.join(self.state.name, "secrets.json"))

    def tearDown(self):
        self.state.cleanup()

    def connect(self, password="abcd-efgh-ijkl-mnop"):
        return run({"cmd": "connectICloud", "appleId": "jane@icloud.com", "password": password,
                    "server": self.base, "cardServer": self.base}, self.env)

    def test_refused(self):
        s = self.connect("wrong")
        self.assertIn("app-specific password", s["error"])
        self.assertEqual(s["accounts"], [])
        self.assertFalse(os.path.exists(os.path.join(self.state.name, "secrets.json")))  # (not kept)

    def test_round_trip(self):
        s = self.connect()
        self.assertNotIn("error", s)
        self.assertEqual([c["title"] for c in s["calendars"]], ["Home"])
        self.assertEqual(s["contacts"][0]["name"], "Jane Doe")
        # the password is in the keyring (here: the test's file), never the account file
        self.assertNotIn("abcd", Path(self.state.name, "irice/cloud/accounts.json").read_text())
        acc, cal = s["accounts"][0]["id"], s["calendars"][0]["id"]
        start = int(time.time() // 3600 * 3600 + 86400) * 1000

        s = run({"cmd": "eventInsert", "account": acc, "calendarId": cal,
                 "fields": {"title": "Dentist", "start": start, "end": start + 3600000, "allDay": False, "notes": "Bring card"}}, self.env)
        self.assertNotIn("error", s)
        ev = next(e for e in s["events"] if "SUMMARY:Dentist" in e["ics"])
        self.assertTrue(ev["writable"])
        self.assertIn("DESCRIPTION:Bring card", ev["ics"])

        s = run({"cmd": "eventPatch", "account": acc, "calendarId": cal, "id": ev["href"], "etag": ev["etag"],
                 "fields": {"title": "Dentist (moved)", "location": "Clinic"}}, self.env)
        self.assertNotIn("error", s)
        ev2 = next(e for e in s["events"] if e["href"] == ev["href"])
        self.assertIn("SUMMARY:Dentist (moved)", ev2["ics"])
        self.assertIn("LOCATION:Clinic", ev2["ics"])
        self.assertIn("DESCRIPTION:Bring card", ev2["ics"])  # (kept)

        # an edit made against an old version is turned away
        s = run({"cmd": "eventPatch", "account": acc, "calendarId": cal, "id": ev["href"], "etag": ev["etag"],
                 "fields": {"title": "Stale"}}, self.env)
        self.assertIn("changed on your other devices", s["error"])

        s = run({"cmd": "eventDelete", "account": acc, "calendarId": cal, "id": ev2["href"], "etag": ev2["etag"]}, self.env)
        self.assertNotIn("error", s)
        self.assertFalse(any(e["href"] == ev["href"] for e in s["events"]))

    def test_repeating_is_read_only(self):
        s = self.connect()
        stamp = time.strftime("%Y%m%dT090000Z", time.gmtime(time.time() + 86400))
        ca.http("PUT", self.cal + "weekly.ics", "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nBEGIN:VEVENT\r\nUID:weekly\r\nDTSTAMP:20260101T000000Z\r\n"
                f"DTSTART:{stamp}\r\nDURATION:PT1H\r\nRRULE:FREQ=WEEKLY\r\nSUMMARY:Standup\r\nEND:VEVENT\r\nEND:VCALENDAR\r\n",
                {"Content-Type": "text/calendar"}, auth=self.auth)
        s = run({"cmd": "sync"}, self.env)
        ev = next(e for e in s["events"] if e["uid"] == "weekly")
        self.assertFalse(ev["writable"])

    def test_disconnect(self):
        s = self.connect()
        s = run({"cmd": "disconnect", "account": s["accounts"][0]["id"]}, self.env)
        self.assertEqual(s["accounts"], [])
        self.assertEqual(s["events"], [])
        self.assertEqual(json.loads(Path(self.state.name, "secrets.json").read_text()), {})


# ── A stand-in Microsoft Graph ──────────────────────────────────────────
class FakeGraph(http.server.ThreadingHTTPServer):
    def __init__(self):
        super().__init__(("127.0.0.1", 0), GraphHandler)
        self.calls = []
        self.events = {"e1": {"id": "e1", "@odata.etag": 'W/"1"', "subject": "Standup", "type": "occurrence", "seriesMasterId": "s1",
                              "start": {"dateTime": "2026-11-02T09:00:00.0000000", "timeZone": "UTC"},
                              "end": {"dateTime": "2026-11-02T09:15:00.0000000", "timeZone": "UTC"}, "isAllDay": False,
                              "location": {"displayName": "Teams"}, "bodyPreview": ""},
                       "e2": {"id": "e2", "@odata.etag": 'W/"7"', "subject": "Holiday", "type": "singleInstance",
                              "start": {"dateTime": "2026-12-25T00:00:00.0000000", "timeZone": "UTC"},
                              "end": {"dateTime": "2026-12-26T00:00:00.0000000", "timeZone": "UTC"}, "isAllDay": True}}
        self.tasks = {"t1": {"id": "t1", "title": "Buy milk", "status": "notStarted",
                             "dueDateTime": {"dateTime": "2026-11-03T00:00:00.0000000", "timeZone": "UTC"}}}


class GraphHandler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def reply(self, code, body=None):
        data = json.dumps(body).encode() if body is not None else b""
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def body(self):
        n = int(self.headers.get("Content-Length") or 0)
        return json.loads(self.rfile.read(n) or b"{}")

    def handle_any(self, method):
        srv, path = self.server, urllib.parse.urlparse(self.path)
        body = self.body() if method in ("POST", "PATCH") else None
        srv.calls.append((method, path.path, urllib.parse.parse_qs(path.query), body, self.headers.get("If-Match")))
        if self.headers.get("Authorization") != "Bearer test-token":
            return self.reply(401, {"error": {}})
        p = path.path.replace("/v1.0", "")
        if p == "/me":
            return self.reply(200, {"userPrincipalName": "sam@outlook.com"})
        if p == "/me/calendars":
            return self.reply(200, {"value": [{"id": "cal1", "name": "Calendar", "hexColor": "#0078d4", "canEdit": True}]})
        if p == "/me/calendars/cal1/calendarView":
            return self.reply(200, {"value": list(srv.events.values())})
        if p == "/me/calendars/cal1/events" and method == "POST":
            srv.events["new"] = dict(body, id="new", type="singleInstance")
            return self.reply(201, srv.events["new"])
        if p.startswith("/me/events/"):
            eid = p.split("/")[-1]
            if method == "DELETE":
                srv.events.pop(eid, None)
                return self.reply(204)
            if self.headers.get("If-Match") != srv.events[eid].get("@odata.etag"):
                return self.reply(412, {"error": {}})
            srv.events[eid].update(body)
            return self.reply(200, srv.events[eid])
        if p == "/me/todo/lists":
            return self.reply(200, {"value": [{"id": "l1", "displayName": "Tasks"}]})
        if p == "/me/todo/lists/l1/tasks":
            if method == "POST":
                srv.tasks["t2"] = dict(body, id="t2", status=body.get("status", "notStarted"))
                return self.reply(201, srv.tasks["t2"])
            return self.reply(200, {"value": [t for t in srv.tasks.values() if t["status"] != "completed"]})
        if p.startswith("/me/todo/lists/l1/tasks/"):
            tid = p.split("/")[-1]
            if method == "DELETE":
                srv.tasks.pop(tid, None)
                return self.reply(204)
            srv.tasks[tid].update(body)
            return self.reply(200, srv.tasks[tid])
        self.reply(404, {"error": {}})

    def do_GET(self):
        self.handle_any("GET")

    def do_POST(self):
        self.handle_any("POST")

    def do_PATCH(self):
        self.handle_any("PATCH")

    def do_DELETE(self):
        self.handle_any("DELETE")


class Microsoft(unittest.TestCase):
    def setUp(self):
        self.srv = FakeGraph()
        threading.Thread(target=self.srv.serve_forever, daemon=True).start()
        self.state = tempfile.TemporaryDirectory()
        self.env = dict(os.environ, XDG_STATE_HOME=self.state.name)
        self.graph = f"http://127.0.0.1:{self.srv.server_address[1]}/v1.0"

    def tearDown(self):
        self.srv.shutdown()
        self.srv.server_close()
        self.state.cleanup()

    def connect(self, token="test-token"):
        return run({"cmd": "connectMicrosoft", "identity": "sam@outlook.com", "graph": self.graph, "testToken": token}, self.env)

    def test_sync(self):
        s = self.connect()
        self.assertNotIn("error", s)
        ev = {e["id"]: e for e in s["events"]}
        self.assertEqual(ev["e1"]["start"], {"dateTime": "2026-11-02T09:00:00Z"})
        self.assertFalse(ev["e1"]["writable"])  # (an occurrence of a series)
        self.assertEqual(ev["e2"]["start"], {"date": "2026-12-25"})
        self.assertTrue(ev["e2"]["writable"])
        self.assertEqual(s["tasks"][0]["text"], "Buy milk")
        self.assertEqual(s["tasks"][0]["date"], "2026-11-03")
        view = next(c for c in self.srv.calls if c[1].endswith("/calendarView"))
        self.assertIn("startDateTime", view[2])

    def test_refused(self):
        s = self.connect("expired")
        self.assertIn("sign in", s["error"].lower())
        self.assertEqual(s["accounts"], [])

    def test_changes(self):
        s = self.connect()
        acc = s["accounts"][0]["id"]
        start = int(ca.datetime(2026, 11, 1, 6, 40, tzinfo=ca.timezone.utc).timestamp() * 1000)
        s = run({"cmd": "eventInsert", "account": acc, "calendarId": "cal1",
                 "fields": {"title": "Lunch", "start": start, "end": start + 3600000, "allDay": False, "location": "Cafe"}}, self.env)
        post = next(c for c in self.srv.calls if c[0] == "POST" and c[1].endswith("/events"))
        self.assertEqual(post[3]["subject"], "Lunch")
        self.assertEqual(post[3]["start"], {"dateTime": "2026-11-01T06:40:00", "timeZone": "UTC"})
        self.assertEqual(post[3]["location"], {"displayName": "Cafe"})

        s = run({"cmd": "eventPatch", "account": acc, "calendarId": "cal1", "id": "e2", "etag": 'W/"7"', "fields": {"title": "Christmas"}}, self.env)
        self.assertNotIn("error", s)
        s = run({"cmd": "eventPatch", "account": acc, "calendarId": "cal1", "id": "e2", "etag": 'W/"old"', "fields": {"title": "x"}}, self.env)
        self.assertIn("changed on your other devices", s["error"])
        s = run({"cmd": "eventDelete", "account": acc, "calendarId": "cal1", "id": "e2"}, self.env)
        self.assertNotIn("e2", [e["id"] for e in s["events"]])

        s = run({"cmd": "taskPatch", "account": acc, "listId": "l1", "id": "t1", "fields": {"done": True}}, self.env)
        self.assertEqual(s["tasks"], [])  # (done: not shown)
        s = run({"cmd": "taskInsert", "account": acc, "listId": "l1", "fields": {"text": "Call mum", "date": "2026-11-05"}}, self.env)
        self.assertEqual([t["text"] for t in s["tasks"]], ["Call mum"])
        post = [c for c in self.srv.calls if c[0] == "POST" and c[1].endswith("/tasks")][-1]
        self.assertEqual(post[3]["dueDateTime"], {"dateTime": "2026-11-05T00:00:00", "timeZone": "UTC"})


if __name__ == "__main__":
    unittest.main()
