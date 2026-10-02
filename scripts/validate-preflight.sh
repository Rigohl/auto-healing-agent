#!/usr/bin/env bash
# Guards de lo que CI y Cloudflare asumen del arbol y NO pueden asumir solos.
#
# Uso: bash scripts/validate-preflight.sh   (sin argumentos, sale != 0 si falla)
#
# Cubre tres invariantes que rompieron builds reales:
#   1. La logica de Preflight de deploy.yml: un REPLACE_WITH en un comentario
#      NO bloquea (falso positivo del P0 corregido en el PR #8), pero en una
#      linea activa SI. Si alguien "simplifica" el sed, el deploy vuelve a
#      caerse por un comentario.
#   2. worker/build.sh ejecutable: sin el bit +x, Workers Builds falla con un
#      error de permisos que no parece de permisos (PR #6, item 2).
#   3. El arbol real de wrangler: el symlink de raiz y worker/wrangler.toml
#      tienen que resolver al mismo archivo, o `wrangler deploy` desde raiz
#      desplegaria una configuracion distinta a la de deploy.yml.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

TMP_ACTIVE=""
TMP_INLINE=""
cleanup() { rm -f "$TMP_ACTIVE" "$TMP_INLINE"; }
trap cleanup EXIT

FAILED=0
pass() { echo "PASS: $1"; }
fail() {
  echo "FAIL: $1"
  FAILED=1
}

# Misma expresion que el paso Preflight de .github/workflows/deploy.yml y el
# job `placeholders` de security.yml. Si divergen, este script miente.
run_preflight() {
  local target_file="$1"
  if sed -e 's/#.*//' "$target_file" | grep -q "REPLACE_WITH"; then
    return 1
  fi
  return 0
}

echo "=== Preflight / placeholders ==="

# 1. La config real pasa.
if run_preflight worker/wrangler.toml; then
  pass "worker/wrangler.toml real pasa el preflight"
else
  fail "worker/wrangler.toml real tiene un REPLACE_WITH activo"
fi

# 2. Un symlink de raiz que se resuelve al mismo archivo tambien pasa.
if run_preflight wrangler.toml; then
  pass "wrangler.toml (raiz) pasa el preflight"
else
  fail "wrangler.toml (raiz) tiene un REPLACE_WITH activo"
fi

# 3. Un placeholder en una linea activa se detecta.
TMP_ACTIVE="$(mktemp)"
cat worker/wrangler.toml >"$TMP_ACTIVE"
echo 'id = "REPLACE_WITH_KV_ID"' >>"$TMP_ACTIVE"
if run_preflight "$TMP_ACTIVE"; then
  fail "un REPLACE_WITH en linea activa NO fue detectado"
else
  pass "un REPLACE_WITH en linea activa se detecta"
fi

# 4. Un placeholder dentro de un comentario no se detecta (falso positivo P0).
TMP_INLINE="$(mktemp)"
cat worker/wrangler.toml >"$TMP_INLINE"
echo 'id = "73014a1b32b7446397461a8d438c8ab2" # do not use REPLACE_WITH_KEY' >>"$TMP_INLINE"
if run_preflight "$TMP_INLINE"; then
  pass "un REPLACE_WITH en comentario NO bloquea"
else
  fail "un REPLACE_WITH en comentario bloquea: falso positivo del P0"
fi

echo "=== Permisos de build ==="

# 5. worker/build.sh tiene que ser ejecutable.
if [ -x worker/build.sh ]; then
  pass "worker/build.sh es ejecutable"
else
  fail "worker/build.sh no tiene bit +x (Workers Builds fallara)"
fi

# 6. El symlink de raiz tiene que apuntar al archivo real, no a si mismo.
if [ "$(readlink wrangler.toml)" = "worker/wrangler.toml" ]; then
  pass "wrangler.toml -> worker/wrangler.toml"
else
  fail "wrangler.toml deberia ser symlink a worker/wrangler.toml"
fi

if [ "$FAILED" -ne 0 ]; then
  echo "=== validate-preflight: FALLO ==="
  exit 1
fi
echo "=== validate-preflight: todo OK ==="