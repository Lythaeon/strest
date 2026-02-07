use std::ffi::OsStr;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::process::{Command, Output};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

pub struct ServerHandle {
    shutdown: mpsc::Sender<()>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        let _send_result = self.shutdown.send(());
        if let Some(handle) = self.thread.take() {
            drop(handle.join());
        }
    }
}

/// Spawn a lightweight HTTP server for tests.
///
/// # Errors
///
/// Returns an error if the listener cannot be created or configured.
pub fn spawn_http_server() -> Result<(String, ServerHandle), String> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .map_err(|err| format!("bind test server failed: {}", err))?;
    let addr = listener
        .local_addr()
        .map_err(|err| format!("server addr failed: {}", err))?;
    listener
        .set_nonblocking(true)
        .map_err(|err| format!("set_nonblocking failed: {}", err))?;

    let (shutdown_tx, shutdown_rx) = mpsc::channel();

    let handle = thread::spawn(move || {
        loop {
            if shutdown_rx.try_recv().is_ok() {
                break;
            }

            match listener.accept() {
                Ok((stream, _)) => {
                    thread::spawn(move || handle_client(stream));
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });

    Ok((
        format!("http://{}", addr),
        ServerHandle {
            shutdown: shutdown_tx,
            thread: Some(handle),
        },
    ))
}

fn handle_client(mut stream: TcpStream) {
    let mut buffer = [0u8; 1024];
    if stream.read(&mut buffer).is_err() {
        return;
    }
    if stream
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK")
        .is_err()
    {
        return;
    }
    if stream.flush().is_err() {
        return;
    }
    drop(stream.shutdown(Shutdown::Both));
}

/// Run the `strest` binary and capture output.
///
/// # Errors
///
/// Returns an error if the binary cannot be executed.
pub fn run_strest<I, S>(args: I) -> Result<Output, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let bin = strest_bin()?;
    Command::new(bin)
        .args(args)
        .env("RUST_LOG", "error")
        .output()
        .map_err(|err| format!("run strest failed: {}", err))
}

fn strest_bin() -> Result<String, String> {
    option_env!("CARGO_BIN_EXE_strest").map_or_else(
        || Err("CARGO_BIN_EXE_strest missing at compile time.".to_owned()),
        |path| Ok(path.to_owned()),
    )
}
