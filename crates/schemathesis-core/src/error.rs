use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Failed to parse OpenAPI schema: {0}")]
    SchemaParseError(String),

    #[error("Invalid specification format: {0}")]
    InvalidSpecification(String),

    #[error("Missing required field: {0}")]
    MissingField(String),
}
