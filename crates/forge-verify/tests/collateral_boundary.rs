use forge_content::parse_and_compile_production;
use forge_kernel::{CompiledContent, EventKind, enumerate_legal_actions};
use forge_replay::Session;
use std::collections::BTreeMap;

const SOURCE: &str = include_str!("../../../content/split-tide.json");
const CAGE: &str = "fume_yards.collateral_cage";
const FILTER: &str = "fume_yards.filter";
const FUEL: &str = "fume_yards.fuel";

fn step(session: &mut Session<'_>, content: &CompiledContent, id: &str, destination: Option<&str>) {
    let parameters = destination
        .map(|value| BTreeMap::from([("destination".to_owned(), value.to_owned())]))
        .unwrap_or_default();
    let action = enumerate_legal_actions(session.state(), content)
        .unwrap()
        .into_iter()
        .find(|action| action.definition_id == id && action.parameters == parameters)
        .unwrap_or_else(|| panic!("canonical collateral preparation omitted {id}"));
    session
        .record(&action)
        .unwrap_or_else(|error| panic!("canonical collateral preparation failed: {error}"));
}

fn hold_market<'a>(content: &'a CompiledContent) -> Session<'a> {
    let mut session = Session::new_game("ilyan", 71, content).unwrap();
    for (id, destination) in [
        ("checkpoint.show_charter", None),
        ("travel_adjacent", Some("lowsail.levee")),
        ("levee.authority_path", None),
        ("travel_adjacent", Some("red_sluice.top")),
        ("top.hold_market", None),
        ("world.enter_aftermath", None),
        ("return.count_dry_stalls", None),
        ("return.visit_workshop", None),
    ] {
        step(&mut session, content, id, destination);
    }
    assert_eq!(session.state().character.resources["coin"], 10);
    session
}

#[test]
fn production_collateral_purchase_charges_exact_four_coins() {
    let content = parse_and_compile_production(SOURCE).unwrap();
    let mut session = hold_market(&content);
    step(
        &mut session,
        &content,
        "travel_adjacent",
        Some("fume_yards.ash_beds"),
    );
    step(
        &mut session,
        &content,
        "fume_yards.buy_collateral_filter",
        None,
    );
    assert_eq!(
        session.state().character.resources["coin"],
        6,
        "collateral purchase did not charge four coins"
    );
    assert_eq!(session.state().world.time, 10);
    assert_eq!(session.state().character.inventory.get(FILTER), Some(&1));
    assert!(session.state().world.storages[CAGE].inventory.is_empty());
    assert_eq!(
        session.state().world.npcs["fume_yards.daro_venn"].inventory,
        BTreeMap::from([(FILTER.to_owned(), 1)])
    );
    assert!(session.trace().steps.last().unwrap().events.iter().any(|event| matches!(
        &event.kind, EventKind::ResourceAdjusted { resource, amount: -4 } if resource == "coin"
    )));
}

#[test]
fn production_collateral_settlement_deposits_the_exact_fuel_lot() {
    let content = parse_and_compile_production(SOURCE).unwrap();
    let mut session = hold_market(&content);
    for (id, destination) in [
        ("travel_adjacent", Some("fume_yards.kiln_bay")),
        ("fume_yards.take_fuel", None),
        ("fume_yards.enter_ash_hatch", None),
        ("fume_yards.read_collateral_docket", None),
        ("fume_yards.settle_collateral_fuel", None),
    ] {
        step(&mut session, &content, id, destination);
    }
    assert!(
        !session.state().character.inventory.contains_key(FUEL)
            && session.state().world.storages[CAGE].inventory
                == BTreeMap::from([(FUEL.to_owned(), 1)]),
        "collateral settlement retained its fuel lot"
    );
    assert_eq!(session.state().character.resources["coin"], 10);
    assert_eq!(session.state().world.time, 13);
    assert_eq!(session.state().character.inventory.get(FILTER), Some(&1));
    assert!(
        session.state().world.npcs["fume_yards.brann_coil"]
            .inventory
            .is_empty()
    );
    let transfers: Vec<_> = session
        .trace()
        .steps
        .last()
        .unwrap()
        .events
        .iter()
        .filter(|event| {
            matches!(
                event.kind,
                EventKind::StorageItemTransferredToCharacter { .. }
                    | EventKind::CharacterItemTransferredToStorage { .. }
            )
        })
        .map(|event| (event.turn, event.kind.clone()))
        .collect();
    assert_eq!(
        transfers,
        vec![
            (
                12,
                EventKind::CharacterItemTransferredToStorage {
                    storage: CAGE.to_owned(),
                    item: FUEL.to_owned(),
                    count: 1
                }
            ),
            (
                12,
                EventKind::StorageItemTransferredToCharacter {
                    storage: CAGE.to_owned(),
                    item: FILTER.to_owned(),
                    count: 1
                }
            ),
        ]
    );
}
