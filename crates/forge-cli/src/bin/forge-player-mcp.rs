use forge_cli::{PlayerMcpConfig, run_player_mcp};
use std::env;
use std::io::{self, BufReader};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let config = match PlayerMcpConfig::parse(&args) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("player adapter error: {error}");
            return ExitCode::from(2);
        }
    };

    let stdin = io::stdin();
    let mut input = BufReader::new(stdin.lock());
    let stdout = io::stdout();
    let mut output = stdout.lock();
    match run_player_mcp(&config, &mut input, &mut output) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("player adapter error: {error}");
            ExitCode::from(2)
        }
    }
}
