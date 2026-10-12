#!/bin/sh
set -eu

if [ -z "${DOWNLOAD_BASE_URL:-}" ]; then
  echo 'DOWNLOAD_BASE_URL must be set to the public download base path.' >&2
  exit 1
fi

if [ "$DOWNLOAD_BASE_URL" != "/" ]; then
  DOWNLOAD_BASE_URL=${DOWNLOAD_BASE_URL%/}
fi
case "$DOWNLOAD_BASE_URL" in
  https://*)
    if ! printf '%s' "$DOWNLOAD_BASE_URL" | grep -Eq '^https://[A-Za-z0-9.-]+(:[0-9]+)?(/[A-Za-z0-9._~%-]+)*$'; then
      echo 'DOWNLOAD_BASE_URL must be an HTTPS URL without credentials, query, or fragment.' >&2
      exit 1
    fi
    ;;
  /*)
    if [ "${DOWNLOAD_BASE_URL#//}" != "$DOWNLOAD_BASE_URL" ] || ! printf '%s' "$DOWNLOAD_BASE_URL" | grep -Eq '^/[A-Za-z0-9._~/%-]*$'; then
      echo 'DOWNLOAD_BASE_URL must be a same-origin path without query or fragment.' >&2
      exit 1
    fi
    ;;
  *)
    echo 'DOWNLOAD_BASE_URL must be an HTTPS URL or a same-origin path.' >&2
    exit 1
    ;;
esac

printf 'window.__APP_CONFIG__ = { downloadBaseUrl: "%s" };\n' "$DOWNLOAD_BASE_URL" > /tmp/runtime-config.js
