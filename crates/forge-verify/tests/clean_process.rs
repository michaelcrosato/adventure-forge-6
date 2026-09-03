use forge_verify::SCENARIO_IDS;
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

#[test]
fn clean_process_outputs_match_each_other_and_checked_witnesses() {
    for scenario in SCENARIO_IDS {
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
