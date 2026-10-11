#!/bin/sh
set -eu

if [ -z "${DOWNLOAD_BASE_URL:-}" ]; then
  echo 'DOWNLOAD_BASE_URL must be set to the public download base path.' >&2
  exit 1
fi

DOWNLOAD_BASE_URL=${DOWNLOAD_BASE_URL%/}
if ! printf '%s' "$DOWNLOAD_BASE_URL" | grep -Eq '^https://[A-Za-z0-9.-]+(:[0-9]+)?(/[A-Za-z0-9._~%-]+)*$'; then
  echo 'DOWNLOAD_BASE_URL must be an HTTPS base path without credentials, query, or fragment.' >&2
  exit 1
fi

printf 'window.__APP_CONFIG__ = { downloadBaseUrl: "%s" };\n' "$DOWNLOAD_BASE_URL" > /tmp/runtime-config.js
