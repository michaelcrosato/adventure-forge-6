# Adventure Forge

Adventure Forge is an action-first, deeply reactive role-playing game in one persistent world. Its game rules are deterministic code. AI helps build, test, and improve the game but never decides authoritative outcomes during play.

The first playable arc is **The Split Tide**. A forged water order and a stolen Tide Key put two communities at risk. Who the player is, what they notice, whom they trust, and how they alter the Red Sluice will permanently change the connected world.

## Current state

Milestone 0 evidence passes, and the project is building out Milestone 1 from a playable CLI slice with authoritative character creation. It is not yet a public release and makes no claim of final world scale. See `PROJECT_STATE.md` for current evidence and `PLAN.md` for the complete product and delivery strategy.

## Play and verify

Create a character, select either proof preset, or run a short deterministic demo:

```bash
cargo run -p forge-cli -- characters
cargo run -p forge-cli -- create
cargo run -p forge-cli -- play --character ilyan
cargo run -p forge-cli -- demo --character rook --output rook.trace.json
```

The creator offers six authored two-way axes—lineage, origin, calling, value, burden, and history—for 64 validated combinations. The CLI submits only the chosen public IDs; the kernel derives the complete character and binds its canonical recipe into state and replay provenance. During play, use a displayed number, `next`, `prev`, `all`, `find TEXT`, `save PATH`, `help`, or `quit`. The player view shows the authoritative tide step, the remaining time before the Lowsail surge, and the exact or bounded tide-step cost of every action. Outcome and ending rows also preview their consequence before selection. Completed discoveries leave the catalog instead of inviting consequence-free repetition. Once the surge resolves, obsolete actions close and `Return to Lowsail` leads to the persistent result and its distinct ending. That route remains available for later visits. A save contains public start inputs, chosen opaque action identities, and final commitments—not hidden world state. Replay and resume reconstruct every step through the authoritative kernel:

```bash
cargo run -p forge-cli -- replay rook.trace.json
cargo run -p forge-cli -- resume rook.trace.json
cargo run -p forge-verify -- check-player rook.trace.json
./verify
```

Catalog search includes the visible consequence previews. NPC information also has a delivery path: ringing Lowsail's warning teaches Oren, and relaying it from the levee teaches Edrik and unlocks his relief-channel help. Discovering a distant event does not automatically make an NPC know it.

The Tide Key is a persistent item: taking it removes it from Yara's stock and opens `Calibrate Gate` at Red Sluice Floor. That provides another preparation route to Split Flow. Save/resume reconstructs the same ownership and calibration through canonical actions. Saves remain tied to their exact game build; this schema/rules change does not silently migrate older saves.

`Climb Hot Face` carries a qualified climber directly to Red Sluice Top in one tide step. It also exposes the destructive overload controls; the climb itself does not choose that outcome.

The first return also brings Oren, Sava, and Mira into the changed market. Their positions persist across revisits without resetting their memories, knowledge, relationships, or stock. Return conversations require actual NPC presence; Oren describes the chosen water route and distinguishes opening the channel from launching the free ferry.

`Rig Towline (3 coin)` uses rope and wire to hire a one-step tow from the docks to the levee, revealing the Culvert Path. The fee is spent, but the gear is returned. Asking Oren and then walking remains a free two-step alternative from the same docks.

Normal observations and creation previews show your own resources and gear with readable names and exact quantities. The kernel derives this read-only supply line; payments and item transfers update it through canonical actions. NPC stock and hidden world records remain private, and replay binds the same public readout.

`forge-verify check-player` is the independent trusted checker for a player-safe save. `./verify` is the non-AI acceptance gate. The browser interface will use the same kernel and action protocol after this CLI slice earns broader play evidence.

The gate also builds reviewed bad-change mutants in a disposable source copy. A selector-free control must pass before ambient action ordering, page ordering, crawler scheduling, and process-identity mutations are each required to fail the fresh-process crawl contract. No mutant hook is compiled into the production crates.

On Linux, the same gate builds a stripped release-only player bundle and runs a locked CLI boundary rehearsal with Bubblewrap. The player process receives only its executable, required runtime libraries, and one writable save directory; the repository and all other host paths remain unmounted. The test clears its environment, isolates its network, enforces resource limits, exercises canary reads and writes, compares two deterministic sessions, and verifies the resulting save outside the sandbox. Its local report is written under `artifacts/local/locked-player-boundary/`.

This scripted rehearsal is process-isolation evidence, not an actual blind-AI playtest, a model prompt-injection test, or proof against binary reverse engineering. Those claims remain separate milestone requirements.

The live model runner defaults to `gpt-5.6-luna` at maximum reasoning. It uses the saved Codex ChatGPT subscription login and has no API-key fallback:

```bash
./tools/live-blind-llm-playtest.sh --auth-check
./tools/live-blind-llm-playtest.sh
```

An accepted run requires clean `main` equal to `origin/main`, compiles the latest release game and independent checker, and starts a fresh ephemeral Codex session outside the repository. Codex runs as UID/GID 65534 in a minimal Bubblewrap filesystem with inference network access, an isolated copy of the saved login, and only the `observe`, `act`, and `finish` player calls exposed through its bundled Code Mode orchestration host. Shell, filesystem, browser, plugin, and other development tools stay disabled. Noninteractive MCP approval is bypassed only inside that external OS sandbox. Its fixed MCP child runs with no capabilities and `no_new_privs`; Landlock limits that game process to the read-only game bundle and writable session, while a seccomp filter denies networking. Startup probes require the game process to fail both a saved-login read and socket creation. Both `OPENAI_API_KEY` and `CODEX_API_KEY` are removed from status checks and the live process. Missing or non-ChatGPT saved authentication fails closed.

The globally installed Vercel plugin remains available to the manager but is disabled and absent from the blind player's isolated Codex home. Successful local reports are written under `artifacts/local/live-blind-llm/`. They preserve the model-authored findings, public transcript, player-safe trace, Codex events, prompt-context audit, tool allowlist, hashes, timing, token usage, and independent replay result. Codex currently injects generic host skill and developer context even with callable development features disabled, so these reports claim source-isolated model play—not a strict proof that every model-visible token came from the game interface.

All checked witnesses are independently regenerated and replayed in fresh processes by the same gate. They remain human-readable while exposing only public observations and opaque commitments. The registry contains two Milestone 0 preset paths, two hybrid creator paths, the missed-surge deadline through its disaster ending, all five chosen Split Tide outcomes, one representative path through each authored area, a matched warning-delivery pair, the Tide Key's ownership-to-Split-Flow path, and the paid towline-to-relief route:

```bash
cargo run -p forge-verify -- scenarios
cargo run -p forge-verify -- crawl
cargo run -p forge-verify -- scale
cargo run -p forge-verify -- check evidence/witnesses/m0-ilyan.json
cargo run -p forge-verify -- check evidence/witnesses/m1-custom-cross-current.json
cargo run -p forge-verify -- check evidence/witnesses/m1-deadline-missed-surge.json
cargo run -p forge-verify -- check evidence/witnesses/m1-outcome-split-flow.json
cargo run -p forge-verify -- check evidence/witnesses/m1-area-red-sluice.json
cargo run -p forge-verify -- check evidence/witnesses/m1-tide-key-split-flow.json
cargo run -p forge-verify -- check evidence/witnesses/m1-paid-towline-relief.json
cargo run -p forge-verify -- check-scale evidence/scale/synthetic-ring-500.json
```

Each scenario has an opaque binding over its ID, canonical authored start, seed, complete parameter maps, ordered action recipe, and semantic expectations. The trusted checker rejects relabeling and alternate valid paths, then verifies required persistent consequences without serializing hidden state. JSON inputs are preflighted for duplicate object keys before typed decoding. Area witnesses are representative contract evidence, not a claim of final world depth.

The checked crawl starts from both authored characters. Within explicit depth, state, frontier, and action budgets, it reconstructs every paged catalog it visits, executes and observes every advertised canonical action, reaches all six current locations, and covers all 51 current definitions. The gate reproduces `evidence/crawls/split-tide.json` byte for byte in separate processes; this is bounded coverage, not a claim that every possible world state was exhausted.

The separate scale fixture compiles exactly 500 terminal synthetic locations in a reciprocal ring, admits their exact runtime ID set, independently proves graph reachability, and completes 500 kernel-enumerated travel transitions back to its start. It reconstructs every page of every travel-state catalog under fixed graph-frontier, catalog-work, and serialization ceilings. Fresh processes reproduce and recheck `evidence/scale/synthetic-ring-500.json` byte for byte. This proves engine and compiler capacity only; the generated nodes are not game content and do not count as authored breadth, area depth, or Skyrim parity.

## Core promises

- Plain words, fast turns, and meaningful actions.
- One world whose people and places remember consequences.
- A character whose identity, abilities, appearance, values, and history matter.
- No hidden menu cap on programmed legal actions.
- Replayable outcomes and mechanically checked evidence.

## Repository history

The four founding briefs are preserved in Git commit `594183b`. Their consolidated requirements and the active roadmap are in `PLAN.md`.
