"""The glass request sender (shell/native/glasssender.hpp) against a stand-in
for Hyprland's request socket: what it retries, what it gives up on, and that
it still sends what's waiting when the shell exits."""
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import threading
import time
import unittest

ROOT = Path(__file__).resolve().parents[1]

# A tiny program around the real header: submits the commands it's given, waits,
# then exits (the sender's destructor sends whatever is still waiting)
DRIVER = r'''
#include "glasssender.hpp"
#include <cstdlib>
#include <thread>
int main(int argc, char** argv) {
    irice::GlassSender sender;
    for (int i = 2; i < argc; i++)
        sender.submit(argv[i], argv[i]);
    std::this_thread::sleep_for(std::chrono::milliseconds(std::atoi(argv[1])));
}
'''


@unittest.skipUnless(shutil.which('g++'), 'requires g++')
class GlassSenderTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory(prefix='irice-glass-ipc-')
        d = Path(cls.tmp.name)
        (d / 'driver.cpp').write_text(DRIVER)
        cls.binary = d / 'driver'
        subprocess.run(['g++', '-std=c++20', '-pthread', '-I', str(ROOT / 'shell/native'), str(d / 'driver.cpp'), '-o', str(cls.binary)],
                       check=True, capture_output=True, timeout=60)

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()

    def setUp(self):
        self.run_dir = tempfile.TemporaryDirectory(prefix='irice-glass-run-')
        self.addCleanup(self.run_dir.cleanup)
        self.socket_dir = Path(self.run_dir.name) / 'hypr/fixture'
        self.socket_dir.mkdir(parents=True)

    def listen(self, replies):
        """Answer each connection with the next reply; record what was sent."""
        server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.addCleanup(server.close)
        server.bind(str(self.socket_dir / '.socket.sock'))
        server.listen()
        server.settimeout(0.2)
        received = []
        stop = threading.Event()

        def serve():
            replies_left = list(replies)
            while not stop.is_set():
                try:
                    connection, _ = server.accept()
                except (TimeoutError, OSError):
                    continue
                with connection:
                    received.append(connection.recv(4096).decode())
                    connection.sendall(replies_left.pop(0) if replies_left else b'ok')

        worker = threading.Thread(target=serve)
        worker.start()
        self.addCleanup(lambda: (stop.set(), worker.join(2)))
        return received

    def send(self, wait_ms, *commands):
        env = dict(os.environ, XDG_RUNTIME_DIR=self.run_dir.name, HYPRLAND_INSTANCE_SIGNATURE='fixture')
        result = subprocess.run([str(self.binary), str(wait_ms), *commands], env=env, capture_output=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_a_refused_request_is_not_sent_again(self):
        received = self.listen([b'unknown request'])
        self.send(1500, 'shape')
        time.sleep(0.3)
        self.assertEqual(received, ['shape'])

    def test_an_unreachable_socket_is_tried_again_until_it_answers(self):
        # No socket at first: the first attempts fail to connect
        def late_listen():
            time.sleep(0.7)
            self.received = self.listen([])
        starter = threading.Thread(target=late_listen)
        starter.start()
        self.send(2500, 'shape')
        starter.join()
        time.sleep(0.3)
        self.assertEqual(self.received, ['shape'])

    def test_it_gives_up_on_a_socket_that_never_appears(self):
        started = time.monotonic()
        self.send(4500, 'shape')  # 0.5 + 1 + 2 s of retries fit in this wait
        self.assertLess(time.monotonic() - started, 6)

    def test_what_is_waiting_at_exit_is_still_sent(self):
        received = self.listen([])
        self.send(0, 'clear-a', 'clear-b')
        time.sleep(0.3)
        sent = ';'.join(received)
        self.assertIn('clear-a', sent)
        self.assertIn('clear-b', sent)


if __name__ == '__main__':
    unittest.main()
