#!/usr/bin/env python3
"""Record the ConfigLens demo cast on a real pty.

asciinema only streams its timeline in real time when it is itself attached to a terminal. Driven
from a non-interactive shell it buffers the whole session and writes every event at the same
timestamp, producing a cast that plays back instantly. This driver gives it a pty.
"""
import os
import pty
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CAST = os.path.join("docs", "demo", "configlens-demo.cast")
COMMAND = "DEMO_PAUSE=1.2 bash demo/record.sh"
ARGV = [
    "asciinema",
    "rec",
    "--cols",
    "100",
    "--rows",
    "30",
    "--idle-time-limit",
    "1.5",
    "--command",
    COMMAND,
    CAST,
    "--overwrite",
]

pid, fd = pty.fork()
if pid == 0:  # child: the recording session owns the pty
    os.chdir(ROOT)
    os.environ["PATH"] = os.path.expanduser("~/.local/bin") + os.pathsep + os.environ["PATH"]
    os.execvp(ARGV[0], ARGV)

while True:
    try:
        chunk = os.read(fd, 65536)
    except OSError:
        break
    if not chunk:
        break
    sys.stdout.write(chunk.decode("utf-8", "replace"))
    sys.stdout.flush()

_, status = os.waitpid(pid, 0)
sys.exit(0 if status == 0 else 1)
