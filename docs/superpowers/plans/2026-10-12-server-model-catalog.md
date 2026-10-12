# Server Model Catalog Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans for native execution, or superpowers:subagent-driven-development if the user selects delegation. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add ten distinct configurable servers and their required CPU/RAM components, with a reusable data-driven addition process.

**Architecture:** A single `ServerCatalog` joins `servers.json` model definitions with the shared `server_parts.json` component catalog. Saved servers carry validated string model IDs. Simulation rules, purchases, power, geometry, artwork and shop offers resolve those definitions instead of the R360 singleton.

**Tech Stack:** Rust 2024, serde/serde_json, Bevy/egui, existing SQLite/RON persistence, Python tooling.

**Spec:** `docs/superpowers/specs/2026-10-12-server-model-catalog-design.md`

## Global Constraints

- Add all ten requested servers as distinct playable hardware models.
- Keep the Dell PowerEdge R360 playable and preserve existing saves.
- Existing uncommitted terminal editor work is outside this change and must be preserved.
- Adding a model needs no Rust changes or new per-model tests.
- Supported CPU lists contain the compatible CPUs available in the game's component catalog.
- Prices and operating power curves are game estimates, documented separately from hardware ratings.
- Run the repository's required `make check` and resolve failures.
- Keep simulation independent of Bevy; UI sends commands through the existing worker boundary.

## Review Focus

- Bare chassis with no onboard data NIC must remain inspectable and accept an installed NIC without creating fictional management interfaces.
- Auto installation must skip CPU-2 DIMM/PCIe slots when CPU 2 is absent and find a valid CPU-1 slot later in the layout.
- RDIMM/LRDIMM class, ranks and rated speeds must be checked together, including the maximum supported population.
- Unknown explicit saved model IDs must produce a load error; absent IDs must preserve R360 compatibility in both JSON and RON.
- Models added to installed JSON must receive shop offers and artwork without a rebuild or per-model Rust branch.

## File and interface map

Create `crates/sim/src/server_hardware/models.rs` for model metadata, IDs, lookup and validated catalog loading; `purchase.rs` for generic server creation and pack installation; `catalog_validation.rs` for catalog/recipe validation. Keep component definitions in `server_hardware.rs` and topology rules in `limits.rs`.

Create `crates/game/src/ui/server_models.rs` for model artwork handles and texture selection, and `server_hardware.rs` for the inspector currently in `ui/mod.rs`. Extend `ui/equipment.rs` to resolve model socket geometry before generic equipment config fallback. Preserve the existing async image preparation in `ui/textures`.

Keep one saved identity on `Server`: `model: ServerModelId`, defaulting to R360. `ServerModelId` wraps a private string, exposes `as_str()`, implements validating `Deserialize`, and is created with `ServerModelId::parse(&str) -> Result<Self, String>`. Catalog model entries use plain IDs so registry decoding cannot recursively initialize itself.

`ServerCatalog` owns `models: Vec<ServerModel>` and `parts: Vec<ServerPart>`; expose `model(&str) -> Option<&ServerModel>`, `default_model() -> &ServerModel`, and `part(&str) -> Option<&ServerPart>`. `Server::model(&self) -> &ServerModel` resolves its validated identity. Retain `server_catalog() -> &'static ServerCatalog` as the one startup cache.

`ServerModel` owns `id`, `manufacturer`, localized text, chassis price, `rack_units`, `chassis: ServerChassis`, `power: ServerPowerProfile`, `onboard_ports`, `power_inlets`, `artwork`, and `full_pack`. Normalized rectangles and face UVs are metadata, not Bevy types.

During the migration, keep `ServerCatalog::chassis` as a compatibility projection
of the default R360 model and retain existing hardware methods as explicitly
R360-default wrappers. Add model-aware methods under new names so each task
continues to compile before all consumers are migrated. Production server paths
must use model-aware methods by the end; the final audit removes any temporary
projection that no longer has a compatibility caller.

## Task 1: Shared components and model registry

**Files:** Create `assets/equipment/servers.json`, `crates/sim/src/server_hardware/{models,catalog_validation}.rs`; modify `server_hardware.rs`, `power/profiles.rs`, `assets/equipment/server_parts.json`; extend `crates/sim/tests/server_limits.rs` and create `server_models.rs`.

**Produces:** The catalog/identity/model interfaces above, `ServerCatalog::validate() -> Result<(), String>`, and `ServerModel::full_pack_price(&self, catalog: &ServerCatalog) -> Result<i64, String>`.

- [x] Add regression cases that reject duplicate model/part IDs, malformed topology/UVs, unsupported component references, and missing full-pack ingredients; require all eleven IDs from the spec to resolve. Full recipe topology/readiness validation is completed in Task 2.
- [x] Populate source-backed model records. Use one documented front SFF SATA/backplane/riser configuration per model; record its variant in EN/RU text and attribution. Set 1U for R360 and DL360 Gen11, 2U for the other models. Use two CPU sockets for each new model, 24 DIMMs for R740/DL380 Gen10 and 32 for the other new models.
- [x] Populate CPUs from the supported vendor tables and Intel specifications. Include 1st Gen Silver 4110, Gold 6130/6152; 2nd Gen Silver 4210R, Gold 6230R/6248R, Platinum 8280L; 3rd Gen Silver 4310, Gold 6330, Platinum 8352Y/8380; 4th Gen Silver 4410Y, Gold 5418Y, Platinum 8462Y+; 5th Gen Silver 4510, Gold 6548Y+, Platinum 8592+. Preserve E-2434. Store actual base clocks, cores/threads, socket, TDP, PCIe lanes/generation and per-CPU memory limits. Include a CPU in a chassis support list only when its manufacturer table permits it.
- [x] Populate DDR4 ECC RDIMMs at 2666/2933 MT/s in 8/16/32/64 GB sizes, DDR4-3200 ECC RDIMMs in 16/32/64/128/256 GB sizes, DDR4 ECC LRDIMMs at 2666/2933 MT/s in 64/128 GB sizes, and DDR5 ECC RDIMMs at 4800/5600 MT/s in 16/32/64/128/256 GB sizes. Verify rank/voltage and supported module density against primary module/platform references. Preserve the existing 16/32 GB DDR5 UDIMMs.
- [x] Move chassis data into `servers.json`; load the two catalog files once with installed overrides and embedded defaults. Validate all metadata and recipes before exposing offers; diagnostics include the model ID and field. Make power profiles cloneable for typed model storage.
- [x] Supply a compatible CPU, matching RAM, CPU-1 PCIe NIC and SATA SSD recipe for each new full pack; retain the exact R360 recipe. Derive prices from the constituent inventory items with checked arithmetic.
- [x] Verify through the Make Rust test target after the code compiles. Generic tests iterate models/parts rather than requiring a new test registration per model.

## Task 2: Model-specific topology and component compatibility

**Files:** Modify `server_hardware/{limits,models,catalog_validation}.rs`, `server_hardware.rs`, `shop_compatibility.rs`, `resources.rs`, `terminal.rs`, `hosting/inventory.rs`; extend `server_limits.rs`, `server_hardware.rs`, `resources.rs` integration tests.

**Consumes:** Task 1 catalog and model metadata.

**Produces:** `ServerHardware::validate_for_model(&self, model: &ServerModel, catalog: &ServerCatalog) -> Result<(), String>`, `memory_status_for_model(&self, model: &ServerModel) -> Result<ServerMemoryStatus, String>`, `memory_status_with_model_catalog(&self, model: &ServerModel, catalog: &ServerCatalog) -> Result<ServerMemoryStatus, String>`, `dimm_slot_indices_for_model(&self, model: &ServerModel) -> Vec<usize>`, `pcie_link_generation_for_model(&self, model: &ServerModel, index: usize) -> Option<u8>`. Keep model-independent compute scoring methods unchanged.

- [x] Cover wrong CPU socket/generation, DDR4/DDR5 mismatch, UDIMM/RDIMM mismatch, RDIMM/LRDIMM mixing, unmatched dual CPUs, unsupported rank/voltage, and CPU-specific capacity limits.
- [x] Replace one chassis memory type with supported types and type-specific module matrix entries. Preserve R360's current operating rates and mixing rules. Compute supported capacity from chassis, installed CPU, available slots and allowed module class.
- [x] Route all slot and limit decisions through the selected model. Auto DIMM and PCIe selection tries each compatible free slot and validates the candidate, so an unavailable CPU-2 slot cannot hide a later CPU-1 slot.
- [x] Test CPU-2 DIMM and PCIe rejection before CPU 2 is installed, acceptance afterward, and rejection of CPU removal while its dependent parts remain.
- [x] Confirm a maximum supported RAM population can be assembled from supplied catalog parts for every model, while one excess module is rejected without consuming inventory.
- [x] Complete catalog recipe validation by constructing each recipe's hardware and checking its topology, compatible drive/card slots, computed component price and PSU load. Reject invalid recipes before creating shop offers.
- [x] Update console `lscpu`, `free`, resource reporting, hosting inventory and compatibility previews to use actual installed parts and the owning model. Check bare servers with zero data ports.

## Task 3: Generic server purchases, saved identity and rack size

**Files:** Create `server_hardware/purchase.rs`; modify `device.rs`, `command.rs`, `shop.rs`, `world.rs`, `error.rs`, `game/src/localization/message.rs`, `game/src/persistence/sqlite.rs`; extend sim `server_models.rs`, `shop.rs`, `datacenter.rs` and persistence module tests.

**Consumes:** Tasks 1–2 registry and compatibility rules.

**Produces:** `ServerConfiguration::{Chassis, FullPack}`, `Command::BuyServer { model_id: String, configuration: ServerConfiguration }`, `PurchaseItem::ServerModel { model_id: String, configuration: ServerConfiguration }`, and `Device::rack_units(&self) -> u8`. Legacy server purchase variants delegate to R360.

- [x] Test every model/configuration purchase, correct names and onboard speeds, insufficient funds/unknown ID rollback, and atomic multi-unit purchases. Assert quoted full-pack totals equal committed costs.
- [x] Build devices and onboard ports from metadata, including zero ports on network-choice chassis. Configure onboard data NICs only; leave BMC/iLO/iDRAC/XCC connectors outside packet forwarding.
- [x] Store and validate model IDs. Test JSON and SQLite/RON round trips retaining model, device, component, port and cable IDs. Test absent IDs becoming R360 and explicit unknown IDs producing a readable decoding error.
- [x] Normalize saved DIMM, PCIe and drive arrays against the resolved model, preserving installed components and legacy refunds. Preserve existing nonconfigurable R360 server behavior.
- [x] Use model price when selling a chassis and model rack units in placement/overlap checks and GUI placement previews. Test 2U overlap, top-of-rack overflow and successful adjacent placements.
- [x] Keep legacy commands and the standalone console protocol working; update existing Rust constructors for the default identity without touching unrelated editor changes.

## Task 4: Power and shared physical geometry

**Files:** Modify `power/{components,consumption,profiles}.rs`, `device.rs`, `world.rs`, `game/src/ui/{equipment,power,cables}.rs` and affected geometry consumers; extend `server_power.rs`, `power_consumption.rs`, `cabling.rs` tests.

**Consumes:** Saved model identity and model profile from Tasks 1–3.

**Produces:** `ServerHardware::ready_for_model(&self, model: &ServerModel) -> bool`, `peak_load_watts_for_model(&self, model: &ServerModel) -> u32`; `ServerModel::port_position(index: usize) -> Option<(f32, f32)>` and `power_inlet_position(index: usize) -> Option<(f32, f32)>` expose model geometry through existing device APIs.

- [x] Test different chassis base draw/PSU capacities, component load changes, and failure when a surviving feed cannot supply the server. Retain R360 power regression expectations.
- [x] Replace R360 power lookups in boot readiness, PSU conversion, redundant inlet counts, connection validation, index rebuild and GUI status with the owning model.
- [x] Use shared onboard port/PSU rectangles and model PCIe/drive faces for simulation cable endpoints and UI click targets. Network port ordering follows model onboard ports followed by installed card port IDs.
- [x] Test a model with no onboard ports and a model with four onboard ports; removing/reinstalling a card must preserve endpoint correctness and material accounting. Test inlet independence with both feeds and either single feed.
- [x] Remove the standalone R360 power/config dependency from production model consumers; keep only an explicit compatibility wrapper if an existing external API requires it.

## Task 5: Automatic shop entries, distinct artwork and inspectors

**Files:** Create `game/src/ui/{server_models,server_hardware}.rs`; modify `ui/mod.rs`, `ui/equipment.rs`, `ui/shop/{artwork,catalog,details,product,layout}.rs`, `ui/shop/catalog/{hardware,components}.rs`, `localization/{items,labels}.rs` and locale JSON as needed; add assets under `assets/equipment/servers/`; extend shop/geometry module tests.

**Consumes:** Model catalog, generic purchase variants and shared geometry.

**Produces:** Model-keyed image handle/texture maps with cached thumbnail and full-resolution lookups; shop offer iteration for every catalog model.

- [x] Preserve the existing R360-only baseline at `docs/screenshots/server-limits-before-en.png`; use a private preview save for new captures. The user's local save remains untouched.
- [x] Obtain distinct front/rear originals or manufacturer diagrams for all new models. Register their paths/face UVs in model data and provenance in `ATTRIBUTION.md`; verify each interactive face matches the selected backplane/riser variant.
- [x] Load every model's artwork by catalog iteration. Keep async preparation/thumbnail reuse and bind only active images. Extend borrowed texture views as needed instead of copying model maps per product row.
- [x] Generate chassis/full-pack offers, search text, price, rack/PSU/CPU/memory/bay/port filters and comparisons from the model data. Resolve component thumbnails by component kind so new CPU/RAM IDs automatically receive artwork.
- [x] Move the hardware inspector into its focused module and display the owning model's slots, memory status, PCIe negotiation, drives and PSUs. Resolve front/rear rack artwork and overlays by the model ID.
- [x] Test eleven model families/twenty-two server offers, all new CPU/RAM offers, English/Russian names, model-specific filtering, inventory ownership and shared compatibility results. Test dynamic iteration with a synthetic added catalog record, rather than hardcoding future model names into production branches.
- [x] Check normalized model face/slot/socket geometry, and capture native rack front/rear views with 1U/2U models plus the model catalog and R760 details.

## Task 6: Easy additions, documentation and full verification

**Files:** Create `tools/add_server_model.py`, `tools/test_add_server_model.py`; update `docs/{SERVER_HARDWARE,SERVER_INTERFACES_AND_POWER,SHOP,POWER_CONSUMPTION}.md`, `ARCHITECTURE.md`, `AGENTS.md`, attribution, and the existing Python package checks if their asset expectations need extending.

**Consumes:** The final JSON schema and dynamic model consumers from Tasks 1–5.

**Produces:** `python3 tools/add_server_model.py --template dell_r760 --id example_2u --manufacturer Example --name 'Example 2U' --front equipment/servers/example_front.png --rear equipment/servers/example_rear.png`, with optional `--catalog` for a custom file and safe write-through-temporary-file behavior.

- [x] Test cloning preserves the template, updates identity/EN/RU names/artwork, refuses duplicate IDs and refuses an unknown template without changing the file.
- [x] Document the command and a complete model JSON example, shared part references, actual CPU/RAM support tables, represented variants and estimated price/power values. State that copied hardware specs must be checked before use.
- [x] Update repository extension guidance so future server work uses the registry, shared compatibility evaluator and common geometry. Preserve all existing optical instructions.
- [x] Extend the packaging test to check the model catalog and nested model artwork; add CPU/memory source links and before/after documentation screenshots.
- [x] Run `make fmt` and `make check`; formatting, Clippy, Rust/Python tests, workspace build and Linux console/window smoke all pass.
- [x] Review the final diff for residual global R360 chassis lookups in production consumers, unsupported/invented ports, per-model artwork branches, save compatibility regressions and accidental edits to the unrelated terminal editor work. Existing standalone R360-default compatibility helpers must be labeled and have no caller handling another model.
- [x] Report ten added models, 17 CPU and 27 RAM options, the JSON extension workflow, and final verification.
