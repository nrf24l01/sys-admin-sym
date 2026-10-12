# Configurable server hardware

The shop reads distinct chassis profiles from `assets/equipment/servers.json` and
shared CPU, RAM and PCIe parts from `assets/equipment/server_parts.json`. Installed JSON overrides embedded defaults
once at startup. Invalid topology or missing limits fail with the filename and
reason. Restart after editing equipment configuration.

## Available server platforms

Each entry represents one configured chassis, including its cooling, redundant
PSUs and optional risers. Data interfaces exclude dedicated BMC management.

| Model | Rack | CPU platform | DIMMs | Represented SATA bays | Data ports |
| --- | --- | --- | --- | --- | --- |
| Dell PowerEdge R360 | 1U | Xeon E-2400 / LGA1700 | 4 DDR5 ECC UDIMM | 4 | 2 × 1GbE |
| Dell PowerEdge R740 | 2U | Xeon Scalable Gen1/2 / LGA3647 | 24 DDR4 RDIMM/LRDIMM | 16 | 4 × 1GbE NDC |
| Dell PowerEdge R750 | 2U | Xeon Scalable Gen3 / LGA4189 | 32 DDR4 RDIMM | 16 | 2 × 1GbE LOM |
| Dell PowerEdge R760 | 2U | Xeon Scalable Gen4/5 / LGA4677 | 32 DDR5 RDIMM | 24 | 2 × 1GbE LOM |
| HPE ProLiant DL380 Gen10 | 2U | Xeon Scalable Gen1/2 / LGA3647 | 24 DDR4 RDIMM/LRDIMM | 16 | 4 × 1GbE embedded |
| HPE ProLiant DL380 Gen11 | 2U | Xeon Scalable Gen4/5 / LGA4677 | 32 DDR5 RDIMM | 8 | PCIe NIC required |
| HPE ProLiant DL360 Gen11 | 1U | Xeon Scalable Gen4/5 / LGA4677 | 32 DDR5 RDIMM | 8 | PCIe NIC required |
| Lenovo ThinkSystem SR650 V2 | 2U | Xeon Scalable Gen3 / LGA4189 | 32 DDR4 RDIMM | 24 | PCIe NIC required |
| Lenovo ThinkSystem SR650 V3 | 2U | Xeon Scalable Gen4/5 / LGA4677 | 32 DDR5 RDIMM | 24 | PCIe NIC required |
| Cisco UCS C240 M6 | 2U | Xeon Scalable Gen3 / LGA4189 | 32 DDR4 RDIMM | 24 | 2 × 10GbE embedded |
| Supermicro SuperServer SYS-221H-TNR | 2U | Xeon Scalable Gen4/5 / LGA4677 | 32 DDR5 RDIMM | 8 | PCIe NIC required |

Every new server supports two matching processors. The component shop includes
17 added Xeon Scalable CPU options across all five generations, plus 27 added
DDR4 RDIMM/LRDIMM and DDR5 RDIMM capacity/speed options. DDR4 reaches 256 GB per
3DS RDIMM or 128 GB per LRDIMM; DDR5 reaches 256 GB per 3DS RDIMM. R740/Gen10
represent up to 3 TB using compatible LRDIMMs and a suitable CPU; later platforms
represent up to 8 TB. The inspector's maximum reflects installed CPUs, their
owned slots and the selected DIMM class. Lower CPU capacities and clock rates
still apply.

Full packs contain a compatible CPU, matching DIMMs, a PCIe I350-T4 and a SATA
SSD. R360 keeps its 16 GB pack; new packs include 32 GB. Purchase applies recipes
in CPU, primary/secondary DIMM, then PCIe order and commits atomically. Maximum
memory builds need both CPUs, with their associated slots. Removing a CPU first
requires removing that CPU's DIMMs/cards; devices on the other socket stay intact.
Individual DIMMs can be installed while assembling a required pair. Unsupported
population counts prevent boot until the required population is complete.

## Adding another server

Run from the repository root:

```sh
python3 tools/add_server_model.py \
  --template dell_r760 --id example_2u --manufacturer Example \
  --name 'Example 2U server' \
  --front equipment/servers/example_2u_front.png \
  --rear equipment/servers/example_2u_rear.png
```

The command preserves the template and creates a new model entry. Replace its
copied CPU support list, DIMM topology/matrix, drive/PCIe layout, power, price,
EN/RU descriptions and full-pack ingredients with the new model's verified data.
Supply the two images and measure their face UVs and connector/slot rectangles;
coordinates are normalized to the cropped chassis face. A complete valid record
is provided in [the JSON example](examples/server-model.json).

Shared components belong in `server_parts.json` and drives in `drives.json`.
Recipe part entries specify `part_id` and physical `slot` for RAM/PCIe; repeating
an entry supplies multiple CPUs or DIMMs. Drive entries specify `drive_id` and
`bay`. Metadata validates IDs, topology, geometry, component references, recipes
and prices at startup. Add source attribution and run `make check`, then restart.
No Rust registration or new per-model renderer branch is needed.

Both installed JSON and bundled defaults use the same catalog. The legacy
`server_parts.json.chassis` projection remains for old standalone R360 API/data
compatibility; the authoritative model definitions and R360 power/translation
metadata are in `servers.json`.

## Chassis and component limits

The chassis specifies CPU sockets, supported CPU IDs, maximum CPU TDP, whether
CPUs must match, and whether components can be changed while powered on. Drive
bays specify their interfaces. PCIe slots specify their owning CPU, electrical
lanes, physical connector width and generation. Each model's `power` profile in
`servers.json` defines per-PSU capacity and conversion efficiency. Both
rear power inlets are connectable independently; see
[interfaces and redundant power](SERVER_INTERFACES_AND_POWER.md) for feed
failover, boot behavior, JSON settings and save migration.

The chassis `memory` object specifies total capacity, channels per CPU, DIMMs per
channel, named socket topology, preferred population order, allowed DIMM counts,
the mixing policy and a module support/speed matrix. Each CPU has its own
`memory` limits for supported types, capacity, channels, total DIMMs, DIMMs per channel and
speed, plus maximum socket count and PCIe limits. RAM parts specify capacity,
type, nominal transfer rate, ranks and voltage. Compatibility uses both chassis
and processor limits. No model-specific capacity or channel numbers live in Rust.

Example chassis memory settings:

```json
{
  "max_capacity_gb": 128,
  "channels_per_cpu": 2,
  "dimms_per_channel": 2,
  "identical_dimms": true,
  "supported_dimm_counts": [1, 2, 3, 4],
  "population_order": [0, 1, 2, 3],
  "slots": [
    { "name": "A1", "cpu_socket": 0, "channel": 0, "position": 0 },
    { "name": "A2", "cpu_socket": 0, "channel": 1, "position": 0 },
    { "name": "A3", "cpu_socket": 0, "channel": 0, "position": 1 },
    { "name": "A4", "cpu_socket": 0, "channel": 1, "position": 1 }
  ],
  "supported_modules": [
    {
      "capacity_gb": 16,
      "ranks": 1,
      "voltage_mv": 1100,
      "rated_speeds_mt_s": [4800, 5600],
      "operating_speeds_mt_s": [4400, 4000]
    },
    {
      "capacity_gb": 32,
      "ranks": 2,
      "voltage_mv": 1100,
      "rated_speeds_mt_s": [4800, 5600],
      "operating_speeds_mt_s": [4400, 3600]
    }
  ]
}
```

`operating_speeds_mt_s` is indexed by DIMMs per channel (first entry = 1 DPC).
The running speed is the lowest installed module, CPU and populated-channel
limit. A primary socket must be populated before the next socket in its channel.
SR650 V3 standard modules run up to 5600/4400 MT/s at 1/2 DPC; its 128/256 GB
3DS modules run up to 5200/4400. Generic modules do not represent Performance+
RDIMMs. The CPU's own limits can reduce these rates further.
Auto placement fills the preferred order; explicit installation/removal addresses
physical slot indices. Shop compatibility and installation use the same checks.

## Published configuration represented in the game

The R360 uses one CPU, four ECC UDIMM slots and up to 128 GB.
[Dell memory specifications](https://www.dell.com/support/manuals/en-us/poweredge-r360/r360_ism/memory-specifications?guid=guid-c2545624-b0c6-4d75-b6b0-2e504269f98e&lang=en-us).
Its two channels map A1/A3 and A2/A4. One DIMM per channel runs at 4400 MT/s;
two run at 4000 MT/s for 16 GB single-rank modules or 3600 MT/s for 32 GB dual-rank
modules. [Dell memory matrix](https://www.dell.com/support/manuals/en-us/poweredge-r360/r360_ism/system-memory-guidelines?guid=guid-b0770d5a-3a3e-490f-ad3f-46377742d34a&lang=en-us).
Matching module specifications and balanced channels follow
[Dell population guidance](https://www.dell.com/support/manuals/en-us/poweredge-r360/r360_ism/general-memory-module-installation-guidelines?guid=guid-29c7e354-ec34-4d1a-9452-64dbde9e0232&lang=en-us).

The Xeon E-2434 has four cores/eight threads, one-socket support, two memory
channels, a 128 GB maximum, DDR5-4800 support and 20 PCIe lanes.
[Intel processor specifications](https://www.intel.com/content/www/us/en/products/sku/236192/intel-xeon-e2434-processor-12m-cache-3-40-ghz/specifications.html),
[Intel DIMM population rules](https://www.intel.com/content/www/us/en/support/articles/000096438/processors/intel-xeon-processors.html).
The modeled butterfly riser has two Gen4 x8 electrical slots with x8/x16 physical
connectors. [Dell riser configuration](https://www.dell.com/support/manuals/en-us/poweredge-r360/r360_ism/expansion-card-installation-guidelines?guid=guid-9ffebd78-0ef4-4614-b698-c450c609054f&lang=en-us).
PCIe cards negotiate the lowest CPU, slot and card generation.

The inspector displays actual GB, maximum GB, channel use, operating speed and
DIMM placement. `lscpu` reports processor topology/limits; `free -h` reports
installed capacity. The existing weighted memory score remains a game resource
metric. Firmware timings, memory training, turbo behavior and measured workload
throughput are not emulated. Component power curves remain configurable estimates.

Inspector screenshots: [before](screenshots/server-limits-before-en.png),
[after in English](screenshots/server-limits-after-en.png),
[after in Russian](screenshots/server-limits-after-ru.png).

## Save compatibility

Existing component IDs and inventories are preserved. Old saves without
`ram_slot_indices` receive assignments in configured population order. Removing a
DIMM preserves the locations of remaining modules. Incompatible saved builds
remain in the save but cannot power on until their configuration is corrected.
New 32 GB DIMMs reuse the existing generic RAM illustration.

## Screenshots

The earlier [R360-only baseline](screenshots/server-limits-before-en.png) shows
how the server catalog looked before model expansion. Native captures of the
[new catalog](screenshots/server-models-shop-after-en.png),
[R760 details](screenshots/server-models-r760-after-en.png), and mixed
[front](screenshots/server-models-rack-front-after-en.png)/
[rear](screenshots/server-models-rack-rear-after-en.png) rack faces use a private
preview save containing all eleven platforms.

Server artwork uses clean chassis faces without guide arrows or labels. The shared
front-face renderer draws opaque empty cavities for unoccupied bays and carriers
only for installed drives. Shop previews use the selected configuration recipe;
rack and placement previews use the actual device inventory. Front and rear faces
may share an atlas with separate UV rectangles.
