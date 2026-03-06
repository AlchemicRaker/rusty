use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::env;
use tracing::{debug, info};

#[derive(Deserialize)]
pub struct GrokResponse {
    pub choices: Vec<Choice>,
}

#[derive(Deserialize)]
pub struct Choice {
    pub message: Message,
}

#[derive(Deserialize)]
pub struct Message {
    content: String,
}

pub struct GrokClient {
    client: reqwest::Client,
    api_key: String,
}

pub enum Model {
    Grok4_1FastReasoning,
    Grok4_1FastNonReasoning,
    GrokCodeFast1,
    Grok4Expert,
}

fn model_to_str(model: Model) -> String {
    match model {
        Model::Grok4_1FastReasoning => "grok-4-1-fast-reasoning".to_string(),
        Model::Grok4_1FastNonReasoning => "grok-4-1-fast-non-reasoning".to_string(),
        Model::GrokCodeFast1 => "grok-code-fast-1".to_string(),
        Model::Grok4Expert => "grok-4-0709".to_string(),
    }
}

impl GrokClient {
    pub fn new() -> Result<Self> {
        let api_key =
            env::var("XAI_API_KEY").expect("XAI_API_KEY environment variable is required");
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
    ) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let payload = serde_json::json!({
            "model": model_to_str(model),
            "messages": [
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
            }
        });

        let resp = self
            .client
            .post("https://api.x.ai/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&payload)
            .send()
            .await
            .expect("Failed to query Grok API");

        let raw_text = resp.text().await?;

        let parsed: GrokResponse =
            serde_json::from_str(&raw_text).expect("Failed to convert Grok Response to JSON");

        let content_str = parsed.choices[0].message.content.clone();

        let typed: T = serde_json::from_str(&content_str)
            .or_else(|_| serde_json::from_value(serde_json::from_str(&content_str)?))
            .expect("Failed to deserialize Grok Response into expected type");

        Ok(typed)
    }
}
