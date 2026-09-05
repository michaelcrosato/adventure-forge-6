use forge_verify::scenario_ids;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn verifier(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_forge-verify"))
        .args(args)
        .output()
        .expect("verifier process starts")
}

fn witness_path(scenario: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../evidence/witnesses")
        .join(format!("{scenario}.json"))
}

fn crawl_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../evidence/crawls/split-tide.json")
}

fn scale_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../evidence/scale/synthetic-ring-500.json")
}

fn pilot_definitions() -> std::collections::BTreeSet<&'static str> {
    [
        "fume_yards.take_stock",
        "fume_yards.press_repair_plugs",
        "fume_yards.pack_catch_screen",
        "fume_yards.fit_catch_screen",
        "fume_yards.load_freight",
        "fume_yards.load_screened_freight",
        "return.patch_stand",
        "return.sort_dry_goods",
        "return.visit_workshop",
    ]
    .into_iter()
    .collect()
}

fn salvage_definitions() -> std::collections::BTreeSet<&'static str> {
    [
        "fume_yards.enter_ash_hatch",
        "fume_yards.leave_ash_hatch",
        "fume_yards.brace_rack",
        "fume_yards.recover_braced_filter",
        "fume_yards.thread_rack_filter",
        "fume_yards.pull_rack_filter",
        "fume_yards.report_with_daro",
        "fume_yards.load_cold_freight",
    ]
    .into_iter()
    .collect()
}

fn market_water_definitions() -> std::collections::BTreeSet<&'static str> {
    [
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
    ]
    .into_iter()
    .collect()
}

fn staffing_definitions() -> std::collections::BTreeSet<&'static str> {
    [
        "fume_yards.share_rescue_account",
        "fume_yards.assign_brann_salvage",
        "fume_yards.recover_staffed_filter",
        "fume_yards.return_with_brann",
    ]
    .into_iter()
    .collect()
}

#[test]
fn clean_process_outputs_match_each_other_and_checked_witnesses() {
    let expected_files: Vec<_> = scenario_ids()
        .map(|scenario| format!("{scenario}.json"))
        .collect();
    let mut checked_files: Vec<_> =
        std::fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../evidence/witnesses"))
            .expect("witness directory exists")
            .map(|entry| {
                entry
                    .expect("witness directory entry is readable")
                    .file_name()
                    .into_string()
                    .expect("witness filename is UTF-8")
            })
            .collect();
    checked_files.sort();
    let mut expected_files = expected_files;
    expected_files.sort();
    assert_eq!(
        checked_files, expected_files,
        "checked witness set is stale"
    );

    for scenario in scenario_ids() {
        let first = verifier(&["emit", scenario]);
        let second = verifier(&["emit", scenario]);
        assert!(first.status.success(), "first emit failed");
        assert!(second.status.success(), "second emit failed");
        assert_eq!(first.stdout, second.stdout, "clean processes diverged");

        let path = witness_path(scenario);
        let checked = std::fs::read(&path).expect("checked witness exists");
        assert_eq!(first.stdout, checked, "checked witness is stale");

        let verified = verifier(&["check", path.to_str().expect("UTF-8 witness path")]);
        assert!(
            verified.status.success(),
            "checked witness failed: {}",
            String::from_utf8_lossy(&verified.stderr)
        );
    }
}

#[test]
fn clean_process_crawls_match_each_other_and_checked_report() {
    let first = verifier(&["crawl"]);
    let second = verifier(&["crawl"]);
    assert!(
        first.status.success(),
        "first crawl failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(second.status.success(), "second crawl failed");
    assert_eq!(first.stdout, second.stdout, "clean-process crawls diverged");

    let checked = std::fs::read(crawl_path()).expect("checked crawl report exists");
    assert_eq!(first.stdout, checked, "checked crawl report is stale");

    let report: serde_json::Value =
        serde_json::from_slice(&first.stdout).expect("crawl report is valid JSON");
    assert_eq!(
        report["covered_definitions"],
        report["advertised_definitions"]
    );
    assert_eq!(
        report["advertised_definitions"].as_array().unwrap().len(),
        95
    );
    assert_eq!(report["reached_locations"].as_array().unwrap().len(), 9);
    let regression = &report["regression"];
    let batchworks = &report["batchworks"];
    let salvage = &report["salvage"];
    let market_water = &report["market_water"];
    let staffing = &report["staffing"];
    assert_eq!(
        regression["required_definitions"].as_array().unwrap().len(),
        60
    );
    for component in [regression, batchworks, salvage, market_water, staffing] {
        assert_eq!(component["build_id"], report["build_id"]);
        assert_eq!(component["verifier_id"], report["verifier_id"]);
        assert_eq!(
            component["advertised_definitions"],
            report["advertised_definitions"]
        );
        let covered = component["covered_definitions"].as_array().unwrap();
        assert!(
            component["required_definitions"]
                .as_array()
                .unwrap()
                .iter()
                .all(|id| covered.contains(id))
        );
    }
    let union = |field: &str| {
        [regression, batchworks, salvage, market_water, staffing]
            .into_iter()
            .flat_map(|component| component[field].as_array().unwrap())
            .map(|value| value.as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>()
    };
    let declared = |field: &str| {
        report[field]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>()
    };
    assert_eq!(
        union("covered_definitions"),
        declared("covered_definitions")
    );
    assert_eq!(
        union("required_definitions"),
        declared("advertised_definitions")
    );
    assert_eq!(union("reached_locations"), declared("reached_locations"));
    assert_eq!(regression["budget"]["max_depth"], 13);
    assert_eq!(regression["budget"]["max_expanded_states"], 4096);
    assert_eq!(regression["budget"]["max_discovered_frontiers"], 65536);
    assert_eq!(regression["budget"]["max_action_executions"], 65536);
    assert_eq!(regression["budget"]["catalog_page_size"], 7);
    let starts = regression["starting_sessions"].as_array().unwrap();
    assert_eq!(starts.len(), 2, "old regression starts remain unseeded");
    for (index, preset) in ["ilyan", "rook"].iter().enumerate() {
        assert_eq!(starts[index]["label"], format!("preset:{preset}"));
        assert_eq!(starts[index]["depth"], 0);
    }
    assert_batchworks_scope(batchworks);
    assert_salvage_scope(salvage);
    assert_market_water_scope(market_water);
    assert_staffing_scope(staffing);
    let core = &regression["split_tide_projection"];
    assert_eq!(core["required_definitions"], core["covered_definitions"]);
    assert_eq!(core["required_locations"], core["reached_locations"]);
    assert_eq!(core["required_definitions"].as_array().unwrap().len(), 51);
    assert_eq!(core["required_locations"].as_array().unwrap().len(), 6);
    let core_and_pilot = core["required_definitions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .chain(pilot_definitions())
        .collect::<std::collections::BTreeSet<_>>();
    let old_required = regression["required_definitions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(old_required, core_and_pilot);
}

#[test]
fn clean_process_scale_runs_match_and_checked_report_verifies() {
    let first = verifier(&["scale"]);
    let second = verifier(&["scale"]);
    assert!(first.status.success(), "first scale run failed");
    assert!(second.status.success(), "second scale run failed");
    assert_eq!(
        first.stdout, second.stdout,
        "clean-process scale runs diverged"
    );

    let path = scale_path();
    let checked = std::fs::read(&path).expect("checked scale report exists");
    assert_eq!(first.stdout, checked, "checked scale report is stale");

    let verified = verifier(&[
        "check-scale",
        path.to_str().expect("UTF-8 scale report path"),
    ]);
    assert!(
        verified.status.success(),
        "checked scale report failed: {}",
        String::from_utf8_lossy(&verified.stderr)
    );

    let report: serde_json::Value =
        serde_json::from_slice(&first.stdout).expect("scale report is valid JSON");
    assert_eq!(report["claim_scope"], "capacity_fixture");
    assert_eq!(report["location_count"], 500);
    assert_eq!(report["hop_count"], 500);
    assert_eq!(report["final_location"], "loc-0000");
}

#[test]
fn clean_process_optional_crawls_match_checked_pilot_report() {
    let first = verifier(&["crawl-optional"]);
    let second = verifier(&["crawl-optional"]);
    assert!(
        first.status.success(),
        "first optional crawl failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(second.status.success(), "second optional crawl failed");
    assert_eq!(
        first.stdout, second.stdout,
        "clean-process optional crawls diverged"
    );
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../evidence/crawls/fume-yards-pilot.json");
    assert_eq!(
        first.stdout,
        std::fs::read(path).expect("checked optional report exists"),
        "checked optional report is stale"
    );
    let report: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    let required = report["required_definitions"].as_array().unwrap();
    let covered = report["covered_definitions"].as_array().unwrap();
    assert_eq!(required.len(), 9);
    assert_eq!(
        required
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>(),
        pilot_definitions()
    );
    assert!(required.iter().all(|id| covered.contains(id)));
    assert_eq!(
        report["advertised_definitions"].as_array().unwrap().len(),
        95
    );
    assert_eq!(report["budget"]["max_depth"], 13);
    assert_eq!(report["budget"]["max_expanded_states"], 96);
    assert_eq!(report["budget"]["max_discovered_frontiers"], 512);
    assert_eq!(report["budget"]["max_action_executions"], 1024);
    assert_eq!(report["budget"]["catalog_page_size"], 7);
    assert_aftermath_starts(&report);
}

fn assert_aftermath_starts(report: &serde_json::Value) {
    let starts = report["starting_sessions"].as_array().unwrap();
    assert_eq!(starts.len(), 3);
    for (index, preset) in ["ilyan", "rook"].iter().enumerate() {
        assert_eq!(starts[index]["label"], format!("preset:{preset}"));
        assert_eq!(starts[index]["depth"], 0);
        assert_eq!(starts[index]["start"]["character_preset_id"], *preset);
        assert_eq!(starts[index]["start"]["seed"], 71);
    }
    // Bind the extra frontier to the separately checked, unchanged production
    // witness, including its seven consumed actions and exact final lineage.
    let hold: serde_json::Value =
        serde_json::from_slice(&std::fs::read(witness_path("m1-outcome-hold-market")).unwrap())
            .unwrap();
    assert_eq!(starts[2]["label"], "scenario:m1-outcome-hold-market");
    assert_eq!(starts[2]["depth"], 7);
    assert_eq!(hold["steps"].as_array().unwrap().len(), 7);
    assert_eq!(starts[2]["start"], starts[0]["start"]);
    assert_eq!(starts[2]["final_receipt"], hold["final_receipt"]);
    assert_eq!(starts[2]["state_id"], hold["final_state_id"]);
}

fn assert_batchworks_scope(report: &serde_json::Value) {
    let required = report["required_definitions"].as_array().unwrap();
    let covered = report["covered_definitions"].as_array().unwrap();
    assert_eq!(required.len(), 13);
    assert!(required.iter().all(|id| covered.contains(id)));
    assert_eq!(
        report["advertised_definitions"].as_array().unwrap().len(),
        95
    );
    assert_eq!(report["budget"]["max_depth"], 20);
    assert_eq!(report["budget"]["max_expanded_states"], 128);
    assert_eq!(report["budget"]["max_discovered_frontiers"], 768);
    assert_eq!(report["budget"]["max_action_executions"], 2048);
    assert_eq!(report["budget"]["catalog_page_size"], 7);
    assert_aftermath_starts(report);
}

#[test]
fn clean_process_batchworks_crawls_match_checked_report() {
    let first = verifier(&["crawl-batchworks"]);
    let second = verifier(&["crawl-batchworks"]);
    assert!(
        first.status.success(),
        "first batchworks crawl failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(second.status.success(), "second batchworks crawl failed");
    assert_eq!(
        first.stdout, second.stdout,
        "clean-process batchworks crawls diverged"
    );
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../evidence/crawls/fume-yards-batchworks.json");
    assert_eq!(
        first.stdout,
        std::fs::read(path).expect("checked batchworks report exists"),
        "checked batchworks report is stale"
    );
    let report: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_batchworks_scope(&report);
}

fn assert_salvage_scope(report: &serde_json::Value) {
    let required = report["required_definitions"].as_array().unwrap();
    let covered = report["covered_definitions"].as_array().unwrap();
    assert_eq!(required.len(), 8);
    assert_eq!(
        required
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>(),
        salvage_definitions()
    );
    assert!(required.iter().all(|id| covered.contains(id)));
    assert_eq!(
        report["advertised_definitions"].as_array().unwrap().len(),
        95
    );
    let locations = report["reached_locations"].as_array().unwrap();
    for location in [
        "fume_yards.ash_beds",
        "fume_yards.kiln_bay",
        "fume_yards.workshop",
    ] {
        assert!(
            locations
                .iter()
                .any(|value| value.as_str() == Some(location))
        );
    }
    assert_eq!(report["budget"]["max_depth"], 20);
    assert_eq!(report["budget"]["max_expanded_states"], 96);
    assert_eq!(report["budget"]["max_discovered_frontiers"], 768);
    assert_eq!(report["budget"]["max_action_executions"], 2048);
    assert_eq!(report["budget"]["catalog_page_size"], 7);
    assert_aftermath_starts(report);
}

#[test]
fn clean_process_salvage_crawls_match_checked_report() {
    let first = verifier(&["crawl-salvage"]);
    let second = verifier(&["crawl-salvage"]);
    assert!(
        first.status.success(),
        "first salvage crawl failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(second.status.success(), "second salvage crawl failed");
    assert_eq!(
        first.stdout, second.stdout,
        "clean-process salvage crawls diverged"
    );
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../evidence/crawls/fume-yards-salvage.json");
    assert_eq!(
        first.stdout,
        std::fs::read(path).expect("checked salvage report exists"),
        "checked salvage report is stale"
    );
    let report: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_salvage_scope(&report);
}

fn assert_market_water_scope(report: &serde_json::Value) {
    let required = report["required_definitions"].as_array().unwrap();
    let covered = report["covered_definitions"].as_array().unwrap();
    assert_eq!(required.len(), 10);
    assert_eq!(
        required
            .iter()
            .map(|id| id.as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>(),
        market_water_definitions()
    );
    assert!(required.iter().all(|id| covered.contains(id)));
    assert_eq!(
        report["advertised_definitions"].as_array().unwrap().len(),
        95
    );
    assert_eq!(report["budget"]["max_depth"], 35);
    assert_eq!(report["budget"]["max_expanded_states"], 128);
    assert_eq!(report["budget"]["max_discovered_frontiers"], 1024);
    assert_eq!(report["budget"]["max_action_executions"], 4096);
    assert_eq!(report["budget"]["catalog_page_size"], 7);
    assert!(report["expanded_states"].as_u64().unwrap() <= 128);
    assert!(report["discovered_frontiers"].as_u64().unwrap() <= 1024);
    assert!(report["successful_actions"].as_u64().unwrap() <= 4096);
    for location in [
        "fume_yards.ash_beds",
        "fume_yards.kiln_bay",
        "fume_yards.workshop",
        "lowsail.return",
    ] {
        assert!(
            report["reached_locations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|id| id.as_str() == Some(location))
        );
    }
    assert_aftermath_starts(report);
}

#[test]
fn clean_process_market_water_crawls_match_checked_report() {
    let first = verifier(&["crawl-market-water"]);
    let second = verifier(&["crawl-market-water"]);
    assert!(
        first.status.success(),
        "first market-water crawl failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(
        second.status.success(),
        "second market-water crawl failed: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(
        first.stdout, second.stdout,
        "clean-process market-water crawls diverged"
    );
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../evidence/crawls/fume-yards-market-water.json");
    assert_eq!(
        first.stdout,
        std::fs::read(path).expect("checked market-water report exists"),
        "checked market-water report is stale"
    );
    let report: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_market_water_scope(&report);
    let combined: serde_json::Value =
        serde_json::from_slice(&std::fs::read(crawl_path()).unwrap()).unwrap();
    assert_eq!(report["build_id"], combined["build_id"]);
    assert_eq!(report["verifier_id"], combined["verifier_id"]);
    assert_eq!(
        report["advertised_definitions"],
        combined["advertised_definitions"]
    );
    assert_eq!(report, combined["market_water"]);
}

fn assert_staffing_scope(report: &serde_json::Value) {
    let required = report["required_definitions"].as_array().unwrap();
    let covered = report["covered_definitions"].as_array().unwrap();
    assert_eq!(required.len(), 4);
    assert_eq!(
        required
            .iter()
            .map(|id| id.as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>(),
        staffing_definitions()
    );
    assert!(required.iter().all(|id| covered.contains(id)));
    assert_eq!(
        report["advertised_definitions"].as_array().unwrap().len(),
        95
    );
    assert_eq!(report["budget"]["max_depth"], 20);
    assert_eq!(report["budget"]["max_expanded_states"], 96);
    assert_eq!(report["budget"]["max_discovered_frontiers"], 768);
    assert_eq!(report["budget"]["max_action_executions"], 2048);
    assert_eq!(report["budget"]["catalog_page_size"], 7);
    assert!(report["expanded_states"].as_u64().unwrap() <= 96);
    assert!(report["discovered_frontiers"].as_u64().unwrap() <= 768);
    assert!(report["successful_actions"].as_u64().unwrap() <= 2048);
    for location in [
        "fume_yards.ash_beds",
        "fume_yards.kiln_bay",
        "fume_yards.workshop",
    ] {
        assert!(
            report["reached_locations"]
                .as_array()
                .unwrap()
                .iter()
                .any(|id| id.as_str() == Some(location))
        );
    }
    assert_aftermath_starts(report);
}

#[test]
fn clean_process_staffing_crawls_match_checked_report() {
    let first = verifier(&["crawl-staffing"]);
    let second = verifier(&["crawl-staffing"]);
    assert!(
        first.status.success(),
        "first staffing crawl failed: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(
        second.status.success(),
        "second staffing crawl failed: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(
        first.stdout, second.stdout,
        "clean-process staffing crawls diverged"
    );
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../evidence/crawls/fume-yards-staffing.json");
    assert_eq!(
        first.stdout,
        std::fs::read(path).expect("checked staffing report exists"),
        "checked staffing report is stale"
    );
    let report: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_staffing_scope(&report);
    let combined: serde_json::Value =
        serde_json::from_slice(&std::fs::read(crawl_path()).unwrap()).unwrap();
    assert_eq!(report["build_id"], combined["build_id"]);
    assert_eq!(report["verifier_id"], combined["verifier_id"]);
    assert_eq!(
        report["advertised_definitions"],
        combined["advertised_definitions"]
    );
    assert_eq!(report, combined["staffing"]);
}
