//! Domain layer - Pure business logic with zero infrastructure dependencies.
//!
//! This is the core of the hexagonal architecture. The domain layer contains:
//! - **Entities**: Objects with identity (e.g., `Page`)
//! - **Value Objects**: Immutable validated data (e.g., `PageNumber`, `BlockAddress`)
//! - **Domain Services**: Business logic (e.g., `PageBuffer`)
//! - **Ports**: Interfaces to the outside world (e.g., `BlockStorage`)
//! - **Domain Errors**: Business rule violations
//!
//! # Hexagonal Architecture
//!
//! ```text
//!     ┌──────────────────────────────────┐
//!     │      Domain Layer (Core)         │
//!     │                                  │
//!     │  ┌────────────────────────────┐  │
//!     │  │  Entities & Value Objects  │  │
//!     │  │  - Page, PageNumber, etc.  │  │
//!     │  └────────────────────────────┘  │
//!     │              ▲                   │
//!     │              │                   │
//!     │  ┌────────────────────────────┐  │
//!     │  │    Domain Services         │  │
//!     │  │    - PageBuffer            │  │
//!     │  └────────────────────────────┘  │
//!     │              │                   │
//!     │              ▼                   │
//!     │  ┌────────────────────────────┐  │
//!     │  │    Ports (Interfaces)      │  │
//!     │  │    - BlockStorage          │  │
//!     │  └────────────────────────────┘  │
//!     └──────────────────────────────────┘
//!                    ▲
//!                    │ implemented by
//!                    │
//!     ┌──────────────────────────────────┐
//!     │      Adapter Layer               │
//!     │  - BlockDeviceAdapter            │
//!     │  - StackBuffer                   │
//!     │  - HeapBuffer                    │
//!     └──────────────────────────────────┘
//! ```
//!
//! # Design Principles
//!
//! 1. **Dependency Inversion**: Domain depends only on abstractions (ports)
//! 2. **No Infrastructure**: No framework types, no I/O, no database code
//! 3. **Pure Functions**: Business logic is testable without mocks
//! 4. **Explicit Rules**: Business rules are visible in the code
//!
//! # Examples
//!
//! ```ignore
//! use fatrs_adapters::domain::{PageBuffer, PageNumber, PageConfig};
//!
//! // Domain service with injected storage
//! let storage = MyStorage::new();  // Implements BlockStorage port
//! let config = PageConfig::new(4096, 8);
//! let mut buffer = PageBuffer::new(storage, config);
//!
//! // Business logic
//! buffer.load(PageNumber::new(0)).await?;
//! buffer.modify(|data| data[0] = 42)?;
//! buffer.flush().await?;
//! ```

pub mod entities;
pub mod value_objects;
pub mod ports;
pub mod error;

mod page_buffer;

// Re-export commonly used types
pub use entities::{Page, PageState};
pub use value_objects::{PageNumber, BlockAddress, PageConfig, PageConfigError, BLOCK_SIZE_512, BLOCK_SIZE_4096, BLOCK_SIZE_128K, BLOCK_SIZE_256K};
pub use ports::BlockStorage;
pub use error::DomainError;
pub use page_buffer::PageBuffer;
