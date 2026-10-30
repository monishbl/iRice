"""Failure-path regressions for the October desktop audit; no live desktop mutation."""
import importlib.util
import io
import json
import os
from pathlib import Path
import ssl
import subprocess
import tempfile
import threading
import time
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / 'shell/assets/scripts'


def load(name, path=None):
    spec = importlib.util.spec_from_file_location(name.replace('-', '_'), path or SCRIPTS / (name + '.py'))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


class LocalSendSafety(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.m = load('localsend')
        self.m.DOWNLOADS = self.tmp.name
        self.events = []
        self.m.emit = lambda ev, **kw: self.events.append((ev, kw))

    def session(self, size, sid='fixture', name='file.txt'):
        s = dict(id=sid, files={'f': dict(fileName=name, size=size)}, tokens={'f': 'token'}, paths=[], done=0,
                 total=size, cancelled=False, claimed=set(), touched=time.monotonic(), **{'from': 'fixture'})
        self.m.sessions[sid] = s
        return s

    def handler(self, data, length):
        h = object.__new__(self.m.Handler)
        h.headers = {'Content-Length': str(length)}
        h.rfile = io.BytesIO(data)
        h.replies = []
        h.reply = lambda code, body=None: h.replies.append(code)
        return h

    def upload(self, h, sid='fixture'):
        h.upload(dict(sessionId=sid, fileId='f', token='token'))

    def test_truncated_upload_removes_partial_file_and_never_finishes(self):
        self.session(100)
        h = self.handler(b'short', 100)
        self.upload(h)
        self.assertEqual(list(Path(self.tmp.name).iterdir()), [])
        self.assertNotIn('finished', [e[0] for e in self.events])
        self.assertIn('failed', [e[0] for e in self.events])

    def test_declared_size_is_binding_and_missing_token_rejected(self):
        self.session(1)
        h = self.handler(b'large', 5)
        self.upload(h)
        self.assertEqual(h.replies, [400])
        h.upload(dict(sessionId='fixture', fileId='missing'))
        self.assertEqual(h.replies[-1], 403)
        self.assertEqual(list(Path(self.tmp.name).iterdir()), [])

    def test_cancel_during_read_cannot_write_or_finish(self):
        self.session(4)
        h = self.handler(b'', 4)
        def read(n):
            self.m.cancel_incoming('fixture')
            return b'data'
        h.rfile.read = read
        self.upload(h)
        self.assertEqual([e[0] for e in self.events], ['failed'])
        self.assertEqual(list(Path(self.tmp.name).iterdir()), [])

    def test_parallel_sessions_preserve_both_same_named_files(self):
        self.session(4, 'a'); self.session(4, 'b')
        threads = [threading.Thread(target=self.upload, args=(self.handler(data, 4), sid)) for sid, data in [('a', b'AAAA'), ('b', b'BBBB')]]
        for t in threads: t.start()
        for t in threads: t.join(3)
        self.assertEqual({p.read_bytes() for p in Path(self.tmp.name).iterdir()}, {b'AAAA', b'BBBB'})
        self.assertEqual(sum(e[0] == 'finished' for e in self.events), 2)

    def test_pending_timeout_clears_session_and_emits_declined(self):
        self.m.state['mode'] = 'everyone'
        raw = json.dumps({'info': {}, 'files': {'f': {'fileName': 'test', 'size': 1}}}).encode()
        h = self.handler(raw, len(raw))
        with patch.object(self.m.threading.Event, 'wait', return_value=False): h.prepare()
        self.assertEqual([e[0] for e in self.events], ['incoming', 'declined'])
        self.assertEqual(self.m.sessions, {})
        # Nobody answered: it expired (not "they declined"); declined here: not expired
        self.assertEqual({k: self.events[-1][1].get(k) for k in ('dir', 'expired')}, {'dir': 'in', 'expired': True})
        def decline(timeout):
            for s in self.m.sessions.values(): s['accepted'] = False
            return True
        h = self.handler(raw, len(raw))
        with patch.object(self.m.threading.Event, 'wait', side_effect=decline): h.prepare()
        self.assertEqual(self.events[-1][0], 'declined')
        self.assertEqual({k: self.events[-1][1].get(k) for k in ('dir', 'expired')}, {'dir': 'in', 'expired': False})

    def test_bad_discovery_and_metadata_are_bounded(self):
        for packet in [[], None, {'fingerprint': 'abc', 'port': 'invalid'}, {'fingerprint': 'abc', 'port': -1}]:
            self.assertFalse(self.m.remember(packet, '127.0.0.1'))
        h = self.handler(b'', self.m.MAX_BODY + 1)
        with self.assertRaises(ValueError): h.body()

    def test_crafted_discovery_packet_never_stops_the_listener(self):
        packets = [json.dumps({'announce': True, 'fingerprint': ['x']}).encode(), b'\xff', b'[]',
                   json.dumps({'fingerprint': 'good', 'port': 53317}).encode()]
        class Sock:
            def recvfrom(_, n):
                if not packets: raise SystemExit
                return packets.pop(0), ('10.0.0.2', 1)
        self.m.multicast_socket = Sock
        with patch('sys.stderr', io.StringIO()), self.assertRaises(SystemExit): self.m.listen()
        self.assertEqual(list(self.m.peers), ['good'])
        self.assertIn('peers', [e[0] for e in self.events])

    def test_tls_rejects_wrong_certificate_and_unencrypted_peer(self):
        self.m.configure_tls(Path(self.tmp.name) / 'tls')
        server = self.m.Server(('127.0.0.1', 0), self.m.Handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        self.addCleanup(server.server_close); self.addCleanup(server.shutdown)
        p = dict(ip='127.0.0.1', port=server.server_address[1], protocol='https', fingerprint='00' * 32)
        c = self.m.connect(p)
        self.addCleanup(c.close)
        with self.assertRaises(ssl.SSLError): c.request('GET', self.m.API + '/info')
        p['protocol'] = 'http'
        with self.assertRaises(ValueError): self.m.connect(p)


class PersistenceAndIdentity(unittest.TestCase):
    def test_state_journals_merge_fields_order_revisions_and_stay_private(self):
        m = load('state-merge')
        with tempfile.TemporaryDirectory() as d:
            p = Path(d); m.merge(p)
            (p/'pending/a.json').write_text(json.dumps({'desktopNote': {'revision': [2, 'a', 1], 'value': 'latest'}}))
            (p/'pending/b.json').write_text(json.dumps({'todos': {'revision': [1, 'b', 1], 'value': ['todo']}, 'desktopNote': {'revision': [1, 'b', 1], 'value': 'old'}}))
            m.merge(p)
            data = json.loads((p/'state.json').read_text())
            self.assertEqual(data['desktopNote'], 'latest'); self.assertEqual(data['todos'], ['todo'])
            self.assertEqual((p/'state.json').stat().st_mode & 0o777, 0o600)
            self.assertEqual(p.stat().st_mode & 0o777, 0o700)
            self.assertEqual(list((p/'pending').iterdir()), [])

    def test_note_collision_and_write_failure_do_not_replace_saved_note(self):
        m = load('save-note')
        with tempfile.TemporaryDirectory() as d:
            p = Path(d)/'note.md'; m.save(p, 'original')
            with self.assertRaises(FileExistsError): m.save(p, 'different')
            self.assertEqual(p.read_text(), 'original')
            self.assertEqual(p.stat().st_mode & 0o777, 0o600)
            self.assertEqual(list(Path(d).iterdir()), [p])

    def test_download_filenames_with_control_characters_are_listed_intact(self):
        # services/Transfers.qml lists partial downloads with this find and
        # splits its NUL-separated "size<TAB>path" entries
        with tempfile.TemporaryDirectory() as d:
            p = Path(d)/'line\nwith\ttab.part'; p.write_bytes(b'123')
            (Path(d)/'done.txt').write_text('x')
            out = subprocess.run(['find', d, '-maxdepth', '1', '-type', 'f', '(', '-name', '*.part', '-o', '-name', '*.crdownload', '-o', '-name', '*.download', ')', '-printf', '%s\\t%p\\0'], capture_output=True, text=True, check=True).stdout
            entries = [(int(e[:e.index('\t')]), e[e.index('\t') + 1:]) for e in out.split('\0') if '\t' in e]
            self.assertEqual(entries, [(3, str(p))])

    def test_icon_cache_detects_nested_change_and_symlink_cycles_end(self):
        m = load('icon-index')
        with tempfile.TemporaryDirectory() as d:
            p=Path(d); theme=p/'theme'; (theme/'scalable/apps').mkdir(parents=True)
            (theme/'index.theme').write_text('[Icon Theme]')
            m.ROOTS=[d]
            cache=p/'cache'; cache.write_text('old'); old=time.time()-20
            for entry in [theme, theme/'index.theme', theme/'scalable', theme/'scalable/apps']: os.utime(entry,(old,old))
            os.utime(cache,(old+10,old+10))
            (theme/'scalable/apps/new.svg').write_text('<svg/>')
            self.assertFalse(m.fresh(cache,['theme']))
            (theme/'scalable/apps/loop').symlink_to(theme, target_is_directory=True)
            self.assertLess(len(list(m.walk_theme(theme))), 5)

    def test_icon_cache_sees_icons_added_inside_a_linked_folder(self):
        # MacTahoe-dark: apps/scalable -> ../../MacTahoe/apps/scalable
        m = load('icon-index')
        with tempfile.TemporaryDirectory() as d:
            p=Path(d); (p/'base/apps/scalable').mkdir(parents=True); (p/'dark/apps').mkdir(parents=True)
            (p/'dark/index.theme').write_text('[Icon Theme]')
            (p/'dark/apps/scalable').symlink_to('../../base/apps/scalable', target_is_directory=True)
            m.ROOTS=[d]
            cache=p/'cache'; cache.write_text('old'); old=time.time()-20
            for entry in [p/'base', p/'base/apps', p/'base/apps/scalable', p/'dark', p/'dark/index.theme', p/'dark/apps']: os.utime(entry,(old,old))
            os.utime(p/'dark/apps/scalable',(old,old),follow_symlinks=False)
            os.utime(cache,(old+10,old+10))
            self.assertTrue(m.fresh(cache,['dark']))
            (p/'base/apps/scalable/new.svg').write_text('<svg/>')
            self.assertFalse(m.fresh(cache,['dark']))

    def test_icon_cache_stays_fresh_with_a_broken_link(self):
        m = load('icon-index')
        with tempfile.TemporaryDirectory() as d:
            p=Path(d); theme=p/'theme'; (theme/'apps').mkdir(parents=True)
            (theme/'apps/gone.svg').symlink_to(p/'missing.svg')
            m.ROOTS=[d]
            cache=p/'cache'; cache.write_text('')
            old=time.time()-20
            for entry in [theme, theme/'apps']: os.utime(entry,(old,old))
            self.assertTrue(m.fresh(cache,['theme']))

    def test_corrupt_state_is_set_aside_and_journals_still_merge(self):
        m = load('state-merge')
        with tempfile.TemporaryDirectory() as d:
            p=Path(d); (p/'pending').mkdir()
            (p/'state.json').write_text('{not json')
            (p/'pending/j-1.json').write_text(json.dumps({'todos': {'revision': [1, 'j', 1], 'value': ['a']}}))
            m.merge(d)
            self.assertEqual(json.loads((p/'state.json').read_text())['todos'], ['a'])
            self.assertEqual(len(list(p.glob('state.json.corrupt-*'))), 1)
            self.assertEqual(list((p/'pending').iterdir()), [])

    def test_reused_process_identity_never_signalled(self):
        m = load('process-identity')
        child = subprocess.Popen(['sleep', '10'], start_new_session=True)
        try:
            with patch.object(m.signal, 'pidfd_send_signal') as send:
                m.stop_group(child.pid, '1')
                send.assert_not_called()
                with self.assertRaises(ValueError): m.handle(child.pid, '1')
            fd = m.handle(child.pid, m.identity(child.pid)[0]); os.close(fd)
            self.assertIsNone(child.poll())
        finally:
            child.terminate(); child.wait()

    def test_helpers_left_after_their_leader_exited_are_still_ended(self):
        # The leak that overheated the laptop: the wrapper dies, the watcher's
        # leader goes with it (pdeathsig), its helpers (pw-dump | …) stay.
        m = load('process-identity')
        token = os.urandom(16).hex()
        leader = subprocess.Popen(['bash', '-c', 'sleep 30 & echo $!; exit 0'], stdout=subprocess.PIPE, start_new_session=True, text=True,
                                  env=dict(os.environ, IRICE_WATCH_TOKEN=token))
        helper = int(leader.stdout.readline())
        start = m.identity(leader.pid)[0] if leader.poll() is None else None
        leader.wait()
        try:
            self.assertEqual(m.identity(helper)[1], leader.pid)  # still in the leader's group
            m.stop_group(leader.pid, start or m.identity(helper)[0], token)
            time.sleep(0.5)
            with self.assertRaises(OSError):
                m.identity(helper)
        finally:
            try: os.kill(helper, 9)
            except OSError: pass

    def test_group_without_its_token_is_never_signalled_after_the_leader_exits(self):
        # A group whose leader is gone could be anyone's: without the record's
        # token in a member's environment (or with no token: an old record),
        # nothing in it is touched
        m = load('process-identity')
        leader = subprocess.Popen(['bash', '-c', 'sleep 30 & echo $!; exit 0'], stdout=subprocess.PIPE, start_new_session=True, text=True,
                                  env=dict(os.environ, IRICE_WATCH_TOKEN='0' * 32))
        stranger = int(leader.stdout.readline()); leader.wait()
        try:
            with patch.object(m.signal, 'pidfd_send_signal') as send:
                for token in ('1' * 32, None, '', 'short'):
                    out = io.StringIO()
                    with patch('sys.stdout', out): m.stop_group(leader.pid, '1', token, dry=True)
                    self.assertEqual(out.getvalue(), '')
                    m.stop_group(leader.pid, '1', token)
                send.assert_not_called()
            with tempfile.TemporaryDirectory() as d:
                (Path(d)/'irice-watchers').mkdir()
                (Path(d)/'irice-watchers'/str(leader.pid)).write_text('1 1 %d 1\n' % leader.pid)  # no token: written before tokens
                with patch.dict(os.environ, {'XDG_RUNTIME_DIR': d}), patch.object(m.signal, 'pidfd_send_signal') as send:
                    m.reap(False)
                    send.assert_not_called()
            self.assertEqual(m.identity(stranger)[1], leader.pid)
        finally:
            os.kill(stranger, 9)

    def test_watch_wrapper_records_a_token_its_watcher_carries(self):
        with tempfile.TemporaryDirectory() as d:
            env = dict(os.environ, XDG_RUNTIME_DIR=d)
            p = subprocess.Popen(['bash', str(SCRIPTS/'watch.sh'), 'sh', '-c', 'echo "$IRICE_WATCH_TOKEN" > "$0"/seen; sleep 30', d],
                                 stdin=subprocess.PIPE, env=env)
            try:
                for _ in range(50):
                    if (Path(d)/'seen').exists() and (Path(d)/'seen').read_text(): break
                    time.sleep(0.05)
                record = next((Path(d)/'irice-watchers').iterdir()).read_text().split()
                self.assertEqual(len(record), 5)
                self.assertEqual(record[4], (Path(d)/'seen').read_text().strip())
                self.assertEqual(len(record[4]), 32)
            finally:
                p.stdin.close(); p.wait(timeout=10)
            self.assertEqual(list((Path(d)/'irice-watchers').iterdir()), [])

    def test_idle_launcher_keeps_custom_listeners_and_uses_this_shell_lock(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d); (p/'bin').mkdir(); (p/'config/hypr').mkdir(parents=True)
            (p/'config/hypr/hypridle.conf').write_text('listener {\n timeout = 1234\n on-timeout = custom-action\n}\n')
            stub=p/'bin/hypridle'
            stub.write_text('#!/bin/sh\ncat "$2" > "$IDLE_CAPTURE"\n'); stub.chmod(0o755)
            env=dict(os.environ, PATH=str(p/'bin')+':'+os.environ['PATH'], IRICE_SANDBOX='1', IRICE_TTY='1', XDG_RUNTIME_DIR=d, XDG_CONFIG_HOME=str(p/'config'), HYPRLAND_INSTANCE_SIGNATURE='fixture', IDLE_CAPTURE=str(p/'captured'))
            subprocess.run(['bash',str(SCRIPTS/'start-idle.sh'),'/some path/qs'],env=env,check=True,timeout=3)
            data=(p/'captured').read_text()
            self.assertIn('timeout = 1234',data)
            self.assertIn('inhibit_sleep = 3',data)
            self.assertIn('before_sleep_cmd = /some\\ path/qs -p ',data)
            self.assertIn(str(ROOT/'shell'),data)
            (p/'captured').unlink(); env['IRICE_TTY']='0'
            subprocess.run(['bash',str(SCRIPTS/'start-idle.sh')],env=env,check=True,timeout=3)
            self.assertFalse((p/'captured').exists())

    def test_idle_launcher_default_and_settings_configs_lock_before_sleep(self):
        # No hypridle.conf of your own: the built-in default, then the timers
        # from Settings (with and without a password grace after sleep)
        with tempfile.TemporaryDirectory() as d:
            p=Path(d); (p/'bin').mkdir(); (p/'config/irice').mkdir(parents=True)
            stub=p/'bin/hypridle'
            stub.write_text('#!/bin/sh\ncat "$2" > "$IDLE_CAPTURE"\n'); stub.chmod(0o755)
            env=dict(os.environ, PATH=str(p/'bin')+':'+os.environ['PATH'], IRICE_SANDBOX='1', IRICE_TTY='1', XDG_RUNTIME_DIR=d, XDG_CONFIG_HOME=str(p/'config'), HYPRLAND_INSTANCE_SIGNATURE='fixture', IDLE_CAPTURE=str(p/'captured'))
            def run():
                subprocess.run(['bash',str(SCRIPTS/'start-idle.sh'),'qs'],env=env,check=True,timeout=3)
                return (p/'captured').read_text()
            lock='qs -p %s ipc call lock lock' % (ROOT/'shell')
            data=run()
            self.assertIn('timeout = 300\n    on-timeout = '+lock,data)
            self.assertIn('lock_cmd = '+lock,data)
            self.assertIn('before_sleep_cmd = '+lock,data)
            self.assertIn('inhibit_sleep = 3',data)
            (p/'config/irice/idle-listeners').write_text('ac lock 120\nbattery sleep 900\nbogus line here\n')
            data=run()
            self.assertIn('timeout = 120\n    on-timeout = %s ac lock' % (SCRIPTS/'idle-action.sh'),data)
            self.assertIn('timeout = 900',data); self.assertNotIn('bogus',data); self.assertNotIn('timeout = 300',data)
            self.assertIn('lock_cmd = '+lock,data)
            self.assertIn('before_sleep_cmd = '+lock,data)
            self.assertIn('inhibit_sleep = 3',data)
            (p/'config/irice/idle-listeners').write_text('ac lock 120\ngrace 30\n')
            data=run()
            self.assertIn('lock_cmd = '+lock,data)
            self.assertIn('before_sleep_cmd = %s before-sleep' % (SCRIPTS/'idle-action.sh'),data)
            self.assertIn('inhibit_sleep = 3',data)
            self.assertEqual((p/('irice-idle-fixture/grace')).read_text().strip(),'30')

    def test_copied_install_resolves_recorded_source(self):
        m = load('update-source')
        with tempfile.TemporaryDirectory() as d:
            p=Path(d); (p/'data/irice').mkdir(parents=True)
            (p/'data/irice/source').write_text(str(ROOT))
            with patch.dict(os.environ, {'XDG_DATA_HOME': str(p/'data'), 'IRICE_UPDATE_REPO': ''}):
                self.assertEqual(m.resolve(p/'installed/shell'), str(ROOT))

    def test_updater_stash_failure_restores_untracked_files(self):
        m = load('updater', ROOT/'tools/irice-update.py')
        with tempfile.TemporaryDirectory() as d:
            p = Path(d); repo = p/'repo'; repo.mkdir()
            subprocess.run(['git', 'init', '-q', str(repo)], check=True)
            subprocess.run(['git', '-C', str(repo), '-c', 'user.name=Test', '-c', 'user.email=test@example.invalid', 'commit', '--allow-empty', '-qm', 'fixture'], check=True)
            m.REPO = str(repo); (repo/'added.txt').write_text('my file')
            m.repo_lock = lambda: None
            m.check = lambda *a, **kw: dict(available=True, upstream='HEAD', conflicts=[dict(id='code:added.txt', kind='code', title='added.txt', change='untracked', choices=['mine', 'new'])])
            realgit = m.git
            def git(*a, **kw):
                if a[:2] == ('diff', '--name-only'): return 'tracked.txt\0'
                if a[:2] == ('stash', 'push'): raise RuntimeError('injected failure')
                return realgit(*a, **kw)
            m.git = git
            with self.assertRaises(RuntimeError): m.apply(str(p/'config'), {'code:added.txt': 'mine'})
            self.assertEqual((repo/'added.txt').read_text(), 'my file')


if __name__ == '__main__': unittest.main()
