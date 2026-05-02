use std::fmt;

use uuid::Uuid;

#[derive(Debug)]
pub enum AgletError {
    /// Referenced entity not found.
    NotFound { entity: &'static str, id: Uuid },

    /// Category name already exists (case-insensitive).
    DuplicateName { name: String },

    /// Attempted to modify or delete a reserved category (When, Entry, Done).
    ReservedName { name: String },

    /// Prefix matches multiple items.
    AmbiguousId {
        prefix: String,
        matches: Vec<String>,
    },

    /// Operation not valid in current state (e.g., assigning to deleted item).
    InvalidOperation { message: String },

    /// SQLite or other storage failure.
    StorageError {
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

impl fmt::Display for AgletError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgletError::NotFound { entity, id } => {
                write!(f, "{entity} not found: {id}")
            }
            AgletError::DuplicateName { name } => {
                write!(f, "category name already exists: {name}")
            }
            AgletError::ReservedName { name } => {
                write!(f, "cannot modify reserved category: {name}")
            }
            AgletError::AmbiguousId { prefix, matches } => {
                write!(
                    f,
                    "ambiguous id prefix '{prefix}', matches: {}",
                    matches.join(", ")
                )
            }
            AgletError::InvalidOperation { message } => {
                write!(f, "invalid operation: {message}")
            }
            AgletError::StorageError { source } => {
                write!(f, "storage error: {source}")
            }
        }
    }
}

impl std::error::Error for AgletError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AgletError::StorageError { source } => Some(source.as_ref()),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for AgletError {
    fn from(err: rusqlite::Error) -> Self {
        AgletError::StorageError {
            source: Box::new(err),
        }
    }
}

pub type Result<T> = std::result::Result<T, AgletError>;
