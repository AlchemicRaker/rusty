# Rusty: Rust + Grok SWE Agent

Rusty is a personal, AI-powered Software Engineering Agent built in Rust, integrated with the Grok API from xAI. It automates tasks like processing GitHub Issues, refining specifications through human-in-the-loop discussions, planning code changes, coding, testing, and creating pull requests—all while ensuring strict human review and no direct merges to protected branches. Designed for budget-conscious private use, especially for oversight from mobile devices.

## Project Status
This is a Work in Progress (WIP). The current implementation includes a basic Cargo workspace with `rusty-core` (library for agent logic) and `rusty-cli` (command-line interface). Dependencies like tokio, serde, reqwest, and tracing are set up. A stub top-level runner with logging, node enums, and basic resumability via JSON is implemented. See GOALS.md for the full design roadmap.

## Features (Planned/MVP)
- GitHub Issue processing with human collaboration via comments.
- Node-based workflow: Ingest → Refine Spec → Plan → Code → Test (with auto-retries) → Submit PR → Monitor.
- Local testing mode for filesystem-based development.
- Budget-aware Grok model selection (e.g., grok-4-1-fast for planning, grok-code-fast-1 for coding).
- Docker deployment with docker-compose for easy setup and future AWS migration.
- Structured outputs via Grok's json_schema for reliability.
- Logging to files and console for debugging.
- Resumability with JSON session files.
- --step flag for paused debugging.

## Setup
1. Clone the repo: `git clone https://github.com/AlchemicRaker/rusty.git`
2. Navigate to the root: `cd rusty`
3. Build: `cargo build`
4. Run CLI example: `cargo run --bin rusty-cli -- --process test-session --step`

### Dependencies
- Rust (latest stable, via rustup)
- Cargo workspace with:
  - tokio (async runtime)
  - serde (JSON serialization)
  - reqwest (HTTP client for Grok API)
  - tracing, tracing-subscriber, tracing-appender (logging)
  - clap (CLI parsing, in rusty-cli)
- Future: octocrab (GitHub API), Docker.

### Environment Variables
- `RUST_LOG=info` (or trace/debug for more verbosity)
- Grok API key: `GROK_API_KEY` (to be added)
- GitHub token: `GITHUB_TOKEN` (to be added)

## Usage
- Start a session: `rusty-cli --process <session_id> [--step]`
- Resume: Same command reloads from `./data/sessions/<session_id>.json`
- Logs: In `./logs/agent.log.<date>`

## Development
- Workspace structure:
  - `rusty-core/`: Core logic (nodes, context, runner)
  - `rusty-cli/`: CLI wrapper
  - `data/sessions/`: JSON state files (gitignored)
  - `logs/`: Log files (gitignored)
  - `prompts/`: Template files (to be added)
- Add nodes in `rusty-core/src/lib.rs` via the enum and run_node match.
- Test: `cargo test`

## Contributing
This is a personal project—feel free to fork or suggest improvements via issues.

## License
None.