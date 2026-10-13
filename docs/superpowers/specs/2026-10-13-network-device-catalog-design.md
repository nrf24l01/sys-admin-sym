# JSON network device catalog

Add the requested ten Cisco routers and ten Cisco switches as distinct playable
models, retaining the existing C1000-24T-4X-L. The user requires all model
properties to be JSON configured and explicitly requested immediate execution.

`assets/equipment/network_devices.json` is authoritative for stable IDs,
category, family, EN/RU names/descriptions, prices, rack height, operating power,
optional adapter ratings, power inlets, front/rear artwork paths/UVs, per-port
names/console names, roles, speed ceilings, cages/modes/FEC and socket rectangles.
Models load once from installed assets with a bundled fallback. Validate IDs,
prices, geometry, port names, cages, roles and power before offering purchases.

Use string-backed saved model identities, one generic purchase command and
catalog iteration for shop/localization/artwork. Keep legacy purchase APIs and
missing-save defaults for the C1111 and C1000; preserve ports, cables, VLANs and
console descriptions on load. Reject unknown saved models clearly.

Built-in data interfaces only; exclude management/console and uninstalled
expansion modules. Combo copper/SFP sockets share one logical interface and
must not both forward simultaneously. C9300 uses the base chassis without an
optional uplink module. Large models require 40/100G QSFP/QSFP28 support through
the existing optics evaluator and matching catalog modules/cables. Existing
simulation command dialect remains a simulator, including Nexus.

Artwork must match catalog socket geometry. New schematic front/rear assets
may be rendered deterministically from the catalog; identify them as schematic
illustrations, and retain current photos for existing models. Hardware references
are official Cisco data sheets; prices and operating power are game estimates.
Document the schema, extension workflow, sources and assumptions. Verify every
model purchase, identity persistence, geometry, rack height, power, routing and
optical links; prove an installed JSON-only new model works without recompilation.
Run the repository's required `make check` and fix failures before completion.
