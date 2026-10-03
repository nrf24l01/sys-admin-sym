"""Exercise the actual CLI through a PTY and a small authenticated console fixture."""
import fcntl
import json
import os
from pathlib import Path
import pty
import queue
import select
import socketserver
import subprocess
import sys
import tempfile
import termios
import threading
import time


commands = queue.Queue()


class ConsoleFixture(socketserver.StreamRequestHandler):
    def handle(self):
        envelope = json.loads(self.rfile.readline())
        assert envelope["password"] == "fixture-password"
        request = envelope["request"]
        if "Connect" in request:
            response = {"Connected": {"device": 1, "prompt": "fixture$"}}
        elif "Run" in request:
            command = request["Run"]["input"].strip()
            commands.put(command)
            response = {"Output": {"lines": [f"executed: {command}"], "prompt": "fixture$", "success": True}}
        else:
            prefix = request["Complete"]["input"]
            start = prefix.rfind(" ") + 1
            token = prefix[start:]
            candidates = [value for value in (["lscpu", "hostname", "free", "ethtool", "ip"] if start == 0 else ["eth0"])
                          if value.startswith(token)]
            response = {"Completions": {"start": start, "candidates": candidates}}
        self.wfile.write((json.dumps(response) + "\n").encode())


class Terminal:
    def __init__(self, binary, port, history):
        self.master, slave = pty.openpty()
        env = os.environ.copy()
        env.update(TERM="xterm-256color", GAME_SSH_PASSWORD="fixture-password", GAME_SSH_HISTORY_DIR=history)

        def controlling_terminal():
            os.setsid()
            fcntl.ioctl(0, termios.TIOCSCTTY, 0)

        self.process = subprocess.Popen([binary, "--host", "127.0.0.1", "--port", str(port), "1"],
                                        stdin=slave, stdout=slave, stderr=slave, env=env,
                                        preexec_fn=controlling_terminal)
        os.close(slave)
        self.output = b""
        self.wait_for(b"fixture$")

    def wait_for(self, marker):
        deadline = time.monotonic() + 5
        while marker not in self.output:
            assert time.monotonic() < deadline, (marker, self.output)
            ready, _, _ = select.select([self.master], [], [], 0.1)
            if ready:
                self.output += os.read(self.master, 65536)

    def execute(self, keys, expected):
        self.output = b""
        os.write(self.master, keys + b"\r")
        actual = commands.get(timeout=5)
        assert actual == expected, (actual, expected, self.output)
        self.wait_for_result(expected)

    def wait_for_result(self, expected):
        marker = f"executed: {expected}".encode()
        self.wait_for(marker)
        self.output = self.output.split(marker, 1)[1]
        self.wait_for(b"fixture$")

    def close(self):
        os.write(self.master, b"\x04")
        assert self.process.wait(timeout=5) == 0
        os.close(self.master)


with socketserver.TCPServer(("127.0.0.1", 0), ConsoleFixture) as server, tempfile.TemporaryDirectory() as history:
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    terminal = None
    try:
        port = server.server_address[1]
        terminal = Terminal(sys.argv[1], port, history)
        terminal.execute(b"hostnme\x1b[D\x1b[Da\x1b[C\x1b[F", "hostname")
        terminal.execute(b"xuname\x1b[H\x1b[3~\x1b[F -a", "uname -a")
        terminal.execute(b"\x1b[A", "uname -a")
        terminal.execute(b"\x1b[A\x1b[A", "hostname")
        terminal.execute(b"free\x1b[A\x1b[B", "free")
        terminal.execute(b"lsc\t", "lscpu")
        terminal.execute(b"ethtool et\t", "ethtool eth0")
        terminal.execute(b"ip link set dev eX up\x1b[H" + b"\x1b[C" * 17 + b"\t\x1b[F", "ip link set dev eth0 up")
        terminal.execute(b"\x12lscpu", "lscpu")
        terminal.execute(b"hostnamx\x7fe", "hostname")
        terminal.execute(b"hostname unwanted\x17", "hostname")
        terminal.execute(b"garbage\x15free", "free")
        terminal.execute(b"hostname unwanted\x01" + b"\x1b[C" * 8 + b"\x0b\x05", "hostname")
        terminal.output = b""
        os.write(terminal.master, b"cancel-this\x03")
        terminal.wait_for(b"fixture$")
        terminal.execute(b"free -h", "free -h")
        terminal.output = b""
        os.write(terminal.master, b"\x1b[200~free -h\nhostname\x1b[201~\r")
        assert commands.get(timeout=5) == "free -h"
        assert commands.get(timeout=5) == "hostname"
        terminal.wait_for_result("hostname")
        terminal.close()
        terminal = Terminal(sys.argv[1], port, history)
        terminal.execute(b"\x1b[A", "hostname")
        terminal.close()
        terminal = None
        env = os.environ.copy()
        env.update(GAME_SSH_PASSWORD="fixture-password", GAME_SSH_HISTORY_DIR=history)
        script = subprocess.run([sys.argv[1], "--port", str(port), "1"],
                                input="free -h\nhostname\n~.\n", text=True,
                                capture_output=True, env=env, timeout=5)
        assert script.returncode == 0, script.stderr
        assert script.stdout == "executed: free -h\nexecuted: hostname\n", script.stdout
        assert commands.get(timeout=5) == "free -h"
        assert commands.get(timeout=5) == "hostname"
        files = list(Path(history).glob("*.history"))
        assert len(files) == 1, files
        text = files[0].read_text()
        assert "lscpu" in text and "hostname" in text
        assert "fixture-password" not in text and "cancel-this" not in text
        assert commands.empty()
    finally:
        if terminal is not None and terminal.process.poll() is None:
            terminal.process.terminate()
            terminal.process.wait(timeout=5)
            os.close(terminal.master)
        server.shutdown()
        thread.join(timeout=5)
