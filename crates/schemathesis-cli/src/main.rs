use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use schemathesis_core::check::Check;
use schemathesis_core::parser::{parse_openapi_json, parse_openapi_yaml};
use schemathesis_generator::{CaseGenerator, Shrinker};
use schemathesis_runner::checks::{
    ContentTypeConformanceCheck, NotAServerErrorCheck, ResponseSchemaConformanceCheck,
    StatusCodeConformanceCheck,
};
use schemathesis_runner::executor::HttpRunner;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(name = "schemathesis-rs")]
#[command(about = "Blazing-fast property-based API testing engine in Rust", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run API property tests against a target schema
    Run {
        /// Path or URL to OpenAPI schema (JSON or YAML)
        schema: String,

        /// Target base URL of the service under test
        #[arg(long)]
        base_url: String,

        /// Maximum generated test cases per endpoint
        #[arg(long, default_value_t = 10)]
        max_examples: usize,

        /// Number of concurrent requests
        #[arg(long, default_value_t = 10)]
        concurrency: usize,

        /// Comma-separated list of checks to run (default: all)
        #[arg(long, default_value = "all")]
        checks: String,

        /// Format of the report (e.g. junit)
        #[arg(long)]
        report: Option<String>,

        /// Path to save the report output
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

struct TestReportItem {
    name: String,
    passed: bool,
    failure_message: Option<String>,
}

fn generate_junit_xml(test_items: &[TestReportItem]) -> String {
    let total_tests = test_items.len();
    let total_failures = test_items.iter().filter(|t| !t.passed).count();

    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str(&format!(
        "<testsuites tests=\"{}\" failures=\"{}\" errors=\"0\">\n",
        total_tests, total_failures
    ));
    xml.push_str(&format!(
        "  <testsuite name=\"schemathesis-rs\" tests=\"{}\" failures=\"{}\" errors=\"0\">\n",
        total_tests, total_failures
    ));

    for item in test_items {
        xml.push_str(&format!(
            "    <testcase name=\"{}\" classname=\"schemathesis.operations\">\n",
            html_escape(&item.name)
        ));
        if let Some(msg) = &item.failure_message {
            xml.push_str(&format!(
                "      <failure message=\"Check failed\">{}</failure>\n",
                html_escape(msg)
            ));
        }
        xml.push_str("    </testcase>\n");
    }

    xml.push_str("  </testsuite>\n");
    xml.push_str("</testsuites>\n");
    xml
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            schema,
            base_url,
            max_examples,
            concurrency,
            checks,
            report,
            output,
        } => {
            println!("🚀 Starting schemathesis-rs...");
            println!("📄 Reading schema from: {}", schema);
            println!("🎯 Base URL: {}", base_url);
            println!("🧪 Max examples per endpoint: {}", max_examples);
            println!("⚡ Concurrency level: {}", concurrency);

            let enabled_checks: HashSet<String> = if checks == "all" {
                [
                    "not_a_server_error",
                    "status_code_conformance",
                    "content_type_conformance",
                    "response_schema_conformance",
                ]
                .into_iter()
                .map(String::from)
                .collect()
            } else {
                checks
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect()
            };

            println!("🛡️ Active checks: {:?}", enabled_checks);

            let schema_content = if schema.starts_with("http://") || schema.starts_with("https://") {
                reqwest::get(&schema)
                    .await
                    .context("Failed to fetch schema from URL")?
                    .text()
                    .await
                    .context("Failed to read response body from schema URL")?
            } else {
                fs::read_to_string(PathBuf::from(&schema))
                    .with_context(|| format!("Failed to read schema file at: {}", schema))?
            };

            let spec = if schema.ends_with(".yaml") || schema.ends_with(".yml") {
                parse_openapi_yaml(&schema_content).context("Failed to parse YAML OpenAPI schema")?
            } else {
                parse_openapi_json(&schema_content).context("Failed to parse JSON OpenAPI schema")?
            };

            println!(
                "✅ Schema loaded successfully: {} (v{}) with {} operations.\n",
                spec.title,
                spec.version,
                spec.operations.len()
            );

            let generator = CaseGenerator::new();
            let shrinker = Shrinker::new();
            let runner = HttpRunner::new(base_url);

            let mut total_passed = 0;
            let mut total_failed = 0;
            let mut report_items = Vec::new();

            for op in &spec.operations {
                let cases = generator.generate_cases(op, max_examples);
                let allowed_codes: Vec<String> = op.responses.keys().cloned().collect();

                let mut active_check_list: Vec<Box<dyn Check>> = Vec::new();
                if enabled_checks.contains("not_a_server_error") {
                    active_check_list.push(Box::new(NotAServerErrorCheck));
                }
                if enabled_checks.contains("status_code_conformance") {
                    active_check_list.push(Box::new(StatusCodeConformanceCheck::new(allowed_codes.clone())));
                }
                if enabled_checks.contains("content_type_conformance") {
                    active_check_list.push(Box::new(ContentTypeConformanceCheck::new(op.responses.clone())));
                }
                if enabled_checks.contains("response_schema_conformance") {
                    active_check_list.push(Box::new(ResponseSchemaConformanceCheck::new(op.responses.clone())));
                }

                let checks: Arc<Vec<Box<dyn Check>>> = Arc::new(active_check_list);

                let batch_results = runner
                    .execute_batch_concurrent(cases, Arc::clone(&checks), concurrency)
                    .await;

                let mut op_passed = true;
                let mut op_failure_details = Vec::new();
                let mut first_failing_case = None;

                for res in batch_results {
                    match res {
                        Ok(exec_res) => {
                            if !exec_res.passed {
                                op_passed = false;
                                op_failure_details = exec_res.check_results;
                                first_failing_case = Some(exec_res.case);
                                break;
                            }
                        }
                        Err(e) => {
                            op_passed = false;
                            op_failure_details = vec![schemathesis_core::check::CheckResult::failure(
                                "network_error",
                                e.to_string(),
                            )];
                            break;
                        }
                    }
                }

                let test_name = format!("{} {}", op.method.as_str(), op.path);

                if op_passed {
                    println!(
                        "  [{}] ... PASSED ({} concurrent checks)",
                        test_name, max_examples
                    );
                    total_passed += 1;
                    report_items.push(TestReportItem {
                        name: test_name,
                        passed: true,
                        failure_message: None,
                    });
                } else {
                    println!("  [{}] ... FAILED", test_name);
                    let mut failure_messages = Vec::new();
                    for check_res in &op_failure_details {
                        if let schemathesis_core::check::CheckStatus::Failure(msg) = &check_res.status {
                            println!("    ❌ [{}]: {}", check_res.check_name, msg);
                            failure_messages.push(format!("[{}]: {}", check_res.check_name, msg));
                        }
                    }

                    // Attempt shrinking
                    if let Some(failing_case) = first_failing_case {
                        let candidates = shrinker.candidates(&failing_case);
                        let mut minimal_reproducer = failing_case.clone();

                        for candidate in candidates {
                            if let Ok(res) = runner.execute_case(&candidate, &checks).await {
                                if !res.passed {
                                    minimal_reproducer = candidate;
                                    break;
                                }
                            }
                        }

                        println!("    🔍 Minimal reproducible case:");
                        println!("       Path: {}", minimal_reproducer.path);
                        if !minimal_reproducer.query_params.is_empty() {
                            println!("       Query: {:?}", minimal_reproducer.query_params);
                        }
                        if let Some(body) = minimal_reproducer.body {
                            println!("       Body: {}", serde_json::to_string(&body).unwrap_or_default());
                        }
                    }

                    report_items.push(TestReportItem {
                        name: test_name,
                        passed: false,
                        failure_message: Some(failure_messages.join("\n")),
                    });

                    total_failed += 1;
                }
            }

            println!("\n📊 Summary: {} passed, {} failed", total_passed, total_failed);

            // Handle reporting
            if let Some(rep_format) = report {
                if rep_format.to_lowercase() == "junit" {
                    let xml_content = generate_junit_xml(&report_items);
                    if let Some(out_path) = output {
                        if let Some(parent) = out_path.parent() {
                            fs::create_dir_all(parent)?;
                        }
                        fs::write(&out_path, &xml_content)
                            .with_context(|| format!("Failed to write JUnit report to {:?}", out_path))?;
                        println!("📝 JUnit report generated at: {:?}", out_path);
                    } else {
                        println!("📝 JUnit report:\n{}", xml_content);
                    }
                }
            }

            if total_failed > 0 {
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
