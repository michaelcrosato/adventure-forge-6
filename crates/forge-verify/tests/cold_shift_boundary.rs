use std::collections::BTreeMap;

use forge_content::parse_and_compile_production;
use forge_kernel::{
    CharacterChoiceSelection, CharacterSelection, CompiledContent, EventKind, KnowledgeProvenance,
    enumerate_legal_actions,
};
use forge_replay::{Session, resume_player_trace};

const SOURCE: &str = include_str!("../../../content/split-tide.json");
const WORK: &str = "fume_yards.workshop";
const BAY: &str = "fume_yards.kiln_bay";
const NESSA: &str = "fume_yards.nessa_tern";
const BRANN: &str = "fume_yards.brann_coil";
const TEST: &str = "fume_yards.charge_dust_test";

fn record(
    session: &mut Session<'_>,
    content: &CompiledContent,
    id: &str,
    destination: Option<&str>,
) {
    let parameters = destination
        .map(|value| BTreeMap::from([("destination".into(), value.into())]))
        .unwrap_or_default();
    let action = enumerate_legal_actions(session.state(), content)
        .unwrap()
        .into_iter()
        .find(|a| a.definition_id == id && a.parameters == parameters)
        .unwrap_or_else(|| panic!("canonical dust-report preparation omitted {id}"));
    session.record(&action).unwrap();
}

#[test]
fn production_dust_report_physically_brings_its_actual_source() {
    let content = parse_and_compile_production(SOURCE).unwrap();
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
    let mut reported = Session::new_custom_game(&selection, 71, &content).unwrap();
    for (id, to) in [
        ("checkpoint.show_charter", None),
        ("travel_adjacent", Some("lowsail.levee")),
        ("levee.authority_path", None),
        ("travel_adjacent", Some("red_sluice.top")),
        ("top.hold_market", None),
        ("world.enter_aftermath", None),
        ("return.count_dry_stalls", None),
        ("return.visit_workshop", None),
        ("fume_yards.take_stock", None),
        ("travel_adjacent", Some(BAY)),
        ("fume_yards.prepare_charge", None),
        ("travel_adjacent", Some(WORK)),
        ("fume_yards.test_unfired_charge", None),
    ] {
        record(&mut reported, &content, id, to);
    }
    assert_eq!(reported.state().world.time, 13);
    let original_source = reported.state().world.npcs[NESSA].knowledge[TEST].clone();
    assert_eq!(original_source.turn, 12);
    assert_eq!(original_source.provenance, KnowledgeProvenance::Witnessed);
    assert!(
        !reported.state().world.npcs[BRANN]
            .knowledge
            .contains_key(TEST)
    );
    let original_character_inventory = reported.state().character.inventory.clone();
    let original_resources = reported.state().character.resources.clone();
    let original_entropy = reported.state().entropy.clone();
    let original_npc_inventory: BTreeMap<_, _> = reported
        .state()
        .world
        .npcs
        .iter()
        .map(|(id, npc)| (id.clone(), npc.inventory.clone()))
        .collect();
    let original_storage = reported.state().world.storages.clone();

    // The matched canonical fork establishes that travel itself carries no
    // technical finding and does not silently move its source.
    let mut unreported = resume_player_trace(&reported.player_trace().unwrap(), &content).unwrap();
    record(&mut unreported, &content, "travel_adjacent", Some(BAY));
    assert_eq!(unreported.state().world.time, 14);
    assert_eq!(unreported.state().world.npcs[NESSA].location, WORK);
    assert!(
        !unreported.state().world.npcs[BRANN]
            .knowledge
            .contains_key(TEST)
    );

    record(&mut reported, &content, "fume_yards.report_test", None);
    // Keep this physical assertion first after the escort. The source-copy
    // defect must fail here, not at a later missing action or replay hash.
    assert_eq!(
        reported.state().world.npcs[NESSA].location,
        BAY,
        "dust report left Nessa at Workshop"
    );
    let state = reported.state();
    assert_eq!(state.world.current_location, BAY);
    assert_eq!(state.world.npcs[BRANN].location, BAY);
    assert_eq!(state.world.time, 14);
    assert!(!state.world.locations[WORK].entities.contains(NESSA));
    assert!(state.world.locations[BAY].entities.contains(NESSA));
    assert_eq!(state.world.npcs[NESSA].knowledge[TEST], original_source);
    let recipient = &state.world.npcs[BRANN].knowledge[TEST];
    assert_eq!(recipient.turn, 13);
    assert_eq!(
        recipient.provenance,
        KnowledgeProvenance::Told { by: NESSA.into() }
    );
    for npc in ["fume_yards.daro_venn", "fume_yards.pera_senn", "oren_pell"] {
        assert!(!state.world.npcs[npc].knowledge.contains_key(TEST));
    }
    assert_eq!(state.character.inventory, original_character_inventory);
    assert_eq!(state.character.resources, original_resources);
    assert_eq!(state.entropy, original_entropy);
    assert_eq!(state.world.storages, original_storage);
    for (id, inventory) in original_npc_inventory {
        assert_eq!(state.world.npcs[&id].inventory, inventory);
    }
    let actual: Vec<_> = reported
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
            )
        })
        .map(|event| (event.turn, event.kind.clone()))
        .collect();
    assert_eq!(
        actual,
        vec![
            (
                13,
                EventKind::NpcMoved {
                    npc: NESSA.into(),
                    from: WORK.into(),
                    to: BAY.into(),
                },
            ),
            (
                13,
                EventKind::Moved {
                    from: WORK.into(),
                    to: BAY.into(),
                },
            ),
            (
                13,
                EventKind::NpcKnowledgeAdded {
                    npc: BRANN.into(),
                    knowledge: TEST.into(),
                },
            ),
        ]
    );
}
