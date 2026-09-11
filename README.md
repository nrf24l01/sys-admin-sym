# Cloud Provider Simulator

A focused Bevy 0.19 + egui 0.42 vertical slice about building one physical rack
and making its network work. The simulation is semantic (topology, VLANs,
subnets, gateways and routing), not packet-by-packet.

## Run

```bash
cargo run --release
```

The game starts with one empty 12U rack and $6,000.

1. Buy a Cisco ISR C1111-8P, Cisco Catalyst C1000-24T-4G-L, and Dell
   PowerEdge R360 servers in the shop.
2. Select an inventory device, open **RACK**, and click an empty rack unit.
3. Power devices from the inspector.
4. In **RACK**, left-click a highlighted RJ45 socket and then another RJ45
   socket. The patch cable appears between the physical connectors. Right-click
   an RJ45 port to configure it without starting a cable. The Catalyst SFP
   cages are shown for physical accuracy but are intentionally inactive in the
   MVP.
5. Create VLANs on the switch, configure access/trunk ports, server IPv4, and
   router subinterfaces.
6. Select a server and use `ip`, `route`, `arp`, `ping <ip>`, or
   `traceroute <ip>` in its terminal.

Save/load uses `cloud-provider-save.db` in the current working directory.

## Development

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

The real-equipment reference textures and their sources are documented in
[`assets/equipment/ATTRIBUTION.md`](assets/equipment/ATTRIBUTION.md).
