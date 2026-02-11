use plotters::prelude::{BitMapBackend, DrawingAreaErrorKind, DrawingBackend};
use thiserror::Error;

use super::{
    ConfigError, DistributedError, HttpError, MetricsError, ScriptError, ServiceError, SinkError,
    ValidationError,
};

type PlottersError = DrawingAreaErrorKind<<BitMapBackend<'static> as DrawingBackend>::ErrorType>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("I/O error: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },
    #[error("CLI error: {source}")]
    Clap {
        #[from]
        source: clap::Error,
    },
    #[error("JSON error: {source}")]
    Json {
        #[from]
        source: serde_json::Error,
    },
    #[error("TOML error: {source}")]
    Toml {
        #[from]
        source: toml::de::Error,
    },
    #[error("HTTP client error: {source}")]
    Reqwest {
        #[from]
        source: reqwest::Error,
    },
    #[error("Join error: {source}")]
    Join {
        #[from]
        source: tokio::task::JoinError,
    },
    #[error("Parse error: {source}")]
    ParseInt {
        #[from]
        source: std::num::ParseIntError,
    },
    #[error("Parse error: {source}")]
    ParseFloat {
        #[from]
        source: std::num::ParseFloatError,
    },
    #[error("UTF-8 error: {source}")]
    Utf8 {
        #[from]
        source: std::str::Utf8Error,
    },
    #[error("Time error: {source}")]
    SystemTime {
        #[from]
        source: std::time::SystemTimeError,
    },
    #[error("Plotting error: {source}")]
    Plotters {
        #[from]
        source: PlottersError,
    },
    #[error("Validation error: {0}")]
    Validation(#[from] ValidationError),
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),
    #[error("HTTP error: {0}")]
    Http(#[from] HttpError),
    #[error("Metrics error: {0}")]
    Metrics(#[from] MetricsError),
    #[error("Distributed error: {0}")]
    Distributed(#[from] DistributedError),
    #[error("Script error: {0}")]
    Script(#[from] ScriptError),
    #[error("Service error: {0}")]
    Service(#[from] ServiceError),
    #[error("Sink error: {0}")]
    Sink(#[from] SinkError),
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn validation<E>(error: E) -> Self
    where
        E: Into<ValidationError>,
    {
        error.into().into()
    }

    pub fn config<E>(error: E) -> Self
    where
        E: Into<ConfigError>,
    {
        error.into().into()
    }

    pub fn http<E>(error: E) -> Self
    where
        E: Into<HttpError>,
    {
        error.into().into()
    }

    pub fn metrics<E>(error: E) -> Self
    where
        E: Into<MetricsError>,
    {
        error.into().into()
    }

    pub fn distributed<E>(error: E) -> Self
    where
        E: Into<DistributedError>,
    {
        error.into().into()
    }

    pub fn script<E>(error: E) -> Self
    where
        E: Into<ScriptError>,
    {
        error.into().into()
    }

    pub fn service<E>(error: E) -> Self
    where
        E: Into<ServiceError>,
    {
        error.into().into()
    }

    pub fn sink<E>(error: E) -> Self
    where
        E: Into<SinkError>,
    {
        error.into().into()
    }
}
