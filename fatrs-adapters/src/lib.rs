//! Block device adapters with hexagonal architecture.
//!
//! This crate provides page buffering and streaming adapters for block devices,
//! structured using hexagonal architecture (ports and adapters pattern).
//!
//! # Architecture
//!
//! The crate is organized into three layers:
//!
//! ## Domain Layer (`domain`)
//! Pure business logic with no infrastructure dependencies:
//! - **Entities**: `Page` with state machine
//! - **Value Objects**: `PageNumber`, `BlockAddress`, `PageConfig`
//! - **Services**: `PageBuffer` with business rules
//! - **Ports**: `BlockStorage` interface
//!
//! ## Adapter Layer (`adapters`)
//! Concrete implementations connecting domain to infrastructure:
//! - **`BlockDeviceAdapter`**: Implements `BlockStorage` using `BlockDevice`
//! - **`StackBuffer`**: Compile-time sized buffer (no_std compatible)
//! - **`HeapBuffer`**: Runtime sized buffer (requires `alloc`)
//!
//! ## Infrastructure Layer (`infrastructure`)
//! High-level utilities built on the domain:
//! - Streaming adapters (future)
//! - Shared resource management
//!
//! # Quick Start
//!
//! ## Stack-Allocated Buffer (no_std)
//!
//! ```ignore
//! use fatrs_adapters::adapters::StackBuffer4K;
//!
//! let device = MyBlockDevice::new();
//! let mut buffer = StackBuffer4K::new(device);
//!
//! // Load page, modify, and flush
//! buffer.load(0).await?;
//! buffer.modify(|data| data[0] = 42)?;
//! buffer.flush().await?;
//! ```
//!
//! ## Heap-Allocated Buffer (requires `alloc`)
//!
//! ```ignore
//! use fatrs_adapters::adapters::{HeapBuffer, presets};
//!
//! let device = MyBlockDevice::new();
//! let mut buffer = HeapBuffer::new(device, presets::PAGE_128K)?;
//!
//! buffer.load(0).await?;
//! buffer.modify(|data| data[0..4].copy_from_slice(b"test"))?;
//! buffer.flush().await?;
//! ```
//!
//! # Hexagonal Architecture Benefits
//!
//! 1. **Testability**: Domain logic can be tested with mock storage
//! 2. **Flexibility**: Easy to swap storage implementations
//! 3. **Separation**: Business rules isolated from infrastructure
//! 4. **No Leaky Abstractions**: Infrastructure details never leak into domain
//!
//! # Features
//!
//! - `alloc`: Enable heap-allocated adapters (`HeapBuffer`)
//! - `std`: Enable standard library features
//! - `log`: Enable logging support
//! - `defmt`: Enable defmt logging for embedded
//! - `runtime-tokio`: Use tokio synchronization primitives
//! - `runtime-generic`: Use async-lock (portable async)

#![cfg_attr(not(test), no_std)]
#![warn(missing_docs)]
#![allow(async_fn_in_trait)]

// Core layers
pub mod domain;
pub mod adapters;
pub mod infrastructure;

// Re-export commonly used types for convenience
pub use domain::{
    BlockAddress, BlockStorage, DomainError, Page, PageConfig, PageConfigError, PageNumber,
    PageState, BLOCK_SIZE_512, BLOCK_SIZE_4096, BLOCK_SIZE_128K, BLOCK_SIZE_256K,
};

pub use adapters::{
    AdapterError, BlockDeviceAdapter, StackBuffer, StackBuffer2K, StackBuffer4K, StackBuffer8K,
    StackBuffer4KBlock4K, StackBuffer128KBlock128K,
};

#[cfg(feature = "alloc")]
pub use adapters::{presets, HeapBuffer};

#[cfg(feature = "embedded-storage")]
pub use adapters::{NorFlashAdapter, NorFlashConfig, NorFlashError, NOR_FLASH_BLOCK_SIZE};

#[cfg(feature = "embedded-storage")]
pub use adapters::{HeaderRotatingDevice, HeaderRotationConfig, HEADER_ROTATION_BLOCK_SIZE};

// Infrastructure layer exports
pub use infrastructure::streaming::{StackPageStream, StreamError};

#[cfg(feature = "alloc")]
pub use infrastructure::streaming::HeapPageStream;

// Re-export embedded_io_async for convenience
pub use embedded_io_async;

// Re-export fatrs_block_device types for convenience
// This allows users to depend only on fatrs-adapters without needing fatrs-block-device directly
pub use fatrs_block_device::{
    BlockDevice,
    SendBlockDevice,
    blocks_to_slice,
    blocks_to_slice_mut,
    slice_to_blocks,
    slice_to_blocks_mut,
};
