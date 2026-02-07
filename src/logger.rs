use tracing_subscriber::{EnvFilter, FmtSubscriber};

pub fn init_logging(verbose: bool) {
    let filter = std::env::var("STREST_LOG")
        .or_else(|_| std::env::var("RUST_LOG"))
        .map_or_else(
            |_| {
                if verbose {
                    EnvFilter::new("debug")
                } else {
                    EnvFilter::new("info")
                }
            },
            |value| EnvFilter::try_new(value).unwrap_or_else(|_| EnvFilter::new("info")),
        );

    let subscriber = FmtSubscriber::builder().with_env_filter(filter).finish();

    if let Err(err) = tracing::subscriber::set_global_default(subscriber) {
        eprintln!("Failed to set global default subscriber: {}", err);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_logging_is_idempotent() {
        init_logging(false);
        init_logging(false);
    }
}
