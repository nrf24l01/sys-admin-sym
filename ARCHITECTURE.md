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

Server chassis dimensions, slot counts, PCIe lane widths, and component models
come from `assets/equipment/server_parts.json`. Purchased parts live in saved
inventory. Installation checks socket and memory compatibility, free bays,
physical PCIe slot width, generation, and the CPU's total available lanes.
Ethernet cards are PCI card variants and allocate ordinary server `Port` records,
so existing cabling, link negotiation, IP configuration, and persistence apply.
Removing a card disconnects its cables and returns it to inventory. A new
chassis requires CPU, RAM, and a power supply before power can be requested;
preexisting servers retain their original assembled behavior.
The initial R360 slot layout follows Dell's
[expansion slot guide](https://www.dell.com/support/manuals/en-in/poweredge-r360/r360_ism/expansion-card-installation-guidelines?guid=guid-9ffebd78-0ef4-4614-b698-c450c609054f&lang=en-us)
documentation. Installed PCIe cards cover the corresponding rear slot in the
rack view, and their sockets use the same JSON-defined positions for painting,
click targets, and cable endpoints. Fans are included in the chassis; older
saves receive a refund for previously purchased fans.
The initial [Xeon E-2434](https://www.intel.com/content/www/us/en/products/sku/236192/intel-xeon-e2434-processor-12m-cache-3-40-ghz/specifications.html)
and [I350 adapter](https://www.intel.com/content/dam/doc/product-brief/ethernet-i350-server-adapter-brief.pdf)
specifications are from Intel; chassis base power is a game estimate.
Drive models and rated throughput/IOPS live in `assets/equipment/drives.json`.
The R360's four SATA bays are declared in the server chassis JSON. Purchased
drives are saved in a separate storage inventory; installation validates the
bay interface and updates the server's power load. The Linux console projects
installed drives as `/dev/sd*` devices, while `ethtool` and `netstat -i` read
the simulated network ports and runtime counters. Drive ratings describe the
model; filesystem and measured disk I/O are not simulated yet.

Cable inventory is domain state: a 305 m bulk box is raw stock, connector packs
provide RJ45 plugs, and each new link consumes a requested cut plus two plugs.
Disconnecting stores the finished lead for reuse. Rack cable shape is presentation
state only: a fixed-endpoint Verlet rope applies gravity and damping while the
network link keeps its exact physical length.
Ethernet and power adapters supply cable identities, projected paths, lengths,
selection state, and connector artwork to one generic `CableLayer`. The parent
owns slack allocation, rope simulation, dragging, hit testing, connector placement,
and jacket/highlight rendering. Visible sections share the available cable length
proportionally; automatic lengths use the same 5% slack rule for both families.

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
## Power

Power is represented by the pure `PowerSystem` domain in `crates/sim`. Rack
mains expose exactly four C13 outlets. UPS and PDU outlets are explicit graph
edges, so a PDU can be downstream of a UPS. Loads are calculated at 230 V in
integer W, VA, and mA; source limits latch trips until reset. UPS battery
energy is persisted and advanced through `NetworkSim::advance_time` using
millisecond simulation time, including transfer, discharge, and recharge.
Legacy saves receive empty wiring and no implicit power source.

Power connections retain their endpoint map for compatibility and persist a
parallel cord-kind map. Missing cord entries in older saves migrate to IEC for
ordinary devices and the supplied Cisco 66 W adapter for ISR C1111 routers.
The router adapter has a four-pin inlet (not a barrel jack), 12 V / 5.5 A
output, 90% efficiency, and PF 0.90; its upstream draw is calculated once at
the adapter boundary.
