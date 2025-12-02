# fatrs-adapters Architecture - Hexagonal Design

## Overview

This document describes the **hexagonal architecture** (ports and adapters pattern) implementation in the `fatrs-adapters` crate. The refactoring from v0.4.0 to v0.5.0 moved from a trait-based approach to a clean hexagonal design with explicit separation of concerns.

## Design Principles

1. **Dependency Inversion**: Domain depends only on abstractions (ports), never on infrastructure
2. **Pure Domain Logic**: Business rules are isolated and testable without I/O
3. **Explicit Boundaries**: Clear separation between domain, application, and infrastructure
4. **Testability First**: All business logic can be tested with mock implementations

## Hexagonal Architecture Layers

```
┌──────────────────────────────────────────────────────────────┐
│                    Application Entry Points                  │
│                   (fatrs-cli, fatrs crate)                   │
└────────────────────────┬─────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────────────┐
│                      ADAPTER LAYER                           │
│  (Infrastructure Implementations - adapters/)                │
│                                                              │
│  ┌────────────────┐  ┌─────────────────┐  ┌──────────────┐ │
│  │  StackBuffer   │  │  HeapBuffer     │  │  PageStream  │ │
│  │  (no_std)      │  │  (alloc)        │  │              │ │
│  └────────┬───────┘  └────────┬────────┘  └──────┬───────┘ │
│           │                    │                   │         │
│           └───────────┬────────┘                   │         │
│                       │                            │         │
│           ┌───────────▼────────────┐               │         │
│           │  BlockDeviceAdapter    │               │         │
│           │  (BlockDevice→Port)    │               │         │
│           └────────────┬───────────┘               │         │
└────────────────────────┼───────────────────────────┼─────────┘
                         │                           │
                         ▼ implements               ▼ uses
┌──────────────────────────────────────────────────────────────┐
│                      DOMAIN LAYER                            │
│  (Pure Business Logic - domain/)                            │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │                    Ports (Interfaces)                 │  │
│  │  ┌──────────────────────────────────────────────┐   │  │
│  │  │  BlockStorage (Secondary/Driven Port)        │   │  │
│  │  │  - What domain NEEDS from infrastructure    │   │  │
│  │  └──────────────────────────────────────────────┘   │  │
│  └──────────────────────────────────────────────────────┘  │
│                           │                                  │
│                           ▼ used by                          │
│  ┌──────────────────────────────────────────────────────┐  │
│  │         Domain Service (PageBuffer<S>)               │  │
│  │  - Core business rules                               │  │
│  │  - Dirty page conflict enforcement                   │  │
│  │  - Page state management                             │  │
│  └──────────────────────────────────────────────────────┘  │
│                           │                                  │
│                           ▼ uses                             │
│  ┌──────────────────────────────────────────────────────┐  │
│  │            Entities & Value Objects                  │  │
│  │  ┌────────────┐  ┌───────────┐  ┌────────────────┐ │  │
│  │  │  Page      │  │PageNumber │  │  BlockAddress  │ │  │
│  │  │  (Entity)  │  │(Value Obj)│  │  (Value Obj)   │ │  │
│  │  └────────────┘  └───────────┘  └────────────────┘ │  │
│  └──────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

## Layer Descriptions

### 1. Domain Layer (`src/domain/`)

The **core** of the application containing pure business logic with **zero** infrastructure dependencies.

#### Components

**Value Objects** (`value_objects/`):
- `PageNumber`: Type-safe page identifier
- `BlockAddress`: Type-safe block address
- `PageConfig`: Immutable page configuration (size, blocks per page)
- `BLOCK_SIZE`: Standard 512-byte block size

**Entities** (`entities/`):
- `Page<T>`: Core entity representing a buffered page
  - Contains data, page number, and state
  - Implements state machine: `Clean` → `Dirty` → `Flushed`
  - `data_mut()` explicitly marks page as dirty
- `PageState`: Enum for `Clean`/`Dirty` states

**Domain Services**:
- `PageBuffer<S: BlockStorage, T>`: Core business logic
  - Enforces **business rule**: Cannot load different page when current is dirty
  - Manages page lifecycle and state transitions
  - Generic over storage implementation (dependency injection)

**Ports** (`ports/`):
- `BlockStorage`: **Secondary (driven) port**
  - Defines what domain NEEDS from infrastructure
  - Interface for block-level I/O operations
  - Implemented by adapters in infrastructure layer

**Domain Errors** (`error.rs`):
- `DomainError<E>`: Business rule violations
  - `DirtyPageConflict`: Attempted to load page with uncommitted changes
  - `NoPageLoaded`: Operation requires loaded page
  - `InvalidConfig`: Invalid page configuration
  - `Storage(E)`: Infrastructure error wrapper

### 2. Adapter Layer (`src/adapters/`)

Concrete implementations that connect the domain to real infrastructure.

#### Primary Adapters (Drive the Domain)

**StackBuffer<D, const N: usize>**:
- Compile-time sized page buffer
- Perfect for `no_std` environments
- Size determined by const generic `N`
- Type aliases: `StackBuffer4K`, `StackBuffer8K`
- Wraps `PageBuffer<BlockDeviceAdapter<D>, Vec<u8>>`

**HeapBuffer<D>** (requires `alloc`):
- Runtime-sized page buffer
- For large pages (128KB, 1MB+)
- Flexible sizing at runtime
- Presets: `PAGE_4K`, `PAGE_128K`, `PAGE_1M`, etc.

#### Secondary Adapters (Implement Ports)

**BlockDeviceAdapter<D>**:
- Implements `BlockStorage` port
- Adapts `fatrs_block_device::BlockDevice` to domain's `BlockStorage`
- Handles alignment and block<→byte conversions
- Thread-safe with proper bounds: `Send + Sync`

**AdapterError<E>**:
- Unified error type for adapter layer
- Converts between domain errors and infrastructure errors
- Feature-gated for `alloc` (uses `String`) vs `no_std` (uses `&'static str`)

### 3. Infrastructure Layer (Future)

High-level utilities built on top of the domain:
- `PageStream`: Stream-based I/O (`Read`/`Write`/`Seek` traits)
- `BufStream`: Single-block buffering
- `Shared<T>`: Runtime-agnostic resource sharing

## Key Design Decisions

### 1. Ports Define Contract, Not Implementation

**Before** (v0.4 - Trait-based):
```rust
pub trait PageBufferOps<D: BlockDevice> {
    async fn read_page(&mut self, page: u32) -> Result<(), Error>;
    // Implementations had behavioral differences!
}
```

**After** (v0.5 - Hexagonal):
```rust
// Port (domain/ports/block_storage.rs)
pub trait BlockStorage: Send + Sync {
    async fn read_blocks(&mut self, addr: BlockAddress, dest: &mut [u8])
        -> Result<(), Self::Error>;
}

// Domain Service uses the port
pub struct PageBuffer<S: BlockStorage, T> {
    storage: S,  // Depends on abstraction, not concrete type
    // ...
}
```

### 2. Explicit Business Rules in Domain

The domain layer explicitly enforces business rules:

```rust
pub async fn load(&mut self, number: PageNumber) -> Result<(), DomainError<S::Error>> {
    // Business rule: Cannot load different page while current is dirty
    if let Some(ref page) = self.current {
        if page.number() != number && page.is_dirty() {
            return Err(DomainError::DirtyPageConflict {
                current: page.number(),
                requested: number,
            });
        }
    }

    // Load from storage through port
    self.storage.read_blocks(addr, &mut buffer).await?;
    // ...
}
```

### 3. Value Objects Prevent Primitive Obsession

Instead of passing raw `u32` values everywhere:

```rust
// ❌ Before: Easy to mix up
fn read_page(page_num: u32, block_addr: u32) -> Result<()>;

// ✅ After: Type-safe
fn read_page(page: PageNumber, block: BlockAddress) -> Result<()>;
```

### 4. Dependency Direction is Inverted

```
Infrastructure → Adapters → Domain Ports
     ↓              ↓            ↑
  BlockDevice  BlockDeviceAdapter  ↑
                      ↓              ↑
                   Domain Service   ↑
                      ↓              ↑
                   Uses Port ────────┘
```

Domain never knows about infrastructure. Infrastructure implements domain's ports.

### 5. Testability Without Mocks

Domain tests use simple in-memory storage:

```rust
#[tokio::test]
async fn test_dirty_page_conflict() {
    let mock_storage = MockStorage::new();  // Implements BlockStorage
    let mut buffer = PageBuffer::new(mock_storage, config);

    buffer.load(PageNumber::new(0)).await.unwrap();
    buffer.modify(|data| data[0] = 42).unwrap();

    // Business rule enforced!
    let result = buffer.load(PageNumber::new(1)).await;
    assert!(matches!(result, Err(DomainError::DirtyPageConflict { .. })));
}
```

## Benefits of Hexagonal Architecture

### 1. Testability

| Aspect | Before (v0.4) | After (v0.5) |
|--------|---------------|--------------|
| **Business logic tests** | Required real BlockDevice | Mock storage in-memory |
| **Test speed** | Slow (I/O) | Fast (pure logic) |
| **Test reliability** | Flaky (I/O errors) | Deterministic |
| **Coverage** | Hard to test edge cases | Easy to test all paths |

### 2. Flexibility

Easy to swap implementations:
- Development: Use `MockStorage`
- Testing: Use `MemoryStorage`
- Production: Use `BlockDeviceAdapter`
- Embedded: Use `NorFlashAdapter`

All without changing domain code!

### 3. Clarity

Business rules are **explicit** and **visible**:
- Dirty page conflicts are enforced in domain
- Page state transitions are in `Page` entity
- No hidden side effects (e.g., `data_mut()` explicitly marks dirty)

### 4. Maintainability

| Concern | Location |
|---------|----------|
| Business rules | `domain/page_buffer.rs` |
| I/O operations | `adapters/block_device_adapter.rs` |
| Error handling | `domain/error.rs` + `adapters/*/error.rs` |
| Type safety | `domain/value_objects/` |

Clear separation of concerns makes code easy to understand and modify.

## Migration from v0.4

### Breaking Changes

1. **Module structure changed**:
   - Old: `buffer_core`, `page_buffer`, `large_page_buffer` flat modules
   - New: `domain`, `adapters` layered modules

2. **Type names changed**:
   - `PageBuffer<D, N>` → `StackBuffer<D, N>` (adapter)
   - `LargePageBuffer<D>` → `HeapBuffer<D>` (adapter)

3. **API changes**:
   - `PageBuffer::read_page()` → `PageBuffer::load()` (returns `()` not `&Page`)
   - Access page via `buffer.current()` or `buffer.current_mut()`

### Migration Guide

**Before (v0.4)**:
```rust
use fatrs_adapters::PageBuffer;

let mut buffer = PageBuffer::<_, 8>::new(device);
buffer.read_page(0).await?;
let data = buffer.data().unwrap();
```

**After (v0.5)**:
```rust
use fatrs_adapters::adapters::StackBuffer4K;

let mut buffer = StackBuffer4K::new(device);
buffer.load(0).await?;
let data = buffer.data()?;
```

## Testing Strategy

### Domain Tests (`domain/`)
- **Unit tests**: Pure business logic
- **No I/O**: Use `MockStorage`
- **Fast**: All tests run in <1s
- **Deterministic**: No flaky tests

```rust
// Test business rule enforcement
#[tokio::test]
async fn test_dirty_page_conflict() {
    let storage = MockStorage::new();
    let mut buffer = PageBuffer::new(storage, config);

    buffer.load(PageNumber::new(0)).await.unwrap();
    buffer.modify(|data| data[0] = 42).unwrap();

    let result = buffer.load(PageNumber::new(1)).await;
    assert!(matches!(result, Err(DomainError::DirtyPageConflict { .. })));
}
```

### Adapter Tests (`adapters/`)
- **Integration tests**: With real `BlockDevice`
- **Verify**: Adapter correctly implements port
- **Edge cases**: Alignment, partial blocks, etc.

### Infrastructure Tests (Future)
- **End-to-end**: Full stack with real devices
- **Performance**: Benchmark throughput
- **Compatibility**: Different block sizes

## Performance Characteristics

### Zero-Cost Abstractions

All abstractions compile down to the same machine code as hand-written implementations:

```rust
// Generic domain service
impl<S: BlockStorage> PageBuffer<S, Vec<u8>> {
    pub async fn load(&mut self, num: PageNumber) -> Result<(), DomainError<S::Error>> {
        self.storage.read_blocks(addr, &mut buffer).await?;
        // ...
    }
}

// Monomorphizes to (with BlockDeviceAdapter):
impl PageBuffer<BlockDeviceAdapter<MyDevice>, Vec<u8>> {
    pub async fn load(&mut self, num: PageNumber) -> Result<(), DomainError<MyDeviceError>> {
        self.storage.device.read(addr, &mut aligned_buffer).await?;
        // Inlined, optimized, zero overhead!
    }
}
```

### Memory Layout

- **StackBuffer**: All data on stack (const generic size)
- **HeapBuffer**: Single `Vec` allocation
- **No virtual dispatch**: Monomorphization eliminates vtable lookups

## Future Enhancements

### 1. Stream Abstraction
Eliminate duplication between `PageStream` and `LargePageStream` using domain services.

### 2. Additional Adapters
- `NorFlashAdapter`: For NOR flash devices
- `NandFlashAdapter`: With wear leveling
- `MemoryAdapter`: Pure RAM storage

### 3. Advanced Features
- Write-through caching
- Read-ahead buffering
- Multi-page transactions

### 4. Async Runtime Flexibility
Support multiple async runtimes:
- Tokio (current)
- async-std
- Embassy (embedded)

## References

- [Hexagonal Architecture](https://alistair.cockburn.us/hexagonal-architecture/)
- [Clean Architecture](https://blog.cleancoder.com/uncle-bob/2012/08/13/the-clean-architecture.html)
- [Dependency Inversion Principle](https://en.wikipedia.org/wiki/Dependency_inversion_principle)
- [Domain-Driven Design](https://martinfowler.com/tags/domain%20driven%20design.html)

---

**Last Updated**: 2025-01-02
**Version**: v0.5.0 (Hexagonal Architecture Refactoring)
