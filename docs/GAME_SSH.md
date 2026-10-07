# game-ssh standalone client

`game-ssh` connects to device consoles in a running Cloud Provider Simulator.
It does not need the game's assets or a Rust installation.

## Download and run

Download a ZIP and its matching `.zip.sha256` file from
[GitHub Releases](https://github.com/nrf24l01/sys-admin-sym/releases), using a
`game-ssh-v*` release. Development builds are available in the
[Build game-ssh workflow](https://github.com/nrf24l01/sys-admin-sym/actions/workflows/build-game-ssh.yml):
select a successful run and download `game-ssh-x86_64-unknown-linux-gnu` or
`game-ssh-x86_64-pc-windows-msvc`. Workflow downloads require GitHub sign-in and
contain an outer ZIP; extract it to find the client ZIP and checksum.

Linux packages are built on Ubuntu 22.04 for x86-64 with glibc. Windows packages
are built for x86-64 using MSVC with a statically linked C runtime, so no Visual
C++ redistributable is needed. The client needs a terminal, not a graphical desktop.

Before extracting the client ZIP, optionally verify it on Linux:

```bash
sha256sum -c game-ssh-x86_64-unknown-linux-gnu.zip.sha256
```

On Windows, compare `Get-FileHash game-ssh-x86_64-pc-windows-msvc.zip -Algorithm SHA256`
with the digest in its `.sha256` file.

Extract the client ZIP. On Linux, run `chmod +x game-ssh/game-ssh` if necessary.
Then run `./game-ssh/game-ssh --help`; on Windows, run
`.\game-ssh\game-ssh.exe --help`. You can also place the executable on your `PATH`.

## Connect

Keep the game running and enable its remote console in Settings. The default
address is `localhost:47655`. Use the password configured in the game; the client
prompts with hidden input. With the executable on your `PATH`:

```bash
game-ssh --list
game-ssh 2
game-ssh 2 -c 'show version'
game-ssh --host localhost --port 51234 --list
```

Choose a device by ID, name, hostname, or in-game IPv4 address. Devices must be
powered on. The connection goes directly to the simulated console. An in-game
IP selects a device without requiring an in-game network path.

Tab completes commands; Ctrl-R searches history. Type `~.` or `logout` to
disconnect. For scripted use, provide the password through `GAME_SSH_PASSWORD`.
This is an SSH-like game client using password authentication over unencrypted
TCP, rather than an implementation of the SSH protocol.

## Build and release workflow

The workflow runs for relevant pushes to `main`, pull requests targeting `main`,
manual **Run workflow** requests, and pushed `game-ssh-v*` tags. Path filters cover
workspace manifests, client and simulation code, embedded equipment JSON,
standalone packaging scripts, this document, and the workflow itself. Tag pushes
run independently of path filters.

Tests and Clippy for the client and simulation, workspace formatting, and ZIP
packaging tests run before building. Linux also runs the interactive terminal
integration test. Both native build jobs run client tests, compile the release
binary, and check `--help`; Windows checks for unwanted runtime DLL dependencies.
Each job uploads a ZIP containing only the client and this README, alongside a
SHA-256 checksum. Artifacts are retained for 30 days. Cargo builds are cached.

Prerequisites are GitHub Actions enabled and GitHub-hosted Ubuntu 22.04/Windows
runners. The workflow installs stable Rust; Python 3, GitHub CLI, and Windows
Visual Studio C++ tools are provided by the runner images. No repository secrets
or external registry are required. Test/build jobs use `contents: read`; only the
tagged release job uses `contents: write` with the automatic `GITHUB_TOKEN`.
Repository or organization policy must permit that token to create releases.

To publish a version after the workflow has been merged, tag the intended commit:

```bash
git tag game-ssh-v0.1.0
git push origin game-ssh-v0.1.0
```

After tests and both builds pass, the workflow publishes the ZIPs and checksums
on that tag's GitHub Release. Rerunning a tag replaces its release assets. Client
releases do not replace the repository's latest game release. Branch, pull request,
and manual runs upload artifacts without publishing a release.
