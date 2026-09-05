#!/usr/bin/env bash
set -euo pipefail

REPO_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_DIR"
mkdir -p "$REPO_DIR/artifacts/local/http-service"
HTTP_SMOKE_DIR="$(mktemp -d "$REPO_DIR/artifacts/local/http-service/run.XXXXXX")"
cargo build --locked --quiet -p forge-server --bin forge-server --example http_smoke
# Default timeout process-group signaling includes the spawned server. Do not
# use --foreground, which would leave descendants outside timeout signaling.
timeout --kill-after=2s 60s target/debug/examples/http_smoke "$REPO_DIR/target/debug/forge-server" \
    > "$HTTP_SMOKE_DIR/player.trace.json"
cargo run --locked --quiet -p forge-verify -- check-player \
    "$HTTP_SMOKE_DIR/player.trace.json" > "$HTTP_SMOKE_DIR/trusted-check.txt"
echo "loopback HTTP export/replay: PASS (10 canonical actions, dropped response, retry and resume)"
echo "evidence: $HTTP_SMOKE_DIR"
