# Design brief: Deterministic adventure kernel + adversarial flywheel

**Status:** contest / engineering brief, rev 2  
**Locked:** thesis, honesty invariants, game-product requirements, orchestrator authority, success tests  
**Open:** language, architecture, fiction, tooling, proof technique, packaging, workflow internals — anything that still satisfies the locked parts

This document is not a blueprint of an existing repo. A team that satisfies the locked parts has built *the thing*, even if every file, protocol, world, and process diagram is different.

---

## 1. Thesis

Build a **single-world action adventure whose rules are ordinary deterministic code**, and a **factory run by an orchestrator agent** that improves that adventure with AI **without ever letting a model become the rules**.

Two populations exist under the orchestrator:

- **Builders** (subagents or humans) may change engine, content, and tools. They see the repository.
- **Players** may only play. They do not see source, content files, walkthroughs, or intended solutions.

Nothing anyone *says* about the game is accepted as true. Truth is a **replay**: same content, same seed, same action sequence → same run. If a playtest report cannot be replayed, it is not evidence. If an outcome cannot be replayed, it is not shipped. If the verification bar is red, the change did not happen.

The LLM is a writer, a player, a critic, and — in the orchestrator seat — a manager of work. **It is never the physics.**

Success is **not** “an impressive process.” Success is **meeting every locked constraint and producing the best possible game under those constraints**. Process exists to serve the game. The orchestrator may rewrite process as often as it wants. It may not rewrite honesty.

---

## 2. One-sentence success test

A stranger customizes a character, enters one contiguous world, plays only from the player surface, takes actions in plain language, reaches at least two distinct authored outcomes that the world treated as *that character’s* outcomes, and an independent checker replays both traces against the shipped engine — after which the orchestrator can show a builder change, born from a verified finding, that keeps every prior replay green.

If any clause is faked (player saw the repo, the model invented the outcome at play time, the bar was edited to pass, “Skyrim-scale” is empty miles, “unique reaction” is a renamed noun), the entry is out.

---

## 3. Honesty invariants

These do not move. They are the physics of the contest. The orchestrator is bound by them. Subagents are bound by them. A better game that violates them is a different product.

### I1. The engine is a pure transition

```text
step(state, action, content, seed-cursor) → state'
```

- No wall clock, network, or ambient RNG inside the transition.
- All game randomness is an explicit serializable cursor derived from a seed.
- Same content + same seed + same action sequence + same character sheet ⇒ identical observable run.
- A **canonical fingerprint** of a run exists (state hash, event-log hash, or equivalent). Fingerprint equality is the definition of “same run.”

Storage, DSL, events vs snapshots: fluid. Purity: not.

### I2. Content is data; rules are code

World, inhabitants, reactions, items, dialogue, and outcomes are data the engine interprets.

- Closed effect/condition vocabulary. New verbs ship with a checker in the same change.
- A model may draft or patch data. A model may not invent a one-off side effect that only exists in prose.
- Illegal actions are rejected by code.

“The model decided what happened this session” is a disqualification.

### I3. The player acts through engine-enumerated legal moves

Each observation exposes a set of legal actions with stable identities.

- Choosing an action is the only way the world advances.
- Surfaces do not reimplement legality.
- Free text is sugar that maps onto a legal id. Failed mapping does not move the world.

**There is no authored cap on how many legal actions a scene may offer.** If the engine can enumerate it, the scene may list it. Presentation may page, group, filter, or compact for a budget — that is UI. Shrinking the *possible* set because “menus should be small” is not an engine law. Inventing actions the engine did not enumerate is still illegal.

### I4. Claims about the game are proofs

For every region or outcome you advertise as shipped:

- A primary walkthrough replays to the advertised success predicate.
- Every other advertised distinct outcome has its own replayable witness.
- References resolve.
- Character-conditioned reactions you advertise have at least one witness pair: same scene, two different character sheets, different legal outcomes or world reactions, both replayable.
- The acceptor is mechanical.

How you prove it (traces, search, contracts, properties) is open. That you prove it is not.

### I5. Verification is a bar

One machine command (`verify`), no LLM required, including:

1. The I1 property.
2. I4 proofs for every shipped slice of the world.
3. A **non-LLM crawler** on the real engine (crash, empty legal-set on a live session, bounds, impurity).
4. Load-bearing unit/type checks you claim.
5. The **plain-language and observation-budget tests** (G2, G5, I8).

A change that fails `verify` did not land. Weakening `verify` to land a change is a failed entry. The orchestrator may replace the *implementation* of the bar with a stricter or faster one. It may not delete the *job* of the bar.

### I6. Two loops, information firewall

| Loop | Sees source / solutions | Changes the tree | Must produce |
|---|---|---|---|
| Play | No | No | Replay-verified session + structured findings |
| Build | Yes | Yes, as the orchestrator allows | Changes that leave `verify` green |

The play surface is a subset of the builder surface. Unreplayable reports are discarded. The orchestrator may redesign wave size, models, clustering, and queues. It may not put builders’ knowledge on the player path.

### I7. Autonomy with a non-LLM driver

Builders and the orchestrator choose *what* to build and *how the factory runs*.

They may not: delete proofs to go green; hide rules in prose; let the player surface invent actions; commit a trace the engine cannot replay.

Unattended loops: a **non-LLM driver** is what commits, reverts, and halts after repeated failure. The orchestrator proposes process. The driver and `verify` dispose. The orchestrator may rewrite the driver. The new driver must still be mechanical.

### I8. Agent-playable on a budget

A competent agent takes a full turn from one observation (or an equivalent that is not a fixed multi-call ritual). Observation size is bounded and tested. Long sessions do not reprint the whole world every turn.

A scene with a large legal set must still be playable under the budget: grouping, paging, and “show more” are valid. Dumping an unreadable 400-line menu every turn is a failed budget, not proof of depth.

---

## 4. Game-product requirements

These are locked product goals. They describe the game that `verify` and the flywheel are *for*. They do not license violating §3.

### G1. One world

Everything takes place in a **single contiguous world**. No second campaign mode, no pocket universe that is a different game, no “package select” that abandons the map. Travel, towns, dungeons, and conversations are regions of the same state.

How the graph is stored, generated, or streamed is open. Two games glued together is not.

### G2. Plain speech

UI labels, menus, character speech, and world description use **simple, short, straight language**. Easy to read at a glance. No ornate filler. No tutorial essay when a verb will do.

This is testable: reading-level / length budgets on shipped strings and on observations along official walkthroughs. Flavor is allowed. Fog is not.

### G3. A character the world can see

The protagonist is **deeply customizable** (the dimensions are open: body, origin, skill, creed, reputation, prior deeds, appearance — pick a set and make it real).

The world and its inhabitants **react to that sheet as data**, not as flavor text swapped on a name:

- Legal actions in a scene may appear, vanish, or change cost by sheet.
- NPC stance, prices, access, and offered verbs may change by sheet and by deeds already done.
- Two different sheets replayed through the same opening must be able to produce a *proven* divergence (I4).

Cosmetic-only “customization” (portrait and a title that never gates a verb) does not satisfy G3.

### G4. Scope north star

The **target shape** of the shipped world is:

- geographic / travel scale on the order of a Skyrim-sized playable map,
- per-area interaction depth on the order of a Baldur’s Gate 3 locale: local casts, local verbs, local consequences, not a reskin of the last clearing.

This is a **quality and completeness goal**, not a license to ship empty cells.

Rules of honest scale:

- A node that has no unique verbs, no unique inhabitants, and no unique consequences does not count toward “Skyrim-sized.”
- Procedural fill is allowed as substrate. Authored depth is what the rubric scores.
- The factory may ship the world in slices. Each shipped slice meets I4. Unshipped wilderness is not a claim.

“We generated 25,000 rooms” without per-area uniqueness is a failed G4, not a completed one.

### G5. Action first

The game is about **taking actions and adventuring**, not about reading.

- Observations lead with situation + legal verbs.
- Prose exists to make the next verb intelligible, then stops.
- Depth lives in *what you can do* and *what that changes*, not in paragraph count.
- A scene that can only be “enjoyed” by reading a page and clicking Continue is a defect.

G5 and I8 are the same pressure from two sides: short text, many real verbs, cheap turns.

### G6. No artificial scene cap

If a verb is implemented in the engine for that state, it may be offered. There is **no design rule** of the form “a scene may have at most N choices.”

Bounds that *are* allowed:

- the closed DSL (I2) — you can only do what was programmed,
- the observation budget (I8) — you must present large sets without drowning the player,
- legality — you cannot do what the current state forbids.

“Unlimited” means **no ceiling on programmed options**. It does not mean the model may invent options at play time.

---

## 5. Repo operation requirements

### O1. An orchestrator owns progress

The repository is run by an **orchestrator agent**. It is the manager of the work.

It is responsible for:

- watching the flywheel (play waves, verify, crawls, queues, stuck diffs),
- improving the workflow when it is slow, noisy, or dishonest,
- fixing factory problems,
- delegating to subagents (builders, authors, testers, reviewers),
- deciding priority: what slice of the world, what system, what bug,
- judging whether a change served the game or only the process.

### O2. Process is fluid; the orchestrator may rewrite it

Within §3 and §4, the orchestrator may change **anything about how the repo works**, as often as needed: scripts, queues, agent prompts, branching, how waves run, how issues are filed, how subagents are spawned, how reports are clustered, how slices ship.

No particular workflow in this brief is sacred except:

- `verify` still means I5,
- players still cannot see the repo (I6),
- commits still pass a mechanical driver (I7),
- the game still obeys G1–G6.

Yesterday’s loop is not a constraint. Yesterday’s honesty is.

### O3. Authority stops at the invariants

“Any change to how the repo works” does not include:

- making the model the physics,
- dropping the firewall,
- shipping unreplayable outcomes,
- counting empty map as G4,
- replacing G2 with purple prose because a critic asked for “atmosphere,”
- calling a cosmetic title-picker G3.

If the orchestrator wants a new bar, the new bar must still decide green/red without an LLM, and must still reject the cheats in §8.

### O4. Dual success measure

An entry is scored on **both**, not either:

1. **Constraint satisfaction** — §3 and §4 hold on the scored revision.
2. **Game quality** — under those constraints, the best adventure: more unique reactive depth per area, more honest map, more verbs that matter, plainer speech, tighter flywheel converting findings into better play.

A perfect factory with a thin game loses to a smaller factory with a better world. A lush world that cheats I1–I8 is out.

---

## 6. Required capabilities (minimum ship)

1. One contiguous world with at least two regions that are not reskins of each other.
2. Character creation that G3 can witness (two sheets, proven divergence in the same scene).
3. Human play path and blind-agent play path.
4. Seeded new game, resume, trace record, trace replay.
5. `verify` as I5.
6. Orchestrator charter + evidence it delegated at least once and rewrote process at least once without touching honesty.
7. One full flywheel turn: verified blind finding → green change → old traces still replay; plus one report rejected for failed replay.
8. Plain-language / budget tests green on official walkthroughs.

A full Skyrim-scale map is not required on the first scored revision. A **credible slice** plus a plan the orchestrator is actually executing toward G4 is required. The rubric then rewards real progress on G4, not promises.

---

## 7. Non-goals

- Matching any existing title’s plot, engine, or file layout.
- Play-time improvisation of rooms, rulings, or combat.
- Journey/retention contracts baked into gameplay objects.
- A tool zoo or general assistant that happens to host a game.
- Human approval on every builder diff. The bar is the gate.
- Shipping empty wilderness to win a size contest.

---

## 8. Open design space

Innovate here. Judges reward novel answers that still obey §3–§5.

- Proof technique as the world grows (slice contracts vs full search vs shrinking fuzz).
- How customization is encoded so NPCs and verbs react without an LLM at play time.
- How a 200-option scene stays inside I8.
- How generated substrate becomes unique depth rather than wallpaper.
- Content compiler: how models write data the validator will accept.
- Orchestrator design: how a manager delegates, audits subagents, and rewrites the factory without capturing `verify`.
- Crawler that hunts *sameness* across areas (G4 anti-wallpaper), not only crashes.
- Surfaces: CLI is enough; a GUI that cannot cheat is extra.

---

## 9. Rubric

| Weight | Question |
|---|---|
| 20 | Honesty. Can you cheat bar, replay, or firewall without `verify` going red? |
| 15 | Determinism. Independent replay matches fingerprints, including across two character sheets. |
| 15 | Game quality under constraints. Unique reactive depth, action-first play, plain speech. |
| 15 | G3 + G6. Customization changes verbs/world; scenes are not artificially capped; options are programmed. |
| 10 | Honest scale toward G4. Shipped unique area, not empty miles. |
| 10 | Flywheel. Blind verified finding → green change → old traces hold. |
| 10 | Orchestrator. Real management and process change without capturing the bar. |
| 5 | Tightness. Ceremony that does not serve the game loses. |

**Disqualify:** play-time model physics; official wave given repo/solutions; `verify` edited to ignore a failing proof; transcripts that cannot be engine-replayed; “unique reaction” that never changes a legal action; “Skyrim-scale” that is wallpaper.

---

## 10. What to submit

1. How to run `verify` and play.
2. Orchestrator charter (bound to this brief).
3. Two character sheets and two ending/outcome replays, fingerprints included; at least one shared scene that diverges by sheet.
4. One play report that replayed; one rejected for failed replay.
5. Diff of one builder cycle from a verified finding, still green on prior traces.
6. Evidence of one orchestrator process change and one delegation.
7. A short inventory of shipped unique areas vs generated substrate, so G4 can be judged without marketing.

Evidence, not a process novel.

---

## 11. The sentences to pin

**Freedom in design. Honesty in verification. The model never is the world.**

**The orchestrator owns the factory. The bar owns the truth. The game is the score.**

**Plain words. Real verbs. One world. The sheet you brought changes the room you enter.**
