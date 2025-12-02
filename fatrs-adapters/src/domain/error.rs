//! Domain-level errors.
//!
//! These errors represent business rule violations and domain-level failures,
//! not infrastructure failures (which come through the port error types).

use crate::domain::value_objects::{PageNumber, PageConfigError};
use core::fmt;

/// Errors that can occur in the domain layer.
#[derive(Debug)]
#[non_exhaustive]
pub enum DomainError<E> {
    /// Attempted to load a different page while current page has uncommitted changes.
    ///
    /// This enforces the business rule that dirty pages must be flushed before
    /// loading a different page.
    DirtyPageConflict {
        /// The currently loaded page number.
        current: PageNumber,
        /// The page number that was requested.
        requested: PageNumber,
    },

    /// No page is currently loaded in the buffer.
    NoPageLoaded,

    /// Invalid page configuration.
    InvalidConfig(PageConfigError),

    /// Storage error from the underlying BlockStorage implementation.
    ///
    /// This wraps errors that come from the infrastructure layer through
    /// the BlockStorage port.
    Storage(E),
}

impl<E: fmt::Display> fmt::Display for DomainError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirtyPageConflict { current, requested } => write!(
                f,
                "Cannot load {} while {} is dirty (uncommitted changes)",
                requested, current
            ),
            Self::NoPageLoaded => write!(f, "No page is currently loaded in the buffer"),
            Self::InvalidConfig(e) => write!(f, "Invalid page configuration: {}", e),
            Self::Storage(e) => write!(f, "Storage error: {}", e),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> core::error::Error for DomainError<E> {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::InvalidConfig(e) => Some(e),
            _ => None,
        }
    }
}

// Note: We don't implement From<E> for DomainError<E> automatically
// to avoid conflicts. Use DomainError::Storage(e) or the ? operator
// with map_err as needed.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dirty_page_conflict_display() {
        let error: DomainError<std::io::Error> = DomainError::DirtyPageConflict {
            current: PageNumber::new(5),
            requested: PageNumber::new(10),
        };

        let msg = format!("{}", error);
        assert!(msg.contains("Page(5)"));
        assert!(msg.contains("Page(10)"));
        assert!(msg.contains("dirty"));
    }

    #[test]
    fn test_no_page_loaded_display() {
        let error: DomainError<std::io::Error> = DomainError::NoPageLoaded;
        let msg = format!("{}", error);
        assert!(msg.contains("No page"));
    }

    #[test]
    fn test_storage_error_conversion() {
        let io_error = std::io::Error::new(std::io::ErrorKind::Other, "test error");
        let domain_error: DomainError<std::io::Error> = DomainError::Storage(io_error);

        match domain_error {
            DomainError::Storage(_) => {}, // Expected
            _ => panic!("Expected Storage variant"),
        }
    }
}
