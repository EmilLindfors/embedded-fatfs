# FATRS Architecture Analysis

## Current Structure Overview

The fatrs crate contains **5,307 lines** of core FAT filesystem logic organized into:

### Core Modules

| Module | Lines | Purpose |
|--------|-------|---------|
| **fs.rs** | 1,753 | Main `FileSystem<IO, TP, OCC>` struct and mount logic |
| **dir.rs** | 1,740 | Directory operations (create, list, navigate) |
| **file.rs** | 897 | File operations (read, write, seek) |
| **table.rs** | 917 | FAT allocation table logic (FAT12/16/32) |

### Supporting Modules

- `boot_sector.rs` - BIOS Parameter Block parsing & validation
- `dir_entry.rs` - Directory entry structures with LFN support
- `error.rs` - Error enum (generic over IO error type)
- `time.rs` - Timestamp handling & time providers
- `io.rs` - IO trait extensions (Read/Write/Seek helpers)
- `share.rs` - Runtime-agnostic `Shared<T>` abstraction
- `send_bounds.rs` - Send/Sync trait management

### Feature-Gated Optimizations

- `fat_cache.rs` - FAT sector caching
- `multi_cluster_io.rs` - Batched multi-cluster I/O
- `dir_cache.rs` - Directory cache
- `cluster_bitmap.rs` - Free cluster bitmap
- `transaction.rs` - Power-loss resilience
- `file_locking.rs` - Concurrent access protection

## Public API

### FileSystem<IO, TP, OCC>

The core type representing a mounted FAT volume.

**Generic Parameters**:
- `IO: Read + Write + Seek` - Storage device (currently directly uses embedded_io_async)
- `TP: TimeProvider` - Time source for timestamps
- `OCC: OemCpConverter` - Character encoding converter

**Key Methods**:
- `new(storage, options)` - Mount filesystem
- `root_dir()` - Get root directory
- `format(...)` - Format a volume

### File<'a, IO, TP, OCC>

Represents an open file.

**Key Methods**:
- `read(buf)` - Read bytes
- `write(buf)` - Write bytes
- `seek(pos)` - Seek to position
- `truncate()` - Resize file
- `flush()` - Commit changes

### Dir<'a, IO, TP, OCC>

Represents a directory with iteration support.

**Key Methods**:
- `create_file(name)` - Create file
- `open_file(name)` - Open existing file
- `create_dir(name)` - Create subdirectory
- `open_dir(name)` - Open subdirectory
- `iter()` - Iterate entries (with LFN support)
- `remove(name)` - Delete file/directory
- `rename(name)` - Rename entry

## Current Architecture Issues

### 1. Generic Over Concrete IO Trait, Not Port

```rust
// ❌ CURRENT
pub struct FileSystem<IO: Read + Write + Seek, TP, OCC> {
    disk: Shared<IO>,  // Directly uses embedded_io_async traits
}

// ✅ TARGET (like fatrs-adapters)
pub struct FileSystem<S: BlockStorage, TP, OCC> {
    storage: Shared<S>, // Uses domain port abstraction
}
```

**Impact**: FileSystem domain logic is tightly coupled to embedded_io_async

### 2. No Clear Layer Separation

Current monolithic structure:
- `fs.rs` (1,753 lines) - Everything crammed in one file
- File operations mixed with infrastructure details
- No distinction between domain rules and I/O

### 3. Hard to Test Domain Logic

To test file allocation, you need:
- A real (or mocked) filesystem image
- Buffer implementing Read+Write+Seek
- No way to test pure business logic in isolation

### 4. Multiple Generic Parameters Are Unclear

```rust
File<'a, IO, TP, OCC>
//        ^^  ^  ^^^
```

## Fatrs-Adapters: Hexagonal Architecture Reference

### Organization (Perfect Hexagonal Pattern)

```
fatrs-adapters/src/
├── domain/                    # Pure business logic
│   ├── entities/              # Objects with identity
│   │   ├── page.rs           # Page entity with state machine
│   │   └── page_state.rs     # PageState enum
│   │
│   ├── value_objects/         # Immutable validated types
│   │   ├── page_number.rs    # PageNumber (newtype u32)
│   │   ├── block_address.rs  # BlockAddress (newtype u32)
│   │   └── page_config.rs    # PageConfig (validated config)
│   │
│   ├── ports/                 # Abstract interfaces
│   │   └── block_storage.rs  # BlockStorage port trait
│   │
│   ├── error.rs              # Domain errors (generic over E)
│   ├── page_buffer.rs        # Domain service (core logic)
│   └── mod.rs                # Architecture documentation
│
├── adapters/                  # Concrete implementations
│   ├── block_device_adapter.rs # BlockDevice → BlockStorage
│   ├── stack_buffer.rs        # Stack-allocated wrapper
│   ├── heap_buffer.rs         # Heap-allocated wrapper
│   └── mod.rs
│
└── infrastructure/            # High-level utilities
    └── streaming/
        ├── stack_page_stream.rs
        ├── heap_page_stream.rs
        └── embedded_io_impl.rs
```

### Port Pattern: BlockStorage

```rust
pub trait BlockStorage: Send + Sync {
    type Error: Error + Send + Sync + 'static;

    async fn read_blocks(&mut self, start: BlockAddress, dest: &mut [u8])
        -> Result<(), Self::Error>;

    async fn write_blocks(&mut self, start: BlockAddress, src: &[u8])
        -> Result<(), Self::Error>;

    async fn size(&mut self) -> Result<u64, Self::Error>;

    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
```

**Key Design**:
- ✅ Owned methods (no `&self` required)
- ✅ Generic error type (stays in domain)
- ✅ Block-aligned semantics
- ✅ Simple, focused interface

### Benefits of Hexagonal Pattern

1. **Testability**: Mock BlockStorage for pure domain testing
2. **Flexibility**: Swap different storage implementations
3. **Clarity**: Three layers are obvious and separate
4. **Composition**: Stack layers (Domain → Adapter → Infrastructure)
5. **No Leakage**: Infrastructure details never reach domain

## Target Architecture

After refactoring, fatrs will follow the same pattern:

```
fatrs/src/
├── domain/                          # Pure business logic
│   ├── ports/
│   │   ├── block_storage.rs         # Abstract storage interface
│   │   ├── time_provider.rs         # Time source interface
│   │   └── oem_converter.rs         # Character encoding interface
│   │
│   ├── value_objects/
│   │   ├── cluster_number.rs        # Validated cluster ID
│   │   ├── sector_number.rs         # Validated sector ID
│   │   ├── file_size.rs             # File size with validation
│   │   └── file_name.rs             # 8.3 + LFN handling
│   │
│   ├── entities/
│   │   ├── file_entry.rs            # File with lifecycle
│   │   ├── dir_entry.rs             # Directory with state
│   │   └── fat_entry.rs             # FAT entry state machine
│   │
│   ├── services/
│   │   ├── cluster_allocator.rs     # Cluster allocation logic
│   │   ├── directory_manager.rs     # Directory operations
│   │   ├── file_manager.rs          # File I/O operations
│   │   └── filesystem_core.rs       # Mount/format/root access
│   │
│   └── error.rs                     # Domain errors
│
├── adapters/
│   ├── embedded_io_adapter.rs       # Read+Write+Seek → BlockStorage
│   ├── tokio_file_adapter.rs        # tokio::fs::File → BlockStorage
│   └── memory_adapter.rs            # Vec<u8> → BlockStorage (testing)
│
└── [existing files]                 # Keep for compatibility
    ├── fs.rs → re-exports domain
    ├── file.rs → re-exports domain
    ├── dir.rs → re-exports domain
    └── ...
```

## Benefits of Refactoring

✅ **Testability** - Mock storage for pure domain logic tests
✅ **Flexibility** - Swap storage implementations easily
✅ **Clarity** - Clean separation of concerns
✅ **Zero-cost** - Compile-time monomorphization, no runtime overhead
✅ **Maintainability** - Each layer is independent and focused
✅ **Backward Compatible** - Existing code continues to work
