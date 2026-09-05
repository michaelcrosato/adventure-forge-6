# Fume Yards: Cinder Batchworks

Status: the cold pilot and controlled-manufacture slice are accepted through cycle 32. The complete district remains unshipped.

The first optional expansion is one small industrial district: Cinder Batchworks. Its three locations share finite stock, a kiln batch, working crews, and freight. It grows the world through material conversion, process timing, salvage, and competing uses for useful goods. It must not become another permit hunt followed by five exclusive ending buttons.

## Acceptance before implementation

Accept the complete district only when all of the following have build-bound evidence:

- Three connected locations, four named inhabitants, and the nine physical interactables below work through canonical actions.
- Four approach families have distinct required operations, resource costs, risks, and persistent results. Different prose or access credentials do not establish another family.
- Materials cannot be duplicated, consumed twice, sold after installation, or replenished by leaving and returning.
- Heat progresses on the existing world timeline after the player starts a batch. Leaving, saving, and returning cannot pause it.
- A player can ignore the district, visit during Split Tide, enter after any tide outcome, or first enter after turn 128.
- Local production choices create concrete new options or costs in Lowsail. They do not reverse an existing tide ending.
- NPC reactions have replayed knowledge sources, including an explicit uninformed comparison.
- The witness matrix below passes alongside every existing regression predicate and the repository `./verify` gate.

These are district acceptance requirements, beyond the current compiler's static action-density check. Production compilation currently requires two obviously possible meaningful non-movement actions per nonterminal location. It does not prove four approaches, three distinct objects, reachable density throughout play, NPC schedules, or comparative depth.

## The problem the player encounters

The batchworks has banked its kiln while a damaged dust screen awaits replacement. Nessa can fire one new ceramic filter from the remaining clean charge. A finished spare exists as Daro's freight collateral. Another fired filter lies inside a sealed waste rack, mixed with brittle rejects.

A filter can protect the batchworks' loading crew, equip Lowsail's new washing stand, or earn money through a freight sale. A single filter cannot serve all three uses. Further production consumes limited fuel and requires exposing or containing the ash. Closing the hot works preserves people and unspent fuel, but abandons that production income.

No hidden disaster runs before the player touches this optional problem. The kiln is banked; its unfired charge is stored dry. Later arrival changes freight needs and the cast's credible reports, not the existence of an arbitrarily expired quest.

## Geography and tangible objects

The proposed area identity is `fume_yards.cinder_batchworks`. New locations and actions use `fume_yards.*`; new items, recipes, facts, and memories use the same prefix. None of the three locations is counted independently as a complete area.

| Location | Connections and purpose | Three state-changing interactables |
| --- | --- | --- |
| `fume_yards.freight_court` — Freight Court | Permanent road from `lowsail.levee`; permanent road to Ash Beds; crew door to Kiln Bay. A resolved tide also admits a new route from `lowsail.return`. | Collateral cage: acquire its single spare by a disclosed transaction. Crew board: assign workers to loading, safe shutdown, or supervised salvage. Freight cradle: load a physical filter for sale or escorted delivery. |
| `fume_yards.kiln_bay` — Kiln Bay | Crew door from Freight Court; low maintenance hatch from Ash Beds. Both directions remain traversable after shutdown. | Charge bench: consume defined ingredients to prepare or reclaim a charge. Firebox: ignite, bank, or abandon the batch. Dust housing: install a filter or a temporary wet screen, with different material and handling costs. |
| `fume_yards.ash_beds` — Ash Beds | Road from Freight Court and maintenance hatch to Kiln Bay. | Sorting table: separate clean mesh from one finite reject lot. Sealed waste rack: recover the fired salvage filter or break it during an explicitly risky shortcut. Catch bed: contain dirty ash or dump it into the local loading lane, changing later freight costs. |

The doors do not require new authority credentials. The maintenance hatch is useful because it reaches the waste rack's back and avoids the dusty loading lane. It costs an extra travel action; rope and trained handling enable carrying the fragile salvage filter through it intact.

Travel costs one world step per edge. Work actions disclose their exact cost. Add reciprocal generic exits only between the levee and Freight Court, and among the three new locations. Do not add a generic exit into `lowsail.return`: that would bypass its required cast arrival. Instead, extend the existing `world.enter_aftermath` location list to all new locations, retaining its outcome guard, Oren/Sava/Mira relocation, once-only movement behavior, and one-step cost. Add a separate outcome-guarded `Visit Batchworks` action from `lowsail.return` to Freight Court. Thus every new node always has a road exit and, after resolution, the canonical aftermath action. Prove this both before and after a deadline crossed locally. Cargo delivery still requires its own witnessed handover after the player arrives; the return action does not deliver goods or relocate Pera.

Fresh work states must expose at least two useful non-movement actions in each visited scene. A fully exhausted scene may become a quiet revisit only through a recorded resolved state; it still offers travel and truthful observations. Do not retain repeatable discoveries or relationship farming to inflate density. Do not mark the new locations terminal merely to bypass the compiler.

## Cast, knowledge, and routines

| Inhabitant | Goal and relationship | Credible information and movement |
| --- | --- | --- |
| `fume_yards.nessa_tern` — Nessa Tern, ceramic maker | Keep her workshop useful; refuses to call an untested filter clean. Relies on Brann's crew and resents Daro holding the finished spare. | Starts at the charge bench. She reads the batch docket and witnesses the player's material test. Lighting with her help records her involvement before any movement. She moves to Freight Court when her batch is drawn or abandoned. |
| `fume_yards.brann_coil` — Brann Coil, shift foreman | Keep workers breathing and paid. Will stop the furnace if their loading lane becomes unsafe. Owes Nessa skilled help; disputes Daro's collateral claim. | Starts in Freight Court. Knows the posted shift and witnessed crew assignments. He learns an indoor test only when Nessa or the present player demonstrates or reports it. Reassignment physically moves him to Ash Beds or Kiln Bay; a shutdown returns him to the court. |
| `fume_yards.daro_venn` — Daro Venn, freight factor | Recover payment on the spare and buy a finished shipment. Accepts money regardless of affection; worker control removes his exclusive loading privilege. | Remains at the collateral cage. Reads his own docket, witnesses cage transactions and sales. He cannot know a covert salvage operation merely because a global filter flag changed. |
| `fume_yards.pera_senn` — Pera Senn, carter | Carry clean freight without exposing her team. Supports whichever arrangement keeps her cart route usable. | Starts at Ash Beds and witnesses loading-lane conditions. An escorted delivery moves her into Lowsail; a return trip moves her back. She carries named witnessed or read facts to a present Oren. Remote recipients acquire no facts until the relevant transfer occurs. |

These are deterministic work routines triggered by actions and batch events, not a claim of a general daily schedule system. No extra unseen workers receive individual injuries or deaths without modeled entities and corresponding evidence.

Every authored NPC fact needs a source table in the implementation: fact ID, source object or named possessor, acquiring action/event, provenance, and permitted recipients. Initial NPC definitions currently do not seed knowledge. Author the acquiring actions explicitly; do not assume a biography populates `NpcState.knowledge`.

For example, `fume_yards.test_clean` is Nessa's witnessed filter test. `fume_yards.report_test` transfers that same fact to present Brann with `Told { by: nessa }`. A player's presence may be described without pretending that the player is a valid named NPC source. Pera learns a shipment's condition by inspecting its load, then tells Oren after arrival. Recipient knowledge, provenance, original acquisition turn, and duplicate-transfer behavior are witness predicates.

## Four approaches that must behave differently

| Family | Required operations | Cost and failure pressure | Persistent result beyond a renamed resolution flag |
| --- | --- | --- | --- |
| Controlled manufacture | Obtain Nessa's finite clean clay and mesh; prepare a charge; install a temporary wet screen or a recovered filter; ignite; draw within the heat window. | Consumes two clay units, one mesh unit, and one fuel unit for one filter. Wet screening additionally consumes one water cask. Late drawing spoils the charge. | Produces one owned, transferable filter. The dust housing's protection persists; remaining stock limits another batch. Nessa's continued skilled work and crew exposure depend on the process actually used. |
| Physical salvage | Approach the waste rack through the hatch; brace fragile pieces; separate the fired filter from rejects. A supervised route trades time for skill. | Consumes a finite brace or two stamina; skilled rope handling retains the rope. An advertised shortcut risks breaking the single recoverable filter using explicit entropy. | Produces the rack's existing item without manufacturing or buying it. The emptied rack and opened maintenance access persist; Daro's spare and clean charge remain available. |
| Supply substitution | Buy Daro's finished spare for four coins, or surrender a specifically identified remaining fuel lot as collateral settlement. Move the item through the actual cage inventory. | Money competes with the established three-coin towline. Fuel settlement prevents a later firing that requires that lot. No charisma roll invents goods or erases payment. | Daro's spare is depleted and his settlement memory changes later freight terms. Clean production can be skipped; the player can export the purchased filter while leaving local protection unresolved. |
| Crew shutdown and reclamation | Show Brann the dust test; stop the firebox; assign the crew to cold sorting; reclaim the unfired charge into cold repair plugs. | Forfeits the fired-filter recipe and its sale. Crew time precludes simultaneously staffing a hurried hot extraction. | The furnace stays shut, Brann changes location, the loading lane becomes safe, and finite cold repair goods replace the hot batch. Repair plugs can seal Lowsail's washing-stand frame but cannot filter water or satisfy Daro's filter order. |

The families can compose. A player may salvage a filter, fit it locally, then fire a clean export. Buying the spare first can protect the crew during production, but spends money available for travel. Shutting down after exporting a filter preserves that sale and changes the later worksite. Cold reclamation produces repair goods only from an unfired charge; shutting down an already lit or spent kiln never recovers clean ingredients. These combinations must follow stock and process state, not a single exclusive district-ending selector.

The content ceiling for this first district is deliberately finite: three locations, four inhabitants, three obtainable filters, one clean firing charge, one fuel lot, one reject lot, one water cask, and one cold repair lot. This is an authored stock budget, never an engine action cap. More repeated production is outside this contract until replenishment has its own economic and temporal rules.

Stock ownership is explicit. Nessa owns the clay, mesh, and fuel. Daro owns the cage spare. Pera is custodian of the rack's single fired filter and the water cask. Brann holds the reject lot and its brace. The rack is a named storage place for Pera's finite stock, not a new anonymous inventory or a recipe that creates an existing filter. Both safe and shortcut recovery require Pera's presence and transfer her exact item after the relevant handling actions. The risky shortcut first transfers the filter; a failed roll consumes that owned item into a broken shard. That loss and entropy draw must be atomic and replayable. A future covert theft approach needs its own possession and knowledge contract; it is not silently claimed here.

The hot batch also needs ownership through every stage. Preparing consumes clay 2 and mesh 1 into one `prepared_charge`. Igniting consumes that charge and fuel 1 into one visible `batch_claim`, records the kiln's active batch, and schedules readiness/spoil. The claim names the player's sole loaded batch; it cannot be traded, installed, or used at another kiln. Drawing consumes the claim into filter 1. Spoiling consumes the same claim into a spoiled-charge item, even while the player is elsewhere. Guards make ready/spoil inert after a completed draw or applicable shutdown. Before ignition, the cold-repair recipe may consume the prepared charge into repair lot 1. Exact input checks prevent repeated drawing or reclamation; a fabricated or missing claim fails state admission/replay rather than silently fixing history. No recipe needs an empty input map, and no timed event conjures an unowned output.

## Concrete consequences and character combinations

Filter destinations are physical consumptions of the item:

- **Fit Dust Filter:** consumes one filter; removes the two-stamina dust surcharge from new heavy loading actions. It leaves the filter unavailable for sale or Lowsail.
- **Fit Market Filter:** consumes one filter at the new Lowsail washing stand. It enables a once-only clean-water supply action and removes the surcharge from the new dirty-freight unloading action. The old market, tide outcome, and existing towline costs remain exactly as previously established.
- **Sell Filter:** consumes one filter for four coins and records Daro's witnessed purchase. It does not also install either filter.
- **Patch Stand:** consumes the cold repair lot and repairs a raised sorting surface. It enables `Sort Dry Goods`, a once-only one-step job paying three coins with present Oren's witnessed acceptance. This works in the cold pilot and in every tide aftermath. Later water-cask installation is an additional use of the repaired stand; repairs alone never produce clean water.

The new clean-water action restores exactly two stamina, once per completed installation. Its finite cask and installation must be recorded before the action becomes legal.

Dust dumped at Ash Beds dirties its own loading lane and adds two stamina to newly authored heavy freight handling until the player contains the remaining ash. The cost is displayed on each such action. Containing ash consumes the water cask; using that same cask for temporary screening or Lowsail makes containment unavailable. No invisible offscreen contamination of the whole basin is allowed.

Character proof must cross abilities, appearance, history, and acquired knowledge:

| Combination | Mechanical difference | Required counterfactual |
| --- | --- | --- |
| Kilnborn heat-sense plus lock-runner handling | One-step supervised hot separation with retained rope, instead of brace preparation plus separation. | Remove either contribution in a canonical custom recipe; the shortcut closes while the slower path remains. Kilnborn lineage alone is not the complete test. |
| Ledger-clerk skill plus a read collateral docket | Fuel settlement can be checked and executed without surrendering the wrong lot; lack of the docket requires its separate inspection action. | Change only calling, then compare exact goods and coin deltas. A substituted praise line is insufficient. |
| Council ink plus Wanted, after Brann has received the actual dust test | Brann offers supervised reclamation despite distrusting the mark; he does not grant Daro's collateral or waive costs. | Hold appearance and burden fixed; omit the credible test transfer. Reclamation delegation closes, while personal shutdown remains possible. |
| Saved-worker history plus Brann's witnessed present-day help | Brann staffs the slow salvage route, removing its brace cost but adding one work step. | Existing backstory alone cannot make this stranger recognize the player. The player must first tell the story, or a credible known witness must relay it; compare with the same present help but no established history. |

Values may change which sworn offer the player can make. They cannot silently force behavior, grant universal honesty detection, or stand in for every inhabitant's perspective. All 64 existing creator combinations must retain a viable ordinary route through the district; special combinations change methods and costs rather than becoming required keys.

## One timeline before and after Split Tide

The authoritative deadline stays `lowsail.next_surge` at world time 16. Every district action advances that same `world.time`. The existing countdown stays visible while unresolved; optional entry explains that work uses tide steps. Inspecting public catalogs or previews costs none.

Before the surge, the player can leave the main road for the batchworks and return through the same levee. Starting optional work does not delay the tide, extend preparation, reopen an outcome, or freeze remote events. If time reaches 16 inside any new location, the existing event resolves once. The player can finish coherent local work and use the existing changed-world return convention.

After Split Flow, work keeps its ordinary supply context. Hold Market leaves the established upland water loss intact: the sole stored cask remains usable, but no fresh-cask replenishment appears. Relief shifts the new washing stand's recipient context uphill. Opening the old channel does not launch Oren's ferry; only the existing ferry ending does. Disaster allows repairs beside the displaced community and never declares the old crossing restored. These context variants require the actual ending flags and witnessed arrival/report for NPC claims.

First arrival after turn 128 must find finite stock unspent if no earlier district action spent it. Igniting at turn 130 uses a heat window relative to 130, not an absolute event authored at turn 20. Leaving an active batch continues that window remotely. Returning after production preserves goods, consumed stock, crew positions, ash, relationships, and earlier tide history.

For a first heat implementation, use one batch only: ignite at time `t`, ready at `t + 2`, spoil at `t + 5`. Draw is legal only after ready and before spoil. As elsewhere, action effects precede the time advance and due-event reduction: drawing at pre-time `t + 4` succeeds and the following spoil event resolves inertly; waiting to pre-time `t + 5` is too late. Ready/spoil order and guard retirement must be explicit. No second timer, local clock, wall clock, or model adjudication is permitted.

## Actual vocabulary and bounded dependencies

The current kernel supports boolean composition, facets/tags, player inventory/resource predicates, world/location flags, NPC presence/knowledge/memory/relationship predicates, resource deltas, player/NPC movement, NPC-to-player stock transfer, NPC learning/memory, deeds, time advance, and explicit random branches. Those cover doors, finite process flags, staffing, witnessed reports, payments, and consequence observations.

The kernel implements item recipes and relative scheduling. Their cycle 32 implementation passes the combined gate; flags cannot substitute for ownership or timing:

1. **Authoritative item transformation and consumption: implemented for the pilot.** `RecipeDefinition` has a namespaced ID and positive integer input/output maps; `ApplyRecipe` selects it. Inputs come from player-owned stock; outputs return there. Empty outputs permit installation/consumption; empty inputs and unchanged maps are rejected. Admission reconstructs exact `RecipeApplied` quantities alongside stock transfers. Full-program legality checks consumption, production, and capacity before listing an action. The staged reducer rolls back the entire transition on failure. Schema v9 and rules v7 now bind this contract; saves remain exact-build-bound.
2. **Relative, one-shot scheduled events: implemented.** Typed deferred event templates hold a positive delay; `ScheduleEvent` uses checked `world.time + delay`. Existing absolute events retain their current semantics. This district needs only uniquely identified ready/spoil events for one batch; repeated scheduling of either ID rejects or is explicitly guarded out. Record scheduling in authoritative event history and validate due times against lineage. Templates must not be silently scheduled at genesis. Reject overflow, undeclared templates, illegal rescheduling, and recursively unbounded scheduling. No periodic scheduler or dynamic arbitrary event program is required here.
3. **Independent district assertions: partial.** Pilot witnesses bind exact ordered recipe IDs, turns, inputs, outputs, forbidden owned goods, NPC stock, local consequences, and witnessed records. Nine additional witnesses bind ordered deferred scheduling/resolution, exact due times, pending queues, and positive/forbidden ownership and local consequences. Every reviewed path must bind its parameters and prove both positive and forbidden postconditions.

A finite one-batch heat model can use typed flags for prepared, lit, ready, drawn, spoiled, and shut down, with explicit mutually incompatible-state checks. This proposal does not require local numeric meters, generic chemistry, continuous temperature, ambient weather, combat, or a full faction simulator. If later design claims any of those behaviors, their actual typed support is a separate prerequisite.

Recipe lineage now accounts for authored stock, NPC-to-player transfers, consumption, and production. This is structural state admission; trusted saves and evidence additionally reconstruct canonical actions through replay. Scheduling now reconstructs relative one-shot history alongside unchanged genesis-authored absolute events. Events cannot schedule events or advance time. Deferred recipe programs must prove their inputs from guards and reachable effect prefixes; runtime whole-program preflight remains atomic and considers every reachable random branch without inspecting entropy.

## Implemented cold-crafting pilot

The first material-conversion slice implements **cold repair goods carried to Lowsail** before the timed furnace. Its acceptance obligations are:

1. Add the typed recipe/consumption support, focused negative boundary tests, and independent replay assertions.
2. Author a connected pilot workshop, Nessa's finite two clay units and one mesh unit, and a witnessed stock handover.
3. `Press Repair Plugs` consumes exactly those inputs and produces one repair lot. `Pack Catch Screen` instead consumes clay 2 / mesh 1 into one coarse catch screen: clay holds its mesh edges in the loading tray. `Fit Catch Screen` consumes it locally and removes a two-stamina cleanup charge from `Load Freight`, a once-only job that pays two coins. The same job without the screen pays two coins and costs two stamina. This screen catches loose loading ash; it cannot replace a fired water filter or kiln filter. Repair delivery and local stamina preservation compete for the same finite inputs, and all job/material actions retire when resolved.
4. Carry the repair lot through the levee and, after any resolved tide, consume it with `Patch Stand` at Lowsail. Complete the newly enabled `Sort Dry Goods` job for exactly three coins; Oren must actually be present and the wage must not repeat. Preserve the tide outcome and establish a changed return to the depleted workshop. The pilot therefore offers a paid local job with preserved stamina versus a repaired remote worksite and its additional paid job; neither choice waits for future furnace or water code to become useful.
5. Bind canonical paths for manufacturing, the competing catch-screen choice and matched unscreened freight job, all five tide contexts, first entry after turn 128, save-before-transform, save-after-transform, and save-after-install. Nessa witnesses and commissions this single two-coin job in the pilot; its completed-job flag and payment memory prevent a repeated wage. Do not imply a general NPC purse simulation.

The pilot's playable workshop does not count as the contracted district or a new verified area. Its route and observations must say exactly what exists. Manager acceptance should prefer delivering this complete causal loop over landing unused crafting primitives alone. Then implement the one-batch scheduler with the controlled-manufacture path; then salvage, supply substitution, staffing, and the complete district evidence. Do not add empty locations early to advertise planned scale.

The implemented location is `fume_yards.workshop`, with Nessa as its single inhabitant. Four recipes and nine action definitions supply the two competing products, finite handover, local freight, Lowsail repair/sorting, and direct resolved-tide visit. The generic levee road and existing aftermath relocation program connect it to the same world. It does not yet implement the three-location geography or four-person cast above. Twenty total production witnesses preserve the sixteen prior paths and add repair, screen, unscreened freight, and late repair. Separate production/replay checks cover all 64 custom starts, all five tide outcomes, deadline crossing during crafting, saves at transformation/install boundaries, and late entry after the existing 128-turn traversal regression.

## Controlled-manufacture amendment (cycle 32 accepted)

The implemented geography is the existing workshop plus a reciprocal road to Kiln Bay. No empty Freight Court or Ash Beds is added. Nessa stays in her workshop with the original clay/mesh stock. Brann and Pera start at Kiln Bay; Brann holds the sole fuel unit and Pera the sole water cask. This supersedes the proposed fuel custodian for this slice. The full district must preserve this lineage when extending routines and geography.

The first firing requires the consumed wet screen, charge, and fuel. It schedules readiness after two world steps and spoil after five, measured from the pre-ignition time; ignition itself takes one step. The visible owned claim becomes one filter on timely draw or one spoiled charge after abandonment, even elsewhere. Present actions record Brann's witnessed knowledge; remote events do not invent his knowledge or memories. Inspecting an abandoned batch supplies the later credible witness.

A filter can be installed locally to preserve two stamina on Brann's once-only three-coin loading job, or sold to present Oren in Lowsail for four coins. Oren is the interim witnessed buyer; this does not implement Daro's collateral or freight docket. Bare loading pays the same three coins and costs two stamina. Those jobs require completed drawing, banking, or reclamation. Unbanked spoil contaminates the freight and permanently cancels this commission. Banking sacrifices the batch but preserves the paid job and saves time for the tide; an early matched draw instead misses the unchanged time-16 surge. Banking therefore has a concrete opportunity cost and benefit.

Reclaiming the unfired charge produces the existing repair lot, closes firing, and retains unspent fuel. The repaired stand then supports the old witnessed sorting job. All paths compete for Nessa's original finite stock. The first manufactured filter cannot restore clean water: its necessary wet screen spends the sole cask. Market filtration, salvage, purchased substitution, crew assignments, NPC relocation routines, character-specific alternatives, and the third location remain unimplemented contract work.

Acceptance adds nine independently specified witnesses for readiness, local installation, sale, bare handling, remote spoil, early bank, matched missed tide, first late manufacture, and cold reclamation. All twenty previous recipes and predicates remain unchanged. A separate production/replay suite covers all 64 custom starts, six old tide contexts, actual 128-turn traversal before late entry, deadline collisions, save forks, and bank-versus-abandon commission loss. Structural state admission is not a substitute for canonical replay authenticity.

The combined world report requires all 73 current definitions. Its regression component still requires the exact previous 60 under the old budgets, with the old 51-definition/six-location projection. The nine-action cold pilot retains its budget. A separate thirteen-action batch crawl has predeclared limits: depth 20, 128 expanded states, 768 frontiers, 2,048 action executions, and seven rows per catalog page. Both optional crawls disclose the same replayed seven-action Hold Market frontier; its actions consume depth and do not count as expanded coverage. Timer effects guide search priorities only; actual canonical waits and queue lineage establish reached states. No arbitrary world-state seed is allowed.

## Witness acceptance matrix

The IDs below reserve intended claims. Before any claim ships, replace each recipe description with an exact reviewed start, seed, complete canonical action/parameter sequence, checkpoint set, and hidden predicates in the scenario registry. This document is not an executable witness and does not pre-approve future paths.

| Proposed witness or check | Exact acceptance predicates |
| --- | --- |
| `fume-yards-cold-repair` | Authored start, actual road entry, named stock transfer, recipe consumes clay 2 / mesh 1 and creates repair lot 1; return handover consumes lot; stand repaired; present Oren's dry-goods job pays coin +3 exactly once and retires; no filter exists; original tide selection unchanged; workshop stock remains depleted on revisit. |
| `fume-yards-cold-screen` | Same initial stock as the repair witness; screen recipe consumes clay 2 / mesh 1 into screen 1; local installation consumes screen and leaves repair lot 0. Matched freight jobs pay coin +2 in both paths, cost stamina 0 with screen / 2 without, and retire; patch action absent and stale press inert. |
| `fume-yards-manufacture` | Clay/mesh become prepared charge 1; ignition consumes charge/fuel into batch claim 1; ready due `t+2`, spoil due `t+5`; guarded draw consumes claim into filter 1; later spoil resolves once without loss; no duplicated charge or stock. |
| `fume-yards-salvage` | Pera is present and her existing waste-rack filter changes owner exactly once; clean charge, fuel, and Daro's spare remain; hatch/carry route and handling cost match the advertised action path. A separately seeded broken shortcut consumes that transferred filter into one shard and cannot retry the depleted rack. |
| `fume-yards-supply` | Buying costs exactly four coins and depletes Daro's spare; a matched one-coin-short state omits purchase; fuel settlement depletes its exact lot and closes firing; no fabricated free alternative. |
| `fume-yards-shutdown` | Credible test transfer precedes delegated shutdown; furnace retires; Brann relocates and indexes agree; cold goods replace the charge; hot production cannot resume; local freight remains viable. |
| `fume-yards-use-tradeoff` | Matched saves from the same owned filter choose local fit, market fit, or sale. Each consumes one filter; respectively prove loading surcharge 0, new market actions, or coin +4; forbid the two unchosen consequences. |
| `fume-yards-composed-production` | Salvage then local fit then firing then export is reachable; total filters respect the three-source stock budget; consumed cask cannot also contain ash; no independent branch replenishes it. |
| `fume-yards-character-pairs` | Four matched comparisons above use canonical custom genesis; exact action-set, inventory/resource, crew-location, and knowledge differences; viable ordinary method for all 64 starts. Seed alone is never used to simulate character differences. |
| `fume-yards-uninformed-report` | Daro/Oren lack local hidden facts before credible transfer; matched delivery teaches only the established fact, exact provenance and time; no stock contents appear in public supplies. |
| `fume-yards-before-surge` | Enter before turn 16; pass 16 while in each new location across separate paths; existing surge resolves exactly once with unchanged disaster predicates; local work remains coherent; no late old outcome becomes legal. |
| `fume-yards-after-outcomes` | Extend each of the five existing outcome-to-return recipes without editing its prefix; verify matching context, still-valid old ending, new destination consumption, and forbidden contradictory tide flags. Also cover the separately missed-deadline disaster cause. |
| `fume-yards-late-entry` | Extend the existing 128-turn relief regression; first enter untouched district after its final checkpoint; no past-due local timer or remote invented knowledge; ignite late and prove relative due times and successful draw. |
| `fume-yards-remote-spoil` | Ignite, leave, cross spoil time elsewhere, return; usable filter 0, claim consumed into spoiled charge 1, spoil exactly once, truthful changed workshop; a missed local batch does not overwrite the main tide ending. |
| `fume-yards-resume-boundaries` | Continuous versus safe-save/resume before consumption, after consumption, before readiness, exactly at readiness, immediately before spoil, and after delivery have identical state, observations, catalogs, entropy, and final receipt. |
| `fume-yards-event-collision` | Arrange readiness/spoil to coincide with tide 16; both event IDs resolve in canonical order, correct pre/post guards apply, text remains under combined budgets; save/resume and separate-process replay agree. |
| `fume-yards-bad-change` | Targeted consume-bypass, duplicate-output, remote timer pause, false NPC-knowledge, and post-install resale corruptions fail their own independent predicates after passing neutral controls. No production mutation hook. |
| `fume-yards-area-contract` | Reach all three locations and nine objects through authored starts; prove four families and representative combinations; per-scene complete catalogs and word limits; changed local/external revisit; inspect action/effect/cast/topology fingerprint against both existing areas. |

Every outcome claim uses a clean-process verifier bound to the exact candidate build. No source-informed scripted witness is called blind play. After mechanical acceptance, source-isolated players must try optional entry, decide whether to abandon the tide objective, and revisit the district through the ordinary browser. Their findings can improve clarity and choice quality; this small area still cannot establish Skyrim breadth or BG3 comparative depth.

## Regression preservation and scope controls

Keep all twenty pre-batch scenario IDs, starts, seeds, exact recipes, and semantic predicates. New authoritative content will change build/state/catalog hashes, so regenerate checked artifacts through trusted tooling; never weaken an old expectation merely because a new build changed hashes. Preserve the existing 128-turn relief path, deadline at 16, single outcome exclusivity, paid towline's three-coin charge and retained gear, key ownership, warning provenance, cast return continuity, catalog completeness, supply privacy, and save/replay behavior.

The existing crawl proves six locations and 51 action definitions under reviewed budgets. Do not silently raise those budgets until all new coverage fits. Keep an explicit old-definition/old-location coverage projection, and add a separately budgeted optional-district crawl with reviewed reachability and work bounds. A full-world report may then combine coverage without pretending the old 56-state crawl exhaustively proves the expansion. Keep the 500-location fixture labeled synthetic.

Preserve browser retry/reload/import, process-instance recovery, exact full-width numbers, capability isolation, and bundle reproduction. The browser displays kernel observations and submits canonical IDs; it gains no recipe calculator, timer authority, local legality, or client-only consequence logic.

No new area claim is accepted until the complete matrix has implementations and evidence. A green pilot loop, a new schema, a larger map, or this proposal alone does not satisfy that condition.
