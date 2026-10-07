# Network feature audit

This audit checks the implementation and player access. It does not certify
interoperability with physical equipment or full IOS compatibility.

## Bugs corrected during this audit

- Duplicate saved WAN records are repaired on load and reload. Untagged and
  VLAN 1 interface configuration now replaces the same logical record, and
  ARP skips unassigned placeholders. Untagged traffic on the physical interface
  remains usable alongside tagged subinterfaces.

- Router VLAN tagging now identifies the active endpoint through passive patch
  panel pairs. ARP and routed traffic retain their tags through that path.
- CDP diagnostics discover the active network device through a patch panel and
  safely skip carrier sockets, which previously could panic without a device owner.
- DHCP and owned-address allocation share one reservation calculation, avoiding
  router, server, carrier, management and other leased addresses.
- `write memory` snapshots the selected device's advanced network configuration.
  Reload restores its bindings, BGP sessions, policies, route preferences,
  DHCP pools, management and spanning-tree state. It preserves other devices,
  inventory and carrier configuration. Leases are not startup configuration.
- Switch `management ip` changes the effective management endpoint instead of
  being shadowed by scenario configuration. It preserves existing VLAN, prefix
  and gateway and validates the replacement. `no management ip` removes it.
- `do ping` and `do traceroute` work from configuration modes. `show arp` and
  `show ip arp` display the selected device's learned neighbors with VLANs.

Regression coverage is in `crates/sim/tests/provider.rs`; existing IOS tests
cover command-mode handling and startup configuration.

## Optical physical layer

SFP/SFP+ modules, fiber cords, DAC/AOC, LC patch panels and the 10G hardware
profiles are now available from the shop and port inspectors. The GUI and
Cisco/Linux diagnostics consume the same physical evaluator as packet
forwarding and resource availability. Reach, polarity, wavelength, fiber,
mode and optical loss faults can disable carrier. See
[Optical networking](OPTICAL_NETWORKING.md) for supported equipment and limits.
Regression coverage is in `crates/sim/tests/optics.rs`; localized UI actions
have focused tests in `crates/game/src/ui/optics.rs`.

## Existing features with incomplete player access

| Feature | Current access | Missing work |
| --- | --- | --- |
| Owned ranges and carrier return routes | IP RANGES | Larger/custom range ordering and aggregate management |
| Interface addressing and VLANs | Inspector and IOS | Native dot1q subinterfaces and full switch virtual interface grammar |
| Static routes | Per-router GUI and IOS | IOS route distance, interface-only route and VRF syntax; recursive next-hop resolution |
| Routing domains | Scenario API; domain-aware routes in router GUI | Per-device interface binding editor and VRF CLI |
| BGP | Scenario API | Per-router configuration and operational commands |
| Packet/source filters | Scenario API; Linux can configure its own interfaces | Per-device ACL editor and router/switch CLI |
| DHCP pools | Scenario API; Linux can configure its own interfaces | Router pool configuration UI/CLI |
| Switch management | Scenario API and simplified `management ip` | GUI for VLAN/prefix/gateway; standard SVI CLI; switch-originated ping |
| Loop prevention | Automatic deterministic spanning forest | Player bridge priority and port state controls; BPDU protocol |
| Advanced startup settings | Saved and restored per device | Complete editable text representation in running/startup output |
| Neighbor diagnostics | IOS ARP table and server tools | IOS MAC table, neighbor clearing and aging controls |

A functioning scenario API does not mean the player can configure that feature
on a router. These missing controls must operate on the selected device's typed
configuration, with validation in the simulation; they should not introduce a
second global router configuration store.

## Model limits still present

- The C1111's LAN ports use routed-port semantics. Its embedded switch, bridge
  domains and SVIs are not modeled. A router plus external switch is the
  currently supported VLAN topology.
- BGP checks eligibility and installs semantic routes; it does not exchange TCP
  sessions or UPDATE messages, process full path attributes or run timers.
- Spanning tree computes a converged forest; it does not exchange BPDUs.
  Link aggregation, OSPF/IS-IS and IPv6 are absent.
- DHCP has explicit broadcast/return paths, but no relay or expiring leases.
  ARP/MAC state resets on topology/routing changes instead of timed aging.
- Filtering is stateless IPv4/protocol filtering, without transport-port rules
  or connection tracking. NAT is absent.
- Capacity is a physical path diagnostic, without queues, contention, QoS or
  congestion traffic. Fiber/transceiver behavior is absent.
- SSH into a guest is a simulated device console. AAA, device authentication,
  BMC behavior and production management-plane isolation are incomplete.
- Diagnostics use explicit external lab hosts. They never reach real networks.

See [provider operations](PROVIDER_NETWORK_GUIDE.md) and
[IOS commands](IOS_GUIDE.md) for the supported configuration workflow.
