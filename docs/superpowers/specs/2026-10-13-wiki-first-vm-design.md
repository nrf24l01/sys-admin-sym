# Wiki and first sold VM guide

## Agreed purpose
Teach a new player to build a hosting provider from an empty room through their
first paid VM order. Wiki is a permanent top-bar window like Shop and Bank.
Markdown articles explain concepts when needed and include supported console
commands. The first-VM checklist validates the actual simulation, accepts GUI or
console configuration, permits steps in any order, and reports current problems.
The first paid VM sale remains a completed milestone after subsequent changes.
The user approved the flow, progress model, layout, and requested implementation.

## UI and content
The resizable Wiki window contains a scrollable navigation/checklist column and
an independently scrolling article pane. It opens on the first-VM overview.
Tasks link to instructional articles; a separate reference collection explains
networks, L2/L3, VLANs, IP addresses, routing, and diagnostics. Controller/node
selectors disambiguate equipment in larger worlds; defaults prefer existing
hosting roles and otherwise select distinct physical servers deterministically.
The guide shows current completion and actionable pending instructions even
when an article is revisited after the first sale. A separate completed banner
records the historical sale; breaking a prerequisite does not erase that banner.

Articles live in `assets/wiki/en` and `assets/wiki/ru`, recursively discovered
from `.md` files; relative paths without the extension are stable article IDs.
English supplies individual missing translations. The first level-one heading
is the title. A Reload button reloads files; unreadable files are reported and
other articles remain usable. Runtime assets are resolved with `asset_root()`
and bundled by the existing package tool. Markdown uses a CommonMark parser
and egui rendering for headings, paragraphs, lists, emphasis, code blocks,
quotes, separators and article links. Code blocks have Copy buttons. Relative
links remain within the Wiki library. No article link executes a command.

## Simulation checks
`NetworkSim::first_vm_guide(controller, node)` produces a read-only report.
The chosen roles are distinct physical servers. Checks cover: owning a switch;
assembling two servers with real Ethernet interfaces and node storage; racking
and powering them and their management switch; connecting management ports to
the same switch (passive patch panels allowed); using a declared non-default
management VLAN; distinct private IPv4 addresses on a shared subnet; bidirectional
packet reachability; active controller service; prepared node registered to that
controller with a working private path; owned public range; reachable guest data
network/gateway; publicly reachable storefront; published VM product with actual
provisioning capacity; first positive non-renewal VM payment.

Check results use existing `link_status`, Linux route selection, packet probes,
hosting preparation, network attachments, and provisioning readiness. A successful
ping is represented by current packet reachability rather than command-history
matching. Raw domain readiness errors accompany actionable localized instructions.
Probes never modify the authoritative world or counters. UI caches the report until the simulation snapshot or chosen roles change,
and refreshes it when the window reopens.

The sale milestone comes from saved service/payment history, excluding zero-price
orders, renewals, and dedicated physical-server products. It survives VM removal,
termination, product removal, and save/load; no save-schema migration is required.

## First journey
Buy switch, two server full packs or equivalent components, cabling, and router.
Connect and power equipment; isolate management on access VLAN 10; configure
10.0.10.10/24 and 10.0.10.11/24 on real management Ethernet interfaces. Verify
ping, install `./setup-main-hoster-server.sh`, then run
`./vm-prepare.sh 10.0.10.10` on the node. Buy a public /29 and apply its delivery;
configure the router WAN/default route and customer VLAN 20 gateway. Connect the
controller public and node data ports on VLAN 20; use guest subnet rules for the
node. Connect the public storefront, publish a modest VM plan, and advance time
for the market to purchase it. Examples explicitly identify each target device,
interface and substitution; persistent Linux configuration is included. Starting
cash may require a loan; the guide explains its schedule and avoids changing cash.

## Verification
Regression coverage exercises empty and existing worlds, crossed role selections,
VLAN/address faults, broken links, real hosting readiness, paid/free/dedicated
sales and history round-trips. Content tests exercise folder discovery, fallback,
reload, missing/broken links, fenced commands and localized article coverage.
Egui tests exercise window opening/closing, navigation, constrained resizing and
text/copy controls. Complete with the repository's required `make check`, including
Linux startup, and capture a Wiki screenshot when a graphical runtime is available.
