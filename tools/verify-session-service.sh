#!/usr/bin/env bash
set -euo pipefail

REPO_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_DIR"

# Keep the exported player-safe artifact and independent result together.
# No model, HTTP listener, host path input, or hidden trace is involved.
mkdir -p "$REPO_DIR/artifacts/local/session-service"
SERVICE_SMOKE_DIR="$(mktemp -d "$REPO_DIR/artifacts/local/session-service/run.XXXXXX")"
cargo run --locked --quiet -p forge-server --example replay_smoke \
    > "$SERVICE_SMOKE_DIR/player.trace.json"
cargo run --locked --quiet -p forge-verify -- check-player \
    "$SERVICE_SMOKE_DIR/player.trace.json" > "$SERVICE_SMOKE_DIR/trusted-check.txt"
echo "session service export/replay: PASS (10 canonical actions, retry and resume)"
echo "evidence: $SERVICE_SMOKE_DIR"
