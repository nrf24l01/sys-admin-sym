# Configurable server hardware

The shop's Dell PowerEdge R360 chassis and its processors/DIMMs use
`assets/equipment/server_parts.json`. Installed JSON overrides embedded defaults
once at startup. Invalid topology or missing limits fail with the filename and
reason. Restart after editing equipment configuration.

## Catalog limits

The chassis specifies CPU sockets, supported CPU IDs, maximum CPU TDP, whether
CPUs must match, and whether components can be changed while powered on. Drive
bays specify their interfaces. PCIe slots specify their owning CPU, electrical
lanes, physical connector width and generation. The existing `power` profile in
`server_config.json` defines per-PSU capacity and conversion efficiency. Both
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
