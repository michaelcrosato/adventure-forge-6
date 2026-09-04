use forge_cli::{PlayerMcpConfig, run_player_mcp, run_player_mcp_socket};
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

    let result = if config.socket_path.is_some() {
        run_player_mcp_socket(&config)
    } else {
        let stdin = io::stdin();
        let mut input = BufReader::new(stdin.lock());
        let stdout = io::stdout();
        let mut output = stdout.lock();
        run_player_mcp(&config, &mut input, &mut output)
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("player adapter error: {error}");
            ExitCode::from(2)
        }
    }
}
