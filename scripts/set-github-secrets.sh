#!/usr/bin/env bash
# set-github-secrets.sh — usa la API de GitHub (path oficial de Actions secrets)
# Requisitos: curl, python3, PyNaCl (pip install pynacl)
#
# Uso:
#   export GH_TOKEN='ghp_...'   # PAT con scope repo o fine-grained: Secrets write
#   export MONGODB_URI='...'
#   export VERCEL_ORG_ID='...'
#   # ... resto de env. Sin defaults: lo que no exportes no se sube.
#   bash scripts/set-github-secrets.sh

set -euo pipefail

OWNER="${GITHUB_OWNER:-Rigohl}"
REPO="${GITHUB_REPO:-auto-healing-agent}"
API="https://api.github.com"

if [[ -z "${GH_TOKEN:-}" ]]; then
  echo "ERROR: export GH_TOKEN=tu_pat (necesita permiso de Actions secrets)"
  exit 1
fi

auth=(-H "Authorization: Bearer $GH_TOKEN" -H "Accept: application/vnd.github+json" -H "X-GitHub-Api-Version: 2022-11-28")

echo ">> Public key de $OWNER/$REPO"
KEY_JSON=$(curl -sS "${auth[@]}" "$API/repos/$OWNER/$REPO/actions/secrets/public-key")
KEY_ID=$(python3 -c 'import json,sys; print(json.load(sys.stdin)["key_id"])' <<< "$KEY_JSON")
PUB_KEY=$(python3 -c 'import json,sys; print(json.load(sys.stdin)["key"])' <<< "$KEY_JSON")

encrypt() {
  local plain="$1"
  python3 - "$PUB_KEY" "$plain" <<'PY'
import sys, base64
from nacl import encoding, public

public_key = sys.argv[1]
secret_value = sys.argv[2]
pk = public.PublicKey(public_key.encode("utf-8"), encoding.Base64Encoder())
sealed = public.SealedBox(pk).encrypt(secret_value.encode("utf-8"))
print(base64.b64encode(sealed).decode("utf-8"))
PY
}

put_secret() {
  local name="$1" value="$2"
  if [[ -z "$value" ]]; then
    echo "  skip $name (vacío)"
    return 0
  fi
  local enc
  enc=$(encrypt "$value")
  local code
  code=$(curl -sS -o /tmp/gh-secret-out -w "%{http_code}" -X PUT "${auth[@]}" \
    -H "Content-Type: application/json" \
    "$API/repos/$OWNER/$REPO/actions/secrets/$name" \
    -d "{\"encrypted_value\":\"$enc\",\"key_id\":\"$KEY_ID\"}")
  if [[ "$code" == "201" || "$code" == "204" ]]; then
    echo "  OK $name ($code)"
  else
    echo "  FAIL $name ($code) $(cat /tmp/gh-secret-out)"
  fi
}

echo ">> Subiendo secrets (solo los que tengas en env)"
# Alineado 2026-10-09 con los secrets que ESTE repo consume de verdad (grep
# de secrets.* en .github/workflows/): CLOUDFLARE_ACCOUNT_ID y
# CLOUDFLARE_API_TOKEN (deploy.yml, deploy-staging.yml), GITHUB_TOKEN y
# WEBHOOK_SECRET (deploy.yml smoke test). Se ELIMINAN los que venian de
# otro proyecto y ningun workflow usa: MONGODB_URI, EXA_API_KEY,
# LINEAR_API_KEY, VERCEL_TOKEN, VERCEL_ORG_ID, VERCEL_PROJECT_ID, GH_PAT
# (la ruta HuggingFace V0 ya fue eliminada el 2026-10-05, ver
# docs/LLM_POLICY.md y DISCREPANCIES.md items 3 y 13).
# Sin valores por defecto: un id embebido se subiria aunque nadie lo
# exportara. Se sube solo lo que el operador define explicitamente.
put_secret CLOUDFLARE_API_TOKEN "${CLOUDFLARE_API_TOKEN:-}"
put_secret CLOUDFLARE_ACCOUNT_ID "${CLOUDFLARE_ACCOUNT_ID:-}"
put_secret GITHUB_TOKEN "${GITHUB_TOKEN:-}"
put_secret WEBHOOK_SECRET "${WEBHOOK_SECRET:-}"

echo ">> Listo. Revisa: https://github.com/$OWNER/$REPO/settings/secrets/actions"
