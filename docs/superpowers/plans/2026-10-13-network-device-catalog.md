# Network Device Catalog Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans for inline execution. Steps use checkbox syntax.

**Goal:** Add all requested Cisco devices with JSON-only model extension.
**Architecture:** One validated registry drives model identity, purchase, ports,
power and presentation. Legacy APIs delegate to its defaults.
**Tech Stack:** Rust, serde, Bevy/egui, JSON, Python asset tooling.
**Spec:** docs/superpowers/specs/2026-10-13-network-device-catalog-design.md

## Global Constraints
- Preserve the user's servers.json edit and existing save identities.
- All per-model properties live in JSON; no model rendering or purchase branches.
- Use NetworkSim::link_status for optical forwarding and terminal/UI status.
- Run make check.

## Review Focus
- Unknown saved IDs fail clearly instead of defaulting silently.
- Mixed cages and combo sockets cannot create duplicate usable interfaces.
- Multi-unit devices agree between rack placement and rendering.
- JSON overrides affect simulation and UI consistently.
- Optical assemblies do not consume bulk copper materials.

### Task 1: Catalog and domain integration
**Files:** network_devices.json; crates/sim/src/network_hardware.rs; device.rs;
world.rs; shop.rs; command.rs; switching/catalog.rs; router_hardware.rs; ios.rs;
power and optics modules; crates/sim/tests/network_hardware.rs.
**Interfaces:** network_device_catalog(), NetworkDeviceModel, NetworkModelId,
Command::BuyNetworkDevice { model_id }, PurchaseItem::NetworkDevice(String).
- [x] Add failing coverage for requested catalog models.
- [x] Create sourced model records and validated string-backed identities.
- [x] Route purchase, ports, console names, rack height and power through models.
- [x] Preserve legacy model migrations and save/load behavior.
- [x] Add high-speed cages and matching optical inventory; test physical links.
- [x] Run focused domain tests; expected pass.

### Task 2: Presentation and extension tooling
**Files:** crates/game/src/ui/network_models.rs; ui/mod.rs; ui/equipment.rs;
ui/shop/catalog/hardware.rs; ui/shop/artwork.rs; localization;
tools/network_device_artwork.py; docs/NETWORK_HARDWARE.md; ATTRIBUTION.md.
**Interfaces:** Consume Task 1 registry and per-port geometry; provide catalog
artwork loading, shop offers and model-localized names.
- [x] Test all model offers/artwork and model socket geometry.
- [x] Iterate registry for textures/offers and eliminate per-model UI branches.
- [x] Render schematic assets from JSON and document provenance/schema/sources.
- [x] Test installed JSON-only addition in an isolated child process.
- [x] Run make check; all checks and smoke pass.
- [x] Review final diff for model hardcoding and save compatibility.


## Execution notes

User explicitly requested immediate execution after confirming JSON ownership.
Implementation stayed in the shared workspace to preserve the existing user
edit to servers.json. A fresh reviewer inspected the full change; regressions
cover its important findings: combo inspector actions and reload, shared
hitboxes, Layer 3 switch management/STP, routed flex ports, nonrouting router
catalogs, and PSU ratings. Mixed-side inlet rendering was also corrected.
Initial sandbox verification could not bind the local console socket; final
make check runs with local networking available. No source tests are skipped.

Texture-mapper discovery and distinct combo socket selection were extended for
the network schema; legacy maps without a side retain their front-face default.
The machine lacked Lavapipe. The matching Arch vulkan-swrast package was extracted
to /tmp/codex-network-lavapipe without changing system packages. Final verification:
`VK_DRIVER_FILES=/tmp/codex-network-lavapipe/usr/share/vulkan/icd.d/lvp_icd.json make check`.
