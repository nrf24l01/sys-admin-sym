# Equipment shop

Open **Shop** to browse All products or choose a category:

| Category | Product types |
| --- | --- |
| Network devices | Routers, switches |
| Servers & parts | Server systems, processors, memory, network cards, storage drives |
| Cables & modules | SFP modules, fiber cables, DAC/AOC cables, copper supplies |
| Rack accessories | Patch panels, cable management |
| Power | UPS, PDU |
| IP resources | Public IPv4 pools |

The sidebar shows the active category's product types and a category switcher.
Overview buttons and breadcrumbs also work with the sidebar collapsed in a
small window. Each category/type keeps its own scroll position. The catalog
contains every purchasable built-in model. Server chassis and full pack are
separately priced configurations; cable families expose concrete length variants.
The product and offer counts distinguish families from purchasable variants.

All products and category overviews show budget filters only. Choose a product
type for its relevant technical filters: RAM capacity and memory type, fiber
grade/arrangement/length for fiber cables, cage/speed/medium for SFP modules,
and so on. Relevant filters remain visible even when the catalog currently
has only one value, such as a single CPU socket or RAM type. Sorting by capacity,
speed or length is offered only for applicable product types. Changing types resets an
inapplicable sort while retaining general price/name sorting.

Servers expose RAM type, CPU socket, RAM slot count, configuration, CPU socket
count, PCIe slot count/generation/width, drive bay count/interface, rack size,
included RJ45 ports and integrated PSU power. Physical capacity comes directly
from the simulation's chassis catalog and is the same for bare/full-pack offers.
RAM type, CPU socket and RAM slots appear first. Server sorting also supports
socket and RAM type alphabetically and RAM/PCIe slots, CPU sockets and drive
bays by count.

Search matches localized names, descriptions, IDs and specifications. Every
space-separated search term must match. Price limits are inclusive. Filters
within one attribute use OR; different attributes use AND. Counts show matching
product families with the current attribute temporarily excluded, so another
value remains selectable. Other choices stay visible with disabled zero counts
as results narrow, and selected values with no results remain removable.
Category/section changes save and deactivate technical filters;
returning restores them. Search and price limits apply across categories.

Use Grid or List and sort by category, name, price, capacity, speed or length. Details
show specifications, inventory counts and requirements. Compare up to three
products of the same type. Small windows collapse the category/filter sidebar;
details use a separate window, while wider windows can show an inline inspector.
All controls and product descriptions support English and Russian.

Within a compatible product type, select a configurable server or an optical
endpoint as a compatibility target.
**Compatible only** checks a currently available installation position or cable
endpoint. It does not guarantee an operational end-to-end link: the other
endpoint, installed modules, route, reach, optical budget, power and interface
state remain subject to the simulation's authoritative link evaluator. Occupied
slots or missing PCIe lane budgets can therefore exclude a physically matching
component until the server is prepared. Clear the target to browse freely.

Quantity purchases show the unit price and complete total. Hover Buy for the
remaining balance; unavailable purchases explain the failure. Purchases are
committed atomically in the simulation and accept 1–100 units. A correlated
worker reply follows the committed snapshot before the next purchase is enabled.
Failed transactions preserve money, inventories, devices and IDs. IPv4
availability excludes overlapping provider pools, not only previously sold /29s.
Finished fiber/DAC/AOC assemblies never consume or return bulk RJ45 materials.

The shop adapts domain catalogs and prices into one presentation catalog in
`crates/game/src/ui/shop/catalog`. Equipment counts and optical cage capabilities
come from domain-created previews rather than duplicated GUI port constants.
`crates/sim/src/shop.rs` owns purchase quotes and atomic quantities. Component and
optical model previews share the installation/endpoint rules used by commands.
Shop browsing state is session-only; the save schema is unchanged.

`assets/equipment/shop_products.png` provides twelve padded transparent artwork
cells for components, supplies, panels, cable management and address allocation.
Rack devices reuse existing artwork with aspect-correct face UVs. Optical items
retain their existing module sprites and the dedicated complete-connector atlas.
See `assets/equipment/ATTRIBUTION.md` and `SHOP_ARTWORK_PROMPT.md` for sources and
the generated atlas prompt.

Bevy decodes source PNGs on its asset workers. The update system transfers the
decoded pixel buffer into an asynchronous preparation task without copying it
on the UI thread. Workers convert straight PNG alpha to the premultiplied linear
RGB expected by egui, then build smaller shared shop atlases in linear space.
Full-resolution equipment textures remain available for racks and item details.

Shop thumbnails use cached 512/1024/2048-pixel atlas sizes selected from the
window width and display scale. Actual resampling runs on workers on initial
asset loading or when a resize/scale event requests an uncached size. Mouse
motion, scrolling, purchases and ordinary redraws reuse existing texture IDs.
Returning to an earlier size reuses that thumbnail. Source changes invalidate
only their own cached textures. GPU uploads use a single mip level and bilinear
sampling; original PNG files retain ordinary transparency.

Only active views register their images with egui. Hidden views and inactive
thumbnail sizes release their egui bindings while retaining the uploaded GPU
images, so reopening a view needs no pixel work or upload. This also keeps
egui's per-frame bind-group construction limited to active shared atlases.

Product grid and list views use uniform virtual rows through `ScrollArea::show_rows`.
Only visible rows plus egui's small overscan are constructed. Product variant,
quantity and purchase state remains keyed by offer/family ID while cards scroll
out of view. Image-fit geometry and column layout are cached until their bounds
or view mode change. Cards draw shared atlas UVs through lightweight texture IDs;
details use the cached full-resolution version.

Searchable product text is cached per immutable translation catalog, including
language switches and runtime translation reloads, instead of being rebuilt
for every offer and facet on each redraw. Development builds optimize
third-party dependencies and lightly optimize workspace code, retaining debug
symbols and assertions.

The simulation worker runs independently of UI redraws. Idle focused windows
refresh at 10 Hz, background windows at 4 Hz, and input wakes the window
immediately. Unchanged modifier-key events are discarded before egui begins a
pass; real transitions remain intact. Text fields use a steady caret because
the current integration turns delayed blink requests into immediate redraws.

## Native previews

- [Previous shop](images/shop-before.png)
- [Redesigned catalog and filters](images/shop-after.png)
- [Server configurations and component artwork](images/shop-components.png)
- [Switch comparison](images/shop-comparison.png)
- [Committed purchase and updated inventory](images/shop-purchase.png)
- [Cable thumbnail before the alpha fix](images/shop-render-before.png)
- [Cable thumbnail after the alpha fix](images/shop-render-after.png)
- [Rendering after a narrow-window resize](images/shop-render-narrow.png)
- [Virtual grid with cached thumbnails](images/shop-cached-grid.png)
- [Russian list view and committed purchase](images/shop-cached-list-ru.png)
- [Cached full-resolution item details](images/shop-cached-details.png)
- [Cached thumbnails after resizing](images/shop-cached-resize.png)
- [New categories and budget-only overview filters](images/shop-categories.png)
- [Fiber filters restored after switching product types](images/shop-fiber-filters.png)
- [SFP module filters](images/shop-module-filters.png)
- [Russian memory category and its relevant filters](images/shop-memory-category-ru.png)
- [Selected server RAM type, socket and RAM slot filters](images/shop-server-filters.png)
- [Server specification sorting](images/shop-server-sort.png)
- [Russian server specifications and filters](images/shop-server-filters-ru.png)
