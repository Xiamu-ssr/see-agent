#!/usr/bin/env bash
# scripts/generate-api-types.sh — Generate TS types from OpenAPI spec
set -e
cd "$(dirname "$0")/.."

.venv/bin/python -c "
from see_agent.server.app import app
import json, pathlib
spec = app.openapi()
pathlib.Path('web/openapi.json').write_text(json.dumps(spec, indent=2))
"

cd web
mkdir -p src/types/generated
npx openapi-typescript openapi.json -o src/types/generated/api.d.ts
rm -f openapi.json
echo "Generated: web/src/types/generated/api.d.ts"
