use std::collections::{BTreeMap, BTreeSet};

use forge_content::{compile, parse, parse_and_compile_production};
use forge_kernel::{
    CanonicalAction, CharacterChoiceSelection, CharacterSelection, CharacterStart, CompiledContent,
    ContentContract, EventKind, GameState, KnowledgeProvenanceKind, enumerate_legal_actions,
    legal_action_digest, step,
};

const SPLIT_TIDE: &str = include_str!("../../../content/split-tide.json");
const SLUICE_OUTCOMES: [&str; 5] = [
    "top.split_flow",
    "top.hold_market",
    "top.divert_relief",
    "top.break_toll",
    "top.overload",
];

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

fn creation_selection(
    content: &CompiledContent,
    name: &str,
    selected: &BTreeMap<&str, &str>,
) -> CharacterSelection {
    let creation = content.character_creation().expect("creation definition");
    CharacterSelection {
        name: name.to_owned(),
        choices: creation
            .slots
            .iter()
            .map(|slot| CharacterChoiceSelection {
                slot_id: slot.id.clone(),
                choice_id: selected
                    .get(slot.id.as_str())
                    .unwrap_or_else(|| panic!("missing test choice for {}", slot.id))
                    .to_string(),
            })
            .collect(),
    }
}

fn binary_selection(content: &CompiledContent, mask: usize) -> CharacterSelection {
    let creation = content.character_creation().expect("creation definition");
    CharacterSelection {
        name: "Mara Venn".to_owned(),
        choices: creation
            .slots
            .iter()
            .enumerate()
            .map(|(index, slot)| CharacterChoiceSelection {
                slot_id: slot.id.clone(),
                choice_id: slot.choices[(mask >> index) & 1].id.clone(),
            })
            .collect(),
    }
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
    assert_eq!(content.actions().count(), 50);
    assert_eq!(content.character_presets().count(), 2);
    let creation = content.character_creation().expect("custom creation");
    assert_eq!(creation.slots.len(), 6);
    assert!(creation.slots.iter().all(|slot| slot.choices.len() == 2));

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
fn all_sixty_four_custom_builds_are_authoritative_distinct_and_playable() {
    let content = content();
    let creation = content.character_creation().expect("creation definition");
    assert_eq!(creation.slots.len(), 6);
    let combination_count = 1usize << creation.slots.len();
    let mut character_ids = BTreeSet::new();
    let mut action_sets = Vec::new();

    for mask in 0..combination_count {
        let selection = binary_selection(&content, mask);
        let state = content
            .new_custom_game(&selection, 71)
            .unwrap_or_else(|error| panic!("custom build {mask} failed: {error}"));
        content
            .validate_state(&state)
            .expect("custom state validates");
        assert!(matches!(
            state.character_start,
            CharacterStart::Custom { .. }
        ));
        assert!(character_ids.insert(state.character.id.clone()));

        let actions = enumerate_legal_actions(&state, &content).expect("custom actions");
        let meaningful_non_movement = actions
            .iter()
            .filter(|action| {
                let definition = content.action(&action.definition_id).expect("definition");
                definition.meaningful && !definition.movement
            })
            .count();
        assert!(
            meaningful_non_movement >= 2,
            "custom build {mask} has only {meaningful_non_movement} meaningful actions"
        );
        action_sets.push(
            actions
                .into_iter()
                .map(|action| action.definition_id)
                .collect::<BTreeSet<_>>(),
        );

        let routed = travel_to(state, &content, "lowsail.docks");
        let routed = apply(routed, &content, "docks.ask_oren");
        let routed = travel_to(routed, &content, "lowsail.levee");
        assert!(
            enumerate_legal_actions(&routed, &content)
                .unwrap()
                .iter()
                .all(
                    |action| action.parameters.get("destination").map(String::as_str)
                        != Some("red_sluice.floor")
                ),
            "custom build {mask} bypassed the authored Sluice routes"
        );
        let routed = apply(routed, &content, "levee.culvert_path");
        assert_eq!(routed.world.current_location, "red_sluice.floor");
        let observation = content.observe(&routed).unwrap();
        assert!(!observation.text.contains("Rook"));
        assert!(!observation.text.contains("Ilyan"));
    }

    assert_eq!(character_ids.len(), combination_count);
    for slot_index in 0..creation.slots.len() {
        assert!(
            (0..combination_count)
                .filter(|mask| mask & (1 << slot_index) == 0)
                .any(|mask| action_sets[mask] != action_sets[mask | (1 << slot_index)]),
            "slot {} never changes the legal action definitions",
            creation.slots[slot_index].id
        );
    }
}

#[test]
fn custom_extremes_reproduce_preset_mechanics_without_copying_preset_identity() {
    let content = content();
    let ilyan_selection = creation_selection(
        &content,
        "Mara Venn",
        &BTreeMap::from([
            ("lineage", "fenborn"),
            ("origin", "lowsail"),
            ("calling", "ledger-clerk"),
            ("value", "order"),
            ("burden", "indebted"),
            ("history", "saved-worker"),
        ]),
    );
    let rook_selection = creation_selection(
        &content,
        "Mara Venn",
        &BTreeMap::from([
            ("lineage", "kilnborn"),
            ("origin", "red-sluice"),
            ("calling", "lock-runner"),
            ("value", "freedom"),
            ("burden", "wanted"),
            ("history", "stole-permit"),
        ]),
    );
    for (selection, preset_id) in [(ilyan_selection, "ilyan"), (rook_selection, "rook")] {
        let mut custom = content
            .custom_character(&selection)
            .expect("custom character");
        assert!(custom.id.starts_with("custom-"));
        custom.id = preset_id.to_owned();
        assert_eq!(
            custom,
            content.character_preset(preset_id).unwrap().character,
            "{preset_id} mechanical fields drifted from its creator extreme"
        );
    }
}

#[test]
fn custom_selection_is_canonical_and_forgery_resistant() {
    let content = content();
    let canonical = binary_selection(&content, 19);
    let mut reordered = canonical.clone();
    reordered.name = "  Mara   Venn ".to_owned();
    reordered.choices.reverse();
    assert_eq!(
        content.new_custom_game(&canonical, 71).unwrap(),
        content.new_custom_game(&reordered, 71).unwrap()
    );

    let state = content.new_custom_game(&canonical, 71).unwrap();
    let mut forged = state.clone();
    let CharacterStart::Custom { selection } = &mut forged.character_start else {
        panic!("expected custom provenance")
    };
    selection.choices[0].choice_id = "not-authored".to_owned();
    assert!(content.validate_state(&forged).is_err());

    let mut forged = state.clone();
    forged.character.lineage = "forged".to_owned();
    assert!(content.validate_state(&forged).is_err());

    let mut duplicate = canonical.clone();
    duplicate.choices[1] = duplicate.choices[0].clone();
    assert!(content.new_custom_game(&duplicate, 71).is_err());
    let mut unknown = canonical.clone();
    unknown.choices[0].choice_id = "not-authored".to_owned();
    assert!(content.new_custom_game(&unknown, 71).is_err());
    let mut unsafe_name = canonical;
    unsafe_name.name = "Mara\u{1b}[31m".to_owned();
    assert!(content.new_custom_game(&unsafe_name, 71).is_err());
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
    assert!(rook_observation.text.contains("wanted face"));
    assert!(
        ilyan_observation
            .text
            .contains("redirect the next surge at Red Sluice")
    );
    assert!(
        rook_observation
            .text
            .contains("reach Red Sluice before the next surge")
    );

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
        Some("Your council mark exposes the forged water order, and Sava accepts your proof.")
    );
    assert!(audit_observation.text.starts_with(
        "Your council mark exposes the forged water order, and Sava accepts your proof."
    ));
    assert!(audit_observation.text.split_whitespace().count() < 100);

    let pressured = apply(
        new_game(&content, "rook"),
        &content,
        "checkpoint.pressure_guard",
    );
    assert_eq!(
        content
            .action_result(&pressured, "checkpoint.ask_sava")
            .unwrap(),
        "Sava points east toward the guarded Red Sluice, one hand near the alarm."
    );
    assert!(pressured.world.npcs["sava_rusk"].remembers("sava_was_pressured"));
}

#[test]
fn dialogue_feedback_names_and_unlocks_the_routes_it_describes() {
    let content = content();

    let mut authority = new_game(&content, "ilyan");
    let sava_result = content
        .action_result(&authority, "checkpoint.ask_sava")
        .unwrap();
    assert_eq!(
        sava_result,
        "Sava points east: the levee road reaches the Red Sluice."
    );
    let opening_description = content.location_description(&authority).unwrap();
    authority = apply(authority, &content, "checkpoint.ask_sava");
    let guided_description = content.location_description(&authority).unwrap();
    assert_ne!(guided_description, opening_description);
    assert!(guided_description.contains("follow the levee road to Red Sluice"));
    authority = apply(authority, &content, "checkpoint.show_charter");
    let charter_observation = content
        .observe_action(&authority, "checkpoint.show_charter")
        .unwrap();
    assert_eq!(
        charter_observation.result.as_deref(),
        Some("Sava honors your charter, opening the Authority Path at Lowsail Levee.")
    );
    assert!(charter_observation.text.ends_with(
        "The chain stands open; Sava directs you to the Authority Path at Lowsail Levee."
    ));
    authority = travel_to(authority, &content, "lowsail.levee");
    assert!(definitions(&authority, &content).contains("levee.authority_path"));
    assert!(
        enumerate_legal_actions(&authority, &content)
            .unwrap()
            .iter()
            .all(
                |action| action.parameters.get("destination").map(String::as_str)
                    != Some("red_sluice.floor")
            )
    );
    let authority = apply(authority, &content, "levee.authority_path");
    assert_eq!(
        content
            .action_result(&authority, "floor.test_pressure")
            .unwrap(),
        "Edrik accepts your seal, then shows pressure building below the gate."
    );

    let mut culvert = travel_to(new_game(&content, "ilyan"), &content, "lowsail.docks");
    culvert = apply(culvert, &content, "docks.ask_oren");
    let oren_observation = content.observe_action(&culvert, "docks.ask_oren").unwrap();
    assert_eq!(
        oren_observation.result.as_deref(),
        Some(
            "Oren reveals the submerged Culvert Path at Lowsail Levee. Take it into the Sluice, climb to Red Sluice Top, then break the toll."
        )
    );
    assert!(culvert.world.flags.contains("culvert_revealed"));
    assert!(culvert.world.npcs["oren_pell"].remembers("oren_revealed_culvert"));
    culvert = travel_to(culvert, &content, "lowsail.levee");
    assert!(definitions(&culvert, &content).contains("levee.culvert_path"));
    culvert = apply(culvert, &content, "levee.culvert_path");
    assert_eq!(
        content
            .action_result(&culvert, "floor.test_pressure")
            .unwrap(),
        "Edrik eyes the culvert mud while the gauge shows rising pressure."
    );
}

#[test]
fn travel_presentation_keeps_canonical_identity_and_names_the_destination() {
    let content = content();
    let docks = travel_to(new_game(&content, "ilyan"), &content, "lowsail.docks");
    let page = content.action_page(&docks, 0, usize::MAX).unwrap();
    let checkpoint = page
        .actions
        .iter()
        .find(|action| {
            action.definition_id == "travel_adjacent"
                && action
                    .parameters
                    .get("destination")
                    .is_some_and(|value| value == "lowsail_market")
        })
        .expect("docks must retain canonical travel back to the checkpoint");

    assert_eq!(checkpoint.parameters["destination"], "lowsail_market");
    assert_eq!(
        checkpoint.parameter_display_values["destination"],
        "Lowsail Checkpoint"
    );
}

#[test]
fn lowsail_flag_changes_sluice_legality_and_knowledge_moves_by_report() {
    let content = content();

    let baseline = apply(
        new_game(&content, "ilyan"),
        &content,
        "checkpoint.show_charter",
    );
    let baseline = travel_to(baseline, &content, "lowsail.levee");
    let inspect = action_for(&baseline, &content, "levee.inspect_damage");
    let transition = step(&baseline, &inspect, &content, &baseline.entropy).unwrap();
    let damage_observation = content.observe_after_transition(&transition).unwrap();
    assert_eq!(damage_observation.text.matches("Fresh marks").count(), 1);
    assert!(
        damage_observation
            .text
            .ends_with("Workers brace the wet embankment.")
    );
    let baseline = transition.into_state();
    let baseline_floor = apply(baseline, &content, "levee.authority_path");
    assert!(!definitions(&baseline_floor, &content).contains("floor.open_relief"));

    let warned = apply(
        new_game(&content, "ilyan"),
        &content,
        "checkpoint.show_charter",
    );
    let warned = travel_to(warned, &content, "lowsail.docks");
    let warning = action_for(&warned, &content, "docks.ring_warning");
    let transition = step(&warned, &warning, &content, &warned.entropy).unwrap();
    let warning_observation = content.observe_after_transition(&transition).unwrap();
    assert_eq!(
        warning_observation.result.as_deref(),
        Some(
            "You sound the market warning. Oren orders every crew uphill; relay his warning at Lowsail Levee so Edrik can open relief."
        )
    );
    assert!(
        warning_observation
            .text
            .ends_with("The docks stand empty beneath the warning bell.")
    );
    let warned = transition.into_state();
    let warned = travel_to(warned, &content, "lowsail.levee");
    let warned_floor = apply(warned.clone(), &content, "levee.authority_path");
    assert!(warned_floor.world.flags.contains("market_warned"));
    assert!(!warned_floor.world.npcs["edrik_voss"].knows("market_warned"));
    assert!(
        !content
            .observe(&warned_floor)
            .unwrap()
            .text
            .contains("Edrik knows Lowsail has been warned")
    );
    assert!(!definitions(&warned_floor, &content).contains("floor.open_relief"));

    assert!(warned.world.npcs["oren_pell"].knows("market_warned"));
    assert_eq!(
        warned.world.npcs["oren_pell"].knowledge["market_warned"]
            .provenance
            .kind(),
        KnowledgeProvenanceKind::Witnessed
    );
    let relayed = apply(warned, &content, "levee.relay_warning");
    let relay_turn = relayed.world.npcs["edrik_voss"].knowledge["market_warned"].turn;
    assert_eq!(
        relayed.world.npcs["edrik_voss"].knowledge["market_warned"].provenance,
        forge_kernel::KnowledgeProvenance::Rumor {
            from: Some("oren_pell".to_owned())
        }
    );
    let relayed_floor = apply(relayed, &content, "levee.authority_path");
    assert!(
        content
            .observe(&relayed_floor)
            .unwrap()
            .text
            .contains("Edrik knows Lowsail has been warned")
    );
    assert!(definitions(&relayed_floor, &content).contains("floor.open_relief"));
    let relief_action = action_for(&relayed_floor, &content, "floor.open_relief");
    let relief = step(
        &relayed_floor,
        &relief_action,
        &content,
        &relayed_floor.entropy,
    )
    .unwrap();
    assert_eq!(
        content
            .observe_after_transition(&relief)
            .unwrap()
            .result
            .as_deref(),
        Some("Edrik follows the warning and opens the safer channel.")
    );
    let returned_levee = travel_to(relief.into_state(), &content, "lowsail.levee");
    assert!(!definitions(&returned_levee, &content).contains("levee.relay_warning"));
    let revisited = apply(returned_levee, &content, "levee.authority_path");
    assert_eq!(
        revisited.world.npcs["edrik_voss"].knowledge["market_warned"].turn,
        relay_turn
    );
    assert_eq!(
        revisited.world.npcs["edrik_voss"].knowledge["relief_plan"]
            .provenance
            .kind(),
        KnowledgeProvenanceKind::Witnessed
    );

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
fn rescued_worker_report_introduces_mira_and_keeps_its_source() {
    let content = content();
    let state = travel_to(new_game(&content, "rook"), &content, "lowsail.levee");
    assert!(!state.world.npcs["mira_kett"].remembers("levee_worker_helped"));
    let action = action_for(&state, &content, "levee.help_worker");
    let transition = step(&state, &action, &content, &state.entropy).unwrap();
    assert_eq!(
        content
            .observe_after_transition(&transition)
            .unwrap()
            .result
            .as_deref(),
        Some("You pull the worker clear. Their report reaches Mira, Red Sluice's crew leader.")
    );
    let helped = transition.into_state();
    let report = helped.world.npcs["mira_kett"].memories["levee_worker_helped"].clone();
    assert_eq!(report.turn, 1);
    assert_eq!(
        report.provenance,
        forge_kernel::KnowledgeProvenance::Read {
            source: "levee_worker_report".to_owned()
        }
    );
    assert_eq!(helped.world.npcs["mira_kett"].location, "red_sluice.top");
    assert_eq!(helped.world.npcs["mira_kett"].relationships["player"], 1);
    assert!(helped.character.deeds.contains("helped_worker"));
    let away = travel_to(helped, &content, "lowsail_market");
    let returned = travel_to(away, &content, "lowsail.levee");
    assert_eq!(
        returned.world.npcs["mira_kett"].memories["levee_worker_helped"],
        report
    );
    assert!(!definitions(&returned, &content).contains("levee.help_worker"));
}

#[test]
fn tide_key_moves_from_yara_and_opens_a_persistent_calibration_route() {
    let content = content();
    let initial = new_game(&content, "rook");
    assert_eq!(
        initial.world.npcs["yara_dene"].inventory["split_tide.tide_key"],
        1
    );
    assert!(
        !initial
            .character
            .inventory
            .contains_key("split_tide.tide_key")
    );

    let mut without_key = travel_to(initial.clone(), &content, "lowsail.docks");
    without_key = apply(without_key, &content, "docks.ask_oren");
    without_key = travel_to(without_key, &content, "lowsail.levee");
    without_key = apply(without_key, &content, "levee.culvert_path");
    assert!(!definitions(&without_key, &content).contains("floor.key_calibration"));

    let docks = travel_to(initial, &content, "lowsail.docks");
    let take_key = action_for(&docks, &content, "docks.press_yara");
    let transfer = step(&docks, &take_key, &content, &docks.entropy).unwrap();
    assert!(transfer.events().iter().any(|event| matches!(
        &event.kind,
        EventKind::NpcItemTransferredToCharacter { npc, item, count: 1 }
            if npc == "yara_dene" && item == "split_tide.tide_key"
    )));
    let key_observation = content.observe_after_transition(&transfer).unwrap();
    assert!(
        key_observation
            .text
            .contains("Use Calibrate Gate at Red Sluice Floor")
    );
    assert!(key_observation.text.contains("You carry the Tide Key"));
    assert!(!key_observation.text.contains("Yara keeps the Tide Key"));
    let mut carrier = transfer.into_state();
    assert_eq!(carrier.character.inventory["split_tide.tide_key"], 1);
    assert!(
        !carrier.world.npcs["yara_dene"]
            .inventory
            .contains_key("split_tide.tide_key")
    );
    assert!(!definitions(&carrier, &content).contains("docks.press_yara"));
    carrier = apply(carrier, &content, "docks.ask_oren");
    carrier = travel_to(carrier, &content, "lowsail.levee");
    carrier = apply(carrier, &content, "levee.culvert_path");
    assert!(definitions(&carrier, &content).contains("floor.key_calibration"));
    carrier = apply(carrier, &content, "floor.key_calibration");
    assert!(carrier.world.flags.contains("sluice_calibrated"));
    assert!(carrier.character.deeds.contains("calibrated_with_tide_key"));
    assert!(carrier.world.npcs["edrik_voss"].remembers("edrik_saw_key_calibration"));
    assert!(!definitions(&carrier, &content).contains("floor.key_calibration"));
    carrier = travel_to(carrier, &content, "red_sluice.top");
    carrier = apply(carrier, &content, "top.check_wheels");
    assert!(definitions(&carrier, &content).contains("top.split_flow"));
    carrier = apply(carrier, &content, "top.split_flow");
    carrier = apply(carrier, &content, "world.enter_aftermath");
    assert_eq!(carrier.character.inventory["split_tide.tide_key"], 1);
    assert!(
        !carrier.world.npcs["yara_dene"]
            .inventory
            .contains_key("split_tide.tide_key")
    );
    assert!(carrier.world.flags.contains("flow_split"));
    assert!(
        content
            .observe(&carrier)
            .unwrap()
            .text
            .contains("both shores still receive a share")
    );
}

#[test]
fn sluice_outcome_persists_to_the_lowsail_return_and_result_text() {
    let content = content();
    let mut state = new_game(&content, "ilyan");
    state = apply(state, &content, "checkpoint.audit_order");
    state = apply(state, &content, "checkpoint.show_charter");
    state = travel_to(state, &content, "lowsail.levee");
    state = apply(state, &content, "levee.authority_path");
    state = apply(state, &content, "floor.read_harmonics");
    assert!(
        content
            .action_result(&state, "floor.read_harmonics")
            .unwrap()
            .contains("Check the wheels at Red Sluice Top, then choose Split Flow")
    );
    state = travel_to(state, &content, "red_sluice.top");
    state = apply(state, &content, "top.check_wheels");
    let available_outcomes = definitions(&state, &content);
    assert!(available_outcomes.contains("top.split_flow"));
    assert!(available_outcomes.contains("top.hold_market"));
    let outcome_page = content.action_page(&state, 0, usize::MAX).unwrap();
    let split_view = outcome_page
        .actions
        .iter()
        .find(|action| action.definition_id == "top.split_flow")
        .unwrap();
    assert_eq!(
        split_view.consequence_preview.as_deref(),
        Some("The gates share water between both shores.")
    );
    let hold_view = outcome_page
        .actions
        .iter()
        .find(|action| action.definition_id == "top.hold_market")
        .unwrap();
    assert_eq!(
        hold_view.consequence_preview.as_deref(),
        Some("The market stays dry while the upland works lose water.")
    );
    assert!(
        outcome_page
            .actions
            .iter()
            .find(|action| action.definition_id == "top.rescue_worker")
            .unwrap()
            .consequence_preview
            .is_none()
    );
    state = apply(state, &content, "top.split_flow");

    assert!(state.world.flags.contains("sluice_outcome_chosen"));
    assert!(state.world.flags.contains("flow_split"));
    for outcome in [
        "top.split_flow",
        "top.hold_market",
        "top.divert_relief",
        "top.break_toll",
        "top.overload",
    ] {
        assert!(
            !definitions(&state, &content).contains(outcome),
            "contradictory outcome remained legal: {outcome}"
        );
    }
    assert!(
        state.world.locations["lowsail.return"]
            .flags
            .contains("market_stable")
    );
    assert!(definitions(&state, &content).contains("world.enter_aftermath"));
    state = apply(state, &content, "world.enter_aftermath");
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
    let ending_page = content.action_page(&state, 0, usize::MAX).unwrap();
    let accord_view = ending_page
        .actions
        .iter()
        .find(|action| action.definition_id == "return.share_water")
        .unwrap();
    assert_eq!(accord_view.label, "Seal Water Accord");
    assert_eq!(
        accord_view.consequence_preview.as_deref(),
        Some("You seal an accord that keeps both shores supplied.")
    );
    state = apply(state, &content, "return.share_water");
    assert!(state.world.flags.contains("ending_accord"));
    assert!(state.character.deeds.contains("returned_for_accord"));
}

#[test]
fn resolved_phase_closes_old_actions_and_hold_aftermath_stays_truthful() {
    let content = content();
    let mut state = apply(
        new_game(&content, "ilyan"),
        &content,
        "checkpoint.show_charter",
    );
    state = travel_to(state, &content, "lowsail.levee");
    state = apply(state, &content, "levee.authority_path");
    state = travel_to(state, &content, "red_sluice.top");
    state = apply(state, &content, "top.hold_market");

    let assert_resolved_catalog = |state: &GameState| {
        let legal = definitions(state, &content);
        assert!(legal.contains("world.enter_aftermath"));
        assert!(
            content
                .location_description(state)
                .unwrap()
                .contains("Enter Lowsail Aftermath"),
            "resolved scene still gives expired directions at {}",
            state.world.current_location
        );
        for definition in legal {
            assert!(
                matches!(
                    definition.as_str(),
                    "travel_adjacent" | "wait_tide" | "world.enter_aftermath"
                ),
                "pre-surge action remained legal after resolution: {definition}"
            );
        }
    };

    assert_resolved_catalog(&state);
    state = travel_to(state, &content, "red_sluice.floor");
    assert_resolved_catalog(&state);
    state = travel_to(state, &content, "lowsail.levee");
    assert_resolved_catalog(&state);
    state = travel_to(state, &content, "lowsail_market");
    assert_resolved_catalog(&state);
    state = travel_to(state, &content, "lowsail.docks");
    assert_resolved_catalog(&state);
    assert!(!definitions(&state, &content).contains("docks.ring_warning"));

    state = apply(state, &content, "world.enter_aftermath");
    assert_eq!(state.world.current_location, "lowsail.return");
    assert_eq!(
        content.location_description(&state).unwrap(),
        "Lowsail stays dry while the upland works crack above the lowered water."
    );
    assert_eq!(
        content.action_result(&state, "return.read_tide").unwrap(),
        "Low water keeps Lowsail dry while the upland works lose their supply."
    );
    let page = content.action_page(&state, 0, usize::MAX).unwrap();
    let council_view = page
        .actions
        .iter()
        .find(|action| action.definition_id == "return.count_dry_stalls")
        .unwrap();
    assert_eq!(council_view.label, "Enforce Council Claim");
    assert_eq!(
        council_view.consequence_preview.as_deref(),
        Some("You enforce council control while the upland works absorb the loss.")
    );
}

#[test]
fn every_sluice_outcome_excludes_the_other_four() {
    let content = content();

    let mut split = apply(
        new_game(&content, "ilyan"),
        &content,
        "checkpoint.show_charter",
    );
    split = travel_to(split, &content, "lowsail.levee");
    split = apply(split, &content, "levee.authority_path");
    split = apply(split, &content, "floor.read_harmonics");
    split = travel_to(split, &content, "red_sluice.top");
    split = apply(split, &content, "top.check_wheels");

    let mut hold = apply(
        new_game(&content, "ilyan"),
        &content,
        "checkpoint.show_charter",
    );
    hold = travel_to(hold, &content, "lowsail.levee");
    hold = apply(hold, &content, "levee.authority_path");
    hold = travel_to(hold, &content, "red_sluice.top");

    let mut relief = travel_to(new_game(&content, "ilyan"), &content, "lowsail.docks");
    relief = apply(relief, &content, "docks.ring_warning");
    relief = apply(relief, &content, "docks.ask_oren");
    relief = travel_to(relief, &content, "lowsail.levee");
    relief = apply(relief, &content, "levee.relay_warning");
    relief = apply(relief, &content, "levee.culvert_path");
    relief = apply(relief, &content, "floor.open_relief");
    relief = travel_to(relief, &content, "red_sluice.top");

    let mut freedom = apply(
        new_game(&content, "rook"),
        &content,
        "checkpoint.blend_workers",
    );
    freedom = travel_to(freedom, &content, "lowsail.levee");
    freedom = apply(freedom, &content, "levee.culvert_path");
    freedom = travel_to(freedom, &content, "red_sluice.top");

    let mut disaster = apply(
        new_game(&content, "rook"),
        &content,
        "checkpoint.use_stolen_permit",
    );
    disaster = travel_to(disaster, &content, "lowsail.levee");
    assert!(definitions(&disaster, &content).contains("levee.stolen_path"));
    disaster = apply(disaster, &content, "levee.stolen_path");
    assert_eq!(
        content
            .action_result(&disaster, "floor.test_pressure")
            .unwrap(),
        "Edrik doubts your permit while the gauge shows rising pressure."
    );
    disaster = apply(disaster, &content, "floor.climb_hot_face");
    disaster = travel_to(disaster, &content, "red_sluice.top");

    for (state, selected) in [
        (split, "top.split_flow"),
        (hold, "top.hold_market"),
        (relief, "top.divert_relief"),
        (freedom, "top.break_toll"),
        (disaster, "top.overload"),
    ] {
        assert!(definitions(&state, &content).contains(selected));
        assert!(!definitions(&state, &content).contains("world.enter_aftermath"));
        assert!(
            enumerate_legal_actions(&state, &content)
                .unwrap()
                .iter()
                .all(
                    |action| action.parameters.get("destination").map(String::as_str)
                        != Some("lowsail.return")
                )
        );
        let resolved = apply(state, &content, selected);
        assert!(resolved.world.flags.contains("sluice_outcome_chosen"));
        let legal_after = definitions(&resolved, &content);
        assert!(legal_after.contains("world.enter_aftermath"));
        assert!(
            SLUICE_OUTCOMES
                .iter()
                .all(|outcome| !legal_after.contains(*outcome)),
            "an outcome remained legal after {selected}"
        );
    }
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

#[test]
fn unchanged_actions_keep_their_order_while_state_bound_ids_rotate() {
    let content = content();
    let initial = new_game(&content, "ilyan");
    let before = enumerate_legal_actions(&initial, &content).unwrap();
    let after_state = apply(initial, &content, "wait_tide");
    let after = enumerate_legal_actions(&after_state, &content).unwrap();

    let semantic_shapes = |actions: &[CanonicalAction]| {
        actions
            .iter()
            .map(|action| (action.definition_id.clone(), action.parameters.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(semantic_shapes(&before), semantic_shapes(&after));
    assert!(
        before
            .iter()
            .zip(&after)
            .all(|(left, right)| left.action_id != right.action_id)
    );
    assert_eq!(
        content
            .action_result(&after_state, "checkpoint.read_flag")
            .unwrap(),
        "Go to the Red Sluice and redirect the next surge before it floods Lowsail."
    );
}

#[test]
fn unresolved_surge_fires_at_sixteen_and_a_prior_outcome_prevents_it() {
    let content = content();
    let mut state = new_game(&content, "ilyan");
    let opening = content.observe(&state).unwrap();
    assert_eq!(opening.world_time, 0);
    assert_eq!(opening.upcoming_events.len(), 1);
    assert_eq!(opening.upcoming_events[0].label, "Lowsail surge");
    assert_eq!(opening.upcoming_events[0].remaining_ticks, 16);
    let mut missing_schedule = state.clone();
    missing_schedule.world.scheduled_events.clear();
    assert!(enumerate_legal_actions(&missing_schedule, &content).is_err());

    state = apply(state, &content, "checkpoint.show_charter");
    state = travel_to(state, &content, "lowsail.docks");
    state = apply(state, &content, "docks.ring_warning");
    state = travel_to(state, &content, "lowsail.levee");
    state = apply(state, &content, "levee.authority_path");
    state = travel_to(state, &content, "red_sluice.top");
    while state.world.time < 15 {
        state = apply(state, &content, "wait_tide");
    }
    assert!(definitions(&state, &content).contains("top.hold_market"));
    let almost_due = content.observe(&state).unwrap();
    assert_eq!(almost_due.upcoming_events[0].remaining_ticks, 1);

    let action = action_for(&state, &content, "wait_tide");
    let transition = step(&state, &action, &content, &state.entropy).unwrap();
    let observation = content.observe_after_transition(&transition).unwrap();
    assert_eq!(observation.world_time, 16);
    assert!(observation.upcoming_events.is_empty());
    assert!(observation.result.as_deref().is_some_and(|result| {
        result.contains("The surge hits before you redirect it. Lowsail floods.")
    }));
    assert!(transition.events().iter().any(|event| matches!(
        &event.kind,
        EventKind::ScheduledEventResolved {
            event_id,
            event_kind,
            applied: true,
        } if event_id == "lowsail.next_surge" && event_kind == "deadline"
    )));
    let missed = transition.into_state();
    assert!(missed.world.flags.contains("surge_missed"));
    assert!(missed.world.flags.contains("sluice_failure"));
    assert!(missed.world.flags.contains("sluice_outcome_chosen"));
    assert!(missed.world.scheduled_events.is_empty());
    assert!(
        missed.world.locations["lowsail.return"]
            .flags
            .contains("market_flooded")
    );
    let after_deadline = definitions(&missed, &content);
    assert!(after_deadline.contains("world.enter_aftermath"));
    assert!(!after_deadline.contains("top.check_wheels"));
    assert!(!after_deadline.contains("top.signal_market"));
    assert!(
        SLUICE_OUTCOMES
            .iter()
            .all(|outcome| !after_deadline.contains(*outcome))
    );
    assert!(
        content
            .location_description(&missed)
            .unwrap()
            .contains("Enter Lowsail Aftermath")
    );

    let missed_floor = travel_to(missed, &content, "red_sluice.floor");
    let floor_after_deadline = definitions(&missed_floor, &content);
    for closed in [
        "floor.test_pressure",
        "floor.stabilize_gauge",
        "floor.read_harmonics",
        "floor.dive_intake",
        "floor.climb_hot_face",
        "floor.open_relief",
        "floor.force_wheel",
        "floor.ack_report",
    ] {
        assert!(
            !floor_after_deadline.contains(closed),
            "deadline preparation remained legal: {closed}"
        );
    }
    assert!(
        content
            .location_description(&missed_floor)
            .unwrap()
            .contains("Enter Lowsail Aftermath")
    );
    let mut missed_roads = missed_floor.clone();
    for destination in ["lowsail.levee", "lowsail_market", "lowsail.docks"] {
        missed_roads = travel_to(missed_roads, &content, destination);
        let description = content.location_description(&missed_roads).unwrap();
        assert!(description.contains("Floodwater"));
        assert!(description.contains("Enter Lowsail Aftermath"));
        assert!(
            content
                .observe(&missed_roads)
                .unwrap()
                .upcoming_events
                .is_empty()
        );
        let catalog = definitions(&missed_roads, &content);
        assert!(catalog.contains("world.enter_aftermath"));
        assert!(catalog.iter().all(|definition| matches!(
            definition.as_str(),
            "travel_adjacent" | "wait_tide" | "world.enter_aftermath"
        )));
    }
    let missed_return = apply(missed_floor, &content, "world.enter_aftermath");
    assert_eq!(missed_return.world.current_location, "lowsail.return");
    assert_eq!(
        content.location_description(&missed_return).unwrap(),
        "Water fills the lower stalls while the market pays for the broken gates."
    );
    let ending_page = content.action_page(&missed_return, 0, usize::MAX).unwrap();
    let flood_view = ending_page
        .actions
        .iter()
        .find(|action| action.definition_id == "return.face_flood")
        .unwrap();
    assert_eq!(flood_view.label, "Answer for Flood");
    assert_eq!(
        flood_view.consequence_preview.as_deref(),
        Some("You face the flooded market and answer for the broken gates.")
    );
    let ended = apply(missed_return, &content, "return.face_flood");
    assert!(ended.world.flags.contains("ending_disaster"));
    assert!(ended.character.deeds.contains("faced_flood"));

    let mut protected = new_game(&content, "ilyan");
    protected = apply(protected, &content, "checkpoint.show_charter");
    protected = travel_to(protected, &content, "lowsail.levee");
    protected = apply(protected, &content, "levee.authority_path");
    protected = travel_to(protected, &content, "red_sluice.top");
    protected = apply(protected, &content, "top.hold_market");
    assert!(
        content
            .observe(&protected)
            .unwrap()
            .upcoming_events
            .is_empty()
    );
    while protected.world.time < 16 {
        protected = apply(protected, &content, "wait_tide");
    }
    assert!(!protected.world.flags.contains("surge_missed"));
    assert!(protected.event_log.iter().any(|event| matches!(
        &event.kind,
        EventKind::ScheduledEventResolved {
            event_id,
            applied: false,
            ..
        } if event_id == "lowsail.next_surge"
    )));
}

#[test]
fn discoveries_retire_and_the_intake_map_unlocks_a_safe_split() {
    let content = content();
    let mut state = new_game(&content, "ilyan");
    assert_eq!(
        content.action("checkpoint.recall_worker").unwrap().label,
        "Recall Earlier Rescue"
    );
    assert_eq!(
        content
            .action_result(&state, "checkpoint.recall_worker")
            .unwrap(),
        "Sava recalls your earlier rescue at the levee."
    );
    state = apply(state, &content, "checkpoint.ask_sava");
    assert!(!definitions(&state, &content).contains("checkpoint.ask_sava"));

    state = apply(state, &content, "checkpoint.audit_order");
    assert_eq!(
        content
            .action_result(&state, "checkpoint.audit_order")
            .unwrap(),
        "Your council mark exposes the forged water order, and Sava accepts your proof."
    );
    assert!(!definitions(&state, &content).contains("checkpoint.audit_order"));

    state = apply(state, &content, "checkpoint.show_charter");
    state = travel_to(state, &content, "lowsail.levee");
    state = apply(state, &content, "levee.authority_path");
    let dive = action_for(&state, &content, "floor.dive_intake");
    let transition = step(&state, &dive, &content, &state.entropy).unwrap();
    let observation = content.observe_after_transition(&transition).unwrap();
    assert_eq!(
        observation.result.as_deref(),
        Some("The intake map reveals a safe split. Check the wheels at Red Sluice Top.")
    );
    state = transition.into_state();
    assert!(!definitions(&state, &content).contains("floor.dive_intake"));
    state = travel_to(state, &content, "red_sluice.top");
    state = apply(state, &content, "top.check_wheels");
    assert!(definitions(&state, &content).contains("top.split_flow"));
}
