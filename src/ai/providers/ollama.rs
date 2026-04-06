use std::time::Duration;
use anyhow::Result;
use crate::ai::provider::LlmProvider;
use crate::ai::embedder::Embedder;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

pub struct OllamaProvider {
    client: reqwest::Client,
    url: String,
    model: String,
}

impl OllamaProvider {
    pub fn new(url: String, model: String) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("failed to build HTTP client"),
            url,
            model,
        }
    }
}

impl LlmProvider for OllamaProvider {
    async fn complete(&self, prompt: &str, system: Option<&str>) -> Result<String> {
        let mut body = serde_json::json!({
            "model": self.model,
            "prompt": prompt,
            "stream": false,
        });
        if let Some(sys) = system {
            body["system"] = serde_json::Value::String(sys.to_string());
        }

        let resp = self.client
            .post(format!("{}/api/generate", self.url))
            .json(&body)
            .send()
            .await?
            .error_for_status()?
            .json::<serde_json::Value>()
            .await?;

        Ok(resp["response"].as_str().unwrap_or("").to_string())
    }

    fn name(&self) -> &str { "ollama" }
}

pub struct OllamaEmbedder {
    client: reqwest::Client,
    url: String,
    model: String,
}

impl OllamaEmbedder {
    pub fn new(url: String, model: String) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("failed to build HTTP client"),
            url,
            model,
        }
    }
}

impl Embedder for OllamaEmbedder {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut results = Vec::new();
        for text in texts {
            let resp = self.client
                .post(format!("{}/api/embed", self.url))
                .json(&serde_json::json!({
                    "model": self.model,
                    "input": text,
                }))
                .send()
                .await?
                .error_for_status()?
                .json::<serde_json::Value>()
                .await?;

            let embedding: Vec<f32> = resp["embeddings"][0]
                .as_array()
                .unwrap_or(&Vec::new())
                .iter()
                .map(|v| v.as_f64().unwrap_or(0.0) as f32)
                .collect();
            results.push(embedding);
        }
        Ok(results)
    }

    fn dimension(&self) -> usize { 768 }
    fn model_name(&self) -> &str { &self.model }
    fn cost_per_token(&self) -> Option<f64> { None } // Local, free
}
