# Guía de los 9 Secrets de GitHub Actions

Configúralos en: Settings → Secrets and variables → Actions → New repository secret.

## 1. MONGODB_URI
URI de conexión a MongoDB Atlas (M0 free).
Ejemplo: `mongodb+srv://user:pass@cluster0.xxx.mongodb.net/auto_healing_agent?retryWrites=true&w=majority`

## 2. HF_TOKEN
Token de Hugging Face con permisos de Inference Providers.
Obtener en: https://huggingface.co/settings/tokens

## 3. HF_MODEL
Modelo a usar. Recomendado: `Qwen/Qwen2.5-Coder-7B-Instruct`

## 4. HF_BASE_URL
Endpoint OpenAI-compatible. Default: `https://router.huggingface.co/v1`

## 5. EXA_API_KEY
API key de Exa (free tier con $10 créditos/mes).
Obtener en: https://dashboard.exa.ai/api-keys

## 6. LINEAR_API_KEY
Personal API Key de Linear.
Obtener en: Linear → Settings → Security & access → Personal API keys

## 7. VERCEL_TOKEN
API Access Token de Vercel.
Obtener en: Vercel → Account → Tokens

## 8. VERCEL_ORG_ID
ID de la organización/team de Vercel (empieza con `team_`).
Está en `.vercel/project.json` o en el dashboard.

## 9. VERCEL_PROJECT_ID
ID del proyecto de Vercel (empieza con `prj_`).
Está en `.vercel/project.json`.

## Opcionales recomendados
- `NOTION_TOKEN`: Integration token de Notion (si usas Notion MCP además de MongoDB).
- `CLOUDFLARE_API_TOKEN`: Para desplegar el Worker.
- `GH_PAT`: Personal Access Token con scope `repo` (solo si el Worker dispara `repository_dispatch` a otro repo).
