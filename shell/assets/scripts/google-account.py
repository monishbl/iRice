#!/usr/bin/env python3
"""Google Calendar and Tasks for iRice: one request per run, JSON on stdin,
JSON lines out (Google.qml).

Several accounts at once: each has its own directory (accounts/<id>/: its
connection, cache and sync position), and what the shell sees is all of them
together, each item marked with its account.

Changes (tasks and events: add, edit, move, delete) go through an outbox
(outbox.json): written to disk before anything is sent, so a change made
offline, or while Google or the helper fails, survives a restart and is sent
on the next run. Until it's sent, what the shell sees already includes it.
Edits carry the ETag the item had, so one that changed on Google meanwhile
isn't overwritten: Google's version is kept and the shell is told once, with
what you had, to copy.

Signing in: through GNOME Online Accounts (GOA), the desktop's account
broker, so iRice never holds a password or refresh token; the account is
added from iRice's Settings by the native helper goa-signin (browser sign-in,
no GNOME Settings app). A custom OAuth client (loopback, PKCE) is the
alternative for people who'd rather use their own.

Syncing is incremental, so a sync where nothing changed costs a handful of
tiny requests: calendars use Google's sync tokens (a token that's expired,
410 Gone, means that calendar is fetched in full again), task lists have no
sync tokens, so only tasks updated since the last sync are asked for
(updatedMin, a minute early to cover clock skew, with deletions included).
The data (cache.json) and the sync position (sync.json) are only replaced
once a whole sync succeeded, and stay private (0600 in a 0700 directory).
No third-party dependencies.
"""
import base64
import hashlib
import http.server
import json
import os
from pathlib import Path
import secrets
import shutil
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
from datetime import datetime, timedelta, timezone

SCOPES = ['https://www.googleapis.com/auth/calendar.events',
          'https://www.googleapis.com/auth/calendar.calendarlist.readonly',
          'https://www.googleapis.com/auth/tasks']
CAL = 'https://www.googleapis.com/calendar/v3'
TASK = 'https://tasks.googleapis.com/tasks/v1'
WINDOW = timedelta(days=366)  # events kept: a year either side of now
SKEW = 60  # seconds asked for again on each task sync (clocks differ)


class HttpError(RuntimeError):
    """Google answered with an error; `code` is the HTTP status."""
    def __init__(self, message, code=0):
        super().__init__(message)
        self.code = code


class Gone(HttpError):
    """A sync token Google no longer accepts, or an item that's been deleted
    (HTTP 410): sync that part in full, or treat the deletion as done."""
    def __init__(self, message='Gone.', code=410):
        super().__init__(message, code)


class Conflict(HttpError):
    """The item changed on Google since it was read (HTTP 412, If-Match)."""
    def __init__(self, message='The item changed on Google.', code=412):
        super().__init__(message, code)


class Offline(RuntimeError):
    """Google couldn't be reached: changes wait in the outbox."""


def emit(value):
    print(json.dumps(value), flush=True)


def atomic(path, value):
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    os.chmod(path.parent, 0o700)
    fd, tmp = tempfile.mkstemp(dir=path.parent)
    try:
        with os.fdopen(fd, 'w') as f:
            json.dump(value, f)
        os.replace(tmp, path)
    finally:
        if os.path.exists(tmp):
            os.unlink(tmp)


def read(path):
    try:
        return json.loads(path.read_text())
    except FileNotFoundError:
        return {}


def request(url, data=None, token=None, method=None, form=False, headers=None):
    headers = dict(headers or {})
    if token:
        headers['Authorization'] = 'Bearer ' + token
    if data is not None:
        headers['Content-Type'] = 'application/x-www-form-urlencoded' if form else 'application/json'
        data = (urllib.parse.urlencode(data) if form else json.dumps(data)).encode()
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=30) as response:
            body = response.read()
            return json.loads(body) if body else {}
    except urllib.error.HTTPError as exc:
        if exc.code == 410:
            raise Gone('Sync position expired.') from None
        if exc.code == 412:
            raise Conflict() from None
        if exc.code >= 500:
            raise Offline('Google is having trouble right now. Changes are kept and sent later.') from None
        # Never include request URLs, tokens or the raw server body in UI errors.
        hints = {400: 'Invalid request or expired sign-in. Reconnect if needed.',
                 401: 'Google sign-in expired. Connect again.',
                 403: 'Access denied. Check Online Accounts permissions. For a custom client, enable Calendar and Tasks APIs.',
                 404: 'This Google item no longer exists.',
                 409: 'The item changed on Google. Sync and try again.',
                 429: 'Google rate limit reached. Try again later.'}
        raise HttpError(hints.get(exc.code, 'Google request failed (HTTP %s).' % exc.code), exc.code) from None
    except (urllib.error.URLError, TimeoutError, ConnectionError):
        raise Offline("Can't reach Google. Check your connection; changes are kept and sent when you're back online.") from None


def quote(value):
    return urllib.parse.quote(value, safe='')


def when(ms, all_day):
    """A time from the shell (ms since the epoch) as Google's start or end."""
    t = datetime.fromtimestamp(ms / 1000).astimezone()
    return {'date': t.date().isoformat()} if all_day else {'dateTime': t.isoformat()}


def ms(value):
    """Google's start or end as ms since the epoch (an all-day date at local midnight)."""
    if value.get('dateTime'):
        return datetime.fromisoformat(value['dateTime'].replace('Z', '+00:00')).timestamp() * 1000
    return datetime.fromisoformat(value['date']).timestamp() * 1000


def event_body(fields, patching=False):
    """The shell's event fields (title, start, end in ms, allDay, location,
    notes) as a Calendar API body. Patching from timed to all-day (or back)
    clears the other kind of time, since a patch merges into what's there."""
    body = {}
    if 'title' in fields:
        if not str(fields['title']).strip():
            raise ValueError('An event needs a title.')
        body['summary'] = fields['title']
    if 'location' in fields:
        body['location'] = fields['location']
    if 'notes' in fields:
        body['description'] = fields['notes']
    if 'start' in fields:
        all_day = bool(fields.get('allDay'))
        for key in ('start', 'end'):
            value = when(fields.get(key, fields['start']), all_day)
            if patching:
                value['dateTime' if all_day else 'date'] = None
            body[key] = value
    return body


def task_body(patch):
    body = {}
    if 'text' in patch:
        if not patch['text'].strip():
            raise ValueError('A task needs a title.')
        body['title'] = patch['text']
    if 'done' in patch:
        body['status'] = 'completed' if patch['done'] else 'needsAction'
        if not patch['done']:
            body['completed'] = None
    if 'date' in patch:
        body['due'] = patch['date'] + 'T00:00:00.000Z' if patch['date'] else None
    if 'notes' in patch:
        body['notes'] = patch['notes']
    return body


class OnlineAccounts:
    """Use the desktop's account broker; never copy its refresh tokens or client ID."""
    name = 'org.gnome.OnlineAccounts'
    prefix = '/org/gnome/OnlineAccounts/Accounts/'

    def call(self, path, interface, method):
        try:
            from gi.repository import Gio
            bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
            return bus.call_sync(self.name, path, interface, method, None, None,
                                 Gio.DBusCallFlags.NONE, 30000, None).unpack()
        except Exception:
            raise RuntimeError('Online Accounts is unavailable or needs sign-in. Open Online Accounts, enable Calendars, and reconnect.') from None

    def accounts(self):
        try:
            objects = self.call('/org/gnome/OnlineAccounts', 'org.freedesktop.DBus.ObjectManager', 'GetManagedObjects')[0]
        except RuntimeError:
            return []
        accounts = []
        for path, interfaces in objects.items():
            account = interfaces.get(self.name + '.Account', {})
            if (path.startswith(self.prefix) and account.get('ProviderType') == 'google'
                    and self.name + '.OAuth2Based' in interfaces):
                accounts.append({'id': path, 'name': account.get('PresentationIdentity', account.get('Identity', 'Google')),
                                 'calendarDisabled': bool(account.get('CalendarDisabled', False))})
        return accounts

    # Calendars switched off for an account in Online Accounts: connecting it
    # to iRice is asking for them, so they're switched on
    def enable_calendar(self, path):
        try:
            from gi.repository import Gio, GLib
            bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
            bus.call_sync(self.name, path, 'org.freedesktop.DBus.Properties', 'Set',
                          GLib.Variant('(ssv)', (self.name + '.Account', 'CalendarDisabled', GLib.Variant('b', False))),
                          None, Gio.DBusCallFlags.NONE, 10000, None)
        except Exception:
            raise RuntimeError('Couldn\'t turn on Calendars for this account. Open Online Accounts and turn it on.') from None

    def access(self, path):
        if path not in [account['id'] for account in self.accounts()]:
            raise RuntimeError('That Google account is no longer in Online Accounts. Connect your Google account again.')
        self.call(path, self.name + '.Account', 'EnsureCredentials')
        token, _expires = self.call(path, self.name + '.OAuth2Based', 'GetAccessToken')
        return token


class Account:
    """One Google account: its connection, cache and sync position."""
    def __init__(self, directory, credentials=None, broker=None):
        self.directory = Path(directory)
        self.id = self.directory.name
        self.credentials = Path(credentials) if credentials else self.directory / 'client.json'
        self.meta = self.directory / 'account.json'  # identity, when it was added
        self.tokens = self.directory / 'tokens.json'
        self.cache = self.directory / 'cache.json'
        self.connection = self.directory / 'connection.json'
        self.position = self.directory / 'sync.json'  # sync tokens and times; never shown
        self.broker = broker or OnlineAccounts()

    @property
    def goa(self):
        return read(self.connection).get('account', '')

    def linked(self):
        return self.tokens.exists() or bool(self.goa)

    def status(self):
        connection = read(self.connection)
        accounts = self.broker.accounts()
        identity = (connection.get('identity') or read(self.meta).get('identity')
                    or next((a['name'] for a in accounts if a['id'] == connection.get('account')), ''))
        return dict(read(self.cache), connected=self.tokens.exists() or bool(connection.get('account')),
                    configured=self.credentials.exists(), backend='goa' if connection.get('account') else 'direct',
                    accountId=connection.get('account', ''), identity=identity,
                    goaAccounts=accounts, signin=bool(self.signin_helper()),
                    goaSettings=shutil.which('gnome-online-accounts-gtk') or shutil.which('gnome-control-center') or '')

    # The native helper that adds the account to Online Accounts (goa-signin,
    # built with iRice's native helpers); without it, Online Accounts' own
    # window is the way
    @staticmethod
    def signin_helper():
        here = Path(__file__).resolve().parent
        built = here.parent.parent / 'native' / 'build' / 'goa-signin'
        return str(built) if os.access(built, os.X_OK) else shutil.which('irice-goa-signin') or ''

    def signin(self):
        helper = self.signin_helper()
        if not helper:
            raise RuntimeError('Connecting from here needs iRice\'s native helpers. Run Software Update (or irice rebuild), or use Online Accounts.')
        import subprocess
        # (the shell moves Settings out of the way while the sign-in window
        # and the browser are up: it would cover them)
        emit({'signingIn': True})
        try:
            out = subprocess.run([helper], capture_output=True, text=True, timeout=330).stdout
        except subprocess.TimeoutExpired:
            raise RuntimeError('Sign-in took too long. Try again.') from None
        reply = {}
        for line in out.splitlines():
            try:
                reply = json.loads(line)
            except ValueError:
                pass
        if not reply.get('account'):
            raise RuntimeError(reply.get('error') or 'Sign-in ended without an answer. Try again.')
        return reply

    def connect(self):
        client = read(self.credentials)
        if not client.get('client_id'):
            raise ValueError('Import a Desktop app OAuth client JSON first.')
        verifier = secrets.token_urlsafe(48)
        challenge = base64.urlsafe_b64encode(hashlib.sha256(verifier.encode()).digest()).rstrip(b'=').decode()
        state = secrets.token_urlsafe(32)
        result = {}

        class Callback(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                url = urllib.parse.urlparse(self.path)
                query = urllib.parse.parse_qs(url.query)
                valid = url.path == '/callback' and secrets.compare_digest(query.get('state', [''])[0], state)
                if valid:
                    result.update(code=query.get('code', [''])[0], error=query.get('error', [''])[0])
                self.send_response(200 if valid else 400)
                self.send_header('Content-Type', 'text/plain; charset=utf-8')
                self.end_headers()
                self.wfile.write(b'Return to iRice Settings.' if valid else b'Invalid authorization response.')
            def log_message(self, *_):
                pass

        with http.server.HTTPServer(('127.0.0.1', 0), Callback) as server:
            server.timeout = 1
            redirect = 'http://127.0.0.1:%d/callback' % server.server_port
            params = dict(client_id=client['client_id'], redirect_uri=redirect, response_type='code',
                          scope=' '.join(SCOPES), state=state, code_challenge=challenge,
                          code_challenge_method='S256', access_type='offline', prompt='consent')
            emit({'authUrl': 'https://accounts.google.com/o/oauth2/v2/auth?' + urllib.parse.urlencode(params)})
            deadline = time.monotonic() + 180
            while not result and time.monotonic() < deadline:
                server.handle_request()
        if not result.get('code'):
            raise RuntimeError('Sign-in cancelled or timed out. Try connecting again.')
        tokens = request('https://oauth2.googleapis.com/token', dict(code=result['code'], client_id=client['client_id'],
                         client_secret=client.get('client_secret', ''), redirect_uri=redirect,
                         grant_type='authorization_code', code_verifier=verifier), form=True)
        if not tokens.get('refresh_token'):
            raise RuntimeError('Google did not provide offline access. Reconnect and grant access.')
        tokens['expires_at'] = time.time() + tokens.get('expires_in', 3600)
        atomic(self.tokens, tokens)

    def access(self):
        account = read(self.connection).get('account')
        if account:
            return self.broker.access(account)
        tokens = read(self.tokens)
        if not tokens.get('refresh_token'):
            raise RuntimeError('Connect your Google account first.')
        if tokens.get('expires_at', 0) < time.time() + 60:
            client = read(self.credentials)
            updated = request('https://oauth2.googleapis.com/token', dict(client_id=client['client_id'],
                              client_secret=client.get('client_secret', ''), refresh_token=tokens['refresh_token'],
                              grant_type='refresh_token'), form=True)
            tokens.update(updated)
            tokens['expires_at'] = time.time() + tokens.get('expires_in', 3600)
            atomic(self.tokens, tokens)
        return tokens['access_token']

    def pages(self, url, token, **params):
        return self.pages_with_token(url, token, **params)[0]

    # Every page, and the sync token Google gives on the last one
    def pages_with_token(self, url, token, **params):
        items = []
        while True:
            data = request(url + '?' + urllib.parse.urlencode(params), token=token)
            items.extend(data.get('items', []))
            if not data.get('nextPageToken'):
                return items, data.get('nextSyncToken', '')
            params['pageToken'] = data['nextPageToken']

    @staticmethod
    def event(row, calendar):
        return dict(id=row['id'], calendarId=calendar['id'], title=row.get('summary', '(Untitled)'),
                    start=row['start'], end=row['end'], location=row.get('location', ''),
                    notes=row.get('description', ''), color=calendar.get('color', ''),
                    calendar=calendar.get('title', 'Google'), url=row.get('htmlLink', ''),
                    etag=row.get('etag', ''), recurring=bool(row.get('recurringEventId')),
                    series=row.get('recurringEventId', ''),
                    writable=calendar.get('writable', True))

    @staticmethod
    def task(row, listing):
        return dict(id=row['id'], listId=listing['id'], listName=listing['title'],
                    text=row.get('title', ''), done=row.get('status') == 'completed',
                    date=row.get('due', '')[:10], notes=row.get('notes', ''), etag=row.get('etag', ''))

    def sync(self, full=False):
        """Bring the cache up to date: only what changed since the last sync."""
        token = self.access()
        started = time.time()
        now = datetime.now(timezone.utc)
        cache = {} if full else read(self.cache)
        # A cache from before events carried their version and series: read again in full once
        if any('series' not in e for e in cache.get('events', [])) or any('etag' not in t for t in cache.get('tasks', [])):
            full, cache = True, {}
        position = {} if full else read(self.position)
        cal_pos = dict(position.get('calendars', {}))
        task_pos = dict(position.get('tasks', {}))

        # Calendars (the list itself has a sync token too)
        calendars = {c['id']: c for c in cache.get('calendars', [])}
        list_token = position.get('calendarList', '')
        try:
            rows, list_token = self.pages_with_token(CAL + '/users/me/calendarList', token,
                                                     **({'syncToken': list_token} if list_token else {}))
        except Gone:
            calendars, list_token = {}, ''
            rows, list_token = self.pages_with_token(CAL + '/users/me/calendarList', token)
        if not position.get('calendarList'):
            calendars = {}
        for row in rows:
            if row.get('deleted') or row.get('hidden'):
                calendars.pop(row['id'], None)
            else:
                calendars[row['id']] = dict(id=row['id'], title=row.get('summary', 'Google'),
                                            color=row.get('backgroundColor', ''), primary=bool(row.get('primary')),
                                            writable=row.get('accessRole', 'owner') in ('owner', 'writer'))

        # Events, per calendar
        events = {(e['calendarId'], e['id']): e for e in cache.get('events', [])}
        for key in [k for k in events if k[0] not in calendars]:
            del events[key]
        for cid in list(cal_pos):
            if cid not in calendars:
                del cal_pos[cid]
        for cid, calendar in calendars.items():
            path = CAL + '/calendars/' + urllib.parse.quote(cid, safe='') + '/events'
            # (an existing event keeps its colour and calendar name current)
            for key, e in events.items():
                if key[0] == cid:
                    e.update(color=calendar['color'], calendar=calendar['title'], writable=calendar.get('writable', True))
            since = cal_pos.get(cid, '')
            rows = None
            if since:
                try:
                    rows, since = self.pages_with_token(path, token, singleEvents='true', maxResults=2500, syncToken=since)
                except Gone:
                    since = ''
            if rows is None:
                for key in [k for k in events if k[0] == cid]:
                    del events[key]
                rows, since = self.pages_with_token(path, token, singleEvents='true', maxResults=2500,
                                                    timeMin=(now - WINDOW).isoformat(), timeMax=(now + WINDOW).isoformat())
            cal_pos[cid] = since
            for row in rows:
                if row.get('status') == 'cancelled' or not row.get('start') or not row.get('end'):
                    events.pop((cid, row['id']), None)
                else:
                    events[(cid, row['id'])] = self.event(row, calendar)

        # Only what's within the window stays (a sync token reports changes anywhere)
        def inside(e):
            def when(x):
                if x.get('dateTime'):
                    return datetime.fromisoformat(x['dateTime'].replace('Z', '+00:00'))
                return datetime.fromisoformat(x['date']).replace(tzinfo=timezone.utc)
            try:
                return when(e['end']) >= now - WINDOW and when(e['start']) <= now + WINDOW
            except (KeyError, ValueError):
                return False
        kept_events = [e for e in events.values() if inside(e)]

        # Tasks: no sync tokens, so whatever was updated since last time
        lists = self.pages(TASK + '/users/@me/lists', token, maxResults=100)
        by_list = {l['id']: l for l in lists}
        tasks = {(t['listId'], t['id']): t for t in cache.get('tasks', []) if t.get('listId') in by_list}
        for lid in list(task_pos):
            if lid not in by_list:
                del task_pos[lid]
        for lid, listing in by_list.items():
            since = task_pos.get(lid)
            params = dict(maxResults=100, showCompleted='true', showHidden='true')
            if since:
                params.update(showDeleted='true', updatedMin=datetime.fromtimestamp(since - SKEW, timezone.utc).isoformat())
            else:
                for key in [k for k in tasks if k[0] == lid]:
                    del tasks[key]
            for row in self.pages(TASK + '/lists/' + urllib.parse.quote(lid, safe='') + '/tasks', token, **params):
                if row.get('deleted'):
                    tasks.pop((lid, row['id']), None)
                else:
                    tasks[(lid, row['id'])] = self.task(row, listing)
            task_pos[lid] = started
        # Publish only once everything succeeded: a failed sync keeps the last complete one
        atomic(self.cache, dict(events=kept_events, tasks=list(tasks.values()), taskLists=lists,
                                calendars=list(calendars.values()), syncedAt=started * 1000))
        atomic(self.position, dict(calendarList=list_token, calendars=cal_pos, tasks=task_pos))
        # An account signed in with your own client has no name from Online
        # Accounts: its primary calendar's id is its address
        primary = next((c['id'] for c in calendars.values() if c.get('primary')), '')
        if primary and not read(self.connection).get('identity') and read(self.meta).get('identity') != primary:
            atomic(self.meta, dict(read(self.meta), identity=primary))

    def calendar(self, cid):
        return next((c for c in read(self.cache).get('calendars', []) if c.get('id') == cid), None)

    # One change from the outbox, sent as it is. A deletion of something
    # already gone counts as done; an edit of something gone is a conflict.
    def send(self, op):
        token = self.access()
        cmd = op['cmd']
        match = {'If-Match': op['etag']} if op.get('etag') else None
        if cmd.startswith('task'):
            url = TASK + '/lists/' + quote(op['listId']) + '/tasks'
            if cmd != 'taskAdd':
                url += '/' + quote(op['id'])
            body = None if cmd == 'taskDelete' else task_body(op.get('patch', {}))
            method = {'taskAdd': 'POST', 'taskUpdate': 'PATCH', 'taskDelete': 'DELETE'}[cmd]
        elif cmd == 'eventAdd':
            url = CAL + '/calendars/' + quote(op.get('calendarId') or 'primary') + '/events/quickAdd?' + urllib.parse.urlencode({'text': op['text']})
            body, method, match = None, 'POST', None
        else:
            url = CAL + '/calendars/' + quote(op['calendarId']) + '/events'
            fields = op.get('fields', {})
            if op.get('series'):
                # Every occurrence: the series itself changes. A new time
                # moves the series by as much as this occurrence moved.
                url += '/' + quote(op['series'])
                if cmd == 'eventPatch' and 'start' in fields:
                    master = request(url, token=token)
                    shift = fields['start'] - op.get('was', fields['start'])
                    start = ms(master['start']) + shift
                    fields = dict(fields, start=start, end=start + fields.get('end', fields['start']) - fields['start'])
                match = None  # (the etag read is this occurrence's, not the series')
            elif cmd != 'eventInsert':
                url += '/' + quote(op['id'])
            body = None if cmd == 'eventDelete' else event_body(fields, patching=cmd == 'eventPatch')
            method = {'eventInsert': 'POST', 'eventPatch': 'PATCH', 'eventDelete': 'DELETE'}[cmd]
        try:
            reply = request(url, body, token=token, method=method, headers=match if cmd not in ('taskAdd', 'eventInsert') else None)
            return reply if isinstance(reply, dict) else {}
        except Gone:
            if not cmd.endswith('Delete'):
                raise Conflict('It was deleted on Google.') from None
        except HttpError as exc:
            if exc.code != 404:
                raise
            if not cmd.endswith('Delete'):
                raise Conflict('It was deleted on Google.') from None
        return {}

    def list_tasks(self, token, listing):
        rows = self.pages(TASK + '/lists/' + urllib.parse.quote(listing['id'], safe='') + '/tasks', token,
                          maxResults=100, showCompleted='true', showHidden='true', showDeleted='false')
        return [self.task(row, listing) for row in rows if not row.get('deleted')]

    # After a change to one task list, only that list is fetched again (a full
    # sync reads every calendar for two years). Without a cache to patch, the
    # full sync it is.
    def sync_list(self, list_id):
        cache = read(self.cache)
        listing = next((l for l in cache.get('taskLists', []) if l.get('id') == list_id), None)
        if listing is None:
            return self.sync()
        tasks = [t for t in cache.get('tasks', []) if t.get('listId') != list_id]
        tasks.extend(self.list_tasks(self.access(), listing))
        atomic(self.cache, dict(cache, tasks=tasks))

    def run(self, req):
        cmd = req.get('cmd', 'status')
        if cmd in ('connectDesktop', 'signin'):
            if self.tokens.exists():
                raise ValueError('Disconnect the current account before connecting another one.')
            identity = ''
            if cmd == 'signin':
                # A Google account already in Online Accounts is used as it
                # is; otherwise one is added (browser sign-in)
                existing = self.broker.accounts()
                if existing:
                    selected, identity = existing[0]['id'], existing[0]['name']
                else:
                    reply = self.signin()
                    selected, identity = reply['account'], reply.get('identity', '')
            else:
                selected = req['account']
            if next((a for a in self.broker.accounts() if a['id'] == selected and a['calendarDisabled']), None):
                self.broker.enable_calendar(selected)
            self.broker.access(selected) # Validate selection and credentials before saving.
            atomic(self.connection, {'account': selected, 'identity': identity})
            self.cache.unlink(missing_ok=True)
            self.position.unlink(missing_ok=True)
            self.sync(full=True)
        elif cmd == 'configure':
            client = read(Path(os.path.expanduser(req['path']))).get('installed', {})
            if not client.get('client_id') or not client.get('client_secret'):
                raise ValueError('Choose a Google OAuth Desktop app client JSON, not a web or service account key.')
            if self.tokens.exists():
                raise ValueError('Disconnect before replacing the OAuth client.')
            atomic(self.credentials, {k: client[k] for k in ('client_id', 'client_secret')})
        elif cmd == 'connect':
            if self.connection.exists():
                raise ValueError('Disconnect the desktop account first.')
            self.connect()
            self.sync(full=True)
        elif cmd == 'sync':
            self.sync(full=bool(req.get('full')))
        elif cmd == 'disconnect':
            tokens = read(self.tokens)
            if tokens.get('refresh_token'):
                request('https://oauth2.googleapis.com/revoke', {'token': tokens['refresh_token']}, form=True)
            self.tokens.unlink(missing_ok=True)
            self.connection.unlink(missing_ok=True)
            self.cache.unlink(missing_ok=True)
            self.position.unlink(missing_ok=True)
        elif cmd == 'forget':
            self.tokens.unlink(missing_ok=True)
            self.connection.unlink(missing_ok=True)
            self.cache.unlink(missing_ok=True)
            self.position.unlink(missing_ok=True)
        elif cmd in ('taskAdd', 'taskUpdate', 'taskDelete'):
            token = self.access()
            url = TASK + '/lists/' + urllib.parse.quote(req['listId'], safe='') + '/tasks'
            body = task_body(req.get('patch', {}))
            if cmd != 'taskAdd':
                url += '/' + urllib.parse.quote(req['id'], safe='')
            request(url, None if cmd == 'taskDelete' else body, token=token,
                    method={'taskAdd': 'POST', 'taskUpdate': 'PATCH', 'taskDelete': 'DELETE'}[cmd])
            emit({'mutationComplete': True})
            self.sync_list(req['listId'])
        elif cmd == 'eventAdd':
            text = req['text'].strip()
            if not text:
                raise ValueError('Enter an event description.')
            request(CAL + '/calendars/primary/events/quickAdd?' + urllib.parse.urlencode({'text': text}),
                    token=self.access(), method='POST')
            emit({'mutationComplete': True})
            self.sync()
        elif cmd != 'status':
            raise ValueError('Unknown Google account command.')
        return self.status()


class Hub:
    """Every connected account (accounts/<id>/), the shared custom OAuth
    client (client.json) and the outbox of changes still to send."""
    LEGACY = ('tokens.json', 'connection.json', 'cache.json', 'sync.json')

    def __init__(self, root):
        self.root = Path(root)
        self.client = self.root / 'client.json'
        self.home = self.root / 'accounts'
        self.outbox = self.root / 'outbox.json'
        self.versions = self.root / 'versions.json'
        self.broker = OnlineAccounts()
        self.migrate()

    # One account kept straight in the root (before several were possible)
    # moves into accounts/ as it is
    def migrate(self):
        if not any((self.root / n).exists() for n in self.LEGACY[:2]):
            return
        target = self.home / secrets.token_hex(3)
        target.mkdir(parents=True, mode=0o700)
        os.chmod(self.home, 0o700)
        for name in self.LEGACY:
            if (self.root / name).exists():
                os.replace(self.root / name, target / name)

    def account(self, aid, must=True):
        found = next((a for a in self.accounts() if a.id == aid), None)
        if found is None and must:
            raise ValueError('That Google account isn\'t connected any more. Sync, then try again.')
        return found

    def accounts(self):
        if not self.home.is_dir():
            return []
        found = [Account(d, self.client, self.broker) for d in self.home.iterdir() if d.is_dir()]
        found = [a for a in found if a.linked()]
        return sorted(found, key=lambda a: (read(a.meta).get('added', 0), a.id))

    def fresh(self):
        directory = self.home / secrets.token_hex(3)
        directory.mkdir(parents=True, mode=0o700)
        os.chmod(self.home, 0o700)
        account = Account(directory, self.client, self.broker)
        atomic(account.meta, {'added': time.time()})
        return account

    # ── The outbox ──────────────────────────────────────────────────────
    def ops(self):
        return read(self.outbox).get('ops', [])

    def save(self, ops):
        atomic(self.outbox, {'ops': ops})

    WRITES = ('taskAdd', 'taskUpdate', 'taskDelete', 'eventAdd', 'eventInsert', 'eventPatch', 'eventDelete')

    def enqueue(self, req):
        accounts = self.accounts()
        if not accounts:
            raise ValueError('Connect your Google account first.')
        aid = req.get('account') or accounts[0].id
        account = self.account(aid)
        cmd = req['cmd']
        op = {k: v for k, v in req.items() if k in ('cmd', 'listId', 'id', 'calendarId', 'patch', 'fields', 'text', 'etag', 'title', 'series', 'was')}
        op.update(account=aid, op=secrets.token_hex(4), at=time.time())
        # Checked now, not when it's sent: a mistake shows while you're there
        if cmd.startswith('event') and cmd != 'eventAdd':
            calendar = account.calendar(op['calendarId'])
            if calendar is not None and not calendar.get('writable', True):
                raise ValueError('This calendar is read-only for your account. Change events in a calendar you own, or ask its owner for edit access.')
            if cmd != 'eventDelete':
                event_body(op.get('fields', {}), patching=cmd == 'eventPatch')
        elif cmd in ('taskAdd', 'taskUpdate'):
            task_body(op.get('patch', {}))
        elif cmd == 'eventAdd' and not str(op.get('text', '')).strip():
            raise ValueError('Enter an event description.')
        ops = self.ops()
        # A change to something not sent yet folds into it
        if str(op.get('id', '')).startswith('pending-'):
            pid = op['id'][len('pending-'):]
            first = next((o for o in ops if o['op'] == pid), None)
            if first is None:
                raise ValueError('That item is already on Google. Sync, then try again.')
            if cmd.endswith('Delete'):
                ops = [o for o in ops if o['op'] != pid]
            elif cmd == 'taskUpdate':
                first['patch'] = dict(first.get('patch', {}), **op.get('patch', {}))
            else:
                first['fields'] = dict(first.get('fields', {}), **op.get('fields', {}))
            self.save(ops)
            return
        self.save(ops + [op])

    # A second change made against the version the first one replaced (two
    # quick edits) isn't a conflict: it goes against the version the first
    # one made. Kept for the last items changed here.
    def current(self, op):
        record = read(self.versions).get(op['account'] + '|' + str(op.get('id', '')))
        if record and op.get('etag') and op['etag'] in record['from']:
            op = dict(op, etag=record['to'])
            if not op['etag']:
                del op['etag']
        return op

    def replaced(self, op, reply):
        versions = read(self.versions)
        key = op['account'] + '|' + str(op['id'])
        record = versions.pop(key, {'from': []})
        record['from'] = (record['from'] + [op.get('etag', ''), record.get('to', '')])[-20:]
        record['to'] = reply.get('etag', '')
        versions[key] = record
        atomic(self.versions, dict(list(versions.items())[-200:]))

    @staticmethod
    def describe(op):
        """What a change was, in words (for the 'couldn't be saved' note)."""
        f = dict(op.get('patch', {}), **op.get('fields', {}))
        parts = []
        if f.get('title') or f.get('text'):
            parts.append(f.get('title') or f.get('text'))
        if 'start' in f:
            fmt = '%a %d %b' if f.get('allDay') else '%a %d %b %H:%M'
            parts.append('from ' + datetime.fromtimestamp(f['start'] / 1000).strftime(fmt)
                         + ('' if 'end' not in f else ' to ' + datetime.fromtimestamp(f['end'] / 1000).strftime(fmt)))
        if 'date' in f:
            parts.append('due ' + f['date'] if f['date'] else 'no due date')
        if 'done' in f:
            parts.append('done' if f['done'] else 'not done')
        for key in ('location', 'notes'):
            if f.get(key):
                parts.append(f[key])
        if op['cmd'].endswith('Delete'):
            parts.append('deleted')
        return ' · '.join(parts) or op['cmd']

    # Send what's waiting, in order. Can't reach Google (or the sign-in
    # expired): stop, keep the rest for later. Google refused one (it changed
    # or was deleted there, or the change is invalid): drop it, keep Google's
    # version, and say so once. Afterwards, only what was touched is fetched.
    def flush(self):
        ops = self.ops()
        touched = {}
        sent = False
        while ops:
            op = ops[0]
            account = self.account(op['account'], must=False)
            if account is not None:
                try:
                    op = self.current(op)
                    reply = account.send(op)
                    if op.get('id') and not op.get('series') and op['cmd'] in ('taskUpdate', 'eventPatch'):
                        self.replaced(op, reply)
                    sent = True
                    kinds = touched.setdefault(account.id, set())
                    kinds.add(('tasks', op['listId']) if op['cmd'].startswith('task') else ('events', ''))
                except (Offline, OSError):
                    self.save(ops)
                    raise
                except Conflict as exc:
                    emit({'conflict': {'title': op.get('title') or Hub.describe(op), 'what': Hub.describe(op),
                                       'reason': 'deleted' if 'deleted' in str(exc) else 'changed'}})
                except HttpError as exc:
                    if exc.code == 401:
                        self.save(ops)
                        raise
                    emit({'conflict': {'title': op.get('title') or Hub.describe(op), 'what': Hub.describe(op), 'reason': str(exc)}})
            ops = ops[1:]
            self.save(ops)
        if sent:
            emit({'mutationComplete': True})
        for aid, kinds in touched.items():
            account = self.account(aid)
            if ('events', '') in kinds:
                account.sync()
            else:
                for _kind, list_id in kinds:
                    account.sync_list(list_id)

    # ── What the shell sees ─────────────────────────────────────────────
    def status(self):
        result = dict(accounts=[], events=[], tasks=[], calendars=[], taskLists=[])
        used = set()
        oldest = None
        for account in self.accounts():
            st = account.status()
            aid = account.id
            used.add(account.goa)
            result['accounts'].append(dict(id=aid, identity=st.get('identity', ''), backend=st.get('backend', ''),
                                           syncedAt=st.get('syncedAt', 0)))
            for key in ('events', 'tasks', 'calendars', 'taskLists'):
                result[key].extend(dict(item, account=aid) for item in st.get(key, []) if isinstance(item, dict))
            synced = st.get('syncedAt', 0)
            oldest = synced if oldest is None else min(oldest, synced)
        ops = self.ops()
        self.overlay(result, ops)
        goa = self.broker.accounts()
        first = result['accounts'][0] if result['accounts'] else {}
        result.update(connected=bool(result['accounts']), configured=self.client.exists(),
                      identity=first.get('identity', ''), backend=first.get('backend', ''),
                      syncedAt=oldest or 0, outbox=len(ops),
                      goaAccounts=[dict(a, used=a['id'] in used) for a in goa],
                      signin=bool(Account.signin_helper()),
                      goaSettings=shutil.which('gnome-online-accounts-gtk') or shutil.which('gnome-control-center') or '')
        return result

    # Changes not sent yet, shown as if they were (so they survive a restart
    # before they reach Google)
    @staticmethod
    def overlay(result, ops):
        events, tasks = result['events'], result['tasks']
        calendars = {(c['account'], c['id']): c for c in result['calendars']}
        lists = {(l['account'], l['id']): l for l in result['taskLists']}

        def find(rows, op, key):
            return next((r for r in rows if r.get('account') == op['account'] and r.get(key) == op.get(key) and r.get('id') == op.get('id')), None)

        def task_fields(patch):
            return {k: v for k, v in patch.items() if k in ('text', 'done', 'date', 'notes')}

        def event_fields(fields):
            out = {k: v for k, v in (('title', fields.get('title')), ('location', fields.get('location')),
                                     ('notes', fields.get('notes'))) if v is not None}
            if 'start' in fields:
                all_day = bool(fields.get('allDay'))
                out['start'] = when(fields['start'], all_day)
                out['end'] = when(fields.get('end', fields['start']), all_day)
            return out

        for op in ops:
            cmd = op['cmd']
            if cmd == 'taskAdd':
                listing = lists.get((op['account'], op['listId']), {})
                tasks.append(dict(dict(text='', done=False, date='', notes=''), **task_fields(op.get('patch', {})),
                                  id='pending-' + op['op'], listId=op['listId'], listName=listing.get('title', ''),
                                  account=op['account'], pending=True))
            elif cmd == 'taskUpdate':
                row = find(tasks, op, 'listId')
                if row:
                    row.update(task_fields(op.get('patch', {})), pending=True)
            elif cmd == 'taskDelete':
                row = find(tasks, op, 'listId')
                if row:
                    tasks.remove(row)
            elif cmd == 'eventInsert':
                calendar = calendars.get((op['account'], op['calendarId']), {})
                events.append(dict(dict(title='', location='', notes=''), **event_fields(op.get('fields', {})),
                                   id='pending-' + op['op'], calendarId=op['calendarId'], account=op['account'],
                                   color=calendar.get('color', ''), calendar=calendar.get('title', 'Google'),
                                   writable=True, recurring=False, etag='', pending=True))
            elif cmd in ('eventPatch', 'eventDelete') and op.get('series'):
                rows = [r for r in events if r.get('account') == op['account'] and r.get('calendarId') == op['calendarId']
                        and r.get('series') == op['series']]
                fields = op.get('fields', {})
                for row in rows:
                    if cmd == 'eventDelete':
                        events.remove(row)
                        continue
                    shown = {k: v for k, v in fields.items() if k not in ('start', 'end', 'allDay')}
                    if 'start' in fields:
                        start = ms(row['start']) + fields['start'] - op.get('was', fields['start'])
                        shown.update(start=start, end=start + fields.get('end', fields['start']) - fields['start'],
                                     allDay=fields.get('allDay'))
                    row.update(event_fields(shown), pending=True)
            elif cmd == 'eventPatch':
                row = find(events, op, 'calendarId')
                if row:
                    row.update(event_fields(op.get('fields', {})), pending=True)
            elif cmd == 'eventDelete':
                row = find(events, op, 'calendarId')
                if row:
                    events.remove(row)

    # ── Requests ────────────────────────────────────────────────────────
    def link_goa(self, selected, identity=''):
        if selected in {a.goa for a in self.accounts()}:
            raise ValueError('That Google account is already connected.')
        if next((a for a in self.broker.accounts() if a['id'] == selected and a['calendarDisabled']), None):
            self.broker.enable_calendar(selected)
        self.broker.access(selected)  # Validate selection and credentials before saving.
        account = self.fresh()
        try:
            atomic(account.connection, {'account': selected, 'identity': identity})
            account.sync(full=True)
        except Exception:
            shutil.rmtree(account.directory, ignore_errors=True)
            raise
        return account

    def run(self, req):
        cmd = req.get('cmd', 'status')
        if cmd == 'signin':
            # An account in Online Accounts not connected yet is used as it
            # is; otherwise one is added (browser sign-in)
            used = {a.goa for a in self.accounts()}
            spare = [a for a in self.broker.accounts() if a['id'] not in used]
            if spare:
                self.link_goa(spare[0]['id'], spare[0]['name'])
            else:
                reply = Account(self.root, self.client, self.broker).signin()
                self.link_goa(reply['account'], reply.get('identity', ''))
        elif cmd == 'connectDesktop':
            chosen = next((a for a in self.broker.accounts() if a['id'] == req['account']), None)
            self.link_goa(req['account'], chosen['name'] if chosen else '')
        elif cmd == 'configure':
            client = read(Path(os.path.expanduser(req['path']))).get('installed', {})
            if not client.get('client_id') or not client.get('client_secret'):
                raise ValueError('Choose a Google OAuth Desktop app client JSON, not a web or service account key.')
            if any(a.tokens.exists() for a in self.accounts()):
                raise ValueError('Disconnect the accounts using your client before replacing it.')
            atomic(self.client, {k: client[k] for k in ('client_id', 'client_secret')})
        elif cmd == 'connect':
            account = self.fresh()
            try:
                account.connect()
                account.sync(full=True)
            except Exception:
                shutil.rmtree(account.directory, ignore_errors=True)
                raise
        elif cmd in ('sync', 'flush'):
            try:
                self.flush()
            except (Offline, OSError):
                if cmd == 'flush':
                    raise
            if cmd == 'sync':
                targets = [self.account(req['account'])] if req.get('account') else self.accounts()
                failed = None
                for account in targets:
                    try:
                        account.sync(full=bool(req.get('full')))
                    except (RuntimeError, OSError, ValueError) as exc:
                        failed = failed or exc
                if failed:
                    raise failed
        elif cmd in ('disconnect', 'forget'):
            accounts = self.accounts()
            account = self.account(req['account']) if req.get('account') else (accounts[0] if len(accounts) == 1 else None)
            if account is None:
                raise ValueError('Choose which account to disconnect.')
            account.run({'cmd': cmd})
            self.save([o for o in self.ops() if o['account'] != account.id])
            shutil.rmtree(account.directory, ignore_errors=True)
        elif cmd in self.WRITES:
            self.enqueue(req)
            self.flush()
        elif cmd != 'status':
            raise ValueError('Unknown Google account command.')
        return self.status()


def main():
    account = Hub(sys.argv[1])
    try:
        emit({'data': account.run(json.loads(sys.stdin.readline()))})
    except (OSError, ValueError, KeyError, RuntimeError) as exc:
        # Do not print paths or raw network errors that could carry account details.
        message = str(exc) if isinstance(exc, (ValueError, RuntimeError)) else 'Google connection or local storage failed. Check your connection and try again.'
        # (offline: the change is kept in the outbox; the shell tries again later)
        emit({'error': message, 'offline': isinstance(exc, (Offline, OSError))})
        try:
            emit({'data': account.status()})
        except (OSError, ValueError):
            pass
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
