# Adventure Forge

Adventure Forge is an action-first, deeply reactive role-playing game in one persistent world. Its game rules are deterministic code. AI helps build, test, and improve the game but never decides authoritative outcomes during play.

The first playable arc is **The Split Tide**. A forged water order and a stolen Tide Key put two communities at risk. Who the player is, what they notice, whom they trust, and how they alter the Red Sluice will permanently change the connected world.

## Current state

Milestone 0 evidence passes, and the project is building out Milestone 1 from a playable CLI slice. It is not yet a public release and makes no claim of final world scale. See `PROJECT_STATE.md` for current evidence and `PLAN.md` for the complete product and delivery strategy.

## Play and verify

List the two current characters, start a game, or run a short deterministic demo:

```bash
cargo run -p forge-cli -- characters
cargo run -p forge-cli -- play --character ilyan
cargo run -p forge-cli -- demo --character rook --output rook.trace.json
```

During play, use a displayed number, `next`, `prev`, `all`, `find TEXT`, `save PATH`, `help`, or `quit`. A save contains public start inputs, chosen opaque action identities, and final commitments—not hidden world state. Replay and resume reconstruct every step through the authoritative kernel:

```bash
cargo run -p forge-cli -- replay rook.trace.json
cargo run -p forge-cli -- resume rook.trace.json
./verify
```

`./verify` is the non-AI acceptance gate. The browser interface will use the same kernel and action protocol after this CLI slice earns broader play evidence.

The checked Milestone 0 witnesses are independently regenerated and replayed in fresh processes by the same gate. They remain human-readable while exposing only public observations and opaque commitments. You can inspect them directly:

```bash
cargo run -p forge-verify -- scenarios
cargo run -p forge-verify -- crawl
cargo run -p forge-verify -- check evidence/witnesses/m0-ilyan.json
cargo run -p forge-verify -- check evidence/witnesses/m0-rook.json
```

The checked crawl starts from both authored characters. Within explicit depth, state, frontier, and action budgets, it reconstructs every paged catalog it visits, executes and observes every advertised canonical action, reaches all six current locations, and covers all 47 current definitions. The gate reproduces `evidence/crawls/split-tide.json` byte for byte in separate processes; this is bounded coverage, not a claim that every possible world state was exhausted.

## Core promises

- Plain words, fast turns, and meaningful actions.
- One world whose people and places remember consequences.
- A character whose identity, abilities, appearance, values, and history matter.
- No hidden menu cap on programmed legal actions.
- Replayable outcomes and mechanically checked evidence.

## Repository history

The four founding briefs are preserved in Git commit `594183b`. Their consolidated requirements and the active roadmap are in `PLAN.md`.
