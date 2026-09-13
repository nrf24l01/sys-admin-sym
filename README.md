# Cloud Provider Simulator

A focused Bevy 0.19 + egui 0.42 vertical slice about building one physical rack
and making its network work. The simulation models physical links, Ethernet,
VLANs, subnets, gateways and routing. Reachability is deterministic and
bounded; this is not a full protocol stack.

## Run

```bash
cargo run --release
```

The game starts with one empty 12U rack and $6,000.

1. Buy a Cisco ISR C1111-8P, Cisco Catalyst C1000-24T-4G-L, and Dell
   PowerEdge R360 servers in the shop.
2. Select an inventory device, open **RACK**, and click an empty rack unit.
3. Buy an APC Smart-UPS or PDU, place it in the rack, then connect active
   devices by clicking an empty power outlet or inlet and then its complementary
   socket. Click the same pending socket to cancel. Right-click an occupied
   socket and choose **Unplug cable**. Use the inspector to request device power;
   an unplugged device remains off. Press Escape to cancel an in-progress power
   or Ethernet connection.
4. In **RACK**, left-click an RJ45 socket and then another RJ45
   socket. Buy a 305 m cable box and RJ45 connector packs in the shop first;
   each new lead consumes its cut length and two plugs. Choose white, gray, blue,
   orange, or red for the jacket. The patch cable appears
   with visible plugs and a physically sagging, draggable jacket. Automatic cuts
   use the straight distance between sockets plus 5% slack, rounded up to a cm.
   Longer reusable leads can be selected explicitly in the shop. Switch link
   Port LEDs use separate link/status and activity indicators. Link/status shows
   the negotiated 10/100/1000 Mbps connection; activity reflects simulated
   traffic. Unplugging
   returns the finished lead for reuse. Right-click an RJ45 port to configure it
   without starting a cable. The Catalyst SFP cages are shown for physical
   accuracy but are intentionally inactive in the MVP.
5. Create VLANs on the switch, configure access/trunk ports, server IPv4, and
   router subinterfaces.
6. Select a switch or router to open its IOS-style console. Start with `enable`
   and `configure terminal`; use `?` for supported commands. VLANs, switchports,
   router IP addresses/subinterfaces, shutdown, and startup configurations are
   backed by the simulator. Each device has its own console and history.
7. Select a server and use `ip`, `route`, `arp`, `ping <ip>`, or
   `traceroute <ip>` in its terminal.

See [the IOS-style console guide](docs/IOS_GUIDE.md) for Cisco reference manuals,
a working switch/router configuration example, interface names, and the exact
compatibility limits. This is a simulated CLI subset, not Cisco IOS firmware.

Save/load uses `cloud-provider-save.db` in the current working directory.

The implemented network layers cover physical media (L1), Ethernet links and
switching (L2), and IPv4/VLAN routing (L3). Transport and application layers
(L4–L7) are outside the current scope.

## Development

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

The real-equipment reference textures and their sources are documented in
[`assets/equipment/ATTRIBUTION.md`](assets/equipment/ATTRIBUTION.md).
## Power model

The simulation exposes a deterministic `PowerSystem` domain model. Active
loads connect explicitly to one of four rack C13 outlets, a PDU outlet, or an
APC Smart-UPS inspired outlet; PDUs may themselves be fed by a UPS. Electrical
limits are evaluated at 230 V using integer W, VA and mA values. UPS defaults
are 1,000 W / 1,500 VA with four outlets; battery capacity and efficiency are
simulation parameters. Rack breakers and source trips latch until reset, and
`tick` advances battery discharge or recharge in simulated seconds.
The rack PDU uses a C14 inlet and is limited to 2,300 W / 10 A.
The default UPS uses a synthetic 900 Wh battery and 90% efficiency. The
reference APC SMT1500RMI2U is rated 230 V, 1,000 W / 1,500 VA with four C13
outlets: https://www.se.com/au/en/product/SMT1500RMI2U/.

Cisco ISR C1111-8P routers are supplied with a dedicated four-pin 66 W,
12 V / 5.5 A DC adapter. The adapter is modeled as a 90%-efficient, PF 0.90
AC load and is selected automatically by legacy `ConnectPower` commands;
explicit `ConnectPowerCord` commands validate that routers use the adapter and
other active devices use an IEC C13/C14 cord.
