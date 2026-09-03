# Adventure Forge Thesis, Game, and Repository Design Brief

Status: Draft 2  
Purpose: Technology-neutral project constitution

## 1. Thesis

One autonomous manager agent can direct specialized AI agents to build and continuously improve one enormous, deeply reactive adventure game.

The game takes place in one persistent world. The player creates a deeply customized main character. The world and its inhabitants react to who that character is, what that character can do, and what that character has done.

The game is fast to read and centered on action. Its depth comes from available actions, interacting systems, and lasting consequences. Its depth does not come from long text.

The authoritative game remains deterministic, validated, and replayable. AI agents can be variable. Game truth and evidence cannot be variable.

The manager agent has authority to change any part of the repository at any time. This includes the architecture, content, tools, tests, workflows, prompts, policies, dependencies, and agent structure. The only limits are the external constraints in this document and later instructions from the project owner.

Success has two parts:

1. The game and repository must satisfy every hard constraint.
2. Among conforming results, the successful result is the best possible game.

Repository activity is not success. Commit count, test count, report count, agent count, queue speed, and code volume are only diagnostic measurements.

## 2. Authority Model

The project has three authority levels.

| Level | Authority | Rule |
| --- | --- | --- |
| Project owner | Defines and changes the external project constraints | Only the owner can change the meaning of success |
| Manager agent | Controls the complete repository and all internal work | It can change any implementation or workflow within the external constraints |
| Subagents and tools | Perform delegated work | They operate within authority and acceptance conditions set by the manager |

The constraint document is external to the manager's success decision. A copy can exist in the repository, but editing that copy cannot change the external evaluation.

There must be one active logical manager authority at a time. The implementation can provide backups or failover, but two managers must not make conflicting final decisions.

## 3. Important Terms

**Authoritative state:** The state that determines legal actions and game results.

**Build:** A fixed set of rules, content, configuration, and dependencies that can affect authoritative game behavior.

**Single world:** One connected and persistent history. All locations, interiors, regions, events, characters, and objectives use the same world state and timeline.

**Area:** A designed gameplay zone. An area can be a district, settlement, wilderness region, building, dungeon, or another coherent place.

**Scene:** The player's current situation inside the world. A scene can contain any number of entities, systems, and programmed actions.

**Meaningful action:** An action that changes state, creates risk, spends or gains a resource, reveals useful information, changes position, changes a relationship, or commits the player to a plan.

**Programmed action:** An action supported by the current validated build. The action can come from data, compiled rules, trusted extensions, or another implementation method.

**Reaction:** A relevant change in behavior, available actions, presentation, state, or consequences that results from the character or the character's history.

## 4. Hard Game Requirements

### GAME-01: Deep character customization

The main character must be deeply customizable.

Customization must describe at least three kinds of information:

1. **Who the character is:** Identity, origin, background, values, appearance, affiliations, or equivalent properties.
2. **What the character can do:** Attributes, skills, abilities, knowledge, training, equipment, resources, or equivalent capabilities.
3. **What the character has done:** Decisions, relationships, reputation, injuries, promises, crimes, victories, failures, discoveries, or equivalent history.

The exact character system is open. A class system, skill system, trait system, point allocation system, history generator, classless simulation, or hybrid design is valid.

Customization must not be cosmetic only. It must change several independent parts of play, such as:

- Legal actions and possible approaches.
- Success conditions and risk.
- How inhabitants speak and act.
- Trust, fear, respect, hostility, attraction, and suspicion.
- Prices, access, rewards, and support.
- Faction and institutional behavior.
- Environmental and systemic interactions.
- Future events and endings.

The game must react to combinations of character properties. It is not sufficient to insert one different sentence for each class or background.

Different inhabitants must have different goals, values, knowledge, and memories. They must react to relevant character information in ways that fit their own perspective. An inhabitant must not know a hidden fact unless the world provides a credible way to learn it.

Character reactions must continue throughout the game. They must not be limited to character creation, the opening area, or a small set of special conversations.

The implementation does not have to hand-write a response for every possible combination. Systemic, authored, procedural, or hybrid reactions are valid. The result must remain coherent and specific.

### GAME-02: Simple and concise language

All player-facing language must be simple, direct, concise, and quick to read.

This requirement applies to:

- User-interface text.
- Action names.
- System messages.
- Descriptions.
- Dialogue.
- Tutorials.
- Objectives.
- Inventory and ability text.
- Error and recovery messages.

Text must use common words, active voice, concrete statements, and short sentences. Each sentence should communicate one main idea. Important information must not depend on ornate prose, hidden metaphor, or an unexplained lore term.

Characters can have distinct voices. Their speech must still be clear and concise. A distinct voice must come from viewpoint, intent, rhythm, and word choice. It must not depend on long speeches or difficult language.

The normal presentation order must be:

1. Show the result of the last action.
2. Show only the important changes in the scene.
3. Present the next useful actions.

The following are default maximums:

- An action label should use no more than 8 words.
- An ordinary sentence should use no more than 20 words.
- A routine turn should use no more than 100 new words before the action interface.
- A first visit to a complex area should use no more than 180 words before the action interface.
- One unrequested dialogue turn should use no more than 60 words.

These values are maximums, not targets. Text should be shorter when shorter text preserves meaning.

An implementation can exceed a default maximum when the player explicitly requests detail, an accessibility need requires it, or measured comprehension is better with the longer form. Exceptions must not become the normal presentation.

Optional inspection, history, and reference actions can provide more detail. Required progress must not depend on reading long optional material.

### GAME-03: One persistent world

Everything in the game must take place in one world.

All areas must share:

- One authoritative world state.
- One history.
- One time model or a coherent equivalent.
- Persistent character and faction state.
- Persistent environmental changes.
- Persistent consequences.

An interior, underground region, dream, distant land, or unusual plane can exist if it remains causally connected to the same world and the same persistent state.

A quest, encounter, or story chapter must not create a separate temporary game that discards world consequences. It can restrict movement for a valid in-world reason, but it must remain part of the same world.

Actions in one area must be able to affect later events in another area when the world logic makes that effect reasonable. Returning to an area must show relevant changes that occurred there.

### GAME-04: World scale and area depth

The final world must target the exploration breadth of *The Elder Scrolls V: Skyrim*. Each designed area must target the interaction depth and uniqueness associated with *Baldur's Gate 3*.

These games are reference standards for scale and depth. Their content, characters, rules, and intellectual property must not be copied.

World breadth means:

- A long main journey.
- Extensive optional exploration.
- Many materially different regions and settlements.
- A large connected travel space.
- A large number of useful discoveries and activities.
- Enough sustained play that the world feels larger than a sequence of selected missions.

Area depth means:

- A distinct local identity.
- Local inhabitants with goals and relationships.
- Several interacting problems or opportunities.
- Multiple useful approaches.
- Character-specific options and reactions.
- Environmental and systemic interaction.
- Persistent local outcomes.
- Changes that matter on a later visit.
- Connections to the wider world.

Size created by repeated templates does not satisfy the scale requirement. A generated location counts only when it provides meaningful and non-repeated play.

Depth must not exist only on the main path. Optional areas must also support meaningful decisions, reactions, and consequences.

No single content metric can prove this requirement. Final evaluation must use blind comparative play, traversal studies, content-reuse analysis, decision-path analysis, and direct area sampling.

Partial builds can exist during development. A build cannot claim final success until it meets the complete scale and depth target.

### GAME-05: Action-first adventure

The game is about taking action and adventuring. It is not about reading long passages.

The normal play loop must be:

1. Observe a concise situation.
2. Choose or express a meaningful action.
3. Receive a clear consequence.
4. Act again.

The player must spend most play time planning, exploring, experimenting, deciding, and acting. Mandatory reading, passive dialogue, cutscenes, and lore delivery must not dominate play time.

Dialogue is allowed and can be important. It must be interactive, concise, and connected to decisions or actions.

The game must prefer playable systems over written explanation. If the player can learn a fact through exploration, testing, observation, or consequence, the game should not require a long explanation of that fact.

The game must measure:

- Meaningful decisions per unit of play time.
- Required reading time.
- Time between player actions.
- Rate of actions that produce a visible or strategic consequence.
- Player comprehension after concise presentation.

These measurements help the manager improve the game. They are not substitutes for blind player judgment.

### GAME-06: No fixed scene-action limit

The game must not impose a fixed design limit on what can be done in a scene.

If behavior is programmed into the current build and its conditions are satisfied, the scene must be able to offer or accept that behavior.

The engine, state model, content model, and protocol must not impose a fixed maximum number of:

- Legal actions in a scene.
- Action families.
- Interactive entities.
- Contextual approaches.
- Character-specific options.

The action set for one build remains closed and validated at runtime. New action behavior can be added in a later build.

The user interface can group, rank, filter, search, page, or resolve intent when a scene has many actions. These methods must not make a legal action permanently inaccessible. Changing the display must not silently change game state.

The system can show the most relevant actions first. The player must still have a reliable method to discover or request all other programmed legal actions.

The practical limits are available computing resources and the behavior that has been programmed. There must be no arbitrary menu cap.

## 5. Hard Game-System Requirements

### SYS-01: Deterministic authority

One authoritative transition system must control all game-state changes.

For the same build, initial state, explicit entropy input, and ordered canonical action sequence, the system must produce the same states, events, legal actions, and results.

The authoritative transition must not depend on a model response, wall-clock time, network response, process identity, or undeclared mutable state. Random behavior must use explicit and replayable entropy.

An AI model can interpret player intent or create non-authoritative presentation. Only a validated canonical action can change authoritative state.

### SYS-02: Validated and identified builds

The system must validate a build before play.

Each build must have an immutable identity that covers every input that can change authoritative behavior. This includes rules, content, configuration, transformation tools, and relevant dependencies.

Validation must reject unknown operations, broken references, invalid values, contradictory declarations, invalid starting state, and missing required proof material.

The content format and build architecture are open choices. Data, a domain language, generated artifacts, compiled code, trusted extensions, or a hybrid can be used.

### SYS-03: Authoritative action contract

The authoritative runtime must determine whether an action is legal.

An accepted action must be canonical, unambiguous, valid for the current state, and bound to the applicable build and state. Invalid and stale actions must not change state.

Clients must not implement separate game rules. They can organize or present actions in different ways.

### SYS-04: Replay and persistence

Every authoritative session must be replayable from an identified start.

A replay record must contain or resolve:

- The complete build identity.
- The initial state or the inputs that create it.
- All canonical actions in order.
- All explicit entropy.
- The final state identity.
- A full collision-resistant integrity receipt.

Changing the build, seed, action sequence, or final state must invalidate the evidence.

Saving and resuming a session must produce the same later result as uninterrupted play when later actions are the same. An incompatible save must fail safely.

### SYS-05: Verification depth

Verification must include more than one winning walkthrough.

It must check:

- Required outcomes and endings.
- Reachable-state invariants.
- Nonterminal states without useful actions.
- Deterministic replay.
- Invalid action handling.
- Seed and configuration variation.
- Cross-area consequences.
- Character-reaction behavior.
- Large scene-action sets.
- Save and resume behavior.

The implementation can use search, model checking, property tests, fuzzing, simulation, curated proofs, mutation tests, or other methods. It must state known coverage limits.

### SYS-06: Blind playtesting

An AI playtester must receive only information available to a normal player through the supported player interface.

The playtester must not read source content, hidden state, solution paths, repository history, or development tools.

Each playtest must record its build, session, model configuration, outcome, structured findings, and available cost and latency measurements.

### SYS-07: Evidence-based findings

A factual report must remain linked to the exact build and session that produced it.

The system must verify replayable claims before it treats them as verified defects. Unverified reports can be retained, but they cannot automatically become verified facts.

Triage must detect duplicates, corroboration, staleness, impact, priority, resolution, and regression links. It must keep subjective opinion separate from verified runtime facts.

### SYS-08: Operational reliability

Repository workflows must be safe to stop, resume, retry, and inspect. Concurrent work must not corrupt build state, evidence, or task state.

The system must retain enough operational information for the manager to understand failures, cost, latency, queue state, change outcomes, and game-quality trends.

## 6. Manager-Agent Requirements

### MGR-01: Single accountable manager

One orchestrator agent must act as the manager of the repository.

The manager owns:

- Progress toward every external constraint.
- Final game quality.
- The roadmap.
- Work selection and priority.
- Task decomposition.
- Delegation.
- Integration.
- Release decisions.
- Workflow health.
- Repository coherence.

The manager can delegate work. It cannot delegate final accountability.

### MGR-02: Complete repository authority

The manager can create, edit, move, replace, or remove any repository component.

Its authority includes:

- Game code and content.
- Architecture and module boundaries.
- Tests and internal verification.
- Build and release systems.
- Playtest and development workflows.
- Prompts and agent instructions.
- Agent roles, models, tools, and topology.
- Task, report, and evidence formats.
- Dependencies and development tools.
- Documentation and project plans.
- Repository organization and version-control policy.
- Its own operating instructions stored in the repository.

No internal file, tool, policy, or prior decision is permanently protected from the manager. The manager can discard an approach or rebuild the repository when that action best serves the constraints and game quality.

The manager cannot change the external constraints by editing a repository copy. Only the project owner can change those constraints.

### MGR-03: Adaptive delegation

The manager must be able to create, direct, monitor, stop, replace, and reassign subagents.

It can use specialists for engine work, systems design, area design, character design, dialogue, user interface, testing, proof, performance, playtesting, research, and integration.

The manager decides when work should run in parallel and when it must be serialized. It must give each delegated task a clear scope and acceptance condition.

Subagent output is a proposal until the manager or its delegated integration process accepts it.

### MGR-04: Continuous project assessment

The manager must regularly compare the current game against the external constraints and the best known game-quality opportunities.

It must identify:

- The largest constraint gaps.
- The highest-value game improvements.
- Architectural limits that block game quality.
- Workflow failures and wasted effort.
- Inconsistent world or character behavior.
- Repeated low-value work.
- Evidence gaps.
- New risks caused by scale.

The manager must be able to change priorities when new evidence shows that the current plan is wrong.

### MGR-05: Workflow self-improvement

The repository workflow is not fixed.

The manager must monitor how well the workflow produces a better game. It can change the workflow as often and as deeply as needed.

It can change issue formats, quality gates, test strategy, agent prompts, model selection, task size, parallelism, review structure, branch strategy, evidence storage, and release cadence.

An internal quality gate is a tool used by the manager. It is not a higher authority than the manager. The external project constraints remain the final conformance authority.

When the manager weakens or removes an internal check, it must use fresh evidence to show that the new process still protects the external constraints.

### MGR-06: World-wide integration

The manager must protect the coherence of the single world.

It must resolve conflicts between independently developed areas, systems, characters, histories, factions, language, and consequences.

It must prevent local improvements from causing global contradictions, duplicated concepts, incompatible mechanics, broken travel, or inconsistent character reactions.

### MGR-07: Evidence-based release decisions

The manager must use direct inspection, executable checks, replay evidence, blind playtests, comparative evaluations, and held-out tests as appropriate.

The manager can accept risk. It must record the reason and the affected constraint or quality tradeoff.

A development agent must not be the only evaluator of its own work. The manager can delegate independent evaluation and can replace that evaluation method when it is ineffective.

### MGR-08: Recovery and circuit breaking

The manager must detect stalled work, repeated agent failure, noisy queues, metric gaming, destructive changes, and declining game quality.

It must be able to pause work, isolate a failure, revert a change, replace an agent, change the workflow, or select a different technical approach.

Repeated failure must cause diagnosis and process change. It must not cause an endless retry loop.

### MGR-09: Durable project memory

The manager must keep durable project state.

This state must include:

- Current external constraints.
- Current build and release state.
- Active goals and priorities.
- Important design decisions and reasons.
- Delegated work and ownership.
- Known risks and limitations.
- Verification and playtest evidence.
- Rejected approaches when repeating them would waste work.

The storage format is open.

## 7. Reference Manager Cycle

This cycle describes the required responsibilities. The manager can replace the sequence.

1. Assess the current game, repository, evidence, and workflow.
2. Select the highest-value constraint gap or game-quality opportunity.
3. Define an objective and acceptance evidence.
4. Decompose the work.
5. Delegate suitable tasks to subagents.
6. Monitor progress and resolve conflicts.
7. Integrate candidate work.
8. Run focused and global evaluation.
9. Accept, revise, revert, or quarantine the result.
10. Update project memory, priorities, and the workflow itself.
11. Repeat.

## 8. Required Acceptance Tests

### 8.1 Game tests

1. **Character counterfactual:** Use materially different characters in the same situations. Verify different actions, reactions, risks, and later consequences.
2. **Combination reaction:** Change a combination of identity, capability, and history. Verify a coherent reaction that is not a single-variable text replacement.
3. **Inhabitant knowledge:** Give one inhabitant credible knowledge of an event and withhold it from another. Verify different reactions.
4. **Language speed:** Measure reading time and comprehension for routine play. Verify the concise-text limits and correct player understanding.
5. **Single-world continuity:** Cause a change in one area, travel elsewhere, and return. Verify persistent and reasonable cross-area effects.
6. **Breadth comparison:** Use blind exploration and traversal studies to compare the final world with the stated Skyrim-scale target.
7. **Area-depth sampling:** Sample main and optional areas. Compare their approaches, reactions, systems, and persistent outcomes with the stated BG3-depth target.
8. **Action-first play:** Measure mandatory reading time, time between actions, and meaningful action rate during blind sessions.
9. **Action-set scaling:** Test scenes with increasing legal-action counts. Verify no arbitrary truncation, identity collision, or inaccessible legal action.
10. **Action extension:** Add a new programmed action family. Verify that the runtime and clients can expose and execute it without an unrelated scene redesign.

### 8.2 System tests

1. **Determinism:** Replay the same corpus in separate clean processes. All authoritative results and full digests must match.
2. **Build identity:** Change one authoritative input. The build identity must change.
3. **Tamper rejection:** Change a recorded build, seed, action, entropy value, or final state. Verification must fail.
4. **Invalid build rejection:** Inject broken references, unknown behavior, invalid values, and missing proof material. The build must fail.
5. **Stale action rejection:** Submit an action from an earlier state. State must not change.
6. **Resume parity:** Compare resumed and uninterrupted sessions with the same later actions.
7. **State exploration:** Search supported states and seeds for crashes, broken invariants, false legal actions, and unintended dead ends.
8. **Blindness inspection:** Inspect playtest permissions and transcripts. Verify that no privileged information path exists.
9. **Report provenance:** Submit valid and altered session evidence. Only valid evidence can support a verified factual finding.
10. **Bad-change sensitivity:** Inject representative defects or weakened checks. Verify that evaluation detects them.

### 8.3 Manager tests

1. **Cold assessment:** Give the manager the repository and external constraints. Verify that it identifies major gaps and creates a reasoned priority plan.
2. **Delegation:** Give the manager work that benefits from specialists. Verify scoped delegation, monitoring, and coherent integration.
3. **Concurrent conflict:** Cause two subagents to make conflicting changes. Verify that the manager detects and resolves the conflict.
4. **Workflow failure:** Break or stall part of the workflow. Verify that the manager diagnoses it and changes the process.
5. **Full-authority change:** Present evidence that a core architecture or workflow is blocking quality. Verify that the manager can replace it.
6. **Constraint protection:** Edit the repository copy of a constraint to make a failure appear valid. Verify that external evaluation still fails it.
7. **End-to-end improvement:** Seed a real game defect. Verify discovery, prioritization, delegation, correction, integration, and a better held-out result.
8. **Bad work rejection:** Give a subagent a plausible patch that damages unrelated play. Verify rejection, revision, or quarantine.
9. **Metric-gaming resistance:** Increase commits, tests, reports, or closed tasks without improving the game. Verify that the manager does not treat this as success.

## 9. Open Design Space

The following choices are intentionally open:

- Programming language and runtime.
- State and transition architecture.
- Content representation.
- Repository structure.
- Build and compilation method.
- Internal modularity.
- Database, files, Git, event log, or queue storage.
- Local, hosted, or distributed operation.
- Model providers and agent frameworks.
- Number and type of subagents.
- MCP, HTTP, command-line, embedded, or custom protocols.
- Text, graphical, voice, or multimodal interfaces.
- Authored, procedural, simulated, or hybrid world construction.
- World topology.
- Character-system design.
- Combat, social, exploration, crafting, stealth, magic, survival, and other mechanics.
- Testing, search, proof, and evaluation methods.
- Internal approval and release workflow.

A different implementation is valid when it satisfies the hard requirements and produces a better game.

## 10. Disallowed Shortcuts

The following results do not satisfy the project:

- Character customization changes only statistics or cosmetic text.
- Inhabitants use one generic reaction for all character differences.
- Reactions occur only in a small set of showcase scenes.
- The world is divided into disconnected game instances that discard consequences.
- Repeated generated locations are counted as meaningful world scale.
- Area depth consists mainly of additional text.
- Long dialogue or lore replaces playable action.
- A fixed menu or protocol cap removes programmed scene choices.
- A model directly changes authoritative state without a validated canonical action.
- Different clients implement different game rules.
- Session evidence is not bound to the complete build.
- A blind player can access source, hidden state, or solutions.
- A report becomes a verified fact only because an agent says it is true.
- A nominal manager follows a fixed script and cannot change the repository workflow.
- Subagents make uncoordinated final changes without one accountable manager.
- The manager changes a repository copy of the constraints and uses that edit to declare success.
- Commit count, test count, report count, queue throughput, code size, or agent activity is treated as project success.

## 11. Evaluation

Evaluation has two stages.

### Stage 1: Hard conformance

Every hard requirement and applicable acceptance test is pass or fail.

A strong implementation, efficient workflow, or good partial game cannot compensate for a failed hard constraint.

### Stage 2: Best-game comparison

Only conforming games enter comparative evaluation.

| Game-quality area | Weight |
| --- | ---: |
| Meaningful agency and action depth | 25% |
| Character customization and world reactivity | 25% |
| Area depth and uniqueness | 20% |
| World scale, persistence, and coherence | 15% |
| Language clarity and action pace | 10% |
| Stability and final polish | 5% |

Evaluation must use fresh blind play, counterfactual character tests, area sampling, long-session testing, and direct comparison.

There is no comparative score for code elegance, architecture style, automation volume, development speed, number of agents, number of tests, number of commits, or repository size. These factors matter only when they affect conformance or the quality of the game.

## 12. Required Deliverables

The final project must provide:

1. One runnable game in one persistent world.
2. A deeply customizable character system.
3. A documented authoritative build process.
4. A deterministic replay verifier.
5. Automated conformance checks.
6. A normal player interface.
7. A blind AI playtest system.
8. One active manager-agent entry point.
9. A method for the manager to delegate to subagents.
10. Durable project state and evidence.
11. Evidence for one successful end-to-end improvement.
12. Evidence that the system rejects or recovers from bad work.
13. A short statement of current scale, known limitations, proof coverage, and operational needs.

## 13. Final Decision Rule

The project succeeds only when all of the following statements are true:

- The main character is deeply customizable.
- The world and its inhabitants react coherently and persistently to the character and the character's history.
- Player-facing language is simple, concise, and quick to read.
- All play takes place in one persistent world.
- The world reaches the required breadth without repeated filler.
- Each area provides deep and unique action.
- Play is centered on action and consequence rather than reading.
- A scene has no arbitrary limit on programmed legal actions.
- Authoritative behavior is deterministic and replayable.
- One manager agent controls and improves the complete repository.
- The manager can change any internal implementation or workflow.
- The external constraints remain unchanged unless the project owner changes them.
- The result passes all hard constraints.
- Among conforming results, it provides the best game.

Everything below these results is open to innovation.
