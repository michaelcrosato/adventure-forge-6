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

fn ids(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|id| (*id).to_owned()).collect()
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SplitTideProjection {
    pub required_definitions: BTreeSet<String>,
    pub covered_definitions: BTreeSet<String>,
    pub required_locations: BTreeSet<String>,
    pub reached_locations: BTreeSet<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ProductionCrawlReport {
    #[serde(flatten)]
    pub crawl: CrawlReport,
    pub split_tide_projection: SplitTideProjection,
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
) -> Result<ProductionCrawlReport, VerifyError> {
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
    Ok(ProductionCrawlReport {
        crawl,
        split_tide_projection: SplitTideProjection {
            required_definitions,
            covered_definitions,
            required_locations,
            reached_locations,
        },
    })
}

pub(super) fn crawl_pilot(content: &CompiledContent) -> Result<CrawlReport, VerifyError> {
    let required = ids(PILOT_ACTIONS);
    let authored = content
        .actions()
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    if authored != ids(SPLIT_TIDE_ACTIONS).union(&required).cloned().collect() {
        return Err(VerifyError::new(
            "optional crawl action contract contains unreviewed additions or omissions",
        ));
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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
