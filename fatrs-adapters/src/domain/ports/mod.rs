//! Ports define the interfaces between the domain and the outside world.
//!
//! In hexagonal architecture, ports are the boundaries of the application:
//! - **Primary (Driving) Ports**: What the domain exposes to the outside world
//! - **Secondary (Driven) Ports**: What the domain needs from the outside world
//!
//! This module contains the **secondary (driven) ports** that the domain
//! depends on for infrastructure concerns like storage.

mod block_storage;

pub use block_storage::BlockStorage;
