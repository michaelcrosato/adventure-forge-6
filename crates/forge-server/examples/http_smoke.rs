//! Trusted real-socket driver for the local API, not blind or browser play.
//! The server runs in another process; only the final player-safe save prints.

use serde_json::{Value, json};
use std::{
    error::Error,
    io::{BufRead, BufReader, Read, Write},
    net::TcpStream,
    process::{Child, Command, Stdio},
    time::Duration,
};

struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn request(
    port: u16,
    token: Option<&str>,
    method: &str,
    path: &str,
    body: &str,
    discard: bool,
) -> Result<String, Box<dyn Error>> {
    let mut socket = TcpStream::connect(("127.0.0.1", port))?;
    socket.set_read_timeout(Some(Duration::from_secs(10)))?;
    socket.set_write_timeout(Some(Duration::from_secs(10)))?;
    let mut headers = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nSec-Fetch-Site: same-origin\r\nSec-Fetch-Dest: empty\r\nContent-Length: {}\r\n",
        body.len()
    );
    if method == "POST" {
        headers.push_str(&format!(
            "Origin: http://127.0.0.1:{port}\r\nContent-Type: application/json\r\n"
        ));
    }
    if let Some(token) = token {
        headers.push_str(&format!("Authorization: Bearer {token}\r\n"));
    }
    headers.push_str("\r\n");
    socket.write_all(headers.as_bytes())?;
    socket.write_all(body.as_bytes())?;
    let mut reader = BufReader::new(socket);
    let mut status = String::new();
    reader.read_line(&mut status)?;
    if !status.starts_with("HTTP/1.1 200 ") {
        return Err("HTTP smoke request failed".into());
    }
    let mut length = None;
    let mut no_store = false;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Err("truncated HTTP headers".into());
        }
        if line == "\r\n" {
            break;
        }
        let lower = line.to_ascii_lowercase();
        if let Some(value) = lower.strip_prefix("content-length:") {
            length = Some(value.trim().parse::<usize>()?);
        }
        if lower.trim() == "cache-control: no-store" {
            no_store = true;
        }
        if lower.starts_with("access-control-") {
            return Err("unexpected CORS header".into());
        }
    }
    if !no_store {
        return Err("missing no-store response".into());
    }
    let length = length.ok_or("missing bounded response length")?;
    if length > 2 * 1024 * 1024 {
        return Err("oversized smoke response".into());
    }
    // The server has accepted the action and sent its response headers. Drop
    // the connection without consuming/decoding the acknowledgment body.
    if discard {
        return Ok(String::new());
    }
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes)?;
    Ok(String::from_utf8(bytes)?)
}

fn call(
    port: u16,
    token: &str,
    method: &str,
    path: &str,
    body: &Value,
) -> Result<Value, Box<dyn Error>> {
    let body = if method == "GET" {
        String::new()
    } else {
        serde_json::to_string(body)?
    };
    Ok(serde_json::from_str(&request(
        port,
        Some(token),
        method,
        path,
        &body,
        false,
    )?)?)
}

fn main() -> Result<(), Box<dyn Error>> {
    let binary = std::env::args()
        .nth(1)
        .ok_or("expected trusted server binary path")?;
    let mut server = Server(
        Command::new(binary)
            .args(["--port", "0"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?,
    );
    let mut output = BufReader::new(server.0.stdout.take().ok_or("server stdout unavailable")?);
    let mut line = String::new();
    output.read_line(&mut line)?;
    let port: u16 = line
        .trim()
        .strip_prefix("Adventure Forge local API: http://127.0.0.1:")
        .ok_or("invalid server startup")?
        .parse()?;
    let bootstrap: Value =
        serde_json::from_str(&request(port, None, "GET", "/api/bootstrap", "", false)?)?;
    let token = bootstrap["token"].as_str().ok_or("missing process token")?;
    let creation = json!({"creation_id":"socket-create", "start":{"kind":"preset","character_preset_id":"rook","seed":"71"}});
    let mut created = call(port, token, "POST", "/api/sessions", &creation)?;
    if call(port, token, "POST", "/api/sessions", &creation)? != created {
        return Err("creation retry changed".into());
    }
    let mut id = created["session_id"]
        .as_str()
        .ok_or("missing session")?
        .to_owned();
    let recipe = [
        ("travel_adjacent", Some("lowsail.docks")),
        ("docks.ring_warning", None),
        ("docks.rig_towline", None),
        ("levee.relay_warning", None),
        ("levee.culvert_path", None),
        ("floor.open_relief", None),
        ("travel_adjacent", Some("red_sluice.top")),
        ("top.divert_relief", None),
        ("world.enter_aftermath", None),
        ("return.move_inland", None),
    ];
    for (index, (definition, destination)) in recipe.into_iter().enumerate() {
        let before = call(
            port,
            token,
            "GET",
            &format!("/api/sessions/{id}"),
            &Value::Null,
        )?;
        let mut page = before["catalog"].clone();
        let action_id = loop {
            if let Some(action) = page["actions"]
                .as_array()
                .ok_or("missing actions")?
                .iter()
                .find(|action| {
                    action["definition_id"] == definition
                        && destination
                            .is_none_or(|target| action["parameters"]["destination"] == target)
                })
            {
                break action["action_id"]
                    .as_str()
                    .ok_or("missing identity")?
                    .to_owned();
            }
            let offset = page["next_offset"]
                .as_str()
                .ok_or("reviewed action unavailable")?;
            page = call(
                port,
                token,
                "POST",
                &format!("/api/sessions/{id}/catalog"),
                &json!({"expected_state_id":before["catalog"]["state_id"],"offset":offset,"page_size":"32"}),
            )?;
        };
        let command = json!({"command_id":format!("socket-action-{index}"),"expected_revision":before["revision"],"expected_state_id":before["catalog"]["state_id"],"action_id":action_id});
        let path = format!("/api/sessions/{id}/actions");
        if index == 2 {
            request(
                port,
                Some(token),
                "POST",
                &path,
                &serde_json::to_string(&command)?,
                true,
            )?;
        }
        let accepted = call(port, token, "POST", &path, &command)?;
        if accepted["revision"]
            .as_str()
            .and_then(|value| value.parse::<usize>().ok())
            != Some(index + 1)
            || call(port, token, "POST", &path, &command)? != accepted
        {
            return Err("action retry advanced or changed".into());
        }
        if index == 2 {
            let checkpoint = request(
                port,
                Some(token),
                "GET",
                &format!("/api/sessions/{id}/save"),
                "",
                false,
            )?;
            call(
                port,
                token,
                "POST",
                &format!("/api/sessions/{id}/close"),
                &json!({}),
            )?;
            if call(port, token, "POST", &path, &command)? != accepted {
                return Err("closed retry changed".into());
            }
            let resume = json!({"creation_id":"socket-resume", "save_json":checkpoint});
            created = call(port, token, "POST", "/api/resume", &resume)?;
            if created["view"] != accepted
                || call(port, token, "POST", "/api/resume", &resume)? != created
            {
                return Err("resume or retry changed".into());
            }
            id = created["session_id"]
                .as_str()
                .ok_or("missing resumed session")?
                .to_owned();
        }
    }
    println!(
        "{}",
        request(
            port,
            Some(token),
            "GET",
            &format!("/api/sessions/{id}/save"),
            "",
            false
        )?
    );
    Ok(())
}
