use std::env;

use async_trait::async_trait;
use zenoh::Session;

use crate::zenoh_store::ZenohStore;

pub const ENDPOINT_ENV_VAR: &str = "ZTASK_ZENOH_ENDPOINT";
const DEFAULT_ENDPOINT: &str = "tcp/localhost:7447";

pub fn resolve_endpoint() -> String {
    env::var(ENDPOINT_ENV_VAR).unwrap_or_else(|_| DEFAULT_ENDPOINT.to_string())
}

pub async fn open_session(endpoint: &str) -> zenoh::Result<Session> {
    let mut config = zenoh::Config::default();
    config.insert_json5("connect/endpoints", &format!(r#"["{endpoint}"]"#))?;
    zenoh::open(config).await
}

pub struct RealZenohStore {
    session: Session,
}

impl RealZenohStore {
    pub fn new(session: Session) -> Self {
        Self { session }
    }
}

#[async_trait]
impl ZenohStore for RealZenohStore {
    async fn get(&self, key_expr: &str) -> Vec<(String, String)> {
        let mut results = Vec::new();
        let Ok(replies) = self.session.get(key_expr).await else {
            return results;
        };
        while let Ok(reply) = replies.recv_async().await {
            if let Ok(sample) = reply.result() {
                let value = sample
                    .payload()
                    .try_to_string()
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                results.push((sample.key_expr().to_string(), value));
            }
        }
        results
    }

    async fn put(&self, key_expr: &str, value: &str) {
        if let Err(err) = self.session.put(key_expr, value).await {
            tracing::warn!("zenoh put {key_expr} failed: {err}");
        }
    }

    async fn delete(&self, key_expr: &str) {
        if let Err(err) = self.session.delete(key_expr).await {
            tracing::warn!("zenoh delete {key_expr} failed: {err}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn resolve_endpoint_defaults_when_unset() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::remove_var(ENDPOINT_ENV_VAR);
        assert_eq!(resolve_endpoint(), "tcp/localhost:7447");
    }

    #[test]
    fn resolve_endpoint_reads_env_var() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::set_var(ENDPOINT_ENV_VAR, "tcp/zenoh-router:7447");
        let result = resolve_endpoint();
        env::remove_var(ENDPOINT_ENV_VAR);
        assert_eq!(result, "tcp/zenoh-router:7447");
    }
}
