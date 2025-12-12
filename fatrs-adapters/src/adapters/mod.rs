//! Adapter layer - Concrete implementations connecting domain to infrastructure.
//!
//! This layer contains adapters that implement the domain's ports, connecting
//! the pure domain logic to actual infrastructure (block devices, file systems, etc.).
//!
//! # Hexagonal Architecture
//!
//! ```text
//!     ┌──────────────────────────────────┐
//!     │      Domain Layer                │
//!     │  - PageBuffer (service)          │
//!     │  - BlockStorage (port)           │
//!     └────────────┬─────────────────────┘
//!                  │
//!                  │ implements
//!                  ▼
//!     ┌──────────────────────────────────┐
//!     │      Adapter Layer               │  ◄── This module
//!     │  - BlockDeviceAdapter            │
//!     │  - StackBuffer                   │
//!     │  - HeapBuffer                    │
//!     └────────────┬─────────────────────┘
//!                  │
//!                  │ uses
//!                  ▼
//!     ┌──────────────────────────────────┐
//!     │  Infrastructure (BlockDevice)    │
//!     └──────────────────────────────────┘
//! ```
//!
//! # Available Adapters
//!
//! - **`BlockDeviceAdapter`**: Adapts `BlockDevice` to `BlockStorage` port
//! - **`StackBuffer`**: Stack-allocated buffer with compile-time sizing
//! - **`HeapBuffer`**: Heap-allocated buffer with runtime sizing (requires `alloc`)

mod block_device_adapter;
mod stack_buffer;
mod error;

#[cfg(feature = "alloc")]
mod heap_buffer;

pub use block_device_adapter::BlockDeviceAdapter;
pub use stack_buffer::{StackBuffer, StackBuffer2K, StackBuffer4K, StackBuffer8K, StackBuffer4KBlock4K, StackBuffer128KBlock128K};
pub use error::AdapterError;

#[cfg(feature = "alloc")]
pub use heap_buffer::{HeapBuffer, presets};

#[cfg(feature = "alloc")]
pub use error::HeapAdapterError;
