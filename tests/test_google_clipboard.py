"""Isolated Google transport/storage and clipboard cache regressions; no real accounts."""
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
clipboard = load('clipboard-previews')


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

    def test_broker_filters_disabled_calendar_and_non_google_accounts(self):
        broker = google.OnlineAccounts()
        account_iface = broker.name + '.Account'
        oauth_iface = broker.name + '.OAuth2Based'
        def account(provider='google', disabled=False):
            return {account_iface: {'ProviderType': provider, 'CalendarDisabled': disabled, 'PresentationIdentity': 'Person'}, oauth_iface: {}}
        objects = {broker.prefix + '1': account(), broker.prefix + '2': account(disabled=True), broker.prefix + '3': account('other')}
        with patch.object(broker, 'call', return_value=(objects,)):
            self.assertEqual(broker.accounts(), [{'id': broker.prefix + '1', 'name': 'Person'}])
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
        replies = [[{'id': 'a@b', 'summary': 'Work'}],
                   [{'id': 'event', 'start': {'date': '2026-10-29'}, 'end': {'date': '2026-10-30'}}],
                   [{'id': 'list', 'title': 'Inbox'}],
                   [{'id': 'task', 'title': 'Test', 'due': '2026-10-30T00:00:00Z', 'status': 'completed'}]]
        with patch.object(self.account, 'access', return_value='token'), patch.object(self.account, 'pages', side_effect=replies):
            self.account.sync()
        data = google.read(self.account.cache)
        self.assertEqual(data['events'][0]['end'], {'date': '2026-10-30'})
        self.assertEqual(data['tasks'][0]['date'], '2026-10-30')
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


class ClipboardTests(unittest.TestCase):
    def test_cached_images_do_not_decode_again(self):
        with tempfile.TemporaryDirectory() as tmp:
            image = Path(tmp) / '1.png'
            image.write_bytes(b'image')
            with patch.object(clipboard.subprocess, 'run') as run, contextlib.redirect_stdout(io.StringIO()) as output:
                clipboard.previews('db', tmp, [('1', str(image))])
                clipboard.previews('db', tmp, [('1', str(image))])
                run.assert_not_called()
            self.assertEqual(output.getvalue().splitlines(), [str(image), str(image)])

    def test_failed_decode_does_not_publish_partial_file(self):
        with tempfile.TemporaryDirectory() as tmp:
            image = Path(tmp) / '2.png'
            def decode(*args, **kwargs):
                kwargs['stdout'].write(b'partial')
                return type('Result', (), {'returncode': 1})()
            with patch.object(clipboard.subprocess, 'run', side_effect=decode), contextlib.redirect_stdout(io.StringIO()) as output:
                clipboard.previews('db', tmp, [('2', str(image))])
            self.assertFalse(image.exists())
            self.assertEqual(output.getvalue(), '')
            self.assertEqual(list(Path(tmp).iterdir()), [])

    def test_decodes_beyond_old_48_image_limit(self):
        with tempfile.TemporaryDirectory() as tmp:
            def decode(*args, **kwargs):
                kwargs['stdout'].write(b'image')
                return type('Result', (), {'returncode': 0})()
            pairs = [(str(i), str(Path(tmp) / (str(i) + '.png'))) for i in range(60)]
            with patch.object(clipboard.subprocess, 'run', side_effect=decode), contextlib.redirect_stdout(io.StringIO()) as output:
                clipboard.previews('db', tmp, pairs)
            self.assertEqual(len(output.getvalue().splitlines()), 60)


if __name__ == '__main__':
    unittest.main()
