use crate::{CrawlBudget, CrawlReport, VerifyError};
use forge_kernel::CompiledContent;
use serde::Serialize;
use std::collections::BTreeSet;

// Reviewed pre-expansion identities from accepted build 2105d1f0. New content
// may add to the graph/catalog but cannot erase any of this coverage.
const SPLIT_TIDE_ACTIONS: &[&str] = &[
    "checkpoint.ask_sava",
    "checkpoint.audit_order",
    "checkpoint.blend_workers",
    "checkpoint.pressure_guard",
    "checkpoint.read_flag",
    "checkpoint.recall_worker",
    "checkpoint.show_charter",
    "checkpoint.use_stolen_permit",
    "docks.ask_oren",
    "docks.audit_ledger",
    "docks.follow_yara",
    "docks.press_yara",
    "docks.rig_towline",
    "docks.ring_warning",
    "docks.search_crate",
    "docks.trade_warning",
    "floor.ack_report",
    "floor.climb_hot_face",
    "floor.dive_intake",
    "floor.force_wheel",
    "floor.key_calibration",
    "floor.open_relief",
    "floor.read_harmonics",
    "floor.stabilize_gauge",
    "floor.test_pressure",
    "levee.authority_path",
    "levee.culvert_path",
    "levee.help_worker",
    "levee.inspect_damage",
    "levee.relay_warning",
    "levee.send_report",
    "levee.stolen_path",
    "return.acknowledge_report",
    "return.ask_oren",
    "return.count_dry_stalls",
    "return.face_flood",
    "return.move_inland",
    "return.open_ferry",
    "return.read_tide",
    "return.share_water",
    "top.break_toll",
    "top.check_wheels",
    "top.divert_relief",
    "top.hold_market",
    "top.overload",
    "top.rescue_worker",
    "top.signal_market",
    "top.split_flow",
    "travel_adjacent",
    "wait_tide",
    "world.enter_aftermath",
];
const SPLIT_TIDE_LOCATIONS: &[&str] = &[
    "lowsail.docks",
    "lowsail.levee",
    "lowsail.return",
    "lowsail_market",
    "red_sluice.floor",
    "red_sluice.top",
];
pub(super) const PILOT_ACTIONS: &[&str] = &[
    "fume_yards.take_stock",
    "fume_yards.press_repair_plugs",
    "fume_yards.pack_catch_screen",
    "fume_yards.fit_catch_screen",
    "fume_yards.load_freight",
    "fume_yards.load_screened_freight",
    "return.patch_stand",
    "return.sort_dry_goods",
    "return.visit_workshop",
];
pub(super) const BATCHWORKS_ACTIONS: &[&str] = &[
    "fume_yards.take_fuel",
    "fume_yards.take_cask",
    "fume_yards.prepare_charge",
    "fume_yards.fit_wet_screen",
    "fume_yards.ignite_batch",
    "fume_yards.draw_filter",
    "fume_yards.bank_kiln",
    "fume_yards.reclaim_charge",
    "fume_yards.fit_dust_filter",
    "fume_yards.load_kiln_freight",
    "fume_yards.load_filtered_kiln_freight",
    "fume_yards.inspect_spoiled_batch",
    "return.sell_filter",
];
pub(super) const SALVAGE_ACTIONS: &[&str] = &[
    "fume_yards.enter_ash_hatch",
    "fume_yards.leave_ash_hatch",
    "fume_yards.brace_rack",
    "fume_yards.recover_braced_filter",
    "fume_yards.thread_rack_filter",
    "fume_yards.pull_rack_filter",
    "fume_yards.report_with_daro",
    "fume_yards.load_cold_freight",
];

pub(super) const MARKET_WATER_ACTIONS: &[&str] = &[
    "fume_yards.read_collateral_docket",
    "fume_yards.buy_collateral_filter",
    "fume_yards.settle_collateral_fuel",
    "fume_yards.return_to_cage",
    "return.order_water_stand",
    "return.fit_market_filter",
    "fume_yards.take_market_cask",
    "fume_yards.escort_market_cask",
    "return.install_market_cask",
    "return.draw_clean_water",
];

pub(super) const STAFFING_ACTIONS: &[&str] = &[
    "fume_yards.share_rescue_account",
    "fume_yards.assign_brann_salvage",
    "fume_yards.recover_staffed_filter",
    "fume_yards.return_with_brann",
];

// Declared in cycle 35 before the first trial. Depth counts canonical actions;
// the three-tick lift remains one action, and the reviewed H7 prefix costs seven.
pub(super) const STAFFING_BUDGET: CrawlBudget = CrawlBudget {
    max_depth: 20,
    max_expanded_states: 96,
    max_discovered_frontiers: 768,
    max_action_executions: 2048,
    catalog_page_size: 7,
};

// Declared in cycle 34 before the first trial. Previous components retain
// their exact limits; the reviewed H7 prefix still consumes seven steps.
pub(super) const MARKET_WATER_BUDGET: CrawlBudget = CrawlBudget {
    max_depth: 35,
    max_expanded_states: 128,
    max_discovered_frontiers: 1024,
    max_action_executions: 4096,
    catalog_page_size: 7,
};

pub(super) const BATCHWORKS_BUDGET: CrawlBudget = CrawlBudget {
    max_depth: 20,
    max_expanded_states: 128,
    max_discovered_frontiers: 768,
    max_action_executions: 2048,
    catalog_page_size: 7,
};
// Declared before the first salvage run. This adds optional coverage without
// enlarging any previous component's depth, state, frontier, or action limits.
pub(super) const SALVAGE_BUDGET: CrawlBudget = CrawlBudget {
    max_depth: 20,
    max_expanded_states: 96,
    max_discovered_frontiers: 768,
    max_action_executions: 2048,
    catalog_page_size: 7,
};

fn ids(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|id| (*id).to_owned()).collect()
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct SplitTideProjection {
    pub required_definitions: BTreeSet<String>,
    pub covered_definitions: BTreeSet<String>,
    pub required_locations: BTreeSet<String>,
    pub reached_locations: BTreeSet<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct RegressionCrawlReport {
    #[serde(flatten)]
    pub crawl: CrawlReport,
    pub split_tide_projection: SplitTideProjection,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
/// Combined coverage from independent crawls. The legacy regression keeps
/// its original ceilings; batch work, salvage, market water, and staffing have explicit
/// budgets. Components do not claim exhaustive reachable-state coverage or
/// admission of a complete new area.
pub struct ProductionCrawlReport {
    pub build_id: String,
    pub verifier_id: String,
    pub advertised_definitions: BTreeSet<String>,
    pub covered_definitions: BTreeSet<String>,
    pub reached_locations: BTreeSet<String>,
    pub regression: RegressionCrawlReport,
    pub batchworks: CrawlReport,
    pub salvage: CrawlReport,
    pub market_water: CrawlReport,
    pub staffing: CrawlReport,
}

impl ProductionCrawlReport {
    pub fn to_pretty_json(&self) -> Result<String, VerifyError> {
        serde_json::to_string_pretty(self)
            .map(|json| json + "\n")
            .map_err(|_| VerifyError::new("could not serialize production crawl report"))
    }
}

pub(super) fn preserve_split_tide(
    crawl: CrawlReport,
) -> Result<RegressionCrawlReport, VerifyError> {
    let required_definitions = ids(SPLIT_TIDE_ACTIONS);
    let required_locations = ids(SPLIT_TIDE_LOCATIONS);
    let covered_definitions = crawl
        .covered_definitions
        .intersection(&required_definitions)
        .cloned()
        .collect();
    let reached_locations = crawl
        .reached_locations
        .intersection(&required_locations)
        .cloned()
        .collect();
    if covered_definitions != required_definitions || reached_locations != required_locations {
        return Err(VerifyError::new(
            "production crawl lost Split Tide regression coverage",
        ));
    }
    Ok(RegressionCrawlReport {
        crawl,
        split_tide_projection: SplitTideProjection {
            required_definitions,
            covered_definitions,
            required_locations,
            reached_locations,
        },
    })
}

fn regression_definitions() -> BTreeSet<String> {
    ids(SPLIT_TIDE_ACTIONS)
        .union(&ids(PILOT_ACTIONS))
        .cloned()
        .collect()
}

fn validate_expansion_catalog(content: &CompiledContent) -> Result<BTreeSet<String>, VerifyError> {
    let authored = content
        .actions()
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    let mut reviewed = regression_definitions();
    reviewed.extend(ids(BATCHWORKS_ACTIONS));
    reviewed.extend(ids(SALVAGE_ACTIONS));
    reviewed.extend(ids(MARKET_WATER_ACTIONS));
    reviewed.extend(ids(STAFFING_ACTIONS));
    if authored != reviewed {
        return Err(VerifyError::new(
            "expansion crawl action contract contains unreviewed additions or omissions",
        ));
    }
    Ok(authored)
}

pub(super) fn crawl_regression(
    content: &CompiledContent,
    budget: CrawlBudget,
) -> Result<RegressionCrawlReport, VerifyError> {
    validate_expansion_catalog(content)?;
    preserve_split_tide(crate::crawler::crawl_targets(
        content,
        budget,
        regression_definitions(),
    )?)
}

pub(super) fn crawl_all(content: &CompiledContent) -> Result<ProductionCrawlReport, VerifyError> {
    let regression = crawl_regression(content, CrawlBudget::default())?;
    let batchworks = crawl_batchworks(content)?;
    let salvage = crawl_salvage(content)?;
    let market_water = crawl_market_water_production(content)?;
    let staffing = crawl_staffing_production(content)?;
    combine_crawls(
        content,
        regression,
        batchworks,
        salvage,
        market_water,
        staffing,
    )
}

fn combine_crawls(
    content: &CompiledContent,
    regression: RegressionCrawlReport,
    batchworks: CrawlReport,
    salvage: CrawlReport,
    market_water: CrawlReport,
    staffing: CrawlReport,
) -> Result<ProductionCrawlReport, VerifyError> {
    let advertised_definitions = validate_expansion_catalog(content)?;
    for report in [
        &regression.crawl,
        &batchworks,
        &salvage,
        &market_water,
        &staffing,
    ] {
        if report.build_id != content.build_id()
            || report.verifier_id != crate::VERIFIER_ID
            || report.advertised_definitions != advertised_definitions
            || !report.is_complete()
        {
            return Err(VerifyError::new(
                "combined crawl has inconsistent identity or coverage",
            ));
        }
    }
    if regression.crawl.required_definitions != regression_definitions()
        || regression.crawl.budget != CrawlBudget::default()
        || batchworks.required_definitions != ids(BATCHWORKS_ACTIONS)
        || batchworks.budget != BATCHWORKS_BUDGET
        || salvage.required_definitions != ids(SALVAGE_ACTIONS)
        || salvage.budget != SALVAGE_BUDGET
    {
        return Err(VerifyError::new(
            "combined crawl component scope or budget differs from contract",
        ));
    }
    check_market_water_report(content, &market_water)?;
    check_staffing_report(content, &staffing)?;
    // Each established component retains its own consequence locations even
    // when a newer component happens to reach the same places.
    if ![
        "fume_yards.kiln_bay",
        "fume_yards.workshop",
        "lowsail.return",
    ]
    .iter()
    .all(|id| batchworks.reached_locations.contains(*id))
        || ![
            "fume_yards.ash_beds",
            "fume_yards.kiln_bay",
            "fume_yards.workshop",
        ]
        .iter()
        .all(|id| salvage.reached_locations.contains(*id))
    {
        return Err(VerifyError::new(
            "combined crawl lost a component consequence location",
        ));
    }
    let covered_definitions = [
        &regression.crawl,
        &batchworks,
        &salvage,
        &market_water,
        &staffing,
    ]
    .into_iter()
    .flat_map(|report| &report.covered_definitions)
    .cloned()
    .collect();
    let reached_locations = [
        &regression.crawl,
        &batchworks,
        &salvage,
        &market_water,
        &staffing,
    ]
    .into_iter()
    .flat_map(|report| &report.reached_locations)
    .cloned()
    .collect();
    let locations: BTreeSet<_> = content.locations().map(|(id, _)| id.clone()).collect();
    if covered_definitions != advertised_definitions || reached_locations != locations {
        return Err(VerifyError::new(
            "combined crawl omitted an authored definition or location",
        ));
    }
    Ok(ProductionCrawlReport {
        build_id: content.build_id().to_owned(),
        verifier_id: crate::VERIFIER_ID.to_owned(),
        advertised_definitions,
        covered_definitions,
        reached_locations,
        regression,
        batchworks,
        salvage,
        market_water,
        staffing,
    })
}

pub(super) fn crawl_pilot(content: &CompiledContent) -> Result<CrawlReport, VerifyError> {
    validate_expansion_catalog(content)?;
    let required = ids(PILOT_ACTIONS);
    // Independent optional coverage budget, fixed before running the pilot.
    // Thirteen steps reach the quickest resolved-tide craft/delivery path.
    let budget = CrawlBudget {
        max_depth: 13,
        max_expanded_states: 96,
        max_discovered_frontiers: 512,
        max_action_executions: 1024,
        catalog_page_size: 7,
    };
    // The unchanged Hold Market witness supplies a resolved-tide frontier.
    // Its canonical seven-action prefix remains part of the depth budget;
    // both ordinary preset starts remain available. Only expanded catalogs
    // count as coverage.
    let report = crate::crawler::crawl_targets_with_scenarios(
        content,
        budget,
        required,
        &["m1-outcome-hold-market"],
    )?;
    if !["fume_yards.workshop", "lowsail.return"]
        .iter()
        .all(|id| report.reached_locations.contains(*id))
    {
        return Err(VerifyError::new(
            "optional crawl missed a pilot consequence location",
        ));
    }
    Ok(report)
}

pub(super) fn crawl_batchworks(content: &CompiledContent) -> Result<CrawlReport, VerifyError> {
    validate_expansion_catalog(content)?;
    // This separately budgeted extension includes the reviewed seven-action
    // aftermath prefix and both ordinary presets. Catalog execution is whole.
    let report = crate::crawler::crawl_targets_with_scenarios(
        content,
        BATCHWORKS_BUDGET,
        ids(BATCHWORKS_ACTIONS),
        &["m1-outcome-hold-market"],
    )?;
    if ![
        "fume_yards.kiln_bay",
        "fume_yards.workshop",
        "lowsail.return",
    ]
    .iter()
    .all(|id| report.reached_locations.contains(*id))
    {
        return Err(VerifyError::new(
            "batchworks crawl missed a consequence location",
        ));
    }
    Ok(report)
}

pub(super) fn crawl_salvage(content: &CompiledContent) -> Result<CrawlReport, VerifyError> {
    validate_expansion_catalog(content)?;
    let report = crate::crawler::crawl_targets_with_scenarios(
        content,
        SALVAGE_BUDGET,
        ids(SALVAGE_ACTIONS),
        &["m1-outcome-hold-market"],
    )?;
    if ![
        "fume_yards.ash_beds",
        "fume_yards.kiln_bay",
        "fume_yards.workshop",
    ]
    .iter()
    .all(|id| report.reached_locations.contains(*id))
    {
        return Err(VerifyError::new(
            "salvage crawl missed a consequence location",
        ));
    }
    Ok(report)
}

pub(super) fn crawl_market_water_production(
    content: &CompiledContent,
) -> Result<CrawlReport, VerifyError> {
    validate_expansion_catalog(content)?;
    let report = crate::crawler::crawl_targets_with_scenarios(
        content,
        MARKET_WATER_BUDGET,
        ids(MARKET_WATER_ACTIONS),
        &["m1-outcome-hold-market"],
    )?;
    check_market_water_report(content, &report)?;
    Ok(report)
}

pub(super) fn crawl_staffing_production(
    content: &CompiledContent,
) -> Result<CrawlReport, VerifyError> {
    validate_expansion_catalog(content)?;
    let report = crate::crawler::crawl_targets_with_scenarios(
        content,
        STAFFING_BUDGET,
        ids(STAFFING_ACTIONS),
        &["m1-outcome-hold-market"],
    )?;
    check_staffing_report(content, &report)?;
    Ok(report)
}

fn reviewed_aftermath_starting_sessions(
    content: &CompiledContent,
) -> Result<Vec<crate::crawler::CrawlStartingSession>, VerifyError> {
    let mut starts = Vec::new();
    for preset in ["ilyan", "rook"] {
        let session =
            forge_replay::Session::new_game(preset, 71, content).map_err(crate::replay_error)?;
        starts.push(crate::crawler::CrawlStartingSession {
            label: format!("preset:{preset}"),
            start: session.trace().start.clone(),
            depth: 0,
            final_receipt: session.trace().final_receipt.clone(),
            state_id: session.state().state_id(),
        });
    }
    let hold = crate::scenarios::run(crate::scenarios::get("m1-outcome-hold-market")?, content)?;
    if hold.trace().steps.len() != 7 {
        return Err(VerifyError::new(
            "optional crawl requires the unchanged seven-step Hold Market prefix",
        ));
    }
    starts.push(crate::crawler::CrawlStartingSession {
        label: "scenario:m1-outcome-hold-market".to_owned(),
        start: hold.trace().start.clone(),
        depth: 7,
        final_receipt: hold.trace().final_receipt.clone(),
        state_id: hold.state().state_id(),
    });
    Ok(starts)
}

pub(super) fn check_market_water_report(
    content: &CompiledContent,
    report: &CrawlReport,
) -> Result<(), VerifyError> {
    let authored = validate_expansion_catalog(content)?;
    if report.build_id != content.build_id()
        || report.verifier_id != crate::VERIFIER_ID
        || report.advertised_definitions != authored
        || report.required_definitions != ids(MARKET_WATER_ACTIONS)
        || !report.covered_definitions.is_subset(&authored)
        || !report.is_complete()
        || report.budget != MARKET_WATER_BUDGET
        || report.expanded_states > MARKET_WATER_BUDGET.max_expanded_states
        || report.discovered_frontiers > MARKET_WATER_BUDGET.max_discovered_frontiers
        || report.successful_actions > MARKET_WATER_BUDGET.max_action_executions
    {
        return Err(VerifyError::new(
            "market-water crawl identity, coverage, or fixed budget differs from contract",
        ));
    }
    let locations: BTreeSet<_> = content.locations().map(|(id, _)| id.clone()).collect();
    if !report.reached_locations.is_subset(&locations)
        || ![
            "fume_yards.ash_beds",
            "fume_yards.kiln_bay",
            "fume_yards.workshop",
            "lowsail.return",
        ]
        .iter()
        .all(|id| report.reached_locations.contains(*id))
    {
        return Err(VerifyError::new(
            "market-water crawl missed a consequence location or added an unknown location",
        ));
    }
    if report.starting_sessions != reviewed_aftermath_starting_sessions(content)? {
        return Err(VerifyError::new(
            "market-water crawl starting sessions differ from canonical preset and H7 lineage",
        ));
    }
    Ok(())
}

pub(super) fn check_staffing_report(
    content: &CompiledContent,
    report: &CrawlReport,
) -> Result<(), VerifyError> {
    let authored = validate_expansion_catalog(content)?;
    if report.build_id != content.build_id()
        || report.verifier_id != crate::VERIFIER_ID
        || report.advertised_definitions != authored
        || report.required_definitions != ids(STAFFING_ACTIONS)
        || !report.covered_definitions.is_subset(&authored)
        || !report.is_complete()
        || report.budget != STAFFING_BUDGET
        || report.expanded_states > STAFFING_BUDGET.max_expanded_states
        || report.discovered_frontiers > STAFFING_BUDGET.max_discovered_frontiers
        || report.successful_actions > STAFFING_BUDGET.max_action_executions
    {
        return Err(VerifyError::new(
            "staffing crawl identity, coverage, or fixed budget differs from contract",
        ));
    }
    let locations: BTreeSet<_> = content.locations().map(|(id, _)| id.clone()).collect();
    if !report.reached_locations.is_subset(&locations)
        || ![
            "fume_yards.ash_beds",
            "fume_yards.kiln_bay",
            "fume_yards.workshop",
        ]
        .iter()
        .all(|id| report.reached_locations.contains(*id))
    {
        return Err(VerifyError::new(
            "staffing crawl missed a consequence location or added an unknown location",
        ));
    }
    if report.starting_sessions != reviewed_aftermath_starting_sessions(content)? {
        return Err(VerifyError::new(
            "staffing crawl starting sessions differ from canonical preset and H7 lineage",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = include_str!("../../../content/split-tide.json");

    #[test]
    fn batchworks_crawl_covers_all_thirteen_targets_with_its_separate_fixed_budget() {
        let content = forge_content::parse_and_compile_production(SOURCE).unwrap();
        let report = crawl_batchworks(&content).unwrap();
        assert_eq!(report.budget, BATCHWORKS_BUDGET);
        assert_eq!(report.required_definitions, ids(BATCHWORKS_ACTIONS));
        assert_eq!(report.required_definitions.len(), 13);
        assert_eq!(report.advertised_definitions.len(), 95);
        assert!(report.is_complete());
        assert_eq!(report.starting_sessions.len(), 3);
        assert_eq!(
            report
                .starting_sessions
                .iter()
                .map(|start| start.depth)
                .collect::<Vec<_>>(),
            vec![0, 0, 7]
        );
        assert!(report.expanded_states <= 128);
        assert!(report.discovered_frontiers <= 768);
        assert!(report.successful_actions <= 2048);
        eprintln!(
            "batchworks: {} states, {} frontiers, {} actions",
            report.expanded_states, report.discovered_frontiers, report.successful_actions
        );
    }

    #[test]
    fn salvage_crawl_covers_eight_targets_under_its_predeclared_budget() {
        let content = forge_content::parse_and_compile_production(SOURCE).unwrap();
        let report = crawl_salvage(&content).unwrap();
        assert_eq!(report.budget, SALVAGE_BUDGET);
        assert_eq!(report.required_definitions, ids(SALVAGE_ACTIONS));
        assert_eq!(report.required_definitions.len(), 8);
        assert_eq!(report.advertised_definitions.len(), 95);
        assert!(report.is_complete());
        assert_eq!(report.starting_sessions.len(), 3);
        assert_eq!(
            report
                .starting_sessions
                .iter()
                .map(|start| start.depth)
                .collect::<Vec<_>>(),
            vec![0, 0, 7]
        );
        assert!(report.expanded_states <= 96);
        assert!(report.discovered_frontiers <= 768);
        assert!(report.successful_actions <= 2048);
        eprintln!(
            "salvage: {} states, {} frontiers, {} actions",
            report.expanded_states, report.discovered_frontiers, report.successful_actions
        );
    }

    #[test]
    fn market_water_crawl_covers_ten_targets_under_its_predeclared_budget() {
        let content = forge_content::parse_and_compile_production(SOURCE).unwrap();
        let report = crawl_market_water_production(&content).unwrap();
        assert_eq!(report.required_definitions, ids(MARKET_WATER_ACTIONS));
        assert_eq!(report.required_definitions.len(), 10);
        assert_eq!(report.advertised_definitions.len(), 95);
        assert_eq!(report.budget, MARKET_WATER_BUDGET);
        assert!(report.is_complete());
        assert_eq!(
            report.starting_sessions,
            reviewed_aftermath_starting_sessions(&content).unwrap()
        );
        assert_eq!(
            report
                .starting_sessions
                .iter()
                .map(|start| start.depth)
                .collect::<Vec<_>>(),
            vec![0, 0, 7]
        );
        assert!(report.expanded_states <= 128);
        assert!(report.discovered_frontiers <= 1024);
        assert!(report.successful_actions <= 4096);
        eprintln!(
            "market-water: {} states, {} frontiers, {} actions",
            report.expanded_states, report.discovered_frontiers, report.successful_actions
        );
    }

    #[test]
    fn staffing_crawl_covers_four_targets_under_its_predeclared_budget() {
        let content = forge_content::parse_and_compile_production(SOURCE).unwrap();
        let report = crawl_staffing_production(&content).unwrap();
        assert_eq!(report.required_definitions, ids(STAFFING_ACTIONS));
        assert_eq!(report.required_definitions.len(), 4);
        assert_eq!(report.advertised_definitions.len(), 95);
        assert_eq!(report.budget, STAFFING_BUDGET);
        assert!(report.is_complete());
        assert_eq!(
            report.starting_sessions,
            reviewed_aftermath_starting_sessions(&content).unwrap()
        );
        assert_eq!(
            report
                .starting_sessions
                .iter()
                .map(|start| start.depth)
                .collect::<Vec<_>>(),
            vec![0, 0, 7]
        );
        assert!(report.expanded_states <= 96);
        assert!(report.discovered_frontiers <= 768);
        assert!(report.successful_actions <= 2048);
        eprintln!(
            "staffing: {} states, {} frontiers, {} actions",
            report.expanded_states, report.discovered_frontiers, report.successful_actions
        );
    }

    fn declared_report(
        content: &CompiledContent,
        required: BTreeSet<String>,
        budget: CrawlBudget,
    ) -> CrawlReport {
        CrawlReport {
            verifier_id: crate::VERIFIER_ID.to_owned(),
            build_id: content.build_id().to_owned(),
            budget,
            expanded_states: 0,
            discovered_frontiers: 0,
            successful_actions: 0,
            max_legal_actions: 0,
            reached_locations: content.locations().map(|(id, _)| id.clone()).collect(),
            covered_definitions: required.clone(),
            required_definitions: required,
            advertised_definitions: content.actions().map(|(id, _)| id.clone()).collect(),
            starting_sessions: Vec::new(),
            execution_receipt: "test aggregation only".to_owned(),
        }
    }

    #[test]
    fn combined_scope_rejects_missing_coverage_identity_and_budget_substitution() {
        let content = forge_content::parse_and_compile_production(SOURCE).unwrap();
        let regression = preserve_split_tide(declared_report(
            &content,
            regression_definitions(),
            CrawlBudget::default(),
        ))
        .unwrap();
        let batchworks = declared_report(&content, ids(BATCHWORKS_ACTIONS), BATCHWORKS_BUDGET);
        let salvage = declared_report(&content, ids(SALVAGE_ACTIONS), SALVAGE_BUDGET);
        let mut market_water =
            declared_report(&content, ids(MARKET_WATER_ACTIONS), MARKET_WATER_BUDGET);
        market_water.starting_sessions = reviewed_aftermath_starting_sessions(&content).unwrap();
        let mut staffing = declared_report(&content, ids(STAFFING_ACTIONS), STAFFING_BUDGET);
        staffing.starting_sessions = reviewed_aftermath_starting_sessions(&content).unwrap();
        let combined = combine_crawls(
            &content,
            regression.clone(),
            batchworks.clone(),
            salvage.clone(),
            market_water.clone(),
            staffing.clone(),
        )
        .unwrap();
        assert_eq!(combined.advertised_definitions.len(), 95);
        assert_eq!(
            combined.covered_definitions,
            combined.advertised_definitions
        );
        assert_eq!(combined.reached_locations.len(), 9);
        for mutation in 0..6 {
            let mut changed = batchworks.clone();
            match mutation {
                0 => {
                    changed.covered_definitions.remove("return.sell_filter");
                }
                1 => changed.build_id.push('x'),
                2 => changed.verifier_id.push('x'),
                3 => {
                    changed.required_definitions.remove("return.sell_filter");
                }
                4 => changed.budget.max_depth += 1,
                _ => {
                    changed.advertised_definitions.remove("return.sell_filter");
                }
            }
            assert!(
                combine_crawls(
                    &content,
                    regression.clone(),
                    changed,
                    salvage.clone(),
                    market_water.clone(),
                    staffing.clone()
                )
                .is_err()
            );
        }
        let mut missing_location = regression.clone();
        missing_location
            .crawl
            .reached_locations
            .remove("fume_yards.kiln_bay");
        let mut missing_location_batch = batchworks.clone();
        missing_location_batch
            .reached_locations
            .remove("fume_yards.kiln_bay");
        let mut missing_location_salvage = salvage.clone();
        missing_location_salvage
            .reached_locations
            .remove("fume_yards.kiln_bay");
        assert!(
            combine_crawls(
                &content,
                missing_location,
                missing_location_batch,
                missing_location_salvage,
                market_water.clone(),
                staffing.clone()
            )
            .is_err()
        );
        let mut omitted = regression;
        omitted.crawl.covered_definitions.remove("top.split_flow");
        assert!(
            combine_crawls(
                &content,
                omitted,
                batchworks,
                salvage,
                market_water,
                staffing.clone()
            )
            .is_err()
        );
        let mut unreviewed = forge_content::parse(SOURCE).unwrap();
        let mut action = unreviewed.actions[0].clone();
        action.id = "test.unreviewed".to_owned();
        unreviewed.actions.push(action);
        let changed = forge_content::compile_production(unreviewed).unwrap();
        assert!(validate_expansion_catalog(&changed).is_err());
    }

    #[test]
    fn combined_salvage_component_rejects_missing_coverage_identity_scope_budget_and_location() {
        let content = forge_content::parse_and_compile_production(SOURCE).unwrap();
        let mut regression = preserve_split_tide(declared_report(
            &content,
            regression_definitions(),
            CrawlBudget::default(),
        ))
        .unwrap();
        let mut batchworks = declared_report(&content, ids(BATCHWORKS_ACTIONS), BATCHWORKS_BUDGET);
        let salvage = declared_report(&content, ids(SALVAGE_ACTIONS), SALVAGE_BUDGET);
        let mut market_water =
            declared_report(&content, ids(MARKET_WATER_ACTIONS), MARKET_WATER_BUDGET);
        market_water.starting_sessions = reviewed_aftermath_starting_sessions(&content).unwrap();
        let mut staffing = declared_report(&content, ids(STAFFING_ACTIONS), STAFFING_BUDGET);
        staffing.starting_sessions = reviewed_aftermath_starting_sessions(&content).unwrap();
        // The established salvage component must retain its own consequence
        // location even when newer synthetic components also cover it.
        regression
            .crawl
            .reached_locations
            .remove("fume_yards.ash_beds");
        batchworks.reached_locations.remove("fume_yards.ash_beds");
        combine_crawls(
            &content,
            regression.clone(),
            batchworks.clone(),
            salvage.clone(),
            market_water.clone(),
            staffing.clone(),
        )
        .unwrap();
        for mutation in 0..7 {
            let mut changed = salvage.clone();
            match mutation {
                0 => {
                    changed
                        .covered_definitions
                        .remove("fume_yards.pull_rack_filter");
                }
                1 => changed.build_id.push('x'),
                2 => changed.verifier_id.push('x'),
                3 => {
                    changed
                        .required_definitions
                        .remove("fume_yards.pull_rack_filter");
                }
                4 => changed.budget.max_expanded_states += 1,
                5 => {
                    changed
                        .advertised_definitions
                        .remove("fume_yards.pull_rack_filter");
                }
                _ => {
                    changed.reached_locations.remove("fume_yards.ash_beds");
                }
            }
            assert!(
                combine_crawls(
                    &content,
                    regression.clone(),
                    batchworks.clone(),
                    changed,
                    market_water.clone(),
                    staffing.clone()
                )
                .is_err(),
                "mutation {mutation} must not weaken combined coverage"
            );
        }
    }

    #[test]
    fn market_water_component_rejects_identity_scope_budget_location_and_seed_corruption() {
        let content = forge_content::parse_and_compile_production(SOURCE).unwrap();
        let regression = preserve_split_tide(declared_report(
            &content,
            regression_definitions(),
            CrawlBudget::default(),
        ))
        .unwrap();
        let batchworks = declared_report(&content, ids(BATCHWORKS_ACTIONS), BATCHWORKS_BUDGET);
        let salvage = declared_report(&content, ids(SALVAGE_ACTIONS), SALVAGE_BUDGET);
        let mut market_water =
            declared_report(&content, ids(MARKET_WATER_ACTIONS), MARKET_WATER_BUDGET);
        market_water.starting_sessions = reviewed_aftermath_starting_sessions(&content).unwrap();
        let mut staffing = declared_report(&content, ids(STAFFING_ACTIONS), STAFFING_BUDGET);
        staffing.starting_sessions = reviewed_aftermath_starting_sessions(&content).unwrap();
        check_market_water_report(&content, &market_water).unwrap();
        for mutation in [
            "coverage",
            "build",
            "verifier",
            "required",
            "advertised",
            "depth budget",
            "state budget",
            "frontier budget",
            "execution budget",
            "page budget",
            "states",
            "frontiers",
            "executions",
            "location",
            "unknown location",
            "unknown coverage",
            "missing seed",
            "seed label",
            "seed depth",
            "seed receipt",
            "seed state",
            "seed start",
            "seed order",
        ] {
            let mut changed = market_water.clone();
            match mutation {
                "coverage" => {
                    changed
                        .covered_definitions
                        .remove("return.draw_clean_water");
                }
                "build" => changed.build_id.push('x'),
                "verifier" => changed.verifier_id.push('x'),
                "required" => {
                    changed
                        .required_definitions
                        .remove("return.draw_clean_water");
                }
                "advertised" => {
                    changed
                        .advertised_definitions
                        .remove("return.draw_clean_water");
                }
                "depth budget" => changed.budget.max_depth += 1,
                "state budget" => changed.budget.max_expanded_states += 1,
                "frontier budget" => changed.budget.max_discovered_frontiers += 1,
                "execution budget" => changed.budget.max_action_executions += 1,
                "page budget" => changed.budget.catalog_page_size += 1,
                "states" => changed.expanded_states = 129,
                "frontiers" => changed.discovered_frontiers = 1025,
                "executions" => changed.successful_actions = 4097,
                "location" => {
                    changed.reached_locations.remove("lowsail.return");
                }
                "unknown location" => {
                    changed.reached_locations.insert("test.unknown".to_owned());
                }
                "unknown coverage" => {
                    changed
                        .covered_definitions
                        .insert("test.unknown".to_owned());
                }
                "missing seed" => {
                    changed.starting_sessions.pop();
                }
                "seed label" => changed.starting_sessions[2].label.push('x'),
                "seed depth" => changed.starting_sessions[2].depth = 0,
                "seed receipt" => changed.starting_sessions[2].final_receipt.push('x'),
                "seed state" => changed.starting_sessions[2].state_id.push('x'),
                "seed start" => {
                    changed.starting_sessions[2].start = changed.starting_sessions[1].start.clone()
                }
                "seed order" => changed.starting_sessions.swap(0, 2),
                _ => unreachable!(),
            }
            assert!(
                check_market_water_report(&content, &changed).is_err(),
                "accepted market-water {mutation}"
            );
            assert!(
                combine_crawls(
                    &content,
                    regression.clone(),
                    batchworks.clone(),
                    salvage.clone(),
                    changed,
                    staffing.clone()
                )
                .is_err(),
                "combined report accepted market-water {mutation}"
            );
        }
    }

    #[test]
    fn staffing_component_rejects_identity_scope_budget_location_and_seed_corruption() {
        let content = forge_content::parse_and_compile_production(SOURCE).unwrap();
        let regression = preserve_split_tide(declared_report(
            &content,
            regression_definitions(),
            CrawlBudget::default(),
        ))
        .unwrap();
        let batchworks = declared_report(&content, ids(BATCHWORKS_ACTIONS), BATCHWORKS_BUDGET);
        let salvage = declared_report(&content, ids(SALVAGE_ACTIONS), SALVAGE_BUDGET);
        let mut staffing = declared_report(&content, ids(STAFFING_ACTIONS), STAFFING_BUDGET);
        staffing.starting_sessions = reviewed_aftermath_starting_sessions(&content).unwrap();
        let mut market_water =
            declared_report(&content, ids(MARKET_WATER_ACTIONS), MARKET_WATER_BUDGET);
        market_water.starting_sessions = reviewed_aftermath_starting_sessions(&content).unwrap();
        check_staffing_report(&content, &staffing).unwrap();
        for mutation in [
            "coverage",
            "build",
            "verifier",
            "required",
            "advertised",
            "depth budget",
            "state budget",
            "frontier budget",
            "execution budget",
            "page budget",
            "states",
            "frontiers",
            "executions",
            "location",
            "unknown location",
            "unknown coverage",
            "missing seed",
            "seed label",
            "seed depth",
            "seed receipt",
            "seed state",
            "seed start",
            "seed order",
        ] {
            let mut changed = staffing.clone();
            match mutation {
                "coverage" => {
                    changed
                        .covered_definitions
                        .remove("fume_yards.recover_staffed_filter");
                }
                "build" => changed.build_id.push('x'),
                "verifier" => changed.verifier_id.push('x'),
                "required" => {
                    changed
                        .required_definitions
                        .remove("fume_yards.recover_staffed_filter");
                }
                "advertised" => {
                    changed
                        .advertised_definitions
                        .remove("fume_yards.recover_staffed_filter");
                }
                "depth budget" => changed.budget.max_depth += 1,
                "state budget" => changed.budget.max_expanded_states += 1,
                "frontier budget" => changed.budget.max_discovered_frontiers += 1,
                "execution budget" => changed.budget.max_action_executions += 1,
                "page budget" => changed.budget.catalog_page_size += 1,
                "states" => changed.expanded_states = 97,
                "frontiers" => changed.discovered_frontiers = 769,
                "executions" => changed.successful_actions = 2049,
                "location" => {
                    changed.reached_locations.remove("fume_yards.ash_beds");
                }
                "unknown location" => {
                    changed.reached_locations.insert("test.unknown".to_owned());
                }
                "unknown coverage" => {
                    changed
                        .covered_definitions
                        .insert("test.unknown".to_owned());
                }
                "missing seed" => {
                    changed.starting_sessions.pop();
                }
                "seed label" => changed.starting_sessions[2].label.push('x'),
                "seed depth" => changed.starting_sessions[2].depth = 0,
                "seed receipt" => changed.starting_sessions[2].final_receipt.push('x'),
                "seed state" => changed.starting_sessions[2].state_id.push('x'),
                "seed start" => {
                    changed.starting_sessions[2].start = changed.starting_sessions[1].start.clone()
                }
                "seed order" => changed.starting_sessions.swap(0, 2),
                _ => unreachable!(),
            }
            assert!(
                check_staffing_report(&content, &changed).is_err(),
                "accepted staffing {mutation}"
            );
            assert!(
                combine_crawls(
                    &content,
                    regression.clone(),
                    batchworks.clone(),
                    salvage.clone(),
                    market_water.clone(),
                    changed
                )
                .is_err(),
                "combined report accepted staffing {mutation}"
            );
        }
    }

    #[test]
    fn legacy_projection_rejects_a_missing_old_action_or_location() {
        let mut report = CrawlReport {
            verifier_id: "fixture".to_owned(),
            build_id: "fixture".to_owned(),
            budget: CrawlBudget::default(),
            expanded_states: 0,
            discovered_frontiers: 0,
            successful_actions: 0,
            max_legal_actions: 0,
            reached_locations: ids(SPLIT_TIDE_LOCATIONS),
            covered_definitions: ids(SPLIT_TIDE_ACTIONS),
            advertised_definitions: ids(SPLIT_TIDE_ACTIONS),
            required_definitions: ids(SPLIT_TIDE_ACTIONS),
            starting_sessions: Vec::new(),
            execution_receipt: "fixture".to_owned(),
        };
        preserve_split_tide(report.clone()).unwrap();
        report.covered_definitions.remove("top.split_flow");
        assert!(preserve_split_tide(report.clone()).is_err());
        report
            .covered_definitions
            .insert("top.split_flow".to_owned());
        report.reached_locations.remove("lowsail.return");
        assert!(preserve_split_tide(report).is_err());
    }
}
