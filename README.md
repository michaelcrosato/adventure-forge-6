# Adventure Forge

Adventure Forge is an action-first, deeply reactive role-playing game in one persistent world. Its game rules are deterministic code. AI helps build, test, and improve the game but never decides authoritative outcomes during play.

The first playable arc is **The Split Tide**. A forged water order and a stolen Tide Key put two communities at risk. Who the player is, what they notice, whom they trust, and how they alter the Red Sluice will permanently change the connected world.

## Current state

Milestone 0 evidence passes, and the project is building out Milestone 1 from a playable local browser and CLI slice with authoritative character creation. It is not yet a public release and makes no claim of final world scale. See `PROJECT_STATE.md` for current evidence and `PLAN.md` for the complete product and delivery strategy.

## Play and verify

For local browser play, run:

```bash
cargo run --locked -p forge-server
```

Open the printed `http://127.0.0.1:38123` address. Choose a preset or create a character, then select the displayed actions. Search includes all current choices, destinations, and consequence previews. Use **Download save** during play or **Save and close** before stopping the server. **Resume a save** imports the exported file without rewriting its numbers. A tab reload can recover the active game while the same server is running; stopping the server discards its in-memory sessions. Saves require the exact game build.

The browser bundle is embedded in the Rust executable, so playing from a checkout needs no Node installation. This remains a single-user local game: do not expose or reverse-proxy the server. Browser development and verification instructions are in [browser/README.md](browser/README.md).

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

Test builds use optimization level 1 with debug assertions and integer overflow checks enabled. For diagnosis with unoptimized test builds through the same complete gate, run:

```bash
CARGO_PROFILE_TEST_OPT_LEVEL=0 \
CARGO_PROFILE_TEST_DEBUG_ASSERTIONS=true \
CARGO_PROFILE_TEST_OVERFLOW_CHECKS=true \
./verify
```

The measured profile comparison and its build, memory, and disk costs are recorded in `PROJECT_STATE.md`.

Catalog search includes the visible consequence previews. NPC information also has a delivery path: ringing Lowsail's warning teaches Oren, and relaying it from the levee teaches Edrik and unlocks his relief-channel help. Discovering a distant event does not automatically make an NPC know it.

The Tide Key is a persistent item: taking it removes it from Yara's stock and opens `Calibrate Gate` at Red Sluice Floor. That provides another preparation route to Split Flow. Save/resume reconstructs the same ownership and calibration through canonical actions. Saves remain tied to their exact game build, with no older-save migration.

`Climb Hot Face` carries a qualified climber directly to Red Sluice Top in one tide step. It also exposes the destructive overload controls; the climb itself does not choose that outcome.

`Open Old Channel` redirects the surge and opens the ferry route. Then use `Return to Lowsail` and `Abolish Ferry Toll` to launch the free ferry. Opening the channel does not complete that ending by itself.

The first return also brings Oren, Sava, and Mira into the changed market. Their positions persist across revisits without resetting their memories, knowledge, relationships, or stock. Return conversations require actual NPC presence; Oren describes the chosen water route and distinguishes opening the channel from launching the free ferry.

`Rig Towline (3 coin)` uses rope and wire to hire a one-step tow from the docks to the levee, revealing the Culvert Path. The fee is spent, but the gear is returned. Asking Oren and then walking remains a free two-step alternative from the same docks.

Normal observations and creation previews show your own resources and gear with readable names and exact quantities. The kernel derives this read-only supply line; payments and item transfers update it through canonical actions. NPC stock and hidden world records remain private, and replay binds the same public readout.

The optional **Fume Yards Workshop** connects to the levee and the returned market. Nessa has enough clay and mesh for one product: repair plugs unlock Oren's three-coin sorting job, while a fitted catch screen saves two stamina on her two-coin freight job. Crafting and installation consume the actual materials. Stock, completed jobs, the fitted screen, and the repaired stand persist on return. Workshop work uses the same tide steps as the main arc, and it remains available after the tide or a late first visit. The complete Fume Yards district is still being built.

`forge-verify check-player` is the independent trusted checker for a player-safe save. `./verify` is the non-AI acceptance gate. The [local session service](crates/forge-server/README.md) supplies the browser's loopback-only HTTP API with canonical action submission, retry-safe acknowledgments, and player-safe save/resume. The browser owns no game rules.

The optional Fume Yards workshop leads to Kiln Bay. Its finite clay and mesh can become cold repair goods, a catch screen, or a timed fired filter. Ignition starts readiness and spoilage on the shared world clock; leaving or saving keeps that clock's history. Drawing a filter enables local protection or a four-coin sale. Banking preserves the three-coin freight commission; abandonment spoils both batch and freight. Ash Beds adds Daro's trapped filter: brace it for two stamina, use combined heat sense and lock-runner handling through the rear hatch, or risk a disclosed 25% break chance. A salvaged filter can protect the one available firing without spending Pera's cask. Cold loading pays three coins but closes the unfired kiln. After personal recovery, reporting cleared access requires Daro to accompany you to Brann. A separate collateral cage holds one filter: pay Daro four coins, or read the docket and settle with the actual fuel lot as a ledger clerk. Spending that fuel closes later firing. At a repaired market stand, Oren can order a water installation. Carry a filter and escort Pera with her cask, then install both for one two-stamina ration. The same filter still competes with local protection and sale. Stock, payment, delivery, and the used ration persist.

A saved-worker history also lets you tell Brann the rescue account. After he witnesses your dust-filter installation, ask him to help at the rack. His lift takes three tide steps, spends no stamina, and pays the single access commission onsite. He leaves kiln supervision until you physically return him; you can cancel or finish the rack yourself. Ordinary recovery remains available to every character. Ash containment, dirty freight, remaining cast routines and character methods, and the complete district contract remain unfinished.

Nessa can test your actual unfired charge at Workshop. Bring her to Brann so he hears her dust finding in person. His three-step cold shift reclaims the charge into plugs and pays the same three-coin freight wage without spending stamina. It permanently closes firing, dust installation and the staffed rack lift. Nessa and Brann return to Workshop; bring Brann back for any unpaid rack report. You can instead return Nessa and keep the charge, or do the old personal work. Her finding concerns dust from that charge; it does not certify a fired filter or contain the district's ash.

The gate also builds reviewed bad-change mutants in a disposable source copy. Seventeen selector-free controls must pass before twenty defects are activated separately. Action ordering, page ordering, crawler scheduling, and process-identity mutations must fail the fresh-process crawl contract. Dedicated tests detect stale-action acceptance, private NPC-stock leakage, reordered hash inputs, repeated entropy draws, lost remote NPC memory, overlong location sentences, an omitted rules source, unconsumed recipe ingredients, duplicated products, absolute instead of relative batch deadlines, remote timer pauses, inverted salvage chance outcomes, an incorrect collateral price, retained fuel after settlement, omitted Brann movement during staffing, and omitted Nessa movement during dust-report delivery. Four incremental probes edit, restore, add, and remove source files; both manifest and game identities must change and then reproduce exactly. No mutant hook is compiled into the production crates. These are representative failures, not exhaustive mutation or conformance coverage.

The combined crawl preserves a regression component covering the previous 60 definitions under the old limits, with an explicit original Split Tide projection. `forge-verify crawl-optional` targets all nine cold-workshop definitions; `forge-verify crawl-batchworks` separately targets all thirteen kiln additions under its own fixed budget. `forge-verify crawl-salvage` separately targets the eight salvage additions; `forge-verify crawl-market-water` targets ten collateral and water additions. `forge-verify crawl-staffing` targets the four staffing additions within its own predeclared budget. `forge-verify crawl-cold-shift` targets the five dust-test and delegated-work additions under its declared limits. All six optional crawls execute complete catalogs and disclose a replayed seven-action Hold Market frontier alongside both presets. The prefix consumes depth. Checked witnesses cover cold crafting, timed manufacture, local use versus sale, bank versus missed tide, remote spoil, reclamation, first late entry, safe/skilled/risky salvage, exact entropy boundaries, protected manufacture, purchased and fuel-settled filters, three competing filter destinations, a complete market-water composition, and matched delivered/undelivered NPC reports. The nine market witnesses also bind every storage balance and directional transfer. Eight staffing witnesses compare character history, actual help and account, physical absence and cancellation, the slower lift, and its full water composition. Thirteen dust-test and cold-work witnesses bind actual delivery, matched personal/delegated work, physical returns, unpaid reports and both water continuations. The registry contains 68 witnesses. These are scripted replay proofs.

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

The checked combined report covers all 100 current definitions and nine connected locations through its separately bounded regression, batch, salvage, market-water, staffing, and cold-shift components. The regression starts from both authored characters and retains every old definition and the original 51-definition/six-location projection. Each component reconstructs complete paged catalogs and executes advertised canonical actions. The gate reproduces all seven crawl artifacts byte for byte in separate processes, including the separately checked cold-pilot report. This is bounded coverage; it does not exhaust every possible world state.

The separate scale fixture compiles exactly 500 terminal synthetic locations in a reciprocal ring, admits their exact runtime ID set, independently proves graph reachability, and completes 500 kernel-enumerated travel transitions back to its start. It reconstructs every page of every travel-state catalog under fixed graph-frontier, catalog-work, and serialization ceilings. Fresh processes reproduce and recheck `evidence/scale/synthetic-ring-500.json` byte for byte. This proves engine and compiler capacity only; the generated nodes are not game content and do not count as authored breadth, area depth, or Skyrim parity.

## Core promises

- Plain words, fast turns, and meaningful actions.
- One world whose people and places remember consequences.
- A character whose identity, abilities, appearance, values, and history matter.
- No hidden menu cap on programmed legal actions.
- Replayable outcomes and mechanically checked evidence.

## Repository history

The four founding briefs are preserved in Git commit `594183b`. Their consolidated requirements and the active roadmap are in `PLAN.md`.
