# Equipment reference image attribution

## Shop catalog artwork

- `shop_products.png`: generated for this project with the built-in OpenAI image
  generation tool and genuine alpha transparency. The 4 × 3 atlas contains CPU,
  RAM, four-port RJ45 NIC, two-cage SFP+ NIC, HDD, SSD, bulk cable box, RJ45 plug
  pack, copper/fiber patch panels, cable manager and address-allocation artwork.
- These are unbranded representative illustrations, not manufacturer photos.
  Decorative socket/plug counts do not supply simulator specifications.
- Complete padded cells and source-correct aspect ratios are mapped in
  `crates/game/src/ui/shop/artwork.rs`. Existing rack and optical artwork retains
  its attribution below. No existing connector or rack assets were replaced.
- The final prompt is preserved in [SHOP_ARTWORK_PROMPT.md](SHOP_ARTWORK_PROMPT.md).

## Generated power equipment artwork

- `ups_faces.png`: representative APC Smart-UPS front/rear texture atlas.
- `pdu_faces.png`: representative eight-outlet rack PDU front/rear atlas.
- `power_connectors.png`: transparent IEC sockets/plugs, Cisco four-pin DC plug,
  and external adapter sprite atlas.
- Created for this project using the built-in OpenAI image generation tool.
  These are game illustrations, not manufacturer photos; tiny rendered labels
  are decorative. Electrical ratings are supplied by the simulator and UI.
- The prompts are preserved in [POWER_TEXTURE_PROMPTS.md](POWER_TEXTURE_PROMPTS.md).
- Cisco connector reference: [Cisco 1000 hardware installation guide](https://www.cisco.com/c/en/us/td/docs/routers/access/1100/hardware/installation/guide/b-cisco-1100-series-hig/isr1k-hig-overview.html).

These manufacturer images are used as visual references in the rack UI. Product
names and trademarks belong to their respective owners. The source-code license
does not grant rights to these images; review the manufacturer terms before
redistributing a packaged build.

The game currently loads `server_rear_clean.png`, `switch_front_clean.png`, and
`router_rear_clean.png`. In the simulation, the connector-bearing
`router_rear_clean.png` is presented as the router's interactive/front face;
`router_front_clean.png` is the physical bezel side and is an opposite-face
reference. The additional opposite-face references are `server_front.png`,
`switch_rear.png`, and `router_front_clean.png`. Small labels and details may
differ from the source images. Socket hitboxes are aligned to the displayed
derivatives.

## Current Cisco rack textures

- `router_rear_clean.png`: Cisco ISR C1111-8P, 2172 × 724 pixels.
  Source: IT Yuda product photograph supplied by the user.
  https://ityuda.ca/cdn/shop/products/cisco-c1111-8p-isr-1100-8-ports-dual-ge-wan-ethernet-router-it-yuda-12394208526381.jpg?v=1753501936
- `switch_front_clean.png`: Cisco Catalyst C1000-24T-4G-L, 2200 × 715 pixels.
  Source: Google-hosted thumbnail supplied by the user; original publisher unknown.
  https://encrypted-tbn0.gstatic.com/images?q=tbn:ANd9GcR5B30rRACOmf_wSaokUQubg8MkZHwilI0E_D5hNtDtLA&s=10
- Processing: OpenAI image tool, upscale and isolate the front panel.

The older reference assets below are retained for reference.

## Cable textures

- `assets/cables/rj45_plug.png`: generated transparent end-on RJ45 plug sprite,
  viewed from the cable outlet with the connector seated in its socket.
- `assets/cables/pvc_jacket.png`: generated repeatable dark PVC jacket texture.
- Processing: OpenAI image tool, generated for this project as generic game
  materials; these assets do not depict a branded product.

## `server_rear.png`

- Model represented: Dell PowerEdge R360
- Source: Dell PowerEdge R360 Installation and Service Manual, rear view
- URL: https://www.dell.com/support/manuals/en-uk/oth-r360/r360_ism/rear-view-of-the-system?guid=guid-09ee6b2e-7833-41a8-8075-56b4de6964dd&lang=en-us
- Direct image: https://dl.dell.com/content/guides/public/Html/r360_ism/images/GUID-1EA46641-4637-4621-BA2C-16A366798F5F-low.jpg
- Copyright: Dell Inc.

## `switch_front.png`

- Model represented: Cisco Catalyst C1000-24T-4G-L
- Source: Cisco Catalyst 1000 Series Hardware Installation Guide
- URL: https://www.cisco.com/c/en/us/td/docs/switches/lan/catalyst1000/hardware/installation/24_48_port_hig/b_c1000_24_48_hig/product_overview.html
- Direct image: https://www.cisco.com/c/dam/en/us/td/i/300001-400000/350001-360000/356001-357000/356400.jpg
- Copyright: Cisco Systems, Inc.

## `switch_4x_front.jpg`

- Model represented: Cisco Catalyst C1000-24T-4X-L; dedicated front photograph.
- Source: SCT Systems model-specific product listing.
- Page: https://www.sct-systems.com/catalog/product_info.php?products_id=4228
- Original image: https://www.sct-systems.com/catalog/images/Cisco-Catalyst-C1000-24T-4X-Switch.jpg
- Original JPEG retained without pixel edits; front-face UV and socket centers are in the UI.
- Product branding: Cisco Systems, Inc.; photograph distributed by SCT Systems.
- Both models share the existing rear artwork because both have the same fanless rear enclosure.

## Catalyst hardware specifications

- `switches.json`: model-specific ports, cage modes, idle/100%-traffic consumption,
  switching bandwidth, forwarding rate and weight.
- Source: https://www.cisco.com/c/en/us/products/collateral/switches/catalyst-1000-series-switches/nb-06-cat1k-ser-switch-ds-cte-en.html
- Interface numbering: https://www.cisco.com/c/en/us/td/docs/switches/lan/catalyst1000/hardware/installation/24_48_port_hig/b_c1000_24_48_hig/product_overview.html
- Rated full-traffic consumption cross-check: https://www.cisco.com/c/en/us/td/docs/switches/lan/catalyst1000/hardware/installation/24_48_port_hig/b_c1000_24_48_hig/technical_specifications.html
- Rack electrical budgets round milliwatts up to whole watts. Separately simulated
  transceiver draw is added conservatively; this is not a calibrated meter model.

## `router_front.png`

- Model represented: Cisco ISR C1111-8P I/O panel
- Source: Cisco 1000 Series Integrated Services Router Hardware Installation Guide
- URL: https://www.cisco.com/c/en/us/td/docs/routers/access/1100/hardware/installation/guide/b-cisco-1100-series-hig/isr1k-hig-overview.html
- Direct image: https://www.cisco.com/c/dam/en/us/td/i/300001-400000/360001-370000/366001-367000/366944.jpg
- Copyright: Cisco Systems, Inc.

## Additional opposite-face references

- `server_front.png`: Dell PowerEdge R360 4 x 3.5-inch front product photograph.
  Source: Dell Technologies Official Store UAE product listing
  (direct image: https://www.dellonline.ae/cdn/shop/files/b6f2c4eb-071e-402b-8be3-4480bdfb8aad-2.jpg?crop=center&height=1200&v=1755248709&width=1200).
  No explicit reuse license was stated on the listing; Dell and the
  photographer retain their rights.
- `switch_rear.png`: Cisco Catalyst 1000 rear panel product photograph,
  C1000-24P-4G-L chassis (same rear enclosure family as C1000-24T-4G-L).
  Source: TelQuest International product gallery (direct image:
  https://www.telquestintl.com/site/images/products/C1000-24P-4G-L-RF.Media-3.jpg?resizeh=1000&resizeid=4&resizew=4000).
  No reuse license was stated on the retailer listing; Cisco and the
  photographer retain their rights. Cisco's official rear-panel figure is
  linked above as the model-reference source.
- `router_front_clean.png`: Cisco ISR 1100 Series C1111-8P bezel/front product
  photograph. Source: Network Devices Inc. product gallery (direct image:
  https://networkdevicesinc.com/cdn/shop/files/c1111-8p_1.webp?v=1737716613).
  No reuse license was stated on the retailer listing; Cisco and the
  photographer retain their rights. This is the physical bezel side and has no
  network sockets; keep the connector-bearing C1111-8P I/O face for any
  interactive socket overlay.
- Processing for these three files: scale to the rack panel aspect ratio and
  PNG conversion only; the source photos retain their white margins and server
  lid perspective. The UI applies the final face UV crop at render time. No
  generated or stylized content was added.
- `power_plugs_rear.png`: generated with the built-in image generation tool for
  this project. Two seated power connectors viewed from their cable-exit rear,
  with genuine alpha transparency. Final prompt: `power_plugs_rear_prompt.md`.

## Optical hardware model references

- `optics.json` uses generic 1G/10G module profiles. Reach, wavelengths and
  Catalyst host family references are from Cisco's
  [Gigabit Ethernet SFP data sheet](https://www.cisco.com/c/en/us/products/collateral/interfaces-modules/gigabit-ethernet-gbic-sfp-modules/datasheet-c78-366584.html),
  [10G SFP+ data sheet](https://www.cisco.com/c/en/us/products/collateral/interfaces-modules/transceiver-modules/data_sheet_c78-455693.html),
  and [Catalyst 1000 data sheet](https://www.cisco.com/c/en/us/products/collateral/switches/catalyst-1000-series-switches/nb-06-cat1k-ser-switch-ds-cte-en.html).
- Module electrical consumption, optical budget values, connector losses,
  attenuation, and prices are representative simulation estimates rather than
  specifications for a particular Cisco transceiver SKU.
- `optical_connectors.png`: generated for this project with the built-in imagegen
  tool and alpha transparency. Includes empty cages, duplex/simplex/copper SFP
  faces, and seated LC/DAC/AOC ends. Final prompt: `optical_connectors_prompt.md`.
  No third-party product photograph was used in this atlas. The 10G switch
  profile reuses the existing Catalyst chassis image and port layout.

## Intel X520-DA2 PCIe adapter

- `server_parts.json` models the retail Intel X520-DA2 (E10G42BTDA), using
  Intel's [X520 product brief](https://www.intel.com/content/dam/doc/product-brief/ethernet-x520-server-adapters-brief.pdf)
  and its [PCI-SIG certification](https://pcisig.com/intel%C2%AE-ethernet-server-adapter-x520-da2-0).
  It has two SFP+ cages, an Intel 82599 controller, optical 1/10GbE modes,
  10GbE direct attach, and a PCIe 2.0 x8 interface.
- The simulator uses an estimated 6 W board load and adds installed module
  loads separately. Intel publishes assembled-adapter typical/maximum power
  by media; those totals must not be added again to the module load. Price
  remains a game economy value. Vendor EEPROM qualification, drivers and
  controller offloads are outside the current hardware simulation.
- Existing `dual_sfpplus_10g` inventory and installed cards migrate to
  `intel_x520_da2` when loading; saved interfaces, modules and cables retain
  their identities. The existing code-drawn PCIe face remains in use.

## Connector shop artwork

- `../cables/optical_shop_connectors.png`: generated for this project with the
  built-in imagegen tool and alpha transparency. Four complete loose connector
  sprites (duplex LC, simplex LC, DAC and AOC) use full padded cells. Shop icons
  do not crop plugs from coiled cables or show seated module faces.
  Final prompt: `../cables/optical_shop_connectors_prompt.md`.

## Operating power estimates

- Per-model `power` blocks in `server_config.json`, `server_parts.json`,
  `router_config.json`, `drives.json`, `ups_config.json`, `pdu_config.json` and
  `optics.json` contain project-authored operating assumptions for boards, memory,
  drives, NICs, cooling, conversion and legacy devices. These are simulator
  parameters, not measured vendor power figures. `switches.json` retains the
  separately cited Cisco idle/full-traffic endpoints.
- CPU sizing endpoint: [Intel Xeon E-2434, 55 W TDP](https://www.intel.com/content/www/us/en/products/sku/236192/intel-xeon-e2434-processor-12m-cache-3-40-ghz/specifications.html).
- [Dell R360 PSU capacity](https://www.dell.com/support/manuals/en-au/poweredge-r360/r360_ism/psu-specifications?guid=guid-ff8a8542-c09e-4bb9-9b35-710b1366b973&lang=en-us)
  and [Cisco C1111 66 W adapter capacity](https://www.cisco.com/c/en/us/products/collateral/routers/1000-series-integrated-services-routers-isr/datasheet-c78-739512.html)
  remain output limits; they do not become constant draw.
- [Calculation details and limitations](../../docs/POWER_CONSUMPTION.md).
