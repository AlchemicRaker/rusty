use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;
use tokio::fs::create_dir_all;
use tracing::info;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub enum RepoConfig {
    Local {
        path: String,
    },
    GitHub {
        owner: String,
        repo: String,
        issue_number: u64,
    },
}
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
    // TODO: save and load here?
}

#[async_trait]
trait RepoService {
    async fn load_issue(&self) -> Result<String>; // "loads" issue summary, at least
}

struct GitHubRepoService {
    client: octocrab::Octocrab,
    owner: String,
    repo: String,
    issue_number: u64,
}

impl GitHubRepoService {
    fn new(owner: String, repo: String, issue_number: u64) -> Result<Self> {
        let token = std::env::var("GITHUB_TOKEN")?;
        let client = octocrab::Octocrab::builder()
            .personal_token(token)
            .build()?;
        Ok(Self {
            client,
            owner,
            repo,
            issue_number,
        })
    }
}

#[async_trait]
impl RepoService for GitHubRepoService {
    async fn load_issue(&self) -> Result<String> {
        let issue = self
            .client
            .issues(&self.owner, &self.repo)
            .get(self.issue_number)
            .await?;
        Ok(format!(
            "Issue #{}: {} - {}",
            issue.number,
            issue.title,
            issue.body.unwrap_or_default()
        ))
    }
}

struct LocalRepoService {
    path: String,
}

#[async_trait]
impl RepoService for LocalRepoService {
    async fn load_issue(&self) -> Result<String> {
        // TODO: local issue thread loaded from file
        Ok(format!("Local dummy issue at path: {}", self.path))
    }
}

#[derive(Debug)]
enum Node {
    IssueIngestor,
}

#[derive(Debug)]
enum ControlFlow {
    Continue { next_node: Node },
    Pause { reason: String, next_node: Node },
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
    repo_config: RepoConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    // logging
    prep_logging().await?;

    // find AgentContext
    let mut context = AgentContext::new(session_id, repo_config);
    let service: Box<dyn RepoService> = match &context.repo_config {
        RepoConfig::Local { path } => Box::new(LocalRepoService { path: path.clone() }),
        RepoConfig::GitHub {
            owner,
            repo,
            issue_number,
        } => Box::new(GitHubRepoService::new(
            owner.clone(),
            repo.clone(),
            *issue_number,
        )?),
    };

    info!("Agent Session {} resumed", context.session_id);

    loop {
        info!(
            "Agent Routing Session {} to Node {:?}",
            context.session_id, context.current_node
        );

        // TODO: implement dispatch_node to call individual nodes
        let result = run_node(&mut context).await?;

        if step_mode {
            info!("Step completed, result {:?}", result);

            break;
        }
        match result {
            ControlFlow::Continue { next_node } => {
                info!(
                    "Advancing {} from {:?} to {:?}",
                    context.session_id, context.current_node, next_node
                );
                context.current_node = next_node;
                break;
            }
            ControlFlow::Pause { reason, next_node } => {
                info!(
                    "Pausing {} in {:?} for {}, will resume to {:?}",
                    context.session_id, context.current_node, reason, next_node
                );
                break;
            }
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
