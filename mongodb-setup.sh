#!/usr/bin/env bash
# Crea DB auto_healing_agent, colección postmortems e índices.
# Uso: MONGODB_URI='mongodb+srv://...' bash mongodb-setup.sh
set -euo pipefail

if [[ -z "${MONGODB_URI:-}" ]]; then
  echo "Set MONGODB_URI first"
  exit 1
fi

# Requiere mongosh (Atlas shell) o node
if command -v mongosh &>/dev/null; then
  mongosh "$MONGODB_URI" --quiet --eval '
    db = db.getSiblingDB("auto_healing_agent");
    db.postmortems.createIndex({ createdAt: -1 });
    db.postmortems.createIndex({ "linearIssue.title": "text" });
    db.postmortems.createIndex({ source: 1 });
    print("OK indexes on auto_healing_agent.postmortems");
  '
else
  npx --yes mongodb-runner@latest 2>/dev/null || true
  node <<'NODE'
  const { MongoClient } = require("mongodb");
  (async () => {
    const c = new MongoClient(process.env.MONGODB_URI);
    await c.connect();
    const col = c.db("auto_healing_agent").collection("postmortems");
    await col.createIndex({ createdAt: -1 });
    await col.createIndex({ "linearIssue.title": "text" });
    await col.createIndex({ source: 1 });
    console.log("OK indexes on auto_healing_agent.postmortems");
    await c.close();
  })().catch((e) => { console.error(e); process.exit(1); });
NODE
fi
