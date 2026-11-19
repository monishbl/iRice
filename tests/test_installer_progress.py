"""The classic installer's screen (tools/installer/progress.py): when it's
used (plain lines otherwise), the commands' output into the log and not on
the terminal, what a failed step shows, and install.sh's own functions
driving it in a real pseudo-terminal."""
import importlib.util
import os
import pty
import re
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("progress", ROOT / "tools/installer/progress.py")
progress = importlib.util.module_from_spec(spec)
spec.loader.exec_module(progress)

ANSI = re.compile(r"\x1b(\[[0-9;?]*[ -/]*[@-~]|[()][0-9A-Za-z]|[=>78])")


def ctl(*f):
    return b"\x1eUI\t" + "\t".join(f).encode() + b"\n"


class Clock:
    def __init__(self):
        self.t = 100.0

    def __call__(self):
        return self.t


class Mode(unittest.TestCase):
    def test_plain_unless_a_real_terminal(self):
        self.assertEqual(progress.choose_mode(False, True, "xterm-kitty", True, 120, 40)[0], "ui")
        self.assertEqual(progress.choose_mode(False, True, "linux", True, 80, 25)[0], "ui")
        self.assertEqual(progress.choose_mode(True, True, "xterm-kitty", True, 120, 40), ("plain", "--plain"))
        self.assertEqual(progress.choose_mode(False, False, "xterm-kitty", True, 120, 40)[0], "plain")  # piped (the full-screen installer)
        self.assertEqual(progress.choose_mode(False, True, "dumb", True, 120, 40)[0], "plain")
        self.assertEqual(progress.choose_mode(False, True, "", True, 120, 40)[0], "plain")
        self.assertEqual(progress.choose_mode(False, True, "foot", False, 120, 40)[0], "plain")  # no /dev/tty
        self.assertEqual(progress.choose_mode(False, True, "foot", True, 30, 10)[0], "plain")

    def test_mode_command(self):
        run = lambda *a, **env: subprocess.run([sys.executable, "-I", str(ROOT / "tools/installer/progress.py"), "mode", *a],
                                               capture_output=True, text=True, env=dict(os.environ, **env)).stdout.strip()
        self.assertEqual(run("--plain", "--stdout-tty", "1", TERM="xterm"), "plain")
        self.assertEqual(run("--stdout-tty", "0", TERM="xterm"), "plain")
        self.assertEqual(run("--stdout-tty", "1", TERM="dumb"), "plain")

    def test_linux_console_gets_plain_characters(self):
        self.assertIs(progress.glyphs_for("linux", {"LANG": "en_US.UTF-8"}), progress.ASCII)
        self.assertIs(progress.glyphs_for("xterm-kitty", {"LANG": "C"}), progress.ASCII)
        self.assertIs(progress.glyphs_for("foot", {"LANG": "en_GB.UTF-8"}), progress.UTF8)


class Stream(unittest.TestCase):
    def setUp(self):
        self.d = tempfile.TemporaryDirectory()
        self.log_path = os.path.join(self.d.name, "install.log")
        self.log = open(self.log_path, "ab")
        self.clock = Clock()
        self.m = progress.Model(self.log_path, self.log, now=self.clock)

    def tearDown(self):
        self.log.close()
        self.d.cleanup()

    def logged(self):
        self.log.flush()
        return Path(self.log_path).read_text()

    def test_output_goes_to_the_log_and_marks_dont(self):
        m = self.m
        m.feed(ctl("plan", "Checking your system", "2", "Installing packages", "30"))
        m.feed(ctl("step", "1") + b"resolving dependencies...\n" + ctl("msg", "note", "System: Arch Linux"))
        # (a mark split between two reads, and a line without its end before it)
        data = b"half a line" + ctl("done", "1") + ctl("step", "2") + b"( 3/90) installing hyprland\n"
        m.feed(data[:15])
        m.feed(data[15:])
        log = self.logged()
        self.assertIn("resolving dependencies...", log)
        self.assertIn("  System: Arch Linux", log)
        self.assertIn("half a line", log)
        self.assertNotIn("\x1e", log)
        self.assertNotIn("UI\t", log)
        self.assertEqual([s["state"] for s in m.steps], ["ok", "run"])
        self.assertEqual(m.pacman["n"], 3)
        self.assertEqual(m.activity(), "Installing hyprland (3 of 90)")
        # the bar: step 1's weight, then pacman's count moves it within step 2
        self.assertGreater(m.fraction(), 2 / 32)
        self.assertLess(m.fraction(), 0.5)

    def test_carriage_returns_and_colours(self):
        m = self.m
        m.feed(b"\x1b[1mDownloading\x1b[0m 10%\rDownloading 50%\rDownloading 100%\nnext\n")
        self.assertEqual(list(m.tail), ["Downloading 100%", "next"])
        self.assertIn("\x1b[1m", self.logged())  # (the log keeps it as it came)

    def test_build_lines_move_the_bar_but_stay_out_of_the_view(self):
        m = self.m
        m.feed(ctl("plan", "Building Tessera", "1") + ctl("step", "1"))
        m.feed(b'BUILD {"phase": "compile", "step": "Compiling slint (212 of 431)", "fraction": 0.5}\n')
        self.assertEqual(m.activity(), "Compiling slint (212 of 431)")
        self.assertAlmostEqual(m.fraction(), 0.5)
        self.assertFalse(any("BUILD" in t for t in m.tail))
        # (never backwards, when the next build starts at 0)
        m.feed(b'BUILD {"phase": "compile", "step": "Compiling glass", "fraction": 0.1}\n')
        self.assertAlmostEqual(m.fraction(), 0.5)

    def test_pause_resume_quit_are_handed_to_the_screen(self):
        acts = self.m.feed(ctl("pause", "Continue with Hyprland 0.57 anyway?") + ctl("resume") + ctl("quit"))
        self.assertEqual(acts, ["pause", "resume", "quit"])
        self.assertTrue(self.m.finished)

    def failed(self):
        m = self.m
        m.feed(ctl("plan", "Checking your system", "2", "Installing packages", "30", "Login", "4"))
        m.feed(ctl("step", "1") + ctl("done", "1") + ctl("step", "2"))
        m.feed(b"error: failed retrieving file 'mesa-25.pkg.tar.zst' from mirror.example\nerror: failed to commit transaction\n")
        m.feed(ctl("msg", "say", "PulseAudio is installed, and PipeWire (what Tessera uses) can't be installed next to it."))
        m.feed(ctl("msg", "warn", "foo isn't in your repositories (left out)"))
        m.feed(ctl("fail", "2", "Fix the problem above, then retry (r), skip it (s) or stop (a)."))
        return m

    def test_a_failed_step_says_what_happened_how_to_fix_it_and_where_the_log_is(self):
        m = self.failed()
        text = "\n".join(progress.render(m, 100, 40, g=progress.UTF8, color=False))
        self.assertIn("✓ Checking your system", text)
        self.assertIn("✗ Installing packages", text)
        self.assertIn("PulseAudio is installed", text)
        self.assertIn("From the log:", text)
        self.assertIn("error: failed to commit transaction", text)
        self.assertIn("! foo isn't in your repositories", text)
        self.assertIn("then retry (r), skip it (s) or stop (a)", text)
        self.assertIn("Full log: " + self.log_path, text)
        self.assertIn("· Login", text)

    def test_the_stop_says_why(self):
        m = self.failed()
        m.feed(ctl("error", "Stopped at: Installing packages.") + ctl("quit"))
        text = "\n".join(progress.render(m, 100, 40, color=False))
        self.assertIn("Stopped", text.splitlines()[0])
        self.assertIn("✗ Stopped at: Installing packages.", text)
        self.assertIn("✗ Stopped at: Installing packages.", self.logged())

    def test_stream_ending_without_quit_marks_the_step(self):
        m = self.m
        m.feed(ctl("plan", "A", "1", "B", "1") + ctl("step", "1") + b"partial")
        m.end()
        self.assertTrue(m.stopped)
        self.assertEqual(m.steps[0]["state"], "fail")
        self.assertIn("partial", list(m.tail))

    def test_fits_the_terminal(self):
        m = self.failed()
        for w, h in ((40, 12), (80, 24), (200, 60)):
            lines = progress.render(m, w, h, color=True)
            self.assertLessEqual(len(lines), h - 1, (w, h))
            for l in lines:
                self.assertLess(progress.cwidth(ANSI.sub("", l)), w, (w, l))
        # many steps done fold into one line on a short terminal
        m2 = progress.Model(self.log_path, None, now=self.clock)
        m2.feed(ctl("plan", *[x for i in range(15) for x in ("Step %d" % i, "1")]))
        for i in range(1, 15):
            m2.feed(ctl("step", str(i)) + ctl("done", str(i)))
        m2.feed(ctl("step", "15"))
        text = "\n".join(progress.render(m2, 80, 16, color=False))
        self.assertIn("steps done", text)
        self.assertIn("Step 14", text)

    def test_details_follow_and_scroll(self):
        m = self.m
        m.feed(b"".join(b"line %d\n" % i for i in range(100)))
        lines, off = progress.render_details(m, 80, 12, 0, color=False)
        self.assertEqual(len(lines), 12)
        self.assertIn("following", lines[0])
        self.assertEqual(lines[-2], "line 99")
        lines, off = progress.render_details(m, 80, 12, 30, color=False)
        self.assertEqual(lines[-2], "line 69")
        self.assertIn("30 lines up", lines[0])
        lines, off = progress.render_details(m, 80, 12, 10 ** 9, color=False)
        self.assertEqual(lines[1], "line 0")

    def test_keys(self):
        self.assertEqual(progress.parse_keys(b"d\x1b[A\x1b[5~\x1b[6~\x1bq\x1b[F"), ["d", "up", "pgup", "pgdn", "esc", "q", "end"])
        self.assertEqual(progress.parse_keys(b"\x1b"), ["esc"])


# install.sh's own functions for the screen, run in a pseudo-terminal
def install_functions():
    """From screen_send to the end of the sudo wrapper (the screen's
    functions, say, note, warn, die, its trap), and step"""
    text = (ROOT / "install.sh").read_text()
    start = text.index("\nscreen_send() {") + 1
    end = text.index('    command sudo "$@"\n}\n', start) + len('    command sudo "$@"\n}\n')
    step = re.search(r"^step\(\) \{.*?^\}\n", text, re.M | re.S).group(0)
    return text[start:end].replace("trap screen_stop EXIT\n", "") + step


def harness(body, screen):
    funcs = install_functions()
    return f"""set -Euo pipefail
ROOT={ROOT}; LOG="$T/install.log"; DRY=0; YES=1; STEP=0; TOTAL=2; SCREEN=0; SCREEN_WANTED={screen}; SCREEN_DIR=""
B="" DIM="" RED="" GRN="" YEL="" BLU="" R=""
{funcs}
exec 3>&1 4>&2
trap screen_stop EXIT
{body}
"""


def in_pty(script, tmp, timeout=30):
    path = os.path.join(tmp, "run.sh")
    Path(path).write_text(script)
    env = dict(os.environ, T=tmp, TERM="xterm-256color", LANG="C.UTF-8", LC_ALL="C.UTF-8", COLUMNS="100", LINES="30")
    env.pop("NO_COLOR", None)
    pid, fd = pty.fork()
    if pid == 0:
        try:
            import fcntl
            import struct
            import termios
            fcntl.ioctl(0, termios.TIOCSWINSZ, struct.pack("HHHH", 30, 100, 0, 0))
            os.execvpe("bash", ["bash", path], env)
        finally:
            os._exit(127)
    out = b""
    end = time.monotonic() + timeout
    while time.monotonic() < end:
        try:
            chunk = os.read(fd, 65536)
        except OSError:
            break
        if not chunk:
            break
        out += chunk
    _, status = os.waitpid(pid, 0)
    os.close(fd)
    return os.waitstatus_to_exitcode(status), out.decode("utf-8", "replace")


NOISY = """
one() { local i; for i in $(seq 1 200); do echo "NOISE $i"; done; say "All 200 lines written"; }
two() { echo "error: RAW-FAILURE happened"; say "What went wrong, in words"; return 1; }
"""


class InPty(unittest.TestCase):
    def test_screen_keeps_the_noise_in_the_log(self):
        with tempfile.TemporaryDirectory() as t:
            code, out = in_pty(harness(NOISY + """
screen_start "First" 1 "Second" 1
step "First" one
step "Second" true
screen_stop
echo "AFTER THE SCREEN"
sleep 0.5  # (tee writes it; bash is the pty's session leader here, and its end hangs tee up)
""", 1), t)
            log = Path(t, "install.log").read_text()
            text = ANSI.sub("", out)
            self.assertEqual(code, 0, text)
            self.assertIn("✓ First", text)
            self.assertIn("✓ Second", text)
            self.assertIn("100%", text)
            self.assertIn("AFTER THE SCREEN", text)
            # (at most a glimpse as the step's activity line, never the stream)
            self.assertLess(text.count("NOISE"), 20, text)
            for i in (1, 100, 200):
                self.assertIn("NOISE %d\n" % i, log)
            self.assertIn("All 200 lines written", log)
            self.assertIn("[1/2] First", log)

    def test_screen_shows_the_failure(self):
        with tempfile.TemporaryDirectory() as t:
            code, out = in_pty(harness(NOISY + """
screen_start "First" 1 "Second" 1
step "First" true
step "Second" two
echo "NOT REACHED"
""", 1), t)
            text = ANSI.sub("", out)
            self.assertEqual(code, 1, text)
            self.assertIn("✗ Second", text)
            self.assertIn("What went wrong, in words", text)
            self.assertIn("RAW-FAILURE happened", text)
            self.assertIn("Stopped at: Second.", text)
            self.assertIn("Full log: ", text)
            self.assertNotIn("NOT REACHED", text)

    def test_plain_prints_everything(self):
        with tempfile.TemporaryDirectory() as t:
            code, out = in_pty(harness(NOISY + """
step "First" one
echo "AFTER"
""", 0), t)
            text = ANSI.sub("", out)
            self.assertEqual(code, 0, text)
            self.assertIn("[1/2] First", text)
            self.assertIn("NOISE 200", text)
            self.assertIn("All 200 lines written", text)


if __name__ == "__main__":
    unittest.main()
