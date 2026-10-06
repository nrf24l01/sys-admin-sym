# Provider networking foundation

This change models infrastructure, not VPS/cloud products or actual Internet access.
All operations remain deterministic and confined to the simulation.

## Object responsibilities

- `NetworkSim` coordinates commands, physical equipment and packet delivery.
- Each `Router` owns its routes. Its GUI binds destinations to local interfaces
  and next hops; connected routes derive from interface addresses.
- `ProviderNetwork` encapsulates external contracts and interface-scoped policies. Changes enter through
  validated `ProviderCommand` operations; snapshots expose read-only views.
- `Ipv4Prefix` owns prefix validation, containment and address-range semantics.
- `RoutedRangeHandoff` presents the higher gateway, WAN address and local
  gateway for an owned range. `RouteOwnedRange` changes its carrier delivery
  atomically; the player configures router interfaces independently.
- `TransitCircuit` owns the external handoff and upstream routing contract.
  Address ownership is independent of a circuit and of server addressing.
- `BgpSession` models explicit import/export policy and session eligibility.
  It is a control-plane model, not a TCP/BGP wire implementation.
- `InterfaceBinding` associates an L3 interface with a routing domain and role.
  Routing domains contain separate forwarding decisions; VLANs remain L2 scope.
- `PacketPolicy` owns ordered filtering and optional source-address validation.
- Routing chooses live next hops by longest prefix, preference and stable ECMP.
- Ethernet loop prevention computes a deterministic per-VLAN spanning tree.
  This represents converged forwarding, not BPDU exchange or protocol timers.
- Capacity reporting distinguishes NIC capacity, path bottlenecks and transit
  capacity. Physical state remains authoritative for failures.

Composition replaces inheritance. Value objects hold validated configuration;
services own one algorithm; no provider-specific logic belongs in Bevy rendering.
Commands must validate before mutating; learned state is never persisted.

Protocol references: [FRR BGP documentation](https://docs.frrouting.org/en/latest/bgp.html)
describes explicit import/export policy, prefix limits and route origination checks.
[RFC 3021](https://www.rfc-editor.org/rfc/rfc3021.html) defines usable `/31`
addresses on point-to-point links. These inform the configuration model; they do
not imply complete protocol conformance.

## Implementation order

1. Provider configuration types, validation and command integration.
2. Explicit Ethernet topology and spanning-tree forwarding.
3. Transit ARP, routed inbound/outbound packets, removal of WAN shortcuts.
4. Routing domains, packet policy, route preference and redundant next hops.
5. Management reachability and capacity diagnostics.
6. Console configuration, migration documentation and regression validation.

## Compatibility

Old saves deserialize. Legacy global route entries migrate to their owning router. Existing cables, addresses and routes remain. Rack LAN
sockets no longer imply a room-wide switch; wire real switches/patch panels.
Legacy WAN flags no longer manufacture Internet replies. Legacy /29 purchases
remain address inventory, but require an actual gateway, transit contract and
routes. No implicit NAT or upstream announcement is introduced by migration.

## Scope boundary

This foundation does not implement customer products, hypervisors, VXLAN/EVPN,
TCP application sessions, actual Internet traffic, or production deployment.
Physical device models retain their advertised hardware limitations. Protocol
models must document their bounds rather than presenting missing behavior as a
successful real-world connection.
