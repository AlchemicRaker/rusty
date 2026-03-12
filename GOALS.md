# GOALS.md: Rusty Project Design and Roadmap

This document is the living reference for the original design plus current reality. Updated March 2026 to match implemented code.

## Project Overview
- Title: Rusty (Rust + Grok SWE Agent)
- Description: Personal senior-engineer-in-the-loop agent that processes GitHub Issues (or local folders), collaborates via comments until spec approved, then plans → codes → tests (auto-retries) → opens draft PR. Never merges to main/release.

## Current State (March 2026) — What Actually Works Today
- Cargo workspace + CLI with `--session`, `--step`, `--local`, `--repo` + `--issue`
- Full Docker setup (multi-stage, non-root, volumes for workspace/sessions/logs/prompts)
- Custom GrokClient using REST `/responses` endpoint (not chat completions)
- Tool calling support (ReadFile tool fully implemented and safe)
- RepoService trait with GitHub (octocrab) and Local implementations
- AgentContext is minimal and working: session_id, current_node, repo_config, issue (richer fields like code_queue deferred)
- IssueIngestor + SpecRefiner nodes fully functional (SpecRefiner uses Grok with json_schema, posts comments, pauses correctly)
- Prompt loading from `/prompts/*.md` files (note: .md extension in code)
- Logging with tracing + rolling files + console
- Resumability via JSON sessions
- `--step` flag works at top-level runner only

Divergences from original v7 design (intentional simplifications):
- No conversation_history pruning or LLM reducer yet
- No code_queue / plan / retry_count yet (will add when we implement Planner/Coder)
- Prompts use .md extension
- Grok model enum currently only has Grok4_1FastReasoning
- No last_grok_responses temp field yet


## Constraints & Assumptions
- Personal project—keep simple, no over-engineering.
- Agent never pushes/merges to protected branches.
- All changes human-reviewed.
- Polling (60s) acceptable for MVP but webhook-upgrade ready.
- Local mode for fast testing.
- Retry limiter + queue prevents cost spirals.
- Pruning uses cheap, non-reasoning LLM; no special offline contingency (Grok dependency inherent).
- Prompts include schema description despite API enforcement for better model compliance.



## Functional Requirements (original vision — still valid)
- Node graph with 7 nodes (only first 2 implemented)
- LLM nodes use grok-4-1-fast (planning/refining) and grok-code-fast-1 (coding)
- Tester auto-retries to Coder (max 3) using shared code_queue
- --step flag at top level only
- Human-in-the-loop via GitHub comments
- Local mode via volume mount
- Pruning rules and temporary Grok outputs planned for later
- **Node Graph**: Custom typed node graph with 7 core nodes. Each node is an async fn that takes &mut AgentContext and returns ControlFlow (Continue(Node), Pause(reason), Halt). Purposes:
  1. **IssueIngestor**: Loads issue details/comments (via octocrab) or local folder files. Checks/loads existing session JSON. Creates initial AgentContext with repo snapshot. Trigger: CLI process/resume or poller. No LLM. Next: SpecRefiner.
  2. **SpecRefiner**: Aggregates issue title + comments into summary. Calls Grok to analyze completeness/gaps. Posts comment if incomplete (or local file). Loops until approved. LLM: grok-4-1-fast, schema: {approved: bool, questions: Vec<String>, refined_spec: String}. Pause if not approved. Next: Planner or self.
  3. **Planner**: Scans repo/local dir. Prompts Grok for structured changes. Validates. Populates code_queue from plan. LLM: grok-4-1-fast, schema: Vec<{file_path: String, start_line: i32, end_line: i32, change_description: String}>. No pause. Next: Coder.
  4. **Coder**: Processes code_queue items: Extracts context, calls Grok for diff, applies (git apply). Includes feedback if from Tester. LLM: grok-code-fast-1, schema: {diff: String}. Optional pause for preview. Next: Tester.
  5. **Tester**: Runs cargo test/clippy/fmt in temp workspace. Parses errors. If failures, generates feedback, increments retry_count, pushes Fix tasks to code_queue front (same format as Edit). Auto-retries up to max (3, configurable). No LLM. Pause on exhaust/unfixable. Next: Coder (retry) or PRSubmitter.
  6. **PRSubmitter**: Creates feature branch, pushes, opens draft PR with summary. No LLM. Next: PostPRMonitor.
  7. **PostPRMonitor**: Posts final comment linking PR, closes session. No LLM. Halt.

## Technical Stack (current)
- Rust + tokio + reqwest + serde + octocrab + tracing family
- Custom Grok client with tools and json_schema
- Docker + docker-compose
- Prompt templates loaded from files


## Roadmap / Next Steps (priority order)
1. Implement Planner node (populate plan & code_queue)
2. Add Coder node + shared CodingTask queue
3. Tester node with Cargo runs and auto-retry loop
4. PRSubmitter + PostPRMonitor
5. Add pruning logic and richer AgentContext
6. Daemon poller / webhook support
7. Full model selector (grok-code-fast-1 etc.)



