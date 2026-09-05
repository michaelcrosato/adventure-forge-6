use forge_content::parse_and_compile_production;
use forge_kernel::{KernelError, enumerate_legal_actions};
use forge_replay::{ReplayError, Session};

const SPLIT_TIDE: &str = include_str!("../../../content/split-tide.json");

fn production_content() -> forge_kernel::CompiledContent {
    parse_and_compile_production(SPLIT_TIDE).expect("Split Tide production content must compile")
}

#[test]
fn stale_action_rejection_preserves_session() {
    let content = production_content();
    let mut session = Session::new_game("rook", 71, &content).expect("Rook session starts");
    let wait = enumerate_legal_actions(session.state(), &content)
        .expect("initial actions enumerate")
        .into_iter()
        .find(|action| action.definition_id == "wait_tide" && action.parameters.is_empty())
        .expect("initial wait action is legal");

    session.record(&wait).expect("first wait is accepted");
    let state_before_rejection = session.state().clone();
    let trace_before_rejection = session
        .trace()
        .to_json()
        .expect("detailed trace serializes");
    let player_save_before_rejection = session
        .player_trace()
        .expect("authored session has a player save")
        .to_json()
        .expect("player save serializes");

    let error = session
        .record(&wait)
        .expect_err("stale action bypassed state binding: old wait was accepted");
    assert!(
        matches!(error, ReplayError::Kernel(KernelError::StaleAction { .. })),
        "stale action bypassed state binding: {error:?}"
    );
    assert_eq!(session.state(), &state_before_rejection);
    assert_eq!(
        session
            .trace()
            .to_json()
            .expect("detailed trace serializes after rejection"),
        trace_before_rejection
    );
    assert_eq!(
        session
            .player_trace()
            .expect("authored session still has a player save")
            .to_json()
            .expect("player save serializes after rejection"),
        player_save_before_rejection
    );

    let fresh_wait = enumerate_legal_actions(session.state(), &content)
        .expect("fresh actions enumerate")
        .into_iter()
        .find(|action| action.definition_id == "wait_tide" && action.parameters.is_empty())
        .expect("fresh wait action is legal");
    assert_ne!(fresh_wait.action_id, wait.action_id);
    session
        .record(&fresh_wait)
        .expect("fresh state-bound wait is accepted");
    assert_eq!(session.state().world.time, 2);
    assert_eq!(session.trace().steps.len(), 2);
}

#[test]
fn public_observation_excludes_npc_stock() {
    let content = production_content();
    let state = content.new_game("rook", 71).expect("Rook state starts");
    assert_eq!(
        state.world.npcs["yara_dene"]
            .inventory
            .get("split_tide.tide_key"),
        Some(&1)
    );

    let before = state.clone();
    let observation = content
        .observe(&state)
        .expect("initial observation renders");
    let expected_resources: Vec<_> = state
        .character
        .resources
        .iter()
        .map(|(id, amount)| {
            (
                id.clone(),
                content
                    .supply_labels()
                    .resources
                    .get(id)
                    .map(String::as_str)
                    .unwrap_or(id)
                    .to_owned(),
                *amount,
            )
        })
        .collect();
    let actual_resources: Vec<_> = observation
        .supplies
        .resources
        .iter()
        .map(|resource| (resource.id.clone(), resource.name.clone(), resource.amount))
        .collect();
    assert_eq!(actual_resources, expected_resources);

    let expected_items: Vec<_> = state
        .character
        .inventory
        .iter()
        .map(|(id, count)| {
            (
                id.clone(),
                content
                    .supply_labels()
                    .items
                    .get(id)
                    .map(String::as_str)
                    .unwrap_or(id)
                    .to_owned(),
                *count,
            )
        })
        .collect();
    let actual_items: Vec<_> = observation
        .supplies
        .items
        .iter()
        .map(|item| (item.id.clone(), item.name.clone(), item.count))
        .collect();
    assert_eq!(
        actual_items, expected_items,
        "public observation leaked NPC stock"
    );
    assert!(
        observation
            .supplies
            .items
            .iter()
            .all(|item| item.id != "split_tide.tide_key"),
        "public observation leaked NPC stock"
    );

    let repeated = content
        .observe(&state)
        .expect("repeated observation renders");
    assert_eq!(repeated, observation);
    assert_eq!(state, before);
}
