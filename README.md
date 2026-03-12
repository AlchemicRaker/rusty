# Rusty: Rust + Grok SWE Agent

Rusty is your personal AI Software Engineering Agent written in Rust. It reads GitHub Issues (or a local workspace), collaborates with you via comments until the spec is approved, then plans, codes, tests (with auto-fixes), and opens a draft PR — never touching protected branches.

Built for phone-based oversight and strict human review. Budget-aware and fully controllable.

## Current State (March 2026)
Fully working:
- Cargo workspace (`rusty-core` + `rusty-cli`)
- Docker + docker-compose setup with proper volumes (`/workspace`, `/sessions`, `/logs`, `/prompts`)
- Custom Grok client using xAI REST `/responses` endpoint with `json_schema` + tool calling
- `read_file` tool (safe, line-limited, path-traversal protected)
- `RepoService` trait (GitHub via octocrab + Local filesystem mode)
- `SpecRefiner` node fully implemented (asks Grok, posts comments, pauses for your input)
- `IssueIngestor` + top-level runner with `--step` mode and resumability via JSON
- Structured logging to rolling files + console
- Non-root Docker user, AWS-migration friendly

Still to come: Planner, Coder (with code_queue), Tester retry loop, PRSubmitter.

## Quick Start

1. Clone & enter directory
2. Copy `.env.example` to `.env` and fill `XAI_API_KEY` + `GITHUB_TOKEN`
3. `docker compose up --build`

Example CLI (inside container or via cargo):
```
rusty --session my-issue-42 --repo AlchemicRaker/myrepo --issue 42 --step
```
Local mode:
```
rusty --session test --local /workspace --step
```

Logs live in `./logs/agent.log.*`  
Sessions saved in `./sessions/`

## Project Structure
- `rusty-core/` — all agent logic, Grok client, nodes, tools
- `rusty-cli/` — thin CLI (clap)
- `./prompts/*.md` — prompt templates (loaded at runtime)
- `./sessions/` — resumable JSON state
- `./workspace/` — your repo (volume-mounted)

See `GOALS.md` for the full design and roadmap.

## Environment Variables
- `XAI_API_KEY` (required)
- `GITHUB_TOKEN` (required for GitHub mode)
- `RUSTY_LOG=debug` (or trace)
