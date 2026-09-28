# schemathesis-rs

A blazing-fast, property-based API testing engine in Rust for OpenAPI specifications.

## Why schemathesis-rs?

[Schemathesis](https://github.com/schemathesis/schemathesis) revolutionized contract and property-based testing for APIs. However, on large OpenAPI schemas with hundreds of endpoints, deep `$ref` trees, and complex validation matrices, Python's runtime and GIL introduce significant CPU latency and memory pressure.

`schemathesis-rs` brings property-based testing to native speeds:
- **Zero-cost parsing and validation**: Native deserialization and schema resolution with zero-copy where possible.
- **True concurrency**: Asynchronous I/O powered by Tokio and Hyper/Reqwest, executing tests without GIL bottlenecks.
- **Deterministic fuzzing and shrinking**: Fast seed-based property generation and minimal reproducible failure case shrinking.
- **Single static binary**: No Python environment, virtualenvs, or dependency friction.

---

## Readme-Driven Development (RDD): CLI Interface

Before writing the implementation, the target developer ergonomics are defined below.

### Basic Run

```bash
# Test an API against a local or remote schema
schemathesis-rs run https://api.example.com/openapi.json --base-url https://api.example.com

# Run against a local schema file
schemathesis-rs run ./specs/petstore.yaml --base-url http://localhost:8080
```

### Targeted Execution & Checks

```bash
# Run specific checks
schemathesis-rs run ./specs/petstore.yaml \
  --base-url http://localhost:8080 \
  --checks not_a_server_error,status_code_conformance,response_schema_conformance

# Limit generation and rate
schemathesis-rs run ./specs/petstore.yaml \
  --base-url http://localhost:8080 \
  --max-examples 50 \
  --concurrency 10 \
  --workers 4
```

### Output Formats

```bash
schemathesis-rs run ./specs/petstore.yaml \
  --base-url http://localhost:8080 \
  --report junit \
  --output ./reports/junit.xml
```

---

## Architecture (Screaming / Modular)

```
schemathesis-rust/
├── Cargo.toml
├── crates/
│   ├── schemathesis-core/       # Domain models, OpenAPI parser, schema AST, check interfaces
│   ├── schemathesis-generator/  # Property-based value generation & shrinking strategies
│   ├── schemathesis-runner/     # Async HTTP executor, runner pipeline, and built-in checks
│   └── schemathesis-cli/        # CLI entry point (Clap), progress reporting, output formatters
└── tests/
    └── integration/             # End-to-end integration test suite
```

---

## Development Methodology

1. **RDD (Readme-Driven Development)**: Define user journeys, public interfaces, and CLI commands upfront.
2. **SDD (Specification-Driven Development)**: Formalize domain contracts in `docs/SPECIFICATION.md` before coding.
3. **TDD (Test-Driven Development)**: Write failing domain unit tests and integration tests prior to implementation.