# Adventure Forge

Adventure Forge is an action-first, deeply reactive role-playing game in one persistent world. Its game rules are deterministic code. AI helps build, test, and improve the game but never decides authoritative outcomes during play.

The first playable arc is **The Split Tide**. A forged water order and a stolen Tide Key put two communities at risk. Who the player is, what they notice, whom they trust, and how they alter the Red Sluice will permanently change the connected world.

## Current state

The project is in Milestone 0 with a playable CLI slice. It is not yet a public release and makes no claim of final world scale. See `PROJECT_STATE.md` for current evidence and `PLAN.md` for the complete product and delivery strategy.

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

## Core promises

- Plain words, fast turns, and meaningful actions.
- One world whose people and places remember consequences.
- A character whose identity, abilities, appearance, values, and history matter.
- No hidden menu cap on programmed legal actions.
- Replayable outcomes and mechanically checked evidence.

## Repository history

The four founding briefs are preserved in Git commit `594183b`. Their consolidated requirements and the active roadmap are in `PLAN.md`.
