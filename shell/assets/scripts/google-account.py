#!/usr/bin/env python3
"""Google desktop OAuth + Calendar/Tasks bridge. JSON request on stdin, JSONL replies.
No third-party dependencies. Tokens and cached account data stay in a private directory.
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
                    and self.name + '.OAuth2Based' in interfaces
                    and not account.get('CalendarDisabled', True)):
                accounts.append({'id': path, 'name': account.get('PresentationIdentity', account.get('Identity', 'Google'))})
        return accounts

    def access(self, path):
        if path not in [account['id'] for account in self.accounts()]:
            raise RuntimeError('The selected Google account is missing or Calendars is disabled in Online Accounts.')
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
        self.broker = OnlineAccounts()

    def status(self):
        connection = read(self.connection)
        return dict(read(self.cache), connected=self.tokens.exists() or bool(connection.get('account')),
                    configured=self.credentials.exists(), backend='goa' if connection.get('account') else 'direct',
                    accountId=connection.get('account', ''),
                    goaAccounts=self.broker.accounts(),
                    goaSettings=shutil.which('gnome-online-accounts-gtk') or shutil.which('gnome-control-center') or '')

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
        items = []
        while True:
            data = request(url + '?' + urllib.parse.urlencode(params), token=token)
            items.extend(data.get('items', []))
            if not data.get('nextPageToken'):
                return items
            params['pageToken'] = data['nextPageToken']

    def sync(self):
        token = self.access()
        now = datetime.now(timezone.utc)
        calendars = self.pages(CAL + '/users/me/calendarList', token)
        events = []
        for calendar in calendars:
            if calendar.get('deleted') or calendar.get('hidden'):
                continue
            path = CAL + '/calendars/' + urllib.parse.quote(calendar['id'], safe='') + '/events'
            rows = self.pages(path, token, singleEvents='true', maxResults=2500,
                              timeMin=(now - timedelta(days=366)).isoformat(),
                              timeMax=(now + timedelta(days=366)).isoformat())
            for row in rows:
                if row.get('status') == 'cancelled' or not row.get('start') or not row.get('end'):
                    continue
                events.append(dict(id=row['id'], calendarId=calendar['id'], title=row.get('summary', '(Untitled)'),
                                   start=row['start'], end=row['end'], location=row.get('location', ''),
                                   notes=row.get('description', ''), color=calendar.get('backgroundColor', ''),
                                   calendar=calendar.get('summary', 'Google'), url=row.get('htmlLink', '')))
        lists = self.pages(TASK + '/users/@me/lists', token, maxResults=100)
        tasks = []
        for listing in lists:
            tasks.extend(self.list_tasks(token, listing))
        # Publish only after every page succeeds; errors retain the last complete snapshot.
        atomic(self.cache, dict(events=events, tasks=tasks, taskLists=lists, syncedAt=time.time() * 1000))

    def list_tasks(self, token, listing):
        rows = self.pages(TASK + '/lists/' + urllib.parse.quote(listing['id'], safe='') + '/tasks', token,
                          maxResults=100, showCompleted='true', showHidden='true', showDeleted='false')
        return [dict(id=row['id'], listId=listing['id'], listName=listing['title'],
                     text=row.get('title', ''), done=row.get('status') == 'completed',
                     date=row.get('due', '')[:10], notes=row.get('notes', '')) for row in rows if not row.get('deleted')]

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
        if cmd == 'connectDesktop':
            if self.tokens.exists():
                raise ValueError('Disconnect the current account before selecting another one.')
            selected = req['account']
            self.broker.access(selected) # Validate selection and credentials before saving.
            atomic(self.connection, {'account': selected})
            self.cache.unlink(missing_ok=True)
            self.sync()
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
            self.sync()
        elif cmd == 'sync':
            self.sync()
        elif cmd == 'disconnect':
            tokens = read(self.tokens)
            if tokens.get('refresh_token'):
                request('https://oauth2.googleapis.com/revoke', {'token': tokens['refresh_token']}, form=True)
            self.tokens.unlink(missing_ok=True)
            self.connection.unlink(missing_ok=True)
            self.cache.unlink(missing_ok=True)
        elif cmd == 'forget':
            self.tokens.unlink(missing_ok=True)
            self.connection.unlink(missing_ok=True)
            self.cache.unlink(missing_ok=True)
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
