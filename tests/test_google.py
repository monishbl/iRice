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

    @staticmethod
    def day(offset):
        from datetime import date, timedelta
        return (date.today() + timedelta(days=offset)).isoformat()

    def change(self, kind, key):
        self.version += 1
        self.changed[(kind, key)] = self.version

    def serving(self, account):
        return _Serving(self, account)

    def __call__(self, url, data=None, token=None, method=None, form=False):
        self.requests.append(url)
        u = urllib.parse.urlparse(url)
        q = {k: v[0] for k, v in urllib.parse.parse_qs(u.query).items()}
        parts = [urllib.parse.unquote(p) for p in u.path.split('/') if p]
        since = q.get('syncToken')
        if since and since in self.expired:
            raise google.Gone('expired')
        fresh = str(self.version)
        if parts[-2:] == ['me', 'calendarList']:
            rows = [dict(c) for c in self.calendars.values()]
            if since:
                rows = [r for r in rows if self.changed.get(('cal', r['id']), 0) > int(since)]
            return {'items': rows, 'nextSyncToken': fresh}
        if parts[-1] == 'events':
            cid = parts[-2]
            rows = list(self.events[cid].values())
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


if __name__ == '__main__':
    unittest.main()
