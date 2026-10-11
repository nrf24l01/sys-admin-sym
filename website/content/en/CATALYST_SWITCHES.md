# Catalyst 1000 switches

Both shop models represent real non-PoE, fanless Cisco products. Hardware comes
from `assets/equipment/switches.json`; the 4X is constructed from its own profile,
not converted from a purchased 4G. Existing purchase IDs and game prices remain
compatible with saves.

| Specification | C1000-24T-4G-L | C1000-24T-4X-L |
| --- | --- | --- |
| Copper ports | 24 × 10/100/1000 RJ45 | 24 × 10/100/1000 RJ45 |
| Optical uplinks | 4 × 1G SFP | 4 × 1/10G SFP+ |
| Copper interface names | Gi1/0/1–24 | Gi1/0/1–24 |
| Uplink interface names | Gi1/0/25–28 | Te1/0/1–4 |
| Switching capacity rating | 56 Gb/s | 128 Gb/s |
| Forwarding rating | 41.67 Mpps | 95.23 Mpps |
| Published idle / 100%-traffic power | 15.84 / 22.80 W | 18.00 / 25.68 W |
| Weight | 2.63 kg | 2.78 kg |

Both use an ARM v7 800 MHz CPU, 512 MB DRAM and 256 MB flash. Ratings are
manufacturer specifications, not a benchmark of this deterministic simulator.
The electrical simulation interpolates between idle and full-traffic values
using active link capacity and recent byte rates, adds installed transceiver
loads separately, then rounds to whole watts. A single ping stays near idle.
See [the power consumption guide](POWER_CONSUMPTION.md) for calculation details. Shop details and `show version` expose the source
values rather than the old generic 80 W estimate.

The 4X uses its own front photograph and copper socket centers. The matching
fanless rear enclosure uses the shared rear reference. Image sources and
specification references are in [ATTRIBUTION.md](../assets/equipment/ATTRIBUTION.md).

## EtherChannel

On two connected switches, configure matching-speed ports and identical access
VLAN or trunk/native/allowed-VLAN settings. For example, connect Gi1/0/1 and
Gi1/0/2 between the switches, then on each:

```text
enable
configure terminal
interface range Gi1/0/1 - 2
channel-group 1 mode active
end
show etherchannel summary
show etherchannel 1 detail
show interfaces Port-channel1
```

Groups 1–6 support static `on`, LACP `active`/`passive`, and PAgP
`desirable`/`auto`. Passive/passive and auto/auto do not form a channel.
Static links require `on` at both ends. LACP supports eight active members and
up to eight hot standby members; other modes support eight members. Different
local group numbers at the two ends are allowed.

`lacp system-priority` and interface `lacp port-priority` select the active LACP
members. Each flow uses one member, and broadcast crosses a bundle once.
Failed links trigger deterministic member selection and MAC forwarding failover.
Spanning tree treats the bundle as one link with aggregate capacity.

```text
configure terminal
port-channel load-balance src-dst-mac
interface Port-channel1
port-channel min-links 2
end
```

Other hash choices are `src-mac` (default) and `dst-mac`. Min-links is 2–8;
`no port-channel min-links` restores the one-link default. Logical interface
`shutdown` disables the bundle. VLAN and speed commands on the logical
interface apply atomically to its physical members. `no channel-group` removes
a physical interface from its bundle. Negotiation is converged state based on
configuration and carrier, not timed LACP/PAgP wire messages.

## QoS

QoS is disabled by default and preserves packet markings. Enabling it makes
ports untrusted by default: ingress DSCP and CoS are reset to the configured
default CoS (zero unless changed). Trust DSCP for IP traffic or trust CoS for
tagged traffic explicitly:

```text
configure terminal
mls qos
interface Gi1/0/1
mls qos trust dscp
interface Gi1/0/2
priority-queue out
srr-queue bandwidth share 25 25 25 25
srr-queue bandwidth limit 50
end
show mls qos interface Gi1/0/2 statistics
```

There are four output queues. Default CoS mapping is 0/1 → queue 2, 2/3 →
queue 3, 4/6/7 → queue 4 and 5 → queue 1. Override up to eight DSCP values
per command with `mls qos srr-queue output dscp-map queue 1 46`. Interface
`mls qos cos 3` sets the default CoS; `no mls qos trust` restores untrusted
classification. `no mls qos` restores pass-through and disables rate limiting.

Weighted queue service and optional strict priority on queue 1 operate within
packet batches (`NetworkSim::transmit_frames`). Egress limits accept 10–90% of
negotiated link speed, use simulation-time token budgets with a 10 ms burst
(minimum one MTU), and count transmitted packets, bytes and drops per queue.
DSCP rewrites remain in the IPv4 packet across routed forwarding. This is a
semantic queue model, not ASIC scheduling, WTD thresholds or physical buffering;
there is no wall-clock delayed packet queue.

## SNMPv2c

Give the switch a reachable management IP and configure communities:

```text
configure terminal
management ip 10.0.0.2
snmp-server community monitor ro
snmp-server community operator rw
snmp-server contact Ops team
snmp-server location Rack 1
end
show snmp
```

On a simulated Linux server with an IP and a network path to that switch:

```text
snmpget -v 2c -c monitor 10.0.0.2 sysDescr.0
snmpget -v 2c -c monitor 10.0.0.2 ifHighSpeed.25
snmpwalk -v 2c -c monitor 10.0.0.2 1.3.6.1.2.1.2.2.1.2
snmpset -v 2c -c operator 10.0.0.2 sysLocation.0 s Rack1
```

System OIDs provide model, per-device uptime, hostname, contact and location.
IF-MIB provides interface names, speed, MTU, administrative/operational state
and real simulator byte counters. High-capacity octet counters and
`ifHighSpeed` report 10G without 32-bit overflow. Interface indexes are 1–28;
index 25 is the first uplink on either model (its IOS name differs).
Writable OIDs are sysName, sysContact, sysLocation and ifAdminStatus (`i 1` up,
`i 2` down). Unknown OIDs and unauthorized writes fail explicitly.

Queries check ARP, routes, VLANs, carrier, switch power, UDP/IP filtering and
community permissions. These commands use the game's deterministic management
path. They do not open host sockets or implement BER encoding, traps, SNMPv3,
ACL-bound communities or a complete MIB collection.

## Saves and reload

EtherChannel, QoS and SNMP settings persist with the world and with
`write memory` / confirmed `reload`. Queue and SNMP counters are operational
state, reset when loading or reloading; uptime starts with device power-on.
Legacy 4X saves recover their model from the existing hardware ID, migrate
Te1/0/25–28 names and descriptions to Te1/0/1–4, and retain port IDs, cables,
modules, VLANs and startup configurations.

Specifications: [Cisco data sheet](https://www.cisco.com/c/en/us/products/collateral/switches/catalyst-1000-series-switches/nb-06-cat1k-ser-switch-ds-cte-en.html)
and [hardware overview](https://www.cisco.com/c/en/us/td/docs/switches/lan/catalyst1000/hardware/installation/24_48_port_hig/b_c1000_24_48_hig/product_overview.html).

Native shop checks: [before](screenshots/catalyst-shop-before.png),
[after in English](screenshots/catalyst-shop-after-en.png),
[Russian hardware details](screenshots/catalyst-shop-after-ru.png).
