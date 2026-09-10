# Architecture

> **Simulation knows nothing about Bevy. UI never directly mutates simulation.
> Bevy coordinates commands between them. IDs belong to simulation; Bevy
> Entities belong to presentation.**

```text
egui → UiAction → Bevy application layer → Command
                                         ↓
                               network-simulation thread
                                         ↓
                             pure Rust NetworkSim/events
                                         ↓
                          immutable snapshot → Bevy/egui
```

## Crates

- `crates/sim`: pure domain model. It depends only on `serde` and `thiserror`.
- `crates/game`: Bevy application, egui rack/topology presentation, simulation
  worker, and SQLite adapter.

The worker thread exclusively owns the mutable `NetworkSim`. Bevy sends domain
commands through a channel and renders cloned snapshots. Rack panels, clickable
ports, and cable paths are derived presentation and are never authoritative.

Links are canonical records. The port-to-link map is a transient index rebuilt
after load. Editor text remains in `EditorDrafts` until **Apply**, so incomplete
addresses never enter the domain state.

## Reachability model

`NetworkSim::ping` performs constrained graph traversal:

1. Verify power, rack placement, interface state, and a source address.
2. Traverse physical links and switch forwarding only where the VLAN is carried.
3. Deliver directly inside the source subnet, otherwise resolve the configured
   gateway on the same L2 domain.
4. Select a connected router subnet or its internet-connected WAN.
5. Return a hop trace or a typed failure such as `VlanBlocked`, `NoGateway`,
   `NoRoute`, or `AddressConflict`.

There are no Ethernet frames or packet timing in this MVP.
