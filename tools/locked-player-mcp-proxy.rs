use std::io::{self, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::process::ExitCode;
use std::thread;

const SOCKET_PATH: &str = "/work/player.sock";

fn run() -> io::Result<()> {
    let mut response_stream = UnixStream::connect(SOCKET_PATH)?;
    let mut request_stream = response_stream.try_clone()?;
    let request_copy = thread::spawn(move || -> io::Result<()> {
        let stdin = io::stdin();
        io::copy(&mut stdin.lock(), &mut request_stream)?;
        request_stream.shutdown(Shutdown::Write)
    });

    let stdout = io::stdout();
    let mut output = stdout.lock();
    io::copy(&mut response_stream, &mut output)?;
    output.flush()?;
    request_copy
        .join()
        .map_err(|_| io::Error::other("request forwarding stopped"))??;
    Ok(())
}

fn main() -> ExitCode {
    if run().is_ok() {
        ExitCode::SUCCESS
    } else {
        eprintln!("player transport error");
        ExitCode::from(2)
    }
}
