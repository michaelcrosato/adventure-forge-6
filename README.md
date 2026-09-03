# Adventure Forge

Adventure Forge is an action-first, deeply reactive role-playing game in one persistent world. Its game rules are deterministic code. AI helps build, test, and improve the game but never decides authoritative outcomes during play.

The first playable arc is **The Split Tide**. A forged water order and a stolen Tide Key put two communities at risk. Who the player is, what they notice, whom they trust, and how they alter the Red Sluice will permanently change the connected world.

## Current state

The project is in Milestone 0: honest kernel foundation. It is not yet a playable release and makes no claim of final world scale. See `PROJECT_STATE.md` for current evidence and `PLAN.md` for the complete product and delivery strategy.

## Intended commands

As the foundation lands, these stable entry points will be maintained:

```bash
./verify
cargo run -p forge-cli -- play
cargo run -p forge-cli -- replay <trace>
```

`./verify` is the non-AI acceptance gate. The browser interface will use the same kernel and action protocol after the CLI slice is proven.

## Core promises

- Plain words, fast turns, and meaningful actions.
- One world whose people and places remember consequences.
- A character whose identity, abilities, appearance, values, and history matter.
- No hidden menu cap on programmed legal actions.
- Replayable outcomes and mechanically checked evidence.

## Repository history

The four founding briefs are preserved in Git commit `594183b`. Their consolidated requirements and the active roadmap are in `PLAN.md`.

