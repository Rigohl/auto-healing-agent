#!/usr/bin/env bash
# Root build.sh wrapper for Cloudflare Workers Builds.
# Delegates execution to worker/build.sh.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/worker"
exec ./build.sh "$@"
