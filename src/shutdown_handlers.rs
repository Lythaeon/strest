use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, poll, read};
use tokio::sync::broadcast;

use crate::shutdown::{ShutdownReceiver, ShutdownSender};

#[cfg(unix)]
use tokio::signal::unix::{SignalKind, signal};

/// Broadcast channel size for shutdown notifications (single signal fan-out).
const SHUTDOWN_CHANNEL_CAPACITY: usize = 1;
/// Keyboard polling interval for Ctrl+C detection in TTY mode.
const KEYBOARD_POLL_INTERVAL: Duration = Duration::from_millis(100);

#[must_use]
pub fn shutdown_channel() -> (ShutdownSender, ShutdownReceiver) {
    broadcast::channel::<()>(SHUTDOWN_CHANNEL_CAPACITY)
}

pub fn setup_keyboard_shutdown_handler(
    shutdown_tx: &ShutdownSender,
) -> tokio::task::JoinHandle<()> {
    let shutdown_tx = shutdown_tx.clone();
    let mut shutdown_rx = shutdown_tx.subscribe();

    tokio::task::spawn_blocking(move || {
        loop {
            match shutdown_rx.try_recv() {
                Ok(()) => break,
                Err(broadcast::error::TryRecvError::Closed) => break,
                Err(broadcast::error::TryRecvError::Empty) => {}
                Err(broadcast::error::TryRecvError::Lagged(_)) => {}
            }

            let has_event = poll(KEYBOARD_POLL_INTERVAL).unwrap_or_default();

            if has_event
                && let Ok(Event::Key(KeyEvent {
                    code: KeyCode::Char('c'),
                    modifiers: KeyModifiers::CONTROL,
                    ..
                })) = read()
            {
                drop(shutdown_tx.send(()));
                break;
            }
        }
    })
}

pub fn setup_signal_shutdown_handler(shutdown_tx: &ShutdownSender) -> tokio::task::JoinHandle<()> {
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
                    drop(shutdown_tx.send(()));
                }
                () = async {
                    if let Some(signal) = term_signal.as_mut() {
                        signal.recv().await;
                    } else {
                        std::future::pending::<()>().await;
                    }
                } => {
                    drop(shutdown_tx.send(()));
                }
            }
        }

        #[cfg(not(unix))]
        {
            tokio::select! {
                _ = shutdown_rx.recv() => {}
                _ = tokio::signal::ctrl_c() => {
                    drop(shutdown_tx.send(()));
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{AppError, AppResult};
    use std::future::Future;
    use std::time::Duration;

    const SIGNAL_HANDLER_SETTLE: Duration = Duration::from_millis(10);
    const SHUTDOWN_HANDLER_TIMEOUT: Duration = Duration::from_secs(1);

    fn run_async_test<F>(future: F) -> AppResult<()>
    where
        F: Future<Output = AppResult<()>>,
    {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| AppError::validation(format!("Failed to build runtime: {}", err)))?;
        runtime.block_on(future)
    }

    #[test]
    fn shutdown_handler_exits_on_signal() -> AppResult<()> {
        run_async_test(async {
            let (shutdown_tx, _) = shutdown_channel();
            let handle = setup_keyboard_shutdown_handler(&shutdown_tx);

            if shutdown_tx.send(()).is_err() {
                return Err(AppError::validation("Failed to send shutdown"));
            }

            tokio::time::timeout(SHUTDOWN_HANDLER_TIMEOUT, handle)
                .await
                .map_err(|err| {
                    AppError::validation(format!("Timed out waiting for shutdown handler: {}", err))
                })?
                .map_err(|err| {
                    AppError::validation(format!("Shutdown task join error: {}", err))
                })?;
            Ok(())
        })
    }

    #[test]
    fn signal_handler_exits_on_shutdown() -> AppResult<()> {
        run_async_test(async {
            let (shutdown_tx, _) = shutdown_channel();
            let handle = setup_signal_shutdown_handler(&shutdown_tx);

            tokio::time::sleep(SIGNAL_HANDLER_SETTLE).await;
            if shutdown_tx.send(()).is_err() {
                return Err(AppError::validation("Failed to send shutdown"));
            }

            tokio::time::timeout(SHUTDOWN_HANDLER_TIMEOUT, handle)
                .await
                .map_err(|err| {
                    AppError::validation(format!("Timed out waiting for shutdown handler: {}", err))
                })?
                .map_err(|err| {
                    AppError::validation(format!("Shutdown task join error: {}", err))
                })?;
            Ok(())
        })
    }
}
