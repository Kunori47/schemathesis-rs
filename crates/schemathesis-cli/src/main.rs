use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use schemathesis_core::check::Check;
use schemathesis_core::parser::{parse_openapi_json, parse_openapi_yaml};
use schemathesis_generator::CaseGenerator;
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
            max_examples: _,
        } => {
            println!("🚀 Starting schemathesis-rs...");
            println!("📄 Reading schema from: {}", schema);
            println!("🎯 Base URL: {}", base_url);

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
                "✅ Schema loaded successfully: {} (v{}) with {} operations.",
                spec.title,
                spec.version,
                spec.operations.len()
            );

            let generator = CaseGenerator::new();
            let runner = HttpRunner::new(base_url);

            let mut total_passed = 0;
            let mut total_failed = 0;

            for op in &spec.operations {
                let case = generator.generate_case(op);
                let allowed_codes: Vec<String> = op.responses.keys().cloned().collect();

                let checks: Vec<Box<dyn Check>> = vec![
                    Box::new(NotAServerErrorCheck),
                    Box::new(StatusCodeConformanceCheck::new(allowed_codes)),
                ];

                print!("  Testing [{} {}] ... ", case.method.as_str(), case.path);

                match runner.execute_case(&case, &checks).await {
                    Ok(result) => {
                        if result.passed {
                            println!("PASSED");
                            total_passed += 1;
                        } else {
                            println!("FAILED");
                            for check_res in result.check_results {
                                if let schemathesis_core::check::CheckStatus::Failure(msg) =
                                    check_res.status
                                {
                                    println!("    ❌ [{}]: {}", check_res.check_name, msg);
                                }
                            }
                            total_failed += 1;
                        }
                    }
                    Err(e) => {
                        println!("ERROR (Network/Connection failed: {})", e);
                        total_failed += 1;
                    }
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
