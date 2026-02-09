#![no_main]

use libfuzzer_sys::fuzz_target;
use std::cell::RefCell;
use std::io::{Seek, SeekFrom, Write};

thread_local! {
    static RUNTIME: RefCell<Option<tokio::runtime::Runtime>> = RefCell::new(None);
    static TMPFILE: RefCell<Option<tempfile::NamedTempFile>> = RefCell::new(None);
}

fn with_runtime<F>(action: F)
where
    F: FnOnce(&tokio::runtime::Runtime),
{
    RUNTIME.with(|cell| {
        if cell.borrow().is_none() {
            if let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                *cell.borrow_mut() = Some(runtime);
            }
        }

        if let Some(runtime) = cell.borrow().as_ref() {
            action(runtime);
        }
    });
}

fuzz_target!(|data: &[u8]| {
    if data.len() > 1_000_000 {
        return;
    }

    let Some(path) = TMPFILE.with(|cell| {
        let mut file_slot = cell.borrow_mut();
        if file_slot.is_none() {
            *file_slot = tempfile::NamedTempFile::new().ok();
        }
        let file = file_slot.as_mut()?;
        let handle = file.as_file_mut();
        if handle.seek(SeekFrom::Start(0)).is_err() {
            return None;
        }
        if handle.set_len(0).is_err() {
            return None;
        }
        if handle.write_all(data).is_err() {
            return None;
        }
        if handle.flush().is_err() {
            return None;
        }
        Some(file.path().to_path_buf())
    }) else {
        return;
    };

    with_runtime(|runtime| {
        let _ = runtime.block_on(async {
            let result =
                strest::metrics::read_metrics_log(&path, 200, &None, 10_000, None).await;
            if let Ok(log) = result {
                let summary = &log.summary;
                debug_assert_eq!(
                    summary.error_requests,
                    summary.total_requests.saturating_sub(summary.successful_requests)
                );
                if summary.total_requests > 0 {
                    debug_assert!(summary.min_latency_ms <= summary.max_latency_ms);
                }
                if summary.successful_requests > 0 {
                    debug_assert!(summary.success_min_latency_ms <= summary.success_max_latency_ms);
                }
            }
            result
        });
    });
});
