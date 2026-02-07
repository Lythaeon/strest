use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, poll, read};
use tokio::sync::broadcast;

#[cfg(unix)]
use tokio::signal::unix::{SignalKind, signal};

pub fn setup_keyboard_shutdown_handler(
    shutdown_tx: &broadcast::Sender<u16>,
) -> tokio::task::JoinHandle<()> {
    let shutdown_tx = shutdown_tx.clone();
    let mut shutdown_rx = shutdown_tx.subscribe();

    tokio::task::spawn_blocking(move || {
        loop {
            match shutdown_rx.try_recv() {
                Ok(_) => break,
                Err(broadcast::error::TryRecvError::Closed) => break,
                Err(broadcast::error::TryRecvError::Empty) => {}
                Err(broadcast::error::TryRecvError::Lagged(_)) => {}
            }

            let has_event = poll(Duration::from_millis(100)).unwrap_or_default();

            if has_event
                && let Ok(Event::Key(KeyEvent {
                    code: KeyCode::Char('c'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                })) = read()
            {
                drop(shutdown_tx.send(1));
                break;
            }
        }
    })
}

pub fn setup_signal_shutdown_handler(
    shutdown_tx: &broadcast::Sender<u16>,
) -> tokio::task::JoinHandle<()> {
    let shutdown_tx = shutdown_tx.clone();
    tokio::spawn(async move {
        let mut shutdown_rx = shutdown_tx.subscribe();

        #[cfg(unix)]
        let mut term_signal = match signal(SignalKind::terminate()) {
            Ok(signal) => Some(signal),
            Err(err) => {
                eprintln!("Failed to register SIGTERM handler: {}", err);
                None
            }
        };

        #[cfg(unix)]
        {
            tokio::select! {
                _ = shutdown_rx.recv() => {}
                _ = tokio::signal::ctrl_c() => {
                    drop(shutdown_tx.send(1));
                }
                () = async {
                    if let Some(signal) = term_signal.as_mut() {
                        signal.recv().await;
                    } else {
                        std::future::pending::<()>().await;
                    }
                } => {
                    drop(shutdown_tx.send(1));
                }
            }
        }

        #[cfg(not(unix))]
        {
            tokio::select! {
                _ = shutdown_rx.recv() => {}
                _ = tokio::signal::ctrl_c() => {
                    drop(shutdown_tx.send(1));
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::time::Duration;

    fn run_async_test<F>(future: F) -> Result<(), String>
    where
        F: Future<Output = Result<(), String>>,
    {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| format!("Failed to build runtime: {}", err))?;
        runtime.block_on(future)
    }

    #[test]
    fn shutdown_handler_exits_on_signal() -> Result<(), String> {
        run_async_test(async {
            let (shutdown_tx, _) = broadcast::channel::<u16>(1);
            let handle = setup_keyboard_shutdown_handler(&shutdown_tx);

            if shutdown_tx.send(1).is_err() {
                return Err("Failed to send shutdown".to_owned());
            }

            tokio::time::timeout(Duration::from_secs(1), handle)
                .await
                .map_err(|err| format!("Timed out waiting for shutdown handler: {}", err))?
                .map_err(|err| format!("Shutdown task join error: {}", err))?;
            Ok(())
        })
    }

    #[test]
    fn signal_handler_exits_on_shutdown() -> Result<(), String> {
        run_async_test(async {
            let (shutdown_tx, _) = broadcast::channel::<u16>(1);
            let handle = setup_signal_shutdown_handler(&shutdown_tx);

            tokio::time::sleep(Duration::from_millis(10)).await;
            if shutdown_tx.send(1).is_err() {
                return Err("Failed to send shutdown".to_owned());
            }

            tokio::time::timeout(Duration::from_secs(1), handle)
                .await
                .map_err(|err| format!("Timed out waiting for shutdown handler: {}", err))?
                .map_err(|err| format!("Shutdown task join error: {}", err))?;
            Ok(())
        })
    }
}
