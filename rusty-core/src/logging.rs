use anyhow::Result;
use std::path::Path;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use tokio::fs::create_dir_all;
pub async fn prep_logging() -> Result<(), Box<dyn std::error::Error>> {
    let logs_path = Path::new("./logs");
    create_dir_all(logs_path).await?;
    let rolling_file_appender = RollingFileAppender::new(Rotation::DAILY, logs_path, "agent.log");

    let env_filter_level = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("trace"));

    tracing_subscriber::registry()
        .with(env_filter_level)
        .with(
            fmt::layer()
                .with_ansi(false)
                .with_writer(rolling_file_appender),
        )
        .with(fmt::layer().with_writer(std::io::stdout))
        .init();
    Ok(())
}
