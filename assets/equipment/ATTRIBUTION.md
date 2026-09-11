# Equipment reference image attribution

These manufacturer images are used as visual references in the rack UI. Product
names and trademarks belong to their respective owners. The source-code license
does not grant rights to these images; review the manufacturer terms before
redistributing a packaged build.

The game loads `server_rear_clean.png`, `switch_front_clean.png`, and
`router_rear_clean.png`. The server remains a cleanup derivative of the Dell
reference below. The Cisco textures are AI-upscaled derivatives of the two
user-provided product photographs, with the front panels cropped at render time.
Small labels and details may differ from the source photos. Socket hitboxes are
aligned to the displayed derivatives.

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
