use std::time::Duration;
use anyhow::Result;
use crate::ai::provider::LlmProvider;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

pub struct ClaudeProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl ClaudeProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("failed to build HTTP client"),
            api_key,
            model,
        }
    }
}

impl LlmProvider for ClaudeProvider {
    async fn complete(&self, prompt: &str, system: Option<&str>) -> Result<String> {
        let mut body = serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "messages": [
                { "role": "user", "content": prompt }
            ]
        });

        if let Some(sys) = system {
            body["system"] = serde_json::Value::String(sys.to_string());
        }

        let resp = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;

        // Claude API returns content as an array of content blocks
        let content = resp["content"]
            .as_array()
            .and_then(|blocks| {
                blocks.iter()
                    .filter(|b| b["type"].as_str() == Some("text"))
                    .map(|b| b["text"].as_str().unwrap_or(""))
                    .collect::<Vec<_>>()
                    .into_iter()
                    .reduce(|a, b| if a.is_empty() { b } else { a })
            })
            .unwrap_or("")
            .to_string();

        Ok(content)
    }

    fn name(&self) -> &str { "claude" }
}
