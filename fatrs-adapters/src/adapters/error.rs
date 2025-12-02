//! Adapter-level errors.

use crate::domain::DomainError;

/// Adapter-level errors for stack-allocated adapters (no_std).
#[derive(Debug)]
pub enum AdapterError<E> {
    /// Domain-level error.
    Domain(&'static str),
    /// Storage error.
    Storage(E),
}

impl<E: core::fmt::Display> AdapterError<E> {
    pub(crate) fn from_domain(err: DomainError<E>) -> Self {
        match err {
            DomainError::Storage(e) => AdapterError::Storage(e),
            DomainError::DirtyPageConflict { .. } => AdapterError::Domain("Dirty page conflict"),
            DomainError::NoPageLoaded => AdapterError::Domain("No page loaded"),
            DomainError::InvalidConfig(_) => AdapterError::Domain("Invalid config"),
        }
    }
}

impl<E: core::fmt::Display> core::fmt::Display for AdapterError<E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Domain(msg) => write!(f, "Domain error: {}", msg),
            Self::Storage(e) => write!(f, "Storage error: {}", e),
        }
    }
}

impl<E: core::fmt::Debug + core::fmt::Display> core::error::Error for AdapterError<E> {}

/// Adapter-level errors for heap-allocated adapters (with alloc).
#[cfg(feature = "alloc")]
#[derive(Debug)]
pub enum HeapAdapterError<E> {
    /// Domain-level error.
    Domain(alloc::string::String),
    /// Storage error.
    Storage(E),
}

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
impl<E: core::fmt::Display> HeapAdapterError<E> {
    pub(crate) fn from_domain(err: DomainError<E>) -> Self {
        use alloc::format;
        match err {
            DomainError::Storage(e) => HeapAdapterError::Storage(e),
            other => HeapAdapterError::Domain(format!("{}", other)),
        }
    }
}

#[cfg(feature = "alloc")]
impl<E: core::fmt::Display> core::fmt::Display for HeapAdapterError<E> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Domain(msg) => write!(f, "Domain error: {}", msg),
            Self::Storage(e) => write!(f, "Storage error: {}", e),
        }
    }
}

#[cfg(feature = "alloc")]
impl<E: core::fmt::Debug + core::fmt::Display> core::error::Error for HeapAdapterError<E> {}
