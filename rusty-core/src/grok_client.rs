use anyhow::Result;
use serde::Deserialize;
use std::env;
use tracing::debug;

use crate::tools;

#[derive(Deserialize)]
pub struct GrokResponse {
    // pub id: String,
    pub output: Vec<OutputItem>,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
pub enum OutputItem {
    #[serde(rename = "message")]
    Message { content: Vec<ContentItem> },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
pub struct ContentItem {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

pub struct GrokClient {
    client: reqwest::Client,
    api_key: String,
}

pub enum Model {
    Grok4_1FastReasoning,
    // Grok4_1FastNonReasoning,
    // GrokCodeFast1,
    // Grok4Expert,
}

pub enum Tool {
    WebSearch,
    // XSearch,
    // CodeExecution,
    // GetRepoOverview,
    // ListDirectory,
    ReadFile,
    // GrepSearch,
    // FindFiles,
}

pub enum ToolCall {
    ReadFile {
        file_path: String,
        start_line: Option<usize>,
        end_line: Option<usize>,
    },
}

impl Tool {
    pub fn to_definition(&self) -> serde_json::Value {
        match self {
            Tool::WebSearch => serde_json::json!({"type": "web_search"}),
            Tool::ReadFile => serde_json::json!({
                "type": "function",
                "function": {
                    "name": "read_file",
                    "description": "Read a file from the workspace with optional line range. Always use relative paths.",
                    "parameters": {
                        "file_path": {"type":"string","description": "Relative path from repository root"},
                        "start_line": {"type":"integer","description":"1-based start line (optional)"},
                        "end_line": {"type":"integer","description":"1-based end line (optional)"},
                    },
                    "required": ["file_path"],
                    "additionalProperties": false
                }
            }),
        }
    }
}

async fn execute_tool(call: ToolCall) -> Result<String> {
    match call {
        ToolCall::ReadFile {
            file_path,
            start_line,
            end_line,
        } => tools::read_file("/workspace", file_path, start_line, end_line).await,
    }
}

fn model_to_str(model: Model) -> String {
    match model {
        Model::Grok4_1FastReasoning => "grok-4-1-fast-reasoning".to_string(),
        // Model::Grok4_1FastNonReasoning => "grok-4-1-fast-non-reasoning".to_string(),
        // Model::GrokCodeFast1 => "grok-code-fast-1".to_string(),
        // Model::Grok4Expert => "grok-4-0709".to_string(),
    }
}

impl GrokClient {
    pub fn new() -> Result<Self> {
        let api_key =
            env::var("XAI_API_KEY").expect("XAI_API_KEY environment variable is required!");
        Ok(Self {
            client: reqwest::Client::new(),
            api_key,
        })
    }

    pub async fn call<T>(
        &self,
        model: Model,
        system_prompt: &str,
        user_prompt: &str,
        schema: serde_json::Value,
        schema_name: &str,
        tools: Option<Vec<Tool>>,
    ) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let tool_defs: Vec<serde_json::Value> = tools
            .unwrap_or_default()
            .iter()
            .map(|t| t.to_definition())
            .collect();

        let payload = serde_json::json!({
            "model": model_to_str(model),
            "input": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_prompt},
            ],
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": schema_name,
                    "strict": true,
                    "schema": schema,
                }
            },
            "tools": tool_defs,
        });

        let resp = self
            .client
            .post("https://api.x.ai/v1/responses")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&payload)
            .send()
            .await
            .expect("Failed to query Grok API");

        let raw_text = resp.text().await?;
        debug!("Raw Grok /responses body: {}", &raw_text);

        let parsed: GrokResponse =
            serde_json::from_str(&raw_text).expect("Failed to convert Grok Response to JSON");

        let content_str = parsed
            .output
            .into_iter()
            .find_map(|item| match item {
                OutputItem::Message { content } => content
                    .into_iter()
                    .find(|c| c.content_type == "output_text")
                    .map(|c| c.text),
                _ => None,
            })
            .ok_or_else(|| anyhow::anyhow!("No output_text content in Grok response"))?;

        let typed: T = serde_json::from_str(&content_str)
            .expect("Failed to deserialize Grok Response into expected type");

        Ok(typed)
    }
}
