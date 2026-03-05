use clap::Parser;
use rusty_core::{RepoConfig, run_agent};
use tokio::runtime::Runtime;

// --session <session_id> --step
#[derive(Parser)]
#[command(version = "0.1", about = "Rusty SWE Agent CLI")]
struct Args {
    #[arg(short, long)]
    session: Option<String>, // Session (issue?) ID for resuming processes

    #[arg(long)]
    step: bool, // Step flag, to advance a single step

    #[arg(long)]
    local: Option<String>,

    #[arg(long)]
    repo: Option<String>, // user/repository on github

    #[arg(long)]
    issue: Option<u64>, // issue number on github
}

fn main() {
    let args = Args::parse();
    let session_id = args.session.expect("Missing session ID");

    let rt = Runtime::new().unwrap();

    let repo_config: RepoConfig = if let Some(path) = args.local {
        RepoConfig::Local { path }
    } else if let (Some(repo), Some(issue_number)) = (args.repo, args.issue) {
        let (owner, repo) = repo
            .split_once("/")
            .expect("--repo must have user/repository");
        RepoConfig::GitHub {
            owner: owner.to_string(),
            repo: repo.to_string(),
            issue_number,
        }
    } else {
        panic!("Must provide --local OR --repo + --issue");
    };

    let _result =
        rt.block_on(async { run_agent(session_id.clone(), args.step, repo_config).await });
}
