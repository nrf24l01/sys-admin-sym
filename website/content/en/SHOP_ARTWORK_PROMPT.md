# Shop product atlas

Generated with the built-in OpenAI image generation tool for this project.
The deliverable is `shop_products.png`, a 1448 × 1086 RGBA atlas with four
columns and three rows. Every complete cell has transparent padding. The
illustrations are representative, unbranded game artwork; product specifications
are supplied by the simulator. The depicted number of plugs, panel sockets, or
address blocks is decorative and does not establish a product quantity.

## Final prompt

Use case: product-mockup. Asset type: one game product-thumbnail sprite atlas,
4 columns by 3 rows, landscape 4:3 aspect ratio. Create a single clean
studio-rendered sprite sheet with EXACTLY TWELVE separate unbranded hardware
product illustrations, one centered fully inside each equal-sized padded cell.
Genuine transparent background across the sheet, no visible grid, no shadows
beyond each product, no text, no labels, no logos, no numbers. Every object must
be fully visible with generous transparent margins occupying no more than 75
percent of cell width or height; no object may cross its cell boundary.
Consistent realistic brushed metal, dark PCB materials, soft white studio
lighting, subtle three-quarter perspective, crisp readable silhouettes at small
thumbnail scale.

Row 1 left to right: (1) square server CPU, silver heat spreader and small green
substrate; (2) single long DDR5 ECC RAM DIMM green PCB with black chips and gold
contacts; (3) PCIe Ethernet network card with four RJ45 copper sockets on its
metal bracket; (4) PCIe network card with two empty SFP+ cages on its metal
bracket.

Row 2 left to right: (1) 3.5 inch enterprise hard drive with silver metal lid and
black base; (2) 2.5 inch enterprise SATA SSD with dark graphite metal enclosure;
(3) brown cardboard pull box of bulk Ethernet cable with a short blue cable
emerging from its side; (4) small orderly group of several complete clear plastic
RJ45 plugs with gold contacts.

Row 3 left to right: (1) black 1U copper patch panel with a row of RJ45 sockets;
(2) black 1U fiber patch panel with a row of aqua duplex LC adapter sockets;
(3) black 1U horizontal rack cable manager with open rounded routing rings;
(4) an unbranded small globe above a tidy group of connected blue address blocks,
a simple dimensional network service illustration.

The long rack panels should be wide and low within their cells, not stretched.
The thumbnail illustrations are representative game artwork, not branded
manufacturer photographs. Maintain EXACT 4x3 row-major ordering and equal cell
boundaries for programmatic UV sampling.
