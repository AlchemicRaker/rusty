use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs::read_to_string;
use tracing::{debug, info};

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

    async fn post_comment(&self, body: &str) -> Result<()>;
}

struct GitHubRepoService {
    client: octocrab::Octocrab,
    owner: String,
    repo: String,
    issue_number: u64,
}

impl GitHubRepoService {
    fn new(owner: String, repo: String, issue_number: u64) -> Result<Self> {
        let token =
            std::env::var("GITHUB_TOKEN").expect("Couldn't find GITHUB_TOKEN in environment");
        let client = octocrab::Octocrab::builder()
            .personal_token(token)
            .build()
            .expect("Failed to create Octocrab client");
        Ok(Self {
            client,
            owner,
            repo,
            issue_number,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum AuthorClass {
    User,
    Agent,
    Other,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Comment {
    pub author: AuthorClass,
    pub body: String,
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
                    "AlchemicRaker" => AuthorClass::User,
                    "mecharaker" => AuthorClass::Agent,
                    _ => AuthorClass::Other,
                };
                Comment {
                    author: author,
                    body: f.body.clone().expect("Expect non-empty body"),
                }
            })
            .filter(|f| match f.author {
                AuthorClass::Other => false, // remove all untrusted messages entirely
                _ => true,
            })
            .collect();

        Ok(Issue {
            number: issue.number,
            title: issue.title,
            body: issue.body.unwrap_or_default(),
            comments,
        })
    }

    async fn post_comment(&self, body: &str) -> Result<()> {
        let issue_handler = self.client.issues(&self.owner, &self.repo);
        issue_handler
            .create_comment(self.issue_number, body)
            .await
            .expect("Failed to post comment to GitHub issue");
        info!("Posted comment to GitHub issue #{}", self.issue_number);
        Ok(())
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

    async fn post_comment(&self, body: &str) -> Result<()> {
        let path = format!("{}/agent_response.md", self.path);
        let content = format!("{}\n\n---\n", body);

        tokio::fs::write(&path, content)
            .await
            .expect("Failed to write local agent response");

        info!("Local response saved to {}", path);

        Ok(())
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
