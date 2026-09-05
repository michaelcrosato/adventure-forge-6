use forge_content::parse_and_compile_production;
use forge_kernel::{
    CanonicalAction, CharacterChoiceSelection, CharacterSelection, CompiledContent, EventKind,
    KnowledgeProvenance, enumerate_legal_actions,
};
use forge_replay::{PlayerTrace, Session, Trace, resume_player_trace, verify};
use std::collections::BTreeMap;
const SOURCE: &str = include_str!("../../../content/split-tide.json");
const W: &str = "fume_yards.workshop";
const K: &str = "fume_yards.kiln_bay";
const NESSA: &str = "fume_yards.nessa_tern";
const BRANN: &str = "fume_yards.brann_coil";
const PERA: &str = "fume_yards.pera_senn";
const TEST: &str = "fume_yards.charge_dust_test";
type Action = (&'static str, Option<&'static str>);
const PREFIX: &[Action] = &[
    ("checkpoint.show_charter", None),
    ("travel_adjacent", Some("lowsail.levee")),
    ("levee.authority_path", None),
    ("travel_adjacent", Some("red_sluice.top")),
    ("top.hold_market", None),
    ("world.enter_aftermath", None),
    ("return.count_dry_stalls", None),
    ("return.visit_workshop", None),
    ("fume_yards.take_stock", None),
    ("travel_adjacent", Some(K)),
    ("fume_yards.prepare_charge", None),
    ("travel_adjacent", Some(W)),
];
const WATER: &[Action] = &[
    ("world.enter_aftermath", None),
    ("return.patch_stand", None),
    ("return.order_water_stand", None),
    ("return.visit_workshop", None),
    ("travel_adjacent", Some("fume_yards.ash_beds")),
    ("fume_yards.buy_collateral_filter", None),
    ("travel_adjacent", Some(W)),
    ("travel_adjacent", Some(K)),
    ("fume_yards.take_market_cask", None),
    ("fume_yards.escort_market_cask", None),
    ("return.fit_market_filter", None),
    ("return.install_market_cask", None),
    ("return.draw_clean_water", None),
];
fn start<'a>(content: &'a CompiledContent) -> Session<'a> {
    let selection = CharacterSelection {
        name: "Cold shift comparison".into(),
        choices: [
            ("lineage", "fenborn"),
            ("origin", "lowsail"),
            ("calling", "ledger-clerk"),
            ("value", "order"),
            ("burden", "wanted"),
            ("history", "stole-permit"),
        ]
        .into_iter()
        .map(|(slot, choice)| CharacterChoiceSelection {
            slot_id: slot.into(),
            choice_id: choice.into(),
        })
        .collect(),
    };
    Session::new_custom_game(&selection, 71, content).unwrap()
}

fn select(
    session: &Session<'_>,
    content: &CompiledContent,
    (id, destination): Action,
) -> CanonicalAction {
    let parameters = destination
        .map(|to| BTreeMap::from([("destination".into(), to.into())]))
        .unwrap_or_default();
    enumerate_legal_actions(session.state(), content)
        .unwrap()
        .into_iter()
        .find(|action| action.definition_id == id && action.parameters == parameters)
        .unwrap_or_else(|| panic!("missing {id} at {}", session.state().world.time))
}

fn record(session: &mut Session<'_>, content: &CompiledContent, action: Action) {
    session.record(&select(session, content, action)).unwrap();
}

fn checkpoint<'a>(session: &Session<'a>, content: &'a CompiledContent) -> Session<'a> {
    let bytes = session.player_trace().unwrap().to_json().unwrap();
    for private in [
        "\"inventory\"",
        "\"storages\"",
        "\"knowledge\"",
        "\"events\"",
        "\"entropy\"",
    ] {
        assert!(!bytes.contains(private));
    }
    let resumed = resume_player_trace(&PlayerTrace::from_json(&bytes).unwrap(), content).unwrap();
    assert_eq!(resumed.state(), session.state());
    assert_eq!(resumed.trace(), session.trace());
    assert_eq!(resumed.player_trace().unwrap().to_json().unwrap(), bytes);
    assert_eq!(
        content.observe(resumed.state()).unwrap(),
        content.observe(session.state()).unwrap()
    );
    assert_eq!(
        enumerate_legal_actions(resumed.state(), content).unwrap(),
        enumerate_legal_actions(session.state(), content).unwrap()
    );
    assert_eq!(
        verify(
            &Trace::from_json(&session.trace().to_json().unwrap()).unwrap(),
            content
        )
        .unwrap(),
        *session.state()
    );
    resumed
}

fn reject_stale(session: &mut Session<'_>, action: &CanonicalAction) {
    let state = session.state().clone();
    let trace = session.trace().clone();
    let save = session.player_trace().unwrap().to_json().unwrap();
    assert!(session.record(action).is_err());
    assert_eq!(session.state(), &state);
    assert_eq!(session.trace(), &trace);
    assert_eq!(session.player_trace().unwrap().to_json().unwrap(), save);
}

fn prepared<'a>(content: &'a CompiledContent) -> Session<'a> {
    let mut session = start(content);
    for &action in PREFIX {
        record(&mut session, content, action);
    }
    session
}
fn relayed<'a>(content: &'a CompiledContent) -> Session<'a> {
    let mut session = prepared(content);
    record(
        &mut session,
        content,
        ("fume_yards.test_unfired_charge", None),
    );
    record(&mut session, content, ("fume_yards.report_test", None));
    session
}
fn assert_source(session: &Session<'_>) {
    let state = session.state();
    let source = &state.world.npcs[NESSA].knowledge[TEST];
    let report = &state.world.npcs[BRANN].knowledge[TEST];
    assert_eq!(
        (source.turn, &source.provenance),
        (12, &KnowledgeProvenance::Witnessed)
    );
    assert_eq!(
        (report.turn, &report.provenance),
        (13, &KnowledgeProvenance::Told { by: NESSA.into() })
    );
}
#[test]
fn cold_shift_native_sources_and_matched_report_saves_preserve_three_step_continuation() {
    let content = parse_and_compile_production(SOURCE).unwrap();
    let p12 = prepared(&content);
    let mut untested = checkpoint(&p12, &content);
    record(&mut untested, &content, ("wait_tide", None));
    record(&mut untested, &content, ("travel_adjacent", Some(K)));
    untested = checkpoint(&untested, &content);
    assert!(
        !untested.state().world.npcs[NESSA]
            .knowledge
            .contains_key(TEST)
    );
    let mut tested = checkpoint(&p12, &content);
    let test = select(&tested, &content, ("fume_yards.test_unfired_charge", None));
    tested.record(&test).unwrap();
    tested = checkpoint(&tested, &content);
    reject_stale(&mut tested, &test);
    assert_eq!(tested.state().world.time, 13);
    assert_eq!(tested.state().world.npcs[NESSA].knowledge[TEST].turn, 12);
    let mut uninformed = checkpoint(&tested, &content);
    record(&mut uninformed, &content, ("travel_adjacent", Some(K)));
    uninformed = checkpoint(&uninformed, &content);
    assert!(
        !uninformed.state().world.npcs[BRANN]
            .knowledge
            .contains_key(TEST)
    );
    assert_eq!(uninformed.state().world.npcs[NESSA].location, W);
    let mut informed = checkpoint(&tested, &content);
    let escort = select(&informed, &content, ("fume_yards.report_test", None));
    informed.record(&escort).unwrap();
    informed = checkpoint(&informed, &content);
    reject_stale(&mut informed, &escort);
    assert_eq!(informed.state().world.npcs[NESSA].location, K);
    assert_source(&informed);
    assert_eq!(informed.state().character, uninformed.state().character);
    assert_eq!(informed.state().world.time, 14);
    for other in [&untested, &uninformed] {
        assert!(
            !enumerate_legal_actions(other.state(), &content)
                .unwrap()
                .iter()
                .any(|a| a.definition_id == "fume_yards.delegate_cold_shift")
        );
    }
    let shift = select(
        &informed,
        &content,
        ("fume_yards.delegate_cold_shift", None),
    );
    let mut repeated = checkpoint(&informed, &content);
    informed.record(&shift).unwrap();
    repeated.record(&shift).unwrap();
    assert_eq!(informed.trace(), repeated.trace());
    assert_eq!(
        informed.player_trace().unwrap().to_json().unwrap(),
        repeated.player_trace().unwrap().to_json().unwrap()
    );
    informed = checkpoint(&informed, &content);
    reject_stale(&mut informed, &shift);
    assert_eq!(informed.state().world.time, 17);
    assert_eq!(informed.trace().steps.len(), 15);
    assert_eq!(
        informed.state().character.resources,
        BTreeMap::from([("coin".into(), 13), ("stamina".into(), 3)])
    );
    assert_eq!(
        informed.state().character.inventory,
        BTreeMap::from([("rope".into(), 1), ("fume_yards.repair_lot".into(), 1)])
    );
    assert_eq!(informed.state().world.npcs[BRANN].location, W);
    assert_eq!(
        informed.state().world.npcs[BRANN].inventory["fume_yards.fuel"],
        1
    );
    assert_source(&informed);
    let events = &informed.trace().steps[14].events;
    let recipe = events
        .iter()
        .position(|e| matches!(e.kind, EventKind::RecipeApplied { .. }))
        .unwrap();
    let payment = events
        .iter()
        .position(|e| matches!(e.kind, EventKind::ResourceAdjusted { .. }))
        .unwrap();
    let movement = events
        .iter()
        .position(|e| matches!(&e.kind,EventKind::NpcMoved{npc,..}if npc==BRANN))
        .unwrap();
    assert!(recipe < payment && payment < movement);
    assert_eq!(events[recipe].turn, 14);
    assert!(
        events
            .iter()
            .any(|e| e.turn == 14 && e.kind == EventKind::TimeAdvanced { ticks: 3 })
    );
    assert!(events.iter().any(|e| e.turn == 17
        && e.kind
            == EventKind::ScheduledEventResolved {
                event_id: "lowsail.next_surge".into(),
                event_kind: "deadline".into(),
                applied: false
            }));
}
#[test]
fn cold_shift_native_cancellation_returns_and_both_water_routes_keep_custody() {
    let content = parse_and_compile_production(SOURCE).unwrap();
    let source = relayed(&content);
    let mut cancelled = checkpoint(&source, &content);
    let nessa_return = select(&cancelled, &content, ("fume_yards.return_with_nessa", None));
    cancelled.record(&nessa_return).unwrap();
    cancelled = checkpoint(&cancelled, &content);
    reject_stale(&mut cancelled, &nessa_return);
    assert_eq!(cancelled.state().world.time, 15);
    assert_source(&cancelled);
    record(&mut cancelled, &content, ("travel_adjacent", Some(K)));
    record(
        &mut cancelled,
        &content,
        ("fume_yards.delegate_cold_shift", None),
    );
    cancelled = checkpoint(&cancelled, &content);
    assert_eq!(cancelled.state().world.time, 19);
    assert_source(&cancelled);
    assert!(
        !cancelled
            .trace()
            .steps
            .last()
            .unwrap()
            .events
            .iter()
            .any(|e| matches!(&e.kind,EventKind::NpcMoved{npc,..}if npc==NESSA))
    );
    assert!(
        !cancelled.state().world.npcs[NESSA]
            .memories
            .contains_key("fume_yards.cold_shift_completed")
    );
    for delegated in [true, false] {
        let mut session = checkpoint(&source, &content);
        let method: &[Action] = if delegated {
            &[("fume_yards.delegate_cold_shift", None)]
        } else {
            &[
                ("fume_yards.reclaim_charge", None),
                ("fume_yards.load_kiln_freight", None),
                ("fume_yards.return_with_nessa", None),
            ]
        };
        for &action in method {
            record(&mut session, &content, action);
        }
        session = checkpoint(&session, &content);
        assert_eq!(session.state().world.time, 17);
        if delegated {
            let mut returned = checkpoint(&session, &content);
            let action = select(
                &returned,
                &content,
                ("fume_yards.return_brann_to_kiln", None),
            );
            returned.record(&action).unwrap();
            returned = checkpoint(&returned, &content);
            reject_stale(&mut returned, &action);
            assert_eq!(returned.state().world.time, 18);
            assert_eq!(returned.state().world.npcs[BRANN].location, K);
            assert_eq!(
                returned.state().world.npcs[BRANN].inventory["fume_yards.fuel"],
                1
            );
        }
        for &action in WATER {
            let chosen = select(&session, &content, action);
            session.record(&chosen).unwrap();
            if matches!(session.state().world.time, 26..=30) {
                session = checkpoint(&session, &content);
                reject_stale(&mut session, &chosen);
            }
        }
        assert_eq!(session.state().world.time, 30);
        assert_eq!(session.trace().steps.len(), if delegated { 28 } else { 30 });
        assert_source(&session);
        assert_eq!(
            session.state().character.resources,
            BTreeMap::from([
                ("coin".into(), 9),
                ("stamina".into(), if delegated { 5 } else { 3 })
            ])
        );
        assert_eq!(
            session.state().character.inventory,
            BTreeMap::from([("rope".into(), 1)])
        );
        assert!(session.state().world.npcs[PERA].inventory.is_empty());
        assert_eq!(session.state().world.npcs[PERA].location, "lowsail.return");
        assert_eq!(
            session.state().world.npcs["oren_pell"].knowledge["fume_yards.market_cask"].turn,
            26
        );
        assert_eq!(
            session.state().world.npcs["oren_pell"].knowledge["fume_yards.market_cask"].provenance,
            KnowledgeProvenance::Told { by: PERA.into() }
        );
        assert!(
            session.state().world.storages["fume_yards.collateral_cage"]
                .inventory
                .is_empty()
        );
        assert_eq!(session.state().entropy.cursor, 0);
    }
}
#[test]
fn cold_shift_native_unpaid_rack_requires_restored_brann_and_pays_once() {
    const ASH: &str = "fume_yards.ash_beds";
    const DARO: &str = "fume_yards.daro_venn";
    const CLEARED: &str = "fume_yards.rack_cleared";
    const REPORT: Action = ("fume_yards.report_with_daro", None);
    let content = parse_and_compile_production(SOURCE).unwrap();
    let mut base = relayed(&content);
    record(
        &mut base,
        &content,
        ("fume_yards.delegate_cold_shift", None),
    );
    let base = checkpoint(&base, &content);

    for bring_brann_first in [true, false] {
        let mut session = checkpoint(&base, &content);
        let arrival = if bring_brann_first {
            ("fume_yards.return_brann_to_kiln", None)
        } else {
            ("travel_adjacent", Some(K))
        };
        for action in [
            arrival,
            ("fume_yards.enter_ash_hatch", None),
            ("fume_yards.brace_rack", None),
            ("fume_yards.recover_braced_filter", None),
        ] {
            record(&mut session, &content, action);
        }
        session = checkpoint(&session, &content);
        assert_eq!(session.state().world.time, 21);
        assert_eq!(session.state().world.current_location, ASH);
        assert_eq!(session.state().world.npcs[DARO].location, ASH);
        assert_eq!(session.state().world.npcs[NESSA].location, W);
        assert_eq!(
            session.state().world.npcs[BRANN].location,
            if bring_brann_first { K } else { W }
        );
        assert_eq!(
            session.state().character.resources,
            BTreeMap::from([("coin".into(), 13), ("stamina".into(), 1)])
        );
        assert_eq!(
            session.state().character.inventory,
            BTreeMap::from([
                ("rope".into(), 1),
                ("fume_yards.repair_lot".into(), 1),
                ("fume_yards.filter".into(), 1),
            ])
        );
        assert!(session.state().world.npcs[DARO].inventory.is_empty());
        assert_eq!(session.state().world.npcs[DARO].knowledge[CLEARED].turn, 20);
        assert_eq!(
            session.state().world.npcs[DARO].knowledge[CLEARED].provenance,
            KnowledgeProvenance::Witnessed
        );
        assert!(
            !session.state().world.npcs[BRANN]
                .knowledge
                .contains_key(CLEARED)
        );
        assert_source(&session);

        if !bring_brann_first {
            assert!(
                !enumerate_legal_actions(session.state(), &content)
                    .unwrap()
                    .iter()
                    .any(|action| action.definition_id == REPORT.0)
            );
            for action in [
                ("travel_adjacent", Some(W)),
                ("fume_yards.return_brann_to_kiln", None),
                ("fume_yards.enter_ash_hatch", None),
            ] {
                record(&mut session, &content, action);
            }
            session = checkpoint(&session, &content);
        }
        let pre_time = if bring_brann_first { 21 } else { 24 };
        assert_eq!(session.state().world.time, pre_time);
        let action = select(&session, &content, REPORT);
        let mut repeated = checkpoint(&session, &content);
        session.record(&action).unwrap();
        repeated.record(&action).unwrap();
        assert_eq!(session.trace(), repeated.trace());
        assert_eq!(
            session.player_trace().unwrap().to_json().unwrap(),
            repeated.player_trace().unwrap().to_json().unwrap()
        );
        session = checkpoint(&session, &content);
        reject_stale(&mut session, &action);
        assert_eq!(session.state().world.time, pre_time + 1);
        assert_eq!(
            session.state().character.resources,
            BTreeMap::from([("coin".into(), 14), ("stamina".into(), 1)])
        );
        assert_eq!(session.state().world.npcs[BRANN].location, K);
        assert_eq!(session.state().world.npcs[DARO].location, K);
        assert_eq!(
            session.state().world.npcs[BRANN].knowledge[CLEARED].turn,
            pre_time
        );
        assert_eq!(
            session.state().world.npcs[BRANN].knowledge[CLEARED].provenance,
            KnowledgeProvenance::Told { by: DARO.into() }
        );
        assert_eq!(session.state().world.npcs[DARO].knowledge[CLEARED].turn, 20);
        assert_eq!(
            session.state().world.npcs[BRANN].inventory["fume_yards.fuel"],
            1
        );
        assert_eq!(session.state().entropy.cursor, 0);
        assert_source(&session);
        let actual: Vec<_> = session
            .trace()
            .steps
            .last()
            .unwrap()
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event.kind,
                    EventKind::NpcMoved { .. }
                        | EventKind::Moved { .. }
                        | EventKind::NpcKnowledgeAdded { .. }
                        | EventKind::ResourceAdjusted { .. }
                )
            })
            .map(|event| (event.turn, event.kind.clone()))
            .collect();
        assert_eq!(
            actual,
            vec![
                (
                    pre_time,
                    EventKind::NpcMoved {
                        npc: DARO.into(),
                        from: ASH.into(),
                        to: K.into()
                    }
                ),
                (
                    pre_time,
                    EventKind::Moved {
                        from: ASH.into(),
                        to: K.into()
                    }
                ),
                (
                    pre_time,
                    EventKind::NpcKnowledgeAdded {
                        npc: BRANN.into(),
                        knowledge: CLEARED.into()
                    }
                ),
                (
                    pre_time,
                    EventKind::ResourceAdjusted {
                        resource: "coin".into(),
                        amount: 1
                    }
                ),
            ]
        );
        record(&mut session, &content, ("fume_yards.enter_ash_hatch", None));
        assert!(
            !enumerate_legal_actions(session.state(), &content)
                .unwrap()
                .iter()
                .any(|action| action.definition_id == REPORT.0)
        );
        assert_eq!(session.state().character.resources["coin"], 14);
    }
}

#[test]
fn cold_shift_native_saves_reject_forged_custody_source_and_shortened_work() {
    let content = parse_and_compile_production(SOURCE).unwrap();
    let mut session = relayed(&content);
    record(
        &mut session,
        &content,
        ("fume_yards.delegate_cold_shift", None),
    );
    checkpoint(&session, &content);
    let safe = session.player_trace().unwrap().to_json().unwrap();
    let wrong = safe.replace("stole-permit", "saved-worker");
    assert_ne!(wrong, safe);
    assert!(resume_player_trace(&PlayerTrace::from_json(&wrong).unwrap(), &content).is_err());
    for defect in ["escort", "source", "time", "recipe", "order", "payment"] {
        let mut trace = session.trace().clone();
        match defect {
            "escort" => trace.steps[13]
                .events
                .retain(|e| !matches!(&e.kind,EventKind::NpcMoved{npc,..}if npc==NESSA)),
            "source" => {
                trace.steps[12]
                    .events
                    .iter_mut()
                    .find(|e| matches!(e.kind, EventKind::NpcKnowledgeAdded { .. }))
                    .unwrap()
                    .turn = 13
            }
            "time" => {
                trace.steps[14]
                    .events
                    .iter_mut()
                    .find(|e| matches!(e.kind, EventKind::TimeAdvanced { .. }))
                    .unwrap()
                    .kind = EventKind::TimeAdvanced { ticks: 1 }
            }
            "recipe" => trace.steps[14]
                .events
                .retain(|e| !matches!(e.kind, EventKind::RecipeApplied { .. })),
            "order" => trace.steps[14].events.reverse(),
            "payment" => {
                trace.steps[14]
                    .events
                    .iter_mut()
                    .find(|e| matches!(e.kind, EventKind::ResourceAdjusted { .. }))
                    .unwrap()
                    .kind = EventKind::ResourceAdjusted {
                    resource: "coin".into(),
                    amount: 4,
                }
            }
            _ => unreachable!(),
        }
        assert!(
            verify(
                &Trace::from_json(&trace.to_json().unwrap()).unwrap(),
                &content
            )
            .is_err(),
            "accepted {defect}"
        );
    }
}
