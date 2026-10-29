#!/usr/bin/env python3
"""Google Calendar and Tasks for iRice: one request per run, JSON on stdin,
JSON lines out (Google.qml).

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


class Gone(RuntimeError):
    """A sync token Google no longer accepts (HTTP 410): sync that part in full."""


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


def request(url, data=None, token=None, method=None, form=False):
    headers = {}
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
        # Never include request URLs, tokens or the raw server body in UI errors.
        hints = {400: 'Invalid request or expired sign-in. Reconnect if needed.',
                 401: 'Google sign-in expired. Connect again.',
                 403: 'Access denied. Check Online Accounts permissions. For a custom client, enable Calendar and Tasks APIs.',
                 404: 'This Google item no longer exists.',
                 409: 'The item changed on Google. Sync and try again.',
                 429: 'Google rate limit reached. Try again later.'}
        raise RuntimeError(hints.get(exc.code, 'Google request failed (HTTP %s).' % exc.code)) from None


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
    def __init__(self, directory):
        self.directory = Path(directory)
        self.credentials = self.directory / 'client.json'
        self.tokens = self.directory / 'tokens.json'
        self.cache = self.directory / 'cache.json'
        self.connection = self.directory / 'connection.json'
        self.position = self.directory / 'sync.json'  # sync tokens and times; never shown
        self.broker = OnlineAccounts()

    def status(self):
        connection = read(self.connection)
        accounts = self.broker.accounts()
        identity = connection.get('identity') or next((a['name'] for a in accounts if a['id'] == connection.get('account')), '')
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
                    calendar=calendar.get('title', 'Google'), url=row.get('htmlLink', ''))

    @staticmethod
    def task(row, listing):
        return dict(id=row['id'], listId=listing['id'], listName=listing['title'],
                    text=row.get('title', ''), done=row.get('status') == 'completed',
                    date=row.get('due', '')[:10], notes=row.get('notes', ''))

    def sync(self, full=False):
        """Bring the cache up to date: only what changed since the last sync."""
        token = self.access()
        started = time.time()
        now = datetime.now(timezone.utc)
        cache = {} if full else read(self.cache)
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
                                            color=row.get('backgroundColor', ''), primary=bool(row.get('primary')))

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
                    e.update(color=calendar['color'], calendar=calendar['title'])
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
            body = {}
            patch = req.get('patch', {})
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


def main():
    account = Account(sys.argv[1])
    try:
        emit({'data': account.run(json.loads(sys.stdin.readline()))})
    except (OSError, ValueError, KeyError, RuntimeError) as exc:
        # Do not print paths or raw network errors that could carry account details.
        message = str(exc) if isinstance(exc, (ValueError, RuntimeError)) else 'Google connection or local storage failed. Check your connection and try again.'
        emit({'error': message})
        try:
            emit({'data': account.status()})
        except (OSError, ValueError):
            pass
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
