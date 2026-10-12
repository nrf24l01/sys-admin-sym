# Cloud Provider Simulator website

A standalone download site built with Vue 3, TypeScript, Tailwind CSS 4, and GSAP. It uses shadcn-vue controls, self-hosted fonts, equipment artwork from the game, and actual gameplay screenshots. The interface and documentation are available in English and Russian with shareable locale URLs.

## Develop

Requires Bun and Node.js 22.12+ (or a newer supported Node.js release).

```sh
cd website
bun install --frozen-lockfile
bun run dev
```

Open the local URL printed by Vite (normally `http://127.0.0.1:5173`).

## Verify

```sh
bun run check
```

This runs strict Vue/TypeScript checking, unit tests, a production build, and Playwright browser tests. Browser tests use `/usr/bin/chromium` when available; otherwise install Playwright's Chromium with `bunx playwright install chromium`. Override the browser executable with `PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH` if needed. Desktop/mobile screenshots are written to `docs/screenshots/`.

The Rust repository still uses its normal root verification command:

```sh
make check
```

## Build and serve

```sh
bun run build
bun run preview
```

`dist/` is a static site. Configure the host to serve `index.html` for page URLs such as `/ru/docs/game-ssh`; this SPA fallback is required for refreshes and shared article links. The host must also serve a `runtime-config.js` that sets `window.__APP_CONFIG__.downloadBaseUrl`. The repository copy is empty, so downloads remain disabled until a base URL is configured. Nothing is deployed by development or build commands. Assets use root-relative paths, so host the site at the domain root unless you adjust Vite's base and the public asset references.

The production container serves the built site on port `8080` and includes SPA routing and a healthcheck. Compose publishes it on localhost so a host reverse proxy can route traffic to it. Set the required public download base URL in `website/.env` first:

```sh
cd website
cp .env.example .env
# The example uses same-origin resources at /resources/.
docker compose -f compose.prod.yml up -d
```

`compose.prod.yml` pulls `ghcr.io/nrf24l01/sys-admin-sym-website:latest`. To build and run the checked-out source locally instead, use `docker compose -f compose.dev.yml up --build -d`. Both Compose configurations fail if `DOWNLOAD_BASE_URL` is empty; neither puts S3 credentials in the container.

## Download sources

The site does not query S3 or GitHub for releases. `DOWNLOAD_BASE_URL` is the complete directory containing downloadable files. With `/resources/`, a button links to `/resources/<filename>` and its checksum to `/resources/<filename>.sha256`. An absolute value such as `https://cdn.example.test/sys-admin-sym/resources/` works too. The value comes from runtime configuration, so it can change without rebuilding the Vite bundle.

Game and standalone client downloads use these filenames for Windows and Linux:

| Package | Windows | Linux |
| --- | --- | --- |
| Game | `cloud-provider-sim-x86_64-pc-windows-msvc.zip` | `cloud-provider-sim-x86_64-unknown-linux-gnu.zip` |
| Client | `game-ssh-x86_64-pc-windows-msvc.zip` | `game-ssh-x86_64-unknown-linux-gnu.zip` |

A matching `<filename>.sha256` becomes a checksum link. The game package includes the client; the separate client requires a running game. The site downloads the ZIP directly and does not send visitors to GitHub Actions or release pages.

If the runtime base URL is not configured, download buttons are disabled. Publishing packages is separate from building the website; the site does not upload or inspect game builds.

Set `DOWNLOAD_BASE_URL` in the container environment; the provided `.env.example` sets it to `/resources/`. The image refuses to start if it is absent and accepts either a same-origin path or an HTTPS URL. When using `/resources/`, configure the site's CDN or reverse proxy to serve that path from the public object storage resources. For a static host, configure its runtime `runtime-config.js` response instead. This value is public and must not contain credentials.

## GitHub Actions and S3

On pushes to `main`, the game workflow publishes Windows and Linux game ZIPs plus SHA-256 files; the game-ssh workflow publishes its standalone ZIPs and checksums. Tagged `game-ssh-v*` releases also refresh the latest client downloads. Both workflows retain their existing GitHub artifacts, and the client workflow continues publishing tagged GitHub releases. The website workflow checks types, unit tests, production build, and browser tests before publishing the container image to GHCR as `ghcr.io/<owner>/<repository>-website:latest` and `:<commit-sha>`. It runs for website changes, offers manual dispatch, and checks pull requests without publish credentials. Manual publication uses the `main` branch.

Create these repository Actions secrets before the next successful `main` build:

| Secret | Value |
| --- | --- |
| `S3_RW_ACCESS_KEY_ID` | Rotated write key ID |
| `S3_RW_SECRET_ACCESS_KEY` | Rotated write secret |
| `S3_BUCKET` | Bucket name |
| `S3_ENDPOINT_URL` | S3 API endpoint, using HTTPS |
| `S3_REGION` | S3 signing region |

The game workflows store immutable packages under `downloads/versions/<commit-sha>/` and publish the current ZIP and checksum files under `resources/`. The website image is published only to GHCR. The configured directory path must resolve those resource keys directly. Bucket/CDN read access must allow visitors to fetch ZIPs and `.sha256` files; a browser must never receive an S3 access key. Set the GHCR package visibility to public if visitors should pull the image without authentication.

The workflows reject non-HTTPS S3 endpoints. No S3 credentials are stored in this repository; rotate any keys previously pasted into chat before adding their replacements to GitHub Secrets. The provided read-only key is not embedded in the frontend; static browsers cannot safely hold private S3 credentials.

## Documentation and languages

`/en` and `/ru` are the localized home pages. `/en/docs` and `/ru/docs` contain the complete documentation reader: guide search, grouped navigation, heading links, commands, tables, original screenshots, and equipment sources. Switching language keeps the article and its heading anchor. Direct locale URLs take precedence over the saved preference; `/` uses that preference.

All 24 documents have English and Russian editions in `content/en` and `content/ru`. The original English image-generation prompts remain verbatim in the Russian source records for reproducibility, with translated titles and introductions. Markdown is loaded per article from the built site, sanitized with DOMPurify, and rendered locally. Relative guide/image links and GitHub documentation links stay on site. Manufacturer reference links retain their external sources. Reading guides does not require the GitHub API.

Refresh the English snapshots and documentation screenshots from the game repository:

```sh
bun run docs:sync
```

This copies `README.md`, `ARCHITECTURE.md`, `docs/*.md`, asset attribution and original prompt records. It rewrites player download instructions to the site's release downloads without changing root documents or Russian translations. After source changes, update the corresponding Russian edition and rebuild. Both editions are included in the document coverage tests. The snapshot preserves technical claims from the source guides, including older project notes.

## Structure

- `src/app`: application shell, locale routes, and design tokens.
- `src/pages`: home and documentation page composition.
- `src/features/showcase`: rack, gameplay, and terminal sections.
- `src/features/downloads`: validated release data, request state, and download controls.
- `src/features/docs`: guide catalog, Markdown rendering, search navigation, and heading contents.
- `src/shared`: language, configuration, HTTP behavior, and class utilities.
- `src/components/ui`: shadcn-vue primitives.
- `content`: bundled English/Russian documentation.
- `public/images` and `public/docs-assets`: attributed game imagery and original documentation screenshots.
- `tools/sync_docs.py`: local source synchronization.
- `tests`: browser interaction and responsive checks.

See [ATTRIBUTION.md](ATTRIBUTION.md) for asset sources, [docs/design.md](docs/design.md) for the design, and [docs/implementation-plan.md](docs/implementation-plan.md) for the implementation checklist.
