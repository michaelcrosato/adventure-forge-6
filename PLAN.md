# Adventure Forge: Initial Plan

Status: Active  
Owner: Manager agent  
Started: 2026-09-03  
Founding inputs: archived in Git commit `594183b`

## 1. Mission

Build the best possible action-first role-playing game in one persistent world. The final world must seek the exploration breadth of *Skyrim* and the local choice depth of *Baldur's Gate 3* without copying either game's setting, content, or intellectual property.

The player creates a specific person. Who they are, what they can do, and what they have done must change legal actions, risk, knowledge, relationships, prices, access, world events, and endings throughout play.

The game is the product. The repository, agent workflow, test volume, and code volume matter only when they make the game more truthful, deeper, broader, clearer, or more reliable.

## 2. Definition of success

The project is complete only when all of these statements have direct evidence:

1. One normal player interface runs the complete game.
2. One connected world provides breadth comparable to the target reference and no counted area is filler.
3. Main-path and optional areas provide distinctive casts, mechanics, approaches, reactions, consequences, and revisits.
4. Character creation has many orthogonal dimensions and combinations produce coherent systemic differences throughout the world.
5. Play stays concise and action-first.
6. Every legal state change comes from a validated canonical action in deterministic code.
7. Saves, outcomes, findings, and advertised claims have build-bound replay evidence.
8. One mechanical `verify` command exercises all hard conformance gates without an AI model.
9. Blind player agents cannot access source, hidden state, authored solutions, or builder tools.
10. One accountable manager controls priorities, delegation, integration, releases, and workflow changes.
11. Blind comparative play supports the breadth and depth claims.
12. The shipped game is the strongest conforming result we can produce, not merely a minimal technical demonstration.

Partial releases must state their actual shipped scale. A scalable engine, generated map, test fixture, or roadmap is not evidence that the final content target has been met.

## 3. Non-negotiable product rules

### 3.1 The model never becomes the world

The authoritative operation is conceptually:

```text
step(build, state, canonical_action, entropy) -> transition
```

It cannot depend on a model response, network, wall clock, locale, process identity, ambient randomness, or undeclared mutable state. AI may build content, map free text to an enumerated action, play, critique, or manage work. It may never invent an authoritative result at play time.

### 3.2 Content is data and behavior is closed

World content uses typed conditions and effects interpreted by the kernel. Unknown operations, invalid values, duplicate identities, broken references, contradictory declarations, invalid starting state, and missing required witnesses fail the build. A new behavior ships with compiler, reducer, and verification support in the same change.

### 3.3 Legal actions come from the kernel

The kernel enumerates the complete legal action set and gives every action a stable identity bound to its build and pre-state. Invalid, fabricated, and stale actions change nothing.

There is no authored maximum action count. Presentation may rank, group, search, or page a large set, but the union of the presented catalog must equal the kernel set. Resource exhaustion is explicit; silent truncation is forbidden.

### 3.4 One world means one history

Every exterior, interior, dungeon, conversation, dream, and distant region shares one world identity, timeline, event history, and persistent state. Content may load in regions, but a region is never a disposable game instance. Credible effects cross area boundaries and remain visible on return.

### 3.5 A character is more than a build

The character sheet is extensible and initially includes:

- lineage and place of origin;
- background and prior work;
- physical, practical, mental, and social aptitudes;
- trained skills and learned knowledge;
- values and sworn beliefs;
- traits and flaws;
- visible appearance, attire, and markings;
- affiliations, standing, and reputation;
- equipment, resources, injuries, and conditions;
- deeds, promises, crimes, discoveries, and relationships.

Conditions can query any facet and combinations of facets. Different inhabitants have their own goals, values, relationships, memories, and knowledge provenance. Nobody reacts to a hidden fact without a credible path to learn it.

### 3.6 Plain words and fast turns

Default shipped-text limits use the strictest founding targets:

- action labels: one to three words preferred, eight words maximum;
- ordinary sentences: eighteen words maximum;
- area descriptions: one or two short sentences;
- routine observation: under one hundred new words before actions;
- first visit to a complex area: under one hundred eighty words;
- unrequested dialogue: one short turn and never over sixty words.

Text uses common words, active voice, and concrete facts. Optional inspection can add detail. Required progress cannot depend on an exposition wall.

Every live scene normally provides at least two non-movement state-changing actions. Depth lives in verbs, systems, tradeoffs, and consequences rather than paragraph count.

### 3.7 Claims are proofs

Every advertised outcome and shipped area has replayable witnesses tied to the exact build. Verified factual findings reproduce against their cited session. Subjective play feedback stays clearly separate. Altered evidence fails verification.

## 4. Creative direction: Veyra Basin

The world begins in Veyra Basin, a land shaped by managed tides, old floodworks, competing civic charters, mobile markets, and communities that depend on one another while fighting over water and passage.

The setting supports physical adventure, social leverage, ecology, law, trade, craft, stealth, survival, and large persistent consequences without requiring long lore explanations. Water links the first local choice to the wider world in a way the player can see.

### 4.1 First playable arc: The Split Tide

A forged water order and a stolen copper Tide Key will drive the next surge toward either Lowsail Market or the upland works. The player has one tide cycle to learn who changed the order, choose whom to trust, and alter the Red Sluice. The player then returns through the same world and sees the lasting result.

#### Lowsail Market

A floating market under curfew. Its defining systems are social stealth, disguise, suspicion, recognition, rumor provenance, permits, evidence, warrants, and faction authority.

Key interactables include the council checkpoint, ferry crane, courier skiff, tide ledger, warning bell, cargo deck, and submerged culvert.

#### Red Sluice

A vertical hydraulic gate tower connected by levee road and culvert. Its defining systems are climbing, route height, water pressure, flow routing, repair, rescue, and sabotage.

Key interactables include the service face, intake, gate wheels, pressure vents, worker cage, broken gauge, and signal mast.

Neither area is an instance. Market choices change Sluice access and risk. Sluice choices change water, prices, authority, inhabitants, routes, and dialogue on the return journey.

### 4.2 Initial cast and bounded knowledge

- **Sava Rusk**, council inspector: wants order and a quiet investigation. Knows official records and witnessed checkpoint events.
- **Oren Pell**, ferry broker: wants open routes and debt relief. Knows dock rumors and smuggler paths.
- **Yara Dene**, courier: stole the Tide Key to stop a purge. Knows her employer and one hidden route.
- **Edrik Voss**, sluice engineer: knows the gate defect and repair sequence. He does not know market events until news reaches him.
- **Mira Kett**, worker leader: wants paid crews and shared water. Knows the worker bypass and remembers rescue, threats, and deaths.

Knowledge records carry provenance: witnessed, told, read, inferred, or spread as rumor. Transfer is an authoritative event.

### 4.3 Counterfactual starting characters

The first proof pair intentionally combines several facets:

- **Ilyan Vale:** Fenborn ledger clerk; high Insight and Presence; values Order; has Tide-Ear; owes a debt; bears council ink; once saved a worker.
- **Rook Ash:** Kilnborn lock runner; high Might and Finesse; values Freedom; has Heat-Sense; is Wanted; bears a kiln scar; once stole a permit.

At the same checkpoint, Ilyan can audit the order and gain lawful access. Rook can blend with workers, find the culvert, or pressure the guard. Sava is warmer to Ilyan and suspicious of Rook; Oren behaves in the opposite direction. Prices, risk, information, later routes, and possible outcomes must also diverge so this is more than substituted text.

### 4.4 Initial outcomes

The first arc supports at least these persistent results:

1. Repair the works and split the flow: both communities survive; workers gain a water seat; the council loses its monopoly.
2. Back the council and hide the evidence: the market survives; upland works dry; prices rise; workers strike.
3. Help the couriers and evacuate: the market moves; an old ferry route opens; the council issues a warrant.
4. Expose the purge and open the relief channel: the lowlands flood safely; workers take the Sluice; trade moves inland.
5. Miss the tide or overload the gates: the world enters an authored disaster state with deaths, changed routes, scarcity, and hostility.

Each result needs a replay witness, meaningful intermediate divergence, and visible consequences on return. Failure continues the world rather than acting as a detached game-over screen whenever coherent play remains possible.

## 5. Technical architecture

### 5.1 Stack decision

Use a Rust workspace for authoritative and verification code:

- `forge-kernel`: state, conditions, effects, action enumeration, deterministic entropy, transition events, observation data, and canonical identities;
- `forge-content`: strict source schema, compiler, reference validation, text validation, area contracts, and canonical content packs;
- `forge-cli`: new game, play, save, replay, inspect public evidence, content compilation, and the top-level `verify` entry point;
- `forge-server`: later HTTP/session adapter using the same kernel, with serialized and idempotent commands;
- `forge-verify`: replay, crawling, properties, mutation corpus, scale fixtures, and evidence attestation.

Use a React and TypeScript browser client after the kernel and CLI slice is playable. It renders observations and submits canonical action identities. It owns no legality, consequence, random roll, character reaction, or quest rule.

Rust is chosen to make the authority portable, explicit, integer-safe, fast at large state exploration, and shareable with WebAssembly if offline replay becomes valuable. The first release favors a small dependency surface over framework breadth.

### 5.2 Authoritative state

One `GameState` contains:

- world and build identities;
- integer time and deterministic event schedule;
- current player location and the connected world graph;
- the complete character sheet, inventory, equipment, resources, injuries, and conditions;
- inhabitant location, goals, stance, relationships, memories, and knowledge;
- faction standing, law, prices, rumors, quests, discoveries, and promises;
- sparse persistent environmental overlays for every changed area;
- explicit seed, entropy algorithm version, and entropy cursor.

Immutable content and sparse mutable overlays remain separate. Region packs may load lazily, but global identities, timeline, scheduler, and consequences remain unified.

Authoritative quantities use integers or defined fixed-point types. Ordered maps and explicit sorting prevent iteration order from changing a result.

### 5.3 Transition pipeline

The kernel separates four jobs:

1. `legal_actions(state, content)` returns the complete, stably ordered action set.
2. `validate_action(state, action)` rejects unknown, illegal, stale, wrong-build, and malformed input without mutation.
3. `step(state, action, content, entropy)` applies a closed effect program and emits canonical events.
4. `observe(state, events, content, view)` returns bounded player-visible facts plus access to the full action catalog.

Action instances include definition ID, actor, targets, canonical parameters, build ID, and pre-state identity. Stable action-set digests and cursors make pagination complete and tamper-evident.

### 5.4 Conditions and effects

The initial typed condition vocabulary covers:

- facet, tag, skill, aptitude, and value checks;
- inventory, equipment, resource, and status checks;
- location, route, time, and environment checks;
- relationship, faction, suspicion, and reputation checks;
- memory, knowledge, rumor, discovery, deed, and promise checks;
- boolean composition with explicit `all`, `any`, and `not`.

The initial effect vocabulary covers:

- move an actor;
- give, take, equip, and consume an item;
- change a resource, status, relationship, faction standing, suspicion, or price modifier;
- add a memory or transfer knowledge with provenance;
- reveal a fact or route;
- set an environment or quest flag;
- spawn, move, disable, or remove an entity;
- apply injury, recovery, risk, and deterministic checks;
- advance time and schedule a future event.

Definitions are namespaced strings, never array positions. Extensions are versioned. No prose field can hide an effect.

### 5.5 Canonical identity and replay

Canonical serialization is explicit and versioned. Hash inputs use normalized UTF-8, stable keys and ordering, integer values, and no timestamps or absolute paths.

The build ID covers every behavior-changing input, including:

- kernel and rules artifact;
- schema, condition, and effect versions;
- compiler and transformations;
- canonical compiled content;
- authoritative configuration and start definitions;
- pinned toolchain and dependency lock.

A trace records the build, start specification, character, seed, every canonical action, entropy use, pre/post state identities, event and legal-set identities, final state, and a receipt chain. A save is a checked trace prefix plus an optional state checkpoint. Wrong-build or altered material fails safely.

### 5.6 Player surfaces

The CLI is the first real player surface and the reference for blind tests. The browser follows the same observation/action protocol. Both support:

- concise result-first observations;
- action groups and counts;
- deterministic paging and search;
- inspection of all programmed legal actions;
- save, resume, and trace export;
- no hidden-state or source access.

Free text may later map to an existing action identity. A failed or ambiguous mapping changes nothing.

## 6. World-scale production strategy

The target is a large authored world supported by tools, not a large generated claim.

### 6.1 World shape

Grow outward from Veyra Basin into connected watersheds, uplands, civic centers, industrial belts, coasts, and wild frontiers. Each major region introduces a mechanic bundle that interacts with existing systems. Early candidates include:

- Fume Yards: craft, heat, contamination, labor, and supply chains;
- Civic Ring: law, testimony, evidence, favors, and public authority;
- Saltwild: weather, ecology, tracking, shelter, and migration;
- Glass Coast: sailing, salvage, storms, tides, and rival ports;
- Root March: living terrain, medicine, cultivation, and territorial memory.

### 6.2 Area contract

An area counts toward shipped scale only after it has:

- a distinct local identity and mechanic mix;
- named inhabitants with goals, relationships, knowledge, and schedules;
- several interacting problems or opportunities;
- at least four materially useful approach families;
- at least three meaningful state-changing interactables in dense locations;
- character-specific actions and combinatorial reactions;
- persistent outcomes and a changed revisit;
- at least one credible effect beyond its own boundary;
- replay witnesses and bounded-text validation;
- an action/effect/cast fingerprint sufficiently different from nearby areas.

Procedural terrain, roads, weather, common ecology, and travel events can form substrate. They do not count as finished areas until they pass the contract.

### 6.3 Scale gates

- Prove the engine and compiler on a synthetic graph of at least 500 locations.
- Track generated substrate separately from authored and verified areas.
- Require at least half of shipped locations to contain three or more meaningful state-changing interactables; aim higher as tools improve.
- Run anti-sameness analysis over action families, effects, inhabitants, topology, mechanics, and outcomes.
- Sample optional areas as aggressively as the main path.
- Use blind traversal, decision-path studies, long sessions, and comparison against the reference targets before making final breadth or depth claims.

No single numeric target proves the final requirement.

## 7. Verification bar

One non-AI `verify` command must eventually cover all of the following. The command reports bounded coverage honestly and never calls a model.

1. Format, lint, static analysis, unit tests, and declared dependency policy.
2. Canonical build identity and sensitivity to every authoritative input.
3. Schema rejection for unknown behavior, malformed values, duplicates, broken references, invalid starts, and missing evidence.
4. Independent clean-process deterministic replay, including observations, legal sets, events, state hashes, and final receipts.
5. Tamper rejection for build, seed, character, entropy, actions, intermediate receipts, checkpoint, and final state.
6. Stale and invalid action no-op behavior.
7. Save/resume parity with uninterrupted play.
8. Bounded BFS/DFS and seeded property exploration for crashes, invalid state, false legal actions, nonterminal dead ends, and invariant breaks.
9. Character counterfactuals, combination reactions, different NPC knowledge, and persistent reactions.
10. Cross-area effects and changed revisits in the single world.
11. Witness traces for every advertised outcome and shipped-area contract.
12. Static and runtime language, readability, observation, dialogue, and action-density budgets.
13. A 256-action stress scene whose complete kernel set equals all catalog pages with no collision or truncation.
14. A 500-plus-location scale fixture and performance budget.
15. Blind-player isolation, including source canaries, filesystem, network, debug-output, hidden-state, path, and prompt-injection probes.
16. Evidence provenance: valid reports attest; changed and wrong-build reports fail; opinion remains unverified.
17. Bad-change sensitivity through representative mutants such as nondeterministic ordering, omitted manifest input, stale-action bypass, action truncation, lost remote memory, weakened prose checks, and hidden-state leakage.
18. One complete improvement cycle from replayed blind finding to accepted change while prior regression predicates remain green.

The verification contract protects the external requirements. The manager may replace its implementation for better coverage or speed, but never edit it merely to admit a failing change.

## 8. Blind play and evidence

Builder and player work use an information firewall:

| Role | Sees source or solutions | Changes repository | Produces |
| --- | --- | --- | --- |
| Manager/builder | Yes | Under manager control | Candidate build or content change |
| Blind player | No | No | Player-surface transcript and replay trace |
| Mechanical verifier | Only declared build/evidence inputs | No | Pass/fail evidence and coverage |

Blind sessions run under a separate identity and eventually a locked container or microVM. They receive only a compiled player bundle or remote player protocol. Source canaries make accidental leakage testable.

Reports identify exact build, session, player/model configuration, trace, result, latency, available cost, and structured observations. Facts become verified only after replay. Triage tracks duplicates, corroboration, staleness, impact, priority, fix, and regression evidence.

## 9. Manager operating model

There is one active logical manager. It owns the roadmap, architecture, task selection, delegation, integration, verification, release calls, coherence, and workflow health.

The recurring cycle is:

1. Inspect the current game, evidence, risks, and workflow.
2. Choose the largest conformance gap or highest-value player improvement.
3. Define acceptance evidence before implementation.
4. Delegate independent bounded work when parallelism improves output.
5. Treat agent output as a proposal until reviewed and integrated.
6. Run focused checks and the global bar appropriate to the change.
7. Accept, revise, quarantine, or reject based on evidence.
8. Update durable status, decisions, risks, ownership, and next priorities.
9. Improve the workflow when evidence shows wasted effort or weak protection.

`gpt-5.6-luna` at maximum reasoning is the initial subagent benchmark for bounded architecture, implementation, content, and review work. The manager records wall time, accepted output, correction effort, regressions, and cost when available. A different model replaces it only when repeated comparable tasks show a better quality-adjusted result. Speed matters because iteration improves the game; unchecked volume does not.

Concurrent work must use explicit scopes and acceptance criteria. The manager resolves cross-area concepts, identity namespaces, mechanical overlap, lore contradictions, merge conflicts, and final decisions.

## 10. Delivery roadmap

### Milestone 0: Honest kernel foundation

Deliver:

- Rust workspace and pinned toolchain;
- canonical state and content identities;
- explicit deterministic entropy;
- typed character, world, actor, memory, knowledge, and environmental state;
- complete action enumeration, stale binding, reducer, and events;
- strict two-area content compilation;
- first CLI loop;
- trace recording and replay;
- initial `verify` covering determinism, invalid actions, content references, prose budgets, and no action cap.

Exit evidence: two opposite characters enter the same Lowsail scene, receive materially different legal actions and reactions, and replay to identical fingerprints.

### Milestone 1: Complete Split Tide slice

Deliver both authored areas, their mechanic bundles, cast, knowledge transfer, multiple routes, persistent return states, five outcomes, saves, crawler, counterfactual witnesses, 256-action stress fixture, 500-node scale fixture, and a usable CLI.

Exit evidence: every declared outcome replays; a market action changes the Sluice; the Sluice changes the returned market; save/resume matches continuous play; the full gate is green.

### Milestone 2: Player product and blind flywheel

Deliver the HTTP adapter, responsive browser client, character creator, action catalog search/paging, accessible presentation, locked blind-player environment, evidence-backed report intake, and mechanical promotion driver.

Exit evidence: a player completes the slice in the browser; a blind agent produces a verified finding; an altered report is rejected; a delegated fix improves the game without breaking earlier witnesses.

### Milestone 3: System breadth

Add combat, stealth, social leverage, law, craft, survival, ecology, travel, economy, faction simulation, schedules, companion relationships, injury, recovery, and systemic environment interactions. Systems must compose rather than create isolated minigames.

Exit evidence: held-out situations support several viable builds and cross-system approaches without prose-only branching or rule duplication.

### Milestone 4: World production engine

Add content authoring tools, world namespace allocation, area templates that enforce contracts without enforcing sameness, dependency-aware region packs, outcome/witness authoring, anti-repetition reports, automated play sampling, and safe parallel integration.

Exit evidence: multiple independently authored regions integrate into one history, stay distinct under blind sampling, and do not regress the core proofs.

### Milestone 5: Breadth expansion

Expand connected verified regions in quality-gated waves. Each wave adds authored exploration, optional depth, cross-region consequences, companions, factions, quests, secrets, and systemic interactions. Generated substrate remains separately labeled.

Exit evidence for every wave: all new area contracts, outcome witnesses, global coherence checks, old replays or explicit compatibility policy, performance targets, and blind quality sampling pass.

### Milestone 6: Final conformance and comparative polish

Run complete long-session, character-counterfactual, traversal, content-reuse, decision-path, optional-area, accessibility, performance, and blind comparative studies. Close every hard-constraint gap and prioritize player-facing quality until further changes no longer improve held-out results.

Exit evidence: the requirement-by-requirement completion audit has direct authoritative proof for every claim, including the full world-scale and area-depth target.

## 11. Immediate execution queue

1. Create the Rust workspace, project charter, status ledger, and one-command developer interface.
2. Implement canonical serialization, hashing, entropy, core IDs, and immutable action binding.
3. Implement the minimum condition/effect vocabulary and strict content compiler.
4. Build Lowsail's checkpoint as the first counterfactual scene.
5. Add replay receipts and clean-process replay tests before expanding content.
6. Extend the connected graph to Red Sluice and prove one cross-area return consequence.
7. Make the CLI pleasant enough for genuine play, then run the first blind session.

The manager reassesses this order whenever direct evidence reveals a larger risk or better player-value opportunity.

## 12. Known initial risks

- A content DSL can become expressive enough to hide arbitrary code or too weak to create deep areas.
- Build hashing can omit a behavior-changing compiler, dependency, configuration, or generated input.
- Collection ordering, serialization, locale, async scheduling, or accidental I/O can introduce nondeterminism.
- Action generation can create combinatorial noise even when it has no artificial cap.
- Generic tags can produce shallow renamed reactions rather than perspective-specific behavior.
- Regional loading can accidentally split the world or lose distant consequences.
- Numeric scale can reward repeated content unless area contracts and blind sampling remain strict.
- A client, free-text mapper, or AI player can accidentally become a second rules engine.
- Verification can be weakened, overfit, or mistaken for proof beyond its measured coverage.
- Fast parallel content work can create duplicated ideas and global contradictions.

These are roadmap inputs, not reasons to reduce the target.
