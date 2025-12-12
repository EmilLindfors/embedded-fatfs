//! Streaming wrappers around page buffers.
//!
//! This module provides streaming I/O on top of the page buffer adapters,
//! implementing async Read/Write/Seek traits for integration with file systems
//! and other I/O frameworks.
//!
//! # Send/Sync Properties
//!
//! These streams automatically adapt to your environment:
//! - **With runtime features** (tokio/async-lock): `Send + Sync` (uses Arc)
//! - **Without runtime features** (embedded): No unnecessary bounds
//!
//! No manual trait bounds needed - the compiler handles everything!

mod stack_page_stream;
mod heap_page_stream;
mod embedded_io_impl;

pub use stack_page_stream::StackPageStream;

#[cfg(feature = "alloc")]
pub use heap_page_stream::HeapPageStream;

use core::fmt;

/// Unified I/O error type for streaming operations.
#[derive(Debug)]
pub enum StreamError<E> {
    /// Error from the underlying storage.
    Storage(E),
    /// I/O operation would exceed storage bounds.
    OutOfBounds,
    /// Invalid seek position.
    InvalidSeek,
}

impl<E: fmt::Display> fmt::Display for StreamError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(e) => write!(f, "Storage error: {}", e),
            Self::OutOfBounds => write!(f, "Operation would exceed storage bounds"),
            Self::InvalidSeek => write!(f, "Invalid seek position"),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> core::error::Error for StreamError<E> {}

// Implement embedded_io_async::Error so our streams can be used with embedded_io_async
impl<E: fmt::Debug + fmt::Display> embedded_io_async::Error for StreamError<E> {
    fn kind(&self) -> embedded_io_async::ErrorKind {
        match self {
            Self::Storage(_) => embedded_io_async::ErrorKind::Other,
            Self::OutOfBounds => embedded_io_async::ErrorKind::InvalidInput,
            Self::InvalidSeek => embedded_io_async::ErrorKind::InvalidInput,
        }
    }
}

/// Seek position for stream operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeekFrom {
    /// Offset from the start of the stream.
    Start(u64),
    /// Offset relative to the current position.
    Current(i64),
    /// Offset from the end of the stream.
    End(i64),
}
