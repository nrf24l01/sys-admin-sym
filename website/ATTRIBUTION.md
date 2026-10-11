# Website asset attribution

The rack illustration is composed in HTML from existing game equipment textures. It is labeled as equipment from the game, rather than a gameplay screenshot. Face cropping follows the game's `equipment_uv` mapping in `crates/game/src/ui/mod.rs`. Source raster files are copied without modification.

| Website file | Repository source |
| --- | --- |
| `public/images/server-front.png` | `assets/equipment/server_front.png` |
| `public/images/server-rear.png` | `assets/equipment/server_rear_clean.png` |
| `public/images/switch-front.png` | `assets/equipment/switch_front_clean.png` |
| `public/images/switch-rear.png` | `assets/equipment/switch_rear.png` |
| `public/images/router-front.png` | `assets/equipment/router_rear_clean.png` (the interactive connector-bearing face) |
| `public/images/router-rear.png` | `assets/equipment/router_front_clean.png` (the opposite physical bezel face) |
| `public/images/ups.png` | `assets/equipment/ups_faces.png` |
| `public/images/cabling.png` | `docs/screenshots/cable-management-after-en.png` |
| `public/images/server-inspector.png` | `docs/screenshots/dell-server-after-en.png` |
| `public/images/equipment-shop.png` | `docs/images/shop-components.png` |

The equipment's original sources and processing history are recorded in the game's [equipment attribution](../assets/equipment/ATTRIBUTION.md), also copied into `public/images/SOURCE-ATTRIBUTION.md`. Manufacturer photographs retain their original rights and trademarks; copied attribution does not grant additional licensing rights. UPS artwork is an existing generated game illustration.

Oswald, IBM Plex Sans, and IBM Plex Mono are self-hosted through Fontsource and use the SIL Open Font License. License copies ship in `public/font-licenses/`. Icons come from Lucide (ISC license); control primitives are generated from shadcn-vue (MIT license). The small rack favicon and wordmark symbol are original SVG artwork created for this website.

Documentation in `content/en` is copied from the game repository's README, architecture, guides, equipment attribution, and original artwork prompts. `content/ru` contains Russian translations, with original generation prompts retained verbatim for reproducibility. `public/docs-assets/images` and `public/docs-assets/screenshots` preserve the corresponding source image files without modification. The bilingual Equipment sources article displays the full attribution on site.
