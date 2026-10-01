#!/usr/bin/env bash
# Build del Worker Rust/WASM. Lo usan wrangler ([build] en wrangler.toml),
# Cloudflare Workers Builds (Build command: bash worker/build.sh o bash build.sh) y GitHub Actions.

set -euo pipefail

# Garantiza que worker-build se ejecute siempre dentro del directorio worker/
cd "$(dirname "$0")"

export PATH="$HOME/.cargo/bin:$PATH"

if ! command -v cargo >/dev/null 2>&1; then
  echo "[build.sh] cargo no encontrado: instalando rustup (minimal)"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain 1.88.0
  # shellcheck disable=SC1091
  . "$HOME/.cargo/env"
fi

rustup target add wasm32-unknown-unknown

if ! command -v worker-build >/dev/null 2>&1; then
  cargo install --locked worker-build --version 0.1.1
fi

worker-build --release
