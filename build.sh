#!/usr/bin/env bash
# Root build script wrapper for Cloudflare Workers Builds and local builds.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec bash "$SCRIPT_DIR/worker/build.sh" "$@"
