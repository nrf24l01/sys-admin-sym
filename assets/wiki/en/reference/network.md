# Networks and physical connections

A network lets devices exchange data along connected paths. An Ethernet port is
a socket on a device; a cable joins two sockets. A **link** is operational only
when the participating devices, interfaces and physical hardware support it.

A switch connects local Ethernet devices. A router connects IP networks. A server
runs the controller or customer workloads. A patch panel is passive: it organizes
cabling and does not replace a switch or regenerate an optical signal.

Typical small-provider wiring is: upstream handoff → edge router → switch →
servers. Management and customer traffic may share a switch while using separate
VLANs. Power must be connected separately from network cables.

Build from the bottom upward: power, physical link, VLAN transport, IP addresses,
routes, then application service. Higher layers cannot fix missing lower layers.

Read [Layer 2 and VLANs](layer2-vlans.md), then [Layer 3 and routes](layer3-routes.md).
