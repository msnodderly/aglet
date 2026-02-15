use std::fmt;

use uuid::Uuid;

#[derive(Debug)]
pub enum AgendaError {
    /// Referenced entity not found.
    NotFound { entity: &'static str, id: Uuid },

    /// Category name already exists (case-insensitive).
    DuplicateName { name: String },

    /// Attempted to modify or delete a reserved category (When, Entry, Done).
    ReservedName { name: String },

    /// Operation not valid in current state (e.g., assigning to deleted item).
    InvalidOperation { message: String },

    /// SQLite or other storage failure.
    StorageError { source: Box<dyn std::error::Error + Send + Sync> },
}

impl fmt::Display for AgendaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgendaError::NotFound { entity, id } => {
                write!(f, "{entity} not found: {id}")
            }
            AgendaError::DuplicateName { name } => {
                write!(f, "category name already exists: {name}")
            }
            AgendaError::ReservedName { name } => {
                write!(f, "cannot modify reserved category: {name}")
            }
            AgendaError::InvalidOperation { message } => {
                write!(f, "invalid operation: {message}")
            }
            AgendaError::StorageError { source } => {
                write!(f, "storage error: {source}")
            }
        }
    }
}

impl std::error::Error for AgendaError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AgendaError::StorageError { source } => Some(source.as_ref()),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for AgendaError {
    fn from(err: rusqlite::Error) -> Self {
        AgendaError::StorageError {
            source: Box::new(err),
        }
    }
}

pub type Result<T> = std::result::Result<T, AgendaError>;
