use async_trait::async_trait;

use crate::error::AppResult;

#[async_trait]
pub(crate) trait LocalRunPort<TAdapterArgs, TOutcome> {
    async fn run_local(&self, adapter_args: TAdapterArgs) -> AppResult<TOutcome>;
}

#[async_trait]
pub(crate) trait ReplayRunPort<TAdapterArgs> {
    async fn run_replay(&self, adapter_args: TAdapterArgs) -> AppResult<()>;
}

#[async_trait]
pub(crate) trait CleanupPort<TCleanupArgs> {
    async fn run_cleanup(&self, cleanup_args: TCleanupArgs) -> AppResult<()>;
}

#[async_trait]
pub(crate) trait ComparePort<TCompareArgs> {
    async fn run_compare(&self, compare_args: TCompareArgs) -> AppResult<()>;
}

pub(crate) trait ServicePort<TAdapterArgs> {
    fn handle_service_action(&self, adapter_args: TAdapterArgs) -> AppResult<()>;
}

pub(crate) async fn execute_local<TPort, TAdapterArgs, TOutcome>(
    adapter_args: TAdapterArgs,
    local_port: &TPort,
) -> AppResult<TOutcome>
where
    TPort: LocalRunPort<TAdapterArgs, TOutcome> + Sync,
{
    local_port.run_local(adapter_args).await
}

pub(crate) async fn execute_replay<TPort, TAdapterArgs>(
    adapter_args: TAdapterArgs,
    replay_port: &TPort,
) -> AppResult<()>
where
    TPort: ReplayRunPort<TAdapterArgs> + Sync,
{
    replay_port.run_replay(adapter_args).await
}

pub(crate) async fn execute_cleanup<TPort, TCleanupArgs>(
    cleanup_args: TCleanupArgs,
    cleanup_port: &TPort,
) -> AppResult<()>
where
    TPort: CleanupPort<TCleanupArgs> + Sync,
{
    cleanup_port.run_cleanup(cleanup_args).await
}

pub(crate) async fn execute_compare<TPort, TCompareArgs>(
    compare_args: TCompareArgs,
    compare_port: &TPort,
) -> AppResult<()>
where
    TPort: ComparePort<TCompareArgs> + Sync,
{
    compare_port.run_compare(compare_args).await
}

pub(crate) fn execute_service<TPort, TAdapterArgs>(
    adapter_args: TAdapterArgs,
    service_port: &TPort,
) -> AppResult<()>
where
    TPort: ServicePort<TAdapterArgs>,
{
    service_port.handle_service_action(adapter_args)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    use crate::error::AppResult;

    use super::{
        CleanupPort, ComparePort, LocalRunPort, ReplayRunPort, ServicePort, execute_cleanup,
        execute_compare, execute_local, execute_replay, execute_service,
    };

    struct FakeLocalPort {
        called: AtomicBool,
        seen: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl LocalRunPort<String, usize> for FakeLocalPort {
        async fn run_local(&self, adapter_args: String) -> AppResult<usize> {
            self.called.store(true, Ordering::SeqCst);
            if let Ok(mut seen) = self.seen.lock() {
                seen.push(adapter_args.clone());
            }
            Ok(adapter_args.len())
        }
    }

    struct FakeReplayPort {
        called: AtomicBool,
        seen: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl ReplayRunPort<String> for FakeReplayPort {
        async fn run_replay(&self, adapter_args: String) -> AppResult<()> {
            self.called.store(true, Ordering::SeqCst);
            if let Ok(mut seen) = self.seen.lock() {
                seen.push(adapter_args);
            }
            Ok(())
        }
    }

    struct FakeCleanupPort {
        called: AtomicBool,
        seen: Arc<Mutex<Vec<u64>>>,
    }

    #[async_trait::async_trait]
    impl CleanupPort<u64> for FakeCleanupPort {
        async fn run_cleanup(&self, cleanup_args: u64) -> AppResult<()> {
            self.called.store(true, Ordering::SeqCst);
            if let Ok(mut seen) = self.seen.lock() {
                seen.push(cleanup_args);
            }
            Ok(())
        }
    }

    struct FakeComparePort {
        called: AtomicBool,
        seen: Arc<Mutex<Vec<u64>>>,
    }

    #[async_trait::async_trait]
    impl ComparePort<u64> for FakeComparePort {
        async fn run_compare(&self, compare_args: u64) -> AppResult<()> {
            self.called.store(true, Ordering::SeqCst);
            if let Ok(mut seen) = self.seen.lock() {
                seen.push(compare_args);
            }
            Ok(())
        }
    }

    struct FakeServicePort {
        called: AtomicBool,
        seen: Arc<Mutex<Vec<String>>>,
    }

    impl ServicePort<String> for FakeServicePort {
        fn handle_service_action(&self, adapter_args: String) -> AppResult<()> {
            self.called.store(true, Ordering::SeqCst);
            if let Ok(mut seen) = self.seen.lock() {
                seen.push(adapter_args);
            }
            Ok(())
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn execute_local_calls_port() -> AppResult<()> {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let port = FakeLocalPort {
            called: AtomicBool::new(false),
            seen: seen.clone(),
        };

        let len = execute_local("local".to_owned(), &port).await?;

        if !port.called.load(Ordering::SeqCst) {
            return Err(crate::error::AppError::validation(
                "expected local port to be called",
            ));
        }
        if len != 5 {
            return Err(crate::error::AppError::validation(
                "expected local outcome to be returned",
            ));
        }
        if let Ok(seen) = seen.lock()
            && seen.as_slice() != ["local"]
        {
            return Err(crate::error::AppError::validation(
                "expected local args to be forwarded",
            ));
        }

        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn execute_replay_calls_port() -> AppResult<()> {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let port = FakeReplayPort {
            called: AtomicBool::new(false),
            seen: seen.clone(),
        };

        execute_replay("replay".to_owned(), &port).await?;

        if !port.called.load(Ordering::SeqCst) {
            return Err(crate::error::AppError::validation(
                "expected replay port to be called",
            ));
        }
        if let Ok(seen) = seen.lock()
            && seen.as_slice() != ["replay"]
        {
            return Err(crate::error::AppError::validation(
                "expected replay args to be forwarded",
            ));
        }

        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn execute_cleanup_calls_port() -> AppResult<()> {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let port = FakeCleanupPort {
            called: AtomicBool::new(false),
            seen: seen.clone(),
        };

        execute_cleanup(42, &port).await?;

        if !port.called.load(Ordering::SeqCst) {
            return Err(crate::error::AppError::validation(
                "expected cleanup port to be called",
            ));
        }
        if let Ok(seen) = seen.lock()
            && seen.as_slice() != [42]
        {
            return Err(crate::error::AppError::validation(
                "expected cleanup args to be forwarded",
            ));
        }

        Ok(())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn execute_compare_calls_port() -> AppResult<()> {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let port = FakeComparePort {
            called: AtomicBool::new(false),
            seen: seen.clone(),
        };

        execute_compare(77, &port).await?;

        if !port.called.load(Ordering::SeqCst) {
            return Err(crate::error::AppError::validation(
                "expected compare port to be called",
            ));
        }
        if let Ok(seen) = seen.lock()
            && seen.as_slice() != [77]
        {
            return Err(crate::error::AppError::validation(
                "expected compare args to be forwarded",
            ));
        }

        Ok(())
    }

    #[test]
    fn execute_service_calls_port() -> AppResult<()> {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let port = FakeServicePort {
            called: AtomicBool::new(false),
            seen: seen.clone(),
        };

        execute_service("service".to_owned(), &port)?;

        if !port.called.load(Ordering::SeqCst) {
            return Err(crate::error::AppError::validation(
                "expected service port to be called",
            ));
        }
        if let Ok(seen) = seen.lock()
            && seen.as_slice() != ["service"]
        {
            return Err(crate::error::AppError::validation(
                "expected service args to be forwarded",
            ));
        }

        Ok(())
    }
}
