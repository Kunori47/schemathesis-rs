use schemathesis_core::check::{Check, CheckResult};
use schemathesis_core::model::{GeneratedCase, ResponsePayload};
use std::collections::{HashMap, HashSet};

pub struct NotAServerErrorCheck;

impl Check for NotAServerErrorCheck {
    fn name(&self) -> &'static str {
        "not_a_server_error"
    }

    fn evaluate(&self, _case: &GeneratedCase, response: &ResponsePayload) -> CheckResult {
        if response.status_code >= 500 {
            CheckResult::failure(
                self.name(),
                format!("Server error detected with status code: {}", response.status_code),
            )
        } else {
            CheckResult::success(self.name())
        }
    }
}

pub struct StatusCodeConformanceCheck {
    pub allowed_status_codes: HashSet<String>,
}

impl StatusCodeConformanceCheck {
    pub fn new(allowed: impl IntoIterator<Item = String>) -> Self {
        Self {
            allowed_status_codes: allowed.into_iter().collect(),
        }
    }
}

impl Check for StatusCodeConformanceCheck {
    fn name(&self) -> &'static str {
        "status_code_conformance"
    }

    fn evaluate(&self, _case: &GeneratedCase, response: &ResponsePayload) -> CheckResult {
        let code_str = response.status_code.to_string();
        let wildcard_2xx = format!("{}xx", code_str.chars().next().unwrap_or(' '));

        if self.allowed_status_codes.contains(&code_str)
            || self.allowed_status_codes.contains(&wildcard_2xx)
            || self.allowed_status_codes.contains("default")
        {
            CheckResult::success(self.name())
        } else {
            CheckResult::failure(
                self.name(),
                format!(
                    "Status code {} is not declared in OpenAPI responses ({:?})",
                    response.status_code, self.allowed_status_codes
                ),
            )
        }
    }
}

pub struct ResponseSchemaConformanceCheck {
    pub responses: HashMap<String, serde_json::Value>,
}

impl ResponseSchemaConformanceCheck {
    pub fn new(responses: HashMap<String, serde_json::Value>) -> Self {
        Self { responses }
    }

    fn validate_json_value(schema: &serde_json::Value, value: &serde_json::Value) -> Result<(), String> {
        let expected_type = match schema.get("type").and_then(serde_json::Value::as_str) {
            Some(t) => t,
            None => return Ok(()),
        };

        match expected_type {
            "integer" => {
                if !value.is_i64() && !value.is_u64() {
                    return Err(format!("Expected integer, got {}", value));
                }
            }
            "number" => {
                if !value.is_number() {
                    return Err(format!("Expected number, got {}", value));
                }
            }
            "boolean" => {
                if !value.is_boolean() {
                    return Err(format!("Expected boolean, got {}", value));
                }
            }
            "string" => {
                if !value.is_string() {
                    return Err(format!("Expected string, got {}", value));
                }
            }
            "array" => {
                let arr = match value.as_array() {
                    Some(a) => a,
                    None => return Err(format!("Expected array, got {}", value)),
                };
                if let Some(items_schema) = schema.get("items") {
                    for (i, item) in arr.iter().enumerate() {
                        Self::validate_json_value(items_schema, item)
                            .map_err(|e| format!("Array item [{}]: {}", i, e))?;
                    }
                }
            }
            "object" => {
                let obj = match value.as_object() {
                    Some(o) => o,
                    None => return Err(format!("Expected object, got {}", value)),
                };

                if let Some(req_arr) = schema.get("required").and_then(serde_json::Value::as_array) {
                    for req in req_arr {
                        if let Some(req_name) = req.as_str() {
                            if !obj.contains_key(req_name) {
                                return Err(format!("Missing required object property: '{}'", req_name));
                            }
                        }
                    }
                }

                if let Some(props) = schema.get("properties").and_then(serde_json::Value::as_object) {
                    for (k, prop_schema) in props {
                        if let Some(prop_val) = obj.get(k) {
                            Self::validate_json_value(prop_schema, prop_val)
                                .map_err(|e| format!("Property '{}': {}", k, e))?;
                        }
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }
}

impl Check for ResponseSchemaConformanceCheck {
    fn name(&self) -> &'static str {
        "response_schema_conformance"
    }

    fn evaluate(&self, _case: &GeneratedCase, response: &ResponsePayload) -> CheckResult {
        let code_str = response.status_code.to_string();
        let wildcard = format!("{}xx", code_str.chars().next().unwrap_or(' '));

        let resp_def = self.responses.get(&code_str)
            .or_else(|| self.responses.get(&wildcard))
            .or_else(|| self.responses.get("default"));

        let resp_def = match resp_def {
            Some(d) => d,
            None => return CheckResult::success(self.name()),
        };

        let schema = resp_def
            .get("content")
            .and_then(|c| c.get("application/json"))
            .and_then(|j| j.get("schema"));

        let schema = match schema {
            Some(s) => s,
            None => return CheckResult::success(self.name()),
        };

        let body = match &response.body {
            Some(b) => b,
            None => {
                return CheckResult::failure(
                    self.name(),
                    format!("Response returned status {} with empty body, but schema is defined", response.status_code),
                )
            }
        };

        match Self::validate_json_value(schema, body) {
            Ok(()) => CheckResult::success(self.name()),
            Err(err_msg) => CheckResult::failure(
                self.name(),
                format!("Response body schema violation: {}", err_msg),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use schemathesis_core::model::HttpMethod;
    use serde_json::json;

    fn dummy_case() -> GeneratedCase {
        GeneratedCase {
            method: HttpMethod::Get,
            path: "/test".into(),
            headers: HashMap::new(),
            query_params: vec![],
            body: None,
        }
    }

    #[test]
    fn test_not_a_server_error_passes_on_200() {
        let check = NotAServerErrorCheck;
        let resp = ResponsePayload {
            status_code: 200,
            headers: HashMap::new(),
            body: None,
        };
        assert!(check.evaluate(&dummy_case(), &resp).is_success());
    }

    #[test]
    fn test_not_a_server_error_fails_on_500() {
        let check = NotAServerErrorCheck;
        let resp = ResponsePayload {
            status_code: 500,
            headers: HashMap::new(),
            body: None,
        };
        assert!(!check.evaluate(&dummy_case(), &resp).is_success());
    }

    #[test]
    fn test_status_code_conformance() {
        let check = StatusCodeConformanceCheck::new(vec!["200".into(), "404".into()]);
        let ok_resp = ResponsePayload {
            status_code: 200,
            headers: HashMap::new(),
            body: None,
        };
        assert!(check.evaluate(&dummy_case(), &ok_resp).is_success());

        let unannounced_resp = ResponsePayload {
            status_code: 400,
            headers: HashMap::new(),
            body: None,
        };
        assert!(!check.evaluate(&dummy_case(), &unannounced_resp).is_success());
    }

    #[test]
    fn test_response_schema_conformance_passes_when_valid() {
        let mut responses = HashMap::new();
        responses.insert(
            "200".into(),
            json!({
                "content": {
                    "application/json": {
                        "schema": {
                            "type": "object",
                            "required": ["id", "title"],
                            "properties": {
                                "id": { "type": "integer" },
                                "title": { "type": "string" }
                            }
                        }
                    }
                }
            }),
        );

        let check = ResponseSchemaConformanceCheck::new(responses);
        let resp = ResponsePayload {
            status_code: 200,
            headers: HashMap::new(),
            body: Some(json!({ "id": 1, "title": "Valid Book" })),
        };
        assert!(check.evaluate(&dummy_case(), &resp).is_success());
    }

    #[test]
    fn test_response_schema_conformance_fails_when_missing_required_field() {
        let mut responses = HashMap::new();
        responses.insert(
            "200".into(),
            json!({
                "content": {
                    "application/json": {
                        "schema": {
                            "type": "object",
                            "required": ["id", "title"],
                            "properties": {
                                "id": { "type": "integer" },
                                "title": { "type": "string" }
                            }
                        }
                    }
                }
            }),
        );

        let check = ResponseSchemaConformanceCheck::new(responses);
        let resp = ResponsePayload {
            status_code: 200,
            headers: HashMap::new(),
            body: Some(json!({ "id": 1 })), // missing "title"
        };
        let result = check.evaluate(&dummy_case(), &resp);
        assert!(!result.is_success());
    }
}
