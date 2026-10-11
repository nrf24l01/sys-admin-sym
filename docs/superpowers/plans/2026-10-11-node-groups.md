# Hosting Node Groups Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement each task with regression tests.

**Goal:** Configure virtualization nodes together and select their live group in products.
**Architecture:** Persistent groups hold shared settings and membership; commands atomically materialize settings onto nodes. Product consumers resolve current membership.
**Tech Stack:** Rust 2024, serde/RON persistence, Bevy/egui.
**Spec:** `docs/superpowers/specs/2026-10-11-node-groups-design.md`

## Global Constraints

Preserve existing drive and scrolling changes and old saves. Keep simulation logic in `crates/sim`, presentation in `crates/game`. Add EN/RU copy. Verify via `make check`.

## Review Focus

- A member missing a named interface must reject the whole edit without partial changes.
- New/removing/moving members must change group placement without modifying existing guests.
- Product group references must never turn into an unrestricted fallback on deletion.
- Group policy edits must affect admission and runtime immediately.
- Old saves must load with empty new fields and retain node configuration.

### Task 1: Persistent groups and synchronized commands

**Files:** `crates/sim/src/hosting/{types,mod,groups}.rs`, `crates/sim/tests/hosting.rs`.
**Interfaces:** `HostingNodeGroup { id, name, nodes, policy, ipv4_ranges, networks }`; `HostingGroupNetwork { interface, subnet, vlan }`; `HostingState.node_groups`; commands `SaveNodeGroup`, `DeleteNodeGroup`, `SetNodeGroup { node, group }`; `hosting_node_group(DeviceId) -> Option<&HostingNodeGroup>`; `validate_hosting_node_group(&HostingNodeGroup) -> Result<(), String>`.

- [x] Add regression tests for synchronized policy/ranges/interface resolution, atomic invalid edits, one-group membership, move/leave/delete behavior and save compatibility; observe missing-feature failure.
- [x] Add serde-default state/selectors, group model and atomic commands. Reuse node validators and guard individual edits for grouped nodes.
- [x] Run `make test-hosting` and inspect results.

### Task 2: Live product placement

**Files:** `crates/sim/src/hosting/{groups,inventory,provisioning,addressing,market}.rs`, `crates/sim/tests/hosting.rs`.
**Interfaces:** `HostingProduct.eligible_groups: BTreeSet<u64>`; `hosting_product_nodes(&HostingProduct) -> BTreeSet<DeviceId>`.

- [x] Add tests purchasing through group-only products, adding/removing members, preserving retained leases and rejecting unknown/empty placement; observe failures.
- [x] Resolve group membership in every product-node consumer and remove references when deleting groups.
- [x] Run `make test-hosting` and inspect results.

### Task 3: Group editing and product/node controls

**Files:** `crates/game/src/app/hosting.rs`, `crates/game/src/ui/hosting{.rs,/groups.rs,/policy.rs,/group_tests.rs}`, locales and `docs/HOSTING.md`.
**Interfaces:** Consumes the Task 1/2 model and commands. Groups tab index 7 preserves existing tab indices. Share policy controls; group networking uses interface names.

- [x] Add actual egui tests for creating/saving a group, selecting it in products and node membership controls in EN/RU; observe failures.
- [x] Implement group drafts/editor, current membership summary, group product selectors and grouped-node controls. Keep scroll layout responsive.
- [x] Update hosting documentation; run `make test-hosting`.

### Task 4: Whole-change verification

- [x] Request independent review and address material findings with regressions.
- [x] Run `make check VK_DRIVER_FILES=/tmp/sys-admin-drive-lavapipe/lvp_icd.json` and inspect every result.
