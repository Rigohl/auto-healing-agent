#!/usr/bin/env bash
# Envoltorio para Workers Builds con Root directory = raiz del repo.
# worker-build y wrangler esperan correr dentro de worker/.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${ROOT_DIR}/worker"
exec bash ./build.sh
