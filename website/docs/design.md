# Cloud Provider Simulator website

Build a standalone Vue website in `website/`. The user approved the hardware-editorial direction and requested implementation in a separate folder. All website source, documentation, dependencies, and assets stay here; no deployment or game changes.

## Experience

The primary task is obtaining the game and optional standalone game-ssh client for Windows or Linux. A compact English/Russian navigation leads to gameplay, the terminal client, downloads, on-site documentation. The game download includes game-ssh; the standalone client requires a running game.

The opening pairs oversized condensed typography with a rack assembled from existing game equipment artwork, explicitly captioned as game equipment. Real gameplay screenshots substantiate hardware and cabling sections. Hosting is described from existing docs without fabricated screenshots or claims of a full protocol stack. A terminal example uses real supported commands.

Warm paper `#ECEAE3`, ink `#191C1B`, equipment black `#101518`, teal `#287568`, and muted amber `#B77A2C`. Self-hosted fonts, thin dividers, square buttons, deliberate spacing, no gradients or ornamental cards. Restrained GSAP entrance motion, native scrolling, and reduced-motion support. Responsive down to 320px; keyboard focus and semantic landmarks.

## Downloads

Load public GitHub releases for `nrf24l01/sys-admin-sym` through a centralized, cancellable request. Validate external data and select only stable published release ZIPs with exact expected game/client platform filenames. Resolve each platform/package independently, keeping checksums and metadata with their release. Never invent a version, asset, size, or download URL. Missing packages have disabled download buttons. Loading, missing packages, and API failures are explained separately; failures offer retry. Release notes expand on site, and checksums link directly to release files. Visitors never need to browse Actions or release listings. Optional build-time HTTPS URL overrides support privately hosted game/client ZIPs without a frontend backend.

Language persists when local storage is available; storage failures never prevent rendering. Unsupported operating systems are not silently described as supported. Copy buttons show feedback and handle clipboard denial.

## Documentation

English and Russian pages have shareable locale URLs. The reader bundles every project guide, developer reference, equipment attribution, and original artwork prompt, with source screenshots. Search by guide title, use grouped navigation and heading links, and switch languages while retaining the article and anchor. Sanitize Markdown, rewrite documentation links to local routes, and preserve manufacturer reference sources. Docs remain readable without the GitHub API.

## Implementation and validation

Vue 3 + strict TypeScript, Vite, Tailwind 4, GSAP, Bun, and shadcn-vue primitives. Feature components remain separate from a small App shell. Vitest tests exercise download selection and network failures; Playwright checks desktop/mobile layout, OS and language changes, release-only downloads, clipboard feedback, and reduced motion. Run website checks and the repository's required `make check`. Preserve source attribution for copied imagery and fonts.
