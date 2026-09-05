use forge_content::parse_and_compile_production;
use forge_kernel::{CompiledContent, EventKind, enumerate_legal_actions};
use forge_replay::Session;
use std::collections::BTreeMap;

const SOURCE: &str = include_str!("../../../content/split-tide.json");
const MARKER: &str = "salvage chance ignored its 75 percent boundary";
const FILTER: &str = "fume_yards.filter";
const SHARD: &str = "fume_yards.shard";

fn step(
    session: &mut Session<'_>,
    content: &CompiledContent,
    definition: &str,
    destination: Option<&str>,
) {
    let parameters = destination
        .map(|location| BTreeMap::from([("destination".to_owned(), location.to_owned())]))
        .unwrap_or_default();
    let action = enumerate_legal_actions(session.state(), content)
        .unwrap()
        .into_iter()
        .find(|action| action.definition_id == definition && action.parameters == parameters)
        .unwrap_or_else(|| panic!("canonical salvage preparation omitted {definition}"));
    session
        .record(&action)
        .unwrap_or_else(|error| panic!("canonical salvage preparation failed: {error}"));
}

#[test]
fn production_salvage_chance_respects_strict_75_percent_boundary() {
    let content = parse_and_compile_production(SOURCE).unwrap();
    // Reviewed first-draw answers, deliberately straddling the strict `<75`
    // test. The expected branch is never calculated using the kernel result.
    for (seed, value, filters, shards) in [
        (27, 10_902_710_238_276_814_474, 1, 0),
        (123, 13_032_462_758_197_477_675, 0, 1),
    ] {
        let mut session = Session::new_game("ilyan", seed, &content).unwrap();
        for (definition, destination) in [
            ("checkpoint.show_charter", None),
            ("travel_adjacent", Some("lowsail.levee")),
            ("levee.authority_path", None),
            ("travel_adjacent", Some("red_sluice.top")),
            ("top.hold_market", None),
            ("world.enter_aftermath", None),
            ("return.count_dry_stalls", None),
            ("return.visit_workshop", None),
            ("travel_adjacent", Some("fume_yards.kiln_bay")),
            ("fume_yards.enter_ash_hatch", None),
        ] {
            step(&mut session, &content, definition, destination);
        }
        assert_eq!(session.state().entropy.cursor, 0);
        assert_eq!(session.state().world.time, 10);
        assert_eq!(
            session.state().world.npcs["fume_yards.daro_venn"]
                .inventory
                .get(FILTER),
            Some(&1)
        );
        let action = enumerate_legal_actions(session.state(), &content)
            .unwrap()
            .into_iter()
            .find(|action| {
                action.definition_id == "fume_yards.pull_rack_filter"
                    && action.parameters.is_empty()
            })
            .expect("canonical 75 percent recovery is legal before the roll");
        let recorded = session
            .record(&action)
            .unwrap_or_else(|error| panic!("{MARKER}: {error}"));
        assert_eq!(
            recorded.entropy_draws.len(),
            1,
            "production salvage draw count changed"
        );
        assert_eq!(
            recorded.entropy_draws[0].value, value,
            "production salvage draw value changed"
        );
        assert_eq!(recorded.entropy_before.cursor, 0);
        assert_eq!(recorded.entropy_after.cursor, 1);
        assert_eq!(recorded.entropy_after.seed, seed);
        let random: Vec<_> = recorded
            .events
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::RandomDraw {
                    algorithm,
                    cursor,
                    value,
                } => Some((event.turn, algorithm.as_str(), *cursor, *value)),
                _ => None,
            })
            .collect();
        assert_eq!(random, vec![(10, "splitmix64-v1", 0, value)]);
        assert_eq!(
            (
                session
                    .state()
                    .character
                    .inventory
                    .get(FILTER)
                    .copied()
                    .unwrap_or(0),
                session
                    .state()
                    .character
                    .inventory
                    .get(SHARD)
                    .copied()
                    .unwrap_or(0),
            ),
            (filters, shards),
            "{MARKER}"
        );
        assert!(
            !session.state().world.npcs["fume_yards.daro_venn"]
                .inventory
                .contains_key(FILTER),
            "{MARKER}"
        );
        let breaks: Vec<_> = recorded
            .events
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::RecipeApplied {
                    recipe,
                    inputs,
                    outputs,
                } => Some((event.turn, recipe.as_str(), inputs.clone(), outputs.clone())),
                _ => None,
            })
            .collect();
        let expected = if shards == 0 {
            Vec::new()
        } else {
            vec![(
                10,
                "fume_yards.break_filter",
                BTreeMap::from([(FILTER.to_owned(), 1)]),
                BTreeMap::from([(SHARD.to_owned(), 1)]),
            )]
        };
        assert_eq!(breaks, expected, "{MARKER}");
    }
}
