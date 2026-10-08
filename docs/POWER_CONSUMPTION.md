# Device power consumption

Power now follows installed hardware and simulated work. PSU capacity, CPU TDP,
NIC ratings and UPS output limits describe sizing limits; they are not constant
operating consumption. The inspector shows actual powered draw, an estimated
full-load figure, component contributions, utilization and sustained workload
controls. A powered-off device or an incomplete server chassis contributes zero
load to its circuit. Passive patch panels and cable managers consume no power.

## Calculation

Every powered model has a `"power": {}` block in its equipment JSON. Operating
curves are explicitly estimated; manufacturer ratings remain sizing metadata. These curves
are deterministic gameplay estimates, not vendor measurements or an electrical
model of every firmware power-saving state.

For each component, `power = idle + (full_load - idle) × utilization`.
Intermediate calculation uses milliwatts; the circuit rounds the combined input
up once to whole watts. Utilization is clamped to 0–100%.

For an assembled Dell R360:

- Board: 22–30 W, including onboard management electronics.
- Each CPU: its own idle/full-load power curve (8–55 W for the supplied E-2434).
  The CPU TDP remains separate sizing metadata.
- RAM: its own module curve (1.6–4 W for the supplied 16 GB DIMM), reflecting
  refresh and memory activity.
- SATA HDD: 5 W spinning idle up to its configured active draw; SSD: 1 W idle
  up to its configured active draw. Empty drive bays add nothing.
- NIC: its own idle/full-load curve. Negotiated links and real traffic
  increase draw toward the board rating, independently for each installed card.
- Optical modules: their catalog draw, counted once and separately from NICs.
- Fans: 3–10 W following CPU/storage activity.
- PSU losses: DC component demand is converted to AC input using an estimated
  load-dependent efficiency curve. The 600 W rating is DC output capacity.

The supplied full-pack server typically idles around 46 W with no network links;
its CPU, RAM and storage load can increase that draw substantially. Exact whole
watts depend on active links, services, installed components and recent work.
Legacy servers without a component inventory use an estimated 45–180 W envelope.

Catalyst 4G/4X idle and full-traffic endpoints come from their existing Cisco
specifications. A quarter of the dynamic allowance follows negotiated active
link capacity; the remainder follows byte rate over total installed capacity.
A single ping therefore stays close to idle instead of instantly selecting the
full-traffic value. Transceiver draw is added separately.

The C1111 router uses an estimated 8–40 W DC envelope driven by CPU demand,
links and traffic. Its 66 W adapter is a capacity limit, not its idle consumption.
Adapter loss and power factor are applied upstream once, including behind PDUs
and UPSes.

PDUs add their existing 5 W overhead when available. UPS draw includes downstream
load, an estimated 8 W controller baseline, conversion loss and actual battery
charging. The controller also drains the battery with no connected devices.
Disabling battery output stops that drain; charging on available mains follows
the existing UPS behavior. Source inspectors separate downstream output from
own draw, losses and charging.

## JSON configuration

| File in `assets/equipment` | `power` settings |
| --- | --- |
| `server_config.json` | Board/fan curves, PSU capacity/efficiency, power factor, legacy-server envelope |
| `server_parts.json` | Per-CPU, per-DIMM and per-NIC curves; NIC link share |
| `drives.json` | Per-drive idle/full-load curves |
| `switches.json` | Per-Catalyst curve, link share and power factor |
| `router_config.json` | Router curve/link share; adapter DC limits, efficiency and AC power factor |
| `ups_config.json` | Output limits, battery capacity, conversion efficiency, charging, own draw and outlets |
| `pdu_config.json` | Output/current limits, own draw and outlets |
| `optics.json` | Per-module curves, including DAC/AOC ends; cages check peak module draw |

For example, `router_config.json` contains:

```json
"power": {
  "idle_mw": 8000,
  "peak_mw": 40000,
  "link_share_permille": 250,
  "power_factor_percent": 90,
  "adapter": {
    "output_watts": 66,
    "output_volts": 12,
    "output_current_ma": 5500,
    "efficiency_percent": 90,
    "power_factor_percent": 90
  }
}
```

Curves use milliwatts; capacity/charging/own-draw fields ending in `watts` use
watts. `link_share_permille` uses 0–1000, percentages use 1–100, and PSU efficiency
points contain `[load_permille, efficiency_permille]`. Idle/full-load values are
operating estimates, separate from CPU `tdp_w` and electrical capacity ratings.
Constant-draw hardware uses identical idle and peak values. Passive panels,
cable managers and fiber strands add no electrical load; DAC/AOC ends use their
referenced transceiver model without double counting.

The simulation first reads `assets/equipment` relative to the working directory,
once per catalog/profile. Restart after changing these files; rebuilding is not
required. Missing files use embedded defaults, including headless tools started
without an asset directory. Existing files with invalid configuration fail with
the filename and parse error. Profiles validate curve ordering, link shares,
power factors, positive capacities, supported outlet counts and ordered PSU
efficiency points. No per-frame file reads or hot reload are performed.

Old saves recalculate model power on load. UPS/PDU settings follow the current
JSON while preserving enable/trip state and stored battery energy; lowering
battery capacity clamps energy rather than filling it. Adapter limits apply
when connecting a router and when its workload changes; exceeding either DC
watt or current capacity turns its output off until demand falls.

## Simulated work

The packet engine records bytes per physical endpoint in ten 100 ms buckets.
The denominator is one second, including quiet time. Traffic is bounded by
negotiated full-duplex capacity, and switches/NICs compare it with installed
port capacity. Stale traffic expires at simulation-time bucket boundaries.

Guest file reads/writes contribute bytes toward installed storage throughput;
`/tmp`, `/proc` and `/dev` are treated as memory/virtual filesystems. Guest commands
add an estimated CPU work budget based on command size and file bytes. Active
services add a small background CPU allowance. Reading `power`, `ps` or `uptime`
does not itself create CPU load. Power calculation uses guest simulation data;
it never reads host CPU utilization or runs host workloads.

For sustained demand, use the inspector sliders or the simulated server command:

```text
power
power workload 75 50 25
power workload 0 0 0
```

The three percentages add CPU, memory and storage demand to automatic activity.
Routers expose CPU demand only. Invalid percentages fail atomically. The domain
API is `Command::SetDeviceWorkload` with per-mille fields (0–1000).

## Electrical behavior and saves

Updated demand feeds the existing rack/PDU breakers, adapter limits and UPS
battery model. Simulated time advances piecewise while activity expires, so a
long time step does not charge its entire duration at an obsolete busy reading.
Once recent work is gone, idle time advances in one step. Split and combined time
advances produce the same battery result.

Workload settings persist in the world. Old saves default to zero sustained
workload and recalculate consumption from their existing hardware. Activity,
byte windows and cached power estimates are transient and rebuilt on load.
Turning power off retains configured workload for the next startup but consumes
zero circuit watts. Calculations run in the simulation worker; UI readouts use
cached results and slider edits submit when the drag finishes.

Rating references: [Intel Xeon E-2434 specifications](https://www.intel.com/content/www/us/en/products/sku/236192/intel-xeon-e2434-processor-12m-cache-3-40-ghz/specifications.html),
[Dell R360 PSU specifications](https://www.dell.com/support/manuals/en-au/poweredge-r360/r360_ism/psu-specifications?guid=guid-ff8a8542-c09e-4bb9-9b35-710b1366b973&lang=en-us),
[Cisco ISR 1000 data sheet](https://www.cisco.com/c/en/us/products/collateral/routers/1000-series-integrated-services-routers-isr/datasheet-c78-739512.html).

## Inspector comparison

Native game captures use the same full-pack R360 with no network links. The old
constant calculation shows 169 W. The new inspector shows 46 W at idle and 97 W
with sustained CPU demand at 75%; the rack circuit updates to match. Slider
values persist after release, clearing the workload returns to idle, and turning
the device off shows zero draw while retaining its configured workload.

- [Before: constant component ratings](screenshots/power-consumption-before.png)
- [After: idle breakdown and workload controls](screenshots/power-consumption-idle-en.png)
- [After: 75% CPU demand](screenshots/power-consumption-load-en.png)
- [Powered off: zero draw and activity, retained workload](screenshots/power-consumption-off-en.png)
- [Russian inspector](screenshots/power-consumption-idle-ru.png)

A separate native verification run used temporary, intentionally artificial JSON
overrides: a 500 W PSU with 100% conversion efficiency and changed component
curves. The [inspector shows those overrides](screenshots/power-consumption-json-override-en.png),
including 54 W idle and the 500 W capacity, without rebuilding the binary. The
repository defaults remain the original estimates.
