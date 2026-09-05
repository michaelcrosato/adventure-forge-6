use forge_kernel::{
    ActionDefinition, CanonicalAction, Character, CharacterPreset, CompiledContent, Condition,
    ContentContract, ContentDraft, Effect, EventKind, LocationDefinition, RecipeDefinition,
    StorageDefinition, StringRef, enumerate_legal_actions,
};
use forge_replay::{PlayerTrace, Session, Trace, resume_player_trace, verify};
use std::collections::BTreeMap;

const CHEST: &str = "work.chest";
const ORE: &str = "work.ore";
const BAR: &str = "work.bar";

fn transfer(withdraw: bool, item: &str, count: u32) -> Effect {
    if withdraw {
        Effect::TransferStorageItemToCharacter {
            storage: CHEST.to_owned(),
            item: item.to_owned(),
            count,
        }
    } else {
        Effect::TransferCharacterItemToStorage {
            storage: CHEST.to_owned(),
            item: item.to_owned(),
            count,
        }
    }
}

fn action(id: &str, mut effects: Vec<Effect>) -> ActionDefinition {
    effects.push(Effect::AdvanceTime { ticks: 1 });
    ActionDefinition {
        id: id.to_owned(),
        label: id.to_owned(),
        category: "Action".to_owned(),
        result: "The work is done.".to_owned(),
        result_variants: Vec::new(),
        locations: Vec::new(),
        condition: Condition::Always,
        effects,
        parameters: Vec::new(),
        meaningful: false,
        movement: false,
    }
}

fn content() -> CompiledContent {
    let character = Character {
        id: "worker".to_owned(),
        lineage: "fenborn".to_owned(),
        origin: "gate".to_owned(),
        background: "smith".to_owned(),
        aptitudes: Default::default(),
        skills: Default::default(),
        values: Default::default(),
        traits: Default::default(),
        flaws: Default::default(),
        appearance: Default::default(),
        affiliations: Default::default(),
        reputation: Default::default(),
        knowledge: Default::default(),
        inventory: BTreeMap::from([(ORE.to_owned(), 1)]),
        resources: Default::default(),
        injuries: Default::default(),
        deeds: Default::default(),
        promises: Default::default(),
        discoveries: Default::default(),
        facets: Default::default(),
    };
    let location = |id: &str, exit: &str| LocationDefinition {
        id: id.to_owned(),
        name: id.to_owned(),
        description: "A quiet work site.".to_owned(),
        description_variants: Vec::new(),
        exits: vec![exit.to_owned()],
        terminal: true,
    };
    CompiledContent::try_compile(ContentDraft {
        schema_version: "forge-schema-v10".to_owned(),
        rules_version: "forge-rules-v8".to_owned(),
        world_id: "storage-fixture".to_owned(),
        contract: ContentContract::Fixture,
        start_location: "gate".to_owned(),
        character_presets: vec![CharacterPreset {
            id: "worker".to_owned(),
            display_name: "Worker".to_owned(),
            summary: "A worker arrives.".to_owned(),
            character,
        }],
        character_creation: None,
        supply_labels: Default::default(),
        recipes: vec![RecipeDefinition {
            id: "work.refine".to_owned(),
            inputs: BTreeMap::from([(ORE.to_owned(), 2)]),
            outputs: BTreeMap::from([(BAR.to_owned(), 1)]),
        }],
        storages: vec![StorageDefinition {
            id: CHEST.to_owned(),
            name: "Work chest".to_owned(),
            location: "gate".to_owned(),
            inventory: BTreeMap::from([(ORE.to_owned(), 2)]),
        }],
        locations: vec![location("gate", "yard"), location("yard", "gate")],
        npcs: Vec::new(),
        timed_events: Vec::new(),
        deferred_events: Vec::new(),
        actions: vec![
            action(
                "exchange",
                vec![
                    transfer(false, ORE, 1),
                    transfer(true, ORE, 2),
                    Effect::ApplyRecipe {
                        recipe: "work.refine".to_owned(),
                    },
                    transfer(false, BAR, 1),
                ],
            ),
            action(
                "leave",
                vec![Effect::MoveCharacter {
                    location: StringRef::Literal("yard".to_owned()),
                }],
            ),
            action(
                "return",
                vec![Effect::MoveCharacter {
                    location: StringRef::Literal("gate".to_owned()),
                }],
            ),
            action("withdraw", vec![transfer(true, BAR, 1)]),
            action("deposit", vec![transfer(false, BAR, 1)]),
            action(
                "random-withdraw",
                vec![
                    Effect::RandomChance {
                        success_percent: 50,
                        on_success: Box::new(Effect::MoveCharacter {
                            location: StringRef::Literal("yard".to_owned()),
                        }),
                        on_failure: Box::new(Effect::Noop),
                    },
                    Effect::MoveCharacter {
                        location: StringRef::Literal("gate".to_owned()),
                    },
                    transfer(true, BAR, 1),
                ],
            ),
        ],
    })
    .unwrap()
}

fn select(session: &Session<'_>, content: &CompiledContent, id: &str) -> CanonicalAction {
    enumerate_legal_actions(session.state(), content)
        .unwrap()
        .into_iter()
        .find(|action| action.definition_id == id)
        .unwrap_or_else(|| panic!("missing {id}"))
}

fn checkpoint<'a>(session: &Session<'a>, content: &'a CompiledContent) -> Session<'a> {
    let save = session.player_trace().unwrap().to_json().unwrap();
    for hidden in ["\"storages\"", "\"inventory\"", "\"events\"", "\"entropy\""] {
        assert!(!save.contains(hidden), "safe save exposes {hidden}");
    }
    let resumed = resume_player_trace(&PlayerTrace::from_json(&save).unwrap(), content).unwrap();
    assert_eq!(resumed.state(), session.state());
    assert_eq!(resumed.trace(), session.trace());
    assert_eq!(resumed.player_trace().unwrap().to_json().unwrap(), save);
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

#[test]
fn storage_native_save_roundtrips_preserve_balances_locality_recipe_events_and_entropy() {
    let content = content();
    for seed in [27, 123] {
        let mut uninterrupted = Session::new_game("worker", seed, &content).unwrap();
        let mut resumed = checkpoint(&uninterrupted, &content);
        for id in [
            "exchange",
            "leave",
            "return",
            "withdraw",
            "deposit",
            "random-withdraw",
        ] {
            let left = select(&uninterrupted, &content, id);
            let right = select(&resumed, &content, id);
            assert_eq!(left, right);
            let recorded = uninterrupted.record(&left).unwrap();
            assert_eq!(resumed.record(&right).unwrap(), recorded);
            assert_eq!(resumed.state(), uninterrupted.state());
            if id == "exchange" {
                assert_eq!(
                    uninterrupted.state().world.storages[CHEST].inventory,
                    BTreeMap::from([(ORE.to_owned(), 1), (BAR.to_owned(), 1)])
                );
                assert!(uninterrupted.state().character.inventory.is_empty());
                assert!(matches!(&recorded.events[..], [
                    forge_kernel::Event { kind: EventKind::CharacterItemTransferredToStorage { storage, item, count: 1 }, .. },
                    forge_kernel::Event { kind: EventKind::StorageItemTransferredToCharacter { count: 2, .. }, .. },
                    forge_kernel::Event { kind: EventKind::RecipeApplied { .. }, .. },
                    forge_kernel::Event { kind: EventKind::CharacterItemTransferredToStorage { count: 1, .. }, .. },
                    forge_kernel::Event { kind: EventKind::TimeAdvanced { ticks: 1 }, .. },
                ] if storage == CHEST && item == ORE));
            }
            if id == "leave" {
                let ids = enumerate_legal_actions(uninterrupted.state(), &content).unwrap();
                assert!(!ids.iter().any(|action| action.definition_id == "withdraw"));
                assert_eq!(
                    uninterrupted.state().world.storages[CHEST].inventory[BAR],
                    1
                );
            }
            let before = resumed.trace().clone();
            let state_before = resumed.state().clone();
            assert!(
                resumed.record(&right).is_err(),
                "old storage action must stay stale"
            );
            assert_eq!(resumed.trace(), &before);
            assert_eq!(resumed.state(), &state_before);
            resumed = checkpoint(&resumed, &content);
        }
        assert_eq!(uninterrupted.state().world.time, 6);
        assert_eq!(uninterrupted.state().world.current_location, "gate");
        assert_eq!(
            uninterrupted.state().character.inventory,
            BTreeMap::from([(BAR.to_owned(), 1)])
        );
        assert_eq!(
            uninterrupted.state().world.storages[CHEST].inventory,
            BTreeMap::from([(ORE.to_owned(), 1)])
        );
        assert_eq!(uninterrupted.state().entropy.cursor, 1);
        assert_eq!(
            uninterrupted.player_trace().unwrap(),
            resumed.player_trace().unwrap()
        );
    }
}

#[test]
fn storage_trace_rejects_changed_direction_count_identity_and_missing_transfer() {
    let content = content();
    let mut session = Session::new_game("worker", 71, &content).unwrap();
    let action = select(&session, &content, "exchange");
    session.record(&action).unwrap();
    let original = Trace::from_json(&session.trace().to_json().unwrap()).unwrap();
    assert_eq!(verify(&original, &content).unwrap(), *session.state());
    let mut mutations = Vec::new();
    let mut wrong_direction = original.clone();
    wrong_direction.steps[0].events[0].kind = EventKind::StorageItemTransferredToCharacter {
        storage: CHEST.to_owned(),
        item: ORE.to_owned(),
        count: 1,
    };
    mutations.push(wrong_direction);
    for kind in [
        EventKind::CharacterItemTransferredToStorage {
            storage: CHEST.to_owned(),
            item: ORE.to_owned(),
            count: 2,
        },
        EventKind::CharacterItemTransferredToStorage {
            storage: "work.other".to_owned(),
            item: ORE.to_owned(),
            count: 1,
        },
    ] {
        let mut changed = original.clone();
        changed.steps[0].events[0].kind = kind;
        mutations.push(changed);
    }
    let mut missing = original.clone();
    missing.steps[0].events.remove(0);
    mutations.push(missing);
    for changed in mutations {
        let decoded = Trace::from_json(&changed.to_json().unwrap()).unwrap();
        assert!(verify(&decoded, &content).is_err());
    }
    let mut save = serde_json::to_value(session.player_trace().unwrap()).unwrap();
    save["action_ids"] = serde_json::json!([]);
    assert!(
        resume_player_trace(
            &PlayerTrace::from_json(&save.to_string()).unwrap(),
            &content
        )
        .is_err()
    );
}
