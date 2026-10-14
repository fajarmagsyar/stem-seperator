"""Launcher environment regression tests; no GUI is opened."""
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import unittest

LAUNCHER = Path(__file__).resolve().parents[1] / "tools/launch-linux.sh"

class LauncherTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.runtime = self.root / "session"
        self.runtime.mkdir(mode=0o700)
        shutil.copyfile(LAUNCHER, self.root / "start.sh")
        binary = self.root / "stem-desktop"
        binary.write_text('#!/usr/bin/env python3\nimport json,os,sys\nprint(json.dumps({"wayland":os.getenv("WAYLAND_DISPLAY"),"display":os.getenv("DISPLAY"),"runtime":os.getenv("XDG_RUNTIME_DIR"),"bus":os.getenv("DBUS_SESSION_BUS_ADDRESS"),"args":sys.argv[1:]}))\n')
        binary.chmod(0o700)
        self.sockets = []

    def tearDown(self):
        for handle in self.sockets:
            handle.close()
        self.temp.cleanup()

    def bind(self, name):
        handle = socket.socket(socket.AF_UNIX)
        handle.bind(str(self.runtime / name))
        self.sockets.append(handle)

    def run_launcher(self, extra=None):
        environment = os.environ.copy()
        for key in ("DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET", "DBUS_SESSION_BUS_ADDRESS"):
            environment.pop(key, None)
        environment["XDG_RUNTIME_DIR"] = str(self.runtime)
        environment.update(extra or {})
        return subprocess.run(["bash", str(self.root / "start.sh"), "argument with spaces"], env=environment, text=True, capture_output=True)

    def test_discovers_single_owned_wayland_socket_and_bus(self):
        self.bind("wayland-1")
        self.bind("bus")
        (self.runtime / "wayland-1.lock").touch()
        result = self.run_launcher()
        self.assertEqual(result.returncode, 0, result.stderr)
        values = json.loads(result.stdout)
        self.assertEqual(values["wayland"], "wayland-1")
        self.assertEqual(values["bus"], f"unix:path={self.runtime}/bus")
        self.assertEqual(values["args"], ["argument with spaces"])

    def test_preserves_explicit_display_and_bus(self):
        self.bind("wayland-1")
        result = self.run_launcher({"DISPLAY": ":7", "DBUS_SESSION_BUS_ADDRESS": "unix:path=/explicit"})
        values = json.loads(result.stdout)
        self.assertEqual(values["display"], ":7")
        self.assertIsNone(values["wayland"])
        self.assertEqual(values["bus"], "unix:path=/explicit")

    def test_missing_session_has_actionable_error(self):
        result = self.run_launcher()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("desktop terminal", result.stderr)
        self.assertEqual(result.stdout, "")

    def test_ambiguous_session_requires_explicit_selection(self):
        self.bind("wayland-0")
        self.bind("wayland-1")
        result = self.run_launcher()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("WAYLAND_DISPLAY", result.stderr)
        self.assertEqual(result.stdout, "")

if __name__ == "__main__":
    unittest.main()
