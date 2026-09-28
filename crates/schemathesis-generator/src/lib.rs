pub mod shrink;
pub mod strategy;

pub use shrink::Shrinker;
pub use strategy::ValueGenerator;

use schemathesis_core::model::{GeneratedCase, Operation, ParameterLocation};
use std::collections::HashMap;

pub struct CaseGenerator {
    value_generator: ValueGenerator,
}

impl CaseGenerator {
    pub fn new() -> Self {
        Self {
            value_generator: ValueGenerator::new(),
        }
    }

    pub fn with_seed(seed: u64) -> Self {
        Self {
            value_generator: ValueGenerator::with_seed(seed),
        }
    }

    /// Generates a single test case for an operation
    pub fn generate_case(&self, operation: &Operation) -> GeneratedCase {
        let mut path = operation.path.clone();
        let mut query_params = Vec::new();
        let mut headers = HashMap::new();

        for param in &operation.parameters {
            let sample_value = match &param.schema {
                Some(schema) => {
                    let val = self.value_generator.generate_from_schema(schema);
                    match val {
                        serde_json::Value::String(s) => s,
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        other => other.to_string(),
                    }
                }
                None => "sample".to_string(),
            };

            match param.location {
                ParameterLocation::Path => {
                    path = path.replace(&format!("{{{}}}", param.name), &sample_value);
                }
                ParameterLocation::Query => {
                    query_params.push((param.name.clone(), sample_value));
                }
                ParameterLocation::Header => {
                    headers.insert(param.name.clone(), sample_value);
                }
                ParameterLocation::Cookie => {}
            }
        }

        let body = operation.request_body.as_ref().map(|rb| {
            // Check for application/json content schema
            if let Some(content_schema) = rb
                .get("content")
                .and_then(|c| c.get("application/json"))
                .and_then(|json_obj| json_obj.get("schema"))
            {
                self.value_generator.generate_from_schema(content_schema)
            } else {
                self.value_generator.generate_from_schema(rb)
            }
        });

        GeneratedCase {
            method: operation.method.clone(),
            path,
            headers,
            query_params,
            body,
        }
    }

    /// Generates multiple varied test cases for an operation
    pub fn generate_cases(&self, operation: &Operation, count: usize) -> Vec<GeneratedCase> {
        let mut cases = Vec::with_capacity(count);
        for _ in 0..count {
            cases.push(self.generate_case(operation));
        }
        cases
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemathesis_core::model::{HttpMethod, Parameter, ParameterLocation};

    #[test]
    fn test_generate_multiple_cases_with_request_body() {
        let op = Operation {
            id: Some("createUser".into()),
            method: HttpMethod::Post,
            path: "/users".into(),
            parameters: vec![Parameter {
                name: "version".into(),
                location: ParameterLocation::Query,
                required: true,
                schema: Some(serde_json::json!({ "type": "string", "enum": ["v1", "v2"] })),
            }],
            request_body: Some(serde_json::json!({
                "content": {
                    "application/json": {
                        "schema": {
                            "type": "object",
                            "properties": {
                                "email": { "type": "string", "format": "email" },
                                "age": { "type": "integer", "minimum": 18, "maximum": 99 }
                            }
                        }
                    }
                }
            })),
            responses: HashMap::new(),
        };

        let generator = CaseGenerator::new();
        let cases = generator.generate_cases(&op, 3);

        assert_eq!(cases.len(), 3);
        for case in cases {
            assert_eq!(case.method, HttpMethod::Post);
            assert_eq!(case.path, "/users");
            assert_eq!(case.query_params.len(), 1);
            assert!(["v1", "v2"].contains(&case.query_params[0].1.as_str()));
            assert!(case.body.is_some());
            let body = case.body.unwrap();
            assert_eq!(
                body.get("email").and_then(|e| e.as_str()),
                Some("test.user@example.com")
            );
        }
    }
}
