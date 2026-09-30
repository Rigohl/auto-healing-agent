#!/usr/bin/env bash
# LEGACY indexes for postmortems
set -euo pipefail
if [[ -z "${MONGODB_URI:-}" ]]; then
  echo "Set MONGODB_URI first"
  exit 1
fi
if command -v mongosh &>/dev/null; then
  mongosh "$MONGODB_URI" --quiet --eval '
    db = db.getSiblingDB("auto_healing_agent");
    db.postmortems.createIndex({ createdAt: -1 });
    db.postmortems.createIndex({ "linearIssue.title": "text" });
    db.postmortems.createIndex({ source: 1 });
    print("OK");
  '
else
  echo "Install mongosh or run indexes manually"
fi
