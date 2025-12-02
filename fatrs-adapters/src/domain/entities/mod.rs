//! Domain entities for the page buffer system.
//!
//! Entities are objects that have identity and lifecycle. In this domain,
//! the primary entity is a `Page`, which represents a buffered page of storage.

mod page;
mod page_state;

pub use page::Page;
pub use page_state::PageState;
