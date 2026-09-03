use forge_verify::{EvidenceWitness, SCENARIO_IDS, check_witness, generate_witness};
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
        [command, path] if command == "check" => {
            let witness = read_witness(Path::new(path))?;
            check_witness(&witness).map_err(|error| error.to_string())?;
            Ok(format!(
                "VERIFIED {} with {} step(s)\n",
                witness.scenario_id,
                witness.steps.len()
            ))
        }
        [command] if command == "scenarios" => Ok(format!("{}\n", SCENARIO_IDS.join("\n"))),
        [] | [_] => Err(usage()),
        _ => Err(format!("unexpected arguments\n{}", usage())),
    }
}

fn read_witness(path: &Path) -> Result<EvidenceWitness, String> {
    let file =
        File::open(path).map_err(|error| format!("could not open {}: {error}", path.display()))?;
    let mut bytes = Vec::new();
    BufReader::new(file)
        .take(MAX_WITNESS_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("could not read {}: {error}", path.display()))?;
    if bytes.len() as u64 > MAX_WITNESS_BYTES {
        return Err("evidence witness exceeds the 4 MiB limit".to_owned());
    }
    let json =
        std::str::from_utf8(&bytes).map_err(|_| "evidence witness is not UTF-8".to_owned())?;
    EvidenceWitness::from_json(json).map_err(|error| error.to_string())
}

fn usage() -> String {
    "usage: forge-verify emit SCENARIO | check PATH | scenarios".to_owned()
}
