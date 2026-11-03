"""The welcome screen shows once, on a new setup: Config's one-time
migration marks settings saved before it as welcomed, and leaves a fresh
config (written from the defaults, or by install.sh) to show it. Runs the real
Config.qml offscreen with iRice's Quickshell build when it's installed."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

# The newest one-time settings change (Config.qml onLoaded: defaultsVersion)
LATEST = 3

ROOT = Path(__file__).resolve().parents[1]
BUNDLE = Path.home()/'.local/share/irice/quickshell'
QS = BUNDLE/'bin/qs'

# Prints welcome.done (and the defaults version) half a second after the
# settings are ready (a fresh file is written, then read back), then quits
PROBE = '''
Timer {
    interval: 50; repeat: true; running: true
    onTriggered: { if (!Config.ready) return; stop(); report.start(); }
}
Timer {
    id: report; interval: 600
    onTriggered: { console.log("WELCOME " + Config.options.welcome.done + " " + Config.options.defaultsVersion); Qt.quit(); }
}
'''


@unittest.skipUnless(QS.exists(), 'requires bundled Quickshell')
class WelcomeMigration(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='irice-welcome-test-')
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        (self.base/'runtime').mkdir(mode=0o700)
        self.config = self.base/'config/irice/config.json'
        self.env = dict(os.environ, QT_QPA_PLATFORM='offscreen', XDG_RUNTIME_DIR=str(self.base/'runtime'), XDG_STATE_HOME=str(self.base/'state'), XDG_CONFIG_HOME=str(self.base/'config'), XDG_CACHE_HOME=str(self.base/'cache'), QML_IMPORT_PATH=str(BUNDLE/'lib/qt6/qml'))

    def run_config(self):
        home = self.base/'shell'
        (home/'common').mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT/'shell/common/Config.qml', home/'common/Config.qml')
        (home/'shell.qml').write_text('import QtQuick\nimport Quickshell\nimport qs.common\nShellRoot { settings.watchFiles: false\n' + PROBE + '\n}\n')
        try:
            p = subprocess.run([str(QS), '-p', str(home)], env=self.env, capture_output=True, text=True, timeout=10)
        except subprocess.TimeoutExpired as e:
            self.fail('QML timed out:\n' + str(e.stdout) + str(e.stderr))
        out = p.stdout + p.stderr
        self.assertNotIn('ReferenceError', out, out)
        self.assertNotIn('TypeError', out, out)
        line = [l for l in out.splitlines() if 'WELCOME ' in l]
        self.assertTrue(line, out)
        done, version = line[-1].split('WELCOME ', 1)[1].split()
        return done == 'true', int(version)

    def write(self, data):
        self.config.parent.mkdir(parents=True, exist_ok=True)
        self.config.write_text(json.dumps(data, indent=4))

    def test_settings_saved_before_it_count_as_welcomed(self):
        self.write({'appearance': {'mode': 'dark'}, 'defaultsVersion': 1})
        self.assertEqual(self.run_config(), (True, LATEST))

    def test_a_new_setup_shows_it(self):
        # (no file: written from the defaults, welcome.done false; the
        # migration sees that file at the next start and leaves it)
        self.assertFalse(self.run_config()[0])
        self.assertFalse(json.loads(self.config.read_text())['welcome']['done'])
        self.assertEqual(self.run_config(), (False, LATEST))

    def test_installs_first_config_shows_it(self):
        # (what install.sh writes on a first install)
        self.write({'appearance': {'mode': 'auto'}, 'welcome': {'done': False}, 'wallpaper': {'dynamic': 'appearance'}})
        self.assertEqual(self.run_config(), (False, LATEST))

    def test_finished_stays_finished(self):
        self.write({'welcome': {'done': True}, 'defaultsVersion': 2})
        self.assertEqual(self.run_config(), (True, LATEST))


if __name__ == '__main__':
    unittest.main()
