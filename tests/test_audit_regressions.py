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

    def test_bad_discovery_and_metadata_are_bounded(self):
        for packet in [[], None, {'fingerprint': 'abc', 'port': 'invalid'}, {'fingerprint': 'abc', 'port': -1}]:
            self.assertFalse(self.m.remember(packet, '127.0.0.1'))
        h = self.handler(b'', self.m.MAX_BODY + 1)
        with self.assertRaises(ValueError): h.body()

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

    def test_download_filenames_with_control_characters_remain_valid_json(self):
        m = load('transfers')
        with tempfile.TemporaryDirectory() as d:
            p = Path(d)/'line\nwith\ttab.part'; p.write_bytes(b'123')
            result = json.loads(json.dumps(m.downloads(d)))
            self.assertEqual(result, [{'path': str(p), 'bytes': 3}])

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
