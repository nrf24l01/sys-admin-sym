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

The equipment shop adapts domain models into one presentation catalog with
category-specific filters, complete artwork, product details and comparison.
`Command::Purchase` quotes model prices and commits quantity purchases atomically.
The shop receives a correlated worker result after the committed snapshot;
compatibility previews share installation rules with domain commands.
See [Equipment shop](docs/SHOP.md) for browsing and extension behavior.

## Crates

- `crates/sim`: pure domain model. It depends only on `serde` and `thiserror`.
- `crates/game`: Bevy application, egui rack/topology presentation, simulation
  worker, and SQLite adapter.
- `crates/terminal`: standalone `game-ssh` CLI, with options, transport client,
  console session, and editor modules. It has no dependency on Bevy.

`ConsoleEditor` owns the Rustyline editor, `ConsoleHelper` handles completion and
history hints, and `HistoryFile` persists history per endpoint and device.
Completion requests use the authenticated console transport and read command
grammar and device state through `sim::completion`; they do not execute commands
or change console modes. Piped input uses the separate script session path.

`game::console::LocalConsoleServer` owns the configured TCP listener and its thread.
It sends typed console requests to the same simulation worker used by the GUI.
`ConsoleService` resolves device IDs, names, hostnames, and in-game IP addresses,
then executes commands against the worker's authoritative `NetworkSim`. Replies
go to the client; console output and updated snapshots also go to Bevy. The
protocol types live in `sim::remote`. Each request carries a password that the
listener checks before forwarding it to the simulation. This bridge does not simulate SSH or TCP
inside the game network.

`SettingsPlugin` loads the console configuration independently of game saves.
Its Settings window keeps editable drafts separate from the active settings.
Applying host/port changes rebinds the listener; a failed bind restores the previous
listener. Password changes take effect on the next request. `SettingsStore` writes
the configuration through a temporary file and restricts Unix permissions to the
owner. Save, Load, and New Game actions are presented in the Settings window.

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
chassis includes a 600 W power supply and requires CPU and RAM before power can be requested;
preexisting servers retain their original assembled behavior.
The shop also offers a full pack that installs a CPU, 16 GB RAM, a four-port NIC,
and a 960 GB SSD in one atomic purchase. Separate power supplies are no longer
sold; older saves receive a one-time refund for owned or installed PSUs. Server
compute capacity sums core count times clock MHz across CPUs. Memory capacity
uses the installed module's DDR generation and DIMM class coefficients. Live
network capacity sums negotiated rates on connected data ports; `mgmt0` is a
separate management NIC and does not contribute resource capacity.
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

`PortConnector` records the socket independently from VLAN/IP configuration.
`optics` owns cage capabilities, separate transceiver and finished assembly
inventories, typed installation commands, and the shared `link_status` evaluator.
The original Catalyst has four 1G SFP cages; a separate JSON hardware profile
provides 1/10G SFP+ uplinks, and the PCIe catalog includes an Intel X520-DA2
(two SFP+ cages, PCIe 2.0 x8). Generic prototype NIC IDs migrate on load without
recreating their interfaces.
LC patch panels reuse passive paired-port forwarding without regenerating signals.
Cages validate physical form, supported speed/lane/FEC modes, and module power.
The evaluator checks installation, power, enablement, complete cable paths,
module/fiber/wavelength compatibility, total reach, polarity, and optical loss.
GUI LEDs, diagnostics, packet forwarding, counters and server resource availability
consume that result. Component traversal is bounded and searches the connected
cable component instead of scanning all datacenter ports.

Fiber cords and DAC/AOC assemblies are purchased whole; disconnecting returns
the same saved instance rather than generating RJ45 stock. Rerouting validates
the purchased length before mutation. Hot swapping drops carrier while retaining
port configuration; a cable's logical endpoint remains reserved for replacement.
Saved defaults preserve existing copper-only worlds; loading normalizes stale
hardware attachments and restores inventory ID counters. Module power contributes
to electrical load. RJ45 stock, five jacket colors and the existing 100 m copper
channel limit remain available. See [optical networking](docs/OPTICAL_NETWORKING.md)
for the catalog, player workflow, units and current physical-model limits.

`RoomCableLayout` defines 220 fixed cable managers: 40 on vertical trays,
100 above and below racks, and 80 at horizontal/vertical tray intersections.
The room renderer uses the same layout for horizontal trays. IDs 1–40 retain
their original locations, and loading older saves adds the new managers.

The room owns two uplink RJ45 handoff sockets and one unpatched LAN socket per
rack. Outlets are saved with ordinary links; rack sockets do not form an implicit
broadcast network. Use explicit switches and paired patch-panel devices.

`ProviderNetwork` encapsulates address inventory, transit contracts, explicit
routing domains, BGP import/export policy, filtering, DHCP pools and management
configuration. `ProviderCommand` validates changes before mutation. Public
address ownership is independent of transit; incoming packets use an upstream
route to an edge next hop and the same packet engine as internal traffic. The
legacy WAN flag no longer creates replies or installs routes. Named external
lab hosts respond only when reachable and available. No host Internet traffic
is generated.

Each `Router` owns its saved `domain_routes` alongside legacy static routes.
The routing editor issues typed commands scoped to that device and chooses only
its own egress interfaces. Old globally stored routes migrate into their owning
router during save loading. Route replacement validates before mutation.
The **IP RANGES** window projects owned pools and carrier routes into a routed
allocation: public prefix, selected uplink, higher subnet/gateway, WAN next hop
and suggested local gateway. `RouteOwnedRange` validates all changed carrier
routes before applying them. Selection never configures a player device.
`RoutedRangeHandoff` is a read-only view over the existing transit contracts;
there is no duplicate assignment store. Guest `netctl` mutations enforce
interface ownership.

Router forwarding uses live interfaces, longest prefixes, preference and stable
ECMP. Domain routes name their domain and VLAN; legacy static routes belong to
domain zero. Optional ARP tracking withdraws unavailable next hops. BGP sessions
model peer eligibility, policy and route installation, with immediate state
changes rather than wire-protocol timers. Ordered interface policies filter IPv4
frames; assigned-source validation also guards ARP. A deterministic per-VLAN
spanning forest models converged bridge forwarding.

`DataCenterResources` credits servers based on actual packet reachability; NIC
rates are inventory capacity, not guaranteed end-to-end bandwidth.
`NetworkCapacity` separately reports live transit capacity and the capacity
remaining after the largest circuit fails. Traced Ethernet paths retain
intermediate ports for bottleneck diagnostics. Management SSH requires packet
reachability and the server's SSH service; switch management includes a subnet,
VLAN and optional gateway. `mgmt0` remains an OS interface, not a standby BMC.

See [provider design](docs/PROVIDER_NETWORK_DESIGN.md) for object responsibilities
and [provider operations](docs/PROVIDER_NETWORK_GUIDE.md) for configuration,
compatibility and protocol bounds.

## Simulated Linux guests

Each server owns a serialized `ServerOs`: a `GuestFilesystem`, working directory,
environment, bounded command history, `LinuxRoute` records, loopback state, and
service state. Existing saves default to an empty OS map and initialize a guest
on its first console command. Secondary NIC addresses also use a serde default.
Selling a server removes its guest and associated SSH sessions.

`LinuxShell` tokenizes quoting and shell operators, expands guest variables,
and delegates execution to `CommandRegistry`. Every registered `LinuxCommand`
object owns `execute` and `suggest` methods. Command names, help, execution, and
argument completion use that registry. Command objects share domain services for
files, networking, and service management. Pipes and
redirection carry guest text, preserving exact bytes for `printf`, `echo`, and
`cat`. All execution happens in the simulation; no subprocess, host filesystem,
Linux image, or real network connection is involved. Script nesting is bounded.
`TerminalRenderer` presents output and input, including command pasting and
completion; it does not execute guest commands.

To add a Linux command, implement `LinuxCommand` and register its object in
`CommandRegistry::builtins`. No shell dispatch switch or separate list of command
templates needs updating. `CompletionContext` exposes parsed arguments and
read-only game state, plus helpers for interfaces, addresses, files, services,
environment variables, and drives. The command's `suggest` method chooses the
next argument candidates. `CompletionInput` recognizes the command at the cursor
after pipes/conditionals, redirection, and incomplete quoted input. Filtering,
deduplication, shell escaping, and replacement offsets belong to the registry.
Both the in-game terminal and external terminal client use this same completion
API. IOS completion retains its existing configuration-mode grammar.

Server NICs retain a primary address for inspector compatibility and additional
IPv4 addresses. ARP, conflicts, public allocation, and ICMP delivery inspect all
addresses. `server_route_selection` selects the longest prefix and then lowest
metric, and supplies the source address and next hop to the packet engine.
Connected routes derive from NIC addresses. Explicit default routes supersede
legacy per-address gateways. Runtime address and route commands apply immediately.

`LinuxServices` applies `/etc/network/interfaces` to a cloned simulation and
commits it only after every selected stanza and post-up command succeeds.
Static configuration, explicit-server DHCP, netmasks, gateways, DNS file configuration,
and post-up routes are supported. Reboot clears runtime addresses/routes and
reapplies enabled networking configuration. SSH service state controls new
management connections and the simulated listener shown by `ss`.

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
are cleared or rebuilt when topology revisions change. Transit and external endpoints are explicit provider configuration; legacy WAN
flags are retained only for save compatibility. No external host network is used.

The implemented packet slice covers L1–L3 Ethernet/IPv4/ICMP. DHCP and BGP have
semantic models with the bounds described in the provider guide. The simulator has no TCP/UDP sessions, transport
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
