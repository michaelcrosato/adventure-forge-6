use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::process::{self, Command};
use std::thread;
use std::time::{Duration, Instant};

const PR_GET_NO_NEW_PRIVS: i32 = 39;

unsafe extern "C" {
    fn getuid() -> u32;
    fn getgid() -> u32;
    fn prctl(option: i32, ...) -> i32;
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let result = match args.get(1).map(String::as_str) {
        Some("listen") => listen(&args),
        Some("probe") => probe(&args),
        Some("memory") => memory_probe(),
        Some("files") => file_descriptor_probe(),
        Some("processes") => process_probe(),
        Some("park") => park_probe(),
        Some("flood") => output_probe(),
        Some("sleep") => sleep_probe(),
        _ => Err("unknown probe mode"),
    };
    if let Err(message) = result {
        eprintln!("locked-boundary probe failed: {message}");
        process::exit(1);
    }
}

fn listen(args: &[String]) -> Result<(), &'static str> {
    let port_path = args.get(2).ok_or("listener needs a port path")?;
    let connection_path = args.get(3).ok_or("listener needs a result path")?;
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|_| "listener bind failed")?;
    listener
        .set_nonblocking(true)
        .map_err(|_| "listener setup failed")?;
    let port = listener
        .local_addr()
        .map_err(|_| "listener address failed")?
        .port();
    fs::write(port_path, port.to_string()).map_err(|_| "listener port write failed")?;

    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        match listener.accept() {
            Ok(_) => {
                fs::write(connection_path, b"connected")
                    .map_err(|_| "listener result write failed")?;
                return Ok(());
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(_) => return Err("listener accept failed"),
        }
    }
    Ok(())
}

fn probe(args: &[String]) -> Result<(), &'static str> {
    let canary_path = args.get(2).ok_or("probe needs a canary path")?;
    let port = args
        .get(3)
        .ok_or("probe needs a port")?
        .parse::<u16>()
        .map_err(|_| "probe port is invalid")?;

    if env::current_dir().map_err(|_| "working directory unavailable")? != Path::new("/session") {
        return Err("working directory escaped /session");
    }

    let mut actual_environment: Vec<_> = env::vars().collect();
    actual_environment.sort();
    let mut expected_environment = vec![
        ("HOME".to_owned(), "/nonexistent".to_owned()),
        ("PATH".to_owned(), "/nonexistent".to_owned()),
        ("PWD".to_owned(), "/session".to_owned()),
        ("RUST_BACKTRACE".to_owned(), "0".to_owned()),
        ("RUST_LOG".to_owned(), "off".to_owned()),
    ];
    expected_environment.sort();
    if actual_environment != expected_environment {
        return Err("environment was not cleared");
    }

    // SAFETY: these libc calls take no pointers and only inspect this process.
    let (uid, gid, no_new_privileges) = unsafe {
        (
            getuid(),
            getgid(),
            prctl(PR_GET_NO_NEW_PRIVS, 0_u64, 0_u64, 0_u64, 0_u64),
        )
    };
    if uid != 65_534 || gid != 65_534 {
        return Err("sandbox identity is not nobody");
    }
    if no_new_privileges != 1 {
        return Err("no_new_privs is not active");
    }

    if File::open(canary_path).is_ok() {
        return Err("host canary became readable");
    }
    for path in ["/escape", "/bundle/escape", "/nonexistent/escape"] {
        if OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .is_ok()
        {
            return Err("write escaped /session");
        }
    }

    let session_probe = "/session/write-probe";
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(session_probe)
        .map_err(|_| "/session is not writable")?;
    file.write_all(b"bounded")
        .map_err(|_| "/session write failed")?;
    drop(file);
    fs::remove_file(session_probe).map_err(|_| "/session cleanup failed")?;

    if Command::new("/bin/sh").status().is_ok() {
        return Err("a shell is executable inside the sandbox");
    }

    let address = SocketAddr::from(([127, 0, 0, 1], port));
    if TcpStream::connect_timeout(&address, Duration::from_millis(250)).is_ok() {
        return Err("host network canary was reachable");
    }

    println!("locked-boundary-probe-v1: pass");
    Ok(())
}

fn sleep_probe() -> Result<(), &'static str> {
    thread::sleep(Duration::from_secs(30));
    Err("wall-clock limit did not terminate the probe")
}

fn memory_probe() -> Result<(), &'static str> {
    let mut allocation = Vec::<u8>::new();
    if allocation.try_reserve_exact(300 * 1024 * 1024).is_ok() {
        return Err("address-space limit allowed a 300 MiB reservation");
    }
    println!("memory-limit-probe-v1: pass");
    Ok(())
}

fn file_descriptor_probe() -> Result<(), &'static str> {
    let mut files = Vec::new();
    for _ in 0..512 {
        match File::open("/bundle/program") {
            Ok(file) => files.push(file),
            Err(_) => {
                println!("file-limit-probe-v1: pass");
                return Ok(());
            }
        }
    }
    Err("file-descriptor limit allowed 128 extra files")
}

fn output_probe() -> Result<(), &'static str> {
    let chunk = [b'x'; 8 * 1024];
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    for _ in 0..512 {
        if output.write_all(&chunk).is_err() {
            return Ok(());
        }
    }
    Err("output limit allowed four MiB")
}

fn process_probe() -> Result<(), &'static str> {
    let mut children = Vec::new();
    let mut limit_reached = false;
    for _ in 0..128 {
        match Command::new("/bundle/runtime/loader")
            .args([
                "--library-path",
                "/bundle/runtime",
                "/bundle/program",
                "park",
            ])
            .spawn()
        {
            Ok(child) => children.push(child),
            Err(_) => {
                limit_reached = true;
                break;
            }
        }
    }
    for child in &mut children {
        let _ = child.kill();
        let _ = child.wait();
    }
    if !limit_reached {
        return Err("process limit allowed 512 children");
    }
    println!("process-limit-probe-v1: pass");
    Ok(())
}

fn park_probe() -> Result<(), &'static str> {
    thread::sleep(Duration::from_secs(30));
    Ok(())
}
