use schemathesis_core::check::{Check, CheckResult};
use schemathesis_core::model::{GeneratedCase, ResponsePayload};
use std::collections::HashMap;

#[derive(Debug)]
pub struct ExecutionResult {
    pub case: GeneratedCase,
    pub response: Option<ResponsePayload>,
    pub check_results: Vec<CheckResult>,
    pub passed: bool,
}

pub struct HttpRunner {
    base_url: String,
    client: reqwest::Client,
}

impl HttpRunner {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
        }
    }

    pub async fn execute_case(
        &self,
        case: &GeneratedCase,
        checks: &[Box<dyn Check>],
    ) -> Result<ExecutionResult, reqwest::Error> {
        let full_url = format!("{}{}", self.base_url.trim_end_matches('/'), case.path);

        let mut req_builder = match case.method {
            schemathesis_core::model::HttpMethod::Get => self.client.get(&full_url),
            schemathesis_core::model::HttpMethod::Post => self.client.post(&full_url),
            schemathesis_core::model::HttpMethod::Put => self.client.put(&full_url),
            schemathesis_core::model::HttpMethod::Delete => self.client.delete(&full_url),
            schemathesis_core::model::HttpMethod::Patch => self.client.patch(&full_url),
            schemathesis_core::model::HttpMethod::Head => self.client.head(&full_url),
            schemathesis_core::model::HttpMethod::Options => {
                self.client.request(reqwest::Method::OPTIONS, &full_url)
            }
        };

        for (k, v) in &case.headers {
            req_builder = req_builder.header(k, v);
        }

        if !case.query_params.is_empty() {
            req_builder = req_builder.query(&case.query_params);
        }

        if let Some(body) = &case.body {
            req_builder = req_builder.json(body);
        }

        let resp = req_builder.send().await?;
        let status = resp.status().as_u16();

        let mut headers = HashMap::new();
        for (name, val) in resp.headers() {
            if let Ok(str_val) = val.to_str() {
                headers.insert(name.as_str().to_string(), str_val.to_string());
            }
        }

        let body: Option<serde_json::Value> = resp.json().await.ok();

        let response_payload = ResponsePayload {
            status_code: status,
            headers,
            body,
        };

        let mut check_results = Vec::new();
        let mut passed = true;

        for check in checks {
            let res = check.evaluate(case, &response_payload);
            if !res.is_success() {
                passed = false;
            }
            check_results.push(res);
        }

        Ok(ExecutionResult {
            case: case.clone(),
            response: Some(response_payload),
            check_results,
            passed,
        })
    }
}
