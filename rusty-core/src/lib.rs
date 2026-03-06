mod grok_client;
mod logging;
mod repo_service;
use crate::{grok_client::GrokClient, repo_service::RepoService};
use anyhow::{Context, Result};
use repo_service::Issue;
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
    issue: Issue, // brief auto-generated issue summary
}

impl AgentContext {
    fn new(session_id: String, repo_config: RepoConfig, issue: Issue) -> Self {
        Self {
            session_id,
            repo_config,
            current_node: Node::IssueIngestor,
            issue,
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
    IssueIngestor, // currently nonfunctional - issue state is refreshed regardless of which node is resumed
    SpecRefiner,   // implementing now
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
    println!("a");
    logging::prep_logging().await?;
    println!("b");
    // restore or generate baseline AgentContext

    let service = repo_service::create_repo_service(repo_config.clone())
        .expect("Failed to create repo service");
    let issue = service
        .load_issue()
        .await
        .expect("Failed to load issues from repo");
    println!("c");
    let restored_context = AgentContext::load_from_json(session_id.clone()).await;
    let mut context = match restored_context {
        Ok(context) => AgentContext { issue, ..context }, // always patch in latest Issue state
        Err(_) => AgentContext::new(session_id, repo_config, issue),
    };
    println!("d");
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

    // always save state after the loop, regardless of reason for pause or halt
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
    info!(
        "Session {} Ingested Issue {}",
        context.session_id, context.issue.title
    );

    Ok(ControlFlow::Continue {
        next_node: Node::SpecRefiner,
    })
}

#[derive(Deserialize)]
struct SpecDecision {
    #[serde(rename = "approved_and_ready_for_implementation")]
    approved: bool,
    questions: Vec<String>,
    refined_spec: String,
}

async fn spec_refiner(
    context: &mut AgentContext,
    service: &Box<dyn RepoService>,
) -> Result<ControlFlow, Box<dyn std::error::Error>> {
    let grok = GrokClient::new().expect("Failed to create a GrokClient");

    let system = load_prompt("spec_refiner")
        .await
        .expect("Failed to load spec refiner prompt");

    let repo_string = match &context.repo_config {
        RepoConfig::GitHub {
            owner,
            repo,
            issue_number: _,
        } => format!("Repository: https://github.com/{}/{}\n", owner, repo),
        _ => "".to_string(),
    };

    let user = format!(
        "{}Issue title: {}\nBody: {}\nComments: {:?}",
        repo_string, context.issue.title, context.issue.body, context.issue.comments
    );

    let schema = serde_json::json!({
        "name": "spec_decision",
        "schema": {
            "type": "object",
            "properties": {
                "approved_and_ready_for_implementation": { "type": "boolean" },
                "questions": {"type":"array", "items": {"type":"string"}},
                "refined_spec": {"type":"string"}
            },
            "required": ["approved_and_ready_for_implementation", "questions", "refined_spec"],
            "additionalProperties": false
        }
    });

    let decision: SpecDecision = grok
        .call(
            grok_client::Model::Grok4_1FastReasoning,
            &system,
            &user,
            schema,
            "spec_decision",
        )
        .await
        .expect("Failed to call Grok to get a spec decision");

    info!(
        "Grok decision: approved={}, questions={:?}, refined_spec={}",
        decision.approved, decision.questions, decision.refined_spec
    );

    if decision.approved {
        // Respond with a refined spec
        Ok(ControlFlow::Continue {
            next_node: Node::Planner,
        })
    } else {
        // Respond with a list of questions
        Ok(ControlFlow::Pause {
            reason: format!("Spec needs clarification: {:?}", decision.questions),
            next_node: Node::SpecRefiner,
        })
    }
}

async fn load_prompt(name: &str) -> Result<String> {
    let path = format!("prompts/{}.md", name);
    let content = read_to_string(&path)
        .await
        .expect(format!("Failed to read prompt file {}", path).as_str());
    Ok(content)
}
