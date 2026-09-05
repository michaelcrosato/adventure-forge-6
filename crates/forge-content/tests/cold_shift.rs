use std::collections::{BTreeMap, BTreeSet};

use forge_content::parse_and_compile_production;
use forge_kernel::{
    CanonicalAction, CharacterChoiceSelection, CharacterSelection, CompiledContent, EventKind,
    GameState, KnowledgeProvenance, enumerate_legal_actions, step,
};

const SOURCE: &str = include_str!("../../../content/split-tide.json");
const W: &str = "fume_yards.workshop";
const K: &str = "fume_yards.kiln_bay";
const A: &str = "fume_yards.ash_beds";
const L: &str = "lowsail.return";
const N: &str = "fume_yards.nessa_tern";
const B: &str = "fume_yards.brann_coil";
const D: &str = "fume_yards.daro_venn";
const P: &str = "fume_yards.pera_senn";
const TEST: &str = "fume_yards.test_unfired_charge";
const REPORT: &str = "fume_yards.report_test";
const SHIFT: &str = "fume_yards.delegate_cold_shift";
const NBACK: &str = "fume_yards.return_with_nessa";
const BBACK: &str = "fume_yards.return_brann_to_kiln";
const FACT: &str = "fume_yards.charge_dust_test";
const CHARGE: &str = "fume_yards.prepared_charge";
const REPAIR: &str = "fume_yards.repair_lot";
const FILTER: &str = "fume_yards.filter";
const FUEL: &str = "fume_yards.fuel";
const CASK: &str = "fume_yards.water_cask";
const CAGE: &str = "fume_yards.collateral_cage";
const OFFER: &str = "Make plugs, save two stamina; firing, dust fitting and rack lifts end; return Brann from Workshop.";
type Spec = (&'static str, Option<&'static str>);
const HOLD: &[Spec] = &[
    ("checkpoint.show_charter", None),
    ("travel_adjacent", Some("lowsail.levee")),
    ("levee.authority_path", None),
    ("travel_adjacent", Some("red_sluice.top")),
    ("top.hold_market", None),
    ("world.enter_aftermath", None),
    ("return.count_dry_stalls", None),
];

fn content() -> CompiledContent {
    parse_and_compile_production(SOURCE).unwrap()
}
fn select(s: &GameState, c: &CompiledContent, id: &str, to: Option<&str>) -> CanonicalAction {
    let parameters = to
        .map(|x| BTreeMap::from([("destination".into(), x.into())]))
        .unwrap_or_default();
    enumerate_legal_actions(s, c)
        .unwrap()
        .into_iter()
        .find(|a| a.definition_id == id && a.parameters == parameters)
        .unwrap_or_else(|| {
            panic!(
                "missing {id} {to:?} at {} t{}; {}",
                s.world.current_location,
                s.world.time,
                c.observe(s).unwrap().text
            )
        })
}
fn catalog(s: &GameState, c: &CompiledContent) -> BTreeSet<String> {
    let all = enumerate_legal_actions(s, c).unwrap();
    let mut ids = vec![];
    let mut offset = 0;
    loop {
        let page = c.action_page(s, offset, 7).unwrap();
        assert_eq!(page.total, all.len());
        ids.extend(page.actions.into_iter().map(|a| a.action_id));
        if let Some(next) = page.next_offset {
            offset = next
        } else {
            break;
        }
    }
    assert_eq!(
        ids,
        all.iter().map(|a| a.action_id.clone()).collect::<Vec<_>>()
    );
    all.into_iter().map(|a| a.definition_id).collect()
}
fn apply(s: GameState, c: &CompiledContent, id: &str, to: Option<&str>) -> GameState {
    let action = select(&s, c, id, to);
    let ticks = if id == SHIFT { 3 } else { 1 };
    let view = c
        .action_page(&s, 0, usize::MAX)
        .unwrap()
        .actions
        .into_iter()
        .find(|a| a.action_id == action.action_id)
        .unwrap();
    assert_eq!(
        (view.time_cost.minimum_ticks, view.time_cost.maximum_ticks),
        (ticks, ticks)
    );
    let t = step(&s, &action, c, &s.entropy).unwrap();
    let o = c.observe_after_transition(&t).unwrap();
    assert!(
        o.text.split_whitespace().count() + o.supplies.summary().split_whitespace().count() < 100,
        "{}",
        o.text
    );
    assert_eq!(t.state().world.time, s.world.time + ticks);
    catalog(t.state(), c);
    t.into_state()
}
fn act(s: GameState, c: &CompiledContent, id: &str) -> GameState {
    apply(s, c, id, None)
}
fn travel(s: GameState, c: &CompiledContent, to: &str) -> GameState {
    apply(s, c, "travel_adjacent", Some(to))
}
fn run(mut s: GameState, c: &CompiledContent, path: &[Spec]) -> GameState {
    for (id, to) in path {
        s = apply(s, c, id, *to)
    }
    s
}
fn absent(s: &GameState, c: &CompiledContent, ids: &[&str]) {
    let set = catalog(s, c);
    for id in ids {
        assert!(!set.contains(*id), "unexpected {id} at t{}", s.world.time)
    }
}
fn flag(s: &GameState, at: &str, f: &str) -> bool {
    s.world.locations[at].flags.contains(f)
}
fn custom(c: &CompiledContent, choices: &[&str; 6], seed: u64) -> GameState {
    c.new_custom_game(
        &CharacterSelection {
            name: "Cold shift comparison".into(),
            choices: ["lineage", "origin", "calling", "value", "burden", "history"]
                .into_iter()
                .zip(choices)
                .map(|(s, ch)| CharacterChoiceSelection {
                    slot_id: s.into(),
                    choice_id: (*ch).into(),
                })
                .collect(),
        },
        seed,
    )
    .unwrap()
}
fn start(c: &CompiledContent) -> GameState {
    custom(
        c,
        &[
            "fenborn",
            "lowsail",
            "ledger-clerk",
            "order",
            "wanted",
            "stole-permit",
        ],
        71,
    )
}
fn prepare_from_return(s: GameState, c: &CompiledContent) -> GameState {
    run(
        s,
        c,
        &[
            ("return.visit_workshop", None),
            ("fume_yards.take_stock", None),
            ("travel_adjacent", Some(K)),
            ("fume_yards.prepare_charge", None),
            ("travel_adjacent", Some(W)),
        ],
    )
}
fn prepared(c: &CompiledContent) -> GameState {
    prepare_from_return(run(start(c), c, HOLD), c)
}
fn tested(c: &CompiledContent) -> GameState {
    act(prepared(c), c, TEST)
}
fn relayed(c: &CompiledContent) -> GameState {
    act(tested(c), c, REPORT)
}
fn delegated(c: &CompiledContent) -> GameState {
    act(relayed(c), c, SHIFT)
}
fn personal(c: &CompiledContent) -> GameState {
    run(
        relayed(c),
        c,
        &[
            ("fume_yards.reclaim_charge", None),
            ("fume_yards.load_kiln_freight", None),
            (NBACK, None),
        ],
    )
}
fn surge(s: &GameState) -> Vec<(u64, bool)> {
    s.event_log
        .iter()
        .filter_map(|e| match &e.kind {
            EventKind::ScheduledEventResolved {
                event_id, applied, ..
            } if event_id == "lowsail.next_surge" => Some((e.turn, *applied)),
            _ => None,
        })
        .collect()
}

#[test]
fn test_and_actual_delivery_preserve_goods_and_original_sources() {
    let c = content();
    let before = prepared(&c);
    let s = act(before.clone(), &c, TEST);
    assert_eq!(s.world.time, 13);
    assert_eq!(s.character.inventory, before.character.inventory);
    assert_eq!(s.character.resources, before.character.resources);
    assert_eq!(s.world.npcs[N].knowledge[FACT].turn, 12);
    assert_eq!(
        s.world.npcs[N].knowledge[FACT].provenance,
        KnowledgeProvenance::Witnessed
    );
    absent(&s, &c, &[TEST]);
    let alone = travel(s.clone(), &c, K);
    assert!(!alone.world.npcs[B].knows(FACT));
    absent(&alone, &c, &[SHIFT]);
    assert!(catalog(&alone, &c).contains("fume_yards.reclaim_charge"));
    let delivered = act(s.clone(), &c, REPORT);
    assert_eq!(delivered.world.npcs[N].location, K);
    assert_eq!(delivered.world.npcs[B].location, K);
    assert_eq!(delivered.world.npcs[N].knowledge[FACT].turn, 12);
    assert_eq!(delivered.world.npcs[B].knowledge[FACT].turn, 13);
    assert_eq!(
        delivered.world.npcs[B].knowledge[FACT].provenance,
        KnowledgeProvenance::Told { by: N.into() }
    );
    assert_eq!(delivered.character.inventory, alone.character.inventory);
    assert_eq!(delivered.character.resources, alone.character.resources);
    let changed = &delivered.event_log[s.event_log.len()..];
    let nmove = changed
        .iter()
        .position(|e| matches!(&e.kind,EventKind::NpcMoved{npc,to,..} if npc==N && to==K))
        .unwrap();
    let pmove = changed
        .iter()
        .position(|e| matches!(&e.kind,EventKind::Moved{to,..} if to==K))
        .unwrap();
    let learned=changed.iter().position(|e|matches!(&e.kind,EventKind::NpcKnowledgeAdded{npc,knowledge} if npc==B && knowledge==FACT)).unwrap();
    assert!(nmove < pmove && pmove < learned);
    assert_eq!(changed[learned].turn, 13);
    for npc in [D, P, "oren_pell"] {
        assert!(!delivered.world.npcs[npc].knows(FACT));
    }
    let action = select(&s, &c, REPORT, None);
    assert!(step(&delivered, &action, &c, &delivered.entropy).is_err());
}

#[test]
fn shift_has_truthful_prechoice_costs_three_ticks_and_one_consumption_payment() {
    let c = content();
    let s = relayed(&c);
    let o = c.observe(&s).unwrap();
    assert!(o.text.contains(OFFER), "{}", o.text);
    for part in [
        "plugs",
        "firing",
        "dust fitting",
        "rack lifts",
        "Workshop",
        "save two stamina",
    ] {
        assert!(o.text.contains(part), "{}", o.text)
    }
    let a = select(&s, &c, SHIFT, None);
    let row = c
        .action_page(&s, 0, usize::MAX)
        .unwrap()
        .actions
        .into_iter()
        .find(|v| v.action_id == a.action_id)
        .unwrap();
    assert_eq!(row.label, "Cold Crew Shift (+3 coin, no stamina)");
    let after = act(s.clone(), &c, SHIFT);
    let other = personal(&c);
    assert_eq!(
        (
            after.world.time,
            after.character.resources["coin"],
            after.character.resources["stamina"]
        ),
        (17, 13, 3)
    );
    assert_eq!(
        (
            other.world.time,
            other.character.resources["coin"],
            other.character.resources["stamina"]
        ),
        (17, 13, 1)
    );
    assert_eq!(after.character.inventory, other.character.inventory);
    assert_eq!(after.character.inventory[REPAIR], 1);
    assert!(!after.character.inventory.contains_key(CHARGE));
    assert_eq!(after.world.npcs[B].location, W);
    assert_eq!(other.world.npcs[B].location, K);
    assert_eq!(after.world.npcs[N].location, W);
    assert_eq!(after.world.npcs[B].inventory[FUEL], 1);
    assert_eq!(
        after.world.npcs[B].memories["fume_yards.crew_charge_reclaimed"].subject,
        "Brann reclaimed the player's unfired charge."
    );
    assert_eq!(
        after.world.npcs[B].memories["fume_yards.kiln_freight_paid"].turn,
        14
    );
    assert!(
        !after.world.npcs[N]
            .memories
            .contains_key("fume_yards.cold_shift_completed")
    );
    assert_eq!(surge(&after), vec![(17, false)]);
    assert_eq!(surge(&other), vec![(16, false)]);
    assert_eq!(after.entropy, s.entropy);
    assert!(step(&after, &a, &c, &after.entropy).is_err());
    let returned = act(after, &c, BBACK);
    absent(
        &returned,
        &c,
        &[
            SHIFT,
            "fume_yards.reclaim_charge",
            "fume_yards.prepare_charge",
            "fume_yards.fit_dust_filter",
            "fume_yards.load_kiln_freight",
            "fume_yards.load_cold_freight",
            "fume_yards.ignite_batch",
        ],
    );
    assert!(!flag(&returned, A, "fume_yards.salvage_assignment_spent"));
}

#[test]
fn every_custom_start_retains_the_ordinary_method_and_can_establish_the_report() {
    let c = content();
    let mut count = 0;
    for mask in 0..64 {
        let selection = CharacterSelection {
            name: "Ordinary cold worker".into(),
            choices: c
                .character_creation()
                .unwrap()
                .slots
                .iter()
                .enumerate()
                .map(|(i, s)| CharacterChoiceSelection {
                    slot_id: s.id.clone(),
                    choice_id: s.choices[(mask >> i) & 1].id.clone(),
                })
                .collect(),
        };
        let s = c.new_custom_game(&selection, 71).unwrap();
        let stamina = s.character.resources["stamina"];
        let coin = s.character.resources["coin"];
        let prepared = run(
            s,
            &c,
            &[
                ("travel_adjacent", Some("lowsail.levee")),
                ("travel_adjacent", Some(W)),
                ("fume_yards.take_stock", None),
                ("travel_adjacent", Some(K)),
                ("fume_yards.prepare_charge", None),
            ],
        );
        let ordinary = run(
            prepared.clone(),
            &c,
            &[
                ("fume_yards.reclaim_charge", None),
                ("fume_yards.load_kiln_freight", None),
            ],
        );
        assert_eq!(ordinary.character.resources["coin"], coin + 3);
        assert_eq!(ordinary.character.resources["stamina"], stamina - 2);
        assert!(!ordinary.world.npcs[N].knows(FACT));
        assert!(!ordinary.world.npcs[B].knows(FACT));
        let s = travel(prepared, &c, W);
        let s = act(s, &c, TEST);
        let s = act(s, &c, REPORT);
        let s = act(s, &c, SHIFT);
        assert_eq!(s.character.resources["coin"], coin + 3);
        assert_eq!(s.character.resources["stamina"], stamina);
        assert_eq!(s.character.inventory[REPAIR], 1);
        count += 1;
    }
    assert_eq!(count, 64);
}

#[test]
fn council_ink_and_wanted_change_the_reaction_without_becoming_a_work_gate() {
    let c = content();
    for (lineage, burden, reacts) in [
        ("fenborn", "wanted", true),
        ("fenborn", "indebted", false),
        ("kilnborn", "wanted", false),
    ] {
        let s = custom(
            &c,
            &[
                lineage,
                "lowsail",
                "ledger-clerk",
                "order",
                burden,
                "stole-permit",
            ],
            71,
        );
        let s = run(
            s,
            &c,
            &[
                ("travel_adjacent", Some("lowsail.levee")),
                ("travel_adjacent", Some(W)),
                ("fume_yards.take_stock", None),
                ("travel_adjacent", Some(K)),
                ("fume_yards.prepare_charge", None),
                ("travel_adjacent", Some(W)),
                (TEST, None),
            ],
        );
        let a = select(&s, &c, REPORT, None);
        let t = step(&s, &a, &c, &s.entropy).unwrap();
        let text = c.observe_after_transition(&t).unwrap().text;
        assert_eq!(
            text.contains("council ink and wanted face trouble Brann"),
            reacts,
            "{text}"
        );
        assert!(text.contains(OFFER), "{text}");
        assert!(catalog(t.state(), &c).contains(SHIFT));
    }
}

#[test]
fn nessa_walkaway_and_return_restore_presence_without_erasing_the_report() {
    let c = content();
    let s = relayed(&c);
    let walked = travel(s.clone(), &c, W);
    absent(&walked, &c, &["fume_yards.load_freight"]);
    assert!(
        c.observe(&walked)
            .unwrap()
            .text
            .contains("Nessa remains at Kiln Bay")
    );
    let returned = act(s.clone(), &c, NBACK);
    assert_eq!(returned.character, walked.character);
    assert!(catalog(&returned, &c).contains("fume_yards.load_freight"));
    assert_eq!(
        returned.world.npcs[B].knowledge[FACT],
        s.world.npcs[B].knowledge[FACT]
    );
    let returned = travel(returned, &c, K);
    let offer = c.observe(&returned).unwrap().text;
    assert!(offer.contains(OFFER), "{offer}");
    let n = returned.world.npcs[N].clone();
    let offset = returned.event_log.len();
    let resumed = act(returned, &c, SHIFT);
    assert_eq!(resumed.world.time, 19);
    assert_eq!(resumed.world.npcs[N], n);
    assert!(
        !resumed.event_log[offset..]
            .iter()
            .any(|e| matches!(&e.kind,EventKind::NpcMoved{npc,..} if npc==N))
    );
    let recovered = act(travel(walked, &c, K), &c, NBACK);
    assert_eq!(recovered.world.npcs[N].location, W);
    absent(&recovered, &c, &[TEST, REPORT]);
}

#[test]
fn fuel_ownership_and_consumed_cask_survive_delegated_reclamation() {
    let c = content();
    for mode in ["brann", "player", "cage", "wet"] {
        let s = act(run(start(&c), &c, HOLD), &c, "return.visit_workshop");
        let s = act(s, &c, "fume_yards.take_stock");
        let mut s = travel(s, &c, K);
        if mode == "player" || mode == "cage" {
            s = act(s, &c, "fume_yards.take_fuel");
        }
        if mode == "wet" {
            s = act(s, &c, "fume_yards.take_cask");
            s = act(s, &c, "fume_yards.fit_wet_screen");
        }
        s = act(s, &c, "fume_yards.prepare_charge");
        if mode == "cage" {
            s = run(
                s,
                &c,
                &[
                    ("fume_yards.enter_ash_hatch", None),
                    ("fume_yards.read_collateral_docket", None),
                    ("fume_yards.settle_collateral_fuel", None),
                    ("fume_yards.leave_ash_hatch", None),
                ],
            );
        }
        let fuel_owner = (
            s.character.inventory.get(FUEL).copied(),
            s.world.npcs[B].inventory.get(FUEL).copied(),
            s.world.storages[CAGE].inventory.get(FUEL).copied(),
        );
        let pera = s.world.npcs[P].clone();
        s = travel(s, &c, W);
        s = act(s, &c, TEST);
        s = act(s, &c, REPORT);
        s = act(s, &c, SHIFT);
        assert_eq!(
            (
                s.character.inventory.get(FUEL).copied(),
                s.world.npcs[B].inventory.get(FUEL).copied(),
                s.world.storages[CAGE].inventory.get(FUEL).copied()
            ),
            fuel_owner
        );
        assert_eq!(s.world.npcs[P], pera);
        assert_eq!(s.character.inventory[REPAIR], 1);
        if mode == "wet" {
            assert!(!s.character.inventory.contains_key(CASK));
            s = act(s, &c, "world.enter_aftermath");
            s = act(s, &c, "return.patch_stand");
            absent(&s, &c, &["return.order_water_stand"]);
        }
    }
}

#[test]
fn nessa_can_return_after_ignition_banking_spoilage_or_permanent_protection() {
    let c = content();
    for mode in ["heating", "ready", "banked", "spoiled", "protected"] {
        let mut s = relayed(&c);
        if mode == "protected" {
            s = run(
                s,
                &c,
                &[
                    ("fume_yards.enter_ash_hatch", None),
                    ("fume_yards.buy_collateral_filter", None),
                    ("fume_yards.leave_ash_hatch", None),
                    ("fume_yards.fit_dust_filter", None),
                ],
            );
        } else {
            s = run(
                s,
                &c,
                &[
                    ("fume_yards.take_fuel", None),
                    ("fume_yards.take_cask", None),
                    ("fume_yards.fit_wet_screen", None),
                    ("fume_yards.ignite_batch", None),
                ],
            );
            if mode == "banked" {
                s = act(s, &c, "fume_yards.bank_kiln");
            }
            if mode == "ready" {
                s = act(s, &c, "wait_tide");
                assert!(flag(&s, K, "fume_yards.batch_ready"));
                assert!(
                    c.observe(&s)
                        .unwrap()
                        .text
                        .contains("Draw or bank before spoilage")
                );
            }
            if mode == "spoiled" {
                for _ in 0..4 {
                    s = act(s, &c, "wait_tide");
                }
            }
        }
        absent(&s, &c, &[SHIFT]);
        assert!(catalog(&s, &c).contains(NBACK));
        let test = s.world.npcs[N].knowledge[FACT].clone();
        let report = s.world.npcs[B].knowledge[FACT].clone();
        let before = s.world.time;
        s = act(s, &c, NBACK);
        assert_eq!(s.world.time, before + 1);
        assert_eq!(s.world.npcs[N].location, W);
        assert_eq!(s.world.npcs[N].knowledge[FACT], test);
        assert_eq!(s.world.npcs[B].knowledge[FACT], report);
        assert!(catalog(&s, &c).contains("fume_yards.load_freight"));
    }
}

#[test]
fn aftermath_travel_leaves_nessa_at_the_bay_until_her_real_return() {
    let c = content();
    let s = relayed(&c);
    let source = s.world.npcs[N].knowledge[FACT].clone();
    let s = act(s, &c, "world.enter_aftermath");
    assert_eq!(s.world.npcs[N].location, K);
    let s = act(s, &c, "return.visit_workshop");
    assert!(
        c.observe(&s)
            .unwrap()
            .text
            .contains("Nessa remains at Kiln Bay")
    );
    absent(&s, &c, &["fume_yards.load_freight"]);
    let s = act(travel(s, &c, K), &c, NBACK);
    assert_eq!(s.world.npcs[N].location, W);
    assert_eq!(s.world.npcs[N].knowledge[FACT], source);
    assert!(catalog(&s, &c).contains("fume_yards.load_freight"));
}

#[test]
fn prior_rescue_account_does_not_reopen_dust_or_staffing_after_cold_work() {
    let c = content();
    let s = custom(
        &c,
        &[
            "fenborn",
            "lowsail",
            "ledger-clerk",
            "order",
            "wanted",
            "saved-worker",
        ],
        71,
    );
    let s = prepare_from_return(run(s, &c, HOLD), &c);
    let s = act(act(s, &c, TEST), &c, REPORT);
    let s = act(s, &c, "fume_yards.share_rescue_account");
    let memory = s.world.npcs[B].memories["fume_yards.rescue_account_heard"].clone();
    let s = act(act(s, &c, SHIFT), &c, BBACK);
    assert_eq!(
        s.world.npcs[B].memories["fume_yards.rescue_account_heard"],
        memory
    );
    absent(
        &s,
        &c,
        &[
            "fume_yards.fit_dust_filter",
            "fume_yards.assign_brann_salvage",
        ],
    );
    let text = c.observe(&s).unwrap().text;
    assert!(
        text.contains("Brann needed dust protection before freight work closed"),
        "{text}"
    );
    assert!(!text.contains("needs an installed dust filter"), "{text}");
    assert!(!flag(&s, A, "fume_yards.salvage_assignment_spent"));
}

#[test]
fn brann_return_preserves_unpaid_intact_and_broken_access_after_actual_walkaway() {
    let c = content();
    for broken in [false, true] {
        let mut s = if broken {
            let base = custom(
                &c,
                &[
                    "fenborn",
                    "lowsail",
                    "ledger-clerk",
                    "order",
                    "wanted",
                    "stole-permit",
                ],
                123,
            );
            let s = prepare_from_return(run(base, &c, HOLD), &c);
            let s = act(s, &c, TEST);
            let s = act(s, &c, REPORT);
            act(s, &c, SHIFT)
        } else {
            delegated(&c)
        };
        s = travel(s, &c, K);
        s = act(s, &c, "fume_yards.enter_ash_hatch");
        if broken {
            s = act(s, &c, "fume_yards.pull_rack_filter");
            assert_eq!(s.character.inventory["fume_yards.shard"], 1);
        } else {
            s = act(s, &c, "fume_yards.brace_rack");
            s = act(s, &c, "fume_yards.recover_braced_filter");
        }
        absent(&s, &c, &["fume_yards.report_with_daro"]);
        assert!(
            c.observe(&s).unwrap().text.contains("return Brann")
                || c.observe(&s).unwrap().text.contains("Return Brann")
        );
        let coin = s.character.resources["coin"];
        s = travel(s, &c, W);
        s = act(s, &c, BBACK);
        s = act(s, &c, "fume_yards.enter_ash_hatch");
        s = act(s, &c, "fume_yards.report_with_daro");
        assert_eq!(s.character.resources["coin"], coin + 1);
        assert_eq!(s.world.npcs[B].location, K);
        assert_eq!(s.world.npcs[D].location, K);
        s = act(s, &c, "fume_yards.enter_ash_hatch");
        absent(&s, &c, &["fume_yards.report_with_daro"]);
    }
}

fn water(s: GameState, c: &CompiledContent) -> GameState {
    run(
        s,
        c,
        &[
            ("world.enter_aftermath", None),
            ("return.patch_stand", None),
            ("return.order_water_stand", None),
            ("return.visit_workshop", None),
            ("travel_adjacent", Some(A)),
            ("fume_yards.buy_collateral_filter", None),
            ("travel_adjacent", Some(W)),
            ("travel_adjacent", Some(K)),
            ("fume_yards.take_market_cask", None),
            ("fume_yards.escort_market_cask", None),
            ("return.fit_market_filter", None),
            ("return.install_market_cask", None),
            ("return.draw_clean_water", None),
        ],
    )
}
#[test]
fn matched_water_composition_preserves_stock_and_returnable_brann_on_revisit() {
    let c = content();
    for (base, stamina) in [(delegated(&c), 5), (personal(&c), 3)] {
        let brann = base.world.npcs[B].location.clone();
        let s = water(base, &c);
        assert_eq!(
            (
                s.world.time,
                s.character.resources["coin"],
                s.character.resources["stamina"]
            ),
            (30, 9, stamina)
        );
        assert_eq!(s.character.inventory, BTreeMap::from([("rope".into(), 1)]));
        assert!(s.world.npcs[N].inventory.is_empty());
        assert!(s.world.npcs[P].inventory.is_empty());
        assert!(s.world.storages[CAGE].inventory.is_empty());
        assert_eq!(s.world.npcs[D].inventory[FILTER], 1);
        assert_eq!(s.world.npcs[B].inventory[FUEL], 1);
        assert_eq!(s.world.npcs[B].location, brann);
        assert_eq!(s.world.npcs[P].knowledge["fume_yards.market_cask"].turn, 26);
        assert_eq!(
            s.world.npcs["oren_pell"].knowledge["fume_yards.market_cask"].turn,
            26
        );
        absent(&s, &c, &["return.draw_clean_water"]);
        let s = act(s, &c, "return.visit_workshop");
        if brann == W {
            assert!(catalog(&s, &c).contains(BBACK));
            let s = act(s, &c, BBACK);
            assert_eq!(s.world.npcs[B].location, K);
            absent(&s, &c, &[SHIFT, "fume_yards.load_kiln_freight"]);
        }
    }
}

#[test]
fn workshop_and_market_wages_are_separate_and_remain_once_only() {
    let c = content();
    let s = delegated(&c);
    let s = act(s, &c, "fume_yards.load_freight");
    assert_eq!(
        (
            s.character.resources["coin"],
            s.character.resources["stamina"]
        ),
        (15, 1)
    );
    absent(&s, &c, &["fume_yards.load_freight"]);
    let s = run(
        s,
        &c,
        &[
            ("world.enter_aftermath", None),
            ("return.patch_stand", None),
            ("return.sort_dry_goods", None),
        ],
    );
    assert_eq!(
        (
            s.character.resources["coin"],
            s.character.resources["stamina"]
        ),
        (18, 1)
    );
    absent(&s, &c, &["return.sort_dry_goods"]);
    assert_eq!(s.world.npcs[N].memories["fume_yards.freight_paid"].turn, 17);
}

const SPLIT: &[Spec] = &[
    ("checkpoint.show_charter", None),
    ("travel_adjacent", Some("lowsail.levee")),
    ("levee.authority_path", None),
    ("floor.read_harmonics", None),
    ("travel_adjacent", Some("red_sluice.top")),
    ("top.check_wheels", None),
    ("top.split_flow", None),
    ("world.enter_aftermath", None),
    ("return.share_water", None),
];
const RELIEF: &[Spec] = &[
    ("checkpoint.show_charter", None),
    ("travel_adjacent", Some("lowsail.docks")),
    ("docks.ring_warning", None),
    ("travel_adjacent", Some("lowsail.levee")),
    ("levee.relay_warning", None),
    ("levee.authority_path", None),
    ("floor.open_relief", None),
    ("travel_adjacent", Some("red_sluice.top")),
    ("top.divert_relief", None),
    ("world.enter_aftermath", None),
    ("return.move_inland", None),
];
const FERRY: &[Spec] = &[
    ("checkpoint.blend_workers", None),
    ("travel_adjacent", Some("lowsail.levee")),
    ("levee.culvert_path", None),
    ("travel_adjacent", Some("red_sluice.top")),
    ("top.break_toll", None),
    ("world.enter_aftermath", None),
    ("return.open_ferry", None),
];
const OVERLOAD: &[Spec] = &[
    ("checkpoint.blend_workers", None),
    ("travel_adjacent", Some("lowsail.levee")),
    ("levee.culvert_path", None),
    ("floor.force_wheel", None),
    ("travel_adjacent", Some("red_sluice.top")),
    ("top.overload", None),
    ("world.enter_aftermath", None),
    ("return.face_flood", None),
];

#[test]
fn delegated_work_preserves_all_old_outcomes_and_missed_deadline() {
    let c = content();
    let mut deadline = vec![("wait_tide", None); 16];
    deadline.extend([("world.enter_aftermath", None), ("return.face_flood", None)]);
    for (runner, path, ending) in [
        (false, HOLD, "ending_council"),
        (false, SPLIT, "ending_accord"),
        (false, RELIEF, "ending_relief"),
        (true, FERRY, "ending_freedom"),
        (true, OVERLOAD, "ending_disaster"),
        (false, deadline.as_slice(), "ending_disaster"),
    ] {
        let genesis = if runner {
            custom(
                &c,
                &[
                    "kilnborn",
                    "red-sluice",
                    "lock-runner",
                    "freedom",
                    "wanted",
                    "saved-worker",
                ],
                71,
            )
        } else {
            start(&c)
        };
        let before = run(genesis, &c, path);
        assert!(before.world.flags.contains(ending));
        let s = prepare_from_return(before.clone(), &c);
        let s = act(s, &c, TEST);
        let s = act(s, &c, REPORT);
        let s = act(s, &c, SHIFT);
        let s = act(s, &c, BBACK);
        assert_eq!(s.world.time, before.world.time + 11);
        assert_eq!(s.world.flags, before.world.flags);
        assert_eq!(s.world.locations[L].flags, before.world.locations[L].flags);
        assert_eq!(
            s.character.resources["stamina"],
            before.character.resources["stamina"]
        );
        assert_eq!(
            s.character.resources["coin"],
            before.character.resources["coin"] + 3
        );
        for npc in [
            "sava_rusk",
            "oren_pell",
            "mira_kett",
            "edrik_voss",
            "yara_dene",
            D,
            P,
        ] {
            assert_eq!(s.world.npcs[npc], before.world.npcs[npc]);
        }
        assert_eq!(s.world.npcs[N].location, W);
        assert_eq!(s.world.npcs[B].location, K);
        let s = act(s, &c, "world.enter_aftermath");
        assert_eq!(s.world.flags, before.world.flags);
    }
}

#[test]
fn unresolved_surge_resolves_after_three_tick_work_at_the_actual_destination() {
    let c = content();
    let s = run(
        start(&c),
        &c,
        &[
            ("checkpoint.show_charter", None),
            ("travel_adjacent", Some("lowsail.levee")),
            ("travel_adjacent", Some(W)),
            ("fume_yards.take_stock", None),
            ("travel_adjacent", Some(K)),
            ("fume_yards.prepare_charge", None),
            ("travel_adjacent", Some(W)),
            (TEST, None),
            (REPORT, None),
        ],
    );
    let s = run(s, &c, &[("wait_tide", None); 5]);
    assert_eq!(s.world.time, 14);
    let s = act(s, &c, SHIFT);
    assert_eq!(s.world.time, 17);
    assert_eq!(s.world.current_location, W);
    assert_eq!(surge(&s), vec![(17, true)]);
    assert!(s.world.flags.contains("surge_missed"));
    assert!(s.world.flags.contains("sluice_failure"));
    assert!(flag(&s, L, "market_flooded"));
    assert_eq!(s.character.inventory[REPAIR], 1);
}

#[test]
fn first_test_after_real_128_step_old_world_traversal_keeps_finite_stock_and_source_time() {
    let c = content();
    let mut s = run(start(&c), &c, HOLD);
    while s.world.time < 127 {
        s = run(
            s,
            &c,
            &[
                ("travel_adjacent", Some("lowsail.docks")),
                ("travel_adjacent", Some("lowsail_market")),
                ("travel_adjacent", Some("lowsail.levee")),
                ("world.enter_aftermath", None),
            ],
        );
    }
    s = run(s, &c, &[("wait_tide", None); 2]);
    assert_eq!(s.world.time, 129);
    assert!(!s.world.npcs[N].knows(FACT));
    let s = prepare_from_return(s, &c);
    let s = act(s, &c, TEST);
    let s = act(s, &c, REPORT);
    let s = act(s, &c, SHIFT);
    let s = act(s, &c, BBACK);
    assert_eq!(s.world.time, 140);
    assert_eq!(s.world.npcs[N].knowledge[FACT].turn, 134);
    assert_eq!(s.world.npcs[B].knowledge[FACT].turn, 135);
    assert_eq!(
        s.world.npcs[B].memories["fume_yards.crew_charge_reclaimed"].turn,
        136
    );
    assert_eq!(surge(&s), vec![(16, false)]);
    assert_eq!(s.world.npcs[D].inventory[FILTER], 1);
    assert_eq!(s.world.storages[CAGE].inventory[FILTER], 1);
}

#[test]
fn prior_access_clearing_and_payment_do_not_prevent_branns_physical_return() {
    let c = content();
    for already_paid in [false, true] {
        let s = relayed(&c);
        let s = run(
            s,
            &c,
            &[
                ("fume_yards.enter_ash_hatch", None),
                ("fume_yards.brace_rack", None),
                ("fume_yards.recover_braced_filter", None),
            ],
        );
        let s = if already_paid {
            act(s, &c, "fume_yards.report_with_daro")
        } else {
            act(s, &c, "fume_yards.leave_ash_hatch")
        };
        let mut s = act(s, &c, SHIFT);
        let coin = s.character.resources["coin"];
        if already_paid {
            assert_eq!(s.world.time, 21);
            s = travel(s, &c, K);
            assert_eq!(s.world.time, 22);
            assert_eq!(s.world.npcs[B].location, W);
            assert!(flag(&s, A, "fume_yards.report_paid"));
            let text = c.observe(&s).unwrap().text;
            assert!(!text.contains("unpaid rack reports"), "{text}");
            assert!(text.contains("Brann waits at Workshop"), "{text}");
            s = travel(s, &c, W);
        }
        let s = act(s, &c, BBACK);
        if already_paid {
            assert_eq!(s.world.time, 24);
        }
        assert_eq!(s.world.npcs[B].location, K);
        assert_eq!(s.character.resources["coin"], coin);
        if already_paid {
            assert_eq!(s.world.npcs[D].location, K);
            let s = act(s, &c, "fume_yards.enter_ash_hatch");
            absent(&s, &c, &["fume_yards.report_with_daro"]);
        } else {
            let s = act(s, &c, "fume_yards.enter_ash_hatch");
            let s = act(s, &c, "fume_yards.report_with_daro");
            assert_eq!(s.character.resources["coin"], coin + 1);
        }
    }
}
