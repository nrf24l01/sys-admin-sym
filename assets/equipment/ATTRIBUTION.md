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
- Front crop and RJ45/SFP+ socket rectangles: `switch_4x_config.json`, measured relative to the photographed face.
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

## Server compatibility specifications

`server_parts.json` memory and CPU limits follow the manufacturer references in
[Server hardware](../../docs/SERVER_HARDWARE.md). The added 32 GB DDR5 ECC UDIMM
uses the existing generic RAM image; its power curve is a project-authored
estimate. The R360's represented CPU support list includes cataloged processors,
not every processor sold by Dell. Router port roles follow the Cisco guide linked
in [IOS guide](../../docs/IOS_GUIDE.md).

## R360 networking and redundant PSU behavior

The R360 onboard NIC count, dual PSU support and hot-spare distinction follow
[Dell's NIC specifications](https://www.dell.com/support/manuals/en-bm/poweredge-r360/r360_ism/nic-port-specifications?guid=guid-7be6485b-7c21-4030-9f52-d3a10af9cc8e),
[PSU specifications](https://www.dell.com/support/manuals/en-ca/poweredge-r360/r360_ism/psu-specifications?guid=guid-ff8a8542-c09e-4bb9-9b35-710b1366b973&lang=en-us)
and [hot-spare documentation](https://www.dell.com/support/manuals/en-us/poweredge-r360/r360_ism/hot-spare-feature?guid=guid-0bac5ab4-79f5-4a6b-a473-83e87d82b321).
The game uses balanced load sharing with project-authored efficiency estimates,
not Dell hot-spare mode or measured server power traces. No new image assets.

## Additional server models and components

`servers.json` profiles use manufacturer-authored reference documents. Each
entry represents the SATA/SFF configuration described in the server hardware
guide; optional media bays, management sockets and rear drive cages pictured
in source artwork are not additional simulated inventory. CPU and DIMM limits
use the supported catalog components rather than every vendor order code.
Operating curves and prices remain project-authored game estimates.

- Dell R740: [technical guide](https://i.dell.com/sites/csdocuments/shared-content_data-sheets_documents/en/aa/poweredge_r740_r740xd_technical_guide.pdf), pages 12–13 images and processor/memory/riser tables. `servers/dell_r740_{front,rear}.png` contains original embedded image objects from this guide.
- Dell R750: [technical guide](https://i.dell.com/sites/csdocuments/product_docs/en/poweredge-r750-technical-guide.pdf), 16-SFF front and eight-slot rear references. `servers/dell_r750_{front,rear}.png` contains its original embedded image objects.
- Dell R760: [specifications](https://i.dell.com/sites/csdocuments/Product_Docs/en/poweredge-r760-spec-sheet.pdf) and installation guide for CPU/memory/riser options. `servers/dell_r760_{front,rear}.jpg` preserves the original [Nam Viet product gallery collage](https://maychunamviet.vn/upload/img/products/Dell_R760_2.5inch_2.jpg); UI UVs select its front or rear face. Source listing: https://maychunamviet.vn/dell-emc-poweredge-r760. Dell and the source photographer retain their rights; no explicit reuse license was stated.
- HPE DL380 Gen10: [QuickSpecs a00008180enw](https://www.hpe.com/ca/en/collaterals/collateral.a00008180enw.html). Original image objects for `servers/hpe_dl380_gen10_{front,rear}.png` were obtained from this [unaltered document mirror](https://texnostar.uz/upload/iblock/b87/g0owmm3utgz3xunqw1h2hc6io0frftkl.pdf), overview pages 1–4.
- HPE DL380 Gen11: [QuickSpecs a50004307enw](https://www.hpe.com/asia_pac/en/collaterals/collateral.a50004307enw.html). Original image objects for `servers/hpe_dl380_gen11_{front,rear}.png` were obtained from the [HPE syndicated document copy](https://cdn.cs.1worldsync.com/syndication/feeds/hp-ent/inline-content/SY/B/E/BE75F69E37395A1C5D3655D68ECB6754FF5FD929_source.pdf), overview pages 1–4.
- HPE DL360 Gen11: [QuickSpecs a50004306enw](https://www.hpe.com/us/en/collaterals/collateral.a50004306enw.html). Original image objects for `servers/hpe_dl360_gen11_{front,rear}.png` were obtained from the [Ingram Micro document copy](https://pimcontent1.ingrammicro.com/mx/assets/media/brochure/0b12764_hpe-proliant-dl360-gen11.pdf), overview pages 1–4. The represented chassis includes the closed-loop cooling option needed for higher-TDP CPUs.
- Lenovo SR650 V2: [product guide LP1392](https://lenovopress.lenovo.com/lp1392-thinksystem-sr650-v2-server). `servers/lenovo_sr650_v2_front.png` and `servers/lenovo_sr650_v2_rear.png` are the original front/rear callout images linked by that guide.
- Lenovo SR650 V3: [product guide LP1601](https://lenovopress.lenovo.com/lp1601-thinksystem-sr650-v3-server). `servers/lenovo_sr650_v3_front.png` is its original front callout image; the guide itself links the SR650 V2 eight-slot rear image used as `servers/lenovo_sr650_v3_rear.png` for this shared enclosure configuration.
- Cisco C240 M6: [data sheet](https://www.cisco.com/c/en/us/products/collateral/servers-unified-computing/ucs-c-series-rack-servers/ucs-c240-m6-rack-server-ds.html) and [SFF specification sheet](https://www.cisco.com/c/dam/en/us/products/collateral/servers-unified-computing/ucs-c-series-rack-servers/c240m6-sff-specsheet.pdf). `servers/cisco_c240_m6_{front,rear}.png` contains original image objects from the 24-SFF and eight-slot I/O-centric chassis figures.
- Supermicro SYS-221H-TNR: [product specifications and callout figures](https://www.supermicro.com/en/products/system/hyper/2u/sys-221h-tnr?mlg=0). `servers/supermicro_sys_221h_tnr_{front,rear}.jpg` preserves its original [front](https://www.supermicro.com/files_SYS/images/System/SYS-221H-TNR_callout_front.JPG) and [rear](https://www.supermicro.com/files_SYS/images/System/SYS-221H-TNR_callout_rear.JPG) images. The eight-x8 riser configuration assigns slots 1–4 to CPU 2 and slots 5–8 to CPU 1; the profile includes the documented optional 1600W C14 PSU.

Clean server face atlases (`servers/*_faces_clean.png`) were edited with the
built-in image generation tool using the linked manufacturer references above.
They replace the annotated HPE, Lenovo, Cisco and Supermicro inputs and the
angled R760 collage in the runtime catalog. The original reference files remain
available in the source assets. Atlases contain isolated orthographic front/rear
faces, without callout lines or labels, populated disks, optional rear disks or
NICs. Dell R740/R750 retain their clean original photographs. Opaque empty-bay
and installed-carrier overlays derive from device inventory and shop recipes.
Source owners retain rights to reference designs and trademarks. The edit prompt
set is recorded in [SERVER_ARTWORK_PROMPTS.md](SERVER_ARTWORK_PROMPTS.md).

Processor base clocks, core/thread counts, TDP, socket, PCIe and memory limits
follow Intel ARK and the manufacturer processor support tables above, including
[Intel's second-generation brief](https://www.intel.com/content/www/us/en/products/docs/processors/xeon/2nd-gen-xeon-scalable-processors-brief.html),
[Silver 4310 specifications](https://www.intel.com/content/www/us/en/products/sku/215277/intel-xeon-silver-4310-processor-18m-cache-2-10-ghz/specifications.html),
[Platinum 8280L specifications](https://www.intel.com/content/www/us/en/products/sku/192472/intel-xeon-platinum-8280l-processor-38-5m-cache-2-70-ghz/specifications.html),
and [Lenovo's processor reference](https://lenovopress.lenovo.com/lp1262-intel-xeon-scalable-processor-comparison).

DDR4/DDR5 capacities, classes, ranks, voltage and operating rates follow the
chassis memory tables above. In particular, the R740's 16GB 2666/2933 RDIMMs are
2R; its 128GB LRDIMM is 2666 8R, while HPE's 128GB 2933 LRDIMM is 4R. R740
2933 RDIMMs drop to 2666 with 2DPC. Later DDR5 CPU generations have separate
2DPC limits; Supermicro's board additionally caps 2DPC at 4400. SR650 V3 uses
standard RDIMM rates of 5600/4400 and 3DS RDIMM rates of 5200/4400; the generic
parts do not claim Lenovo Performance+ capability. Component shop
thumbnails reuse the existing generated CPU/RAM cells by component kind.

## Cisco network device product artwork

The network catalog uses downloaded Cisco documentation artwork and clean
product catalog images cropped to the complete device panel. Each model's JSON
artwork paths select its image. The same panel appears on both rack sides where
separate opposite-side artwork is unavailable. Images remain copyrighted by
Cisco and/or their original creators; the project source license does not grant
reuse rights. Existing C1111 and C1000 images retain their attribution.

| Model | Asset | Artwork source |
| --- | --- | --- |
| Cisco C1101-4P | `network/c1101_4p_photo.png` | [Cisco ISR 1100 datasheet, Figure 13](https://www.cisco.com/c/en/us/products/collateral/routers/1000-series-integrated-services-routers-isr/datasheet-c78-739512.html) |
| Cisco ISR 4321 | `network/isr_4321_photo.png` | [Cisco branch routing portfolio, page 59](https://community.cisco.com/kxiwq67737/attachments/kxiwq67737/5951-discussions-small-business-routers/38064/1/Overview%20of%20Cisco%27s%20Branch%20Routing%20Portfolio.pdf) |
| Cisco ISR 4331 | `network/isr_4331_photo.png` | [Comel-IT](https://sklep.comel-it.com/pl/products/isr4331-k9-1757.html) |
| Cisco ISR 4351 | `network/isr_4351_photo.png` | [eBay listing](https://www.ebay.co.uk/itm/115807835869) |
| Cisco C8200L-1N-4T | `network/c8200l_1n_4t_photo.png` | [NetworkTigers](https://www.networktigers.com/products/c8200l-1n-4t-cisco-edge-platform) |
| Cisco C8200-1N-4T | `network/c8200_1n_4t_photo.png` | [Cisco Catalyst 8200 datasheet, Figure 1](https://www.cisco.com/c/en/us/products/collateral/routers/catalyst-8200-series-edge-platforms/nb-06-cat8200-series-edge-plat-ds-cte-en.html) |
| Cisco C8300-1N1S-6T | `network/c8300_1n1s_6t_photo.png` | [Cisco Catalyst 8300 datasheet, Figure 4](https://www.cisco.com/c/en/us/products/collateral/routers/catalyst-8300-series-edge-platforms/datasheet-c78-744088.html) |
| Cisco ASR 1001-X | `network/asr_1001_x_photo.png` | [Comel-IT](https://sklep.comel-it.com/en/products/asr1001-x-5384.html) |
| Cisco C8500-12X4QC | `network/c8500_12x4qc_photo.png` | [Cloud Appliances](https://www.cloudappliances.co.uk/cisco-routers/catalyst-8500/cisco-c8500-12x4qc/) |
| Cisco CBS250-24T-4G | `network/cbs250_24t_4g_photo.png` | [LDLC](https://www.ldlc.com/en/product/PB00394367.html) |
| Cisco CBS350-24T-4G | `network/cbs350_24t_4g_photo.png` | [Senetic](https://www.senetic.de/product/CBS350-24T-4G-EU) |
| Cisco C1300-24T-4G | `network/c1300_24t_4g_photo.png` | [Network Devices Inc.](https://networkdevicesinc.com/products/c1300-24t-4g) |
| Cisco WS-C2960X-24TS-L | `network/ws_c2960x_24ts_l_photo.png`, `network/ws_c2960x_24ts_l_rear_photo.png` | [Dedicated Networks](https://dedicatednetworksinc.com/product/new-open-box-cisco-ws-c2960x-24ts-l-switch/) |
| Cisco C9200L-24T-4G-E | `network/c9200l_24t_4g_e_photo.png` | [DS3 Comunicaciones](https://www.ds3comunicaciones.com/cisco/catalyst/C9200L-24T-4G-E.html) |
| Cisco C9300-24T-E | `network/c9300_24t_e_photo.png` | [Viettuans C9300-24T catalog artwork](https://viettuans.vn/uploads/2023/11/cisco-c9300-24t-a.jpg) (same C9300-24T chassis; license tier differs) |
| Cisco C9500-24Y4C-E | `network/c9500_24y4c_e_photo.png` | [Comel-IT](https://sklep.comel-it.com/en/products/c9500-24y4c-e-24x-1-10-25g-sfp28-uplink-4x-40g-100g-qsfp28-2tbps-network-essentials-l3-macsec-128-cisco-catalyst-9500-switch-13204.html) |
| Cisco N9K-C93180YC-FX3 | `network/n9k_c93180yc_fx3_photo.png` | [eBay listing](https://www.ebay.de/itm/225533367708) |
| Cisco N9K-C9364C-GX | `network/n9k_c9364c_gx_photo.png` | [Cisco Nexus 9300-GX datasheet, Figure 3](https://www.cisco.com/c/en/us/products/collateral/switches/nexus-9000-series-switches/nexus-9300-gx-series-switches-ds.html) |

The artwork above shows complete chassis panels, with perspective
rectified where needed. Panel edges fill the artwork crop, following the
existing C1000-24T-4G-L reference, with no studio background, case lids or
sidewalls in the displayed face. Front socket rectangles in
`network_devices.json` are measured against these normalized panels. Reference
artwork can show fitted optional modules; their extra ports are not
playable unless listed in the model JSON.
New 40/100G optics reuse the existing duplex optical sprite family.

The six documentation-style replacements are C1101-4P, ISR 4321,
C8200-1N-4T, C8300-1N1S-6T, C9300-24T-E and N9K-C9364C-GX. Five use
Cisco documentation pixels; the C9300 uses a sharper, straight-on product
catalog image instead of the low-resolution datasheet family image. No device
panels were generated or redrawn as schematics.
