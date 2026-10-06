# Provider network operations

Configure each device through its own Inspector and console. Select a router (or
one of its routed ports) and open **Routing table…** to add, edit or remove routes.
There is no central NETWORK configuration window. Each router stores its own
routes and forwards packets only through its own interfaces and physical links.

## Configure a routed connection in the GUI

1. Open **IP RANGES** from the top bar (beside Settings). Order a `/29` range,
   select it in the range list, select the delivery uplink and click **Apply range
   delivery**. The page shows the higher network subnet/gateway, your router's
   WAN address, and the LAN gateway address for this range.
2. Cable that uplink socket to your router's WAN interface. For the first uplink,
   the default handoff is `192.0.2.0/30`: higher network gateway `192.0.2.1`,
   router WAN address `192.0.2.2/30`. Configure the WAN address on your router.
   Existing scenario handoffs retain their own addressing; always use the page's values.
3. Open that router's **Routing table…**. Add destination `0.0.0.0/0`, select its
   WAN interface, choose **Via gateway**, and enter the higher network gateway.
   Alternatively, in IOS global configuration mode use `ip route 0.0.0.0 0.0.0.0 GATEWAY`.
4. Configure another router interface with the range's LAN gateway. For range
   `203.0.113.0/29`, this is `203.0.113.1/29`. Connect servers directly or through
   a switch to that LAN interface. Connected subnet routes appear automatically.
5. Configure server addresses in the range, for example `203.0.113.2/29`, gateway
   `203.0.113.1`. Power all devices and test with `ping 8.8.8.8`.

Applying range delivery installs the carrier's route to the displayed WAN IP.
It leaves player interfaces and routing tables for the player to configure.
Multiple ranges on one uplink share its handoff subnet and WAN next hop. Moving
a range removes its old carrier route; update your router's WAN addressing,
default route and cabling using the new uplink's instructions. Delivery choices
persist with saves. Selecting **Unassigned** removes the carrier route.

For multiple routers, configure forward and return routes on **every** router.
A route on the edge router does not configure a downstream router. Each next hop
must be reachable through the chosen interface's subnet and actual cabling.
Lower route preference wins for equal prefixes; optional neighbor tracking
withdraws a route when ARP resolution fails. Editing validates the replacement
before removing the original route. IOS `show ip route` displays the device's
configured connected and static routes, including GUI routes.

Configuration persists with ordinary saves. Diagnostics use simulated Ethernet,
ARP and IPv4 forwarding; they never send traffic to the host computer's network.

## Advanced scenario command reference

The examples below document `ProviderConsole::run` for scenario tooling. It is
not an in-game global controller. Linux `netctl` can configure only the current
guest's own interfaces; it cannot change another device or the external carrier.
Range delivery is configured in **IP RANGES**, and router routes in that
router's routing editor. Advanced BGP/filtering scenario APIs remain available;
complete per-device GUI editors for those features are not yet implemented.

## A routed provider with static transit

Cable a server to a router's LAN interface, and the router's copper WAN interface
to an uplink socket. A switch or passive patch panel may be inserted in either
path. Power and install all active devices.

Configure the router WAN as `192.0.2.2/30` and LAN as `203.0.113.1/24` using
its Inspector or IOS console. Configure the server as `203.0.113.10/24`, gateway
`203.0.113.1`. An untagged server interface can leave Access VLAN blank; a switch
access port then selects its broadcast domain.

The following example assumes uplink port `1`, router WAN port `100`, router
LAN port `102`, router device `20`, and server data port `110`. Substitute the IDs
from your world. Commands use numeric simulation IDs, not interface names.

```text
show ports
pool add 203.0.113.0/24 provider-aggregate
transit add 1 192.0.2.1/30 64501 1000 transit-A
upstream-route add 1 203.0.113.0/24 192.0.2.2
route add 20 0 0.0.0.0/0 100 1 192.0.2.1 1 keep
ping-in 203.0.113.10
show transit
show capacity
```

From the server, `ping 8.8.8.8` checks the outbound path. `ping-in` originates an
inbound echo request from the external diagnostic host `198.51.100.1`. Both need
working return routes. Address ownership alone does not create connectivity.

The IPv4 shop purchases address inventory independently of uplinks. Its `/29`
assignment button uses a convenience layout with the first usable address
reserved for your gateway. For a different layout, add an owned pool and use:

```text
allocate 110 203.0.113.0/24 203.0.113.1 untagged
```

Allocation avoids used device, transit, management and lease addresses. `/31`
pools can use both addresses; a `/32` can allocate its single address. Explicit
server routes are still needed when the allocated address has no on-link gateway.
Private addresses require deliberate translation for public Internet access;
NAT is not implemented by this foundation.

## Replace the static upstream contract with eBGP

With the same topology, remove the static default and upstream return contract,
then configure the session and its policies:

```text
route delete 20 0 0.0.0.0/0 100 1 192.0.2.1 1 keep
upstream-route delete 1 203.0.113.0/24 192.0.2.2
offer add 1 0.0.0.0/0
authorize add 1 203.0.113.0/24 24 64500
bgp add 100 1 1 64500 64501 10 20
bgp import 100 1 0.0.0.0/0 add
bgp export 100 1 203.0.113.0/24 add
show bgp
```

A session needs a reachable upstream peer, matching peer ASN, distinct local and
peer ASNs, and an accepted-prefix count within its limit. Imports and exports
are explicitly configured; an empty policy imports or exports nothing. An import
prefix permits itself and more-specific prefixes contained within it.

Exports need owned inventory, a matching origin authorization and a matching
connected/static route in the edge's routing domain. This public-transit model
rejects private and special-use source ranges and global IPv4 announcements more
specific than `/24`; small customer subnets should sit inside a larger announced
aggregate. Documentation prefixes are usable within the simulation.

`show bgp` reports session state and export policy eligibility. Routes are
installed only while the session is established. `transit state 1 down` disables
the circuit; `bgp state 100 1 down` administratively disables the session.

This models eBGP eligibility and forwarding consequences. TCP handshakes, BGP
UPDATE messages, full path attributes, keepalive/hold timers, IRR and live RPKI
repositories are outside the current model.

## Routing domains and filtering

VLANs select broadcast domains. Routing domains select separate router tables.
Bind each relevant router interface and add routes to that domain explicitly:

```text
bind 102 1 10 private
bind 100 1 10 public
route add 20 10 0.0.0.0/0 100 1 192.0.2.1 1 keep
show bindings
show routes
```

Legacy IOS static routes stay in domain `0`. A route explicitly configured in
domain `10` does not appear in domain `20`, including on different subinterfaces
of one physical port. Roles (`public`, `private`, `management`, `storage`, `oob`)
describe intent; choose domains and packet policies to enforce access.

Packet rules are ordered, with the first match deciding. For example, allow
only assigned source addresses at an access/edge interface, deny one destination,
and permit other traffic:

```text
policy in 102 1 permit
sources 102 1 203.0.113.10/32
rule in 102 1 0.0.0.0/0 8.8.8.8/32 1 deny
show policies
```

Protocol `1` is ICMP; `any` matches every modeled IPv4 protocol. Policies can be
attached to server, switch or router interfaces. Source validation also applies
to ARP sender addresses. Rules are stateless; transport ports and connection
tracking are not modeled. `policy delete in 102 1` removes the entire attachment.
DHCP discover/offer messages are semantic Ethernet exchanges and do not currently
pass through IPv4 protocol/port filtering.

## Redundancy and capacity

Configure a second uplink with its own handoff subnet and routes or BGP session.
Lower route preference wins after longest-prefix selection. Equal-cost routes
use a stable source/destination hash. Link failure removes routes using that
physical interface. BGP loss removes learned routes immediately.

`keep` leaves a static route installed while its interface remains up, even if
its next hop stops answering ARP. `track` withdraws it while ARP resolution fails.
An untracked unresolved `/32` continues to take precedence over a working `/24`.

Switches use a deterministic per-VLAN spanning forest by default. It blocks
redundant bridge links and recomputes after topology changes. `stp SWITCH off`
disables that switch's participation. This is converged loop prevention, with
bounded forwarding and loop-drop telemetry; it does not emulate IEEE BPDU
exchange, bridge priorities or convergence timers. Link aggregation is not modeled.

`show capacity` separates server-facing NIC capacity from physical transit
capacity and the remaining transit capacity after the largest circuit fails.
`path_capacity_mbps` in the domain API reports a traced path's bottleneck.
These are capacity diagnostics, not a traffic scheduler: queueing, congestion,
DDoS traffic and shared-fabric throughput are not simulated.

## Management and DHCP

Use explicit management switches and cables. Rack LAN sockets no longer join
racks automatically, and plugging into a bare socket does not create a DHCP
server. Existing patch-panel devices provide explicit paired front/rear ports.

```text
management 30 10.10.0.1/24 99 none
dhcp add 102 1 203.0.113.0/24 203.0.113.20 203.0.113.30 203.0.113.1
```

The first command configures switch device `30` with a management address on
VLAN `99`. The VLAN and forwarding path must be active. Management SSH uses the
same routing, VLAN and filtering checks as packets, and server SSH service state
still controls access. A management ping is not an authentication check.

The second command configures a DHCP pool on an addressed server/router interface.
A Linux `iface eth0 inet dhcp` stanza acquires a lease only through an actual
broadcast and return path. Leases persist; deleted clients/servers are removed
from lease records. Lease expiration, relays and DHCP packet serialization are
outside the model. `mgmt0` is an OS management NIC; it does not emulate a BMC
that survives the server OS being powered off.

## External diagnostic hosts

The default external lab includes `198.51.100.1`, `8.8.8.8` and `1.1.1.1`.
Other public addresses do not automatically answer:

```text
external add 9.9.9.9 echo up
external add 9.9.9.9 silent up
external delete 9.9.9.9
show external
```

## Existing saves

Old saves remain readable. Duplicate legacy router records for untagged/VLAN 1
are repaired in both running and startup configuration on load, preferring an
addressed record over an unassigned placeholder. Valid tagged subinterfaces
remain separate. Save after loading to persist the repaired records.

Purchased `/29` blocks are imported as address
inventory; their old uplink field records purchase provenance only. Existing
addresses and cables remain, but configurations relying on shared rack sockets
or the legacy WAN flag need explicit switching, transit and routes.

IPv6, optical transceivers/fiber, overlays, cloud workloads and real network
access remain outside this implementation. The object boundaries are described
in [the design document](PROVIDER_NETWORK_DESIGN.md).
