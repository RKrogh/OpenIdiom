use anyhow::Result;

#[allow(dead_code)]
pub trait LlmProvider: Send + Sync {
    fn complete(
        &self,
        prompt: &str,
        system: Option<&str>,
    ) -> impl std::future::Future<Output = Result<String>> + Send;

    fn name(&self) -> &str;
}
