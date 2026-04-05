use anyhow::Result;

#[allow(dead_code)]
pub trait Embedder: Send + Sync {
    fn embed(
        &self,
        texts: &[String],
    ) -> impl std::future::Future<Output = Result<Vec<Vec<f32>>>> + Send;

    fn dimension(&self) -> usize;
    fn model_name(&self) -> &str;
    fn cost_per_token(&self) -> Option<f64>;
}
