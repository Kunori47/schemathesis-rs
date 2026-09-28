# Software Specification Document (SDD)

## 1. Objective

Provide a high-throughput, memory-efficient, property-based testing engine in Rust that validates HTTP APIs against OpenAPI (v3.0.x and v3.1.x) specifications.

---

## 2. Core Domain Architecture

The domain follows Hexagonal Architecture principles, decoupling schema parsing and data generation from I/O transport.

```
+-------------------------------------------------------------+
|                      schemathesis-cli                       |
+-------------------------------------------------------------+
                              |
+-------------------------------------------------------------+
|                     schemathesis-runner                     |
|  - Concurrent test scheduler (Tokio)                        |
|  - Check assertions (5xx, schema conformance, content-type) |
+-------------------------------------------------------------+
          |                                       |
+--------------------------+       +--------------------------+
|  schemathesis-generator  |       |    schemathesis-core     |
|  - Property strategies   |       |  - OpenAPI AST           |
|  - Shrinking engine      |       |  - Validation schema     |
+--------------------------+       |  - Domain entities       |
                                   +--------------------------+
```

---

## 3. Domain Model Specifications

### 3.1. `schemathesis-core`

#### Entities

- `ApiSpecification`:
  - `title: String`
  - `version: String`
  - `servers: Vec<ServerUrl>`
  - `operations: Vec<Operation>`

- `Operation`:
  - `id: Option<String>`
  - `method: HttpMethod` (`GET`, `POST`, `PUT`, `DELETE`, `PATCH`, `HEAD`, `OPTIONS`)
  - `path_template: String` (e.g. `/users/{id}`)
  - `parameters: Vec<Parameter>`
  - `request_body: Option<RequestBodyDefinition>`
  - `responses: HashMap<StatusCodeRange, ResponseDefinition>`

- `Parameter`:
  - `name: String`
  - `location: ParameterLocation` (`Query`, `Header`, `Path`, `Cookie`)
  - `required: bool`
  - `schema: SchemaDefinition`

- `Check`:
  - Trait defining assertion contracts:
    ```rust
    pub trait Check: Send + Sync {
        fn name(&self) -> &'static str;
        fn evaluate(&self, case: &GeneratedCase, response: &ResponsePayload) -> CheckResult;
    }
    ```

---

## 4. Built-in Checks Specification

1. **`not_a_server_error`**:
   - Asserts response status code `< 500`.
   - Fails if `status_code >= 500`.

2. **`status_code_conformance`**:
   - Asserts response status code is explicitly defined in the OpenAPI operation response map (or covered by a `default` or `XX` wildcard).

3. **`content_type_conformance`**:
   - Asserts the `Content-Type` header matches defined media types in the schema for that status code.

4. **`response_schema_conformance`**:
   - Validates response payload against the JSON Schema defined for the returned status code and content type.

---

## 5. Generator & Shrinking Specification

- **Input Generation**: For every operation, construct a composite strategy combining path params, query params, headers, and request body.
- **Shrinking Guarantee**: When a check fails on a generated input, the engine must iteratively shrink parameter lengths, integer values, and object fields to find the minimal reproducible failure payload.

---

## 6. Exit Codes

- `0`: All tests passed without check violations.
- `1`: One or more checks failed (contract or server errors discovered).
- `2`: Configuration or schema parsing error.
