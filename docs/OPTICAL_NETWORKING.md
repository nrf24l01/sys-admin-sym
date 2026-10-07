# Optical networking

## Player workflow

1. Open **Shop → Network → Optics & fiber**. Buy two compatible modules and a
   finished fiber cord. Buy the Catalyst C1000-24T-4X-L profile or an
   Intel X520-DA2 PCIe adapter when you need 10G. The original C1000-24T-4G-L
   has 1G cages.
2. Install and power the devices. A server needs a CPU and RAM, and its SFP+
   adapter needs an available x8-or-wider slot with eight PCIe lanes.
3. Click a cage in the rack. Its inspector lists compatible spare modules.
   Install one at each active end, then choose an owned cable in the inspector
   and click the destination socket. Both active endpoints require installed
   optical modules before a fiber connection is accepted. Routing anchors can
   be selected in between.
4. For a passive LC panel, connect both its front and rear. It does not need
   power and does not regenerate the signal. Configure the total duplex path
   so that each transmitter reaches the opposite receiver; flip a cord's
   polarity in the inspector if required.
5. Configure VLANs/IPs/routes on the devices as usual. Hardware alone does not
   supply an IP address, routing, transit or Internet connectivity.

DAC and AOC have permanently attached ends. They plug directly into two empty,
compatible SFP+ cages and are disconnected/reused as one assembly. They cannot
pass through a fiber patch panel. The **Modules and fiber cables** inventory
shows spare items; a selected port lists only usable installation choices.
Disconnecting returns the same finished cable and never creates bulk RJ45 stock.
Removing a NIC returns its modules and attached cables to inventory.

## Initial catalog

| Module | Cable | Mode | Nominal channel reach |
| --- | --- | --- | --- |
| 1G SX, 850 nm | OM3/OM4 duplex LC | 1G | 550 m |
| 1G LX10, 1310 nm | OS2 duplex LC | 1G | 10 km |
| 1G copper SFP | Ordinary RJ45 lead | 1G on current hosts | 100 m |
| 10G SR, 850 nm | OM3 / OM4 duplex LC | 10G | 300 m / 400 m |
| 10G LR, 1310 nm | OS2 duplex LC | 10G | 10 km |
| 1G BiDi U/D, complementary 1310/1490 nm | OS2 simplex LC | 1G | 10 km |
| 10G DAC | Attached copper assembly | 10G | Purchased 1/3/5 m |
| 10G AOC | Attached active optical assembly | 10G | Purchased 3/10/30 m |

Finished fiber cords have catalog lengths. The evaluator adds every cord's
length through passive panels; every connector and adapter contributes loss.
Nominal reach alone does not guarantee enough received optical power.

The physical references are Cisco's [1G SFP data sheet](https://www.cisco.com/c/en/us/products/collateral/interfaces-modules/gigabit-ethernet-gbic-sfp-modules/datasheet-c78-366584.html),
[10G SFP+ data sheet](https://www.cisco.com/c/en/us/products/collateral/interfaces-modules/transceiver-modules/data_sheet_c78-455693.html),
and [Catalyst 1000 data sheet](https://www.cisco.com/c/en/us/products/collateral/switches/catalyst-1000-series-switches/nb-06-cat1k-ser-switch-ds-cte-en.html).
The server NIC is the Intel X520-DA2 (Intel 82599, two SFP+ cages, PCIe 2.0 x8),
referenced from Intel's [X520 product brief](https://www.intel.com/content/dam/doc/product-brief/ethernet-x520-server-adapters-brief.pdf)
and [PCI-SIG certification](https://pcisig.com/intel%C2%AE-ethernet-server-adapter-x520-da2-0).
Its estimated 6 W board load is combined with separate module loads; Intel's
published adapter totals already include the selected media. Legacy generic
card IDs migrate on load while keeping interface and cable identities.
Vendor-qualified EEPROM lists and controller offloads are not simulated.

Generic module transmit powers, sensitivity/overload thresholds, electrical
loads, attenuation, connector losses and prices are simulation estimates,
not specifications for exact branded transceiver SKUs.

## Diagnostics

The inspector displays the shared physical fault and DOM TX/RX power where
supported. Cisco consoles support:

```text
show inventory
show interfaces transceiver
show interfaces Te1/0/25 transceiver
```

The original 1G switch uses `Gi1/0/25` through `Gi1/0/28`; the 10G profile uses
`Te1/0/25` through `Te1/0/28`. Linux servers use:

```text
ethtool INTERFACE
ethtool -m INTERFACE
```

Hot swapping retains VLAN/IP/interface settings and changes carrier immediately.
For convenient replacement, the simulator keeps the cable's logical endpoint
reserved while a module is out. It does not model loose connectors lying beside
an empty cage. Device `write memory` saves interface configuration; modules and
cables are physical world inventory saved by the game's save operation.

## Domain model and extension rules

`crates/sim/src/optics` separates model catalog, typed inventory commands,
physical compatibility/evaluation, and terminal diagnostics. The UI only issues
commands and presents domain results. `NetworkSim::link_status` is the source
for packets, GUI, CLI, activity and resource availability; no optical bypass
or implicit routes are created.

`assets/equipment/optics.json` supplies stable model IDs and inline
`display_name`/`desc` language maps. GUI messages use semantic IDs in locale
catalogs. Hardware profiles declare cage kind, supported speed/lane/FEC modes,
and maximum module power. Module modes must intersect host modes and the
other endpoint's modes. A newer cage's shape alone never grants compatibility.
The switch profile reuses the existing 24-RJ45/four-cage factory, and the panel
profile reuses the 24-pair factory. Different port layouts need a factory and
rack-projection extension, while the physical link evaluator stays shared.
Physical catalog definitions are bundled at build time; item presentation
translations also support runtime overrides like the other equipment files.

Lengths use centimeters, optical powers use milli-dBm, losses use milli-dB,
wavelengths use nanometers, and electrical loads use milliwatts. Integer
arithmetic keeps outcomes deterministic. Add SKUs by adding catalog entries
with unique stable IDs; preserve existing IDs/lengths for save compatibility.
Model instances and ownership use saved IDs independent of interface config.
Older saves default to an empty optics inventory and preserve copper wiring.

SFP28 and 25G rates, lane counts and FEC are schema foundations; this catalog
ships only 1G/10G equipment. New physical families, QSFP/breakout, MPO,
WDM muxes, vendor EEPROM locks, detailed DDM temperature/voltage/bias,
thermal throttling, per-lane errors and measured throughput need additional
domain behavior. They are not exposed as working equipment yet.
