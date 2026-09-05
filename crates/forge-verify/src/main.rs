use forge_replay::PlayerTrace;
use forge_verify::{
    EvidenceWitness, MAX_PLAYER_TRACE_BYTES, SCALE_MAX_REPORT_BYTES, ScaleReport,
    check_player_trace, check_scale_report, check_witness, generate_batchworks_crawl_report,
    generate_crawl_report, generate_market_water_crawl_report, generate_optional_crawl_report,
    generate_salvage_crawl_report, generate_scale_report, generate_staffing_crawl_report,
    generate_witness, scenario_ids,
};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::process::ExitCode;

const MAX_WITNESS_BYTES: u64 = 4 * 1024 * 1024;

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(2)
        }
    }
}

fn run(args: Vec<String>) -> Result<String, String> {
    match args.as_slice() {
        [command, scenario] if command == "emit" => generate_witness(scenario)
            .and_then(|witness| witness.to_pretty_json())
            .map_err(|error| error.to_string()),
        [command] if command == "crawl" => generate_crawl_report()
            .and_then(|report| report.to_pretty_json())
            .map_err(|error| error.to_string()),
        [command] if command == "crawl-optional" => generate_optional_crawl_report()
            .and_then(|report| report.to_pretty_json())
            .map_err(|error| error.to_string()),
        [command] if command == "crawl-batchworks" => generate_batchworks_crawl_report()
            .and_then(|report| report.to_pretty_json())
            .map_err(|error| error.to_string()),
        [command] if command == "crawl-salvage" => generate_salvage_crawl_report()
            .and_then(|report| report.to_pretty_json())
            .map_err(|error| error.to_string()),
        [command] if command == "crawl-market-water" => generate_market_water_crawl_report()
            .and_then(|report| report.to_pretty_json())
            .map_err(|error| error.to_string()),
        [command] if command == "crawl-staffing" => generate_staffing_crawl_report()
            .and_then(|report| report.to_pretty_json())
            .map_err(|error| error.to_string()),
        [command] if command == "scale" => generate_scale_report()
            .and_then(|report| report.to_pretty_json())
            .map_err(|error| error.to_string()),
        [command, path] if command == "check-scale" => {
            let report = read_scale_report(Path::new(path))?;
            check_scale_report(&report).map_err(|error| error.to_string())?;
            Ok(format!(
                "VERIFIED synthetic capacity fixture with {} locations and {} hops\n",
                report.location_count, report.hop_count
            ))
        }
        [command, path] if command == "check" => {
            let witness = read_witness(Path::new(path))?;
            check_witness(&witness).map_err(|error| error.to_string())?;
            Ok(format!(
                "VERIFIED {} with {} step(s)\n",
                witness.scenario_id,
                witness.steps.len()
            ))
        }
        [command, path] if command == "check-player" => {
            let trace = read_player_trace(Path::new(path))?;
            let checked = check_player_trace(&trace).map_err(|error| error.to_string())?;
            Ok(format!(
                "VERIFIED PLAYER TRACE\nVerifier: {}\nBuild: {}\nSteps: {}\nFinal state: {}\nFinal receipt: {}\n",
                checked.verifier_id,
                checked.build_id,
                checked.action_count,
                checked.final_state_id,
                checked.final_receipt,
            ))
        }
        [command] if command == "scenarios" => Ok(format!(
            "{}\n",
            scenario_ids().collect::<Vec<_>>().join("\n")
        )),
        [] | [_] => Err(usage()),
        _ => Err(format!("unexpected arguments\n{}", usage())),
    }
}

fn read_witness(path: &Path) -> Result<EvidenceWitness, String> {
    let json = read_bounded_utf8(path, MAX_WITNESS_BYTES, "evidence witness")?;
    EvidenceWitness::from_json(&json).map_err(|error| error.to_string())
}

fn read_scale_report(path: &Path) -> Result<ScaleReport, String> {
    let json = read_bounded_utf8(path, SCALE_MAX_REPORT_BYTES as u64, "scale report")?;
    ScaleReport::from_json(&json).map_err(|error| error.to_string())
}

fn read_player_trace(path: &Path) -> Result<PlayerTrace, String> {
    let json = read_bounded_utf8(path, MAX_PLAYER_TRACE_BYTES, "player trace")?;
    PlayerTrace::from_json(&json).map_err(|_| "player trace contains invalid JSON".to_owned())
}

fn read_bounded_utf8(path: &Path, max_bytes: u64, kind: &str) -> Result<String, String> {
    let file =
        File::open(path).map_err(|error| format!("could not open {}: {error}", path.display()))?;
    let mut bytes = Vec::new();
    BufReader::new(file)
        .take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    if bytes.len() as u64 > max_bytes {
        return Err(format!("{kind} exceeds its {max_bytes}-byte input limit"));
    }
    let json = std::str::from_utf8(&bytes).map_err(|_| format!("{kind} is not UTF-8"))?;
    Ok(json.to_owned())
}

fn usage() -> String {
    "usage: forge-verify crawl | crawl-optional | crawl-batchworks | crawl-salvage | crawl-market-water | crawl-staffing | scale | check-scale PATH | emit SCENARIO | check PATH | check-player PATH | scenarios".to_owned()
}
