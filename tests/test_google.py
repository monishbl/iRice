"""The Google account helper (google-account.py): sign-in, storage and sync,
with the network and accounts stubbed out; no real account is used."""
import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import threading
import urllib.parse
import urllib.request
import urllib.error
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1] / 'shell/assets/scripts'


def load(name):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / (name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


google = load('google-account')


class FakeGoogle:
    """Calendar and Tasks as far as the helper uses them: sync tokens that
    report only what changed since they were handed out (or 410 when
    expired), updatedMin and deletions for tasks. Counts the requests."""

    def __init__(self):
        from collections import defaultdict
        self.calendars, self.lists = {}, {}
        self.events, self.tasks = defaultdict(dict), defaultdict(dict)
        self.version = 0  # bumped on every change; a sync token is the version it was given at
        self.changed = {}  # (kind, id) -> version
        self.expired = set()
        self.requests = []
        self.writes = []

    @staticmethod
    def day(offset):
        from datetime import date, timedelta
        return (date.today() + timedelta(days=offset)).isoformat()

    def change(self, kind, key):
        self.version += 1
        self.changed[(kind, key)] = self.version

    def serving(self, account):
        return _Serving(self, account)

    offline = False

    def etag(self):
        return '"%d"' % (self.version + 1)

    # Writes: create, patch (merging, with If-Match) and delete
    def write(self, parts, data, method, headers, q):
        match = (headers or {}).get('If-Match')
        if parts[-1] == 'quickAdd':
            cid = parts[-3]
            self.change('ev', cid + '/q%d' % self.version)
            eid = 'q%d' % self.version
            self.events[cid][eid] = {'id': eid, 'summary': q['text'], 'etag': self.etag(),
                                     'start': {'date': self.day(1)}, 'end': {'date': self.day(2)}}
            return dict(self.events[cid][eid])
        events = 'events' in parts
        if events:
            i = parts.index('events')
            store, kind, owner = self.events[parts[i - 1]], 'ev', parts[i - 1]
            item = parts[i + 1] if len(parts) > i + 1 else None
            if self.calendars.get(owner, {}).get('accessRole') == 'reader':
                raise google.HttpError('Access denied.', 403)
        else:
            i = len(parts) - 1 - parts[::-1].index('tasks')
            if i == len(parts) - 1 and method != 'POST':
                i = len(parts) - 2 - parts[-2::-1].index('tasks')
            store, kind, owner = self.tasks[parts[i - 1]], 'task', parts[i - 1]
            item = parts[i + 1] if len(parts) > i + 1 else None
        if method == 'POST':
            self.version += 1
            item = ('e' if events else 't') + str(self.version)
            row = dict(data, id=item, etag=self.etag(), _updated=google.time.time())
            store[item] = row
            self.change(kind, owner + '/' + item)
            return dict(row)
        row = store.get(item)
        if row is None or row.get('status') == 'cancelled' or row.get('deleted'):
            raise google.HttpError('This Google item no longer exists.', 404)
        if match and match != row.get('etag'):
            raise google.Conflict()
        self.writes.append((method, kind, owner, item))
        if method == 'DELETE':
            if events:
                store[item] = {'id': item, 'status': 'cancelled'}
            else:
                store[item] = {'id': item, 'deleted': True, '_updated': google.time.time()}
        else:
            for key, value in data.items():
                if isinstance(value, dict) and isinstance(row.get(key), dict):
                    merged = dict(row[key], **value)
                    row[key] = {k: v for k, v in merged.items() if v is not None}
                elif value is None:
                    row.pop(key, None)
                else:
                    row[key] = value
            row['_updated'] = google.time.time()
            row['etag'] = self.etag()
        self.change(kind, owner + '/' + item)
        return {} if method == 'DELETE' else {k: v for k, v in row.items() if k != '_updated'}

    def __call__(self, url, data=None, token=None, method=None, form=False, headers=None):
        if self.offline:
            raise google.Offline("Can't reach Google.")
        self.requests.append(url)
        u = urllib.parse.urlparse(url)
        q = {k: v[0] for k, v in urllib.parse.parse_qs(u.query).items()}
        parts = [urllib.parse.unquote(p) for p in u.path.split('/') if p]
        if method in ('POST', 'PATCH', 'DELETE'):
            return self.write(parts, data or {}, method, headers, q)
        since = q.get('syncToken')
        if since and since in self.expired:
            raise google.Gone('expired')
        fresh = str(self.version)
        if parts[-2:] == ['me', 'calendarList']:
            rows = [dict(c) for c in self.calendars.values()]
            if since:
                rows = [r for r in rows if self.changed.get(('cal', r['id']), 0) > int(since)]
            return {'items': rows, 'nextSyncToken': fresh}
        if len(parts) > 2 and parts[-2] == 'events':
            return dict(self.events[parts[-3]][parts[-1]])
        if parts[-1] == 'events':
            cid = parts[-2]
            rows = [{k: v for k, v in r.items() if k != '_updated'} for r in self.events[cid].values()]
            if since:
                rows = [dict(r) for r in rows if self.changed.get(('ev', cid + '/' + r['id']), 0) > int(since)]
            else:
                rows = [r for r in rows if r.get('status') != 'cancelled']
            return {'items': rows, 'nextSyncToken': fresh}
        if parts[-2:] == ['@me', 'lists']:
            return {'items': list(self.lists.values())}
        if parts[-1] == 'tasks':
            lid = parts[-2]
            rows = list(self.tasks[lid].values())
            if 'updatedMin' in q:
                after = google.datetime.fromisoformat(q['updatedMin']).timestamp()
                rows = [r for r in rows if r.get('_updated', 0) >= after]
            if q.get('showDeleted') != 'true':
                rows = [r for r in rows if not r.get('deleted')]
            return {'items': [{k: v for k, v in r.items() if k != '_updated'} for r in rows]}
        raise AssertionError('unexpected request ' + url)


class _Serving:
    def __init__(self, api, account):
        self.api, self.account = api, account

    def __enter__(self):
        self.patches = [patch.object(google, 'request', side_effect=self.api),
                        patch.object(self.account, 'access', return_value='token')]
        for p in self.patches:
            p.start()

    def __exit__(self, *exc):
        for p in self.patches:
            p.stop()


class GoogleTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.account = google.Account(Path(self.temp.name) / 'account')
        broker_accounts = patch.object(self.account.broker, 'accounts', return_value=[])
        broker_accounts.start()
        self.addCleanup(broker_accounts.stop)

    def test_desktop_account_uses_broker_without_storing_tokens(self):
        account = '/org/gnome/OnlineAccounts/Accounts/123'
        with patch.object(self.account.broker, 'access', return_value='broker-token') as access, patch.object(self.account, 'sync'):
            result = self.account.run({'cmd': 'connectDesktop', 'account': account})
            self.assertEqual(self.account.access(), 'broker-token')
            access.assert_called_with(account)
        self.assertTrue(result['connected'])
        self.assertEqual(result['backend'], 'goa')
        self.assertFalse(self.account.tokens.exists())
        with patch.object(google, 'request') as revoke:
            self.account.run({'cmd': 'disconnect'})
            revoke.assert_not_called() # shared desktop account must survive
        self.assertFalse(self.account.connection.exists())

    def test_broker_lists_google_accounts_and_marks_calendars_off(self):
        broker = google.OnlineAccounts()
        account_iface = broker.name + '.Account'
        oauth_iface = broker.name + '.OAuth2Based'
        def account(provider='google', disabled=False):
            return {account_iface: {'ProviderType': provider, 'CalendarDisabled': disabled, 'PresentationIdentity': 'Person'}, oauth_iface: {}}
        objects = {broker.prefix + '1': account(), broker.prefix + '2': account(disabled=True), broker.prefix + '3': account('other')}
        with patch.object(broker, 'call', return_value=(objects,)):
            # Calendars switched off is listed (connecting turns them on); other providers aren't
            self.assertEqual(broker.accounts(), [{'id': broker.prefix + '1', 'name': 'Person', 'calendarDisabled': False},
                                                 {'id': broker.prefix + '2', 'name': 'Person', 'calendarDisabled': True}])
        with patch.object(broker, 'accounts', return_value=[]), patch.object(broker, 'call') as call:
            with self.assertRaises(RuntimeError):
                broker.access('/not/an/account')
            call.assert_not_called()

    def test_credentials_are_private_and_require_desktop_client(self):
        client = Path(self.temp.name) / 'client.json'
        client.write_text(json.dumps({'web': {'client_id': 'wrong'}}))
        with self.assertRaises(ValueError):
            self.account.run({'cmd': 'configure', 'path': str(client)})
        client.write_text(json.dumps({'installed': {'client_id': 'desktop', 'client_secret': 'secret'}}))
        result = self.account.run({'cmd': 'configure', 'path': str(client)})
        self.assertTrue(result['configured'])
        self.assertEqual(self.account.credentials.stat().st_mode & 0o777, 0o600)
        self.assertEqual(self.account.directory.stat().st_mode & 0o777, 0o700)

    def test_oauth_loopback_checks_state_and_uses_pkce(self):
        google.atomic(self.account.credentials, {'client_id': 'desktop', 'client_secret': 'secret'})
        threads, auth = [], {}
        failures = []
        def emitted(message):
            if 'authUrl' not in message:
                return
            auth.update(urllib.parse.parse_qs(urllib.parse.urlparse(message['authUrl']).query))
            def callback():
                try:
                    target = auth['redirect_uri'][0]
                    with self.assertRaises(urllib.error.HTTPError) as rejected:
                        urllib.request.urlopen(target + '?code=bad&state=wrong', timeout=3)
                    rejected.exception.close()
                    url = target + '?' + urllib.parse.urlencode({'code': 'accepted', 'state': auth['state'][0]})
                    with urllib.request.urlopen(url, timeout=3) as response:
                        self.assertEqual(response.status, 200)
                except Exception as exc:
                    failures.append(exc)
            thread = threading.Thread(target=callback)
            threads.append(thread)
            thread.start()
        with patch.object(google, 'emit', side_effect=emitted), patch.object(google, 'request', return_value={'access_token': 'access', 'refresh_token': 'refresh'}) as exchange:
            self.account.connect()
        for thread in threads:
            thread.join()
        self.assertEqual(failures, [])
        body = exchange.call_args.args[1]
        self.assertEqual(body['code'], 'accepted')
        challenge = google.base64.urlsafe_b64encode(google.hashlib.sha256(body['code_verifier'].encode()).digest()).rstrip(b'=').decode()
        self.assertEqual(auth['code_challenge'], [challenge])
        self.assertEqual(auth['code_challenge_method'], ['S256'])
        self.assertNotIn('client_secret', auth)
        self.assertEqual(self.account.tokens.stat().st_mode & 0o777, 0o600)

    def test_pagination_preserves_query(self):
        with patch.object(google, 'request', side_effect=[{'items': [1], 'nextPageToken': 'a/b'}, {'items': [2]}]) as req:
            self.assertEqual(self.account.pages('https://example.invalid', 'token', showHidden='true'), [1, 2])
            self.assertIn('showHidden=true', req.call_args.args[0])
            self.assertIn('pageToken=a%2Fb', req.call_args.args[0])

    def test_refresh_preserves_refresh_token(self):
        google.atomic(self.account.credentials, {'client_id': 'desktop'})
        google.atomic(self.account.tokens, {'refresh_token': 'refresh', 'expires_at': 0})
        with patch.object(google, 'request', return_value={'access_token': 'new', 'expires_in': 3600}):
            self.assertEqual(self.account.access(), 'new')
        self.assertEqual(google.read(self.account.tokens)['refresh_token'], 'refresh')

    def test_failed_sync_keeps_complete_cache(self):
        google.atomic(self.account.cache, {'events': ['old'], 'tasks': ['old']})
        with patch.object(self.account, 'access', return_value='token'), patch.object(self.account, 'pages', side_effect=RuntimeError('offline')):
            with self.assertRaises(RuntimeError):
                self.account.sync()
        self.assertEqual(google.read(self.account.cache)['events'], ['old'])

    def test_sync_all_day_events_tasks_and_timezone_preserved(self):
        api = FakeGoogle()
        api.calendars['work'] = {'id': 'work', 'summary': 'Work'}
        api.events['work']['event'] = {'id': 'event', 'start': {'date': api.day(1)}, 'end': {'date': api.day(2)}}
        api.lists['list'] = {'id': 'list', 'title': 'Inbox'}
        api.tasks['list']['task'] = {'id': 'task', 'title': 'Test', 'due': api.day(1) + 'T00:00:00Z', 'status': 'completed'}
        with api.serving(self.account):
            self.account.sync()
        data = google.read(self.account.cache)
        self.assertEqual(data['events'][0]['end'], {'date': api.day(2)})
        self.assertEqual(data['tasks'][0]['date'], api.day(1))
        self.assertTrue(data['tasks'][0]['done'])

    def test_task_patch_encodes_ids_and_clears_completion(self):
        with patch.object(self.account, 'access', return_value='token'), patch.object(self.account, 'sync'), patch.object(google, 'request') as req, contextlib.redirect_stdout(io.StringIO()):
            self.account.run({'cmd': 'taskUpdate', 'listId': 'a/b', 'id': 'c/d', 'patch': {'done': False, 'date': ''}})
            self.assertTrue(req.call_args.args[0].endswith('/a%2Fb/tasks/c%2Fd'))
            self.assertEqual(req.call_args.args[1], {'status': 'needsAction', 'completed': None, 'due': None})
            self.assertEqual(req.call_args.kwargs['method'], 'PATCH')

    def test_disconnect_revokes_before_removing_and_forget_is_offline(self):
        google.atomic(self.account.tokens, {'refresh_token': 'refresh'})
        google.atomic(self.account.cache, {'tasks': [1]})
        with patch.object(google, 'request', side_effect=RuntimeError('offline')):
            with self.assertRaises(RuntimeError):
                self.account.run({'cmd': 'disconnect'})
        self.assertTrue(self.account.tokens.exists())
        with patch.object(google, 'request') as req:
            self.account.run({'cmd': 'forget'})
            req.assert_not_called()
        self.assertFalse(self.account.tokens.exists())
        self.assertFalse(self.account.cache.exists())

    def test_a_task_change_refreshes_only_its_list(self):
        google.atomic(self.account.cache, {'events': ['kept'], 'taskLists': [{'id': 'a', 'title': 'A'}, {'id': 'b', 'title': 'B'}],
                                           'tasks': [{'id': '1', 'listId': 'a'}, {'id': '2', 'listId': 'b'}]})
        fresh = [{'id': '3', 'listId': 'a', 'listName': 'A', 'text': 'new', 'done': False, 'date': '', 'notes': ''}]
        with patch.object(self.account, 'access', return_value='token'), patch.object(self.account, 'sync') as full, \
                patch.object(self.account, 'list_tasks', return_value=fresh), patch.object(google, 'request'), contextlib.redirect_stdout(io.StringIO()):
            self.account.run({'cmd': 'taskUpdate', 'listId': 'a', 'id': '1', 'patch': {'done': True}})
            full.assert_not_called()
        data = google.read(self.account.cache)
        self.assertEqual(data['events'], ['kept'])
        self.assertEqual(sorted(t['id'] for t in data['tasks']), ['2', '3'])


class IncrementalSync(unittest.TestCase):
    """What a sync asks Google for, and what the cache holds after."""

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.account = google.Account(Path(self.temp.name) / 'account')
        self.api = api = FakeGoogle()
        api.calendars['work'] = {'id': 'work', 'summary': 'Work', 'backgroundColor': '#123456'}
        api.events['work']['a'] = {'id': 'a', 'summary': 'Standup', 'start': {'dateTime': api.day(1) + 'T09:00:00Z'}, 'end': {'dateTime': api.day(1) + 'T09:15:00Z'}}
        api.lists['inbox'] = {'id': 'inbox', 'title': 'Inbox'}
        api.tasks['inbox']['t1'] = {'id': 't1', 'title': 'Milk', 'status': 'needsAction', '_updated': 1}
        with api.serving(self.account):
            self.account.sync()
        api.requests.clear()

    def events(self):
        return {e['id']: e for e in google.read(self.account.cache)['events']}

    def tasks(self):
        return {t['id']: t for t in google.read(self.account.cache)['tasks']}

    def test_nothing_changed_asks_for_changes_only(self):
        with self.api.serving(self.account):
            self.account.sync()
        events = [u for u in self.api.requests if '/events' in u]
        self.assertTrue(all('syncToken=' in u and 'timeMin' not in u for u in events))
        self.assertTrue(all('syncToken=' in u for u in self.api.requests if 'calendarList' in u))
        self.assertTrue(all('updatedMin=' in u and 'showDeleted=true' in u for u in self.api.requests if u.endswith('/tasks') or '/tasks?' in u))
        self.assertEqual(len(self.api.requests), 4)  # calendar list, one calendar, task lists, one task list
        self.assertEqual(set(self.events()), {'a'})
        self.assertEqual(set(self.tasks()), {'t1'})

    def test_changes_and_cancellations_merge_into_the_cache(self):
        api = self.api
        api.events['work']['b'] = {'id': 'b', 'summary': 'Lunch', 'start': {'date': api.day(2)}, 'end': {'date': api.day(3)}}
        api.change('ev', 'work/b')
        api.events['work']['a'] = {'id': 'a', 'status': 'cancelled'}
        api.change('ev', 'work/a')
        with api.serving(self.account):
            self.account.sync()
        self.assertEqual(set(self.events()), {'b'})
        self.assertEqual(self.events()['b']['color'], '#123456')

    def test_an_expired_sync_token_fetches_that_calendar_in_full(self):
        api = self.api
        api.expired.add(google.read(self.account.position)['calendars']['work'])
        api.events['work']['c'] = {'id': 'c', 'summary': 'New', 'start': {'date': api.day(4)}, 'end': {'date': api.day(5)}}
        api.change('ev', 'work/c')
        with api.serving(self.account):
            self.account.sync()
        self.assertEqual(set(self.events()), {'a', 'c'})
        self.assertTrue(any('timeMin=' in u for u in api.requests if '/events' in u))
        self.assertNotIn(google.read(self.account.position)['calendars']['work'], api.expired)

    def test_tasks_updated_or_deleted_since_last_sync_are_merged(self):
        api = self.api
        now = google.time.time()
        api.tasks['inbox']['t1'] = {'id': 't1', 'deleted': True, '_updated': now}
        api.tasks['inbox']['t2'] = {'id': 't2', 'title': 'Bread', 'status': 'completed', '_updated': now}
        api.tasks['inbox']['old'] = {'id': 'old', 'title': 'Unchanged, not resent', '_updated': 1}
        with api.serving(self.account):
            self.account.sync()
        self.assertEqual(set(self.tasks()), {'t2'})
        self.assertTrue(self.tasks()['t2']['done'])

    def test_a_removed_calendar_or_task_list_drops_its_items(self):
        api = self.api
        api.calendars['work']['deleted'] = True
        api.change('cal', 'work')
        del api.lists['inbox']
        with api.serving(self.account):
            self.account.sync()
        self.assertEqual(self.events(), {})
        self.assertEqual(self.tasks(), {})
        position = google.read(self.account.position)
        self.assertEqual(position['calendars'], {})
        self.assertEqual(position['tasks'], {})

    def test_sync_position_is_private_and_a_failed_sync_keeps_it(self):
        self.assertEqual(self.account.position.stat().st_mode & 0o777, 0o600)
        before = google.read(self.account.position)
        with patch.object(google, 'request', side_effect=RuntimeError('offline')), patch.object(self.account, 'access', return_value='token'):
            with self.assertRaises(RuntimeError):
                self.account.sync()
        self.assertEqual(google.read(self.account.position), before)

    def test_signin_reuses_an_account_already_in_online_accounts(self):
        acc = {'id': google.OnlineAccounts.prefix + '7', 'name': 'me@example.com', 'calendarDisabled': True}
        with patch.object(self.account.broker, 'accounts', return_value=[acc]), \
                patch.object(self.account.broker, 'enable_calendar') as enable, \
                patch.object(self.account.broker, 'access', return_value='token'), \
                patch.object(self.account, 'signin') as browser, \
                patch.object(self.account, 'sync') as sync:
            result = self.account.run({'cmd': 'signin'})
        browser.assert_not_called()
        enable.assert_called_once_with(acc['id'])
        sync.assert_called_once_with(full=True)
        self.assertEqual(result['identity'], 'me@example.com')
        self.assertTrue(result['connected'])

    def test_signin_without_an_account_runs_the_helper(self):
        added = {'account': google.OnlineAccounts.prefix + '9', 'identity': 'new@example.com'}
        listed = [[], [{'id': added['account'], 'name': 'new@example.com', 'calendarDisabled': False}]]
        with patch.object(self.account.broker, 'accounts', side_effect=lambda: listed.pop(0) if len(listed) > 1 else listed[0]), \
                patch.object(self.account.broker, 'access', return_value='token'), \
                patch.object(self.account, 'signin', return_value=added) as browser, \
                patch.object(self.account, 'sync'):
            self.account.run({'cmd': 'signin'})
        browser.assert_called_once()
        self.assertEqual(google.read(self.account.connection)['account'], added['account'])

    def test_a_cancelled_browser_signin_says_so(self):
        import subprocess
        reply = subprocess.CompletedProcess([], 0, stdout='{"error": "Sign-in was cancelled.", "cancelled": true}\n')
        with patch.object(self.account, 'signin_helper', return_value='/bin/true'), patch('subprocess.run', return_value=reply):
            with self.assertRaisesRegex(RuntimeError, 'cancelled'):
                self.account.signin()




class Router:
    """Several fake Google accounts behind one request(): each account's
    token (its Online Accounts path) picks its own FakeGoogle."""
    def __init__(self, apis):
        self.apis = apis

    def __call__(self, url, data=None, token=None, method=None, form=False, headers=None):
        return self.apis[token](url, data, token, method, form, headers)


class Accounts(unittest.TestCase):
    """Several accounts, event editing and the outbox, through the Hub (what
    the shell runs), with Google faked per account."""

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / 'google'
        self.apis = {}
        self.goa = []
        self.notes = []
        for p in (patch.object(google, 'request', side_effect=Router(self.apis)),
                  patch.object(google.Account, 'access', autospec=True, side_effect=lambda account: account.goa),
                  patch.object(google.OnlineAccounts, 'accounts', side_effect=lambda: [dict(a) for a in self.goa]),
                  patch.object(google.OnlineAccounts, 'access', return_value='ok'),
                  patch.object(google, 'emit', side_effect=self.notes.append)):
            p.start()
            self.addCleanup(p.stop)

    def hub(self):
        return google.Hub(self.root)

    def add(self, goa, name):
        api = FakeGoogle()
        api.calendars['primary'] = {'id': 'primary', 'summary': name, 'primary': True, 'accessRole': 'owner'}
        api.lists['inbox'] = {'id': 'inbox', 'title': 'Inbox'}
        self.apis[goa] = api
        self.goa.append({'id': goa, 'name': name, 'calendarDisabled': False})
        return api

    def connect(self, goa):
        return self.hub().run({'cmd': 'connectDesktop', 'account': goa})

    def ids(self, status, key='events'):
        return sorted((x['account'], x['id']) for x in status[key])

    def test_two_accounts_sync_independently(self):
        work, home = self.add('/goa/work', 'work@example.com'), self.add('/goa/home', 'me@example.com')
        work.events['primary']['a'] = {'id': 'a', 'summary': 'Standup', 'start': {'date': work.day(1)}, 'end': {'date': work.day(2)}}
        home.events['primary']['a'] = {'id': 'a', 'summary': 'Dentist', 'start': {'date': home.day(3)}, 'end': {'date': home.day(4)}}
        self.connect('/goa/work')
        status = self.connect('/goa/home')
        self.assertEqual([a['identity'] for a in status['accounts']], ['work@example.com', 'me@example.com'])
        self.assertEqual(sorted(e['title'] for e in status['events']), ['Dentist', 'Standup'])
        # Same calendar and event ids in both: told apart by account
        self.assertEqual(len({(e['account'], e['calendarId'], e['id']) for e in status['events']}), 2)
        self.assertTrue(all(c['account'] for c in status['calendars']))
        # Each keeps its own sync position; a change in one is seen only there
        home.events['primary']['b'] = {'id': 'b', 'summary': 'Gym', 'start': {'date': home.day(5)}, 'end': {'date': home.day(6)}}
        home.change('ev', 'primary/b')
        work.requests.clear()
        status = self.hub().run({'cmd': 'sync'})
        self.assertIn('Gym', [e['title'] for e in status['events']])
        self.assertTrue(all('syncToken=' in u for u in work.requests if '/events' in u))
        # The same account can't be connected twice; disconnecting one keeps the other
        with self.assertRaisesRegex(ValueError, 'already connected'):
            self.connect('/goa/work')
        home_id = next(a['id'] for a in status['accounts'] if a['identity'] == 'me@example.com')
        status = self.hub().run({'cmd': 'disconnect', 'account': home_id})
        self.assertEqual([a['identity'] for a in status['accounts']], ['work@example.com'])
        self.assertEqual([e['title'] for e in status['events']], ['Standup'])

    def test_signin_uses_an_unconnected_account_before_the_browser(self):
        self.add('/goa/work', 'work@example.com')
        self.add('/goa/home', 'me@example.com')
        self.connect('/goa/work')
        with patch.object(google.Account, 'signin') as browser:
            status = self.hub().run({'cmd': 'signin'})
        browser.assert_not_called()
        self.assertEqual(len(status['accounts']), 2)
        self.assertTrue(all(a['used'] for a in status['goaAccounts']))

    def test_one_account_from_before_moves_into_accounts(self):
        self.add('/goa/old', 'old@example.com')
        legacy = google.Account(self.root)
        google.atomic(legacy.connection, {'account': '/goa/old', 'identity': 'old@example.com'})
        google.atomic(legacy.cache, {'events': [], 'tasks': [{'id': 't', 'listId': 'inbox', 'text': 'Kept'}]})
        status = self.hub().run({'cmd': 'status'})
        self.assertEqual([a['identity'] for a in status['accounts']], ['old@example.com'])
        self.assertEqual([t['text'] for t in status['tasks']], ['Kept'])
        self.assertFalse((self.root / 'connection.json').exists())

    def test_events_are_created_edited_moved_and_deleted(self):
        api = self.add('/goa/me', 'me@example.com')
        aid = self.connect('/goa/me')['accounts'][0]['id']
        soon = google.datetime.now().replace(hour=9, minute=0, second=0, microsecond=0) + google.timedelta(days=7)
        start = soon.timestamp() * 1000
        status = self.hub().run({'cmd': 'eventInsert', 'account': aid, 'calendarId': 'primary',
                                 'fields': {'title': 'Review', 'start': start, 'end': start + 3600000, 'allDay': False, 'location': 'Room 2'}})
        event = next(e for e in status['events'] if e['title'] == 'Review')
        self.assertEqual(event['location'], 'Room 2')
        self.assertTrue(event['writable'])
        self.assertTrue(event['etag'])
        stored = api.events['primary'][event['id']]
        self.assertIn('dateTime', stored['start'])
        # Moved to an all-day event the next day: the old time is cleared
        next_day = soon.replace(hour=0) + google.timedelta(days=1)
        day = next_day.timestamp() * 1000
        status = self.hub().run({'cmd': 'eventPatch', 'account': aid, 'calendarId': 'primary', 'id': event['id'], 'etag': event['etag'],
                                 'fields': {'start': day, 'end': day + 86400000, 'allDay': True, 'notes': 'Bring slides'}})
        stored = api.events['primary'][event['id']]
        self.assertEqual(stored['start'], {'date': next_day.date().isoformat()})
        self.assertEqual(stored['end'], {'date': (next_day + google.timedelta(days=1)).date().isoformat()})
        self.assertEqual(stored['description'], 'Bring slides')
        event = next(e for e in status['events'] if e['id'] == event['id'])
        status = self.hub().run({'cmd': 'eventDelete', 'account': aid, 'calendarId': 'primary', 'id': event['id'], 'etag': event['etag']})
        self.assertNotIn(event['id'], [e['id'] for e in status['events']])
        self.assertEqual(self.hub().ops(), [])

    def test_a_read_only_calendar_refuses_edits(self):
        api = self.add('/goa/me', 'me@example.com')
        api.calendars['team'] = {'id': 'team', 'summary': 'Team', 'accessRole': 'reader'}
        api.events['team']['x'] = {'id': 'x', 'summary': 'Offsite', 'start': {'date': api.day(1)}, 'end': {'date': api.day(2)}}
        status = self.connect('/goa/me')
        aid = status['accounts'][0]['id']
        self.assertFalse(next(e for e in status['events'] if e['id'] == 'x')['writable'])
        self.assertFalse(next(c for c in status['calendars'] if c['id'] == 'team')['writable'])
        for cmd, extra in (('eventPatch', {'id': 'x', 'fields': {'title': 'Mine now'}}), ('eventDelete', {'id': 'x'}),
                           ('eventInsert', {'fields': {'title': 'New', 'start': 0, 'end': 0, 'allDay': True}})):
            with self.assertRaisesRegex(ValueError, 'read-only'):
                self.hub().run(dict({'cmd': cmd, 'account': aid, 'calendarId': 'team'}, **extra))
        self.assertEqual(self.hub().ops(), [])
        self.assertEqual(api.writes, [])

    def test_editing_a_recurring_event_changes_that_occurrence_only(self):
        api = self.add('/goa/me', 'me@example.com')
        for d in (1, 8):
            iid = 'weekly_' + api.day(d).replace('-', '')
            api.events['primary'][iid] = {'id': iid, 'summary': 'Sync', 'recurringEventId': 'weekly', 'etag': '"e%d"' % d,
                                          'start': {'date': api.day(d)}, 'end': {'date': api.day(d + 1)}}
        status = self.connect('/goa/me')
        aid = status['accounts'][0]['id']
        first = next(e for e in status['events'] if e['id'].endswith(api.day(1).replace('-', '')))
        self.assertTrue(first['recurring'])
        status = self.hub().run({'cmd': 'eventPatch', 'account': aid, 'calendarId': 'primary', 'id': first['id'],
                                 'etag': first['etag'], 'fields': {'title': 'Sync (moved room)'}})
        self.assertEqual(api.writes, [('PATCH', 'ev', 'primary', first['id'])])
        self.assertEqual(sorted(e['title'] for e in status['events']), ['Sync', 'Sync (moved room)'])

    def test_editing_every_occurrence_changes_the_series(self):
        api = self.add('/goa/me', 'me@example.com')
        nine = google.datetime.now().replace(hour=9, minute=0, second=0, microsecond=0)
        def at(t):
            return {'dateTime': t.astimezone().isoformat()}
        # The series itself (not listed with singleEvents) and two occurrences
        master = {'id': 'weekly', 'summary': 'Sync', 'recurrence': ['RRULE:FREQ=WEEKLY'], 'etag': '"m"',
                  'start': at(nine - google.timedelta(days=14)), 'end': at(nine - google.timedelta(days=14, hours=-1))}
        for d in (1, 8):
            iid = 'weekly_%d' % d
            api.events['primary'][iid] = {'id': iid, 'summary': 'Sync', 'recurringEventId': 'weekly',
                                          'start': at(nine + google.timedelta(days=d)), 'end': at(nine + google.timedelta(days=d, hours=1))}
        status = self.connect('/goa/me')
        api.events['primary']['weekly'] = dict(master)  # (only after the sync: it isn't one of the listed occurrences)
        aid = status['accounts'][0]['id']
        first = next(e for e in status['events'] if e['id'] == 'weekly_1')
        self.assertEqual(first['series'], 'weekly')
        # An hour later, every week: the series moves an hour, from where it starts
        was = google.ms(first['start'])
        api.offline = True
        with self.assertRaises(google.Offline):
            self.hub().run({'cmd': 'eventPatch', 'account': aid, 'calendarId': 'primary', 'id': first['id'], 'series': 'weekly',
                            'was': was, 'fields': {'title': 'Weekly sync', 'start': was + 3600000, 'end': was + 7200000, 'allDay': False}})
        shown = {e['id']: e for e in self.hub().status()['events']}
        self.assertEqual({shown[i]['title'] for i in ('weekly_1', 'weekly_8')}, {'Weekly sync'})
        self.assertEqual(google.ms(shown['weekly_8']['start']), google.ms(api.events['primary']['weekly_8']['start']) + 3600000)
        api.offline = False
        self.hub().run({'cmd': 'flush'})
        self.assertEqual(api.writes, [('PATCH', 'ev', 'primary', 'weekly')])
        self.assertEqual(api.events['primary']['weekly']['summary'], 'Weekly sync')
        self.assertEqual(google.ms(api.events['primary']['weekly']['start']), google.ms(master['start']) + 3600000)
        # Deleting every occurrence deletes the series
        self.hub().run({'cmd': 'eventDelete', 'account': aid, 'calendarId': 'primary', 'id': 'weekly_8', 'series': 'weekly'})
        self.assertEqual(api.writes[-1], ('DELETE', 'ev', 'primary', 'weekly'))

    def test_changes_made_offline_survive_a_restart_and_go_out_later(self):
        api = self.add('/goa/me', 'me@example.com')
        api.tasks['inbox']['t1'] = {'id': 't1', 'title': 'Milk', 'status': 'needsAction', 'etag': '"t1"', '_updated': 1}
        aid = self.connect('/goa/me')['accounts'][0]['id']
        api.offline = True
        # Two changes while offline (one to a task added offline): kept in
        # order, on disk, privately, and already shown
        with self.assertRaises(google.Offline):
            self.hub().run({'cmd': 'taskUpdate', 'account': aid, 'listId': 'inbox', 'id': 't1', 'etag': '"t1"', 'patch': {'done': True}})
        with self.assertRaises(google.Offline):
            self.hub().run({'cmd': 'taskAdd', 'account': aid, 'listId': 'inbox', 'patch': {'text': 'Bread'}})
        hub = self.hub()
        pending = next(t for t in hub.status()['tasks'] if t['text'] == 'Bread')
        with self.assertRaises(google.Offline):
            hub.run({'cmd': 'taskUpdate', 'account': aid, 'listId': 'inbox', 'id': pending['id'], 'patch': {'text': 'Rye bread'}})
        self.assertEqual(hub.outbox.stat().st_mode & 0o777, 0o600)
        self.assertEqual([o['cmd'] for o in hub.ops()], ['taskUpdate', 'taskAdd'])
        shown = {t['text']: t for t in self.hub().status()['tasks']}
        self.assertTrue(shown['Milk']['done'])
        self.assertIn('Rye bread', shown)
        self.assertEqual(self.hub().status()['outbox'], 2)
        # Back online, after a restart: the next sync sends them first
        api.offline = False
        status = google.Hub(self.root).run({'cmd': 'sync'})
        self.assertEqual(status['outbox'], 0)
        self.assertEqual(api.tasks['inbox']['t1']['status'], 'completed')
        self.assertIn('Rye bread', [t.get('title') for t in api.tasks['inbox'].values()])
        self.assertEqual(sorted(t['text'] for t in status['tasks']), ['Milk', 'Rye bread'])

    def test_a_change_that_lost_to_google_keeps_googles_version_and_says_so_once(self):
        api = self.add('/goa/me', 'me@example.com')
        api.events['primary']['m'] = {'id': 'm', 'summary': 'Meeting', 'etag': '"old"', 'start': {'date': api.day(1)}, 'end': {'date': api.day(2)}}
        aid = self.connect('/goa/me')['accounts'][0]['id']
        api.events['primary']['m'].update(summary='Meeting (moved)', etag='"new"')  # changed on the phone
        api.change('ev', 'primary/m')
        status = self.hub().run({'cmd': 'eventPatch', 'account': aid, 'calendarId': 'primary', 'id': 'm', 'etag': '"old"',
                                 'title': 'Meeting', 'fields': {'title': 'Meeting with Sam'}})
        conflicts = [n['conflict'] for n in self.notes if 'conflict' in n]
        self.assertEqual(len(conflicts), 1)
        self.assertEqual(conflicts[0]['title'], 'Meeting')
        self.assertIn('Meeting with Sam', conflicts[0]['what'])
        self.assertEqual(conflicts[0]['reason'], 'changed')
        self.assertEqual(api.events['primary']['m']['summary'], 'Meeting (moved)')
        self.assertEqual(self.hub().ops(), [])
        self.hub().run({'cmd': 'sync'})
        self.assertEqual(len([n for n in self.notes if 'conflict' in n]), 1)  # once

    def test_deleted_on_google_an_edit_is_a_conflict_a_deletion_is_done(self):
        api = self.add('/goa/me', 'me@example.com')
        api.tasks['inbox']['t'] = {'id': 't', 'title': 'Call', 'status': 'needsAction', '_updated': 1}
        aid = self.connect('/goa/me')['accounts'][0]['id']
        api.tasks['inbox']['t'] = {'id': 't', 'deleted': True, '_updated': google.time.time()}
        self.hub().run({'cmd': 'taskDelete', 'account': aid, 'listId': 'inbox', 'id': 't'})
        self.assertEqual([n for n in self.notes if 'conflict' in n], [])
        self.hub().run({'cmd': 'taskUpdate', 'account': aid, 'listId': 'inbox', 'id': 't', 'patch': {'text': 'Call back'}})
        self.assertEqual([n['conflict']['reason'] for n in self.notes if 'conflict' in n], ['deleted'])
        self.assertEqual(self.hub().ops(), [])

    def test_two_quick_edits_to_one_item_are_not_a_conflict(self):
        api = self.add('/goa/me', 'me@example.com')
        api.tasks['inbox']['t'] = {'id': 't', 'title': 'Call', 'status': 'needsAction', 'etag': '"t"', '_updated': 1}
        aid = self.connect('/goa/me')['accounts'][0]['id']
        # Both made against the version shown before the first reached Google
        for text in ('Call Sam', 'Call Sam back'):
            self.hub().run({'cmd': 'taskUpdate', 'account': aid, 'listId': 'inbox', 'id': 't', 'etag': '"t"', 'patch': {'text': text}})
        self.assertEqual([n for n in self.notes if 'conflict' in n], [])
        self.assertEqual(api.tasks['inbox']['t']['title'], 'Call Sam back')
        # A change on the phone meanwhile is still caught
        api.tasks['inbox']['t'].update(title='Call Kim', etag='"phone"')
        self.hub().run({'cmd': 'taskUpdate', 'account': aid, 'listId': 'inbox', 'id': 't', 'etag': '"t"', 'patch': {'text': 'Mine'}})
        self.assertEqual(len([n for n in self.notes if 'conflict' in n]), 1)
        self.assertEqual(api.tasks['inbox']['t']['title'], 'Call Kim')

    def test_changes_queue_in_order_and_go_out_in_order(self):
        api = self.add('/goa/me', 'me@example.com')
        aid = self.connect('/goa/me')['accounts'][0]['id']
        api.offline = True
        for text in ('one', 'two', 'three'):
            with self.assertRaises(google.Offline):
                self.hub().run({'cmd': 'taskAdd', 'account': aid, 'listId': 'inbox', 'patch': {'text': text}})
        api.offline = False
        self.hub().run({'cmd': 'flush'})
        made = sorted(api.tasks['inbox'].values(), key=lambda t: int(t['id'][1:]))
        self.assertEqual([t['title'] for t in made], ['one', 'two', 'three'])


if __name__ == '__main__':
    unittest.main()
