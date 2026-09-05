# Local browser player

The React/TypeScript client renders the Rust server's public observations and
complete canonical action catalog. It does not calculate game rules, legality,
outcomes, inventory, or entropy. Numeric API fields remain decimal strings;
portable saves are opaque UTF-8 text, never parsed by JavaScript.

## Play

From the repository root:

```bash
cargo run --locked -p forge-server
```

Open the exact printed loopback address. Pick a preset or use the six authored
creation selectors. The optional seed defaults to 71. Search covers the full
current catalog, including public destinations and outcome previews; display
pages do not impose a game-action cap.

Download a save before stopping the server. **Save and close** closes the
session first, then downloads its retained final save. If the download fails,
the closed session still offers **Download save**. Use **New character** after
closing to reach the save importer or creator. Saves only resume against the
same game build.

One server process owns one active game. Other tabs can observe it and stale
actions are rejected by Rust. The tab journal only stores versioned transport
recovery data, not the server capability or hidden game state. An uncertain
mutation disables other actions until its exact request is reconciled. A
server restart requires explicit recovery; an old pending request must not
create or change a game in a new server instance.

This is local play, not public hosting, cloud saves, multiplayer, or a browser
blind-player isolation claim. Clearing storage may lose a pending request;
the same server can recover its active game. Stopping the server loses every
in-memory session and acknowledgment ledger.

## Develop and verify

Node 22.22.1 and npm 10.9.4 are pinned for bundle verification. From this directory:

```bash
npm ci
npm run typecheck
npm test
npm run build
```

Then restart `cargo run --locked -p forge-server` from the repository root.
There is no Vite proxy or separate development API. Cargo embeds `dist` at
compile time; running the executable never reads browser files from disk.
Commit the generated `dist` files alongside source and lock changes.

From the repository root, `./tools/verify-browser.sh` installs the locked
dependencies, checks types and focused tests, tests bundle admission failures,
and requires a rebuild to reproduce every existing asset path and byte.
`./verify` includes that check before the Rust and game-evidence gates.

The Rust build rejects symlinks, unsafe paths, unsupported asset types, and
source maps. HTTP responses expose an independent `x-forge-ui-build`
fingerprint of exact embedded assets. It is not a game-build or replay receipt.
Player claims still need a downloaded save checked by `forge-verify`.
