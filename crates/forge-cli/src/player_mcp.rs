use super::{CliError, atomic_write, load_content, short_hash, write_trace};
use forge_kernel::{
    ActionView, CompiledContent, Observation, enumerate_legal_actions, validate_unique_json_keys,
};
use forge_replay::Session;
use serde_json::{Value, json};
use std::io::{BufRead, Read, Write};
use std::path::PathBuf;

const MCP_PROTOCOL_VERSION: &str = "2025-06-18";
const MAX_MCP_LINE_BYTES: usize = 128 * 1024;
const MAX_MCP_REQUESTS: usize = 512;
const MAX_PUBLIC_TRANSCRIPT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerMcpConfig {
    pub character: String,
    pub seed: u64,
    pub trace_path: PathBuf,
    pub transcript_path: PathBuf,
    pub observation_canary: String,
    pub minimum_turns: usize,
    pub maximum_turns: usize,
    pub socket_path: Option<PathBuf>,
}

impl PlayerMcpConfig {
    pub fn parse(args: &[String]) -> Result<Self, CliError> {
        let mut character = None;
        let mut seed = None;
        let mut trace_path = None;
        let mut transcript_path = None;
        let mut observation_canary = None;
        let mut minimum_turns = None;
        let mut maximum_turns = None;
        let mut socket_path = None;
        let mut index = 0usize;
        while index < args.len() {
            let target = match args[index].as_str() {
                "--character" => &mut character,
                "--trace" => &mut trace_path,
                "--transcript" => &mut transcript_path,
                "--canary" => &mut observation_canary,
                "--socket" => &mut socket_path,
                "--seed" | "--min-turns" | "--max-turns" => {
                    let option = args[index].clone();
                    index = index
                        .checked_add(1)
                        .ok_or_else(|| CliError::new("player adapter argument overflow"))?;
                    let value = args
                        .get(index)
                        .ok_or_else(|| CliError::new(format!("{option} requires a value")))?;
                    match option.as_str() {
                        "--seed" => set_once(&mut seed, value, "--seed")?,
                        "--min-turns" => set_once(&mut minimum_turns, value, "--min-turns")?,
                        "--max-turns" => set_once(&mut maximum_turns, value, "--max-turns")?,
                        _ => unreachable!("matched numeric player adapter option"),
                    }
                    index += 1;
                    continue;
                }
                _ => return Err(CliError::new("unknown player adapter option")),
            };
            let option = args[index].clone();
            index = index
                .checked_add(1)
                .ok_or_else(|| CliError::new("player adapter argument overflow"))?;
            let value = args
                .get(index)
                .ok_or_else(|| CliError::new(format!("{option} requires a value")))?;
            set_once(target, value, &option)?;
            index += 1;
        }

        let seed = parse_number(seed, "--seed")?;
        let minimum_turns = parse_number(minimum_turns, "--min-turns")?;
        let maximum_turns = parse_number(maximum_turns, "--max-turns")?;
        if minimum_turns == 0 || maximum_turns == 0 || minimum_turns > maximum_turns {
            return Err(CliError::new(
                "player adapter turn bounds must be positive and ordered",
            ));
        }
        if maximum_turns > 64 {
            return Err(CliError::new(
                "player adapter maximum exceeds the 64-turn limit",
            ));
        }
        let character = character.ok_or_else(|| CliError::new("--character is required"))?;
        if character.is_empty() || character.len() > 64 || !character.is_ascii() {
            return Err(CliError::new("player adapter character id is invalid"));
        }
        let observation_canary =
            observation_canary.ok_or_else(|| CliError::new("--canary is required"))?;
        if !(16..=128).contains(&observation_canary.len())
            || !observation_canary
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(CliError::new("player adapter canary is invalid"));
        }
        let trace_path =
            PathBuf::from(trace_path.ok_or_else(|| CliError::new("--trace is required"))?);
        let transcript_path = PathBuf::from(
            transcript_path.ok_or_else(|| CliError::new("--transcript is required"))?,
        );
        if trace_path == transcript_path {
            return Err(CliError::new(
                "player trace and transcript paths must differ",
            ));
        }
        let socket_path = socket_path.map(PathBuf::from);
        if socket_path
            .as_ref()
            .is_some_and(|path| path == &trace_path || path == &transcript_path)
        {
            return Err(CliError::new(
                "player socket, trace, and transcript paths must differ",
            ));
        }

        Ok(Self {
            character,
            seed,
            trace_path,
            transcript_path,
            observation_canary,
            minimum_turns,
            maximum_turns,
            socket_path,
        })
    }
}

#[cfg(unix)]
pub fn run_player_mcp_socket(config: &PlayerMcpConfig) -> Result<(), CliError> {
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;

    let path = config
        .socket_path
        .as_ref()
        .ok_or_else(|| CliError::new("player adapter socket path is required"))?;
    let listener = UnixListener::bind(path)
        .map_err(|_| CliError::new("could not create player adapter socket"))?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o666))
        .map_err(|_| CliError::new("could not secure player adapter socket"))?;
    let (stream, _) = listener
        .accept()
        .map_err(|_| CliError::new("could not accept player adapter connection"))?;
    let mut input = std::io::BufReader::new(
        stream
            .try_clone()
            .map_err(|_| CliError::new("could not prepare player adapter connection"))?,
    );
    let mut output = stream;
    run_player_mcp(config, &mut input, &mut output)
}

#[cfg(not(unix))]
pub fn run_player_mcp_socket(_config: &PlayerMcpConfig) -> Result<(), CliError> {
    Err(CliError::new("player adapter sockets require Unix"))
}

fn set_once(target: &mut Option<String>, value: &str, option: &str) -> Result<(), CliError> {
    if target.replace(value.to_owned()).is_some() {
        Err(CliError::new(format!(
            "duplicate player adapter option {option}"
        )))
    } else {
        Ok(())
    }
}

fn parse_number<T>(value: Option<String>, option: &str) -> Result<T, CliError>
where
    T: std::str::FromStr,
{
    value
        .ok_or_else(|| CliError::new(format!("{option} is required")))?
        .parse()
        .map_err(|_| CliError::new(format!("{option} must be a positive integer")))
}

pub fn run_player_mcp<R: BufRead, W: Write>(
    config: &PlayerMcpConfig,
    input: &mut R,
    output: &mut W,
) -> Result<(), CliError> {
    let content = load_content()?;
    let mut session = Session::new_game(&config.character, config.seed, &content)
        .map_err(super::public_session_error)?;
    let mut observation = content
        .observe(session.state())
        .map_err(|_| CliError::new("could not render the starting scene"))?;
    let mut transcript = format!(
        "Adventure Forge live blind player transcript\nBuild: {}\nCharacter: {}\nSeed: {}\nObservation canary: {}\n\n",
        observation.build_id, config.character, config.seed, config.observation_canary
    );
    append_view(
        &mut transcript,
        "Start",
        &public_view(&session, &content, &observation, config)?,
    )?;
    persist_public_session(config, &session, &transcript)?;

    let mut finished = false;
    let mut request_count = 0usize;
    loop {
        let Some(line) = read_mcp_line(input)? else {
            persist_public_session(config, &session, &transcript)?;
            return if finished {
                Ok(())
            } else {
                Err(CliError::new("player adapter disconnected before finish"))
            };
        };
        request_count = request_count
            .checked_add(1)
            .ok_or_else(|| CliError::new("player adapter request limit reached"))?;
        if request_count > MAX_MCP_REQUESTS {
            return Err(CliError::new("player adapter request limit reached"));
        }
        validate_unique_json_keys(&line)
            .map_err(|_| CliError::new("player adapter request contains invalid JSON"))?;
        let request: Value = serde_json::from_str(&line)
            .map_err(|_| CliError::new("player adapter request contains invalid JSON"))?;
        let Some(method) = request.get("method").and_then(Value::as_str) else {
            return Err(CliError::new("player adapter request omitted its method"));
        };
        let Some(id) = request.get("id").cloned() else {
            if method.starts_with("notifications/") {
                continue;
            }
            return Err(CliError::new("player adapter request omitted its id"));
        };

        let response = match method {
            "initialize" => initialize_response(&id, &request),
            "ping" => rpc_result(&id, json!({})),
            "tools/list" => rpc_result(&id, tools_list()),
            "tools/call" => {
                let params = request.get("params").unwrap_or(&Value::Null);
                tool_call_response(
                    &id,
                    params,
                    config,
                    &content,
                    &mut session,
                    &mut observation,
                    &mut transcript,
                    &mut finished,
                )?
            }
            _ => rpc_error(&id, -32601, "method not found"),
        };
        write_json_line(output, &response)?;
    }
}

fn read_mcp_line<R: BufRead>(input: &mut R) -> Result<Option<String>, CliError> {
    let mut line = String::new();
    let limit = u64::try_from(MAX_MCP_LINE_BYTES)
        .map_err(|_| CliError::new("player adapter input limit is unavailable"))?
        .checked_add(2)
        .ok_or_else(|| CliError::new("player adapter input limit is unavailable"))?;
    let bytes = Read::take(input, limit)
        .read_line(&mut line)
        .map_err(|_| CliError::new("could not read player adapter input"))?;
    if bytes == 0 {
        return Ok(None);
    }
    if line.len() > MAX_MCP_LINE_BYTES {
        return Err(CliError::new("player adapter input exceeds 128 KiB"));
    }
    Ok(Some(line))
}

fn initialize_response(id: &Value, request: &Value) -> Value {
    let protocol_version = request
        .pointer("/params/protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or(MCP_PROTOCOL_VERSION);
    rpc_result(
        id,
        json!({
            "protocolVersion": protocol_version,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "adventure-forge-player", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "Play only through observe, act, and finish. Every act submits one displayed action number to the deterministic game."
        }),
    )
}

fn tools_list() -> Value {
    json!({
        "tools": [
            {
                "name": "observe",
                "description": "Show the current player-visible scene and the complete numbered legal action catalog.",
                "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
            },
            {
                "name": "act",
                "description": "Perform exactly one action from the latest complete numbered catalog.",
                "inputSchema": {
                    "type": "object",
                    "properties": { "action_number": { "type": "integer", "minimum": 1 } },
                    "required": ["action_number"],
                    "additionalProperties": false
                }
            },
            {
                "name": "finish",
                "description": "End the playtest after the minimum number of actions and save its replay-verifiable player trace.",
                "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
            }
        ]
    })
}

#[allow(clippy::too_many_arguments)]
fn tool_call_response(
    id: &Value,
    params: &Value,
    config: &PlayerMcpConfig,
    content: &CompiledContent,
    session: &mut Session<'_>,
    observation: &mut Observation,
    transcript: &mut String,
    finished: &mut bool,
) -> Result<Value, CliError> {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return Ok(tool_error(id, "tool call omitted its name"));
    };
    let arguments = params.get("arguments").unwrap_or(&Value::Null);
    if !arguments.is_null() && !arguments.is_object() {
        return Ok(tool_error(id, "tool arguments must be an object"));
    }
    match name {
        "observe" => {
            if !empty_arguments(arguments) {
                return Ok(tool_error(id, "observe accepts no arguments"));
            }
            let view = public_view(session, content, observation, config)?;
            Ok(tool_success(id, view))
        }
        "act" => {
            if *finished {
                return Ok(tool_error(id, "the playtest is already finished"));
            }
            let turn_count = session.trace().steps.len();
            if turn_count >= config.maximum_turns {
                return Ok(tool_error(id, "the turn limit is reached; call finish now"));
            }
            let Some(number) = arguments.get("action_number").and_then(Value::as_u64) else {
                return Ok(tool_error(id, "act requires an integer action_number"));
            };
            if number == 0 || arguments.as_object().is_none_or(|values| values.len() != 1) {
                return Ok(tool_error(
                    id,
                    "act requires only one positive action_number",
                ));
            }
            let Ok(index) = usize::try_from(number.saturating_sub(1)) else {
                return Ok(tool_error(id, "action_number is outside this catalog"));
            };
            let page = content
                .action_page(session.state(), 0, usize::MAX)
                .map_err(|_| CliError::new("could not render current legal actions"))?;
            let Some(selected_view) = page.actions.get(index).cloned() else {
                return Ok(tool_error(id, "action_number is outside this catalog"));
            };
            let action = enumerate_legal_actions(session.state(), content)
                .map_err(|_| CliError::new("could not enumerate current legal actions"))?
                .into_iter()
                .find(|action| action.action_id == selected_view.action_id)
                .ok_or_else(|| CliError::new("displayed action became stale"))?;
            let recorded = session
                .record(&action)
                .map_err(super::public_session_error)?;
            *observation = recorded.observation;
            let view = public_view(session, content, observation, config)?;
            append_view(
                transcript,
                &format!(
                    "Turn {} — {}",
                    session.trace().steps.len(),
                    public_action_label(&selected_view)
                ),
                &view,
            )?;
            persist_public_session(config, session, transcript)?;
            Ok(tool_success(id, view))
        }
        "finish" => {
            if !empty_arguments(arguments) {
                return Ok(tool_error(id, "finish accepts no arguments"));
            }
            let turns = session.trace().steps.len();
            if turns < config.minimum_turns {
                return Ok(tool_error(
                    id,
                    &format!(
                        "play at least {} actions before finishing; {} completed",
                        config.minimum_turns, turns
                    ),
                ));
            }
            if !*finished {
                let ending = format!(
                    "Session finished after {turns} action(s).\nObservation canary: {}\nBuild: {}",
                    config.observation_canary,
                    session.trace().build_id
                );
                append_view(transcript, "Finish", &ending)?;
                *finished = true;
                persist_public_session(config, session, transcript)?;
            }
            Ok(tool_success(
                id,
                format!(
                    "Session finished after {turns} action(s). The player trace is saved.\nObservation canary: {}",
                    config.observation_canary
                ),
            ))
        }
        _ => Ok(tool_error(id, "unknown player tool")),
    }
}

fn empty_arguments(arguments: &Value) -> bool {
    arguments.is_null() || arguments.as_object().is_some_and(serde_json::Map::is_empty)
}

fn public_view(
    session: &Session<'_>,
    content: &CompiledContent,
    observation: &Observation,
    config: &PlayerMcpConfig,
) -> Result<String, CliError> {
    let page = content
        .action_page(session.state(), 0, usize::MAX)
        .map_err(|_| CliError::new("could not render current legal actions"))?;
    if page.actions.len() != page.total || page.next_offset.is_some() {
        return Err(CliError::new(
            "could not present the complete legal catalog",
        ));
    }
    let mut view = format!(
        "Observation canary: {}\nTurn: {}/{}\n\n{}\n{}\n{} legal action(s) · set {}\nActions 1–{} of {}:\n",
        config.observation_canary,
        session.trace().steps.len(),
        config.maximum_turns,
        observation.title,
        observation.text,
        observation.action_count,
        short_hash(&observation.action_set_digest),
        page.actions.len(),
        page.total
    );
    for (index, action) in page.actions.iter().enumerate() {
        view.push_str(&format!(
            "  {}. {}\n",
            index + 1,
            public_action_label(action)
        ));
    }
    view.push_str(
        "Choose one action_number with act, or call finish when your playtest is complete.",
    );
    if view.len() > MAX_PUBLIC_TRANSCRIPT_BYTES {
        return Err(CliError::new(
            "public player view exceeds the resource budget",
        ));
    }
    Ok(view)
}

fn public_action_label(action: &ActionView) -> String {
    let parameters = action
        .parameters
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join(", ");
    if parameters.is_empty() {
        format!("[{}] {}", action.category, action.label)
    } else {
        format!("[{}] {} — {parameters}", action.category, action.label)
    }
}

fn append_view(transcript: &mut String, heading: &str, view: &str) -> Result<(), CliError> {
    transcript.push_str(heading);
    transcript.push('\n');
    transcript.push_str(view);
    transcript.push_str("\n\n");
    if transcript.len() > MAX_PUBLIC_TRANSCRIPT_BYTES {
        return Err(CliError::new(
            "public player transcript exceeds the 16 MiB limit",
        ));
    }
    Ok(())
}

fn persist_public_session(
    config: &PlayerMcpConfig,
    session: &Session<'_>,
    transcript: &str,
) -> Result<(), CliError> {
    write_trace(&config.trace_path, session)?;
    atomic_write(&config.transcript_path, transcript.as_bytes())
}

fn tool_success(id: &Value, text: String) -> Value {
    rpc_result(
        id,
        json!({ "content": [{ "type": "text", "text": text }], "isError": false }),
    )
}

fn tool_error(id: &Value, message: &str) -> Value {
    rpc_result(
        id,
        json!({ "content": [{ "type": "text", "text": message }], "isError": true }),
    )
}

fn rpc_result(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_error(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn write_json_line<W: Write>(output: &mut W, value: &Value) -> Result<(), CliError> {
    serde_json::to_writer(&mut *output, value)
        .map_err(|_| CliError::new("could not encode player adapter output"))?;
    output.write_all(b"\n").map_err(super::io_error)?;
    output.flush().map_err(super::io_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_replay::{PlayerTrace, resume_player_trace};
    use std::fs;
    use std::io::Cursor;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEST: AtomicU64 = AtomicU64::new(0);

    fn test_config() -> PlayerMcpConfig {
        let nonce = NEXT_TEST.fetch_add(1, Ordering::Relaxed);
        let stem = format!("forge-player-mcp-{}-{nonce}", std::process::id());
        PlayerMcpConfig {
            character: "ilyan".to_owned(),
            seed: 71,
            trace_path: std::env::temp_dir().join(format!("{stem}.trace.json")),
            transcript_path: std::env::temp_dir().join(format!("{stem}.transcript.txt")),
            observation_canary: "blind-observation-0123456789abcdef".to_owned(),
            minimum_turns: 1,
            maximum_turns: 4,
            socket_path: None,
        }
    }

    fn call(id: u64, name: &str, arguments: Value) -> String {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        })
        .to_string()
    }

    fn cleanup(config: &PlayerMcpConfig) {
        let _ = fs::remove_file(&config.trace_path);
        let _ = fs::remove_file(&config.transcript_path);
        if let Some(path) = &config.socket_path {
            let _ = fs::remove_file(path);
        }
    }

    #[test]
    fn mcp_exposes_only_public_game_tools_and_writes_a_verifiable_trace() {
        let config = test_config();
        let input = [
            json!({
                "jsonrpc": "2.0",
                "id": 0,
                "method": "initialize",
                "params": { "protocolVersion": MCP_PROTOCOL_VERSION }
            })
            .to_string(),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }).to_string(),
            json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }).to_string(),
            call(2, "observe", json!({})),
            call(3, "act", json!({ "action_number": 1 })),
            call(4, "finish", json!({})),
        ]
        .join("\n")
            + "\n";
        let mut reader = Cursor::new(input.as_bytes());
        let mut output = Vec::new();
        run_player_mcp(&config, &mut reader, &mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        let responses: Vec<Value> = output
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(responses.len(), 5);
        let tool_names: Vec<_> = responses[1]["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        assert_eq!(tool_names, ["observe", "act", "finish"]);
        assert!(output.contains(&config.observation_canary));
        for forbidden in [
            "event_log",
            "scheduled_events",
            "\"entropy\"",
            "\"knowledge\"",
            "initial_state",
            "final_state_id",
        ] {
            assert!(!output.contains(forbidden));
        }

        let trace_json = fs::read_to_string(&config.trace_path).unwrap();
        let trace = PlayerTrace::from_json(&trace_json).unwrap();
        assert_eq!(trace.action_count(), 1);
        let content = load_content().unwrap();
        let resumed = resume_player_trace(&trace, &content).unwrap();
        assert_eq!(resumed.trace().steps.len(), 1);
        let transcript = fs::read_to_string(&config.transcript_path).unwrap();
        assert!(transcript.contains("Session finished after 1 action(s)."));
        assert!(transcript.contains(&config.observation_canary));
        cleanup(&config);
    }

    #[test]
    fn invalid_tool_input_is_inert_and_minimum_turns_are_enforced() {
        let config = test_config();
        let input = [
            call(0, "act", json!({ "action_number": 0 })),
            call(1, "act", json!({ "action_number": 999_999 })),
            call(2, "finish", json!({})),
        ]
        .join("\n")
            + "\n";
        let mut reader = Cursor::new(input.as_bytes());
        let mut output = Vec::new();
        assert!(run_player_mcp(&config, &mut reader, &mut output).is_err());
        let output = String::from_utf8(output).unwrap();
        let responses: Vec<Value> = output
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(responses.len(), 3);
        assert_eq!(responses[0]["result"]["isError"], true);
        assert_eq!(responses[1]["result"]["isError"], true);
        assert_eq!(responses[2]["result"]["isError"], true);
        let trace_json = fs::read_to_string(&config.trace_path).unwrap();
        assert_eq!(
            PlayerTrace::from_json(&trace_json).unwrap().action_count(),
            0
        );
        cleanup(&config);
    }

    #[test]
    fn adapter_arguments_and_json_transport_are_strict() {
        let strings = |values: &[&str]| -> Vec<String> {
            values.iter().map(|value| (*value).to_owned()).collect()
        };
        let parsed = PlayerMcpConfig::parse(&strings(&[
            "--character",
            "rook",
            "--seed",
            "9",
            "--trace",
            "/tmp/a.json",
            "--transcript",
            "/tmp/a.txt",
            "--canary",
            "blind-observation-abcdef",
            "--min-turns",
            "2",
            "--max-turns",
            "8",
            "--socket",
            "/tmp/player.sock",
        ]))
        .unwrap();
        assert_eq!(parsed.character, "rook");
        assert_eq!(parsed.minimum_turns, 2);
        assert_eq!(parsed.maximum_turns, 8);
        assert_eq!(parsed.socket_path, Some(PathBuf::from("/tmp/player.sock")));
        assert!(
            PlayerMcpConfig::parse(&strings(&["--character", "rook", "--character", "ilyan"]))
                .is_err()
        );

        let config = test_config();
        let duplicate = "{\"jsonrpc\":\"2.0\",\"id\":1,\"id\":2,\"method\":\"ping\"}\n";
        let mut reader = Cursor::new(duplicate.as_bytes());
        let mut output = Vec::new();
        assert!(run_player_mcp(&config, &mut reader, &mut output).is_err());
        assert!(output.is_empty());
        cleanup(&config);
    }

    #[cfg(unix)]
    #[test]
    fn unix_socket_transport_requires_and_records_an_explicit_finish() {
        use std::io::{Read, Write};
        use std::net::Shutdown;
        use std::os::unix::net::UnixStream;
        use std::thread;
        use std::time::Duration;

        let mut config = test_config();
        config.socket_path = Some(config.trace_path.with_extension("sock"));
        let worker_config = config.clone();
        let worker = thread::spawn(move || run_player_mcp_socket(&worker_config));
        let socket_path = config.socket_path.as_ref().unwrap();
        let mut stream = (0..100)
            .find_map(|_| match UnixStream::connect(socket_path) {
                Ok(stream) => Some(stream),
                Err(_) => {
                    thread::sleep(Duration::from_millis(1));
                    None
                }
            })
            .expect("player socket should become ready");
        let input = [
            call(1, "act", json!({ "action_number": 1 })),
            call(2, "finish", json!({})),
        ]
        .join("\n")
            + "\n";
        stream.write_all(input.as_bytes()).unwrap();
        stream.shutdown(Shutdown::Write).unwrap();
        let mut output = String::new();
        stream.read_to_string(&mut output).unwrap();
        worker.join().unwrap().unwrap();

        assert_eq!(output.lines().count(), 2);
        assert!(output.contains("Session finished after 1 action(s)."));
        assert!(
            fs::read_to_string(&config.transcript_path)
                .unwrap()
                .contains("Finish")
        );
        cleanup(&config);
    }
}
