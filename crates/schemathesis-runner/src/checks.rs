use schemathesis_core::check::{Check, CheckResult};
use schemathesis_core::model::{GeneratedCase, ResponsePayload};
use std::collections::HashSet;

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

#[cfg(test)]
mod tests {
    use super::*;
    use schemathesis_core::model::HttpMethod;
    use std::collections::HashMap;

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
}
