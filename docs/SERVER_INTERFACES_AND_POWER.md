# Dell server interfaces and redundant power

The R360 exposes two onboard 1 Gb Ethernet interfaces, `eth0` and `eth1`.
Installed NICs add their own interfaces. SSH uses a configured Ethernet
interface selected by the Linux routing table; it accepts secondary and VLAN
addresses too. There is no synthetic `mgmt0` OS interface or socket. Dell's
dedicated iDRAC connector is a separate BMC and is not simulated as a Linux NIC.

Both rear C14 sockets are usable independently. PSU 1 is the right socket
(the original inlet); PSU 2 is the left socket. Connect either socket first,
then an outlet, or start at the outlet. Each cord has its own anchors,
selection, unplug action and AC load. The inspector shows both feeds and their
draw, and warns when only one feed is available.
The inspector scrolls so every installed NIC remains accessible below the
hardware and power controls.

The simulated supplies run in balanced load-sharing mode. Live supplies split
the actual DC component demand, and each converts its share to AC using its
own load-dependent efficiency. Two connections do not charge the full server
load twice. If one input disappears, the other immediately carries the whole
load, provided its rating and the upstream source limits permit it. A source
overload can cause cascading protection trips in the same simulation update.
Two cords on the same rack, PDU, or UPS share its failure; use independent
A/B feeds when redundancy is required. A UPS can keep a feed live on battery.

Losing every usable feed or exceeding remaining PSU capacity stops the
server. Complete power loss ends SSH sessions, stops guest services and
clears temporary IP addresses and routes. When requested power is still on,
restoring a feed boots the server, starts enabled services and reapplies
`/etc/network/interfaces`. Services follow their saved enable state.
A single-feed failover does not reboot or clear the guest's network
configuration. Disk files survive power loss.

## JSON configuration

- `server_parts.json`: `chassis.psu_bays` defines included supplies and usable
  inlets (1–2 for this chassis). Both 600 W supplies are included by default.
- `server_config.json`: `power.psu.capacity_watts` is the DC rating of **each**
  PSU, `power.psu.efficiency_permille` is its efficiency curve, and
  `power.power_factor_percent` determines AC VA/current.
- Component operating envelopes remain in the equipment/parts/drive `power`
  objects. UPS/PDU limits and losses remain in their own JSON catalogs.
- Socket rectangles live in `server_config.json` under `ports.c14`;
  `ports.rj-45` contains only the two onboard NICs.

Efficiency and component operating curves are gameplay estimates, not measured
R360 telemetry. Dell's optional hot-spare sleep policy is not simulated; this
model explicitly uses both live PSUs in load-sharing mode. Off-state BMC and
standby consumption and millisecond boot/switchover timing are not modeled.

## Save compatibility

Legacy first-inlet cords retain their identity, routes and connections.
Loading an old save removes `mgmt0`, preserves real Ethernet/NIC port IDs,
and returns any lead attached to the removed port as a finished cable once.
Bulk cable and RJ45 connectors are not refunded. Removed-interface IP addresses
and routes are not transferred automatically; configure a real NIC instead.
Text files referring to `mgmt0` remain available for editing.
Both PSU connections persist in new saves.

## References and previews

- [Dell R360 NIC specifications](https://www.dell.com/support/manuals/en-bm/poweredge-r360/r360_ism/nic-port-specifications?guid=guid-7be6485b-7c21-4030-9f52-d3a10af9cc8e)
- [Dell R360 PSU specifications](https://www.dell.com/support/manuals/en-ca/poweredge-r360/r360_ism/psu-specifications?guid=guid-ff8a8542-c09e-4bb9-9b35-710b1366b973&lang=en-us)
- [Dell hot-spare policy](https://www.dell.com/support/manuals/en-us/poweredge-r360/r360_ism/hot-spare-feature?guid=guid-0bac5ab4-79f5-4a6b-a473-83e87d82b321)

Native previews use a private full-pack server save; the user's save is unchanged.

| Language | Before | Both PSUs connected | All installed Ethernet interfaces |
| --- | --- | --- | --- |
| English | [Before](screenshots/dell-server-before-en.png) | [After](screenshots/dell-server-after-en.png) | [Scrolled inspector](screenshots/dell-server-interfaces-en.png) |
| Russian | [Before](screenshots/dell-server-before-ru.png) | [After](screenshots/dell-server-after-ru.png) | [Scrolled inspector](screenshots/dell-server-interfaces-ru.png) |

In this idle fixture, both feeds draw 24 W each (48 W total). Unplugging PSU 1
leaves the server online with [PSU 2 drawing 46 W](screenshots/dell-server-one-feed-en.png).
The different total follows the configured efficiency curve at each PSU's load.
Cutting the remaining source leaves the [server offline at 0 W](screenshots/dell-server-outage-en.png).
These values demonstrate the estimated model, rather than measured Dell power draw.
