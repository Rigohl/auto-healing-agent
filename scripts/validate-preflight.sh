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
#   3. El arbol real de wrangler: la raiz (symlink a worker/wrangler.toml,
#      o archivo solo-comentarios desde 2026-10-03) y worker/wrangler.toml
#      nunca representan configuraciones distintas.
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

# 6. El arbol de wrangler: dos formas validas para la raiz, ambas fail-
#    closed frente a una config divergente (2026-10-03: la API usada para
#    editar el repo no puede recrear symlinks; lo que NUNCA se acepta es
#    una config activa distinta de la de worker/wrangler.toml).
if [ -L wrangler.toml ]; then
  if [ "$(readlink wrangler.toml)" = "worker/wrangler.toml" ]; then
    pass "wrangler.toml -> worker/wrangler.toml (symlink)"
  else
    fail "wrangler.toml (symlink) apunta a otra cosa"
  fi
elif [ -f wrangler.toml ]; then
  # Dos formas validas para la raiz (decision del dueno 2026-10-05, commit
  # 37c3fd5a: Root directory = / en Workers Builds):
  #   (a) solo comentarios: nada desplegable (fail-closed), o
  #   (b) config "puntero": SOLO name/main/compatibility_date/[build], con
  #       main apuntando al arbol de worker/ (worker/build/...) y command
  #       ejecutando build.sh. Cualquier OTRA config activa (bindings, colas,
  #       DO, vars) divergiria de worker/wrangler.toml y debe fallar.
  ACTIVE="$(sed -e 's/#.*//' wrangler.toml | tr -d '[:space:]')"
  if [ -z "$ACTIVE" ]; then
    pass "wrangler.toml raiz solo comentarios: nada desplegable (fail-closed)"
  else
    MAIN_OK="$(grep -E '^[[:space:]]*main[[:space:]]*=' wrangler.toml | grep -c 'worker/build/')"
    BUILD_OK="$(grep -E '^[[:space:]]*command[[:space:]]*=' wrangler.toml | grep -c 'build.sh')"
    EXTRA_KEYS="$(sed -e 's/#.*//' wrangler.toml | grep -E '^[[:space:]]*[a-zA-Z_]' | grep -vE '^[[:space:]]*(name|main|compatibility_date|command)[[:space:]]*=' | wc -l)"
    if [ "$MAIN_OK" -ge 1 ] && [ "$BUILD_OK" -ge 1 ] && [ "$EXTRA_KEYS" -eq 0 ]; then
      pass "wrangler.toml raiz = config puntero a worker/ (Root directory = /, commit 37c3fd5a)"
    else
      fail "wrangler.toml regular diverge de worker/wrangler.toml (main no apunta a worker/build/, build sin build.sh, o keys activos de mas)"
    fi
  fi
else
  fail "wrangler.toml no existe"
fi

if [ "$FAILED" -ne 0 ]; then
  echo "=== validate-preflight: FALLO ==="
  exit 1
fi
echo "=== validate-preflight: todo OK ==="