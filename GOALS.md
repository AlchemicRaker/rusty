# GOALS.md: Rusty Project Design and Roadmap

This document captures the full design requirements, goals, and discussion points from the planning phase. It serves as a reference to resume development if paused. Based on the draft requirements (v7), with all details incorporated, including node graph purposes, LLM specifics, and other elements discussed.

## Project Overview
- **Title**: Rusty (Rust + Grok SWE Agent)
- **Description**: A reliable, senior-engineer-in-the-loop Software Engineering Agent that consumes GitHub Issues (or local folders), collaborates via issue comments until spec approved, then plans → codes (queue-based) → tests (with auto-retries) → opens draft PR on secondary branch. Never merges to protected branches. Designed for phone-based oversight and full human review.
- **End Goal**: Integrated with GitHub for Issues → discussion → PR creation. For private/personal repos. Agent acts as a "senior engineer" for code review but enables coding via phone (no typing code manually).

## Goals & Scope
- Private, personal use on private repositories.
- Strict code-review flow: human always reviews every change.
- Budget-conscious model usage (selector for tasks).
- No media generation (focus on code).
- Full control via custom node graph (no pre-baked frameworks).
- Avoid naming conflict with existing SWE-agent project.
- Treat users as adults; no moralizing.
- Current State: WIP with Cargo workspace, deps (tokio, serde, reqwest, tracing, clap), stub runner in rusty-core, CLI in rusty-cli. Logging fixed with blocking appender. Dummy nodes and resumability implemented.

## Functional Requirements
- **Node Graph**: Custom typed node graph with 7 core nodes. Each node is an async fn that takes &mut AgentContext and returns ControlFlow (Continue(Node), Pause(reason), Halt). Purposes:
  1. **IssueIngestor**: Loads issue details/comments (via octocrab) or local folder files. Checks/loads existing session JSON. Creates initial AgentContext with repo snapshot. Trigger: CLI process/resume or poller. No LLM. Next: SpecRefiner.
  2. **SpecRefiner**: Aggregates issue title + comments into summary. Calls Grok to analyze completeness/gaps. Posts comment if incomplete (or local file). Loops until approved. LLM: grok-4-1-fast, schema: {approved: bool, questions: Vec<String>, refined_spec: String}. Pause if not approved. Next: Planner or self.
  3. **Planner**: Scans repo/local dir. Prompts Grok for structured changes. Validates. Populates code_queue from plan. LLM: grok-4-1-fast, schema: Vec<{file_path: String, start_line: i32, end_line: i32, change_description: String}>. No pause. Next: Coder.
  4. **Coder**: Processes code_queue items: Extracts context, calls Grok for diff, applies (git apply). Includes feedback if from Tester. LLM: grok-code-fast-1, schema: {diff: String}. Optional pause for preview. Next: Tester.
  5. **Tester**: Runs cargo test/clippy/fmt in temp workspace. Parses errors. If failures, generates feedback, increments retry_count, pushes Fix tasks to code_queue front (same format as Edit). Auto-retries up to max (3, configurable). No LLM. Pause on exhaust/unfixable. Next: Coder (retry) or PRSubmitter.
  6. **PRSubmitter**: Creates feature branch, pushes, opens draft PR with summary. No LLM. Next: PostPRMonitor.
  7. **PostPRMonitor**: Posts final comment linking PR, closes session. No LLM. Halt.
- **LLM Integration**: Custom Grok client in REST API with json_schema enforcement. Typed calls, retry on babble. Prompts specify schema in text for reliability.
- **Tester Retry Loop**: Auto-feedback to Coder for compile/lint/test fixes (max 3 retries; no human pause until exhausted). Uses same code_queue format (CodingTask::Edit or ::Fix with feedback).
- **Code Queue**: Shared Vec<CodingTask> in context. Planner populates with Edit from plan; Tester adds Fix with failure_summary.
- **Resumability**: Serialize full AgentContext to JSON on pause/step. Load on resume. Sessions in ./data/sessions/.
- **--step Flag**: Top-level only—forces pause after every node. In orchestrator loop.
- **Human-in-the-Loop**: Via GitHub Issue comments (post questions, wait for replies). Local: stdout + file.
- **Local Testing Mode**: --local flag + Docker volume mount. Trait-based RepoService (GitHub impl with octocrab, Local with fs/git).
- **Triggers**: CLI commands (process <id> [--local /path] [--step], resume <id>, daemon for 60s polling). Architecture webhook-ready (pluggable EventSource trait).
- **Pruning Rules**: For conversation_history: Aggressive—latest ~2 full messages, 3rd as LLM-reduced summary of older (grok-4-1-fast, non-reasoning/direct, fallback to truncate if offline). Reduce iteratively until <=3. Prompt: history_reducer.txt.
- **Debugging**: Temporary last_grok_responses in context (runtime-only, cleared post-step by runner).

## Non-Functional Requirements
- rusty-core as pure lib crate (decoupled from interfaces).
- Custom Grok client: json_schema, typed responses, retry.
- Docker + docker-compose (volumes for data/logs, env config, AWS-migration friendly: no host binds, secrets).
- Logging: tracing to rolling files + console (blocking appender for reliability in short runs).
- Persistence: In-memory + JSON sessions (Postgres later, AWS-hosted).
- Security: Token scopes prevent merges/pushes to main; secrets via env (GROK_API_KEY, GITHUB_TOKEN).
- Performance: Response times under 5s target; handle large codebases scalably.
- Compatibility: Latest stable Rust, cross-OS.

## Technical Stack
- **Language**: Rust (latest stable)
- **Async**: tokio
- **HTTP**: reqwest
- **Serialization**: serde
- **GitHub**: octocrab
- **Logging**: tracing + tracing-subscriber + tracing-appender (blocking)
- **Container**: Docker + docker-compose
- **CLI**: rusty-cli crate (clap)
- **Repo Abstraction**: RepoService trait (GitHub + Local)
- **Models**: grok-4-1-fast (planning/refining/pruning), grok-code-fast-1 (coding), grok-4 (rare high-stakes)
- **Prompt Templates**: File-based (./prompts/*.txt), loaded at runtime. Initials:
  - spec_refiner.txt: "You are a spec refiner for a software agent. Given this GitHub issue title: {title}, description: {desc}, and conversation history: {history}. Analyze for completeness. If gaps exist, list questions. If ready, approve and provide a refined spec. Respond only in JSON: {schema}."
  - planner.txt: "You are a code planner. Given spec: {spec} and codebase overview (files: {file_list}, key contents: {snippets}). Plan minimal changes needed. For each: file_path (relative), start_line, end_line, change_description (precise, no code). Respond only in JSON array: {schema}."
  - coder.txt: "You are a Rust coder. Edit this file snippet (lines {start}-{end}): {context}. Apply this change: {description}. If feedback: {feedback}. Output only the unified diff patch in JSON: {schema}. No explanations."
  - history_reducer.txt: "Summarize this older conversation history concisely, keeping key specs/decisions: {old_history}. Respond only in JSON: { 'summary': string }."

## AgentContext (State Object) – Flexible Guidance
Struct with: session_id: String, repo: RepoInfo, github_issue: Option<GitHubIssue>, current_node: Node, conversation_history: Vec<Message>, plan: Vec<PlannedChange>, code_queue: Vec<CodingTask>, retry_count: u8, max_retries: u8 (3 default), test_results: Option<TestSummary>, pr_info: Option<PrInfo>, created_at/last_paused_at: DateTime<Utc>, total_tokens_used: u64. Temporary last_grok_responses: Vec<GrokResponse> (cleared post-step). Evolve as needed; not rigid.

## Constraints & Assumptions
- Personal project—keep simple, no over-engineering.
- Agent never pushes/merges to protected branches.
- All changes human-reviewed.
- Polling (60s) acceptable for MVP but webhook-upgrade ready.
- Local mode for fast testing.
- Retry limiter + queue prevents cost spirals.
- Pruning uses cheap, non-reasoning LLM; no special offline contingency (Grok dependency inherent).
- Prompts include schema description despite API enforcement for better model compliance.

## Roadmap / Next Steps
- Implement IssueIngestor + RepoService trait.
- Add Grok client + first LLM node (SpecRefiner).
- Integrate octocrab for GitHub.
- Docker setup.
- Full nodes, prompts dir, pruning logic.
- Daemon poller.
- Webhook support.
- Postgres migration.
- AWS compatibility checks.

Discussions: Use REST API (not chat completions), json_schema for stability. Security/reliability as needed. Custom state machines for control. Model strategy for budget. No web UI, CLI-first then GitHub. Name change from swe-agent to Rusty.