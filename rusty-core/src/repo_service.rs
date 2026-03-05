use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs::read_to_string;

#[derive(Serialize, Deserialize, Clone)]
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

#[async_trait]
pub trait RepoService {
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
        let issue_path = format!("{}/summary.txt", self.path);
        let issue_summary = read_to_string(Path::new(&issue_path))
            .await
            .expect("issue path to exist");
        Ok(issue_summary.to_string())
    }
}

pub fn create_repo_service(repo_config: RepoConfig) -> Result<Box<dyn RepoService>> {
    match &repo_config {
        RepoConfig::Local { path } => Ok(Box::new(LocalRepoService { path: path.clone() })),
        RepoConfig::GitHub {
            owner,
            repo,
            issue_number,
        } => Ok(Box::new(GitHubRepoService::new(
            owner.clone(),
            repo.clone(),
            *issue_number,
        )?)),
    }
}
