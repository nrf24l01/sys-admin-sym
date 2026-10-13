# Network device models

`assets/equipment/network_devices.json` is the authoritative router/switch
catalog. The installed JSON is loaded at startup, with the bundled catalog as a
fallback when no installed file exists. Restart after editing it; recompiling
Rust is unnecessary. The shop, simulation, localization and texture loader use
the same registry. Existing `router_config.json`, `switch_config.json` and
`switches.json` are historical asset references, not model definitions.

## Add a model

1. Copy a model entry and assign unique `id` and `shop_id` values. Use lowercase
   letters, digits and underscores for saved IDs. `shop_id` normally equals `id`;
   the existing default offers retain old shop IDs for compatibility.
2. Set `category` (`router` or `switch`), `manufacturer`, `family`, `name`, EN/RU
   `display_name` and `desc`, `price`, `rack_units`, and `routing_enabled`.
3. Replace `ports` with the built-in data interfaces. Each entry defines `name`,
   `ios_name`, `connector`, `role`, `speed_mbps`, `side`, `socket.rect`, optional
   `cage`, `optical_socket`, optional `leds.status`/`leds.activity` positions,
   `flex`, `trunk`, and `default_wan`.
4. Set the operating `power` curve, optional DC `adapter`, AC `psu` rating and
   efficiency curve, and `power_inlets`. Each inlet has a connector, side and
   normalized socket rectangle. Multiple AC inlets support redundant feeds.
   Use `IecC14` for AC inlets without an adapter, or `CiscoFourPin` /
   `CiscoSixPin` for a DC inlet with its supplied `adapter` profile. The profile
   supplies the voltage, current, capacity and conversion efficiency.
5. Supply `artwork.front` and `artwork.rear`, each with `path`, normalized `uv`
   and `source_size`, using Cisco documentation artwork or clean product
   catalog images. Match the C1000-24T-4G-L reference: a complete, straight-on chassis panel filling the crop from edge
   to edge. Rectify camera perspective when necessary; exclude studio padding,
   case lids and sidewalls. Prepared panel images use the full UV rectangle
   `[0, 0, 1, 1]`, 2200 pixels wide and 240 pixels high per rack unit. Set socket
   rectangles against this rendered face, never against a generic grid or the
   uncropped source image. Prefer clean manufacturer documentation panels
   with even lighting and crisp port details. Avoid used-hardware camera shots.
   Do not use generated schematic faces.
   Use a distinct photograph of the opposite face for `artwork.rear`; do not
   repeat the port-side image. `Front` is the game's interactive I/O face,
   even when manufacturer documentation calls it the rear. Align each power
   inlet with the face on which its physical socket appears. For example, the
   C1111 four-pin and ISR4321 six-pin inputs belong on the I/O face;
   their opposite bezels contain no power sockets.
6. Add `source` and `notes`; record image provenance in `ATTRIBUTION.md`.
7. Run `make check`. Invalid catalogs identify the model and field at startup.

[Artwork comparison with the C1000 reference](images/cisco-artwork-comparison.jpg)
shows the current catalog faces at equal rack width, starting with the C1000 reference.

[Router front/rear comparison](images/cisco-router-front-rear-comparison.jpg)
shows every router's two faces with the configured power-inlet bounds marked.

[Corrected rear panels](images/cisco-network-rear-comparison.jpg) show the requested
router and switch repairs with power socket bounds. The [previous crops](images/cisco-network-rears-before.jpg)
are retained for comparison. Rear panel preparation removes background padding and
straightens the photographed panel; case lids and projecting PSU extraction grips
outside the panel plane are excluded from the playable face.

Removing a catalog model also removes its purchase offer and artwork. Saves that
still contain that model cannot load until its saved devices are removed; unknown
identities are rejected instead of silently substituting different hardware.

All socket rectangles use `[left, top, right, bottom]` in the rendered face's
normalized coordinates. Ports have a stable array order: saved links use
concrete port IDs, so do not reorder or remove interfaces on a model already in
use. Create a new model ID for a different hardware configuration.

`routing_enabled` selects the existing routing/SVI implementation. Switches
with routing enabled keep switch shop classification and switching services.
`role: "switch"` initializes an access or trunk port; `role: "routed"`
initializes a routed port. `flex: true` permits conversion between those roles.
`default_wan` supplies the legacy DHCP/Internet designation; new models normally
leave it false and use explicit interface and routing configuration.

An RJ45 interface with a `cage` and `optical_socket` represents a combo socket.
Installing an optic selects its optical socket; removing it restores copper.
Both sockets share one port ID, configuration and cable ownership. Disconnect
its copper cable before installing an optic. Management and console sockets are
excluded from playable data interfaces. Optional expansion modules are absent.

Cages define `kind`, a power budget and exact Ethernet `modes` (speed, lane
count and FEC). Supported speeds are 10/100 Mb/s and 1/10/25/40/100 Gb/s. SFP,
SFP+, SFP28, QSFP+ and QSFP28 families retain physical compatibility rules;
QSFP is not interchangeable with SFP. 40/100G LR4 optics use the existing OS2
LC-duplex assemblies and a simplified aggregate optical budget.

`mode_blocks` describes shared hardware resources: an installed/enabled optic
on `trigger_port` at or above `min_speed_mbps` disables the indices listed in
`blocked_ports`. Rules are model data and are evaluated by `link_status`.
The C8500's bay 1 QSFP at 40G excludes its four SFP+ sockets, and at 100G also
excludes bay 0. Bay 2's 100G mode excludes its two additional 40G sockets.
Breakout interfaces and alternate firmware modes are not simulated.

Prices and operating power are game estimates. Published PSU/adapter ratings, interface counts, cage families and chassis
heights follow the referenced hardware. Fixed internal supplies whose output
ratings are not documented carry an explicitly marked estimated game ceiling
in the model notes; operating draw and efficiency curves are game estimates.
Business, IOS XE and Nexus models share the simulator's console dialect; no
vendor firmware, licensing, dynamic routing or stack/backplane is emulated.
C9300-24T-E is the base chassis with 24 data ports and no optional uplink module.
Existing save files retain defaults, concrete port/cable identities and VLANs;
unknown saved model IDs produce a load error rather than silently changing
hardware. Legacy C1000 RON enum identifiers remain readable.

## Cisco hardware references

Every entry carries its primary URL in `source`. Family references:

- [ISR 1100](https://www.cisco.com/c/en/us/products/collateral/routers/1000-series-integrated-services-routers-isr/datasheet-c78-739512.html)
- [ISR 4000](https://www.cisco.com/c/en/us/products/collateral/routers/4000-series-integrated-services-routers-isr/data_sheet-c78-732542.html)
- [Catalyst 8200](https://www.cisco.com/c/en/us/products/collateral/routers/catalyst-8200-series-edge-platforms/nb-06-cat8200-series-edge-plat-ds-cte-en.html)
- [Catalyst 8300](https://www.cisco.com/c/en/us/products/collateral/routers/catalyst-8300-series-edge-platforms/cat-8300-series-edge-qa.html)
- [ASR 1001-X](https://www.cisco.com/c/en/us/td/docs/routers/asr1000/install/guide/1001-x/asr1hig-book/router_overview.html)
- [Catalyst 8500](https://www.cisco.com/c/en/us/products/collateral/routers/catalyst-8500-series-edge-platforms/datasheet-c78-744089.html)
- [C8500 bay layout](https://www.cisco.com/c/en/us/td/docs/routers/cloud_edge/c8500/hardware-installation-guide/b_C8500_HIG/m_Router_Overview.html)
- [Business 250](https://www.cisco.com/c/en/us/products/collateral/switches/business-250-series-smart-switches/nb-06-bus250-smart-switch-ds-cte-en.html)
- [Business 350](https://www.cisco.com/c/en/us/products/collateral/switches/business-350-series-managed-switches/datasheet-c78-744156.html)
- [Catalyst 1000](https://www.cisco.com/c/en/us/products/collateral/switches/catalyst-1000-series-switches/nb-06-cat1k-ser-switch-ds-cte-en.html)
- [Catalyst 1300](https://www.cisco.com/c/en/us/products/collateral/switches/catalyst-1300-series-switches/nb-06-cat1300-ser-data-sheet-cte-en.html)
- [Catalyst 2960-X](https://www.cisco.com/c/en/us/products/collateral/switches/catalyst-2960-x-series-switches/datasheet_c78-728232.html)
- [Catalyst 9200](https://www.cisco.com/c/en/us/products/collateral/switches/catalyst-9200-series-switches/nb-06-cat9200-ser-data-sheet-cte-en.html)
- [Catalyst 9300](https://www.cisco.com/c/en/us/products/collateral/switches/catalyst-9300-series-switches/nb-06-cat9300-ser-data-sheet-cte-en.html)
- [Catalyst 9500](https://www.cisco.com/c/en/us/products/collateral/switches/catalyst-9500-series-switches/nb-06-cat9500-ser-data-sheet-cte-en.html)
- [Nexus 93180YC-FX3](https://www.cisco.com/c/en/us/products/collateral/switches/nexus-9000-series-switches/datasheet-c78-744052.html)
- [Nexus 9364C-GX](https://www.cisco.com/c/en/us/products/collateral/switches/nexus-9000-series-switches/nexus-9300-gx-series-switches-ds.html)
