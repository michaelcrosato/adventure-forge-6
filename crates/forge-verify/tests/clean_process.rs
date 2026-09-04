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
    assert_eq!(report["reached_locations"].as_array().unwrap().len(), 6);
}
