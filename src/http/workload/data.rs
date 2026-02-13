use std::sync::{
    Arc,
    atomic::{AtomicU64, AtomicUsize, Ordering},
};

use rand::distributions::Distribution;
use rand::thread_rng;
use rand_regex::Regex as RandRegex;
use reqwest::{Client, Request};
use tokio::sync::{Semaphore, mpsc};

use crate::{
    args::{ConnectToMapping, HttpMethod, Scenario},
    error::{AppError, AppResult, HttpError},
    metrics::{LogSink, Metrics},
    shutdown::ShutdownSender,
};

#[derive(Clone)]
pub(in crate::http) enum Workload {
    Single(Arc<Request>),
    SingleDynamic(Arc<SingleRequestSpec>),
    Scenario(
        Arc<Scenario>,
        Arc<Vec<ConnectToMapping>>,
        Option<String>,
        Option<AuthConfig>,
    ),
}

#[derive(Debug, Clone)]
pub(crate) enum AuthConfig {
    Basic {
        username: String,
        password: String,
    },
    SigV4 {
        access_key: String,
        secret_key: String,
        session_token: Option<String>,
        region: String,
        service: String,
    },
}

#[derive(Debug)]
pub(in crate::http) struct RequestLimiter {
    limit: Option<u64>,
    counter: AtomicU64,
}

impl RequestLimiter {
    pub(in crate::http) fn new(limit: Option<u64>) -> Option<Self> {
        limit.map(|limit| RequestLimiter {
            limit: Some(limit),
            counter: AtomicU64::new(0),
        })
    }

    pub(in crate::http) fn try_reserve(&self, shutdown_tx: &ShutdownSender) -> bool {
        let Some(limit) = self.limit else {
            return true;
        };
        loop {
            let current = self.counter.load(Ordering::Relaxed);
            if current >= limit {
                drop(shutdown_tx.send(()));
                return false;
            }
            let Some(next) = current.checked_add(1) else {
                drop(shutdown_tx.send(()));
                return false;
            };
            if self
                .counter
                .compare_exchange(current, next, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                return true;
            }
        }
    }
}

#[derive(Debug)]
pub(in crate::http) struct IndexedList {
    items: Vec<String>,
    cursor: AtomicUsize,
}

impl IndexedList {
    const fn new(items: Vec<String>) -> Self {
        Self {
            items,
            cursor: AtomicUsize::new(0),
        }
    }

    pub(super) fn next(&self) -> Option<String> {
        if self.items.is_empty() {
            return None;
        }
        let idx = self.cursor.fetch_add(1, Ordering::Relaxed);
        let len = self.items.len();
        let selected = idx.rem_euclid(len);
        self.items.get(selected).cloned()
    }
}

#[derive(Clone)]
pub(in crate::http) enum BodySource {
    Static(String),
    Lines(Arc<IndexedList>),
}

impl BodySource {
    pub(in crate::http) fn from_lines(lines: Vec<String>) -> Self {
        Self::Lines(Arc::new(IndexedList::new(lines)))
    }
}

#[derive(Clone)]
pub(in crate::http) enum UrlSource {
    Static(String),
    List(Arc<IndexedList>),
    Regex(Arc<RandRegex>),
}

impl UrlSource {
    pub(in crate::http) fn from_list(urls: Vec<String>) -> Self {
        Self::List(Arc::new(IndexedList::new(urls)))
    }

    pub(super) fn next_url(&self) -> AppResult<String> {
        match self {
            UrlSource::Static(url) => Ok(url.clone()),
            UrlSource::List(list) => list
                .next()
                .ok_or_else(|| AppError::http(HttpError::UrlListEmpty)),
            UrlSource::Regex(regex) => {
                let mut rng = thread_rng();
                Ok(regex.sample(&mut rng))
            }
        }
    }
}

#[derive(Clone)]
pub(in crate::http) enum FormFieldSpec {
    Text { name: String, value: String },
    File { name: String, path: String },
}

#[derive(Clone)]
pub(in crate::http) struct SingleRequestSpec {
    pub(in crate::http) method: HttpMethod,
    pub(in crate::http) url: UrlSource,
    pub(in crate::http) headers: Vec<(String, String)>,
    pub(in crate::http) body: BodySource,
    pub(in crate::http) form: Option<Vec<FormFieldSpec>>,
    pub(in crate::http) connect_to: Vec<ConnectToMapping>,
    pub(in crate::http) auth: Option<AuthConfig>,
}

pub(in crate::http) struct WorkerContext<'ctx> {
    pub(in crate::http) shutdown_tx: &'ctx ShutdownSender,
    pub(in crate::http) rate_limiter: Option<&'ctx Arc<Semaphore>>,
    pub(in crate::http) request_limiter: Option<&'ctx Arc<RequestLimiter>>,
    pub(in crate::http) in_flight_counter: &'ctx Arc<AtomicU64>,
    pub(in crate::http) wait_ongoing: bool,
    pub(in crate::http) latency_correction: bool,
    pub(in crate::http) client: &'ctx Client,
    pub(in crate::http) log_sink: &'ctx Option<Arc<LogSink>>,
    pub(in crate::http) metrics_tx: &'ctx mpsc::Sender<Metrics>,
}

pub(in crate::http) struct ScenarioRunContext<'ctx> {
    pub(in crate::http) client: &'ctx Client,
    pub(in crate::http) scenario: &'ctx Scenario,
    pub(in crate::http) connect_to: &'ctx [ConnectToMapping],
    pub(in crate::http) host_header: Option<&'ctx str>,
    pub(in crate::http) auth: Option<&'ctx AuthConfig>,
    pub(in crate::http) expected_status_code: u16,
    pub(in crate::http) log_sink: &'ctx Option<Arc<LogSink>>,
    pub(in crate::http) metrics_tx: &'ctx mpsc::Sender<Metrics>,
    pub(in crate::http) request_seq: &'ctx mut u64,
}
