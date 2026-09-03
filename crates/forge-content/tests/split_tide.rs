use std::collections::BTreeSet;

use forge_content::{compile, parse, parse_and_compile_production};
use forge_kernel::{
    CanonicalAction, CompiledContent, ContentContract, GameState, KnowledgeProvenanceKind,
    enumerate_legal_actions, legal_action_digest, step,
};

const SPLIT_TIDE: &str = include_str!("../../../content/split-tide.json");

fn content() -> CompiledContent {
    parse_and_compile_production(SPLIT_TIDE)
        .unwrap_or_else(|error| panic!("split-tide compile failed: {error}"))
}

fn new_game(content: &CompiledContent, preset_id: &str) -> GameState {
    content
        .new_game(preset_id, 71)
        .unwrap_or_else(|error| panic!("new game {preset_id} failed: {error}"))
}

fn definitions(state: &GameState, content: &CompiledContent) -> BTreeSet<String> {
    enumerate_legal_actions(state, content)
        .expect("valid state must enumerate")
        .into_iter()
        .map(|action| action.definition_id)
        .collect()
}

fn action_for(
    state: &GameState,
    content: &CompiledContent,
    definition_id: &str,
) -> CanonicalAction {
    enumerate_legal_actions(state, content)
        .expect("valid state must enumerate")
        .into_iter()
        .find(|action| action.definition_id == definition_id)
        .unwrap_or_else(|| {
            panic!(
                "{definition_id} is not legal at {}",
                state.world.current_location
            )
        })
}

fn apply(state: GameState, content: &CompiledContent, definition_id: &str) -> GameState {
    let action = action_for(&state, content, definition_id);
    step(&state, &action, content, &state.entropy)
        .unwrap_or_else(|error| panic!("{definition_id} failed: {error}"))
        .into_state()
}

fn travel_to(state: GameState, content: &CompiledContent, destination: &str) -> GameState {
    let action = enumerate_legal_actions(&state, content)
        .expect("valid state must enumerate")
        .into_iter()
        .find(|action| {
            action.definition_id == "travel_adjacent"
                && action
                    .parameters
                    .get("destination")
                    .is_some_and(|value| value == destination)
        })
        .unwrap_or_else(|| {
            panic!(
                "cannot travel from {} to {destination}",
                state.world.current_location
            )
        });
    let entropy = state.entropy.clone();
    step(&state, &action, content, &entropy)
        .unwrap_or_else(|error| panic!("travel to {destination} failed: {error}"))
        .into_state()
}

#[test]
fn split_tide_is_a_production_pack_with_two_full_presets() {
    let content = content();

    assert_eq!(content.contract(), ContentContract::Production);
    assert_eq!(content.start_location(), "lowsail_market");
    assert_eq!(content.world_id(), "veyra-basin");
    assert_eq!(content.locations().count(), 6);
    assert_eq!(content.npcs().count(), 5);
    assert_eq!(content.actions().count(), 47);
    assert_eq!(content.character_presets().count(), 2);

    let ilyan = content.character_preset("ilyan").expect("Ilyan preset");
    assert_eq!(ilyan.display_name, "Ilyan Vale");
    assert_eq!(ilyan.character.id, "ilyan");
    assert_eq!(ilyan.character.lineage, "fenborn");
    assert_eq!(ilyan.character.background, "ledger-clerk");
    assert_eq!(ilyan.character.aptitudes["insight"], 8);
    assert!(ilyan.character.skills.contains("read-current"));
    assert!(ilyan.character.values.contains("order"));
    assert!(ilyan.character.traits.contains("tide-ear"));
    assert!(ilyan.character.flaws.contains("indebted"));
    assert_eq!(ilyan.character.appearance["marking"], "council-ink");
    assert!(ilyan.character.knowledge.contains("forged_order_hint"));
    assert_eq!(ilyan.character.inventory["rope"], 1);
    assert_eq!(ilyan.character.resources["coin"], 10);
    assert!(ilyan.character.deeds.contains("saved_worker"));

    let rook = content.character_preset("rook").expect("Rook preset");
    assert_eq!(rook.display_name, "Rook Ash");
    assert_eq!(rook.character.id, "rook");
    assert_eq!(rook.character.lineage, "kilnborn");
    assert_eq!(rook.character.background, "lock-runner");
    assert_eq!(rook.character.aptitudes["might"], 8);
    assert!(rook.character.skills.contains("climb"));
    assert!(rook.character.values.contains("freedom"));
    assert!(rook.character.traits.contains("heat-sense"));
    assert!(rook.character.flaws.contains("wanted"));
    assert_eq!(rook.character.appearance["marking"], "kiln-scar");
    assert!(rook.character.knowledge.contains("gate_fault"));
    assert_eq!(rook.character.inventory["wire"], 1);
    assert_eq!(rook.character.resources["coin"], 5);
    assert!(rook.character.deeds.contains("stole_permit"));
}

#[test]
fn presets_change_same_scene_observation_and_legal_action_definitions() {
    let content = content();
    let ilyan = new_game(&content, "ilyan");
    let rook = new_game(&content, "rook");

    assert_eq!(ilyan.world.current_location, "lowsail_market");
    assert_eq!(rook.world.current_location, "lowsail_market");
    let ilyan_observation = content.observe(&ilyan).expect("Ilyan observation");
    let rook_observation = content.observe(&rook).expect("Rook observation");
    assert_eq!(ilyan_observation.location_id, "lowsail_market");
    assert_ne!(
        ilyan_observation.action_set_digest,
        rook_observation.action_set_digest
    );
    assert_ne!(ilyan_observation.text, rook_observation.text);
    assert!(ilyan_observation.text.contains("council mark"));
    assert!(rook_observation.text.contains("wanted runner"));

    let ilyan_definitions = definitions(&ilyan, &content);
    let rook_definitions = definitions(&rook, &content);
    assert!(ilyan_definitions.contains("checkpoint.audit_order"));
    assert!(ilyan_definitions.contains("checkpoint.show_charter"));
    assert!(ilyan_definitions.contains("checkpoint.recall_worker"));
    assert!(!ilyan_definitions.contains("checkpoint.blend_workers"));
    assert!(rook_definitions.contains("checkpoint.blend_workers"));
    assert!(rook_definitions.contains("checkpoint.pressure_guard"));
    assert!(rook_definitions.contains("checkpoint.use_stolen_permit"));
    assert!(!rook_definitions.contains("checkpoint.audit_order"));
    assert!(
        ilyan_definitions
            .symmetric_difference(&rook_definitions)
            .count()
            >= 4
    );
}

#[test]
fn result_first_and_conditional_prose_show_character_and_npc_reactions() {
    let content = content();
    let ilyan = new_game(&content, "ilyan");
    let audit = action_for(&ilyan, &content, "checkpoint.audit_order");
    let audited = step(&ilyan, &audit, &content, &ilyan.entropy).expect("audit action");
    let audit_observation = content
        .observe_after_transition(&audited)
        .expect("audit result observation");
    assert_eq!(
        audit_observation.result.as_deref(),
        Some("Your council mark makes the forgery hard to deny.")
    );
    assert!(
        audit_observation
            .text
            .starts_with("Your council mark makes the forgery hard to deny.")
    );
    assert!(audit_observation.text.split_whitespace().count() <= 100);

    let pressured = apply(
        new_game(&content, "rook"),
        &content,
        "checkpoint.pressure_guard",
    );
    assert_eq!(
        content
            .action_result(&pressured, "checkpoint.ask_sava")
            .unwrap(),
        "Sava answers with one hand near the alarm."
    );
    assert!(pressured.world.npcs["sava_rusk"].remembers("sava_was_pressured"));
}

#[test]
fn lowsail_flag_changes_sluice_legality_and_knowledge_moves_by_report() {
    let content = content();

    let baseline_floor = travel_to(
        travel_to(new_game(&content, "ilyan"), &content, "lowsail.levee"),
        &content,
        "red_sluice.floor",
    );
    assert!(!definitions(&baseline_floor, &content).contains("floor.open_relief"));

    let warned_floor = travel_to(
        travel_to(
            apply(
                travel_to(new_game(&content, "ilyan"), &content, "lowsail.docks"),
                &content,
                "docks.ring_warning",
            ),
            &content,
            "lowsail.levee",
        ),
        &content,
        "red_sluice.floor",
    );
    assert!(warned_floor.world.flags.contains("market_warned"));
    assert!(definitions(&warned_floor, &content).contains("floor.open_relief"));

    let reported = apply(
        new_game(&content, "ilyan"),
        &content,
        "checkpoint.audit_order",
    );
    assert!(reported.world.npcs["sava_rusk"].knows("forged_order"));
    assert_eq!(
        reported.world.npcs["sava_rusk"].knowledge["forged_order"]
            .provenance
            .kind(),
        KnowledgeProvenanceKind::Witnessed
    );
    assert!(!reported.world.npcs["edrik_voss"].knows("forged_order"));

    let reported = travel_to(reported, &content, "lowsail.levee");
    let reported = apply(reported, &content, "levee.send_report");
    assert!(reported.world.npcs["edrik_voss"].knows("forged_order"));
    assert_eq!(
        reported.world.npcs["edrik_voss"].knowledge["forged_order"]
            .provenance
            .kind(),
        KnowledgeProvenanceKind::Told
    );
    assert!(reported.world.npcs["edrik_voss"].remembers("edrik_received_report"));
}

#[test]
fn sluice_outcome_persists_to_the_lowsail_return_and_result_text() {
    let content = content();
    let mut state = new_game(&content, "ilyan");
    state = apply(state, &content, "checkpoint.audit_order");
    state = travel_to(state, &content, "lowsail.levee");
    state = travel_to(state, &content, "red_sluice.floor");
    state = apply(state, &content, "floor.read_harmonics");
    state = travel_to(state, &content, "red_sluice.top");
    state = apply(state, &content, "top.check_wheels");
    state = apply(state, &content, "top.split_flow");

    assert!(state.world.flags.contains("flow_split"));
    assert!(
        state.world.locations["lowsail.return"]
            .flags
            .contains("market_stable")
    );
    state = travel_to(state, &content, "lowsail.return");
    assert_eq!(
        content.location_description(&state).unwrap(),
        "The market stands above calm water while both shores still receive a share."
    );
    let return_observation = content
        .observe_action(&state, "return.read_tide")
        .expect("return observation");
    assert_eq!(
        return_observation.result.as_deref(),
        Some("Calm water leaves both shores with a fair share.")
    );
    assert!(
        return_observation
            .text
            .starts_with("Calm water leaves both shores")
    );
    assert!(definitions(&state, &content).contains("return.share_water"));
    state = apply(state, &content, "return.share_water");
    assert!(state.world.flags.contains("ending_accord"));
    assert!(state.character.deeds.contains("returned_for_accord"));
}

#[test]
fn production_text_ids_and_action_pages_are_stable() {
    let first = content();
    let second = content();
    assert_eq!(first.build_id(), second.build_id());
    assert_eq!(first.build_id().len(), 64);
    assert_eq!(forge_kernel::compute_build_id(&first), first.build_id());

    let first_state = new_game(&first, "ilyan");
    let second_state = new_game(&second, "ilyan");
    assert_eq!(first_state.state_id(), second_state.state_id());
    assert_eq!(first_state.state_id().len(), 64);

    for (_, location) in first.locations() {
        let sentences = location
            .description
            .split(['.', '!', '?'])
            .filter(|sentence| !sentence.trim().is_empty())
            .collect::<Vec<_>>();
        assert!(sentences.len() <= 2);
        assert!(
            sentences
                .iter()
                .all(|sentence| sentence.split_whitespace().count() <= 18)
        );
        assert!(location.description_variants.iter().all(|variant| {
            variant
                .text
                .split(['.', '!', '?'])
                .filter(|sentence| !sentence.trim().is_empty())
                .count()
                == 1
                && variant.text.split_whitespace().count() <= 18
        }));
    }
    for (_, action) in first.actions() {
        assert!(!action.category.trim().is_empty());
        assert!(action.category.split_whitespace().count() <= 3);
        assert!(!action.result.trim().is_empty());
        assert!(action.result.split_whitespace().count() <= 60);
        assert!(
            action
                .result
                .split(['.', '!', '?'])
                .filter(|sentence| !sentence.trim().is_empty())
                .all(|sentence| sentence.split_whitespace().count() <= 18)
        );
        assert!(action.result_variants.iter().all(|variant| {
            variant
                .text
                .split(['.', '!', '?'])
                .filter(|sentence| !sentence.trim().is_empty())
                .count()
                == 1
                && variant.text.split_whitespace().count() <= 18
        }));
    }

    let mut offset = 0;
    let mut page_ids = Vec::new();
    let mut digest = None;
    let full = enumerate_legal_actions(&first_state, &first).unwrap();
    let expected_ids: Vec<_> = full.iter().map(|action| action.action_id.clone()).collect();
    loop {
        let page = first.action_page(&first_state, offset, 3).unwrap();
        assert_eq!(page.build_id, first.build_id());
        assert_eq!(page.state_id, first_state.state_id());
        assert_eq!(page.total, full.len());
        if let Some(expected) = &digest {
            assert_eq!(expected, &page.digest);
        } else {
            digest = Some(page.digest.clone());
        }
        page_ids.extend(page.actions.iter().map(|action| action.action_id.clone()));
        match page.next_offset {
            Some(next) => offset = next,
            None => break,
        }
    }
    assert_eq!(page_ids, expected_ids);
    assert_eq!(
        digest.as_deref(),
        Some(legal_action_digest(&full).unwrap().as_str())
    );

    let mut draft = parse(SPLIT_TIDE).expect("real fixture must parse");
    draft.locations[0].description = "One two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen.".to_owned();
    let error = compile(draft).expect_err("overlong location text must fail compilation");
    assert!(error.to_string().contains("exceeds 18 words"));
}
