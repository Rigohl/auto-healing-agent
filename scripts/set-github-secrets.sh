#!/usr/bin/env bash
# set-github-secrets.sh — usa la API de GitHub (path oficial de Actions secrets)
# Requisitos: curl, python3, PyNaCl (pip install pynacl)
#
# Uso:
#   export GH_TOKEN='ghp_...'   # PAT con scope repo o fine-grained: Secrets write
#   export MONGODB_URI='...'
#   export HF_TOKEN='...'
#   # ... resto de env
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
KEY_ID=$(python3 -c 'import json,sys; print(json.load(sys.stdin)["key_id"])' <<<<"$KEY_JSON")
PUB_KEY=$(python3 -c 'import json,sys; print(json.load(sys.stdin)["key"])' <<<<"$KEY_JSON")

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
put_secret MONGODB_URI "${MONGODB_URI:-}"
put_secret HF_TOKEN "${HF_TOKEN:-}"
put_secret HF_MODEL "${HF_MODEL:-Qwen/Qwen2.5-Coder-7B-Instruct}"
put_secret HF_BASE_URL "${HF_BASE_URL:-https://router.huggingface.co/v1}"
put_secret EXA_API_KEY "${EXA_API_KEY:-}"
put_secret LINEAR_API_KEY "${LINEAR_API_KEY:-}"
put_secret VERCEL_TOKEN "${VERCEL_TOKEN:-}"
put_secret VERCEL_ORG_ID "${VERCEL_ORG_ID:-team_9ILQS3IIe8K4jzv23DSrID4U}"
put_secret VERCEL_PROJECT_ID "${VERCEL_PROJECT_ID:-prj_GIuo1hR7R6HKmvSAki93tol4pJrs}"
put_secret GH_PAT "${GH_PAT:-}"

echo ">> Listo. Revisa: https://github.com/$OWNER/$REPO/settings/secrets/actions"
