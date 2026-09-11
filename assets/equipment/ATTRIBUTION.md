# Equipment reference image attribution

These manufacturer images are used as visual references in the rack UI. Product
names and trademarks belong to their respective owners. The source-code license
does not grant rights to these images; review the manufacturer terms before
redistributing a packaged build.

The game loads `server_rear_clean.png`, `switch_front_clean.png`, and
`router_rear_clean.png`. They are OpenAI image-tool cleanup derivatives of the
manufacturer reference figures below: manual callouts and margins were removed,
then each panel was normalized to a 2200 x 200 rack texture. They are not original
manufacturer product images.

For `switch_front_clean.png`, both 6 x 2 RJ45 banks are composited directly from
the official Cisco figure rather than generated, preserving the exact 24-port
geometry used by the rack hitboxes.

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
