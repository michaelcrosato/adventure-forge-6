use std::collections::{BTreeMap, BTreeSet};

use forge_content::parse_and_compile_production;
use forge_kernel::{
    CanonicalAction, CharacterChoiceSelection, CharacterSelection, CompiledContent, EventKind,
    GameState, KnowledgeProvenance, enumerate_legal_actions, step,
};

const SOURCE: &str = include_str!("../../../content/split-tide.json");
const ASH: &str = "fume_yards.ash_beds";
const BAY: &str = "fume_yards.kiln_bay";
const WORK: &str = "fume_yards.workshop";
const RETURN: &str = "lowsail.return";
const BRANN: &str = "fume_yards.brann_coil";
const DARO: &str = "fume_yards.daro_venn";
const FILTER: &str = "fume_yards.filter";
const STORY: &str = "fume_yards.share_rescue_account";
const ASSIGN: &str = "fume_yards.assign_brann_salvage";
const LIFT: &str = "fume_yards.recover_staffed_filter";
const BACK: &str = "fume_yards.return_with_brann";
const ACCOUNT: &str = "fume_yards.rescue_account_heard";
const ACTIVE: &str = "fume_yards.salvage_assignment_active";
const SPENT: &str = "fume_yards.salvage_assignment_spent";
const CLEAR: &str = "fume_yards.rack_cleared";
const REPORT: &str = "fume_yards.report_with_daro";
type Spec = (&'static str, Option<&'static str>);

fn content() -> CompiledContent {
    parse_and_compile_production(SOURCE).expect("staffing compiles under the unchanged text limit")
}
fn select(
    state: &GameState,
    content: &CompiledContent,
    id: &str,
    to: Option<&str>,
) -> CanonicalAction {
    let parameters = to
        .map(|to| BTreeMap::from([("destination".into(), to.into())]))
        .unwrap_or_default();
    enumerate_legal_actions(state, content)
        .unwrap()
        .into_iter()
        .find(|a| a.definition_id == id && a.parameters == parameters)
        .unwrap_or_else(|| {
            panic!(
                "missing {id} {to:?} at {} time {}",
                state.world.current_location, state.world.time
            )
        })
}
fn apply(state: GameState, content: &CompiledContent, id: &str, to: Option<&str>) -> GameState {
    let action = select(&state, content, id, to);
    let views = content.action_page(&state, 0, usize::MAX).unwrap();
    let view = views
        .actions
        .iter()
        .find(|v| v.action_id == action.action_id)
        .unwrap();
    let ticks = if id == LIFT { 3 } else { 1 };
    assert_eq!(
        (view.time_cost.minimum_ticks, view.time_cost.maximum_ticks),
        (ticks, ticks)
    );
    let transition = step(&state, &action, content, &state.entropy).unwrap();
    let observation = content.observe_after_transition(&transition).unwrap();
    assert!(
        observation.text.split_whitespace().count()
            + observation.supplies.summary().split_whitespace().count()
            < 100,
        "{}",
        observation.text
    );
    assert_eq!(transition.state().world.time, state.world.time + ticks);
    transition.into_state()
}
fn act(state: GameState, content: &CompiledContent, id: &str) -> GameState {
    apply(state, content, id, None)
}
fn travel(state: GameState, content: &CompiledContent, to: &str) -> GameState {
    apply(state, content, "travel_adjacent", Some(to))
}
fn run(mut state: GameState, content: &CompiledContent, path: &[Spec]) -> GameState {
    for (id, to) in path {
        state = apply(state, content, id, *to);
    }
    state
}
fn legal(state: &GameState, content: &CompiledContent) -> BTreeSet<String> {
    enumerate_legal_actions(state, content)
        .unwrap()
        .into_iter()
        .map(|a| a.definition_id)
        .collect()
}
fn absent(state: &GameState, content: &CompiledContent, ids: &[&str]) {
    let actual = legal(state, content);
    for id in ids {
        assert!(
            !actual.contains(*id),
            "unexpected {id} at time {}",
            state.world.time
        );
    }
}
fn flag(state: &GameState, at: &str, key: &str) -> bool {
    state.world.locations[at].flags.contains(key)
}
fn full_catalog(state: &GameState, content: &CompiledContent) {
    let expected: Vec<_> = enumerate_legal_actions(state, content)
        .unwrap()
        .into_iter()
        .map(|a| a.action_id)
        .collect();
    let mut actual = Vec::new();
    let mut offset = 0;
    loop {
        let page = content.action_page(state, offset, 7).unwrap();
        assert_eq!(page.total, expected.len());
        actual.extend(page.actions.into_iter().map(|a| a.action_id));
        if let Some(next) = page.next_offset {
            offset = next;
        } else {
            break;
        }
    }
    assert_eq!(actual, expected);
}
fn custom(content: &CompiledContent, choices: &[&str; 6]) -> GameState {
    let selection = CharacterSelection {
        name: "Taren Venn".into(),
        choices: ["lineage", "origin", "calling", "value", "burden", "history"]
            .into_iter()
            .zip(choices)
            .map(|(slot_id, choice_id)| CharacterChoiceSelection {
                slot_id: slot_id.into(),
                choice_id: (*choice_id).into(),
            })
            .collect(),
    };
    content.new_custom_game(&selection, 71).unwrap()
}
fn clerk(content: &CompiledContent, history: &str) -> GameState {
    custom(
        content,
        &[
            "fenborn",
            "lowsail",
            "ledger-clerk",
            "order",
            "indebted",
            history,
        ],
    )
}
const HOLD: &[Spec] = &[
    ("checkpoint.show_charter", None),
    ("travel_adjacent", Some("lowsail.levee")),
    ("levee.authority_path", None),
    ("travel_adjacent", Some("red_sluice.top")),
    ("top.hold_market", None),
    ("world.enter_aftermath", None),
    ("return.count_dry_stalls", None),
];
fn common12(content: &CompiledContent, history: &str) -> GameState {
    let state = run(clerk(content, history), content, HOLD);
    let state = act(state, content, "return.visit_workshop");
    let state = travel(state, content, ASH);
    let state = act(state, content, "fume_yards.buy_collateral_filter");
    let state = travel(state, content, WORK);
    let state = travel(state, content, BAY);
    assert_eq!(state.world.time, 12);
    state
}
fn common13(content: &CompiledContent) -> GameState {
    let state = act(
        common12(content, "saved-worker"),
        content,
        "fume_yards.fit_dust_filter",
    );
    assert_eq!(
        (
            state.world.time,
            state.character.resources["coin"],
            state.character.resources["stamina"]
        ),
        (13, 6, 3)
    );
    state
}
fn assigned(content: &CompiledContent) -> GameState {
    let state = act(common13(content), content, STORY);
    act(state, content, ASSIGN)
}
fn staffed20(content: &CompiledContent) -> GameState {
    let state = act(assigned(content), content, LIFT);
    let state = act(state, content, BACK);
    act(state, content, "fume_yards.load_cold_freight")
}
fn ordinary20(content: &CompiledContent) -> GameState {
    run(
        common13(content),
        content,
        &[
            ("fume_yards.enter_ash_hatch", None),
            ("fume_yards.brace_rack", None),
            ("fume_yards.recover_braced_filter", None),
            (REPORT, None),
            ("fume_yards.load_cold_freight", None),
            ("wait_tide", None),
            ("wait_tide", None),
        ],
    )
}
fn surge(state: &GameState) -> Vec<(u64, bool)> {
    state
        .event_log
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::ScheduledEventResolved {
                event_id, applied, ..
            } if event_id == "lowsail.next_surge" => Some((e.turn, *applied)),
            _ => None,
        })
        .collect()
}

#[test]
fn all_custom_histories_require_actual_account_and_help_while_retaining_ordinary_recovery() {
    let content = content();
    let slots = &content.character_creation().unwrap().slots;
    let mut recognized = 0;
    for mask in 0..64 {
        let selection = CharacterSelection {
            name: "Taren Venn".into(),
            choices: slots
                .iter()
                .enumerate()
                .map(|(i, slot)| CharacterChoiceSelection {
                    slot_id: slot.id.clone(),
                    choice_id: slot.choices[(mask >> i) & 1].id.clone(),
                })
                .collect(),
        };
        let mut state = content.new_custom_game(&selection, 71).unwrap();
        state = travel(state, &content, "lowsail.levee");
        state = travel(state, &content, WORK);
        state = travel(state, &content, ASH);
        state = act(state, &content, "fume_yards.buy_collateral_filter");
        state = travel(state, &content, WORK);
        state = travel(state, &content, BAY);
        state = act(state, &content, "fume_yards.fit_dust_filter");
        assert_eq!(state.world.time, 7);
        full_catalog(&state, &content);
        absent(&state, &content, &[ASSIGN]);
        assert!(!state.world.npcs[BRANN].knows(ACCOUNT));
        let saved = state.character.deeds.contains("saved_worker");
        assert_eq!(legal(&state, &content).contains(STORY), saved);
        if saved {
            let told = act(state.clone(), &content, STORY);
            let account = &told.world.npcs[BRANN].knowledge[ACCOUNT];
            assert_eq!(account.turn, 7);
            assert_eq!(account.provenance, KnowledgeProvenance::Witnessed);
            assert!(legal(&told, &content).contains(ASSIGN));
            recognized += 1;
        }
        let stamina = state.character.resources["stamina"];
        state = act(state, &content, "fume_yards.enter_ash_hatch");
        state = act(state, &content, "fume_yards.brace_rack");
        state = act(state, &content, "fume_yards.recover_braced_filter");
        assert_eq!(state.character.resources["stamina"], stamina - 2);
        assert_eq!(state.character.inventory[FILTER], 1);
        assert!(state.world.npcs[DARO].inventory.is_empty());
        assert!(!state.world.npcs[BRANN].knows(CLEAR));
    }
    assert_eq!(recognized, 32);
}

#[test]
fn account_alone_does_not_replace_the_actual_consumed_dust_filter() {
    let content = content();
    let before = common12(&content, "saved-worker");
    let told = act(before.clone(), &content, STORY);
    assert_eq!(told.character.inventory[FILTER], 1);
    assert!(!flag(&told, BAY, "fume_yards.dust_filter_fitted"));
    assert!(!told.world.npcs[BRANN].remembers("fume_yards.dust_filter_fitted"));
    assert_eq!(told.world.npcs[BRANN].knowledge[ACCOUNT].turn, 12);
    absent(&told, &content, &[STORY, ASSIGN]);
    let helped = act(told.clone(), &content, "fume_yards.fit_dust_filter");
    assert!(!helped.character.inventory.contains_key(FILTER));
    assert!(legal(&helped, &content).contains(ASSIGN));
    let mut forged = told;
    forged
        .world
        .locations
        .get_mut(BAY)
        .unwrap()
        .flags
        .insert("fume_yards.dust_filter_fitted".into());
    assert!(match enumerate_legal_actions(&forged, &content) {
        Err(_) => true,
        Ok(actions) => !actions.iter().any(|a| a.definition_id == ASSIGN),
    });
    let other = common12(&content, "stole-permit");
    absent(&other, &content, &[STORY, ASSIGN]);
    assert_eq!(other.character.inventory, before.character.inventory);
    assert_eq!(other.character.resources, before.character.resources);
}

#[test]
fn three_step_staffed_lift_preserves_stamina_and_binds_actual_shared_time() {
    let content = content();
    let state = assigned(&content);
    assert_eq!(state.world.time, 15);
    assert_eq!(state.world.current_location, ASH);
    assert_eq!(state.world.npcs[BRANN].location, ASH);
    assert!(flag(&state, ASH, ACTIVE));
    assert!(flag(&state, ASH, SPENT));
    let before_events = state.event_log.len();
    let entropy = state.entropy.clone();
    let stale = select(&state, &content, LIFT, None);
    let lifted = act(state, &content, LIFT);
    assert_eq!(
        (
            lifted.world.time,
            lifted.character.resources["coin"],
            lifted.character.resources["stamina"]
        ),
        (18, 7, 3)
    );
    assert_eq!(lifted.character.inventory[FILTER], 1);
    assert_eq!(lifted.entropy, entropy);
    assert!(lifted.world.npcs[DARO].inventory.is_empty());
    assert!(flag(&lifted, ASH, ACTIVE));
    for npc in [BRANN, DARO] {
        let fact = &lifted.world.npcs[npc].knowledge[CLEAR];
        assert_eq!(fact.turn, 15);
        assert_eq!(fact.provenance, KnowledgeProvenance::Witnessed);
    }
    let effects: Vec<_> = lifted.event_log[before_events..]
        .iter()
        .filter(|e| {
            matches!(
                e.kind,
                EventKind::NpcItemTransferredToCharacter { .. }
                    | EventKind::ResourceAdjusted { .. }
                    | EventKind::TimeAdvanced { .. }
                    | EventKind::ScheduledEventResolved { .. }
            )
        })
        .map(|e| (e.turn, e.kind.clone()))
        .collect();
    assert_eq!(
        effects[0],
        (
            15,
            EventKind::NpcItemTransferredToCharacter {
                npc: DARO.into(),
                item: FILTER.into(),
                count: 1
            }
        )
    );
    assert_eq!(
        effects[1],
        (
            15,
            EventKind::ResourceAdjusted {
                resource: "coin".into(),
                amount: 1
            }
        )
    );
    assert_eq!(effects[2], (15, EventKind::TimeAdvanced { ticks: 3 }));
    assert_eq!(effects.len(), 4);
    assert_eq!(surge(&lifted), vec![(18, false)]);
    let snapshot = lifted.clone();
    assert!(step(&lifted, &stale, &content, &lifted.entropy).is_err());
    assert_eq!(lifted, snapshot);
    absent(&lifted, &content, &[LIFT, REPORT]);
    assert!(legal(&lifted, &content).contains(BACK));
    full_catalog(&lifted, &content);
    let staffed = staffed20(&content);
    let ordinary = ordinary20(&content);
    assert_eq!(staffed.world.time, 20);
    assert_eq!(ordinary.world.time, 20);
    assert_eq!(staffed.character.inventory, ordinary.character.inventory);
    assert_eq!(staffed.character.resources["coin"], 10);
    assert_eq!(ordinary.character.resources["coin"], 10);
    assert_eq!(staffed.character.resources["stamina"], 3);
    assert_eq!(ordinary.character.resources["stamina"], 1);
    assert_eq!(
        ordinary.world.npcs[BRANN].knowledge[CLEAR].provenance,
        KnowledgeProvenance::Told { by: DARO.into() }
    );
    assert_eq!(ordinary.world.npcs[DARO].location, BAY);
    assert_eq!(staffed.world.npcs[DARO].location, ASH);
    assert_eq!(surge(&ordinary), vec![(16, false)]);
    assert_ne!(staffed.event_log, ordinary.event_log);
}

fn prepared_with_account(content: &CompiledContent) -> GameState {
    run(
        common13(content),
        content,
        &[
            ("fume_yards.take_fuel", None),
            ("travel_adjacent", Some(WORK)),
            ("fume_yards.take_stock", None),
            ("travel_adjacent", Some(BAY)),
            ("fume_yards.prepare_charge", None),
            (STORY, None),
        ],
    )
}

#[test]
fn physical_absence_suspends_prepared_kiln_work_and_cancellation_restores_supervision() {
    let content = content();
    let prepared = prepared_with_account(&content);
    assert!(legal(&prepared, &content).contains("fume_yards.ignite_batch"));
    let state = act(prepared, &content, ASSIGN);
    let state = act(state, &content, "fume_yards.leave_ash_hatch");
    assert_eq!(state.world.current_location, BAY);
    assert_eq!(state.world.npcs[BRANN].location, ASH);
    absent(
        &state,
        &content,
        &[
            "fume_yards.ignite_batch",
            "fume_yards.reclaim_charge",
            "fume_yards.take_fuel",
            ASSIGN,
        ],
    );
    let text = content.observe(&state).unwrap().text;
    assert!(
        text.contains("Brann") && text.contains("Ash Beds"),
        "{text}"
    );
    let state = act(state, &content, "fume_yards.enter_ash_hatch");
    let stale = select(&state, &content, BACK, None);
    let state = act(state, &content, BACK);
    assert_eq!(state.world.npcs[BRANN].location, BAY);
    assert!(!flag(&state, ASH, ACTIVE));
    assert!(flag(&state, ASH, SPENT));
    assert!(legal(&state, &content).contains("fume_yards.ignite_batch"));
    assert!(legal(&state, &content).contains("fume_yards.reclaim_charge"));
    assert_eq!(state.character.resources["coin"], 6);
    assert_eq!(state.character.inventory["fume_yards.prepared_charge"], 1);
    assert_eq!(state.world.npcs[DARO].inventory[FILTER], 1);
    assert!(!state.world.npcs[BRANN].knows(CLEAR));
    absent(&state, &content, &[ASSIGN, STORY, BACK]);
    let snapshot = state.clone();
    assert!(step(&state, &stale, &content, &state.entropy).is_err());
    assert_eq!(state, snapshot);
    let state = act(state, &content, "fume_yards.enter_ash_hatch");
    let state = act(state, &content, "fume_yards.brace_rack");
    let state = act(state, &content, "fume_yards.recover_braced_filter");
    assert_eq!(state.character.inventory[FILTER], 1);
}

#[test]
fn an_active_batch_prevents_assignment_without_pausing_its_deadlines() {
    let content = content();
    let prepared = prepared_with_account(&content);
    let mut state = act(prepared, &content, "fume_yards.ignite_batch");
    absent(&state, &content, &[ASSIGN]);
    let pending = state.world.scheduled_events.clone();
    assert_eq!(pending.len(), 2);
    state = act(state, &content, "wait_tide");
    state = act(state, &content, "wait_tide");
    assert!(flag(&state, BAY, "fume_yards.batch_ready"));
    absent(&state, &content, &[ASSIGN]);
    assert_eq!(state.world.npcs[BRANN].location, BAY);
    assert!(
        state
            .world
            .scheduled_events
            .iter()
            .all(|event| pending.contains(event))
    );
    state = travel(state, &content, WORK);
    state = act(state, &content, "wait_tide");
    assert!(flag(&state, BAY, "fume_yards.batch_spoiled"));
    assert!(
        !state
            .character
            .inventory
            .contains_key("fume_yards.batch_claim")
    );
}

#[test]
fn personal_recovery_during_assignment_keeps_return_and_the_single_old_report_payment() {
    let content = content();
    let state = assigned(&content);
    let state = act(state, &content, "fume_yards.brace_rack");
    absent(&state, &content, &[LIFT]);
    let state = act(state, &content, "fume_yards.recover_braced_filter");
    assert_eq!(state.character.resources["coin"], 6);
    assert!(!state.world.npcs[BRANN].knows(CLEAR));
    assert_eq!(state.world.npcs[BRANN].location, ASH);
    assert!(legal(&state, &content).contains(BACK));
    absent(&state, &content, &[LIFT, REPORT]);
    let state = act(state, &content, BACK);
    let state = act(state, &content, "fume_yards.enter_ash_hatch");
    let state = act(state, &content, REPORT);
    assert_eq!(state.character.resources["coin"], 7);
    assert_eq!(state.character.inventory[FILTER], 1);
    assert_eq!(
        state.world.npcs[BRANN].knowledge[CLEAR].provenance,
        KnowledgeProvenance::Told { by: DARO.into() }
    );
    assert_eq!(state.world.npcs[DARO].location, BAY);
    let state = act(state, &content, "fume_yards.enter_ash_hatch");
    absent(&state, &content, &[LIFT, REPORT, BACK]);
}

#[test]
fn personal_breakage_during_assignment_keeps_truthful_return_and_one_report_payment() {
    let content = content();
    let start = run(content.new_game("ilyan", 123).unwrap(), &content, HOLD);
    let assigned = run(
        start,
        &content,
        &[
            ("return.visit_workshop", None),
            ("travel_adjacent", Some(ASH)),
            ("fume_yards.buy_collateral_filter", None),
            ("travel_adjacent", Some(WORK)),
            ("travel_adjacent", Some(BAY)),
            ("fume_yards.fit_dust_filter", None),
            (STORY, None),
            (ASSIGN, None),
        ],
    );
    let state = act(assigned, &content, "fume_yards.pull_rack_filter");
    assert_eq!(state.world.time, 16);
    assert_eq!(state.entropy.cursor, 1);
    assert_eq!(state.character.inventory.get(FILTER), None);
    assert_eq!(state.character.inventory["fume_yards.shard"], 1);
    assert_eq!(state.character.resources["coin"], 6);
    assert!(state.world.npcs[DARO].inventory.is_empty());
    assert!(!state.world.npcs[BRANN].knows(CLEAR));
    assert!(!flag(&state, ASH, "fume_yards.report_paid"));
    assert!(content.observe(&state).unwrap().text.contains(
        "The rack filter broke; return Brann before bringing Daro to report the cleared access."
    ));
    absent(&state, &content, &[LIFT, REPORT]);
    let state = act(state, &content, BACK);
    let state = act(state, &content, "fume_yards.enter_ash_hatch");
    let state = act(state, &content, REPORT);
    assert_eq!(state.character.resources["coin"], 7);
    assert_eq!(state.character.inventory["fume_yards.shard"], 1);
    assert_eq!(state.world.npcs[BRANN].knowledge[CLEAR].turn, 18);
    assert_eq!(
        state.world.npcs[BRANN].knowledge[CLEAR].provenance,
        KnowledgeProvenance::Told { by: DARO.into() }
    );
    let state = act(state, &content, "fume_yards.enter_ash_hatch");
    absent(&state, &content, &[LIFT, REPORT, BACK]);
}

#[test]
fn structural_absent_custodian_still_allows_branns_actual_return() {
    let content = content();
    let mut state = assigned(&content);
    // A structural boundary fixture, not a claim that another production action moves Daro here.
    state.world.npcs.get_mut(DARO).unwrap().location = WORK.into();
    state
        .world
        .locations
        .get_mut(ASH)
        .unwrap()
        .entities
        .remove(DARO);
    state
        .world
        .locations
        .get_mut(WORK)
        .unwrap()
        .entities
        .insert(DARO.into());
    assert!(content.validate_state(&state).is_err());
    // Make the synthetic relocation internally consistent; trusted replay still rejects
    // this invented history because no recorded canonical action performed it.
    state.event_log.push(forge_kernel::Event {
        turn: state.world.time,
        kind: EventKind::NpcMoved {
            npc: DARO.into(),
            from: ASH.into(),
            to: WORK.into(),
        },
    });
    absent(&state, &content, &[LIFT]);
    assert!(
        content
            .observe(&state)
            .unwrap()
            .text
            .contains("Brann waits beside the rack; return him through the hatch to Kiln Bay.")
    );
    let returned = act(state, &content, BACK);
    assert_eq!(returned.world.current_location, BAY);
    assert_eq!(returned.world.npcs[BRANN].location, BAY);
    assert_eq!(returned.world.npcs[DARO].location, WORK);
    assert_eq!(returned.world.npcs[DARO].inventory[FILTER], 1);
    assert_eq!(returned.character.resources["coin"], 6);
    assert!(!flag(&returned, ASH, ACTIVE));
    assert!(flag(&returned, ASH, SPENT));
    assert!(!returned.world.npcs[BRANN].knows(CLEAR));
}

#[test]
fn structural_staffed_effect_program_rejects_missing_stock_and_overflow_before_payment() {
    let mut draft = forge_content::parse(SOURCE).unwrap();
    // Isolate the exact production effect program from the demand guard: full possession also
    // retires acquisition, so testing only that guard would not exercise transfer preflight.
    let lift = draft.actions.iter_mut().find(|a| a.id == LIFT).unwrap();
    lift.condition = forge_content::Condition::Always;
    draft.contract = forge_content::ContentContract::Fixture;
    let isolated = forge_content::compile(draft).unwrap();
    let neutral = assigned(&isolated);
    assert!(legal(&neutral, &isolated).contains(LIFT));
    let lifted = act(neutral.clone(), &isolated, LIFT);
    assert_eq!(lifted.character.resources["coin"], 7);
    assert_eq!(lifted.character.inventory[FILTER], 1);
    assert_eq!(lifted.world.time, 18);
    let original = content();
    assert_eq!(
        isolated.action(LIFT).unwrap().effects,
        original.action(LIFT).unwrap().effects
    );
    let mut depleted = neutral.clone();
    depleted
        .world
        .npcs
        .get_mut(DARO)
        .unwrap()
        .inventory
        .remove(FILTER);
    let mut full = neutral;
    full.character.inventory.insert(FILTER.into(), u32::MAX);
    for state in [depleted, full] {
        let before = state.clone();
        absent(&state, &isolated, &[LIFT]);
        assert_eq!(state, before);
        let returned = act(state, &isolated, BACK);
        assert_eq!(returned.character.resources["coin"], 6);
        assert!(!flag(&returned, ASH, CLEAR));
        assert!(!flag(&returned, ASH, "fume_yards.report_paid"));
        assert!(!returned.world.npcs[BRANN].knows(CLEAR));
        assert_eq!(returned.entropy.cursor, 0);
    }
}

#[test]
fn a_three_step_lift_crosses_the_real_unresolved_surge_without_expiring_stock() {
    let content = content();
    let mut state = clerk(&content, "saved-worker");
    state = travel(state, &content, "lowsail.levee");
    state = travel(state, &content, WORK);
    state = travel(state, &content, ASH);
    state = act(state, &content, "fume_yards.buy_collateral_filter");
    state = travel(state, &content, WORK);
    state = travel(state, &content, BAY);
    state = act(state, &content, "fume_yards.fit_dust_filter");
    state = act(state, &content, STORY);
    state = act(state, &content, ASSIGN);
    while state.world.time < 15 {
        state = act(state, &content, "wait_tide");
    }
    let state = act(state, &content, LIFT);
    assert_eq!(state.world.time, 18);
    assert_eq!(surge(&state), vec![(18, true)]);
    assert!(state.world.flags.contains("sluice_failure"));
    assert!(flag(&state, RETURN, "market_flooded"));
    assert_eq!(state.character.inventory[FILTER], 1);
    assert_eq!(state.world.npcs[BRANN].knowledge[CLEAR].turn, 15);
    let state = act(state, &content, BACK);
    let state = act(state, &content, "world.enter_aftermath");
    let state = act(state, &content, "return.face_flood");
    assert!(state.world.flags.contains("ending_disaster"));
    assert_eq!(surge(&state), vec![(18, true)]);
}

#[test]
fn finished_unprotected_freight_does_not_promise_an_impossible_staffing_prerequisite() {
    let content = content();
    for already_told in [true, false] {
        let mut state = common12(&content, "saved-worker");
        if already_told {
            state = act(state, &content, STORY);
        }
        state = run(
            state,
            &content,
            &[
                ("travel_adjacent", Some(WORK)),
                ("fume_yards.take_stock", None),
                ("travel_adjacent", Some(BAY)),
                ("fume_yards.prepare_charge", None),
                ("fume_yards.reclaim_charge", None),
                ("fume_yards.load_kiln_freight", None),
                ("world.enter_aftermath", None),
                ("return.patch_stand", None),
                ("return.order_water_stand", None),
                ("return.fit_market_filter", None),
                ("return.visit_workshop", None),
                ("travel_adjacent", Some(BAY)),
            ],
        );
        assert!(flag(&state, BAY, "fume_yards.kiln_freight_loaded"));
        assert!(!flag(&state, BAY, "fume_yards.dust_filter_fitted"));
        absent(&state, &content, &["fume_yards.fit_dust_filter", ASSIGN]);
        assert_eq!(state.world.npcs[DARO].inventory[FILTER], 1);
        if already_told {
            let text = content.observe(&state).unwrap().text;
            assert!(
                !text.contains("needs an installed dust filter"),
                "staffing promises protection after that work has closed: {text}"
            );
        } else {
            assert!(
                !legal(&state, &content).contains(STORY),
                "rescue account offers staffing after protection is permanently closed"
            );
        }
        state = act(state, &content, "fume_yards.enter_ash_hatch");
        assert!(legal(&state, &content).contains("fume_yards.pull_rack_filter"));
    }
}

const SPLIT: &[Spec] = &[
    ("checkpoint.show_charter", None),
    ("travel_adjacent", Some("lowsail.levee")),
    ("levee.authority_path", None),
    ("floor.read_harmonics", None),
    ("travel_adjacent", Some("red_sluice.top")),
    ("top.check_wheels", None),
    ("top.split_flow", None),
    ("world.enter_aftermath", None),
    ("return.share_water", None),
];
const RELIEF: &[Spec] = &[
    ("checkpoint.show_charter", None),
    ("travel_adjacent", Some("lowsail.docks")),
    ("docks.ring_warning", None),
    ("travel_adjacent", Some("lowsail.levee")),
    ("levee.relay_warning", None),
    ("levee.authority_path", None),
    ("floor.open_relief", None),
    ("travel_adjacent", Some("red_sluice.top")),
    ("top.divert_relief", None),
    ("world.enter_aftermath", None),
    ("return.move_inland", None),
];
const FERRY: &[Spec] = &[
    ("checkpoint.blend_workers", None),
    ("travel_adjacent", Some("lowsail.levee")),
    ("levee.culvert_path", None),
    ("travel_adjacent", Some("red_sluice.top")),
    ("top.break_toll", None),
    ("world.enter_aftermath", None),
    ("return.open_ferry", None),
];
const OVERLOAD: &[Spec] = &[
    ("checkpoint.blend_workers", None),
    ("travel_adjacent", Some("lowsail.levee")),
    ("levee.culvert_path", None),
    ("floor.force_wheel", None),
    ("travel_adjacent", Some("red_sluice.top")),
    ("top.overload", None),
    ("world.enter_aftermath", None),
    ("return.face_flood", None),
];
fn staff_from_return(state: GameState, content: &CompiledContent) -> GameState {
    run(
        state,
        content,
        &[
            ("return.visit_workshop", None),
            ("travel_adjacent", Some(ASH)),
            ("fume_yards.buy_collateral_filter", None),
            ("travel_adjacent", Some(WORK)),
            ("travel_adjacent", Some(BAY)),
            ("fume_yards.fit_dust_filter", None),
            (STORY, None),
            (ASSIGN, None),
            (LIFT, None),
            (BACK, None),
        ],
    )
}

#[test]
fn staffing_preserves_all_five_outcomes_and_the_missed_surge_aftermath() {
    let content = content();
    let mut deadline = vec![("wait_tide", None); 16];
    deadline.extend([("world.enter_aftermath", None), ("return.face_flood", None)]);
    for (runner, path, ending) in [
        (false, SPLIT, "ending_accord"),
        (false, HOLD, "ending_council"),
        (false, RELIEF, "ending_relief"),
        (true, FERRY, "ending_freedom"),
        (true, OVERLOAD, "ending_disaster"),
        (false, deadline.as_slice(), "ending_disaster"),
    ] {
        let genesis = if runner {
            custom(
                &content,
                &[
                    "kilnborn",
                    "red-sluice",
                    "lock-runner",
                    "freedom",
                    "wanted",
                    "saved-worker",
                ],
            )
        } else {
            clerk(&content, "saved-worker")
        };
        let before = run(genesis, &content, path);
        assert!(before.world.flags.contains(ending));
        let after = staff_from_return(before.clone(), &content);
        assert_eq!(after.world.flags, before.world.flags);
        assert_eq!(
            after.world.locations[RETURN].flags,
            before.world.locations[RETURN].flags
        );
        assert_eq!(after.world.time, before.world.time + 12);
        assert_eq!(
            after.character.resources["coin"],
            before.character.resources["coin"] - 3
        );
        assert_eq!(
            after.character.resources["stamina"],
            before.character.resources["stamina"]
        );
        assert_eq!(after.entropy, before.entropy);
        assert_eq!(after.world.npcs[BRANN].location, BAY);
        assert_eq!(after.world.npcs[DARO].location, ASH);
        for (id, npc) in &before.world.npcs {
            if id != BRANN && id != DARO {
                assert_eq!(&after.world.npcs[id], npc);
            }
        }
        let returned = act(after, &content, "world.enter_aftermath");
        assert_eq!(returned.world.flags, before.world.flags);
        assert_eq!(
            returned.world.locations[RETURN].flags,
            before.world.locations[RETURN].flags
        );
        assert_eq!(returned.world.npcs[BRANN].location, BAY);
        assert_eq!(returned.world.npcs[DARO].location, ASH);
    }
}

#[test]
fn first_staffing_after_long_old_world_traversal_keeps_stock_and_original_knowledge_times() {
    let content = content();
    let mut state = run(clerk(&content, "saved-worker"), &content, HOLD);
    while state.world.time < 127 {
        state = run(
            state,
            &content,
            &[
                ("travel_adjacent", Some("lowsail.docks")),
                ("travel_adjacent", Some("lowsail_market")),
                ("travel_adjacent", Some("lowsail.levee")),
                ("world.enter_aftermath", None),
            ],
        );
    }
    state = act(state, &content, "wait_tide");
    state = act(state, &content, "wait_tide");
    assert_eq!(state.world.time, 129);
    assert_eq!(state.world.npcs[DARO].inventory[FILTER], 1);
    assert!(!state.world.npcs[BRANN].knows(ACCOUNT));
    let state = staff_from_return(state, &content);
    assert_eq!(
        (
            state.world.time,
            state.character.resources["coin"],
            state.character.resources["stamina"]
        ),
        (141, 7, 3)
    );
    assert_eq!(state.world.npcs[BRANN].knowledge[ACCOUNT].turn, 135);
    assert_eq!(state.world.npcs[BRANN].knowledge[CLEAR].turn, 137);
    assert_eq!(
        state.world.npcs[BRANN].memories["fume_yards.returned_from_rack"].turn,
        140
    );
    assert_eq!(surge(&state), vec![(16, false)]);
    let brann = state.world.npcs[BRANN].clone();
    let daro = state.world.npcs[DARO].clone();
    let state = act(state, &content, "fume_yards.enter_ash_hatch");
    absent(
        &state,
        &content,
        &[LIFT, BACK, REPORT, "fume_yards.recover_braced_filter"],
    );
    let text = content.observe(&state).unwrap().text;
    assert!(
        text.contains("Daro remains here") && text.contains("Brann returned"),
        "{text}"
    );
    let state = act(state, &content, "fume_yards.leave_ash_hatch");
    assert_eq!(state.world.npcs[BRANN], brann);
    assert_eq!(state.world.npcs[DARO], daro);
}

fn water_after20(state: GameState, content: &CompiledContent) -> GameState {
    run(
        state,
        content,
        &[
            ("travel_adjacent", Some(WORK)),
            ("fume_yards.take_stock", None),
            ("fume_yards.press_repair_plugs", None),
            ("world.enter_aftermath", None),
            ("return.patch_stand", None),
            ("return.order_water_stand", None),
            ("return.fit_market_filter", None),
            ("return.visit_workshop", None),
            ("travel_adjacent", Some(BAY)),
            ("fume_yards.take_market_cask", None),
            ("fume_yards.escort_market_cask", None),
            ("return.install_market_cask", None),
            ("return.draw_clean_water", None),
        ],
    )
}

#[test]
fn saved_stamina_carries_through_real_market_water_and_spent_stock_revisits() {
    let content = content();
    let staffed = water_after20(staffed20(&content), &content);
    let ordinary = water_after20(ordinary20(&content), &content);
    assert_eq!(staffed.world.time, 33);
    assert_eq!(ordinary.world.time, 33);
    assert_eq!(staffed.character.resources["coin"], 10);
    assert_eq!(ordinary.character.resources["coin"], 10);
    assert_eq!(staffed.character.resources["stamina"], 5);
    assert_eq!(ordinary.character.resources["stamina"], 3);
    assert_eq!(
        staffed.character.inventory,
        BTreeMap::from([("rope".into(), 1)])
    );
    assert_eq!(ordinary.character.inventory, staffed.character.inventory);
    for state in [&staffed, &ordinary] {
        absent(
            state,
            &content,
            &[
                "return.draw_clean_water",
                "return.install_market_cask",
                "return.fit_market_filter",
            ],
        );
        assert_eq!(state.world.npcs["fume_yards.pera_senn"].location, RETURN);
        assert_eq!(
            state.world.npcs["oren_pell"].knowledge["fume_yards.market_cask"].turn,
            30
        );
        assert_eq!(
            state.world.npcs[BRANN].inventory,
            BTreeMap::from([("fume_yards.fuel".into(), 1)])
        );
        assert!(
            state.world.storages["fume_yards.collateral_cage"]
                .inventory
                .is_empty()
        );
        assert!(
            state.world.npcs["fume_yards.nessa_tern"]
                .inventory
                .is_empty()
        );
        assert!(state.world.npcs[DARO].inventory.is_empty());
    }
    let old_npcs = staffed.world.npcs.clone();
    let resources = staffed.character.resources.clone();
    let state = act(staffed, &content, "return.visit_workshop");
    absent(&state, &content, &["fume_yards.take_stock"]);
    let state = travel(state, &content, ASH);
    absent(
        &state,
        &content,
        &[LIFT, BACK, REPORT, "fume_yards.buy_collateral_filter"],
    );
    let text = content.observe(&state).unwrap().text;
    assert!(
        text.contains("Daro remains here") && text.contains("Brann returned"),
        "{text}"
    );
    let state = act(state, &content, "fume_yards.leave_ash_hatch");
    absent(
        &state,
        &content,
        &[STORY, ASSIGN, "fume_yards.load_cold_freight"],
    );
    assert_eq!(state.world.npcs, old_npcs);
    assert_eq!(state.character.resources, resources);
}

#[test]
fn staffing_after_completed_protected_freight_does_not_promise_to_reopen_kiln_work() {
    let content = content();
    let state = act(common13(&content), &content, "fume_yards.load_cold_freight");
    let state = act(state, &content, STORY);
    let text = content.observe(&state).unwrap().text;
    assert!(
        !text.contains("kiln work waits"),
        "staffing offer promises work after its completion: {text}"
    );
    let result = content.action_result(&state, ASSIGN).unwrap();
    assert!(
        !result.contains("Kiln work waits"),
        "assignment promises completed kiln work: {result}"
    );
    let state = act(state, &content, ASSIGN);
    let state = act(state, &content, "fume_yards.leave_ash_hatch");
    let text = content.observe(&state).unwrap().text;
    assert!(
        !text.contains("resume kiln work"),
        "foreman absence promises completed kiln work: {text}"
    );
    assert_eq!(state.world.npcs[BRANN].location, ASH);
    let state = act(state, &content, "fume_yards.enter_ash_hatch");
    let state = act(state, &content, LIFT);
    let state = act(state, &content, BACK);
    assert_eq!(state.character.resources["coin"], 10);
    assert_eq!(state.character.resources["stamina"], 3);
    assert_eq!(state.character.inventory[FILTER], 1);
    assert!(flag(&state, BAY, "fume_yards.kiln_closed"));
    assert!(flag(&state, BAY, "fume_yards.kiln_freight_loaded"));
    absent(
        &state,
        &content,
        &[
            ASSIGN,
            "fume_yards.load_cold_freight",
            "fume_yards.load_filtered_kiln_freight",
            "fume_yards.ignite_batch",
        ],
    );
}

#[test]
fn spoiled_unprotected_freight_retires_new_staffing_advice_without_hiding_ordinary_salvage() {
    let content = content();
    for already_told in [false, true] {
        let state = run(clerk(&content, "saved-worker"), &content, HOLD);
        let mut state = run(
            state,
            &content,
            &[
                ("return.visit_workshop", None),
                ("travel_adjacent", Some(BAY)),
            ],
        );
        if already_told {
            state = act(state, &content, STORY);
        }
        state = run(
            state,
            &content,
            &[
                ("travel_adjacent", Some(WORK)),
                ("fume_yards.take_stock", None),
                ("travel_adjacent", Some(BAY)),
                ("fume_yards.take_cask", None),
                ("fume_yards.take_fuel", None),
                ("fume_yards.prepare_charge", None),
                ("fume_yards.fit_wet_screen", None),
                ("fume_yards.ignite_batch", None),
                ("wait_tide", None),
                ("wait_tide", None),
                ("wait_tide", None),
                ("wait_tide", None),
            ],
        );
        assert!(flag(&state, BAY, "fume_yards.freight_spoiled"));
        assert!(!flag(&state, BAY, "fume_yards.dust_filter_fitted"));
        absent(
            &state,
            &content,
            &[STORY, ASSIGN, "fume_yards.fit_dust_filter"],
        );
        if already_told {
            let text = content.observe(&state).unwrap().text;
            assert!(text.contains("before freight work closed"), "{text}");
            assert!(!text.contains("needs an installed dust filter"), "{text}");
        }
        state = act(state, &content, "fume_yards.enter_ash_hatch");
        assert!(legal(&state, &content).contains("fume_yards.brace_rack"));
        assert_eq!(state.world.npcs[DARO].inventory[FILTER], 1);
        assert_eq!(
            state.world.storages["fume_yards.collateral_cage"].inventory[FILTER],
            1
        );
    }
}
