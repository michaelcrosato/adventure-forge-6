use forge_content::parse_and_compile_production;
use forge_kernel::CanonicalAction;
use forge_replay::{Session, Trace, verify};

const SPLIT_TIDE: &str = include_str!("../../../content/split-tide.json");

fn select(
    session: &Session<'_>,
    content: &forge_kernel::CompiledContent,
    definition_id: &str,
    parameter: Option<(&str, &str)>,
) -> CanonicalAction {
    forge_kernel::enumerate_legal_actions(session.state(), content)
        .expect("real state must enumerate")
        .into_iter()
        .find(|action| {
            action.definition_id == definition_id
                && parameter.is_none_or(|(name, value)| {
                    action
                        .parameters
                        .get(name)
                        .is_some_and(|found| found == value)
                })
        })
        .unwrap_or_else(|| panic!("missing legal action {definition_id}"))
}

fn content() -> forge_kernel::CompiledContent {
    parse_and_compile_production(SPLIT_TIDE).expect("Split Tide must compile")
}

fn record(
    session: &mut Session<'_>,
    content: &forge_kernel::CompiledContent,
    definition_id: &str,
    parameter: Option<(&str, &str)>,
) {
    let action = select(session, content, definition_id, parameter);
    session
        .record(&action)
        .unwrap_or_else(|error| panic!("recording {definition_id} failed: {error}"));
}

#[test]
fn real_split_tide_path_round_trips_and_replays() {
    let content = content();
    let mut session = Session::new_game("ilyan", 71, &content).expect("session starts");

    record(&mut session, &content, "checkpoint.audit_order", None);
    record(&mut session, &content, "checkpoint.show_charter", None);
    record(
        &mut session,
        &content,
        "travel_adjacent",
        Some(("destination", "lowsail.levee")),
    );
    record(&mut session, &content, "levee.authority_path", None);
    record(&mut session, &content, "floor.read_harmonics", None);
    record(
        &mut session,
        &content,
        "travel_adjacent",
        Some(("destination", "red_sluice.top")),
    );
    record(&mut session, &content, "top.check_wheels", None);
    record(&mut session, &content, "top.split_flow", None);
    record(&mut session, &content, "world.enter_aftermath", None);

    assert!(session.state().world.flags.contains("flow_split"));
    assert_eq!(
        session.trace().steps.last().unwrap().observation.text,
        "You enter Lowsail's aftermath and face what the changed water has done. The market stands above calm water while both shores still receive a share."
    );

    let encoded = session.trace().to_json().expect("trace serializes");
    let decoded = Trace::from_json(&encoded).expect("trace parses");
    let verified = verify(&decoded, &content).expect("real trace verifies");
    assert_eq!(&verified, session.state());
    assert_eq!(decoded.final_state_id, verified.state_id());
    assert_eq!(decoded.final_receipt, decoded.steps.last().unwrap().receipt);
}

#[test]
fn malformed_production_state_is_rejected_without_panicking() {
    let content = content();
    let mut malformed = content.new_game("ilyan", 71).expect("new game");
    malformed.world.locations.remove("lowsail_market");
    let error = match Session::new(malformed, &content) {
        Ok(_) => panic!("malformed state must fail"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("location"));
}
