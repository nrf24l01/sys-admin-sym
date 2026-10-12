# Clean server face artwork

The built-in image generation tool edited manufacturer front/rear references
into transparent paired atlases under `servers/*_faces_clean.png`. Runtime UVs
select just the straight chassis faces and exclude surrounding padding.
The original reference photographs/diagrams and their source links are retained
in `ATTRIBUTION.md`. No raster manipulation scripts are used.

## Shared edit prompt

Edit both supplied manufacturer pictures into one transparent game sprite atlas,
front chassis alone in the upper half and rear chassis alone in the lower half.
Use strictly straight-on orthographic elevations, equal chassis widths, no
visible top/side surfaces, angled perspective, background or external shadows.
Preserve the model-specific enclosure, rack ears, vents, I/O and included PSUs.
Remove all diagram arrows, callout lines, numbered circles, floating labels and
captions, including those crossing the chassis. Keep authentic device markings.
Front drive apertures must be empty dark cavities with subtle internal rails:
no drive carriers, handles, disks or blanking trays. No installed PCIe NICs or
optional rear disks. Leave a clear transparent gap between both chassis faces.
Render clean realistic metal and black plastic with crisp boundaries.

## Model constraints

- [Dell R760](servers/dell_r760_faces_clean.png): 24 vertical front bays; eliminate the angled front photograph and
  extra surfaces from the original collage; keep the eight-slot rear layout.
- [HPE DL380 Gen10](servers/hpe_dl380_gen10_faces_clean.png): 16 vertical front bays at right; retain left optical/control
  region, replace optional left drives with a vent and optional rear drives
  with PCIe blank covers; keep lower rear I/O and right-hand PSUs.
- [HPE DL380 Gen11](servers/hpe_dl380_gen11_faces_clean.png): eight vertical front bays at right, large middle vent;
  replace optional rear drives above PSUs with PCIe blank covers.
- [HPE DL360 Gen11](servers/hpe_dl360_gen11_faces_clean.png): thin 1U faces; six horizontal front bays in three pairs at
  left, two additional bays below optical/control area at right; right-hand
  PSUs and blank optional OCP opening at rear.
- [Lenovo SR650 V2](servers/lenovo_sr650_v2_faces_clean.png): use the 24-SFF front view, ignore the alternate 12-LFF
  diagram; three/three/two PCIe covers at rear, PSUs at lower right.
- [Lenovo SR650 V3](servers/lenovo_sr650_v3_faces_clean.png): 24-SFF front, V3 operator panel; three/three/two PCIe covers
  at rear, PSUs at lower right.
- [Cisco C240 M6](servers/cisco_c240_m6_faces_clean.png): silver 24-SFF front; eight-slot I/O-centric rear; preserve
  the integrated Ethernet ports and right-hand PSUs; remove PCIe diagram text.
- [Supermicro SYS-221H-TNR](servers/supermicro_sys_221h_tnr_faces_clean.png): eight front bays at center with broad ventilation
  blocks on both sides; preserve lower-left PSUs, upper-left/middle and
  right-hand PCIe areas; remove colored numbered circles and yellow lines.

Exact bay occupancy and slot counts are rendered from `servers.json` and device
inventory. They do not depend on whether a reference photograph shows drives.

A targeted Supermicro edit corrected the first generated front's seven openings:
replace only the central bank with exactly eight equally sized empty apertures,
matching original source positions 0–7, without printed callout numbers. Preserve
the entire rear face, vents, frame, row placement and transparent canvas.

A localized HPE Gen10 edit removed an incorrect generated model-name badge from
the lower right front ear while retaining the green manufacturer logo. The
catalog/inspector carries the verified model name; enclosure geometry stays fixed.
