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

The backend exclusively owns mutable `NetworkSim` state. Bevy sends domain
commands through the application boundary and renders snapshots/events. Rack
panels, clickable ports, and cable paths are derived presentation and are never
authoritative. Runtime telemetry is separate from saved configuration so
activity indicators do not change persistence data.

Links are canonical records. The port-to-link map is a transient index rebuilt
after load. Editor text remains in `EditorDrafts` until **Apply**, so incomplete
addresses never enter the domain state.

Cable inventory is domain state: a 305 m bulk box is raw stock, connector packs
provide RJ45 plugs, and each new link consumes a requested cut plus two plugs.
Disconnecting stores the finished lead for reuse. Rack cable shape is presentation
state only: a fixed-endpoint Verlet rope applies gravity and damping while the
network link keeps its exact physical length.

`PortConnector` records physical media independently from VLAN/IP
configuration. This milestone permits cable creation only between RJ45 ports;
the Catalyst SFP cages exist in the domain and rack projection but reject links
until SFP transceivers and fiber media are implemented.

RJ45 links negotiate the lower of the endpoints' advertised and hardware
maximum rates (10, 100, or 1000 Mbps), and copper links over 100 m remain down.
Link/status checks connector, power, placement, enablement, cable length, and
negotiated rate. Activity uses runtime transmit/receive timestamps. Inventory
models shared bulk stock, finished leads, and five common jacket colors rather
than separate supplier SKUs.

## Network backend and OSI scope

`NetworkSim::transmit_frame` is the synchronous bounded Ethernet engine. It
validates the physical link, applies access/trunk VLAN tagging, learns source
MAC addresses per switch and VLAN, then forwards known unicast or floods
broadcast traffic through a bounded work queue. It returns deterministic
endpoint deliveries instead of touching Bevy or the UI.

`NetworkSim::transmit_icmp` builds on that engine for the implemented IPv4
slice. It validates the source interface, resolves the next hop with an ARP
broadcast and cached MAC entry, sends an IPv4 ICMP echo request, and recursively
delivers the echo reply while recording semantic hops. TTL and recursion bounds
prevent malformed topologies from running indefinitely. `ping` runs the same
path against a clone for an immutable diagnostic; `ping_mut` records runtime
telemetry for UI activity indicators.

Configuration (ports, VLANs, IPv4 addresses and routes) is serialized. Learned
ARP/MAC state, frame counters and timestamps live in the runtime backend and
are cleared or rebuilt when topology revisions change. The WAN is an abstract
router interface compatibility feature; it does not access an external
network.

L1–L3 behavior is implemented. The simulator has no TCP/UDP sessions, transport
reliability, presentation encoding, application protocols, wire serialization,
or wall-clock packet timing (L4–L7).
