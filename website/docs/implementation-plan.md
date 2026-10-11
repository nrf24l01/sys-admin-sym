# Download website implementation plan

**Goal:** Deliver the approved game download site entirely in `website/`.

**Architecture:** A static Vue application composes the showcase, direct release downloads, and a bundled bilingual documentation reader. Locale routes preserve guide and heading navigation. A centralized GitHub request validates published download metadata; missing packages remain unavailable.

**Stack:** Vue 3, TypeScript, Tailwind 4, GSAP, Bun, shadcn-vue, Vitest, Playwright.

**Spec:** [design.md](design.md)

## Constraints and review focus

- Keep changes inside `website/`; do not publish or change the Rust application.
- No invented release URLs, versions, file sizes, or unsupported platform claims.
- Check release mixtures, prereleases, unsafe URLs, network failures, and empty results.
- Check 320px mobile layout, keyboard controls, language persistence, and reduced motion.
- Clipboard and storage failure must preserve a usable page.

## Tasks

- [x] Scaffold the Bun/Vite application and add shadcn-vue button, tabs, and accordion primitives. Add tests for selecting stable game/client assets and rejecting unrelated or unsafe release data; observe failures before implementing the resolver and centralized API handling.
- [x] Build the editorial page with self-hosted typography, attributed equipment artwork, actual screenshots, bilingual copy, mobile navigation, terminal command copying, and restrained accessible GSAP motion.
- [x] Connect the shared download state to primary calls to action and platform tabs. Handle loading, empty and failed requests, retry, checksums, and configured HTTPS links.
- [x] Verify with typecheck, meaningful unit and browser tests, desktop/mobile screenshots, production build, and the required root `make check`. Document local preview, deployment output, and release configuration.

## Execution notes

Implementation stays in the requested folder in the shared checkout. The user's instruction to create the approved website supplies implementation authorization; no additional approval round, branch, commit, or hosted site is needed.

## Release downloads and on-site documentation revision

- [x] Remove workflow fallbacks; download stable release ZIPs directly and disable missing packages. Render release notes in place.
- [x] Add locale routes and a searchable documentation reader with local links, bundled images, safe Markdown, heading navigation, and complete English/Russian content.
- [x] Add a source-sync command, update setup notes, run browser regressions and full repository verification, and capture revised views.
