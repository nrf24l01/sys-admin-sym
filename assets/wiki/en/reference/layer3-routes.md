# Layer 3, IP addresses and routes

**Layer 3** moves IP packets between networks. An interface address has a prefix:
`10.0.10.11/24` belongs to `10.0.10.0/24`. The prefix describes network membership;
a VLAN describes Ethernet membership. Devices need both to match for direct
same-network communication.

For a destination on its own subnet, a server uses ARP to find the destination
MAC address and sends directly. For a destination elsewhere, it selects a route
and typically sends the frame to a gateway's MAC while keeping the remote
**IP destination** in the packet.

A router consults its routing table and forwards through the selected interface.
The most specific matching prefix wins. A default route, `0.0.0.0/0`, handles
other destinations. The next-hop gateway must itself be reachable.

Both directions matter. Your default route sends traffic upstream; the upstream
needs a route back to your purchased public range. In this game, **IP RANGES →
Apply range delivery** supplies that upstream contract. It does not configure
your router or Linux interfaces. Buying an address range alone creates no route.

Private RFC1918 addresses such as `10.0.10.0/24` suit management. Customer Internet
endpoints need usable addresses from owned public ranges. This tutorial uses
explicit public routing; the simulation does not provide implicit NAT.

Diagnostics on Linux:

```sh
ip -br addr
ip route
ip route get 8.8.8.8
traceroute 8.8.8.8
```

On the router: `show ip interface brief`, `show ip route` and `traceroute ADDRESS`.
See the [customer routing lesson](../tutorial/data-network.md).
