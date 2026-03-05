mod logging;
mod repo_service;
use crate::repo_service::RepoService;
use anyhow::Result;
pub use repo_service::RepoConfig;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs::{read_to_string, write};
use tracing::{info, trace, warn};

#[derive(Serialize, Deserialize)]
struct AgentContext {
    session_id: String,
    current_node: Node,
    repo_config: RepoConfig,
    issue_summary: Option<String>, // brief auto-generated issue summary
}

impl AgentContext {
    fn new(session_id: String, repo_config: RepoConfig) -> Self {
        Self {
            session_id,
            repo_config,
            current_node: Node::IssueIngestor,
            issue_summary: None,
        }
    }
    // save and load from local sessions folder
    async fn save_to_json(&self) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        let path_str = format!("sessions/{}_context.json", self.session_id).to_string();
        let path = Path::new(&path_str);
        write(path, json).await?;
        Ok(())
    }
    async fn load_from_json(session_id: String) -> Result<Self> {
        let path_str = format!("sessions/{}_context.json", session_id).to_string();
        let path = Path::new(&path_str);
        let json = read_to_string(path).await?;
        let context: AgentContext = serde_json::from_str(&json)?;
        Ok(context)
    }
}

#[derive(Debug, Serialize, Deserialize)]
enum Node {
    IssueIngestor,
    SpecRefiner,
    Planner,
    Coder,
    Tester,
    PRSubmitter,
    PostPR,
}

#[derive(Debug)]
enum ControlFlow {
    Continue { next_node: Node },
    Pause { reason: String, next_node: Node },
    Halt,
}

pub async fn run_agent(
    session_id: String,
    step_mode: bool,
    repo_config: RepoConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    // logging
    logging::prep_logging().await?;

    // restore or generate baseline AgentContext
    let restored_context = AgentContext::load_from_json(session_id.clone()).await;
    let mut context = match restored_context {
        Ok(context) => context,
        Err(_) => AgentContext::new(session_id, repo_config.clone()),
    };

    let service = repo_service::create_repo_service(repo_config)?;

    info!("Agent Session {} resumed", context.session_id);

    loop {
        info!(
            "Agent Routing Session {} to Node {:?}",
            context.session_id, context.current_node
        );

        let result = dispatch_node(&mut context, &service).await?;

        if step_mode {
            trace!("Step completed, result {:?}", result);
            break;
        }
        match result {
            ControlFlow::Continue { next_node } => {
                info!(
                    "Advancing {} from {:?} to {:?}",
                    context.session_id, context.current_node, next_node
                );
                context.current_node = next_node;
            }
            ControlFlow::Pause { reason, next_node } => {
                info!(
                    "Pausing {} in {:?} for {}, will resume to {:?}",
                    context.session_id, context.current_node, reason, next_node
                );
                break;
            }
            ControlFlow::Halt => {
                info!("Halting...");
                break;
            }
        }
    }
    // save state
    context
        .save_to_json()
        .await
        .expect("Failed to persist session state");
    info!("Agent Session {} suspended", context.session_id);
    Ok(())
}

async fn dispatch_node(
    context: &mut AgentContext,
    service: &Box<dyn RepoService>,
) -> Result<ControlFlow, Box<dyn std::error::Error>> {
    trace!(
        "Dispatching {}({:?})",
        context.session_id, context.current_node
    );
    match context.current_node {
        Node::IssueIngestor => issue_ingestor(context, service).await,
        Node::SpecRefiner => spec_refiner(context, service).await,
        _ => {
            warn!(
                "Node {:?} is undefined; Halting Session {}",
                context.current_node, context.session_id
            );
            Ok(ControlFlow::Halt)
        }
    }
}

// For brand new issues, populate some baseline session state (AgentContext) just once
async fn issue_ingestor(
    context: &mut AgentContext,
    service: &Box<dyn RepoService>,
) -> Result<ControlFlow, Box<dyn std::error::Error>> {
    context.issue_summary = Some(service.load_issue().await?);

    info!(
        "Session {} Ingested Issue {:?}",
        context.session_id, context.issue_summary
    );

    Ok(ControlFlow::Continue {
        next_node: Node::SpecRefiner,
    })
}

async fn spec_refiner(
    context: &mut AgentContext,
    service: &Box<dyn RepoService>,
) -> Result<ControlFlow, Box<dyn std::error::Error>> {
    Ok(ControlFlow::Pause {
        next_node: Node::SpecRefiner,
        reason: "Waiting for issue clarification from user.".to_string(),
    })
}
