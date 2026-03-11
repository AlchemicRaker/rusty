use anyhow::Result;
use anyhow::anyhow;
use serde::Deserialize;
use std::env;
use tracing::{debug, info};

use crate::tools;

#[derive(Deserialize)]
pub struct GrokResponse {
    pub id: String,
    pub output: Vec<OutputItem>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
pub enum OutputItem {
    #[serde(rename = "message")]
    Message { content: Vec<ContentItem> },

    #[serde(rename = "function_call")]
    FunctionCall {
        call_id: String,
        name: String,
        arguments: String,
    },

    #[serde(other)]
    Other,
}

#[derive(Deserialize, Debug)]
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

impl Tool {
    pub fn to_definition(&self) -> serde_json::Value {
        match self {
            Tool::WebSearch => serde_json::json!({"type": "web_search"}),

            Tool::ReadFile => serde_json::json!({
                "type": "function",
                "name": "read_file",
                "description": "CRITICAL TOOL - ALWAYS USE THIS to inspect the codebase. You MUST provide a concrete file_path. Examples: 'Cargo.toml', 'src/main.rs', 'src/lib.rs', 'README.md'. Never call this tool without a valid file_path. Use forward slashes only.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "file_path": {
                            "type": "string",
                            "description": "REQUIRED. Relative path from repository root (e.g. Cargo.toml or src/main.rs)"
                        },
                        "start_line": {
                            "type": "integer",
                            "description": "Optional. 1-based start line"
                        },
                        "end_line": {
                            "type": "integer",
                            "description": "Optional. 1-based end line"
                        }
                    },
                    "required": ["file_path"],
                    "additionalProperties": false
                }
            }),
        }
    }
}

pub enum ToolCall {
    ReadFile {
        file_path: String,
        start_line: Option<usize>,
        end_line: Option<usize>,
    },
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

        let mut previous_response_id: Option<String> = None;
        let mut input = vec![
            serde_json::json!({"role": "system", "content": system_prompt}),
            serde_json::json!({"role": "user", "content": user_prompt}),
        ];
        let model_string = model_to_str(model);

        const MAX_TOOL_TURNS: usize = 150;

        for turn in 1..=MAX_TOOL_TURNS {
            let is_final_turn = turn == MAX_TOOL_TURNS;
            let tools_for_this_turn = if is_final_turn {
                vec![]
            } else {
                tool_defs.clone()
            };
            let payload = if is_final_turn || tool_defs.iter().count() == 0 {
                serde_json::json!({
                    "model": model_string,
                    "input": input,
                    "response_format": {
                        "type": "json_schema",
                        "json_schema": {
                            "name": schema_name,
                            "strict": true,
                            "schema": schema,
                        }
                    },
                    "previous_response_id": previous_response_id,
                })
            } else {
                serde_json::json!({
                    "model": model_string,
                    "input": input,
                    "response_format": {
                        "type": "json_schema",
                        "json_schema": {
                            "name": schema_name,
                            "strict": true,
                            "schema": schema,
                        }
                    },
                    "tools": tools_for_this_turn,
                    "tool_choice": "auto",
                    "previous_response_id": previous_response_id,
                })
            };

            let resp = self
                .client
                .post("https://api.x.ai/v1/responses")
                .header("Authorization", format!("Bearer {}", self.api_key))
                .json(&payload)
                .send()
                .await?;

            let raw_text = resp.text().await?;
            debug!("Grok response (turn {}) raw: {}", turn, &raw_text);

            let parsed: GrokResponse =
                serde_json::from_str(&raw_text).expect("Failed to convert Grok Response to JSON");

            previous_response_id = Some(parsed.id.clone());

            let mut tool_outputs = vec![];
            for item in &parsed.output {
                if let OutputItem::FunctionCall {
                    call_id,
                    name,
                    arguments,
                } = item
                {
                    info!("Grok called tool: {} (call_id: {})", name, call_id);
                    info!("With arguments: {}", arguments);

                    let tool_call = match name.as_str() {
                        "read_file" => {
                            // Defensive: if Grok forgets parameters, tell it immediately
                            if arguments.trim() == "{}" || arguments.trim().is_empty() {
                                info!(
                                    "Grok called read_file with EMPTY arguments — sending correction"
                                );
                                tool_outputs.push(serde_json::json!({
                                    "type": "function_call_output",
                                    "call_id": call_id,
                                    "output": "ERROR: read_file was called without any arguments. You MUST provide 'file_path'. Example: {\"file_path\": \"Cargo.toml\"}. Try again with a valid file path."
                                }));
                                continue;
                            }
                            let args: serde_json::Value = serde_json::from_str(arguments)
                                .unwrap_or_else(|_| serde_json::json!({}));
                            ToolCall::ReadFile {
                                file_path: args["file_path"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .to_string(),
                                start_line: args["start_line"].as_u64().map(|v| v as usize),
                                end_line: args["end_line"].as_u64().map(|v| v as usize),
                            }
                        }
                        _ => continue,
                    };

                    let result = execute_tool(tool_call).await?;
                    info!("Tool response: {}", result);
                    tool_outputs.push(serde_json::json!({
                        "type": "function_call_output",
                        "call_id": call_id,
                        "output": result
                    }));
                }
            }

            if tool_outputs.is_empty() {
                // final structured response
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

                //TODO: handle errors: {"code":"The service is currently unavailable","error":"Service temporarily unavailable. The model is at capacity and currently cannot serve this request. Please try again later."}
                let typed: T = serde_json::from_str(&content_str)
                    .expect("Failed to deserialize Grok Response into expected type");

                return Ok(typed);
            }

            input = tool_outputs;

            if turn == MAX_TOOL_TURNS - 1 {
                info!(
                    "Tool limit reached ({} turns). Informing Grok tools are exhausted.",
                    MAX_TOOL_TURNS,
                );
                input.push(serde_json::json!({
                    "role": "user",
                    "content": "IMPORTANT: Tools are no longer available. You have reached the maximum number of tool calls. If you still cannot resolve the situation, analyze and challenge your approach to solving the problem, identify and report how it went wrong, and provide your final best JSON response according to the required schema anyway."
                }));
                // One final loop
            }
        }

        Err(anyhow!("Unexpected exit from tool loop"))
    }
}
