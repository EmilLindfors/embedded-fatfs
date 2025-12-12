//! Infrastructure layer - high-level I/O utilities built on the domain.
//!
//! This module provides streaming wrappers around the page buffer adapters,
//! adding async Read/Write/Seek capabilities for integration with higher-level
//! file systems and I/O frameworks.

pub mod streaming;
