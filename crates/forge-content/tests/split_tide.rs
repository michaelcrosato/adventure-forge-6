use std::collections::{BTreeMap, BTreeSet};

use forge_content::{compile, parse, parse_and_compile};
use forge_kernel::{
    Character, CompiledContent, EntropyState, GameState, KnowledgeProvenanceKind, NpcState,
    WorldState, enumerate_legal_actions, step,
};

const SPLIT_TIDE: &str = include_str!("../../../content/split-tide.json");

fn content() -> CompiledContent {
    parse_and_compile(SPLIT_TIDE)
        .unwrap_or_else(|error| panic!("split-tide compile failed: {error}"))
}

fn npc(id: &str, location: &str) -> NpcState {
    NpcState {
        id: id.to_owned(),
        location: location.to_owned(),
        goals: BTreeSet::new(),
        values: BTreeSet::new(),
        tags: BTreeSet::new(),
        relationships: BTreeMap::from([("player".to_owned(), 0)]),
        memories: BTreeMap::new(),
        knowledge: BTreeMap::new(),
        inventory: BTreeMap::new(),
        suspicion: 0,
    }
}

fn state(content: &CompiledContent, character: Character) -> GameState {
    let npcs = BTreeMap::from([
        (
            "sava_rusk".to_owned(),
            npc("sava_rusk", "lowsail.checkpoint"),
        ),
        ("oren_pell".to_owned(), npc("oren_pell", "lowsail.docks")),
        ("yara_dene".to_owned(), npc("yara_dene", "lowsail.docks")),
        (
            "edrik_voss".to_owned(),
            npc("edrik_voss", "red_sluice.floor"),
        ),
        ("mira_kett".to_owned(), npc("mira_kett", "red_sluice.top")),
    ]);
    let mut locations = content.empty_location_runtime();
    for (id, inhabitant) in &npcs {
        locations
            .get_mut(&inhabitant.location)
            .expect("NPC location must be in content")
            .entities
            .insert(id.clone());
    }
    let world = WorldState::new(content.world_id(), "lowsail.checkpoint", locations, npcs);
    let state = GameState::new(
        content.build_id().to_owned(),
        world,
        character,
        EntropyState::new(71),
    );
    content
        .validate_state(&state)
        .expect("fixture state must satisfy the public content contract");
    state
}

fn ilyan() -> Character {
    Character {
        id: "ilyan".to_owned(),
        lineage: "fenborn".to_owned(),
        origin: "lowsail".to_owned(),
        background: "ledger-clerk".to_owned(),
        aptitudes: BTreeMap::from([
            ("might".to_owned(), 3),
            ("finesse".to_owned(), 4),
            ("insight".to_owned(), 8),
            ("presence".to_owned(), 7),
        ]),
        skills: BTreeSet::from(["audit".to_owned()]),
        values: BTreeSet::from(["order".to_owned()]),
        traits: BTreeSet::from(["tide-ear".to_owned()]),
        flaws: BTreeSet::from(["indebted".to_owned()]),
        appearance: BTreeMap::from([("marking".to_owned(), "council-ink".to_owned())]),
        affiliations: BTreeMap::from([("council".to_owned(), 3)]),
        reputation: BTreeMap::from([("lawful".to_owned(), 3)]),
        knowledge: BTreeSet::new(),
        inventory: BTreeMap::from([("rope".to_owned(), 1)]),
        resources: BTreeMap::from([("coin".to_owned(), 10)]),
        injuries: BTreeSet::new(),
        deeds: BTreeSet::from(["saved_worker".to_owned()]),
        promises: BTreeSet::new(),
        discoveries: BTreeSet::new(),
        facets: BTreeMap::new(),
    }
}

fn rook() -> Character {
    Character {
        id: "rook".to_owned(),
        lineage: "kilnborn".to_owned(),
        origin: "red-sluice".to_owned(),
        background: "lock-runner".to_owned(),
        aptitudes: BTreeMap::from([
            ("might".to_owned(), 8),
            ("finesse".to_owned(), 8),
            ("insight".to_owned(), 2),
            ("presence".to_owned(), 3),
        ]),
        skills: BTreeSet::from(["climb".to_owned(), "pick".to_owned()]),
        values: BTreeSet::from(["freedom".to_owned()]),
        traits: BTreeSet::from(["heat-sense".to_owned()]),
        flaws: BTreeSet::from(["wanted".to_owned()]),
        appearance: BTreeMap::from([("marking".to_owned(), "kiln-scar".to_owned())]),
        affiliations: BTreeMap::from([("workers".to_owned(), 2)]),
        reputation: BTreeMap::from([("notoriety".to_owned(), 3)]),
        knowledge: BTreeSet::new(),
        inventory: BTreeMap::from([("rope".to_owned(), 1), ("wire".to_owned(), 1)]),
        resources: BTreeMap::from([("coin".to_owned(), 5)]),
        injuries: BTreeSet::new(),
        deeds: BTreeSet::from(["stole_permit".to_owned()]),
        promises: BTreeSet::new(),
        discoveries: BTreeSet::new(),
        facets: BTreeMap::new(),
    }
}

fn definitions(state: &GameState, content: &CompiledContent) -> BTreeSet<String> {
    enumerate_legal_actions(state, content)
        .expect("valid state must enumerate")
        .into_iter()
        .map(|action| action.definition_id)
        .collect()
}

fn apply(state: GameState, content: &CompiledContent, definition_id: &str) -> GameState {
    let action = enumerate_legal_actions(&state, content)
        .expect("valid state must enumerate")
        .into_iter()
        .find(|action| action.definition_id == definition_id)
        .unwrap_or_else(|| {
            panic!(
                "{definition_id} is not legal at {}",
                state.world.current_location
            )
        });
    step(&state, &action, content, &state.entropy)
        .unwrap_or_else(|error| panic!("{definition_id} failed: {error}"))
        .state
}

fn travel_to(mut state: GameState, content: &CompiledContent, destination: &str) -> GameState {
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
    state = step(&state, &action, content, &entropy)
        .unwrap_or_else(|error| panic!("travel to {destination} failed: {error}"))
        .state;
    state
}

#[test]
fn split_tide_loads_as_one_connected_world_with_five_npcs() {
    let content = content();
    assert_eq!(content.world_id(), "veyra-basin");
    assert_eq!(content.locations().count(), 6);
    assert_eq!(content.npcs().count(), 5);
    for location in [
        "lowsail.checkpoint",
        "lowsail.docks",
        "lowsail.levee",
        "red_sluice.floor",
        "red_sluice.top",
        "lowsail.return",
    ] {
        assert!(content.location(location).is_some(), "missing {location}");
    }
    for npc_id in [
        "sava_rusk",
        "oren_pell",
        "yara_dene",
        "edrik_voss",
        "mira_kett",
    ] {
        assert!(content.npc(npc_id).is_some(), "missing {npc_id}");
    }
    assert!(content.actions().count() >= 35);
}

#[test]
fn ilyan_and_rook_have_materially_different_checkpoint_definitions() {
    let content = content();
    let ilyan_definitions = definitions(&state(&content, ilyan()), &content);
    let rook_definitions = definitions(&state(&content, rook()), &content);

    assert!(ilyan_definitions.contains("checkpoint.audit_order"));
    assert!(ilyan_definitions.contains("checkpoint.show_charter"));
    assert!(ilyan_definitions.contains("checkpoint.recall_worker"));
    assert!(!ilyan_definitions.contains("checkpoint.blend_workers"));
    assert!(!ilyan_definitions.contains("checkpoint.pressure_guard"));
    assert!(rook_definitions.contains("checkpoint.blend_workers"));
    assert!(rook_definitions.contains("checkpoint.pressure_guard"));
    assert!(rook_definitions.contains("checkpoint.use_stolen_permit"));
    assert!(!rook_definitions.contains("checkpoint.audit_order"));
    assert!(!rook_definitions.contains("checkpoint.show_charter"));
    assert!(ilyan_definitions.contains("checkpoint.read_flag"));
    assert!(rook_definitions.contains("checkpoint.read_flag"));
    assert!(
        ilyan_definitions
            .symmetric_difference(&rook_definitions)
            .count()
            >= 4
    );
}

#[test]
fn lowsail_warning_changes_sluice_legality_and_report_transfers_knowledge() {
    let content = content();

    let baseline_floor = travel_to(
        travel_to(
            travel_to(state(&content, ilyan()), &content, "lowsail.docks"),
            &content,
            "lowsail.levee",
        ),
        &content,
        "red_sluice.floor",
    );
    assert!(!definitions(&baseline_floor, &content).contains("floor.open_relief"));

    let warned_floor = travel_to(
        travel_to(
            apply(
                travel_to(state(&content, ilyan()), &content, "lowsail.docks"),
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

    let mut reported = apply(state(&content, ilyan()), &content, "checkpoint.audit_order");
    assert!(reported.world.npcs["sava_rusk"].knows("forged_order"));
    assert_eq!(
        reported.world.npcs["sava_rusk"].knowledge["forged_order"]
            .provenance
            .kind(),
        KnowledgeProvenanceKind::Witnessed
    );
    assert!(reported.world.npcs["sava_rusk"].remembers("sava_witnessed_audit"));
    assert!(!reported.world.npcs["edrik_voss"].knows("forged_order"));

    reported = travel_to(reported, &content, "lowsail.levee");
    reported = apply(reported, &content, "levee.send_report");
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
fn sluice_split_persists_to_the_lowsail_return() {
    let content = content();
    let mut state = state(&content, ilyan());
    state = apply(state, &content, "checkpoint.audit_order");
    state = travel_to(state, &content, "lowsail.levee");
    state = travel_to(state, &content, "red_sluice.floor");
    state = apply(state, &content, "floor.read_harmonics");
    state = travel_to(state, &content, "red_sluice.top");
    state = apply(state, &content, "top.check_wheels");
    state = apply(state, &content, "top.split_flow");
    assert!(
        state.world.locations["lowsail.return"]
            .flags
            .contains("market_stable")
    );
    assert!(state.world.flags.contains("flow_split"));

    state = travel_to(state, &content, "lowsail.return");
    assert!(definitions(&state, &content).contains("return.share_water"));
    state = apply(state, &content, "return.share_water");
    assert!(state.world.flags.contains("ending_accord"));
    assert!(state.character.deeds.contains("returned_for_accord"));
}

#[test]
fn build_and_state_ids_are_stable_and_compiler_enforces_concise_text() {
    let first = content();
    let second = content();
    assert_eq!(first.build_id(), second.build_id());
    assert_eq!(first.build_id().len(), 64);
    assert_eq!(forge_kernel::compute_build_id(&first), first.build_id());

    let first_state = state(&first, ilyan());
    let second_state = state(&second, ilyan());
    assert_eq!(first_state.state_id(), second_state.state_id());
    assert_eq!(first_state.state_id().len(), 64);

    for (_, location) in first.locations() {
        assert!(location.description.split_whitespace().count() <= 36);
        assert!(location.description.matches('.').count() <= 2);
    }
    for (_, action) in first.actions() {
        assert!(action.label.split_whitespace().count() <= 8);
    }

    let mut draft = parse(SPLIT_TIDE).expect("real fixture must parse");
    draft.locations[0].description = "One two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen.".to_owned();
    let error = compile(draft).expect_err("overlong location text must fail compilation");
    assert!(error.to_string().contains("exceeds 18 words"));

    assert_eq!(
        first
            .location("lowsail.checkpoint")
            .expect("checkpoint")
            .description,
        "A red flag hangs over the checkpoint. Sava guards the only dry path to Lowsail Market."
    );
}
