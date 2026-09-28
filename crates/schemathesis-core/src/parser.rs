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

/// Resolves a JSON pointer reference such as "#/components/schemas/Pet" against root document
pub fn resolve_ref<'a>(root: &'a Value, ref_str: &str) -> Option<&'a Value> {
    if !ref_str.starts_with("#/") {
        return None;
    }
    let parts = ref_str.trim_start_matches("#/").split('/');
    let mut current = root;
    for part in parts {
        let unescaped = part.replace("~1", "/").replace("~0", "~");
        current = current.get(unescaped)?;
    }
    Some(current)
}

/// Recursively resolves all $ref occurrences inside a schema value
pub fn dereference_schema(root: &Value, schema: &Value) -> Value {
    if let Some(ref_str) = schema.get("$ref").and_then(Value::as_str) {
        if let Some(resolved) = resolve_ref(root, ref_str) {
            return dereference_schema(root, resolved);
        }
    }

    match schema {
        Value::Object(map) => {
            let mut new_map = serde_json::Map::new();
            for (k, v) in map {
                new_map.insert(k.clone(), dereference_schema(root, v));
            }
            Value::Object(new_map)
        }
        Value::Array(arr) => {
            Value::Array(arr.iter().map(|item| dereference_schema(root, item)).collect())
        }
        other => other.clone(),
    }
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
                        // Resolve parameter ref if present
                        let p_resolved = if let Some(ref_str) = p.get("$ref").and_then(Value::as_str) {
                            resolve_ref(root, ref_str).unwrap_or(p)
                        } else {
                            p
                        };

                        if let (Some(name), Some(loc_str)) = (
                            p_resolved.get("name").and_then(Value::as_str),
                            p_resolved.get("in").and_then(Value::as_str),
                        ) {
                            let location = match loc_str {
                                "query" => ParameterLocation::Query,
                                "header" => ParameterLocation::Header,
                                "path" => ParameterLocation::Path,
                                "cookie" => ParameterLocation::Cookie,
                                _ => continue,
                            };
                            let required = p_resolved.get("required").and_then(Value::as_bool).unwrap_or(false);
                            let schema = p_resolved.get("schema").map(|s| dereference_schema(root, s));

                            parameters.push(Parameter {
                                name: name.to_string(),
                                location,
                                required,
                                schema,
                            });
                        }
                    }
                }

                let request_body = op_val.get("requestBody").map(|rb| {
                    let rb_resolved = if let Some(ref_str) = rb.get("$ref").and_then(Value::as_str) {
                        resolve_ref(root, ref_str).unwrap_or(rb)
                    } else {
                        rb
                    };
                    dereference_schema(root, rb_resolved)
                });

                let mut responses = HashMap::new();
                if let Some(resp_obj) = op_val.get("responses").and_then(Value::as_object) {
                    for (code, resp_val) in resp_obj {
                        let resp_resolved = if let Some(ref_str) = resp_val.get("$ref").and_then(Value::as_str) {
                            resolve_ref(root, ref_str).unwrap_or(resp_val)
                        } else {
                            resp_val
                        };
                        responses.insert(code.clone(), dereference_schema(root, resp_resolved));
                    }
                }

                operations.push(Operation {
                    id: op_id,
                    method,
                    path: path.clone(),
                    parameters,
                    request_body,
                    responses,
                    raw: Some(op_val.clone()),
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
    fn test_parse_openapi_with_refs() {
        let raw = r##"{
            "openapi": "3.0.3",
            "info": { "title": "Ref API", "version": "1.0.0" },
            "paths": {
                "/pets": {
                    "post": {
                        "parameters": [
                            { "$ref": "#/components/parameters/TrackingHeader" }
                        ],
                        "requestBody": {
                            "$ref": "#/components/requestBodies/PetBody"
                        },
                        "responses": {
                            "201": { "$ref": "#/components/responses/CreatedResponse" }
                        }
                    }
                }
            },
            "components": {
                "parameters": {
                    "TrackingHeader": {
                        "name": "X-Track-Id",
                        "in": "header",
                        "required": true,
                        "schema": { "$ref": "#/components/schemas/Uuid" }
                    }
                },
                "requestBodies": {
                    "PetBody": {
                        "content": {
                            "application/json": {
                                "schema": { "$ref": "#/components/schemas/NewPet" }
                            }
                        }
                    }
                },
                "responses": {
                    "CreatedResponse": {
                        "description": "Item created successfully"
                    }
                },
                "schemas": {
                    "Uuid": {
                        "type": "string",
                        "format": "uuid"
                    },
                    "NewPet": {
                        "type": "object",
                        "properties": {
                            "name": { "type": "string" },
                            "tag": { "type": "string" }
                        }
                    }
                }
            }
        }"##;

        let spec = parse_openapi_json(raw).expect("should parse schema with refs");
        assert_eq!(spec.operations.len(), 1);
        let op = &spec.operations[0];
        assert_eq!(op.method, HttpMethod::Post);

        // Parameter resolved from $ref
        assert_eq!(op.parameters.len(), 1);
        assert_eq!(op.parameters[0].name, "X-Track-Id");
        assert_eq!(op.parameters[0].location, ParameterLocation::Header);
        let param_schema = op.parameters[0].schema.as_ref().unwrap();
        assert_eq!(param_schema.get("type").and_then(Value::as_str), Some("string"));
        assert_eq!(param_schema.get("format").and_then(Value::as_str), Some("uuid"));

        // Request body resolved from nested $ref
        let rb = op.request_body.as_ref().unwrap();
        let body_schema = rb.get("content")
            .and_then(|c| c.get("application/json"))
            .and_then(|j| j.get("schema"))
            .unwrap();
        assert_eq!(body_schema.get("type").and_then(Value::as_str), Some("object"));
        let props = body_schema.get("properties").unwrap();
        assert!(props.get("name").is_some());
        assert!(props.get("tag").is_some());

        // Response resolved from $ref
        assert!(op.responses.contains_key("201"));
        let resp = op.responses.get("201").unwrap();
        assert_eq!(resp.get("description").and_then(Value::as_str), Some("Item created successfully"));
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
