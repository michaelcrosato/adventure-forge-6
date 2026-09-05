use forge_content::{compile_production, parse, parse_and_compile_production};
use forge_kernel::{
    CompiledContent, EventKind, GameState, KnowledgeProvenance, Memory, enumerate_legal_actions,
};
use forge_replay::{PlayerTrace, Session, Trace, resume_player_trace, verify};

const SPLIT_TIDE: &str = include_str!("../../../content/split-tide.json");

fn record(
    session: &mut Session<'_>,
    content: &CompiledContent,
    definition_id: &str,
    destination: Option<&str>,
) {
    let parameters = destination
        .map(|value| ("destination".to_owned(), value.to_owned()))
        .into_iter()
        .collect();
    let action = enumerate_legal_actions(session.state(), content)
        .expect("production actions enumerate")
        .into_iter()
        .find(|action| action.definition_id == definition_id && action.parameters == parameters)
        .unwrap_or_else(|| panic!("expected canonical action {definition_id}"));
    session.record(&action).expect("canonical action records");
}

fn assert_towline_memory(state: &GameState) {
    // This is an authored semantic oracle, independent of the current reducer,
    // compiled effect program, serialized checkpoint, and replayed state.
    let expected = Memory {
        id: "oren_saw_towline".to_owned(),
        subject: "Oren watched the player rig a paid towline.".to_owned(),
        turn: 2,
        provenance: KnowledgeProvenance::Witnessed,
    };
    assert_eq!(
        state.world.npcs["oren_pell"]
            .memories
            .get("oren_saw_towline"),
        Some(&expected),
        "remote NPC memory was lost or changed"
    );
}

fn save_and_resume<'content>(
    session: &Session<'_>,
    content: &'content CompiledContent,
) -> Session<'content> {
    let saved = session
        .player_trace()
        .expect("production session exports a safe trace")
        .to_json()
        .expect("safe trace serializes");
    assert!(!saved.contains("oren_saw_towline"));
    let resumed = resume_player_trace(
        &PlayerTrace::from_json(&saved).expect("safe trace decodes"),
        content,
    )
    .expect("safe trace reconstructs");
    assert_towline_memory(resumed.state());
    assert_eq!(resumed.state(), session.state());
    assert_eq!(resumed.trace().final_receipt, session.trace().final_receipt);
    resumed
}

#[test]
fn remote_npc_memory_survives_movement_save_replay_and_return() {
    let content = parse_and_compile_production(SPLIT_TIDE).expect("production content compiles");
    let mut session = Session::new_game("rook", 71, &content).expect("Rook session starts");
    assert!(
        !session.state().world.npcs["oren_pell"]
            .memories
            .contains_key("oren_saw_towline")
    );
    record(
        &mut session,
        &content,
        "travel_adjacent",
        Some("lowsail.docks"),
    );
    record(&mut session, &content, "docks.ring_warning", None);
    record(&mut session, &content, "docks.rig_towline", None);
    assert_eq!(session.state().world.time, 3);
    assert_eq!(session.state().world.current_location, "lowsail.levee");
    assert_eq!(
        session.state().world.npcs["oren_pell"].location,
        "lowsail.docks"
    );
    assert_towline_memory(session.state());

    let purchase_events = &session.trace().steps[2].events;
    let memory_position = purchase_events
        .iter()
        .position(|event| {
            event.turn == 2
                && matches!(
                    &event.kind,
                    EventKind::NpcMemoryAdded { npc, memory }
                        if npc == "oren_pell" && memory == "oren_saw_towline"
                )
        })
        .expect("Oren witnesses the towline at turn two");
    let movement_position = purchase_events
        .iter()
        .position(|event| {
            matches!(
                &event.kind,
                EventKind::Moved { from, to }
                    if from == "lowsail.docks" && to == "lowsail.levee"
            )
        })
        .expect("the paid route leaves Oren at the docks");
    assert!(memory_position < movement_position);

    let mut resumed = save_and_resume(&session, &content);
    for (definition_id, destination) in [
        ("levee.relay_warning", None),
        ("levee.culvert_path", None),
        ("floor.open_relief", None),
        ("travel_adjacent", Some("red_sluice.top")),
        ("top.check_wheels", None),
        ("top.divert_relief", None),
        ("world.enter_aftermath", None),
        ("return.move_inland", None),
    ] {
        record(&mut session, &content, definition_id, destination);
        record(&mut resumed, &content, definition_id, destination);
        assert_towline_memory(session.state());
        assert_towline_memory(resumed.state());
    }
    assert_eq!(session.state().world.time, 11);
    assert_eq!(session.state().world.current_location, "lowsail.return");
    assert_eq!(
        session.state().world.npcs["oren_pell"].location,
        "lowsail.return"
    );
    assert!(session.state().world.flags.contains("flow_relief"));
    assert_eq!(resumed.state(), session.state());
    assert_eq!(
        resumed.player_trace().unwrap(),
        session.player_trace().unwrap()
    );

    let detailed = Trace::from_json(&session.trace().to_json().unwrap()).unwrap();
    let replayed = verify(&detailed, &content).expect("build-bound detailed trace replays");
    assert_towline_memory(&replayed);
    assert_eq!(&replayed, session.state());
    save_and_resume(&session, &content);
}

#[test]
fn production_prose_rejects_sentence_over_eighteen_words() {
    let accepted = "Sava guards the dry checkpoint while workers carry heavy rope toward the docks beneath the red tide warning.";
    let rejected = "Sava guards the dry checkpoint while workers carry heavy rope toward the docks beneath the bright red tide warning.";
    assert_eq!(accepted.split_whitespace().count(), 18);
    assert_eq!(rejected.split_whitespace().count(), 19);

    let mut source = parse(SPLIT_TIDE).expect("production source parses");
    let checkpoint = source
        .locations
        .iter()
        .position(|location| location.id == "lowsail_market")
        .expect("production checkpoint exists");
    source.locations[checkpoint].description = accepted.to_owned();
    compile_production(source.clone()).expect("exactly eighteen words remain admissible");

    // Only one adjective changes. All actions, variants, supplies, event
    // budgets, references, and the production contract remain identical.
    source.locations[checkpoint].description = rejected.to_owned();
    let error =
        compile_production(source).expect_err("production prose admitted a nineteen-word sentence");
    assert_eq!(
        error.issues,
        ["location lowsail_market description sentence exceeds 18 words"],
        "prose rejection must identify the actual sentence boundary"
    );
}
