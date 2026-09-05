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
    assert!(first.status.success(), "first crawl failed");
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
    assert_eq!(report["reached_locations"].as_array().unwrap().len(), 7);
    let core = &report["split_tide_projection"];
    assert_eq!(core["required_definitions"], core["covered_definitions"]);
    assert_eq!(core["required_locations"], core["reached_locations"]);
    assert_eq!(core["required_definitions"].as_array().unwrap().len(), 51);
    assert_eq!(core["required_locations"].as_array().unwrap().len(), 6);
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
    assert!(required.iter().all(|id| covered.contains(id)));
    assert_eq!(
        report["advertised_definitions"].as_array().unwrap().len(),
        60
    );
    assert_eq!(report["budget"]["max_depth"], 13);
    assert_eq!(report["budget"]["max_expanded_states"], 96);
    assert_eq!(report["budget"]["max_discovered_frontiers"], 512);
    assert_eq!(report["budget"]["max_action_executions"], 1024);
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
