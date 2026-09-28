use schemathesis_core::model::GeneratedCase;
use serde_json::Value;

pub struct Shrinker;

impl Default for Shrinker {
    fn default() -> Self {
        Self::new()
    }
}

impl Shrinker {
    pub fn new() -> Self {
        Self
    }

    /// Generates smaller candidate test cases from a failing test case
    pub fn candidates(&self, case: &GeneratedCase) -> Vec<GeneratedCase> {
        let mut candidates = Vec::new();

        // 1. Shrink query parameters by dropping one query parameter at a time
        if case.query_params.len() > 1 {
            for i in 0..case.query_params.len() {
                let mut shrunk_case = case.clone();
                shrunk_case.query_params.remove(i);
                candidates.push(shrunk_case);
            }
        }

        // 2. Shrink query parameter values (shorten strings / towards 0)
        for (i, (key, val)) in case.query_params.iter().enumerate() {
            if val.len() > 1 {
                let mut shrunk_case = case.clone();
                shrunk_case.query_params[i] = (key.clone(), val[..val.len() / 2].to_string());
                candidates.push(shrunk_case);
            }
        }

        // 3. Shrink JSON body
        if let Some(body) = &case.body {
            for shrunk_body in self.shrink_value(body) {
                let mut shrunk_case = case.clone();
                shrunk_case.body = Some(shrunk_body);
                candidates.push(shrunk_case);
            }
        }

        candidates
    }

    pub fn shrink_value(&self, val: &Value) -> Vec<Value> {
        let mut results = Vec::new();

        match val {
            Value::Object(map) => {
                // Try dropping one key at a time
                if map.len() > 1 {
                    for key in map.keys() {
                        let mut new_map = map.clone();
                        new_map.remove(key);
                        results.push(Value::Object(new_map));
                    }
                }

                // Try shrinking individual field values
                for (k, v) in map {
                    for shrunk_v in self.shrink_value(v) {
                        let mut new_map = map.clone();
                        new_map.insert(k.clone(), shrunk_v);
                        results.push(Value::Object(new_map));
                    }
                }
            }
            Value::Array(arr) => {
                if !arr.is_empty() {
                    // Try empty array
                    results.push(Value::Array(Vec::new()));

                    // Try half length
                    if arr.len() > 1 {
                        let half = arr[..arr.len() / 2].to_vec();
                        results.push(Value::Array(half));
                    }
                }
            }
            Value::String(s) => {
                if s.len() > 1 {
                    results.push(Value::String(s[..s.len() / 2].to_string()));
                } else if !s.is_empty() {
                    results.push(Value::String(String::new()));
                }
            }
            Value::Number(num) => {
                if let Some(i) = num.as_i64() {
                    if i != 0 {
                        results.push(Value::from(i / 2));
                        results.push(Value::from(0));
                    }
                }
            }
            _ => {}
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemathesis_core::model::HttpMethod;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn test_shrink_query_params_drops_items() {
        let shrinker = Shrinker::new();
        let case = GeneratedCase {
            method: HttpMethod::Get,
            path: "/test".into(),
            headers: HashMap::new(),
            query_params: vec![
                ("a".into(), "1".into()),
                ("b".into(), "2".into()),
                ("c".into(), "3".into()),
            ],
            body: None,
        };

        let candidates = shrinker.candidates(&case);
        assert!(!candidates.is_empty());
        // One of the candidates should have 2 query params
        assert!(candidates.iter().any(|c| c.query_params.len() == 2));
    }

    #[test]
    fn test_shrink_json_body_drops_properties() {
        let shrinker = Shrinker::new();
        let body = json!({
            "name": "Super Long Test Name",
            "age": 42,
            "roles": ["admin", "editor"]
        });

        let shrunk = shrinker.shrink_value(&body);
        assert!(!shrunk.is_empty());
        // Verify at least one shrunk value has fewer keys
        assert!(shrunk.iter().any(|v| v.as_object().is_some_and(|m| m.len() < 3)));
    }
}
