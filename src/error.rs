//! Типы ошибок и обработка исключений

use std::fmt;

#[derive(Debug, Clone)]
pub struct InvariantError(pub String);

impl fmt::Display for InvariantError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invariant Error: {}", self.0)
    }
}

impl std::error::Error for InvariantError {}

impl InvariantError {
    pub fn new<S: Into<String>>(msg: S) -> Self {
        InvariantError(msg.into())
    }

    pub fn overflow(msg: &str) -> Self {
        InvariantError(format!("Overflow: {}", msg))
    }

    pub fn underflow(msg: &str) -> Self {
        InvariantError(format!("Underflow: {}", msg))
    }

    pub fn violation(msg: &str) -> Self {
        InvariantError(format!("Invariant violation: {}", msg))
    }
}

pub type Result<T> = std::result::Result<T, InvariantError>;
