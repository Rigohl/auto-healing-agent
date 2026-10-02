#!/usr/bin/env bash
# Build del Worker Rust/WASM. Lo usan wrangler ([build] en wrangler.toml),
# Cloudflare Workers Builds (Build command: bash build.sh) y GitHub Actions.
#
# La imagen de Workers Builds no lista Rust entre sus herramientas: si falta
# `cargo`, se instala rustup en modo minimal. El toolchain exacto lo fija
# ../rust-toolchain.toml (stable + wasm32-unknown-unknown).
#
# worker-build --release (y cargo) buscan Cargo.toml en el directorio actual.
# Este script tiene invocadores con CWD distintos: worker/ (wrangler [build] y
# Workers Builds con Root directory = worker) y la raiz del repo (build.sh de
# la raiz delega aqui cuando Workers Builds corre con Root directory = raiz).
# Fijar el CWD al directorio del propio script hace que cualquier invocador
# funcione sin depender de donde este parado.
set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

export PATH="$HOME/.cargo/bin:$PATH"

if ! command -v cargo >/dev/null 2>&1; then
  echo "[build.sh] cargo no encontrado: instalando rustup (minimal)"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal --default-toolchain stable
  # shellcheck disable=SC1091
  . "$HOME/.cargo/env"
fi

rustup target add wasm32-unknown-unknown

if ! command -v worker-build >/dev/null 2>&1; then
  cargo install -q worker-build
fi

worker-build --release
