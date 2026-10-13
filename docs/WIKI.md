# Wiki and first VM onboarding

Open **Wiki** from the top bar. This independent, resizable window stays available
throughout the game, including after your first sale. Its default article is
**Sell your first VM**. **First VM guide** shows live tasks; **Articles** opens the
full article list. Both navigation and article content scroll independently.
Click a task for its lesson, current result, and actionable instructions. Code
blocks have a **Copy** button; pasting is an explicit action in a device console.

Select distinct physical servers for **Controller** and **VM node**. Automatic
selection prefers the publicly connected controller, installed controller service,
and registered node, then falls back deterministically to owned physical servers.
These selectors scope checks; they do not change device roles or configurations.
Installing services and preparing a node establishes the actual hosting roles.

## Progress and correctness

`NetworkSim::first_vm_guide` computes current prerequisites from saved world state.
They accept either GUI or console setup and do not require following a fixed order.
Management links use the authoritative physical link evaluator, permit passive
patch panels, and connect the selected servers to the same switch. The management
lesson checks a declared VLAN other than default VLAN 1, compatible private
addresses, and actual bidirectional packet reachability using Linux route selection.

Controller installation, private node registration, guest gateway discovery,
public storefront and product admission reuse the existing hosting simulation.
Internal preparation can complete before the public storefront is connected.
Product readiness tests the selected node rather than accepting a different node
from a product's eligible set. Published paid VM products need usable public IPv4,
real resource capacity, and working paths before their task is complete.

A task can become pending again after a broken link or configuration change.
The separate **First paid VM sold** milestone uses saved non-renewal payments and
immutable purchased service specifications. A positive prepaid VM purchase counts;
a zero payment, a renewal, or dedicated physical-server purchase does not. Service
and payment history preserve that milestone through VM destruction, termination,
refunds, product deletion and save/load. The guide does not add manual completion
flags or modify the save schema, economy, provisioning, or market demand.

## Authoring Markdown articles

Articles are runtime assets beneath:

```text
assets/wiki/
  en/
    tutorial/first-vm.md
    tutorial/management-vlan.md
    reference/layer2-vlans.md
  ru/
    tutorial/first-vm.md
    tutorial/management-vlan.md
    reference/layer2-vlans.md
```

The loader recursively discovers `.md` files and sorts their relative paths.
The path without `.md` is the stable article ID. The first level-one heading is
the display title. Add a Markdown file to add a browsable article; no Rust registry
edit is needed. A translation uses the same relative path under its language
folder. Missing individual translations fall back to English.

The renderer uses pulldown-cmark for CommonMark parsing, with egui presentation of
headings, paragraphs, ordered/unordered lists, emphasis, inline code, fenced code,
block quotes, separators and links. Raw HTML and embedded images are not rendered;
use fenced text for topology diagrams. Task-list Markdown is static article text;
the live first-VM tasks come from the simulation checks.

Article links are relative Markdown links, for example
`[VLANs](../reference/layer2-vlans.md)`. A heading fragment uses its lowercased text,
spaces changed to hyphens and punctuation removed. Links cannot escape the Wiki
root. HTTP/HTTPS links open on an explicit click; other URL schemes do not open.
Unknown articles or fragments show an error in the window. **Back** returns to the
previous article. **Reload articles** rereads the folder, including added/deleted
files. Load errors are shown while other valid articles remain available; symbolic
links are skipped. Packaged builds include the folder through normal asset packaging.

## Extending task checks

Domain task IDs and evaluations live in `crates/sim/src/guide`; presentation labels
and hints live in `assets/locales/{en,ru}.json`. `crates/game/src/ui/wiki.rs` maps each
existing task to its lesson article. Change simulation predicates in the domain
layer and add regression coverage using the hosting integration lab. Keep Wiki
probes read-only; reuse hosting/packet evaluation rather than approximating it in UI.
The UI refreshes its cached report when the simulation snapshot or selected roles
change. Existing registration restores automatic role selection after loading.

Screenshots: [first-VM overview](screenshots/wiki-first-vm.png) and
[management ping task](screenshots/wiki-management-ping.png).
