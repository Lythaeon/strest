use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use hdrhistogram::Histogram;
use hdrhistogram::serialization::{Deserializer, Serializer, V2Serializer};
use std::io::Cursor;

use crate::error::{AppError, AppResult, MetricsError};

#[derive(Debug)]
pub struct LatencyHistogram {
    hist: Histogram<u64>,
}

impl LatencyHistogram {
    /// Create a new latency histogram.
    ///
    /// # Errors
    ///
    /// Returns an error if the histogram cannot be created.
    pub fn new() -> AppResult<Self> {
        let hist = Histogram::<u64>::new(3).map_err(|err| {
            AppError::metrics(MetricsError::Histogram {
                context: "create histogram",
                source: Box::new(err),
            })
        })?;
        Ok(Self { hist })
    }

    /// Record a latency value in milliseconds.
    ///
    /// # Errors
    ///
    /// Returns an error if the value cannot be recorded.
    pub fn record(&mut self, latency_ms: u64) -> AppResult<()> {
        let value = latency_ms.max(1);
        self.hist.record(value).map_err(|err| {
            AppError::metrics(MetricsError::Histogram {
                context: "record latency",
                source: Box::new(err),
            })
        })
    }

    /// Merge another histogram into this one.
    ///
    /// # Errors
    ///
    /// Returns an error if the merge fails.
    pub fn merge(&mut self, other: &LatencyHistogram) -> AppResult<()> {
        self.hist.add(&other.hist).map_err(|err| {
            AppError::metrics(MetricsError::Histogram {
                context: "merge histogram",
                source: Box::new(err),
            })
        })
    }

    #[must_use]
    pub fn percentiles(&self) -> (u64, u64, u64) {
        let count = self.count();
        if count == 0 {
            return (0, 0, 0);
        }

        (
            self.hist.value_at_quantile(0.5),
            self.hist.value_at_quantile(0.9),
            self.hist.value_at_quantile(0.99),
        )
    }

    #[must_use]
    pub fn count(&self) -> u64 {
        self.hist.len()
    }

    /// Encode the histogram as base64.
    ///
    /// # Errors
    ///
    /// Returns an error if the histogram cannot be serialized.
    pub fn encode_base64(&self) -> AppResult<String> {
        let mut buffer = Vec::new();
        V2Serializer::new()
            .serialize(&self.hist, &mut buffer)
            .map_err(|err| {
                AppError::metrics(MetricsError::Histogram {
                    context: "serialize histogram",
                    source: Box::new(err),
                })
            })?;
        Ok(B64.encode(buffer))
    }

    /// Decode a base64 histogram payload.
    ///
    /// # Errors
    ///
    /// Returns an error if the payload cannot be decoded or deserialized.
    pub fn decode_base64(encoded: &str) -> AppResult<Self> {
        let bytes = B64.decode(encoded.as_bytes()).map_err(|err| {
            AppError::metrics(MetricsError::Histogram {
                context: "decode histogram",
                source: Box::new(err),
            })
        })?;
        let mut cursor = Cursor::new(bytes);
        let hist: Histogram<u64> = Deserializer::new()
            .deserialize(&mut cursor)
            .map_err(|err| {
                AppError::metrics(MetricsError::Histogram {
                    context: "deserialize histogram",
                    source: Box::new(err),
                })
            })?;
        Ok(Self { hist })
    }
}
