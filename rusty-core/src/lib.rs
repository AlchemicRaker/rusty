use std::path::Path;
use tokio::fs::create_dir_all;
use tracing::info;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

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
    //info!("Running node: {}", node);
    println!("Running {}({:?})", _ctx.session_id, _ctx.current_node);
    Ok(ControlFlow::Halt)
}

pub async fn run_agent(
    session_id: String,
    step_mode: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // logging
    let logs_path = Path::new("./logs");
    create_dir_all(logs_path).await?;
    let rolling_file_appender = RollingFileAppender::new(Rotation::DAILY, logs_path, "agent.log");
    let (non_blocking_writer, _) = tracing_appender::non_blocking(rolling_file_appender);

    tracing_subscriber::registry()
        .with(EnvFilter::new("info"))
        .with(fmt::layer().with_writer(non_blocking_writer))
        .with(fmt::layer().with_writer(std::io::stdout))
        .init();

    // find AgentContext
    let mut context = AgentContext {
        current_node: Node::IssueIngestor,
        session_id: session_id,
    };
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
    Ok(())
}
