use async_trait::async_trait;

#[async_trait]
pub trait ZenohStore: Send + Sync {
    async fn get(&self, key_expr: &str) -> Vec<(String, String)>;
    async fn put(&self, key_expr: &str, value: &str);
    async fn delete(&self, key_expr: &str);
}
