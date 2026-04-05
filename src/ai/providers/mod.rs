pub mod claude;
pub mod openai;
pub mod ollama;

use anyhow::Result;
use crate::core::vault::AiSection;

use self::claude::ClaudeProvider;
use self::openai::{OpenAiProvider, OpenAiEmbedder};
use self::ollama::{OllamaProvider, OllamaEmbedder};

pub enum AnyProvider {
    Claude(ClaudeProvider),
    OpenAi(OpenAiProvider),
    Ollama(OllamaProvider),
}

pub enum AnyEmbedder {
    OpenAi(OpenAiEmbedder),
    Ollama(OllamaEmbedder),
}

impl AnyProvider {
    pub async fn complete(&self, prompt: &str, system: Option<&str>) -> Result<String> {
        use crate::ai::provider::LlmProvider;
        match self {
            Self::Claude(p) => p.complete(prompt, system).await,
            Self::OpenAi(p) => p.complete(prompt, system).await,
            Self::Ollama(p) => p.complete(prompt, system).await,
        }
    }

    #[allow(dead_code)]
    pub fn name(&self) -> &str {
        use crate::ai::provider::LlmProvider;
        match self {
            Self::Claude(p) => p.name(),
            Self::OpenAi(p) => p.name(),
            Self::Ollama(p) => p.name(),
        }
    }
}

impl AnyEmbedder {
    pub async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        use crate::ai::embedder::Embedder;
        match self {
            Self::OpenAi(e) => e.embed(texts).await,
            Self::Ollama(e) => e.embed(texts).await,
        }
    }

    #[allow(dead_code)]
    pub fn dimension(&self) -> usize {
        use crate::ai::embedder::Embedder;
        match self {
            Self::OpenAi(e) => e.dimension(),
            Self::Ollama(e) => e.dimension(),
        }
    }

    pub fn model_name(&self) -> &str {
        use crate::ai::embedder::Embedder;
        match self {
            Self::OpenAi(e) => e.model_name(),
            Self::Ollama(e) => e.model_name(),
        }
    }

    pub fn cost_per_token(&self) -> Option<f64> {
        use crate::ai::embedder::Embedder;
        match self {
            Self::OpenAi(e) => e.cost_per_token(),
            Self::Ollama(e) => e.cost_per_token(),
        }
    }
}

pub fn create_provider(config: &AiSection) -> Result<AnyProvider> {
    match config.provider.as_str() {
        "claude" => {
            let api_key = std::env::var("ANTHROPIC_API_KEY")
                .map_err(|_| anyhow::anyhow!(
                    "ANTHROPIC_API_KEY not set. Required for provider 'claude'."
                ))?;
            let model = config.model.clone()
                .unwrap_or_else(|| "claude-sonnet-4-6".into());
            Ok(AnyProvider::Claude(ClaudeProvider::new(api_key, model)))
        }
        "openai" => {
            let api_key = std::env::var("OPENAI_API_KEY")
                .map_err(|_| anyhow::anyhow!(
                    "OPENAI_API_KEY not set. Required for provider 'openai'."
                ))?;
            let base_url = config.base_url.clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".into());
            let model = config.model.clone()
                .unwrap_or_else(|| "gpt-4".into());
            Ok(AnyProvider::OpenAi(OpenAiProvider::new(api_key, base_url, model)))
        }
        "ollama" => {
            let url = config.ollama_url.clone()
                .unwrap_or_else(|| "http://localhost:11434".into());
            let model = config.model.clone()
                .unwrap_or_else(|| "llama3".into());
            Ok(AnyProvider::Ollama(OllamaProvider::new(url, model)))
        }
        other => anyhow::bail!("Unknown provider: {other}"),
    }
}

pub fn create_embedder(config: &AiSection) -> Result<AnyEmbedder> {
    match config.embedding_provider.as_str() {
        "openai" => {
            let api_key = std::env::var("OPENAI_API_KEY")
                .map_err(|_| anyhow::anyhow!(
                    "OPENAI_API_KEY not set. Required for embedding provider 'openai'."
                ))?;
            let base_url = config.base_url.clone()
                .unwrap_or_else(|| "https://api.openai.com/v1".into());
            Ok(AnyEmbedder::OpenAi(OpenAiEmbedder::new(
                api_key, base_url, config.embedding_model.clone(),
            )))
        }
        "ollama" => {
            let url = config.ollama_url.clone()
                .unwrap_or_else(|| "http://localhost:11434".into());
            Ok(AnyEmbedder::Ollama(OllamaEmbedder::new(
                url, config.embedding_model.clone(),
            )))
        }
        other => anyhow::bail!("Unknown embedding provider: {other}"),
    }
}
