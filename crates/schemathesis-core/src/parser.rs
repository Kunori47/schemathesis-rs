use crate::error::CoreError;
use crate::model::{ApiSpecification, HttpMethod, Operation, Parameter, ParameterLocation};
use serde_json::Value;
use std::collections::HashMap;

pub fn parse_openapi_json(raw: &str) -> Result<ApiSpecification, CoreError> {
    let json: Value = serde_json::from_str(raw)
        .map_err(|e| CoreError::SchemaParseError(e.to_string()))?;
    parse_openapi_value(&json)
}

pub fn parse_openapi_yaml(raw: &str) -> Result<ApiSpecification, CoreError> {
    let val: Value = serde_yaml::from_str(raw)
        .map_err(|e| CoreError::SchemaParseError(e.to_string()))?;
    parse_openapi_value(&val)
}

fn parse_openapi_value(root: &Value) -> Result<ApiSpecification, CoreError> {
    let info = root.get("info").ok_or_else(|| CoreError::MissingField("info".into()))?;
    let title = info.get("title")
        .and_then(Value::as_str)
        .unwrap_or("Untitled API")
        .to_string();
    let version = info.get("version")
        .and_then(Value::as_str)
        .unwrap_or("1.0.0")
        .to_string();

    let servers = root.get("servers")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|s| s.get("url").and_then(Value::as_str).map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let paths = root.get("paths")
        .and_then(Value::as_object)
        .ok_or_else(|| CoreError::MissingField("paths".into()))?;

    let mut operations = Vec::new();

    for (path, item) in paths {
        if let Some(item_obj) = item.as_object() {
            for (method_str, op_val) in item_obj {
                let method = match method_str.to_lowercase().as_str() {
                    "get" => HttpMethod::Get,
                    "post" => HttpMethod::Post,
                    "put" => HttpMethod::Put,
                    "delete" => HttpMethod::Delete,
                    "patch" => HttpMethod::Patch,
                    "head" => HttpMethod::Head,
                    "options" => HttpMethod::Options,
                    _ => continue,
                };

                let op_id = op_val.get("operationId").and_then(Value::as_str).map(String::from);

                let mut parameters = Vec::new();
                if let Some(param_arr) = op_val.get("parameters").and_then(Value::as_array) {
                    for p in param_arr {
                        if let (Some(name), Some(loc_str)) = (
                            p.get("name").and_then(Value::as_str),
                            p.get("in").and_then(Value::as_str),
                        ) {
                            let location = match loc_str {
                                "query" => ParameterLocation::Query,
                                "header" => ParameterLocation::Header,
                                "path" => ParameterLocation::Path,
                                "cookie" => ParameterLocation::Cookie,
                                _ => continue,
                            };
                            let required = p.get("required").and_then(Value::as_bool).unwrap_or(false);
                            let schema = p.get("schema").cloned();

                            parameters.push(Parameter {
                                name: name.to_string(),
                                location,
                                required,
                                schema,
                            });
                        }
                    }
                }

                let request_body = op_val.get("requestBody").cloned();
                let mut responses = HashMap::new();
                if let Some(resp_obj) = op_val.get("responses").and_then(Value::as_object) {
                    for (code, resp_val) in resp_obj {
                        responses.insert(code.clone(), resp_val.clone());
                    }
                }

                operations.push(Operation {
                    id: op_id,
                    method,
                    path: path.clone(),
                    parameters,
                    request_body,
                    responses,
                });
            }
        }
    }

    Ok(ApiSpecification {
        title,
        version,
        servers,
        operations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_valid_openapi_json() {
        let raw = r#"{
            "openapi": "3.0.3",
            "info": {
                "title": "Sample API",
                "version": "1.0.0"
            },
            "paths": {
                "/users/{id}": {
                    "get": {
                        "operationId": "getUserById",
                        "parameters": [
                            {
                                "name": "id",
                                "in": "path",
                                "required": true,
                                "schema": { "type": "integer" }
                            }
                        ],
                        "responses": {
                            "200": { "description": "OK" },
                            "404": { "description": "Not Found" }
                        }
                    }
                }
            }
        }"#;

        let spec = parse_openapi_json(raw).expect("should parse valid OpenAPI");
        assert_eq!(spec.title, "Sample API");
        assert_eq!(spec.version, "1.0.0");
        assert_eq!(spec.operations.len(), 1);

        let op = &spec.operations[0];
        assert_eq!(op.method, HttpMethod::Get);
        assert_eq!(op.path, "/users/{id}");
        assert_eq!(op.parameters.len(), 1);
        assert_eq!(op.parameters[0].name, "id");
        assert_eq!(op.parameters[0].location, ParameterLocation::Path);
        assert!(op.parameters[0].required);
        assert_eq!(op.responses.len(), 2);
    }

    #[test]
    fn test_parse_missing_paths_returns_error() {
        let raw = r#"{
            "openapi": "3.0.3",
            "info": { "title": "Incomplete API", "version": "0.1" }
        }"#;

        let err = parse_openapi_json(raw).unwrap_err();
        match err {
            CoreError::MissingField(f) => assert_eq!(f, "paths"),
            _ => panic!("Expected MissingField error"),
        }
    }
}
