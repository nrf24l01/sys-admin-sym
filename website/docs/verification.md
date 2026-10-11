# Verification

Validated on 2026-10-11.

- `bun run check`: strict Vue/TypeScript checking, 17 unit tests, production build, and 14 Chromium browser tests passed.
- Download coverage: published platform-specific ZIP/checksum selection, independent client releases, rejected release-page URLs, disabled missing packages, loading/error/retry, and on-site sanitized release notes.
- Documentation coverage: all 24 English and 24 Russian source records load, local guide links resolve, bundled screenshots exist, manufacturer image-source links remain intact, search/navigation/back/reload work, and canonical heading links survive locale changes.
- Browser coverage also includes real imagery, rack controls, persisted EN/RU copy, keyboard tabs, clipboard success/denial, unavailable storage, 320px home/reader layouts, unknown articles, and GSAP/reduced-motion cleanup.
- A production-preview Chromium check passed for English documentation, Russian desktop/mobile articles, and Russian mobile home. Documentation loaded with GitHub API requests blocked; no GitHub browse links or page errors were found. Screenshots were captured and visually inspected in [screenshots](screenshots/).
- Root `make check`: formatting, whitespace, Clippy, 436 Rust tests, 7 Python tests, workspace build, and the real Linux game/authenticated-console smoke test passed.
- The machine lacks a system Lavapipe manifest. The full root check used `VK_DRIVER_FILES` pointing to an existing temporary driver; no system packages were installed or changed.
- Independent read-only review found no remaining Critical or Important issues after the reported fixes.

All source changes are inside `website/`. No deployment or release publication was performed. The public release API returned no releases during verification, so real download packages currently remain unavailable; published ZIPs will be discovered automatically. HTTPS package URL overrides are documented in the README. Original English generation prompts remain verbatim in the Russian source records for reproducibility, with translated titles and introductions.
