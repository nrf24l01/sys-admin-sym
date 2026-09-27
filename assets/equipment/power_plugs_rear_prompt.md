# Rear-facing power plugs

Asset: `power_plugs_rear.png`, 1774 × 887 RGBA PNG.
Generated with the built-in image generation tool. The final artwork was
visually inspected and its transparent alpha channel verified. IEC is at left;
Cisco four-pin DC is at right. Render crops and cable-exit positions are defined
in `CableConnector` in `crates/game/src/ui/cables.rs`.

## Final generation prompt

Generate a PNG sprite atlas with TRANSPARENT BACKGROUND, real alpha channel. Use case product-mockup, game equipment sprites. Canvas wide 1536x768, TWO equal square cells. Entire background fully transparent (alpha=0); NO checkerboard painted anywhere. Two black compact molded cable connector housings, each seen straight from the BACK END, orthographic camera aligned along insertion axis, as when looking at a seated plug's cable-exit rear. IEC power plug rear housing in left cell, Cisco four-pin DC power plug rear housing in right cell. Rear housing only: broad rounded rectangle on left, smaller keyed square with top latch on right. Each has concentric SHORT FORESHORTENED black strain-relief rings surrounding a FLAT ROUND CIRCULAR cable cross-section at the EXACT CENTER of the object. There is NO long tube or visible length of cable. View is parallel to cable insertion axis; no top-down, side, or angled view. Both black back housings look like the rear of a seated RJ45 plug. Matte black plastic grain and subtle product-photo highlights, compact end-on silhouettes. Housings centered in cells and cover 70 percent of each cell width. Nothing below housings. NO exposed metal contacts, blades, pins or mating sockets. NO sockets, screws, text, labels, logos, watermark. NO checkerboard pattern, no white/gray/black solid background. Preserve actual alpha transparency outside objects. Output RGBA PNG, not RGB.
