#!/usr/bin/env bash
set -euo pipefail

REPO_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
BROWSER_DIR="$REPO_DIR/browser"

cd -- "$BROWSER_DIR"
if [[ "$(node --version)" != "v$(<.node-version)" ]]; then
  echo "Browser verification requires the Node version in browser/.node-version." >&2
  exit 1
fi
if [[ "npm@$(npm --version)" != "$(node -p "require('./package.json').packageManager")" ]]; then
  echo "Browser verification requires the npm version in browser/package.json." >&2
  exit 1
fi
npm ci
npm run typecheck
npm test
node --test scripts/check-build.test.mjs
node scripts/check-build.mjs
