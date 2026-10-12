# Server model catalog

## Intent

Add all ten requested servers as distinct playable hardware models. Make adding
another server a data and artwork task, without adding a Rust enum variant,
purchase command, shop branch, or rendering branch for each model.

Keep the Dell PowerEdge R360 playable and preserve existing saves. Existing
uncommitted terminal editor work is outside this change and must be preserved.

## Approach

Use a shared component catalog and one validated server model registry. This
fits the existing deterministic simulation and separates installed parts from
the chassis that constrains them.

Alternatives considered:

- A Rust variant and separate implementation for every server would duplicate
  purchase, validation, power, and presentation logic, making additions costly.
- Model names attached to the R360 profile would not provide the distinct
  hardware requested.

## Catalog and identity

`assets/equipment/servers.json` contains a `models` array. Each model has a stable
string ID, manufacturer, EN/RU name and description, chassis price, rack units,
CPU and DIMM limits/topology, PCIe slots, drive bays, onboard data ports,
power inlet rectangles, PSU capacity/efficiency, board/fan operating curves,
front/rear artwork paths and face UVs, and a full-pack component recipe.

The existing component entries in `server_parts.json` remain shared. Chassis
definitions move into the model registry. Add cataloged Xeon Scalable CPUs and
DDR4/DDR5 ECC RDIMMs needed to assemble the new models. Components reference
stable IDs; compatibility is determined by the selected chassis and installed
CPU, using the existing CPU, memory and PCIe rules.

The registry loads once from installed assets, with a bundled fallback matching
the repository's equipment configuration convention. Models are iterated by
the shop and texture loader. No model-specific registration in Rust is needed.

## Models

| Stable ID | Manufacturer | Model |
| --- | --- | --- |
| `dell_r360` | Dell | PowerEdge R360 (existing) |
| `dell_r740` | Dell | PowerEdge R740 |
| `dell_r750` | Dell | PowerEdge R750 |
| `dell_r760` | Dell | PowerEdge R760 |
| `hpe_dl380_gen10` | HPE | ProLiant DL380 Gen10 |
| `hpe_dl380_gen11` | HPE | ProLiant DL380 Gen11 |
| `hpe_dl360_gen11` | HPE | ProLiant DL360 Gen11 |
| `lenovo_sr650_v2` | Lenovo | ThinkSystem SR650 V2 |
| `lenovo_sr650_v3` | Lenovo | ThinkSystem SR650 V3 |
| `cisco_c240_m6` | Cisco | UCS C240 M6 |
| `supermicro_sys_221h_tnr` | Supermicro | SuperServer SYS-221H-TNR |

Each model represents one documented chassis/backplane/riser configuration.
Descriptions identify that configuration; optional configurations from the same
product family are not combined into an impossible chassis. Supported CPU lists
contain the compatible CPUs available in the game's component catalog.

Manufacturer references establish rack height, CPU platform, DIMM topology,
PCIe layout, drive bay layout, networking and PSU ratings. Prices and operating
power curves are game estimates, documented separately from hardware ratings.

## Simulation integration

A saved server stores its model ID. `DeviceTemplate::Server` continues to identify
the server category; model lookup supplies its specific properties.

One generic model purchase path handles bare chassis and full packs. Legacy
R360 purchase commands delegate to that path. Full-pack prices derive from the
chassis and recipe, and purchases stay atomic. Recipes support component
quantities and explicit installation slots for dual-CPU/multi-DIMM systems.

Server hardware validation resolves the owning model rather than a global
chassis. DIMMs and PCIe slots assigned to CPU 2 require CPU 2. Resource reporting,
boot readiness, installed drive compatibility, PSU sizing, consumption and
redundant power wiring all resolve the same model.

Rack placement, overlap checks and rendering use the actual device rack height.
Onboard ports are allocated from model data with their specified speed and
position. Cable endpoints, click targets and painted sockets share model
geometry. Management-only ports shown in reference artwork are not exposed as
ordinary data interfaces.

## Presentation

The shop automatically generates chassis and full-pack offers for every model,
including prices, manufacturer search text, technical filters, comparison,
inventory counts and compatibility previews. Components use shared artwork by
component kind instead of individual component IDs.

Rack views and shop details use distinct front/rear artwork for each new model,
with data-defined UVs and socket/bay/slot overlays. Preserve the existing
asynchronous texture preparation and thumbnail caching. Store image provenance
in `assets/equipment/ATTRIBUTION.md`.

The hardware inspector displays the selected model's CPU sockets, DIMMs, PCIe
slots, bays and PSU ratings. English and Russian item descriptions come from
the registry. Installation actions keep using concrete device and component IDs.

## Compatibility and validation

Missing model IDs in old saves default to `dell_r360`. Preserve saved component,
port, cable and device identities and existing legacy assembled server behavior.
Apply slot normalization against the resolved model. An explicitly unknown
saved model ID produces a clear load error rather than silently using R360.

Validate unique IDs, positive prices and rack/PSU values, supported component
references, DIMM/CPU topology, normalized geometry, and full-pack recipes.
Catalog errors identify the model and offending field. Reject incompatible
recipes before they can become purchasable offers.

## Extension workflow

Document a complete example in `docs/SERVER_HARDWARE.md`: duplicate a model entry,
assign a new ID, fill verified hardware data and EN/RU text, add its front/rear
images and attribution, select existing compatible components or add new shared
parts, and run `make check`. For properties already supported by the schema,
adding a model needs no Rust changes or new per-model tests.

Provide `tools/add_server_model.py` to duplicate a selected model entry with a new
ID, manufacturer, names and artwork paths. It refuses duplicate IDs and retains
the copied hardware data for the author to replace with verified specifications.
The guide shows both this command and the underlying JSON format.

## Approved component scope

The user approved the design and additionally requested all CPU and RAM parts
required by these platforms. Supply entry, midrange and high-capacity processors
across all five supported Xeon Scalable generations, plus the existing Xeon
E-2434. Supply the DDR4 ECC RDIMM/LRDIMM and DDR5 ECC RDIMM capacities and speeds
needed to assemble and upgrade the represented chassis, including modules for
their represented maximum RAM configurations. Vendor memory classes and ranks
must remain distinguishable, and RDIMM/LRDIMM mixing must be rejected.

CPU support lists are model-specific and based on manufacturer support tables,
including TDP limits. Published RAM/CPU capacity ceilings must be achievable with
the supplied components; a lower-capacity CPU must still enforce its own limit.

## Acceptance and verification

- All ten new models appear as distinct chassis and full-pack offers.
- Every full pack purchases atomically and passes its model's readiness rules.
- Representative DDR4/DDR5 and CPU socket incompatibilities are rejected.
- CPU 2 controls its associated DIMM and PCIe availability.
- 1U/2U rack placement, overlap, artwork and cable endpoints agree.
- Power draw, PSU limits and redundant feeds use the selected model.
- Model identity, components and cable connections survive save/load.
- Legacy saves resolve to R360 while preserving IDs and existing behavior.
- Catalog iteration drives offers and artwork without per-model code branches.
- Add focused regression coverage for changed simulation and geometry behavior.
- Run the repository's required `make check` and resolve failures.
- Capture before/after UI images showing model selection and rack appearance.

## Primary hardware references

- [Dell R740 specifications](https://www.dell.com/learn/vn/en/vnbsd1/shared-content~data-sheets~en/documents~poweredge-r740-spec-sheet.pdf)
- [Dell R750 technical guide](https://i.dell.com/sites/csdocuments/product_docs/en/poweredge-r750-technical-guide.pdf)
- [Dell R760 specifications](https://i.dell.com/sites/csdocuments/Product_Docs/en/poweredge-r760-spec-sheet.pdf)
- [HPE DL380 Gen10 QuickSpecs](https://www.hpe.com/ca/en/collaterals/collateral.a00008180enw.html)
- [HPE DL380 Gen11 QuickSpecs](https://www.hpe.com/asia_pac/en/collaterals/collateral.a50004307enw.html)
- [HPE DL360 Gen11 QuickSpecs](https://www.hpe.com/us/en/collaterals/collateral.a50004306enw.html)
- [Lenovo SR650 V2 product guide](https://lenovopress.lenovo.com/lp1392-thinksystem-sr650-v2-server)
- [Lenovo SR650 V3 product guide](https://lenovopress.lenovo.com/lp1601-thinksystem-sr650-v3-server)
- [Cisco C240 M6 data sheet](https://www.cisco.com/c/en/us/products/collateral/servers-unified-computing/ucs-c-series-rack-servers/ucs-c240-m6-rack-server-ds.html)
- [Supermicro SYS-221H-TNR product specifications](https://www.supermicro.com/en/products/system/hyper/2u/sys-221h-tnr?mlg=0)
