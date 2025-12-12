//! Value objects for the domain layer.
//!
//! Value objects are immutable, validated data types that represent
//! concepts in the domain model. They provide type safety and encapsulate
//! validation logic.

mod page_number;
mod block_address;
mod page_config;

pub use page_number::PageNumber;
pub use block_address::BlockAddress;
pub use page_config::{PageConfig, PageConfigError, BLOCK_SIZE_512, BLOCK_SIZE_4096, BLOCK_SIZE_128K, BLOCK_SIZE_256K};
