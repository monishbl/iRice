"""Regression checks for desktop palettes, multi-file selection and Quick Share.

Run: python3 -m unittest discover -s tests -v
All settings and helper commands are isolated in temporary directories.
"""
import configparser
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import threading
import http.server
import unittest
from unittest.mock import patch

SCRIPTS = Path(__file__).resolve().parents[1] / 'shell/assets/scripts'


def load(name):
    spec = importlib.util.spec_from_file_location(name.replace('-', '_'), SCRIPTS / (name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def ini(path):
    conf = configparser.ConfigParser(interpolation=None)
    conf.optionxform = str
    conf.read(path)
    return conf


class DesktopSharingTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='irice-sharing-test-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.cfg = self.root / 'config'
        self.data = self.root / 'data'
        self.bin = self.root / 'bin'
        self.bin.mkdir()
        for command in ['gsettings', 'gdbus', 'pkill']:
            self.command(command, '#!/bin/sh\nexit 0\n')
        self.env = os.environ.copy()
        self.env.update(XDG_CONFIG_HOME=str(self.cfg), XDG_DATA_HOME=str(self.data),
                        XDG_CACHE_HOME=str(self.root / 'cache'), XDG_DATA_DIRS=str(self.root / 'system'),
                        PATH=str(self.bin) + os.pathsep + self.env.get('PATH', ''))

    def command(self, name, code):
        p = self.bin / name
        p.write_text(code)
        p.chmod(0o755)
        return p

    def run_script(self, name, *args, **kwargs):
        return subprocess.run(['python3', '-I', str(SCRIPTS / name), *args],
                              env=self.env, capture_output=True, text=True, timeout=20, **kwargs)

    def theme(self, mode, *args):
        result = self.run_script('desktop-theme.py', mode, *args)
        self.assertEqual(result.returncode, 0, result.stderr)
        return ini(self.cfg / 'kdeglobals')

    def test_dark_light_and_dark_again_keep_dolphin_and_qt_consistent(self):
        for mode in ['dark', 'light', 'dark']:
            with self.subTest(mode=mode):
                palette = self.theme(mode)
                view = palette['Colors:View']
                brightness = lambda value: sum(map(int, value.split(',')))
                if mode == 'dark':
                    self.assertGreater(brightness(view['ForegroundNormal']), brightness(view['BackgroundNormal']))
                else:
                    self.assertLess(brightness(view['ForegroundNormal']), brightness(view['BackgroundNormal']))
                self.assertEqual(ini(self.cfg / 'dolphinrc')['UiSettings']['ColorScheme'], palette['General']['ColorScheme'])
                qt = ini(self.cfg / 'qt6ct/qt6ct.conf')['Appearance']
                self.assertEqual(qt['custom_palette'], 'true')
                colors = ini(qt['color_scheme_path'])['ColorScheme']['active_colors'].split(', ')
                self.assertEqual(len(colors), 22)
                self.assertNotEqual(colors[6], colors[9])

    def test_preserves_fonts_icons_style_other_settings_and_comments(self):
        self.cfg.mkdir()
        (self.cfg / 'kdeglobals').write_text('# User settings\n[General]\nfont=Inter,11\nColorScheme=Old\n\n[Icons]\nTheme=Custom\n[KDE]\nwidgetStyle=Kvantum\n')
        (self.cfg / 'dolphinrc').write_text('[General]\nShowFullPath=true\n[UiSettings]\nColorScheme=Old\n')
        (self.cfg / 'qt6ct').mkdir()
        (self.cfg / 'qt6ct/qt6ct.conf').write_text('[Appearance]\nstyle=kvantum-dark\ncustom_palette=false\n[Fonts]\nfixed=Mono\n')
        self.theme('dark')
        kde = ini(self.cfg / 'kdeglobals')
        self.assertEqual(kde['General']['font'], 'Inter,11')
        self.assertEqual(kde['Icons']['Theme'], 'Custom')
        self.assertEqual(kde['KDE']['widgetStyle'], 'Kvantum')
        self.assertTrue((self.cfg / 'kdeglobals').read_text().startswith('# User settings'))
        self.assertEqual(ini(self.cfg / 'dolphinrc')['General']['ShowFullPath'], 'true')
        self.assertEqual(ini(self.cfg / 'qt6ct/qt6ct.conf')['Appearance']['style'], 'kvantum-dark')
        self.assertEqual(ini(self.cfg / 'qt6ct/qt6ct.conf')['Fonts']['fixed'], 'Mono')
        self.assertEqual((self.cfg / 'qt6ct/qt6ct.conf').read_text().count('custom_palette='), 1)

    def test_repeated_sync_is_idempotent_and_keeps_permissions(self):
        self.theme('dark')
        path = self.cfg / 'kdeglobals'
        path.chmod(0o640)
        before = path.read_bytes(), path.stat().st_mtime_ns
        self.theme('dark')
        self.assertEqual((path.read_bytes(), path.stat().st_mtime_ns), before)
        self.assertEqual(path.stat().st_mode & 0o777, 0o640)

    def test_keep_kde_does_not_overwrite_custom_palette(self):
        self.theme('dark')
        path = self.cfg / 'kdeglobals'
        before = path.read_bytes()
        self.theme('light', '--keep-kde')
        self.assertEqual(path.read_bytes(), before)

    def test_sync_preserves_symlinked_dotfiles(self):
        self.cfg.mkdir()
        target = self.root / 'managed-kdeglobals'
        target.write_text('[General]\nfont=Inter,11\n')
        link = self.cfg / 'kdeglobals'
        link.symlink_to(target)
        self.theme('dark')
        self.assertTrue(link.is_symlink())
        self.assertEqual(ini(target)['General']['font'], 'Inter,11')
        self.assertIn('Dark', ini(target)['General']['ColorScheme'])

    def test_a_palette_the_user_chose_is_left_alone(self):
        self.cfg.mkdir()
        (self.cfg / 'kdeglobals').write_text('[General]\nColorScheme=Nordic\n')
        (self.cfg / 'qt6ct').mkdir()
        (self.cfg / 'qt6ct/qt6ct.conf').write_text('[Appearance]\ncustom_palette=true\ncolor_scheme_path=/home/me/my.conf\n')
        before = (self.cfg / 'kdeglobals').read_bytes(), (self.cfg / 'qt6ct/qt6ct.conf').read_bytes()
        self.theme('dark')
        self.assertEqual(((self.cfg / 'kdeglobals').read_bytes(), (self.cfg / 'qt6ct/qt6ct.conf').read_bytes()), before)
        # Dolphin had no scheme of its own: it still follows light/dark
        self.assertIn('Dark', ini(self.cfg / 'dolphinrc')['UiSettings']['ColorScheme'])

    def test_invalid_custom_scheme_does_not_replace_settings(self):
        self.theme('dark')
        before = (self.cfg / 'kdeglobals').read_bytes()
        result = self.run_script('desktop-theme.py', 'light', '--scheme', str(self.root / 'missing.colors'))
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual((self.cfg / 'kdeglobals').read_bytes(), before)

    def test_match_apps_off_still_syncs_without_running_matugen(self):
        self.command('matugen', '#!/bin/sh\necho unexpected >&2\nexit 99\n')
        result = subprocess.run(['bash', str(SCRIPTS / 'app-theme.sh'), 'off', '#0a84ff', 'off', '', 'gtk,qt', 'dark'], env=self.env, capture_output=True, text=True, timeout=20)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn('unexpected', result.stderr)
        self.assertIn('Dark', ini(self.cfg / 'dolphinrc')['UiSettings']['ColorScheme'])

    @unittest.skipUnless(shutil.which('matugen'), 'matugen is optional')
    def test_generated_accent_palette_and_turning_matching_off(self):
        for mode, source in [('dark', 'accent'), ('light', 'accent'), ('off', 'off')]:
            with self.subTest(mode=mode):
                result = subprocess.run(['bash', str(SCRIPTS / 'app-theme.sh'), mode, '#0a84ff', source, '', 'qt', 'dark' if mode == 'off' else mode], env=self.env, capture_output=True, text=True, timeout=20)
                self.assertEqual(result.returncode, 0, result.stderr)
                scheme = ini(self.cfg / 'dolphinrc')['UiSettings']['ColorScheme']
                self.assertEqual(scheme, 'iRice' if mode != 'off' else 'iRiceDesktopDark')
                qt = ini(self.cfg / 'qt6ct/qt6ct.conf')['Appearance']
                self.assertGreater(Path(qt['color_scheme_path']).stat().st_size, 0)

    def test_multiselect_returns_json_and_single_select_stays_a_string(self):
        files = [str(self.root / "first file's name.txt"), str(self.root / 'second $file.txt')]
        self.command('zenity', '#!/usr/bin/python3\nimport sys\nfiles=' + repr(files) + '\nprint("\\x1f".join(files) if "--multiple" in sys.argv else files[0])\n')
        result = self.run_script('pick-file.py', '--multiple')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout), files)
        result = self.run_script('pick-file.py')
        self.assertEqual(result.stdout.strip(), files[0])

    def test_cancelled_picker_returns_no_selection(self):
        self.command('zenity', '#!/bin/sh\nexit 1\n')
        result = self.run_script('pick-file.py', '--multiple')
        self.assertEqual(result.returncode, 1)
        self.assertEqual(result.stdout, '')

    def test_localsend_sends_multiple_files_and_respects_decline(self):
        sender, receiver = load('localsend'), load('localsend')
        receiver.DOWNLOADS = str(self.root / 'downloads')
        received, sent = [], []
        def incoming(event, **data):
            received.append((event, data))
            if event == 'incoming':
                receiver.sessions[data['session']]['accepted'] = True
                receiver.answers[data['session']].set()
        receiver.emit = incoming
        sender.emit = lambda event, **data: sent.append((event, data))
        receiver.configure_tls(self.root / 'receiver-tls')
        server = receiver.Server(('127.0.0.1', 0), receiver.Handler)
        worker = threading.Thread(target=server.serve_forever, daemon=True)
        worker.start()
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        sender.peers['receiver'] = {'alias': 'Test receiver', 'ip': '127.0.0.1', 'port': server.server_address[1], 'protocol': 'https', 'fingerprint': receiver.FINGERPRINT}
        paths = [self.root / 'first file.txt', self.root / 'second ü.bin']
        for i, path in enumerate(paths):
            path.write_bytes(bytes([65 + i]) * (1048576 + i))
        receiver.state['mode'] = 'everyone'
        sender.send('receiver', [str(p) for p in paths])
        self.assertIn('finished', [event for event, _ in sent])
        self.assertIn('finished', [event for event, _ in received])
        for path in paths:
            self.assertEqual((Path(receiver.DOWNLOADS) / path.name).read_bytes(), path.read_bytes())
        sent.clear()
        receiver.state['mode'] = 'off'
        sender.send('receiver', [str(paths[0])])
        self.assertIn('declined', [event for event, _ in sent])
        self.assertNotIn('finished', [event for event, _ in sent])

if __name__ == '__main__':
    unittest.main()
