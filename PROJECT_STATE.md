# Project State

Updated: 2026-09-04
Manager cycle: 11
Release state: playable CLI slice; no public release
Active milestone: 1 — Complete Split Tide slice

## Current truth

- The local repository is a Git repository on branch `main`.
- `origin` is the public HTTPS repository `https://github.com/michaelcrosato/adventure-forge-6`.
- The four founding documents are preserved in commit `594183b`, consolidated into `PLAN.md`, and removed from the live tree.
- The authoritative kernel, production content compiler, replay layer, and player CLI pass the local mechanical gate.
- The Split Tide production pack contains 6 connected locations, 5 named inhabitants, 2 complete starting presets, 6 two-way creation axes, 64 valid custom combinations, and 47 programmed action definitions.
- Replay receipts bind authored preset or canonical custom genesis, canonical actions, legal-set identities, entropy, events, states, and player-visible observations.
- Players can start as Ilyan or Rook or create a named custom character, review or preview authored choices, page or search every current legal action, save atomically, resume, and render a verified replay.
- Player input is bounded to 4 KiB per line and 1,024 lines per session; public CLI failures omit host paths, operating-system details, verifier internals, and final-state identifiers.
- A fail-closed Linux Bubblewrap rehearsal runs the stripped release player as UID/GID 65534 with no capabilities, no repository or host tools, no network, a read-only root, and only one writable save directory; it applies and probes process, memory, descriptor, output, wall-clock, and input limits, then checks the exact save with a separately built verifier.
- Player save files omit hidden state, events, entropy, and observations; the trusted replay layer reconstructs those claims from a preset or canonical public creation recipe plus canonical action identities.
- Eleven checked scenario witnesses cover the two Milestone 0 preset paths, two mechanically mixed custom characters, all five exclusive Sluice outcomes through their matching return consequences, and representative paths through both authored areas.
- Every scenario is bound to a reviewed ID, canonical authored start, seed, exact full-parameter action recipe, and hidden semantic postconditions; relabeling and alternate valid path substitution fail verification.
- Content, player-trace, detailed-trace, evidence-witness, and scale-report JSON decoders reject duplicate object keys before typed maps can collapse them.
- A checked, clean-process crawl expands 44 states from both presets, reaches all 6 locations, executes 283 advertised canonical actions, and covers all 47 authored definitions within explicit depth, state, frontier, and action budgets.
- The crawl carries an opaque ordered execution receipt over starts, expansions, complete action catalogs, events, entropy, and transitions; an independent ascending action-ID oracle rejects self-consistent catalog permutations.
- A separate checked capacity fixture compiles exactly 500 terminal synthetic locations, proves the same location-ID set through source, compiled content, runtime admission, independent graph traversal, and 500 canonical travel transitions, and stays within reviewed graph-frontier, catalog-work, and serialization ceilings.
- Choosing one of the five Sluice outcomes now closes the other four, preventing contradictory persistent results in one playthrough while preserving branch coverage across separate states.
- Milestone 0 exit evidence passes. Milestone 1 outcome, representative area, save/resume, catalog, crawl, and synthetic scale evidence now pass; broader player evidence still remains.
- This is a playable tested slice, not a public release or evidence of final scale.
- A live Codex player adapter and launcher now require saved ChatGPT subscription authentication, scrub all API-key variables, isolate the model from the repository and default plugins, expose only numbered public game actions, and independently verify the resulting player trace. The first accepted live run remains pending.

## Constraint evidence

| Area | State | Direct evidence | Next proof |
| --- | --- | --- | --- |
| Deep character customization | Partial | Kernel-owned creation derives 64 combinations from 6 authored axes; every combination validates and has at least 2 meaningful opening actions; each axis changes a legal set in an enumerated context; two hybrid recipes have checked fresh-process witnesses | Broader reactions, risks, prices, relationships, and outcomes across longer play |
| Concise language | Partial | Compiler enforces category, label, sentence, result, variant, and routine-observation budgets | Dialogue, readability, active-voice, and reading tests |
| One persistent world | Partial | One connected state graph; Lowsail warning changes Sluice legality; Sluice changes Lowsail return; consequential outcomes are mutually exclusive | Long-session and broader cross-region continuity |
| Skyrim breadth | Not built | A checked 500-location synthetic fixture proves engine/compiler capacity only; generated nodes count as no authored breadth | Authored expansion waves and blind comparative evidence |
| BG3-like area depth | Not built | Representative checked paths through both authored areas; no comparative depth claim | Blindly sampled Split Tide approaches and outcomes |
| Action-first play | Partial | Result-first observations and scripted CLI transcripts put numbered legal actions directly after concise results | Blind session and action/read measurements |
| No fixed action cap | Partial | Kernel pages cover 256 executable stress actions; CLI `next`, `prev`, `all`, and full-catalog search tests preserve the current catalog; the scale fixture consumes its complete paged travel catalog | Independent large-catalog process witness |
| Deterministic authority | Partial | Fresh processes reproduce the witnesses, crawl, and scale report; a neutral-controlled source-copy gate kills ambient kernel order, page order, frontier order, and process-ID receipt mutants | Broader property exploration plus hash, manifest, entropy, and persistence mutants |
| Build identity and validation | Partial | Trusted manifest binds kernel/compiler/replay sources, lockfile, pinned toolchain, config, schema v3 ABI, entropy, and content; ambiguous duplicate-key JSON is rejected | Independent recomputation and mutation corpus |
| Replay and persistence | Partial | Atomic bounded player-safe saves, CLI replay/resume, full-trace round-trip, recipe and receipt tamper rejection, preset/custom genesis reconstruction, exact resumed parity, and eleven checked clean-process scenarios | Broader corruption corpus and long-session witnesses |
| Blind play | Partial | A fail-closed release-only Bubblewrap rehearsal clears the environment, confines writes, isolates network access, rejects source/symlink probes, scans hidden fields, reproduces two sessions byte-for-byte, and verifies the save afterward | Run an actual blind-capable player/model inside the proven boundary with an embedded observation canary |
| Manager operation | Partial | Charter, plan, accepted delegated implementation, rejected first candidate, and task-splitting process change | Full verified-finding improvement cycle |

## Active queue

1. Run the first actual blind-capable player session through the proven locked boundary, with an embedded observation canary.
2. Add broader text-policy and long-session checks before expanding authored regions.
3. Extend the mutation corpus to hash canonicalization, manifest sensitivity, entropy, stale-action bypass, remote memory, prose checks, and hidden-state leakage.
4. Deepen creator-dependent reactions, risks, prices, relationships, and outcomes from blind and long-session findings.
5. Define the first authored expansion wave only after blind Split Tide findings are replayed.

## Delegation ledger

| Cycle | Lane | Model | Scope | Acceptance | State |
| --- | --- | --- | --- | --- | --- |
| 1 | Architecture review | `gpt-5.6-luna` / max | Read four briefs; recommend authority, content, replay, scaling, and slice architecture | Concise design with first-slice criteria | Accepted into `PLAN.md` |
| 1 | Game design review | `gpt-5.6-luna` / max | Design a two-area reactive vertical slice | World, cast, mechanics, counterfactuals, outcomes, evidence | Accepted as The Split Tide |
| 1 | Verification review | `gpt-5.6-luna` / max | Design mechanical bar and blind flywheel | Test matrix, evidence boundary, failure modes | Accepted into `PLAN.md` |
| 1 | Kernel/content implementation | `gpt-5.6-luna` / max | Implement only workspace, kernel, and content crates | Compiles; focused unit tests green | Rejected once; revised and accepted |
| 1 | Semantic hardening | `gpt-5.6-luna` / max | Close construction, reference, state, build identity, and content-validation gaps | Strict lint and focused tests green | Accepted after manager integration fixes |
| 1 | Enumeration hardening | `gpt-5.6-luna` / max | Make large action sets fallible, complete, stable, and tested | 256 executable actions; allocation failures explicit | Accepted |
| 1 | Split Tide content | `gpt-5.6-luna` / max | Author real two-area data and counterfactual integration tests | Compiler green; character, knowledge, and remote consequence proofs | Accepted |
| 1 | Whole-candidate audit | `gpt-5.6-luna` / max | Independent read-only review of the integrated foundation | Blocker list in a useful review window | Stopped after no timely result |
| 2 | Presentation/start-state | `gpt-5.6-luna` / max | Add production presets, contextual observations, and complete action paging | Focused kernel tests and production migration path | Accepted after manager hardening |
| 2 | Replay/session layer | `gpt-5.6-luna` / max | Add recording, receipts, JSON round-trip, verification, and resume | Determinism, field tampering, and parity tests | Accepted after binding genesis and observations |
| 2 | Production content migration | `gpt-5.6-luna` / max | Migrate Split Tide and its tests without engine edits | Strict compile plus counterfactual, prose, paging, and return proofs | Accepted after restoring the temporarily removed test gate |
| 2 | Presentation audit | `gpt-5.6-luna` / max | Review only start-state and presentation authority | Concrete severity-ranked findings | Accepted; manager closed production-boundary findings |
| 2 | Replay audit | `gpt-5.6-luna` / max | Review only trace integrity, transactionality, and malformed inputs | Concrete severity-ranked findings | Rejected candidate; manager closed panic and transition-forgery paths |
| 3 | CLI implementation | `gpt-5.6-luna` / max | Add the first complete terminal player adapter | Play, paging, search, save, replay, resume, tests | Stopped after no timely implementation; manager completed the lane |
| 3 | CLI boundary audit | `gpt-5.6-luna` / max | Review only authority, hidden output, persistence, navigation, and parsing | Severity-ranked current-tree verdict | Rejected first snapshot; persistence and coverage findings closed before acceptance |
| 4 | Evidence verifier audit | `gpt-5.6-luna` / max | Review only clean-process independence, witness binding, hidden fields, and tamper behavior | Severity-ranked current-tree verdict | Accepted; no P0/P1 findings, duplicated scenario registry removed |
| 6 | Outcome path mapping | `gpt-5.6-luna` / max | Derive shortest valid paths for five outcomes and two cross-area consequences | Exact recipes and persistent expectations | Accepted into the scenario registry |
| 6 | Scenario threat model | `gpt-5.6-luna` / max | Audit relabeling, recipe ambiguity, semantic assertion, and hidden-output risks | P0/P1/P2 findings for the witness expansion | Accepted; binding, exact parameters, postconditions, and substitution tests added |
| 6 | Evidence CLI review | `gpt-5.6-luna` / max | Review scenario naming and command scaling | Stable IDs without new client authority | Accepted; existing thin CLI retained |
| 7 | Scale architecture | `gpt-5.6-luna` / max | Design the smallest truthful 500-location fixture | Capacity proof with explicit non-breadth boundary | Accepted; terminal reciprocal ring selected |
| 7 | Scale benchmark/implementation | `gpt-5.6-luna` / max | Measure and implement an isolated scale report module | 500 canonical hops, paging, state admission, deterministic evidence | Accepted after manager added immutable budgets and stronger ID/topology/catalog checks |
| 7 | Scale threat model | `gpt-5.6-luna` / max | Audit claim scope, reachability, resource limits, and evidence forgery | Severity-ranked P0/P1 findings | Accepted after binding claim scope, exact digests, regeneration, and error tests |
| 8 | Blind-boundary architecture | `gpt-5.6-luna` / max | Define the smallest honest locked-player proof and its claim boundary | Mount, process, canary, and evidence contract | Accepted; scripted sandbox proof separated from an actual blind-AI session |
| 8 | Sandbox capability review | `gpt-5.6-luna` / max | Probe available Linux isolation and CI portability without changing the tree | Tested launcher profile and fail-closed recommendation | Accepted; Bubblewrap works locally and Docker remains only a pinned fallback |
| 8 | Blind-boundary threat model | `gpt-5.6-luna` / max | Audit leakage, escape, prompt, provenance, and false-claim risks | Severity-ranked P0/P1 findings | Accepted; CLI input and diagnostic leaks hardened before the sandbox harness |
| 8 | Locked-boundary candidate audit | `gpt-5.6-luna` / max | Review the integrated sandbox, canaries, CI, claims, and provenance | SHIP/NO-SHIP with only concrete blockers | Initial NO-SHIP; accepted after replacing circular player self-checking with a separately identified verifier and scanning persisted traces |
| 9 | Nondeterminism architecture | `gpt-5.6-luna` / max | Design representative ordering, entropy, hash, and manifest mutants | Exact seams, expected killers, and false-kill controls | Accepted; ordered crawl receipt and source-copy strategy integrated, broader corpus queued |
| 9 | Nondeterminism threat model | `gpt-5.6-luna` / max | Find permutations and process drift the existing evidence could miss | Severity-ranked gaps and independent-oracle criteria | Accepted; order-insensitive crawl projection closed before mutation claims |
| 9 | Mutation harness review | `gpt-5.6-luna` / max | Compare safe, fast mutation strategies for the existing CI bar | Maintainable commands with no production mutant hooks | Accepted; disposable source patch, shared target cache, locked builds, and focused process test selected |
| 10 | Character-creation architecture | `gpt-5.6-luna` / max | Design authoritative modular creation without accepting client-built character sheets | Typed recipe, merge rules, provenance, replay migration, and acceptance tests | Accepted; finite authored patches, canonical selections, and state provenance integrated |
| 10 | Creator CLI benchmark | `gpt-5.6-luna` / max | Design a concise terminal flow that preserves kernel authority | Command grammar, review/preview/back/cancel behavior, save shape, and focused tests | Accepted; `forge create` and public-recipe persistence integrated |
| 10 | Creation threat model | `gpt-5.6-luna` / max | Audit state admission, duplicate keys, tampering, replay, and evidence gaps | P0/P1 blockers plus minimum evidence | Accepted after closing arbitrary-genesis, duplicate-key, canonical-recipe, tamper, and custom-witness gaps |
| 11 | Live blind-boundary audit | `gpt-5.6-luna` / inherited | Review Codex authentication, model-visible context, callable tools, game authority, canaries, and reporting | Severity-ranked findings and fail-closed acceptance gate | Accepted; subscription-only auth and dual process isolation integrated, strict host-context limitation retained |

The first three benchmark tasks produced relevant results with little manager correction. Timing and cost were not exposed by the collaboration interface, so no numeric speed or cost comparison is claimed.

Bounded Luna/max implementation tasks produced useful patches quickly. Broad audit prompts were slower and twice required circuit breaking. Future review tasks should target one risk surface or one diff.

## Decisions

### D-001: Preserve source briefs in history, not the live tree

The root commit archives the four inputs. One current plan prevents stale documents from competing for authority while retaining exact provenance.

### D-002: Rust owns authority

The kernel, content compiler, replay verifier, and server adapter use Rust. A later React/TypeScript client remains a renderer. This trades some initial UI speed for explicit state types, controlled arithmetic, fast exploration, and one portable rules implementation.

### D-003: Prove a dense two-area loop before broad content

The Split Tide combines law/social systems with physical/hydraulic systems and a visible return consequence. Expansion before that loop is replayable would multiply uncertain content.

### D-004: Use strict founding text limits

The default sentence limit is eighteen words and action labels prefer one to three words. These stricter limits better serve fast play; exceptional longer text must be optional or evidence-backed.

### D-005: Count only contracted areas as scale

Generated topology is a capacity or travel substrate claim. It becomes shipped scale only after distinct mechanics, cast, interactions, outcomes, reactions, revisits, cross-area effects, and witnesses pass.

### D-006: Push every commit

Every commit is pushed to `origin/main` immediately. A commit that exists only in the local repository is not a completed integration step.

### D-007: Keep delegated patches small and recoverable

The first hardening task became too broad and was interrupted during a file rewrite, briefly breaking the shared tree. The manager restored it, split the work by file and risk surface, and added smaller acceptance gates. Future delegated edits should preserve a compiling boundary or create cheap checkpoints before broad rewrites.

### D-008: Production starts and observations are replay claims

Production compilation is required by a trusted caller rather than selected only by the content document. A production trace reconstructs its named preset or canonical authored creation recipe and seed, and every initial and post-action player observation is independently recomputed during replay. Raw structurally valid states remain useful kernel inputs, but they are not accepted as evidence without a verified trace lineage.

### D-009: Player saves are replay recipes, not internal witnesses

The detailed verifier trace contains authoritative state, events, entropy, and observations, so it stays behind the trusted player boundary. The portable player trace stores only its format and build, an authored preset or public canonical creation recipe and seed, opaque selected action identities, and final state and receipt commitments. Resume and replay rebuild genesis and every step through compiled content and the complete kernel-enumerated legal set. CLI saves use a bounded same-directory temporary file, durable file flush, atomic rename, and directory sync so a failed write does not first truncate a valid save.

### D-010: Evidence binds the game build and the verifier separately

The game build identity covers authoritative behavior and content. A separate verifier identity covers the evidence generator/checker source, manifest, build script, dependency lock, and toolchain. Checked witnesses bind both identities, a named scenario, the player-safe replay recipe, public observations, selected canonical definitions, and opaque fingerprints for legal sets, events, entropy, states, and receipts. This lets verifier-only improvements invalidate evidence without pretending they changed game behavior.

### D-011: Bounded crawl evidence is coverage, not exhaustiveness

The production crawler starts independently from every authored preset and follows deterministic, coverage-guided frontiers. At each expanded state it reconstructs the complete paged catalog, compares public action views to kernel enumeration, executes every advertised canonical action through the reducer, renders the resulting observation, and validates the resulting state. Depth, expanded-state, discovered-frontier, and action-execution budgets are serialized into the checked report. Success requires every authored definition, but does not claim every reachable world state was explored.

### D-012: A Sluice outcome is a one-way world decision

All five authored Sluice outcomes share one authoritative `sluice_outcome_chosen` gate. Selecting any outcome sets that flag before its branch-specific consequences, so later play cannot combine incompatible water states. The crawler must still reach and execute every outcome on separate branches.

### D-013: Scenario names bind reviewed recipes and semantic claims

The verifier owns one registry for the exact reviewed scenario-ID set, canonical preset or custom start and seed, exact ordered actions with canonical complete parameter maps, and hidden postconditions. Each public witness carries an opaque digest of that specification in addition to game and verifier identities. Generation and checking require the exact recipe, validate persistent world, location, character, visit, observation, and exclusivity expectations, and reject missing, substituted, or duplicate IDs, claims, or recipes. Registry-driven process tests require exactly one checked file per scenario.

### D-014: Synthetic scale is capacity evidence, never shipped breadth

The scale verifier owns a fixed 500-location terminal reciprocal ring and one adjacent-travel definition. Its checked report binds the fixture source, compiled build, verifier, exact location sets, topology, every page of every per-hop catalog, 500 canonical transitions, and immutable graph-frontier, catalog-work, and serialization budgets. Host timing is deliberately not an acceptance input; the checked ceilings are deterministic across machines. The artifact identifies itself as generated substrate and explicitly disclaims authored breadth, NPC or mechanic depth, area quality, and Skyrim parity.

### D-015: Public diagnostics are a deliberately narrow contract

The player process accepts at most 4 KiB per input line and 1,024 lines per session. It reports stable error classes without reflecting host paths, operating-system messages, replay mismatch paths, kernel details, or rejected trace text. Save confirmations omit paths, and replay display omits the final-state identifier while retaining the opaque final receipt. Trusted tooling keeps the detailed diagnostics required to investigate failures outside the player boundary.

### D-016: Isolation evidence and blind-player evidence are separate

The Linux acceptance gate builds a release player in a dedicated target, strips symbols, copies only that executable and its resolved runtime files, and launches it through a fail-closed Bubblewrap profile. The process runs as UID/GID 65534 with no capabilities, `no_new_privs`, nested user namespaces disabled, a read-only root, a cleared environment, an isolated network namespace, no host tools or repository mount, and only `/session` writable. Resource and boundary probes test host canary reads and writes, symlink escape, network reachability, shell absence, memory, descriptors, processes, output, wall time, input size, session length, malformed replay, EOF, deterministic repetition, and trusted replay of the resulting save.

The generated local report identifies itself as a locked CLI surface and isolation probe. It explicitly records that no blind AI, model adapter, per-session embedded secret, or reverse-engineering defense was tested. Collaboration agents retain workspace access, so instruction-only blindness from those agents does not satisfy the product requirement.

The sandboxed player does not certify its own result. A separately built `forge-verify check-player` executable reconstructs the exact player-safe trace through production content and the authoritative kernel, verifies its final commitments, and emits its compile-time verifier identity. The local report binds that checker identity and binary hash independently from the player binary hash.

### D-017: Nondeterminism mutants run only in disposable source copies

The mechanical bar copies current workspace inputs into a temporary directory, applies reviewed mutation hooks there, and builds them with a separate target. Production crates contain no feature-gated or dormant mutant branch. The mutated verifier first regenerates a selector-free crawl and must pass the same fresh-process check; this neutral control prevents changed game or verifier identities from becoming false mutation kills.

Four selectors then introduce undeclared ambient dependence into kernel action order, paged catalog order, first-frontier scheduling, and a process-specific crawl receipt. The checked process test must fail for an expected crawl-contract reason in every case. The production crawler independently requires strictly ascending canonical action IDs, checks every page against that vector, and chains ordered starts, expansions, catalogs, events, entropy, and transitions into its checked execution receipt. Aggregate coverage equality alone is no longer sufficient evidence of deterministic exploration.

### D-018: Character creation is an authored recipe, not a writable sheet

Production content defines a bounded set of ordered slots and typed character patches. The player supplies a short safe name plus exactly one public choice ID per slot; the kernel canonicalizes the vector, rejects missing, unknown, duplicate, conflicting, cosmetic-only, or out-of-bounds definitions, and derives the complete character. It never accepts client-supplied aptitudes, tags, inventory, history, reputation, or facets.

Every state carries character-start provenance that no authored effect can mutate. Production state admission reconstructs the named preset or custom recipe against the exact build and compares all non-progressed character fields. Custom IDs hash the build and canonical selection. Detailed and player-safe replay formats bind the same canonical recipe and seed, while portable saves omit the derived character and hidden world. Source ordering is normalized, semantic patch changes alter the build ID, reserved typed axes cannot be shadowed by extensible facets, and all JSON transport boundaries reject duplicate object keys before typed deserialization.

### D-019: Live model play uses saved subscription authentication only

The live runner accepts only a saved Codex login whose status is `Logged in using ChatGPT`. It removes `OPENAI_API_KEY` and `CODEX_API_KEY` from every authentication check and runs Codex inside a cleared environment, so an ambient key cannot override or rescue a failed subscription login. There is deliberately no API-key fallback. The manager's Vercel plugin is installed globally but never copied, loaded, or exposed to the isolated player session.

### D-020: The model and deterministic game use separate confinement

The release `forge-player-mcp` process owns the session and maps a public ordinal back to the current complete kernel-enumerated action set before recording it. It exposes only `observe`, `act`, and `finish`, requires an explicit successful finish, and persists a player-safe trace after every accepted action. The game process receives a stripped embedded-content bundle, no network, no repository, no authentication, and one writable session directory.

Codex runs from a copied static executable in a separate Bubblewrap mount namespace. It needs inference network access, but receives no repository, source, game binary, default configuration, session history, or plugin tree. A fixed audited proxy connects its sole MCP server to the locked game over a Unix socket. A public observation nonce proves delivery; an unmounted private source canary is required to remain absent. The runner records prompt context, tool configuration, JSONL events, latency, token use, model findings, hashes, and an independent `forge-verify check-player` result.

Codex still injects generic host skill and developer messages even when every callable development feature is disabled. Therefore the pending report may claim a source-isolated honest model session when its gates pass, but not strict conformance to the stronger interpretation that every model-visible token came only from the player protocol.

## Risks requiring early tests

- Canonical hashing may omit an authoritative input or depend on serialization details.
- The condition/effect DSL may become either arbitrary code or shallow tag substitution.
- Stable action enumeration can still overwhelm players even without truncation.
- Character variants may change labels while leaving strategy unchanged.
- NPC knowledge can leak from global flags instead of credible transfer.
- Region-oriented storage can accidentally weaken single-world continuity.
- Verification could overstate bounded exploration or become editable theater.
- Direct kernel calls can evaluate structurally valid states without proving their lineage; evidence claims must enter through verified sessions and traces.

## Current verification snapshot

`./verify` passes formatting, warnings-as-errors, all workspace tests, the subscription-auth policy check, four source-copy nondeterminism mutants after a passing neutral control, the locked-player process rehearsal, and whitespace checks. The snapshot contains 31 kernel tests, 2 content-boundary tests, 10 real-content integration tests, 12 replay unit tests, 2 real Split Tide replay tests, 19 CLI/player-adapter tests, 17 evidence-verifier unit tests, and 3 clean-process integration tests: 96 Rust tests plus the auth policy check, 19 isolated player-boundary process invocations, and 4 deliberately killed mutants. It covers all 64 creator combinations, per-axis legal-action differences, preset-extreme equivalence, typed patch conflicts and bounds, custom provenance forgery rejection, canonical selection ordering, duplicate-key JSON rejection, custom save/replay/resume parity and tampering, deterministic recording, hidden-field omission across all scenarios and the scale report, malformed-state rejection, preset/custom genesis reconstruction, observation binding, atomic replacement and failed-install preservation, exact save-size and player-input boundaries, sanitized public failures, session line limits, independent player-trace checking, complete current-catalog CLI paging/search, the three-tool MCP surface, explicit-finish enforcement, Unix-socket transport, subscription-only authentication, independent canonical catalog ordering, ordered crawl execution receipts, all five exclusive outcome selections, independent crawler resource budgets, exact scenario parameters and reviewed scenario-set enforcement, duplicate registry rejection, relabel and alternate-path rejection, byte-identical checked witness, crawl, and scale evidence from separate processes, and the locked boundary probes described in D-016. The boundary harness emits a player-safe local transcript, trace, independent trusted check, bundle hash manifest, and limitation-bearing report under `artifacts/local/locked-player-boundary/`. The eleven checked scenario files cover both preset proofs, two mechanically mixed custom characters, all five outcome-to-return paths, and representative paths through both authored areas. The production crawl reaches every current location and definition while executing every canonical action advertised by its 44 expanded states. The separate capacity fixture validates exactly 500 synthetic locations, all 500 complete per-state paged catalogs, and 500 canonical hops under fixed deterministic resource ceilings. Fresh processes reproduce and recheck `evidence/scale/synthetic-ring-500.json` byte for byte. Neither bounded report claims exhaustive production state-space coverage, authored world breadth, comparative area depth, or an accepted live blind-player session. The project does not yet cover an accepted live model run, the full mutation corpus, the full text policy, final game scale, or comparative area depth.

## Next reassessment trigger

Reassess after the first actual blind-capable player session through the locked boundary. If players cannot understand consequences from bounded observations, prioritize presentation and action feedback before expanding the map.
