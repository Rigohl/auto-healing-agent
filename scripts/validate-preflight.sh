#!/usr/bin/env bash
# Script de validación de la lógica de Preflight (.github/workflows/deploy.yml)
# Verifica que:
# 1. worker/wrangler.toml actual PASA preflight (comentarios con REPLACE_WITH son ignorados).
# 2. worker/wrangler.toml con placeholder ACTIVO FALLA preflight.
# 3. Comentarios inline con REPLACE_WITH no causan falsos positivos.

set -euo pipefail

run_preflight() {
  local target_file="$1"
  if sed -e "s/#.*//" "$target_file" | grep -q "REPLACE_WITH"; then
    return 1 # Falla preflight (placeholder activo detectado)
  else
    return 0 # Pasa preflight
  fi
}

echo "=== Testing Preflight Validation ==="

# Test 1: Archivo real worker/wrangler.toml
if run_preflight "worker/wrangler.toml"; then
  echo "PASS: Test 1 - worker/wrangler.toml actual pasa el preflight correctamente."
else
  echo "FAIL: Test 1 - worker/wrangler.toml actual fallo el preflight unexpectedly."
  exit 1
fi

# Test 2: Archivo temporal con placeholder activo
TMP_ACTIVE=$(mktemp)
trap 'rm -f "$TMP_ACTIVE" "$TMP_INLINE"' EXIT

cat worker/wrangler.toml > "$TMP_ACTIVE"
echo 'id = "REPLACE_WITH_KV_ID"' >> "$TMP_ACTIVE"

if ! run_preflight "$TMP_ACTIVE"; then
  echo "PASS: Test 2 - Placeholder activo "REPLACE_WITH_KV_ID" fue detectado y fallo el preflight."
else
  echo "FAIL: Test 2 - Placeholder activo NO fue detectado por preflight."
  exit 1
fi

# Test 3: Archivo temporal con comentario inline
TMP_INLINE=$(mktemp)
cat worker/wrangler.toml > "$TMP_INLINE"
echo 'id = "73014a1b32b7446397461a8d438c8ab2" # do not use REPLACE_WITH_KEY' >> "$TMP_INLINE"

if run_preflight "$TMP_INLINE"; then
  echo "PASS: Test 3 - Comentario inline con REPLACE_WITH pasa el preflight."
else
  echo "FAIL: Test 3 - Comentario inline con REPLACE_WITH fallo el preflight."
  exit 1
fi

echo "=== Todos los tests de validacion de Preflight PASARON ==="
