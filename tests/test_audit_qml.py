"""Offscreen integration checks using iRice's Quickshell build when installed."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
BUNDLE = Path.home()/'.local/share/irice/quickshell'
QS = BUNDLE/'bin/qs'


@unittest.skipUnless(QS.exists(), 'requires bundled Quickshell')
class QmlAuditTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='irice-audit-test-')
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name)
        self.env = dict(os.environ, QT_QPA_PLATFORM='offscreen', XDG_RUNTIME_DIR=str(self.base/'runtime'), XDG_STATE_HOME=str(self.base/'state'), XDG_CONFIG_HOME=str(self.base/'config'), XDG_CACHE_HOME=str(self.base/'cache'), QML_IMPORT_PATH=str(BUNDLE/'lib/qt6/qml'))
        (self.base/'runtime').mkdir(mode=0o700)

    def copy(self, home, file):
        target = home/file
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(ROOT/'shell'/file, target)

    def launch(self, home, code):
        (home/'shell.qml').write_text('import QtQuick\nimport Quickshell\nimport qs.common\nimport qs.services\nShellRoot { settings.watchFiles: false\n'+code+'\n}\n')
        proc = subprocess.Popen([str(QS), '-p', str(home)], env=self.env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        def cleanup():
            if proc.poll() is None: proc.kill()
            proc.communicate()
        self.addCleanup(cleanup)
        return proc

    def finish(self, proc):
        try: out, err = proc.communicate(timeout=8)
        except subprocess.TimeoutExpired:
            proc.kill(); out, err = proc.communicate()
            self.fail('QML timed out:\n'+out+err)
        self.assertEqual(proc.returncode, 0, out+err)
        self.assertNotIn('ERROR', out+err, out+err)
        self.assertNotIn('ReferenceError', out+err, out+err)
        self.assertNotIn('TypeError', out+err, out+err)
        return out+err

    def state_home(self, name):
        home=self.base/name
        self.copy(home, Path('common/UiState.qml'))
        self.copy(home, Path('assets/scripts/state-merge.py'))
        (home/'services').mkdir()
        (home/'services/Stub.qml').write_text('import QtQuick\nQtObject {}')
        return home

    def test_immediate_quit_recovers_journal_and_private_state(self):
        home=self.state_home('quick')
        self.finish(self.launch(home, '''
Timer { interval: 30; repeat: true; running: true; onTriggered: {
    if (!UiState.loaded) return; stop(); UiState.data.desktopNote = "durable"; quit.start();
}}
Timer { id: quit; interval: 20; onTriggered: Qt.quit() }
'''))
        state=self.base/'state/irice'
        subprocess.run(['python3',str(home/'assets/scripts/state-merge.py'),str(state)],check=True)
        self.assertEqual(json.loads((state/'state.json').read_text())['desktopNote'], 'durable')
        self.assertEqual((state/'state.json').stat().st_mode & 0o777, 0o600)

    def test_two_shells_merge_unrelated_edits(self):
        procs=[]
        for name, delay, mutation in [('a',300,'UiState.data.desktopNote = "note"'),('b',350,'UiState.data.todos = [{text:"todo", done:false}]')]:
            procs.append(self.launch(self.state_home(name), '''
Timer { interval: 30; repeat: true; running: true; onTriggered: {
    if (!UiState.loaded) return; stop(); edit.start(); quit.start();
}}
Timer { id: edit; interval: '''+str(delay)+'''; onTriggered: {'''+mutation+''';} }
Timer { id: quit; interval: 1100; onTriggered: Qt.quit() }
'''))
        for proc in procs: self.finish(proc)
        saved=json.loads((self.base/'state/irice/state.json').read_text())
        self.assertEqual(saved['desktopNote'],'note'); self.assertEqual(saved['todos'][0]['text'],'todo')

    def test_notes_only_report_success_after_write(self):
        home=self.base/'notes'
        for f in ['services/Notes.qml','assets/scripts/save-note.py']: self.copy(home, Path(f))
        (home/'common').mkdir()
        (home/'common/Config.qml').write_text('pragma Singleton\nimport QtQuick\nimport Quickshell\nSingleton { property var options: ({notes:{folder:'+json.dumps(str(self.base/'notes-saved'))+'}}) }')
        (home/'common/Theme.qml').write_text('pragma Singleton\nimport QtQuick\nimport Quickshell\nSingleton { property var accents: ({yellow:"yellow",red:"red"}) }')
        (home/'services/Island.qml').write_text('pragma Singleton\nimport Quickshell\nSingleton { function post(o) { console.log(o.title); } }')
        out=self.finish(self.launch(home, '''
property int savedCount: 0
Connections { target: Notes; function onSaved(path) { savedCount++; if (savedCount===2) Qt.quit(); } }
Timer { interval: 100; running: true; onTriggered: { Notes.create("Same title",false); Notes.create("Same title",false); } }
'''))
        self.assertEqual(out.count('Note saved'),2)
        paths=list((self.base/'notes-saved').glob('*.md'))
        self.assertEqual(len(paths),2)
        self.assertTrue(all(p.read_text()=='# Same title\n' for p in paths))


if __name__ == '__main__': unittest.main()
