use schemathesis_core::model::{GeneratedCase, Operation, ParameterLocation};
use std::collections::HashMap;

pub struct CaseGenerator;

impl CaseGenerator {
    pub fn new() -> Self {
        Self
    }

    /// Generates a test case for a given operation with default/sample values
    pub fn generate_case(&self, operation: &Operation) -> GeneratedCase {
        let mut path = operation.path.clone();
        let mut query_params = Vec::new();
        let mut headers = HashMap::new();

        for param in &operation.parameters {
            let sample_value = match &param.schema {
                Some(schema) => match schema.get("type").and_then(|v| v.as_str()) {
                    Some("integer") => "42".to_string(),
                    Some("boolean") => "true".to_string(),
                    _ => "test_value".to_string(),
                },
                None => "test_value".to_string(),
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

        GeneratedCase {
            method: operation.method.clone(),
            path,
            headers,
            query_params,
            body: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemathesis_core::model::{HttpMethod, Parameter, ParameterLocation};

    #[test]
    fn test_generate_case_replaces_path_parameters() {
        let op = Operation {
            id: Some("getUser".into()),
            method: HttpMethod::Get,
            path: "/users/{userId}/posts/{postId}".into(),
            parameters: vec![
                Parameter {
                    name: "userId".into(),
                    location: ParameterLocation::Path,
                    required: true,
                    schema: Some(serde_json::json!({ "type": "integer" })),
                },
                Parameter {
                    name: "postId".into(),
                    location: ParameterLocation::Path,
                    required: true,
                    schema: Some(serde_json::json!({ "type": "string" })),
                },
            ],
            request_body: None,
            responses: HashMap::new(),
        };

        let generator = CaseGenerator::new();
        let case = generator.generate_case(&op);

        assert_eq!(case.method, HttpMethod::Get);
        assert_eq!(case.path, "/users/42/posts/test_value");
    }
}
