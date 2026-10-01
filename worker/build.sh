#!/usr/bin/env bash
# Build del Worker Rust/WASM. Lo usan wrangler ([build] en wrangler.toml),
# Cloudflare Workers Builds y GitHub Actions.

set -euo pipefail

# Entra al directorio donde reside este script (worker/)
cd "$(dirname "${BASH_SOURCE[0]}")"

export PATH="$HOME/.cargo/bin:/usr/local/cargo/bin:$PATH"

if ! command -v cargo >/dev/null 2>&1 || ! command -v rustup >/dev/null 2>&1; then
  echo "[build.sh] cargo o rustup no encontrado: instalando rustup"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain 1.88.0
  # shellcheck disable=SC1091
  [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
fi

rustup target add wasm32-unknown-unknown

if ! command -v worker-build >/dev/null 2>&1; then
  cargo install --locked worker-build --version 0.1.1
fi

worker-build --release
