use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use schemathesis_core::check::Check;
use schemathesis_core::parser::{parse_openapi_json, parse_openapi_yaml};
use schemathesis_generator::{CaseGenerator, Shrinker};
use schemathesis_runner::checks::{NotAServerErrorCheck, StatusCodeConformanceCheck};
use schemathesis_runner::executor::HttpRunner;
use std::fs;
use std::path::PathBuf;

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
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            schema,
            base_url,
            max_examples,
        } => {
            println!("🚀 Starting schemathesis-rs...");
            println!("📄 Reading schema from: {}", schema);
            println!("🎯 Base URL: {}", base_url);
            println!("🧪 Max examples per endpoint: {}", max_examples);

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

            for op in &spec.operations {
                let cases = generator.generate_cases(op, max_examples);
                let allowed_codes: Vec<String> = op.responses.keys().cloned().collect();

                let mut op_passed = true;
                let mut op_failure_details = Vec::new();
                let mut first_failing_case = None;

                for case in cases {
                    let checks: Vec<Box<dyn Check>> = vec![
                        Box::new(NotAServerErrorCheck),
                        Box::new(StatusCodeConformanceCheck::new(allowed_codes.clone())),
                    ];

                    match runner.execute_case(&case, &checks).await {
                        Ok(result) => {
                            if !result.passed {
                                op_passed = false;
                                op_failure_details = result.check_results;
                                first_failing_case = Some(case);
                                break;
                            }
                        }
                        Err(e) => {
                            op_passed = false;
                            op_failure_details = vec![schemathesis_core::check::CheckResult::failure(
                                "network_error",
                                e.to_string(),
                            )];
                            first_failing_case = Some(case);
                            break;
                        }
                    }
                }

                if op_passed {
                    println!("  [{} {}] ... PASSED ({} checks)", op.method.as_str(), op.path, max_examples);
                    total_passed += 1;
                } else {
                    println!("  [{} {}] ... FAILED", op.method.as_str(), op.path);
                    for check_res in &op_failure_details {
                        if let schemathesis_core::check::CheckStatus::Failure(msg) = &check_res.status {
                            println!("    ❌ [{}]: {}", check_res.check_name, msg);
                        }
                    }

                    // Attempt shrinking
                    if let Some(failing_case) = first_failing_case {
                        let checks: Vec<Box<dyn Check>> = vec![
                            Box::new(NotAServerErrorCheck),
                            Box::new(StatusCodeConformanceCheck::new(allowed_codes.clone())),
                        ];

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

                    total_failed += 1;
                }
            }

            println!("\n📊 Summary: {} passed, {} failed", total_passed, total_failed);

            if total_failed > 0 {
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
