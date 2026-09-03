# Adventure Forge Agent Charter

This repository has one active logical manager. The manager owns priorities, architecture, integration, verification, release claims, and final game quality. Delegated work is a proposal until the manager accepts it.

Read `PLAN.md` and `PROJECT_STATE.md` before changing the repository.

## Fixed boundaries

- The authoritative game is deterministic code. A model never decides game truth at play time.
- Only a kernel-enumerated canonical action can change state.
- Content uses the closed typed condition/effect vocabulary. Prose cannot hide mechanics.
- All places share one world state, timeline, and persistent history.
- There is no fixed cap on programmed legal actions.
- Player text stays short, direct, concrete, and action-first.
- Game, save, outcome, and defect claims require build-bound replay evidence.
- Blind players cannot receive source, hidden state, solutions, or builder tools.
- Never weaken a verification requirement merely to admit a change.

These boundaries can be implemented differently, but their meaning cannot be changed without an explicit project-owner instruction.

## Working rules

1. Start from the highest-value gap recorded in `PROJECT_STATE.md`.
2. State acceptance evidence before making a material change.
3. Keep authoritative logic in Rust kernel crates. Clients render observations and submit action identities; they do not duplicate rules.
4. Use stable namespaced identifiers, integer or defined fixed-point state, ordered collections, and explicit entropy.
5. Treat content volume as unshipped until its area contract and witnesses pass.
6. Add focused tests with every new condition, effect, action family, persistence behavior, or bug fix.
7. Run the narrow tests while iterating and the repository `./verify` gate before accepting integrated work.
8. Record material architecture, workflow, evidence, scope, or risk changes in `PROJECT_STATE.md`.
9. Preserve unrelated user work and do not rewrite another active agent's scoped files.
10. Do not claim Skyrim breadth, BG3 area depth, blindness, or full conformance from fixtures or indirect evidence.
11. Push every commit to `origin/main` immediately. A local-only commit is incomplete work.

## Content rules

- Action labels: one to three words preferred; eight maximum.
- Ordinary sentences: eighteen words maximum.
- Area description: one or two short sentences.
- Routine observation: below one hundred new words before actions.
- Complex first visit: below one hundred eighty words.
- Unrequested dialogue: one short turn; sixty words maximum.
- Each live scene normally has two or more meaningful non-movement actions.
- Every NPC fact needs a credible knowledge source.
- Character reactions should combine perspective, identity, ability, visible state, and history where relevant.
- Every dense location needs state-changing interactables; every counted area needs distinct mechanics and persistent consequences.

## Definition of done for a change

A change is accepted only when its requested behavior is present, focused checks pass, relevant old evidence still passes, no client authority or hidden nondeterminism was added, and durable project state reflects any material consequence. Passing tests is evidence for what those tests cover, not proof of the entire project goal.
