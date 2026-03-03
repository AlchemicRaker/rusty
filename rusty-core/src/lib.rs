use std::path::Path;
use tokio::fs::create_dir_all;
use tracing::info;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

struct AgentContext {
    session_id: String,
    current_node: Node,
}

#[derive(Debug)]
enum Node {
    IssueIngestor,
}

#[derive(Debug)]
enum ControlFlow {
    Halt,
}

// Dummy node function
async fn run_node(_ctx: &mut AgentContext) -> Result<ControlFlow, Box<dyn std::error::Error>> {
    info!("Running {}({:?})", _ctx.session_id, _ctx.current_node);
    Ok(ControlFlow::Halt)
}

async fn prep_logging() -> Result<(), Box<dyn std::error::Error>> {
    let logs_path = Path::new("./logs");
    create_dir_all(logs_path).await?;
    let rolling_file_appender = RollingFileAppender::new(Rotation::DAILY, logs_path, "agent.log");

    let env_filter_level = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

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

pub async fn run_agent(
    session_id: String,
    step_mode: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // logging
    prep_logging().await?;

    // find AgentContext
    let mut context = AgentContext {
        current_node: Node::IssueIngestor,
        session_id: session_id,
    };
    info!("Agent Session {} resumed", context.session_id);

    loop {
        info!(
            "Agent Routing Session {} to Node {:?}",
            context.session_id, context.current_node
        );
        let result = run_node(&mut context).await?;

        if step_mode {
            info!("Step completed, result {:?}", result);

            break;
        }
        match result {
            ControlFlow::Halt => {
                info!("Halting!");
                break;
            }
        }
    }
    // save state
    info!("Agent Session {} suspended", context.session_id);
    Ok(())
}
