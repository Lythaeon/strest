use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::num::{NonZeroU64, NonZeroUsize};
use std::time::Duration;

use crate::error::{AppError, ValidationError};

#[derive(Debug, Clone, Copy, ValueEnum, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HttpMethod {
    Get,
    Post,
    Patch,
    Put,
    Delete,
}

#[derive(Debug, Clone, Copy, ValueEnum, Deserialize, Serialize, PartialEq, Eq)]
pub enum HttpVersion {
    #[serde(rename = "0.9")]
    #[value(name = "0.9")]
    V0_9,
    #[serde(rename = "1.0")]
    #[value(name = "1.0")]
    V1_0,
    #[serde(rename = "1.1")]
    #[value(name = "1.1")]
    V1_1,
    #[serde(rename = "2")]
    #[value(name = "2")]
    V2,
    #[serde(rename = "3")]
    #[value(name = "3")]
    V3,
}

impl std::str::FromStr for HttpVersion {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim();
        match normalized {
            "0.9" => Ok(HttpVersion::V0_9),
            "1.0" => Ok(HttpVersion::V1_0),
            "1.1" => Ok(HttpVersion::V1_1),
            "2" => Ok(HttpVersion::V2),
            "3" => Ok(HttpVersion::V3),
            _ => Err(AppError::validation(ValidationError::InvalidHttpVersion {
                value: s.to_owned(),
            })),
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    Text,
    Json,
    Jsonl,
    Csv,
    Quiet,
}

#[derive(Debug, Clone, Copy, ValueEnum, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TimeUnit {
    Ns,
    Us,
    Ms,
    S,
    M,
    H,
}

#[derive(Debug, Clone, Copy, ValueEnum, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ControllerMode {
    Auto,
    Manual,
}

impl std::str::FromStr for ControllerMode {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "auto" => Ok(ControllerMode::Auto),
            "manual" => Ok(ControllerMode::Manual),
            _ => Err(AppError::validation(
                ValidationError::InvalidControllerMode {
                    value: s.to_owned(),
                },
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Protocol {
    Http,
    GrpcUnary,
    GrpcStreaming,
    Websocket,
    Tcp,
    Udp,
    Quic,
    Mqtt,
    Enet,
    Kcp,
    Raknet,
}

impl Protocol {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Protocol::Http => "http",
            Protocol::GrpcUnary => "grpc-unary",
            Protocol::GrpcStreaming => "grpc-streaming",
            Protocol::Websocket => "websocket",
            Protocol::Tcp => "tcp",
            Protocol::Udp => "udp",
            Protocol::Quic => "quic",
            Protocol::Mqtt => "mqtt",
            Protocol::Enet => "enet",
            Protocol::Kcp => "kcp",
            Protocol::Raknet => "raknet",
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum LoadMode {
    Arrival,
    Step,
    Ramp,
    Jitter,
    Burst,
    Soak,
}

impl LoadMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            LoadMode::Arrival => "arrival",
            LoadMode::Step => "step",
            LoadMode::Ramp => "ramp",
            LoadMode::Jitter => "jitter",
            LoadMode::Burst => "burst",
            LoadMode::Soak => "soak",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TlsVersion {
    V1_0,
    V1_1,
    V1_2,
    V1_3,
}

impl std::str::FromStr for TlsVersion {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "1.0" | "tls1.0" | "tls1" | "v1.0" => Ok(TlsVersion::V1_0),
            "1.1" | "tls1.1" | "v1.1" => Ok(TlsVersion::V1_1),
            "1.2" | "tls1.2" | "v1.2" => Ok(TlsVersion::V1_2),
            "1.3" | "tls1.3" | "v1.3" => Ok(TlsVersion::V1_3),
            _ => Err(AppError::validation(ValidationError::InvalidTlsVersion {
                value: s.to_owned(),
            })),
        }
    }
}

impl<'de> Deserialize<'de> for TlsVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value
            .parse::<TlsVersion>()
            .map_err(serde::de::Error::custom)
    }
}

impl Serialize for TlsVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let value = match self {
            TlsVersion::V1_0 => "1.0",
            TlsVersion::V1_1 => "1.1",
            TlsVersion::V1_2 => "1.2",
            TlsVersion::V1_3 => "1.3",
        };
        serializer.serialize_str(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PositiveU64(NonZeroU64);

impl PositiveU64 {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

impl TryFrom<u64> for PositiveU64 {
    type Error = ValidationError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        NonZeroU64::new(value)
            .map(PositiveU64)
            .ok_or_else(|| ValidationError::ValueTooSmall { min: 1 })
    }
}

impl std::str::FromStr for PositiveU64 {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value: u64 = s
            .parse()
            .map_err(|err| ValidationError::InvalidNumber { source: err })?;
        PositiveU64::try_from(value)
    }
}

impl From<PositiveU64> for u64 {
    fn from(value: PositiveU64) -> Self {
        value.get()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PositiveUsize(NonZeroUsize);

impl PositiveUsize {
    #[must_use]
    pub const fn get(self) -> usize {
        self.0.get()
    }
}

impl TryFrom<usize> for PositiveUsize {
    type Error = ValidationError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        NonZeroUsize::new(value)
            .map(PositiveUsize)
            .ok_or_else(|| ValidationError::ValueTooSmall { min: 1 })
    }
}

impl std::str::FromStr for PositiveUsize {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value: usize = s
            .parse()
            .map_err(|err| ValidationError::InvalidNumber { source: err })?;
        PositiveUsize::try_from(value)
    }
}

impl From<PositiveUsize> for usize {
    fn from(value: PositiveUsize) -> Self {
        value.get()
    }
}

#[derive(Debug, Clone)]
pub struct LoadProfile {
    pub initial_rpm: u64,
    pub stages: Vec<LoadStage>,
}

#[derive(Debug, Clone)]
pub struct LoadStage {
    pub duration: Duration,
    pub target_rpm: u64,
}

#[derive(Debug, Clone)]
pub struct Scenario {
    pub base_url: Option<String>,
    pub vars: BTreeMap<String, String>,
    pub steps: Vec<ScenarioStep>,
}

#[derive(Debug, Clone)]
pub struct ScenarioStep {
    pub name: Option<String>,
    pub method: HttpMethod,
    pub url: Option<String>,
    pub path: Option<String>,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
    pub assert_status: Option<u16>,
    pub assert_body_contains: Option<String>,
    pub think_time: Option<Duration>,
    pub vars: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ConnectToMapping {
    pub source_host: String,
    pub source_port: u16,
    pub target_host: String,
    pub target_port: u16,
}
