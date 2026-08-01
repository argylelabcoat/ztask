use async_trait::async_trait;

#[async_trait]
pub trait ZenohStore: Send + Sync {
    async fn get(&self, key_expr: &str) -> Vec<(String, String)>;
    async fn put(&self, key_expr: &str, value: &str);
    async fn delete(&self, key_expr: &str);
}

#[cfg(test)]
pub mod fake {
    use super::ZenohStore;
    use async_trait::async_trait;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct FakeStore {
        data: Mutex<BTreeMap<String, String>>,
        pub put_calls: Mutex<Vec<(String, String)>>,
        pub delete_calls: Mutex<Vec<String>>,
    }

    impl FakeStore {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn seed(self, key: &str, value: &str) -> Self {
            self.data.lock().unwrap().insert(key.to_string(), value.to_string());
            self
        }
    }

    fn single_segment_match(pattern: &str, key: &str) -> bool {
        let pattern_parts: Vec<&str> = pattern.split('/').collect();
        let key_parts: Vec<&str> = key.split('/').collect();
        pattern_parts.len() == key_parts.len()
            && pattern_parts
                .iter()
                .zip(key_parts.iter())
                .all(|(p, k)| *p == "*" || p == k)
    }

    fn matches(key_expr: &str, key: &str) -> bool {
        if let Some(prefix) = key_expr.strip_suffix("**") {
            key.starts_with(prefix)
        } else if key_expr.contains('*') {
            single_segment_match(key_expr, key)
        } else {
            key == key_expr
        }
    }

    #[async_trait]
    impl ZenohStore for FakeStore {
        async fn get(&self, key_expr: &str) -> Vec<(String, String)> {
            self.data
                .lock()
                .unwrap()
                .iter()
                .filter(|(k, _)| matches(key_expr, k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        }

        async fn put(&self, key_expr: &str, value: &str) {
            self.put_calls
                .lock()
                .unwrap()
                .push((key_expr.to_string(), value.to_string()));
            self.data
                .lock()
                .unwrap()
                .insert(key_expr.to_string(), value.to_string());
        }

        async fn delete(&self, key_expr: &str) {
            self.delete_calls.lock().unwrap().push(key_expr.to_string());
            let mut data = self.data.lock().unwrap();
            if let Some(prefix) = key_expr.strip_suffix("**") {
                data.retain(|k, _| !k.starts_with(prefix));
            } else {
                data.remove(key_expr);
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn get_returns_seeded_exact_match() {
            let store = FakeStore::new().seed("a/b", "v1");
            assert_eq!(store.get("a/b").await, vec![("a/b".to_string(), "v1".to_string())]);
        }

        #[tokio::test]
        async fn get_matches_wildcard_suffix() {
            let store = FakeStore::new()
                .seed("a/b/c", "v1")
                .seed("a/b/d", "v2")
                .seed("a/x", "v3");
            let mut results = store.get("a/b/**").await;
            results.sort();
            assert_eq!(
                results,
                vec![("a/b/c".to_string(), "v1".to_string()), ("a/b/d".to_string(), "v2".to_string())]
            );
        }

        #[tokio::test]
        async fn get_matches_single_segment_wildcard_shape() {
            let store = FakeStore::new()
                .seed("projects/p1/tasks/t1/status", "PENDING")
                .seed("projects/p1/tasks/t1/time_entered", "now");
            let results = store.get("projects/*/tasks/*/status").await;
            assert_eq!(results, vec![("projects/p1/tasks/t1/status".to_string(), "PENDING".to_string())]);
        }

        #[tokio::test]
        async fn put_then_get_round_trips_and_records_call() {
            let store = FakeStore::new();
            store.put("a/b", "v1").await;
            assert_eq!(store.get("a/b").await, vec![("a/b".to_string(), "v1".to_string())]);
            assert_eq!(store.put_calls.lock().unwrap().as_slice(), [("a/b".to_string(), "v1".to_string())]);
        }

        #[tokio::test]
        async fn delete_removes_matching_prefix_and_records_call() {
            let store = FakeStore::new().seed("a/b/c", "v1").seed("a/x", "v2");
            store.delete("a/b/**").await;
            assert_eq!(store.get("a/b/**").await, Vec::<(String, String)>::new());
            assert_eq!(store.get("a/x").await, vec![("a/x".to_string(), "v2".to_string())]);
            assert_eq!(store.delete_calls.lock().unwrap().as_slice(), ["a/b/**".to_string()]);
        }
    }
}
