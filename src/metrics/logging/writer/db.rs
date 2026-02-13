use tokio_rusqlite::Connection;

use crate::error::{AppError, AppResult, MetricsError};

#[derive(Debug, Clone, Copy)]
pub(super) struct DbRecord {
    pub(super) elapsed_ms: u64,
    pub(super) latency_ms: u64,
    pub(super) status_code: u16,
    pub(super) timed_out: bool,
    pub(super) transport_error: bool,
}

pub(super) const DB_FLUSH_SIZE: usize = 500;

pub(super) async fn flush_db_records(
    conn: &Connection,
    buffer: &mut Vec<DbRecord>,
) -> AppResult<()> {
    if buffer.is_empty() {
        return Ok(());
    }

    let records = std::mem::take(buffer);
    conn.call(move |conn| {
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare(
                "INSERT INTO metrics (elapsed_ms, latency_ms, status_code, timed_out, transport_error)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for record in records {
                stmt.execute(rusqlite::params![
                    clamp_i64(record.elapsed_ms),
                    clamp_i64(record.latency_ms),
                    i64::from(record.status_code),
                    i64::from(u8::from(record.timed_out)),
                    i64::from(u8::from(record.transport_error))
                ])?;
            }
        }
        tx.commit()?;
        Ok(())
    })
    .await
    .map_err(|err| {
        AppError::metrics(MetricsError::External {
            context: "write sqlite metrics",
            source: Box::new(err),
        })
    })?;

    Ok(())
}

fn clamp_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}
