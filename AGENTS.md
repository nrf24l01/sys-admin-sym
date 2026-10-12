# Repository Guidelines

## Project Structure & Module Organization

This is a Rust workspace. `crates/sim` contains deterministic domain logic for racks, devices, cabling, networking, power, and storage. `crates/game` contains the Bevy/egui application, including UI (`src/ui`), app state (`src/app`), plugins, and SQLite persistence. `tools/texture-mapper` is a workspace utility; `tools/package_game.py` builds distributable ZIPs. Runtime images and equipment configuration live in `assets`; keep source attribution current in `assets/equipment/ATTRIBUTION.md`. Integration tests live in `crates/sim/tests`, while smaller Rust tests sit beside the code they cover. See `ARCHITECTURE.md` for design context.

Optical hardware lives in `crates/sim/src/optics`; keep cage compatibility, cable ownership and physical link evaluation there. `NetworkSim::link_status` is authoritative for packet forwarding, GUI and terminal status. Models and inline EN/RU item text live in `assets/equipment/optics.json`. Finished fiber/DAC/AOC assemblies must never consume or return bulk RJ45 materials. See `docs/OPTICAL_NETWORKING.md` for supported profiles and extension rules. Socket menus and inspectors share `assembly_supported_at_port` inventory filtering; physical LC mispatches remain diagnosable by the link evaluator. `assets/equipment/optical_connectors.png` is the original transparent atlas; UV crops and sprite-family mapping live in `crates/game/src/ui/optics.rs`. Shop cables use full padded cells in the dedicated `assets/cables/optical_shop_connectors.png` atlas. Never crop connector bodies from coil images: this clips housings and includes neighboring cable fragments. Group socket inventory by model with available counts, while actions retain a concrete free instance ID.

Server models live in `assets/equipment/servers.json`; shared CPU, RAM and PCIe components live in `server_parts.json`. Resolve each saved server's model for compatibility, power, rack height, artwork and socket geometry. The catalog's default `chassis` and standalone hardware helpers are R360 compatibility APIs, not a profile for arbitrary servers. Add models with `tools/add_server_model.py`, then replace copied specifications, geometry, recipe and attribution with verified model data. Shop offers and artwork iterate the catalog: never add per-model Rust enum variants or rendering branches. Keep full-pack recipes compatible and purchases atomic. See `docs/SERVER_HARDWARE.md` and `docs/examples/server-model.json`.

## Build, Test, and Development Commands

- `make check` (also plain `make` or `make test`): required full verification after each implementation run. Runs formatting, whitespace checks, Clippy, all Rust and Python tests, workspace build, and isolated Linux game/console startup. Fix failures before finishing.
- `make run` / `make run-release`: run the full verification suite before launching the game.
- `make test-hosting`: focused hosting simulation and egui interaction tests.
- `make fmt`: apply Rust formatting.
- `make check-core`: full non-graphical verification for platforms without the Linux smoke runtime. On Linux, install the dependencies documented in README.md and run `make check`.
- `make help`: list available targets. `make smoke-built GAME=... CLIENT=...` checks already-built binaries (used by release CI).

The smoke test requires Xvfb, xauth, and Mesa Lavapipe. It uses a temporary working directory and a unique local console endpoint, and keeps logs in `target/check-logs/`. Do not substitute ad hoc command lists for the Make verification entry point. The underlying Cargo and Python commands remain available as individual Make targets.

The game writes `cloud-provider-save.db` in its working directory. Avoid committing local save data or generated `target` output.

## Coding Style & Naming Conventions

Follow Rust 2024 conventions and `rustfmt` defaults (four-space indentation). Use `snake_case` for modules, functions, and test names; `PascalCase` for types; and `SCREAMING_SNAKE_CASE` for constants. Keep simulation rules in `crates/sim` and presentation or input handling in `crates/game`. Use descriptive asset names and keep equipment JSON mappings aligned with their sprite sheets.

## Testing Guidelines

Add regression tests for changed simulation behavior in the matching `crates/sim/tests/*.rs` file. For UI or asset-coordinate changes, add focused module tests where practical. Name tests after the behavior they verify, and run `make check` before submitting. CI also runs packaging tests and checks that the Linux build starts. The independent `build-game-ssh.yml` workflow builds Linux/Windows clients and publishes releases for `game-ssh-v*` tags; see `docs/GAME_SSH.md`.

## Commit & Pull Request Guidelines

Keep `AGENTS.md` tracked and include any changes to it in commits so future agents have the current repository instructions.

Recent commits use short, imperative subjects (for example, `Fix Linux game package startup under X11`). Keep commits scoped to one coherent change. Pull requests should explain user-visible behavior, list validation commands, and link a related issue when one exists. Include before/after screenshots for UI or sprite changes, and call out new assets, attribution updates, or save-data compatibility effects.
