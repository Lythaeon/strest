//! HTTP request execution and workload orchestration.
mod rate;
mod sender;
mod tls;
pub(crate) mod workload;

#[cfg(test)]
mod tests;

pub(crate) use rate::build_rate_limiter;
pub use sender::setup_request_sender;

#[cfg(test)]
pub(crate) use rate::{RateController, RatePlan, RateStage};
#[cfg(test)]
pub(crate) use tls::{AlpnChoice, resolve_alpn};
