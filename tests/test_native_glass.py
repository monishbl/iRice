"""Exercise the real sender class against a disposable compositor IPC socket."""
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import threading
import unittest

ROOT = Path(__file__).resolve().parents[1]


@unittest.skipUnless(shutil.which('g++'), 'requires g++')
class NativeGlassTests(unittest.TestCase):
    def test_failed_ack_is_retried_without_another_frame(self):
        with tempfile.TemporaryDirectory(prefix='irice-glass-ipc-') as d:
            p=Path(d)
            # Compile the production sender separately from the Qt scene graph.
            source=(ROOT/'shell/native/glasssync.cpp').read_text().split('\nvoid appendNumber(')[0]
            source='\n'.join(line for line in source.splitlines() if not line.startswith(('#include "glasssync.h"', '#include <Q')))
            source+='\n}\nint main() { GlassSender::get().submit("window", "fixture-command"); std::this_thread::sleep_for(std::chrono::milliseconds(1400)); }\n'
            (p/'sender.cpp').write_text(source)
            subprocess.run(['g++','-std=c++20','-pthread',str(p/'sender.cpp'),'-o',str(p/'sender')],check=True,capture_output=True,timeout=30)
            directory=p/'hypr/fixture'; directory.mkdir(parents=True)
            server=socket.socket(socket.AF_UNIX,socket.SOCK_STREAM)
            self.addCleanup(server.close)
            server.bind(str(directory/'.socket.sock')); server.listen(); server.settimeout(3)
            received=[]
            errors=[]
            def serve():
                try:
                    for response in [b'temporary error',b'ok']:
                        connection,_=server.accept()
                        with connection:
                            received.append(connection.recv(4096))
                            connection.sendall(response)
                except Exception as error: errors.append(error)
            worker=threading.Thread(target=serve); worker.start()
            result=subprocess.run([str(p/'sender')],env=dict(os.environ,XDG_RUNTIME_DIR=d,HYPRLAND_INSTANCE_SIGNATURE='fixture'),capture_output=True,timeout=7)
            worker.join(4)
            self.assertEqual(result.returncode,0,result.stderr)
            self.assertEqual(errors,[])
            self.assertEqual(received,[b'fixture-command',b'fixture-command'])
            server.settimeout(.05)
            with self.assertRaises(TimeoutError): server.accept()


if __name__ == '__main__': unittest.main()
