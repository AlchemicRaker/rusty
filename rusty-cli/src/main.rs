use clap::Parser;
use rusty_core::run_agent;
use tokio::runtime::Runtime;

// --session <session_id> --step
#[derive(Parser)]
#[command(version = "0.1", about = "Rusty SWE Agent CLI")]
struct Args {
    #[arg(short, long)]
    session: Option<String>, // Session (issue?) ID for resuming processes

    #[arg(long)]
    step: bool, // Step flag, to advance a single step
}

fn main() {
    let args = Args::parse();
    let session_id = args.session.expect("Missing session ID");

    println!("session {}, skip {}", session_id, args.step);

    let rt = Runtime::new().unwrap();

    let _result = rt.block_on(async { run_agent(session_id.clone(), args.step).await });
}
