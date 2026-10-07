# Equipment shop

Open **Shop** to browse Network, Compute, Power or All products. The sidebar
expands the active category so its filters stay within reach. The catalog
contains every purchasable built-in model. Server chassis and full pack are
separately priced configurations; cable families expose concrete length variants.
The product and offer counts distinguish families from purchasable variants.

Search matches localized names, descriptions, IDs and specifications. Every
space-separated search term must match. Price limits are inclusive. Filters
within one attribute use OR; different attributes use AND. Counts show matching
product families with the current attribute temporarily excluded, so another
value remains selectable. Selected values with no results remain visible and
removable. Category/section changes save and deactivate technical filters;
returning restores them. Search and price limits apply across categories.

Use Grid or List and sort by category, name, price, capacity, speed or length. Details
show specifications, inventory counts and requirements. Compare up to three
products of the same type. Small windows collapse the category/filter sidebar;
details use a separate window, while wider windows can show an inline inspector.
All controls and product descriptions support English and Russian.

Select a configurable server or an optical endpoint as a compatibility target.
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
