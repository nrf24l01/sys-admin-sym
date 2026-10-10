"""Regression checks for verification failures and smoke-test process cleanup."""

import os
from pathlib import Path
import shlex
import shutil
import signal
import subprocess
import sys
import tempfile
import unittest

from smoke_game import failure_in_log, stop_process_group


class SmokeLogTests(unittest.TestCase):
    def test_startup_errors_and_panics_fail_but_software_renderer_warnings_do_not(self):
        for text in (
            "thread 'main' panicked at renderer.rs:4",
            "2026-10-10 ERROR bevy_render: could not create surface",
            "2026-10-10 \x1b[31mERROR\x1b[0m asset_server: missing texture",
        ):
            self.assertTrue(failure_in_log(text), text)
        self.assertFalse(failure_in_log("WARN renderer: software rendering may be slow\nINFO Creating new window"))

    @unittest.skipUnless(hasattr(os, "killpg"), "POSIX process groups required")
    def test_smoke_cleanup_terminates_and_reaps_its_process(self):
        child = subprocess.Popen(
            [sys.executable, "-c", "import time; print('ready', flush=True); time.sleep(30)"],
            start_new_session=True, stdout=subprocess.PIPE, text=True,
        )
        try:
            self.assertEqual(child.stdout.readline().strip(), "ready")
            stop_process_group(child)
            self.assertEqual(child.returncode, -signal.SIGTERM)
        finally:
            if child.poll() is None:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait(timeout=5)
            child.stdout.close()


@unittest.skipUnless(shutil.which("make") and os.name == "posix", "GNU Make on POSIX required")
class MakeChecksTests(unittest.TestCase):
    root = Path(__file__).resolve().parents[1]

    @staticmethod
    def make_environment():
        # Nested Make tests must not inherit the caller's package/tool overrides.
        env = os.environ.copy()
        for name in (
            "MAKEFLAGS", "MFLAGS", "MAKEOVERRIDES", "CARGO", "CARGO_SCOPE",
            "CARGO_TARGET_DIR", "PYTHON", "GAME", "CLIENT", "CHECK_LOGS",
        ):
            env.pop(name, None)
        return env

    def test_run_always_checks_before_launching_and_builds_once(self):
        result = subprocess.run(
            ["make", "--no-print-directory", "-n", "run"],
            cwd=self.root, env=self.make_environment(), capture_output=True, text=True, check=True,
        )
        lines = result.stdout.splitlines()
        ordered = [
            "cargo fmt", "git diff --check", "cargo clippy", "cargo test",
            "-m unittest discover", "cargo build", "tools/smoke_game.py",
        ]
        positions = [next(i for i, line in enumerate(lines) if command in line) for command in ordered]
        self.assertEqual(positions, sorted(positions))
        self.assertEqual(sum("cargo build" in line for line in lines), 1)
        self.assertEqual(lines[-1], '"target/debug/cloud-provider-sim"')

    def test_failed_check_stops_before_tests_build_or_game_launch(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            cargo = root / "check_cargo.py"
            record = root / "commands.txt"
            cargo.write_text(
                "import sys\nfrom pathlib import Path\n"
                f"with Path({str(record)!r}).open('a') as output:\n"
                "    output.write(' '.join(sys.argv[1:]) + '\\n')\n"
                "sys.exit(17 if sys.argv[1] == 'clippy' else 0)\n",
                encoding="utf-8",
            )
            result = subprocess.run(
                ["make", "--no-print-directory", "run",
                 f"CARGO={shlex.quote(sys.executable)} {shlex.quote(str(cargo))}"],
                cwd=self.root, env=self.make_environment(), capture_output=True, text=True,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(record.read_text().splitlines(), [
                "fmt --all -- --check", "clippy --workspace --all-targets --locked -- -D warnings",
            ])
            self.assertNotIn("tools/smoke_game.py", result.stdout)


if __name__ == "__main__":
    unittest.main()
