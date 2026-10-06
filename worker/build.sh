#!/usr/bin/env bash
# Build del Worker Rust/WASM. Lo usan wrangler ([build] en wrangler.toml),
# Cloudflare Workers Builds (Build command: bash build.sh) y GitHub Actions.
#
# La imagen de Workers Builds no lista Rust entre sus herramientas: si falta
# `cargo`, se instala rustup en modo minimal. El toolchain exacto lo fija
# ../rust-toolchain.toml (stable + wasm32-unknown-unknown); --panic-unwind
# usa ademas nightly (rustup la instala sola si falta) para recompilar std.
#
# worker-build --release (y cargo) buscan Cargo.toml en el directorio actual.
# Este script tiene invocadores con CWD distintos: worker/ (wrangler [build] y
# Workers Builds con Root directory = worker) y la raiz del repo (build.sh de
# la raiz delega aqui cuando Workers Builds corre con Root directory = raiz).
# Fijar el CWD al directorio del propio script hace que cualquier invocador
# funcione sin depender de donde este parado.
set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# GUARDA (2026-10-06, tras la falla real de Workers Builds "Could not detect
# a directory containing static files"): si este script termina corriendo sin
# su wrangler.toml al lado, alguien lo copio/movo o el cwd del [build] raiz
# esta mal. La raiz tiene un wrangler.toml ESPEJO (decision dueno 2026-10-06)
# que ejecuta este script con cwd = worker: nunca desde la raiz directamente.
# Fallar AQUI con mensaje claro es mejor que un deploy sin bindings.
if [ ! -f wrangler.toml ] || [ ! -f Cargo.toml ]; then
  echo "[build.sh] ERROR: falta wrangler.toml o Cargo.toml junto a build.sh." >&2
  echo "[build.sh] Este script DEBE ejecutarse desde worker/ (cwd del build)." >&2
  echo "[build.sh] build.sh solo vive en worker/; si esta copia en la raiz," >&2
  echo "[build.sh] reviertelo: el deploy raiz usa [build] cwd = worker." >&2
  exit 1
fi

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

# P1 (roadmap Cloudflare+Rust, Notion "Cloudflare + Rust - Links"): panic =
# unwind. El target wasm32-unknown-unknown compila con panic=abort por
# defecto: un panic en una request termina el isolate (RuntimeError) y
# envenena las requests hermanas. Con --panic-unwind (worker-build 0.8.7,
# publicada en crates.io): std se recompila con -Zbuild-std=std,panic_unwind
# y -Cpanic=unwind (nightly + rust-src + target se instalan solos si
# faltan), wasm-bindgen atrapa los panics en la frontera Rust->JS
# (excepciones PanicError) y registra schedule_reinit() para aborts duros
# (OOM, stack overflow): la request que paniquea falla, las siguientes
# siguen vivas y el DO se recrea de forma transparente. La seguridad de
# unwind la cubren las macros del crate worker (AssertUnwindSafe);
# este worker no usa Closure::new. Fuentes: README de cloudflare/workers-rs
# ("Panic Recovery with --panic-unwind") y blog de Cloudflare "Making Rust
# Workers reliable" (2026-10).
worker-build --release --panic-unwind
