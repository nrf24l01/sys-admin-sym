# Cloud Provider Simulator

A focused Bevy 0.19 + egui 0.42 vertical slice about building one physical rack
and making its network work. The simulation is semantic (topology, VLANs,
subnets, gateways and routing), not packet-by-packet.

## Run

```bash
cargo run --release
```

The game starts with one empty 12U rack and $6,000.

1. Buy a router, switch, and servers in the shop.
2. Select an inventory device, open **RACK**, and click an empty rack unit.
3. Power devices from the inspector.
4. Select ports and click **Use for cable** on both endpoints.
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

The real-equipment development textures and their licenses are documented in
[`assets/equipment/ATTRIBUTION.md`](assets/equipment/ATTRIBUTION.md).

