//! # Rift L1 Core — Reality Fractal Theory (RFT)
//!
//! Математическое ядро распределённой системы с гарантированной консистентностью.

pub mod core;
pub mod token;
pub mod error;

pub use core::CoreState;
pub use token::RiftTokenState;
pub use error::{InvariantError, Result};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }
}
