use schemathesis_core::check::Check;
use schemathesis_core::model::{GeneratedCase, HttpMethod, ResponsePayload};
use schemathesis_core::parser::parse_openapi_json;
use schemathesis_generator::{CaseGenerator, Shrinker};
use schemathesis_runner::checks::{NotAServerErrorCheck, StatusCodeConformanceCheck};
use std::collections::HashMap;

#[test]
fn test_end_to_end_generator_and_checks() {
    let schema_json = r#"{
        "openapi": "3.0.0",
        "info": { "title": "Petstore API", "version": "1.0.0" },
        "paths": {
            "/pets": {
                "get": {
                    "operationId": "listPets",
                    "parameters": [
                        {
                            "name": "limit",
                            "in": "query",
                            "required": false,
                            "schema": { "type": "integer", "minimum": 1, "maximum": 100 }
                        }
                    ],
                    "responses": {
                        "200": { "description": "A paged array of pets" }
                    }
                }
            }
        }
    }"#;

    let spec = parse_openapi_json(schema_json).expect("valid schema");
    assert_eq!(spec.operations.len(), 1);

    let generator = CaseGenerator::new();
    let cases = generator.generate_cases(&spec.operations[0], 5);
    assert_eq!(cases.len(), 5);

    let not_500 = NotAServerErrorCheck;
    let conformance = StatusCodeConformanceCheck::new(vec!["200".to_string()]);

    for case in &cases {
        assert_eq!(case.path, "/pets");
        assert_eq!(case.method, HttpMethod::Get);

        // Simulate 200 response
        let ok_resp = ResponsePayload {
            status_code: 200,
            headers: HashMap::new(),
            body: Some(serde_json::json!([{"id": 1, "name": "Fido"}])),
        };
        assert!(not_500.evaluate(case, &ok_resp).is_success());
        assert!(conformance.evaluate(case, &ok_resp).is_success());

        // Simulate 500 server error response
        let err_resp = ResponsePayload {
            status_code: 500,
            headers: HashMap::new(),
            body: None,
        };
        assert!(!not_500.evaluate(case, &err_resp).is_success());
    }
}

#[test]
fn test_shrinker_reduces_failing_case_payload() {
    let shrinker = Shrinker::new();
    let original_case = GeneratedCase {
        method: HttpMethod::Post,
        path: "/users".into(),
        headers: HashMap::new(),
        query_params: vec![
            ("filter".into(), "very_long_filter_string_that_triggered_error".into()),
            ("sort".into(), "asc".into()),
        ],
        body: Some(serde_json::json!({
            "name": "Alice in Wonderland",
            "metadata": { "nested": true, "extra": "data" }
        })),
    };

    let candidates = shrinker.candidates(&original_case);
    assert!(!candidates.is_empty());

    // Candidates should include shortened query params and dropped keys
    let found_shrunk_query = candidates.iter().any(|c| {
        c.query_params.len() < original_case.query_params.len()
            || c.query_params[0].1.len() < original_case.query_params[0].1.len()
    });
    assert!(found_shrunk_query, "Candidates should include shrunk query params");
}

#[tokio::test]
async fn test_concurrent_batch_execution_handles_all_cases() {
    use schemathesis_runner::HttpRunner;
    use std::sync::Arc;

    let runner = HttpRunner::new("http://127.0.0.1:9".into());
    let cases = vec![
        GeneratedCase {
            method: HttpMethod::Get,
            path: "/item/1".into(),
            headers: HashMap::new(),
            query_params: vec![],
            body: None,
        },
        GeneratedCase {
            method: HttpMethod::Get,
            path: "/item/2".into(),
            headers: HashMap::new(),
            query_params: vec![],
            body: None,
        },
        GeneratedCase {
            method: HttpMethod::Get,
            path: "/item/3".into(),
            headers: HashMap::new(),
            query_params: vec![],
            body: None,
        },
    ];

    let checks: Arc<Vec<Box<dyn Check>>> = Arc::new(vec![Box::new(NotAServerErrorCheck)]);
    let results = runner.execute_batch_concurrent(cases, checks, 2).await;

    // Concurrency pool correctly processed all 3 cases
    assert_eq!(results.len(), 3);
}
