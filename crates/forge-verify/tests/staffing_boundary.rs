use std::collections::BTreeMap;

use forge_content::parse_and_compile_production;
use forge_kernel::{EventKind, enumerate_legal_actions};
use forge_replay::Session;

const SOURCE: &str = include_str!("../../../content/split-tide.json");
const BRANN: &str = "fume_yards.brann_coil";
const BAY: &str = "fume_yards.kiln_bay";
const ASH: &str = "fume_yards.ash_beds";

#[test]
fn production_staffing_physically_removes_brann_from_kiln_supervision() {
    let content = parse_and_compile_production(SOURCE).unwrap();
    let mut session = Session::new_game("ilyan", 71, &content).unwrap();
    for (id, to) in [
        ("checkpoint.show_charter", None),
        ("travel_adjacent", Some("lowsail.levee")),
        ("levee.authority_path", None),
        ("travel_adjacent", Some("red_sluice.top")),
        ("top.hold_market", None),
        ("world.enter_aftermath", None),
        ("return.count_dry_stalls", None),
        ("return.visit_workshop", None),
        ("travel_adjacent", Some(ASH)),
        ("fume_yards.buy_collateral_filter", None),
        ("travel_adjacent", Some("fume_yards.workshop")),
        ("travel_adjacent", Some(BAY)),
        ("fume_yards.fit_dust_filter", None),
        ("fume_yards.share_rescue_account", None),
        ("fume_yards.assign_brann_salvage", None),
    ] {
        let parameters = to
            .map(|to| BTreeMap::from([("destination".into(), to.into())]))
            .unwrap_or_default();
        let action = enumerate_legal_actions(session.state(), &content)
            .unwrap()
            .into_iter()
            .find(|a| a.definition_id == id && a.parameters == parameters)
            .unwrap_or_else(|| panic!("canonical staffing preparation omitted {id}"));
        session.record(&action).unwrap();
    }
    assert_eq!(
        session.state().world.npcs[BRANN].location,
        ASH,
        "staffing left Brann at Kiln Bay"
    );
    assert_eq!(session.state().world.current_location, ASH);
    assert_eq!(session.state().world.time, 15);
    assert!(
        !session.state().world.locations[BAY]
            .entities
            .contains(BRANN)
    );
    assert!(
        session.state().world.locations[ASH]
            .entities
            .contains(BRANN)
    );
    assert_eq!(
        session.state().world.npcs[BRANN].inventory,
        BTreeMap::from([("fume_yards.fuel".into(), 1)])
    );
    let moves: Vec<_> = session
        .trace()
        .steps
        .last()
        .unwrap()
        .events
        .iter()
        .filter(|e| matches!(e.kind, EventKind::NpcMoved { .. } | EventKind::Moved { .. }))
        .map(|e| (e.turn, e.kind.clone()))
        .collect();
    assert_eq!(
        moves,
        vec![
            (
                14,
                EventKind::NpcMoved {
                    npc: BRANN.into(),
                    from: BAY.into(),
                    to: ASH.into()
                }
            ),
            (
                14,
                EventKind::Moved {
                    from: BAY.into(),
                    to: ASH.into()
                }
            ),
        ]
    );
    let walk = enumerate_legal_actions(session.state(), &content)
        .unwrap()
        .into_iter()
        .find(|a| a.definition_id == "fume_yards.leave_ash_hatch")
        .unwrap();
    session.record(&walk).unwrap();
    assert_eq!(session.state().world.current_location, BAY);
    assert_eq!(session.state().world.npcs[BRANN].location, ASH);
    let catalog = enumerate_legal_actions(session.state(), &content).unwrap();
    for id in ["fume_yards.take_fuel", "fume_yards.load_cold_freight"] {
        assert!(
            !catalog.iter().any(|a| a.definition_id == id),
            "absent foreman still supervised {id}"
        );
    }
}
