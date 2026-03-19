use std::fmt;

use agenda_core::error::AgendaError;

/// Unified error type for the TUI crate.
///
/// Replaces the previous `Result<T, String>` pattern, preserving error
/// context and enabling `?` without `.map_err(|e| e.to_string())`.
#[derive(Debug)]
pub enum TuiError {
    /// An error from the agenda-core layer (store or business logic).
    Agenda(AgendaError),

    /// A domain-specific error produced within the TUI itself.
    App(String),
}

impl fmt::Display for TuiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TuiError::Agenda(err) => write!(f, "{err}"),
            TuiError::App(msg) => write!(f, "{msg}"),
        }
    }
}

impl From<AgendaError> for TuiError {
    fn from(err: AgendaError) -> Self {
        TuiError::Agenda(err)
    }
}

impl From<String> for TuiError {
    fn from(msg: String) -> Self {
        TuiError::App(msg)
    }
}

impl From<&str> for TuiError {
    fn from(msg: &str) -> Self {
        TuiError::App(msg.to_string())
    }
}

impl From<std::io::Error> for TuiError {
    fn from(err: std::io::Error) -> Self {
        TuiError::App(err.to_string())
    }
}

pub type TuiResult<T> = std::result::Result<T, TuiError>;
