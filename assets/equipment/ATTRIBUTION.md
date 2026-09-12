# Equipment reference image attribution

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
