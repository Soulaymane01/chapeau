use thiserror::Error;

#[derive(Error, Debug)]
pub enum ChapeauError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("resource not found: {0}")]
    ResourceNotFound(String),

    #[error("domain not found: {0}")]
    DomainNotFound(String),

    #[error("relationship already exists")]
    RelationshipAlreadyExists,

    #[error("invalid resource type: {0}")]
    InvalidResourceType(String),

    #[error("invalid relationship type: {0}")]
    InvalidRelationshipType(String),

    #[error("validation error: {0}")]
    Validation(String),

    #[error("backend error: {0}")]
    Backend(String),

    #[error("discovery error: {0}")]
    Discovery(String),
}

pub type Result<T> = std::result::Result<T, ChapeauError>;
