# Wiki and first sold VM guide Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Add an in-game Markdown Wiki with a correct live guide through the first paid VM sale.

**Architecture:** Simulation derives prerequisite checks and historical sale completion from saved world state. Game loads localized Markdown assets and displays the report in a permanent Wiki window.

**Tech Stack:** Rust 2024, Bevy/egui, pulldown-cmark 0.13.4; existing JSON locales and asset packaging.

**Spec:** `docs/superpowers/specs/2026-10-13-wiki-first-vm-design.md`

## Global Constraints
- Simulation rules stay in `crates/sim`; presentation stays in `crates/game`.
- `NetworkSim::link_status` and existing packet/hosting readiness are authoritative.
- Preserve saves and equipment catalog iteration; do not grant money or force a model.
- Articles come from `assets/wiki/en` and `assets/wiki/ru`, with English fallback.
- Required final verification: `make check`.

## Review Focus
- Multiple unrelated servers must not satisfy a mixed-role checklist.
- A cable, matching address, or declared VLAN alone must not imply working traffic.
- Free orders, dedicated products, and renewals must not count as first VM sales.
- Save history must retain completion after refunds, removal, and load.
- Window resizing, missing content and relative links must remain usable.

## Task 1: Domain progress report
Files: `crates/sim/src/guide/{mod,management}.rs`, `crates/sim/src/lib.rs`,
`crates/sim/src/hosting/node_networks.rs`, `crates/sim/tests/hosting/first_vm_guide.rs`.
Produces: `NetworkSim::first_vm_guide(Option<DeviceId>, Option<DeviceId>) -> FirstVmGuide`;
`FirstVmStep`, `GuideCheck`, detected roles, current checks, durable sale evidence.
- [x] Write/run regressions against the existing hosting Lab for prerequisites, broken management, unrelated roles, and sale history.
- [x] Implement deterministic role resolution, real management checks, shared hosting checks and sale evidence.
- [x] Verify with `make test-rust CARGO_SCOPE='-p cloud-provider-sim --test hosting first_vm_guide'`.

## Task 2: Localized Markdown library and lessons
Files: `crates/game/src/wiki/{mod,content,markdown}.rs`, `crates/game/Cargo.toml`,
`assets/wiki/{en,ru}/**/*.md`, `assets/locales/{en,ru}.json`.
Produces: runtime WikiLibrary, safe relative navigation, parsed article blocks,
copyable code, complete first-VM and networking lessons.
- [x] Write/run content tests for recursive loading, locale fallback, reload and confined relative links.
- [x] Implement content loading and CommonMark block/span rendering model.
- [x] Author matching EN/RU guide and reference articles using implemented console commands.
- [x] Verify library/content tests and links through focused game Make tests.

## Task 3: Wiki window and integration
Files: `crates/game/src/ui/wiki.rs`, `crates/game/src/app/{mod,state,wiki}.rs`,
`crates/game/src/{main,ui/mod}.rs`, README.md, ARCHITECTURE.md, docs/WIKI.md.
Consumes: Task 1 report and Task 2 library; produces Wiki top-bar/window and cached live checklist.
- [x] Write focused egui interaction/navigation/resize tests.
- [x] Implement persistent-access window, role selectors, live status/hints, articles, links and copy buttons.
- [x] Wire Wiki beside Shop/Bank and document authoring and progression behavior.
- [x] Run `make fmt` and `make check`; fix failures, review complete change, capture screenshot.

## Execution notes
User explicitly requested implementation after approving the behavior and architecture.
Proceed inline in the provided workspace and preserve unrelated `.VSCodeCounter/`.
No manual progress flags: current prerequisites come from saved state; first-sale
completion comes from retained hosting history. UI role overrides are presentation
state and existing controller/node registrations restore defaults after reload.

## Verified result

Implemented 14 live checks and 17 article pairs in English/Russian. Domain tests
exercise the shipped Markdown command blocks from a fresh world through an
automatic paid market order. Review fixes cover distinct role labels, moved
controller IPs, management address aliases, preparation before storefront setup,
power before cabling, reopening after closed-window changes, and nested lists.
Visual inspection exposed a two-pixel-per-frame width drift caused by estimated
column spacing; the article pane now uses the actual remaining width, with a
multi-frame navigation regression.

`VK_DRIVER_FILES=/tmp/wiki-lavapipe/lvp_icd.json make check` passed formatting,
whitespace, Clippy, all Rust/Python tests, workspace build, and the real Linux
window/authenticated console smoke test. The temporary driver is the official
[Arch vulkan-swrast package](https://archlinux.org/packages/extra/x86_64/vulkan-swrast/); system packages were not changed. The console tests
and graphical checks ran outside the socket-restricted sandbox. Independent
code review completed; material findings were fixed and verified with regressions.
