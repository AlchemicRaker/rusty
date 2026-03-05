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
    async fn load_issue(&self) -> Result<Issue>;
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

#[derive(Serialize, Deserialize, Clone)]
pub enum CommentClass {
    User,
    Agent,
    Other,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Comment {
    class: CommentClass,
    body: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Issue {
    pub number: u64,
    pub title: String,
    pub body: String,
    pub comments: Vec<Comment>,
}

#[async_trait]
impl RepoService for GitHubRepoService {
    async fn load_issue(&self) -> Result<Issue> {
        let base = self.client.issues(&self.owner, &self.repo);
        let issue = base.get(self.issue_number).await?;

        // let comment_count = self.client.iss
        let first_page = base
            .list_comments(self.issue_number)
            .per_page(100)
            .send()
            .await?;

        let comments: Vec<Comment> = self
            .client
            .all_pages(first_page)
            .await?
            .iter()
            .filter(|f| match f.body {
                Some(_) => true,
                None => false,
            })
            .map(|f| {
                let author = match f.user.login.as_str() {
                    "AlchemicRaker" => CommentClass::Agent,
                    _ => CommentClass::Other,
                };
                Comment {
                    class: author,
                    body: f.body.clone().expect("Expect non-empty body"),
                }
            })
            .collect();

        Ok(Issue {
            number: issue.number,
            title: issue.title,
            body: issue.body.unwrap_or_default(),
            comments,
        })
    }
}

struct LocalRepoService {
    path: String,
}

#[async_trait]
impl RepoService for LocalRepoService {
    async fn load_issue(&self) -> Result<Issue> {
        let path_str = format!("{}/issue.json", self.path).to_string();
        let path = Path::new(&path_str);
        let json = read_to_string(path).await?;
        let context: Issue = serde_json::from_str(&json)?;
        Ok(context)
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
