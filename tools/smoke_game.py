"""Check the real Linux game renderer and authenticated console in an isolated run."""

import argparse
import json
import math
import os
from pathlib import Path
import re
import secrets
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time


class SmokeFailure(RuntimeError):
    pass


def software_environment():
    if not sys.platform.startswith("linux"):
        raise SmokeFailure("The graphical smoke test requires Linux; use make check-core on other platforms")
    for command in ("xvfb-run", "xauth"):
        if shutil.which(command) is None:
            raise SmokeFailure(f"Missing {command}; install xvfb and xauth")
    env = os.environ.copy()
    if not env.get("VK_DRIVER_FILES") and not env.get("VK_ICD_FILENAMES"):
        manifests = sorted(Path("/usr/share/vulkan/icd.d").glob("lvp_icd*.json"))
        if not manifests:
            raise SmokeFailure("Install mesa-vulkan-drivers or set VK_DRIVER_FILES to your Lavapipe manifest")
        env["VK_DRIVER_FILES"] = str(manifests[0])
    env.update(WGPU_BACKEND="vulkan", LIBGL_ALWAYS_SOFTWARE="1", NO_COLOR="1")
    return env


def failure_in_log(text):
    plain = re.sub(r"\x1b\[[0-9;]*m", "", text)
    return "panicked at" in plain or re.search(r"\bERROR\b", plain) is not None


def stop_process_group(process):
    # xvfb-run and its game/X server children belong to our dedicated session.
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait(timeout=5)


def smoke_game(game, client, assets, timeout, logs):
    if not math.isfinite(timeout) or timeout <= 0:
        raise SmokeFailure("Smoke timeout must be positive")
    game, client, assets, logs = game.resolve(), client.resolve(), assets.resolve(), logs.resolve()
    if not assets.is_dir():
        raise SmokeFailure(f"Assets directory not found: {assets}")
    for executable in (game, client):
        if not executable.is_file() or not os.access(executable, os.X_OK):
            raise SmokeFailure(f"Executable not found or not executable: {executable}; run make build")
    env = software_environment()
    logs.mkdir(parents=True, exist_ok=True)
    game_log = logs / "smoke-game.log"
    console_log = logs / "smoke-console.log"
    console_log.write_text("", encoding="utf-8")
    with tempfile.TemporaryDirectory(prefix="cloud-provider-smoke-") as directory:
        workdir = Path(directory)
        # A real executable beside real assets exercises the portable package layout.
        # Container-built binaries cannot rely on their compiled source-tree path.
        staged_game = workdir / "cloud-provider-sim"
        try:
            os.link(game, staged_game)
        except OSError:
            shutil.copy2(game, staged_game)
        (workdir / "assets").symlink_to(assets, target_is_directory=True)
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            port = listener.getsockname()[1]
        password = secrets.token_hex(24)
        settings = workdir / "cloud-provider-settings.json"
        settings.write_text(json.dumps({
            "host": "127.0.0.1", "port": port, "password": password, "language": "en",
        }), encoding="utf-8")
        settings.chmod(0o600)
        client_env = {**env, "GAME_SSH_PASSWORD": password}
        with game_log.open("w", encoding="utf-8") as output:
            process = subprocess.Popen(
                ["xvfb-run", "-a", "-s", "-screen 0 1600x1000x24", str(staged_game)],
                cwd=workdir, env=env, stdout=output, stderr=subprocess.STDOUT,
                start_new_session=True,
            )
            try:
                deadline = time.monotonic() + timeout
                ready_since = None
                while time.monotonic() < deadline:
                    if process.poll() is not None:
                        raise SmokeFailure(f"Game exited before startup completed (status {process.returncode})")
                    text = game_log.read_text(encoding="utf-8", errors="replace")
                    if failure_in_log(text):
                        raise SmokeFailure("Game reported a panic or rendering/startup error")
                    try:
                        response = subprocess.run(
                            [str(client), "--host", "127.0.0.1", "--port", str(port), "--list"],
                            cwd=workdir, env=client_env, capture_output=True, text=True,
                            timeout=min(3.0, max(0.1, deadline - time.monotonic())),
                        )
                        console_log.write_text(response.stdout + response.stderr, encoding="utf-8")
                        connected = response.returncode == 0 and response.stdout.startswith("ID\tTYPE\t")
                    except subprocess.TimeoutExpired:
                        connected = False
                    if connected and "Creating new window" in text:
                        if ready_since is None:
                            ready_since = time.monotonic()
                        # Keep renderer and worker alive/responding through several redraws.
                        if time.monotonic() - ready_since >= 3.0:
                            if process.poll() is not None or failure_in_log(game_log.read_text(encoding="utf-8", errors="replace")):
                                raise SmokeFailure("Game failed after opening its window")
                            print(f"Game window and authenticated console passed. Logs: {logs}")
                            return
                    else:
                        ready_since = None
                    time.sleep(0.25)
                raise SmokeFailure(f"Game window and authenticated console were not ready within {timeout:g}s")
            finally:
                stop_process_group(process)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--game", type=Path, default=Path("target/debug/cloud-provider-sim"))
    parser.add_argument("--client", type=Path, default=Path("target/debug/game-ssh"))
    parser.add_argument("--assets", type=Path, default=Path("assets"))
    parser.add_argument("--timeout", type=float, default=30)
    parser.add_argument("--logs", type=Path, default=Path("target/check-logs"))
    args = parser.parse_args()
    try:
        smoke_game(args.game, args.client, args.assets, args.timeout, args.logs)
    except (SmokeFailure, OSError, subprocess.SubprocessError) as error:
        print(f"Smoke test failed: {error}\nLogs: {args.logs.resolve()}", file=sys.stderr)
        return 1
    return 0


def interrupted(signum, _frame):
    raise SystemExit(128 + signum)


if __name__ == "__main__":
    signal.signal(signal.SIGTERM, interrupted)
    sys.exit(main())
