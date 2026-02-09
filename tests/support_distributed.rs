use std::ffi::OsStr;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

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

/// Spawn a test server or skip when socket permissions are unavailable.
///
/// # Errors
///
/// Returns an error if the server fails for reasons other than insufficient
/// socket permissions.
pub fn spawn_http_server_or_skip() -> Result<Option<(String, ServerHandle)>, String> {
    match spawn_http_server() {
        Ok(result) => Ok(Some(result)),
        Err(err) if err.contains("Operation not permitted") => {
            eprintln!("Skipping e2e test: {}", err);
            Ok(None)
        }
        Err(err) => Err(err),
    }
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

/// Spawn the `strest` binary.
///
/// # Errors
///
/// Returns an error if the process cannot be started.
pub fn spawn_strest<I, S>(args: I) -> Result<Child, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let bin = strest_bin()?;
    Command::new(bin)
        .args(args)
        .env("RUST_LOG", "error")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("spawn strest failed: {}", err))
}

/// Spawn the `strest` binary and capture output.
///
/// # Errors
///
/// Returns an error if the process cannot be started.
pub fn spawn_strest_with_output<I, S>(args: I) -> Result<Child, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let bin = strest_bin()?;
    Command::new(bin)
        .args(args)
        .env("RUST_LOG", "error")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("spawn strest failed: {}", err))
}

/// Wait for a child process to exit.
///
/// # Errors
///
/// Returns an error if waiting fails or the timeout is exceeded.
pub fn wait_for_exit(child: &mut Child, timeout: Duration) -> Result<ExitStatus, String> {
    let start = Instant::now();
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|err| format!("wait failed: {}", err))?
        {
            return Ok(status);
        }
        if start.elapsed() > timeout {
            drop(child.kill());
            return Err("process timed out".to_owned());
        }
        thread::sleep(Duration::from_millis(50));
    }
}

/// Read captured stdout/stderr from a child.
///
/// # Errors
///
/// Returns an error if the streams cannot be read.
pub fn read_child_output(child: &mut Child) -> Result<(String, String), String> {
    let mut stdout = String::new();
    if let Some(mut out) = child.stdout.take() {
        out.read_to_string(&mut stdout)
            .map_err(|err| format!("read stdout failed: {}", err))?;
    }
    let mut stderr = String::new();
    if let Some(mut err_out) = child.stderr.take() {
        err_out
            .read_to_string(&mut stderr)
            .map_err(|err| format!("read stderr failed: {}", err))?;
    }
    Ok((stdout, stderr))
}

/// Pick an available local TCP port.
///
/// # Errors
///
/// Returns an error if a local port cannot be allocated.
pub fn pick_port() -> Result<u16, String> {
    TcpListener::bind("127.0.0.1:0")
        .map_err(|err| format!("bind port failed: {}", err))?
        .local_addr()
        .map_err(|err| format!("port addr failed: {}", err))
        .map(|addr| addr.port())
}

fn strest_bin() -> Result<String, String> {
    option_env!("CARGO_BIN_EXE_strest").map_or_else(
        || Err("CARGO_BIN_EXE_strest missing at compile time.".to_owned()),
        |path| Ok(path.to_owned()),
    )
}
