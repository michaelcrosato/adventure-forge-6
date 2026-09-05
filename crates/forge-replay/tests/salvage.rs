use forge_content::parse_and_compile_production;
use forge_kernel::{
    CanonicalAction, CompiledContent, EntropyState, Event, EventKind, GameState,
    KnowledgeProvenance, enumerate_legal_actions,
};
use forge_replay::{PlayerTrace, Session, Trace, TraceStep, resume_player_trace, verify};
use std::collections::BTreeMap;

const SOURCE: &str = include_str!("../../../content/split-tide.json");
const ASH: &str = "fume_yards.ash_beds";
const KILN: &str = "fume_yards.kiln_bay";
const DARO: &str = "fume_yards.daro_venn";
const BRANN: &str = "fume_yards.brann_coil";
const PERA: &str = "fume_yards.pera_senn";
const NESSA: &str = "fume_yards.nessa_tern";
const FILTER: &str = "fume_yards.filter";
const SHARD: &str = "fume_yards.shard";
const PULL: &str = "fume_yards.pull_rack_filter";
const FACT: &str = "fume_yards.rack_cleared";
const ROLLS: &[(u64, u64, u32, u32)] = &[
    (27, 10_902_710_238_276_814_474, 1, 0),
    (123, 13_032_462_758_197_477_675, 0, 1),
];

fn select(
    session: &Session<'_>,
    content: &CompiledContent,
    id: &str,
    destination: Option<&str>,
) -> CanonicalAction {
    let parameters = destination
        .map(|location| BTreeMap::from([("destination".to_owned(), location.to_owned())]))
        .unwrap_or_default();
    enumerate_legal_actions(session.state(), content)
        .unwrap()
        .into_iter()
        .find(|action| action.definition_id == id && action.parameters == parameters)
        .unwrap_or_else(|| {
            panic!(
                "missing canonical action {id} with {parameters:?} at turn {}",
                session.state().world.time
            )
        })
}

fn record(
    session: &mut Session<'_>,
    content: &CompiledContent,
    id: &str,
    destination: Option<&str>,
) -> TraceStep {
    let action = select(session, content, id, destination);
    session
        .record(&action)
        .unwrap_or_else(|error| panic!("canonical action {id}: {error}"))
}

fn hold_return(content: &CompiledContent, seed: u64) -> Session<'_> {
    let mut session = Session::new_game("ilyan", seed, content).unwrap();
    for (id, destination) in [
        ("checkpoint.show_charter", None),
        ("travel_adjacent", Some("lowsail.levee")),
        ("levee.authority_path", None),
        ("travel_adjacent", Some("red_sluice.top")),
        ("top.hold_market", None),
        ("world.enter_aftermath", None),
        ("return.count_dry_stalls", None),
    ] {
        record(&mut session, content, id, destination);
    }
    assert_eq!(session.state().world.time, 7);
    session
}

fn rack_prefix(content: &CompiledContent, seed: u64) -> Session<'_> {
    let mut session = hold_return(content, seed);
    record(&mut session, content, "return.visit_workshop", None);
    record(&mut session, content, "travel_adjacent", Some(KILN));
    record(&mut session, content, "fume_yards.enter_ash_hatch", None);
    assert_eq!(session.state().world.time, 10);
    assert_eq!(session.state().world.current_location, ASH);
    assert_eq!(session.state().world.npcs[DARO].location, ASH);
    assert_eq!(
        session.state().world.npcs[DARO].inventory.get(FILTER),
        Some(&1)
    );
    assert_eq!(session.state().entropy, EntropyState::new(seed));
    assert!(
        session
            .trace()
            .steps
            .iter()
            .all(|step| step.entropy_draws.is_empty())
    );
    session
}

fn owned(state: &GameState, item: &str) -> u32 {
    state.character.inventory.get(item).copied().unwrap_or(0)
}

fn checkpoint<'a>(session: &Session<'a>, content: &'a CompiledContent) -> Session<'a> {
    let encoded = session.player_trace().unwrap().to_json().unwrap();
    for hidden in [
        "\"events\"",
        "\"entropy\"",
        "\"entropy_draws\"",
        "\"inventory\"",
        "\"observation\"",
        "\"scheduled_events\"",
    ] {
        assert!(!encoded.contains(hidden), "safe save leaked {hidden}");
    }
    let decoded = PlayerTrace::from_json(&encoded).unwrap();
    let resumed = resume_player_trace(&decoded, content).unwrap();
    assert_eq!(resumed.state(), session.state());
    assert_eq!(resumed.trace(), session.trace());
    assert_eq!(
        resumed.player_trace().unwrap(),
        session.player_trace().unwrap()
    );
    assert_eq!(
        content.observe(resumed.state()).unwrap(),
        content.observe(session.state()).unwrap()
    );
    assert_eq!(
        enumerate_legal_actions(resumed.state(), content).unwrap(),
        enumerate_legal_actions(session.state(), content).unwrap()
    );
    let detailed = Trace::from_json(&session.trace().to_json().unwrap()).unwrap();
    assert_eq!(verify(&detailed, content).unwrap(), *session.state());
    resumed
}

fn assert_roll(step: &TraceStep, seed: u64, value: u64, turn: u64, broken: bool) {
    assert_eq!(step.entropy_before, EntropyState::new(seed));
    assert_eq!(
        step.entropy_after,
        EntropyState {
            cursor: 1,
            ..EntropyState::new(seed)
        }
    );
    assert_eq!(step.entropy_draws.len(), 1);
    let draw = &step.entropy_draws[0];
    assert_eq!(draw.before, step.entropy_before);
    assert_eq!(draw.after, step.entropy_after);
    assert_eq!(draw.value, value);
    let mut expected = vec![
        Event {
            turn,
            kind: EventKind::NpcItemTransferredToCharacter {
                npc: DARO.to_owned(),
                item: FILTER.to_owned(),
                count: 1,
            },
        },
        Event {
            turn,
            kind: EventKind::RandomDraw {
                algorithm: "splitmix64-v1".to_owned(),
                cursor: 0,
                value,
            },
        },
    ];
    if broken {
        expected.push(Event {
            turn,
            kind: EventKind::RecipeApplied {
                recipe: "fume_yards.break_filter".to_owned(),
                inputs: BTreeMap::from([(FILTER.to_owned(), 1)]),
                outputs: BTreeMap::from([(SHARD.to_owned(), 1)]),
            },
        });
    }
    let actual: Vec<_> = step
        .events
        .iter()
        .filter(|event| {
            matches!(
                event.kind,
                EventKind::NpcItemTransferredToCharacter { .. }
                    | EventKind::RandomDraw { .. }
                    | EventKind::RecipeApplied { .. }
            )
        })
        .cloned()
        .collect();
    assert_eq!(
        actual, expected,
        "transfer, draw, and optional break must occur in exact order"
    );
}

fn assert_rack_retired(
    session: &mut Session<'_>,
    content: &CompiledContent,
    stale: &CanonicalAction,
) {
    assert!(
        !session.state().world.npcs[DARO]
            .inventory
            .contains_key(FILTER)
    );
    assert!(session.state().world.locations[ASH].flags.contains(FACT));
    let legal = enumerate_legal_actions(session.state(), content).unwrap();
    for id in [
        PULL,
        "fume_yards.brace_rack",
        "fume_yards.recover_braced_filter",
        "fume_yards.thread_rack_filter",
    ] {
        assert!(!legal.iter().any(|action| action.definition_id == id));
    }
    let state = session.state().clone();
    let trace = session.trace().clone();
    let save = session.player_trace().unwrap();
    assert!(session.record(stale).is_err());
    assert_eq!(session.state(), &state);
    assert_eq!(session.trace(), &trace);
    assert_eq!(session.player_trace().unwrap(), save);
}

#[test]
fn production_salvage_boundary_rolls_replay_before_and_after_without_reroll() {
    let content = parse_and_compile_production(SOURCE).unwrap();
    for &(seed, value, filters, shards) in ROLLS {
        let mut session = rack_prefix(&content, seed);
        let mut resumed = checkpoint(&session, &content);
        let stale = select(&session, &content, PULL, None);
        let step = session.record(&stale).unwrap();
        assert_roll(&step, seed, value, 10, shards == 1);
        assert_eq!(
            (
                owned(session.state(), FILTER),
                owned(session.state(), SHARD)
            ),
            (filters, shards)
        );
        assert_eq!(session.state().entropy.cursor, 1);
        assert_eq!(session.state().entropy.seed, seed);
        assert_eq!(session.state().world.time, 11);
        assert_eq!(session.state().character.resources["coin"], 10);
        assert_eq!(session.state().character.resources["stamina"], 3);
        assert_eq!(
            session.state().world.npcs[NESSA]
                .inventory
                .get("fume_yards.clay"),
            Some(&2)
        );
        assert_eq!(
            session.state().world.npcs[NESSA]
                .inventory
                .get("fume_yards.mesh"),
            Some(&1)
        );
        assert_eq!(
            session.state().world.npcs[BRANN]
                .inventory
                .get("fume_yards.fuel"),
            Some(&1)
        );
        assert_eq!(
            session.state().world.npcs[PERA]
                .inventory
                .get("fume_yards.water_cask"),
            Some(&1)
        );
        assert!(
            !session.state().world.npcs[BRANN]
                .knowledge
                .contains_key(FACT)
        );
        let acquired = &session.state().world.npcs[DARO].knowledge[FACT];
        assert_eq!(acquired.turn, 10);
        assert_eq!(acquired.provenance, KnowledgeProvenance::Witnessed);
        let replayed = resumed.record(&stale).unwrap();
        assert_eq!(replayed, step);
        assert_eq!(resumed.state(), session.state());
        assert_eq!(resumed.trace(), session.trace());
        assert_rack_retired(&mut session, &content, &stale);
        checkpoint(&session, &content);

        // Detailed corruption cannot become trusted by retaining otherwise
        // correct state and observation claims.
        for field in ["value", "cursor", "event", "after"] {
            let mut trace = session.trace().clone();
            let step = trace.steps.last_mut().unwrap();
            match field {
                "value" => step.entropy_draws[0].value ^= 1,
                "cursor" => step.entropy_draws[0].before.cursor += 1,
                "event" => {
                    let event = step
                        .events
                        .iter_mut()
                        .find(|event| matches!(event.kind, EventKind::RandomDraw { .. }))
                        .unwrap();
                    let EventKind::RandomDraw { value, .. } = &mut event.kind else {
                        unreachable!()
                    };
                    *value ^= 1;
                }
                "after" => step.entropy_after.cursor += 1,
                _ => unreachable!(),
            }
            assert!(
                verify(&trace, &content).is_err(),
                "accepted altered entropy {field}"
            );
        }
    }
}

#[test]
fn salvage_catalogs_do_not_peek_at_entropy_or_spend_draws() {
    let content = parse_and_compile_production(SOURCE).unwrap();
    let sessions = [rack_prefix(&content, 27), rack_prefix(&content, 123)];
    let mut semantic_sets = Vec::new();
    for session in &sessions {
        let before = session.state().clone();
        let full = enumerate_legal_actions(session.state(), &content).unwrap();
        let mut paged_ids = Vec::new();
        for offset in (0..full.len()).step_by(2) {
            let page = content.action_page(session.state(), offset, 2).unwrap();
            paged_ids.extend(page.actions.into_iter().map(|action| action.action_id));
        }
        assert_eq!(
            paged_ids,
            full.iter()
                .map(|action| action.action_id.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            full,
            enumerate_legal_actions(session.state(), &content).unwrap()
        );
        assert!(full.iter().any(|action| action.definition_id == PULL));
        assert_eq!(session.state(), &before);
        assert_eq!(session.state().entropy.cursor, 0);
        semantic_sets.push(
            full.into_iter()
                .map(|action| (action.definition_id, action.parameters))
                .collect::<Vec<_>>(),
        );
    }
    assert_eq!(semantic_sets[0], semantic_sets[1]);
    assert_ne!(
        sessions[0].state().state_id(),
        sessions[1].state().state_id()
    );
}

#[test]
fn failed_rack_recovery_preserves_an_already_manufactured_filter() {
    let content = parse_and_compile_production(SOURCE).unwrap();
    let mut session = hold_return(&content, 123);
    for (id, destination) in [
        ("return.visit_workshop", None),
        ("fume_yards.take_stock", None),
        ("travel_adjacent", Some(KILN)),
        ("fume_yards.take_cask", None),
        ("fume_yards.take_fuel", None),
        ("fume_yards.prepare_charge", None),
        ("fume_yards.fit_wet_screen", None),
        ("fume_yards.ignite_batch", None),
        ("wait_tide", None),
        ("fume_yards.draw_filter", None),
        ("fume_yards.enter_ash_hatch", None),
    ] {
        record(&mut session, &content, id, destination);
    }
    assert_eq!(session.state().world.time, 18);
    assert_eq!(owned(session.state(), FILTER), 1);
    assert_eq!(owned(session.state(), SHARD), 0);
    assert_eq!(session.state().entropy.cursor, 0);
    let mut resumed = checkpoint(&session, &content);
    let stale = select(&session, &content, PULL, None);
    let step = session.record(&stale).unwrap();
    assert_roll(&step, 123, 13_032_462_758_197_477_675, 18, true);
    assert_eq!(
        (
            owned(session.state(), FILTER),
            owned(session.state(), SHARD)
        ),
        (1, 1)
    );
    assert_eq!(session.state().world.time, 19);
    assert!(step.observation.text.contains("rack filter"));
    assert!(step.observation.text.contains("broke"));
    assert!(!step.observation.text.contains("no filter"));
    assert!(
        session.state().world.locations[KILN]
            .flags
            .contains("fume_yards.batch_drawn")
    );
    assert!(
        !session.state().world.locations[KILN]
            .flags
            .contains("fume_yards.batch_spoiled")
    );
    let spoil: Vec<_> = step
        .events
        .iter()
        .filter_map(|event| match &event.kind {
            EventKind::ScheduledEventResolved {
                event_id, applied, ..
            } if event_id == "fume_yards.batch_spoil" => Some((event.turn, *applied)),
            _ => None,
        })
        .collect();
    assert_eq!(spoil, vec![(19, false)]);
    assert_eq!(resumed.record(&stale).unwrap(), step);
    assert_eq!(resumed.state(), session.state());
    assert_rack_retired(&mut session, &content, &stale);
    checkpoint(&session, &content);
}

#[test]
fn safe_salvage_report_moves_its_source_before_teaching_the_uninformed_foreman() {
    let content = parse_and_compile_production(SOURCE).unwrap();
    let mut recovered = rack_prefix(&content, 71);
    record(&mut recovered, &content, "fume_yards.brace_rack", None);
    let recovery = record(
        &mut recovered,
        &content,
        "fume_yards.recover_braced_filter",
        None,
    );
    assert_eq!(recovered.state().world.time, 12);
    assert_eq!(recovered.state().character.resources["stamina"], 1);
    assert_eq!(owned(recovered.state(), "rope"), 1);
    assert_eq!(
        (
            owned(recovered.state(), FILTER),
            owned(recovered.state(), SHARD)
        ),
        (1, 0)
    );
    assert_eq!(recovered.state().entropy, EntropyState::new(71));
    assert!(recovery.entropy_draws.is_empty());
    assert!(recovery.events.contains(&Event {
        turn: 11,
        kind: EventKind::NpcItemTransferredToCharacter {
            npc: DARO.to_owned(),
            item: FILTER.to_owned(),
            count: 1
        }
    }));
    assert_eq!(recovered.state().world.npcs[DARO].knowledge[FACT].turn, 11);
    assert!(
        !recovered.state().world.npcs[BRANN]
            .knowledge
            .contains_key(FACT)
    );
    assert_rack_retired(&mut recovered, &content, &recovery.action);
    let mut uninformed = checkpoint(&recovered, &content);
    let mut informed = checkpoint(&recovered, &content);
    record(
        &mut uninformed,
        &content,
        "fume_yards.leave_ash_hatch",
        None,
    );
    let report = record(&mut informed, &content, "fume_yards.report_with_daro", None);
    assert_eq!(informed.state().world.time, 13);
    assert_eq!(uninformed.state().world.time, 13);
    assert_eq!(informed.state().world.current_location, KILN);
    assert_eq!(uninformed.state().world.current_location, KILN);
    assert_eq!(informed.state().world.npcs[DARO].location, KILN);
    assert_eq!(uninformed.state().world.npcs[DARO].location, ASH);
    assert!(
        !uninformed.state().world.npcs[BRANN]
            .knowledge
            .contains_key(FACT)
    );
    let knowledge = &informed.state().world.npcs[BRANN].knowledge[FACT];
    assert_eq!(knowledge.turn, 12);
    assert_eq!(
        knowledge.provenance,
        KnowledgeProvenance::Told {
            by: DARO.to_owned()
        }
    );
    assert_eq!(informed.state().world.npcs[DARO].knowledge[FACT].turn, 11);
    assert_eq!(
        informed.state().world.npcs[DARO].knowledge[FACT].provenance,
        KnowledgeProvenance::Witnessed
    );
    assert_eq!(informed.state().character.resources["coin"], 11);
    assert_eq!(uninformed.state().character.resources["coin"], 10);
    let events: Vec<_> = report
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
        .cloned()
        .collect();
    assert_eq!(
        events,
        vec![
            Event {
                turn: 12,
                kind: EventKind::NpcMoved {
                    npc: DARO.to_owned(),
                    from: ASH.to_owned(),
                    to: KILN.to_owned()
                }
            },
            Event {
                turn: 12,
                kind: EventKind::Moved {
                    from: ASH.to_owned(),
                    to: KILN.to_owned()
                }
            },
            Event {
                turn: 12,
                kind: EventKind::NpcKnowledgeAdded {
                    npc: BRANN.to_owned(),
                    knowledge: FACT.to_owned()
                }
            },
            Event {
                turn: 12,
                kind: EventKind::ResourceAdjusted {
                    resource: "coin".to_owned(),
                    amount: 1
                }
            },
        ]
    );
    assert!(
        informed.state().world.locations[KILN]
            .entities
            .contains(DARO)
    );
    assert!(
        !informed.state().world.locations[ASH]
            .entities
            .contains(DARO)
    );
    for npc in [NESSA, PERA, "oren_pell"] {
        assert!(
            !informed.state().world.npcs[npc]
                .knowledge
                .contains_key(FACT)
        );
    }
    checkpoint(&informed, &content);
    checkpoint(&uninformed, &content);
}
