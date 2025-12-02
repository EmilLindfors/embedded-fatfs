# FATRS Hexagonal Architecture Refactoring Plan

## Overview

Transform fatrs core to match the elegant hexagonal structure in fatrs-adapters, while simultaneously fixing the critical file corruption bug.

## Two-Track Approach

### Track A: Fix Critical File Corruption Bug
**Priority**: CRITICAL
**Duration**: 1 week
**Must complete before**: Any production release

### Track B: Hexagonal Architecture Refactoring
**Priority**: HIGH
**Duration**: 6 weeks
**Can proceed**: In parallel with Track A

## Track A: File Corruption Bug Fix (Week 1)

### Goal
Fix the critical bug where writing multiple files causes data corruption due to unflushed directory entries.

### Tasks

#### 1. Fix Drop Implementation
**File**: `fatrs/src/file.rs:460-471`

Add proper cleanup in drop:
```rust
impl Drop for File<'_, IO, TP, OCC> {
    fn drop(&mut self) {
        if self.dir_entry.is_some() && self.is_dirty() {
            // Force synchronous flush or panic
            // Or document that flush() MUST be called before drop
        }
    }
}
```

#### 2. Fix Write Path to Flush Directory Entry
**File**: `fatrs/src/file.rs:369-380`

Add flush after size update:
```rust
async fn update_dir_entry_after_write(&mut self) -> Result<(), Self::Error> {
    if let Some(ref mut editor) = self.dir_entry {
        let new_size = self.offset.max(self.initial_size);
        editor.set_size(new_size);
        self.flush_dir_entry().await?; // ✅ ADD THIS
    }
    Ok(())
}
```

#### 3. Fix Directory Entry Position Caching
**File**: `fatrs/src/dir_entry.rs:643-645`

Add position validation:
```rust
pub(crate) fn editor(&self) -> DirEntryEditor {
    // Add generation counter or validation
    DirEntryEditor::new(self.entry.clone(), self.pos, self.generation)
}
```

#### 4. Validate Position Before Write
**File**: `fatrs/src/dir_entry.rs:541-550`

Add validation:
```rust
pub(crate) async fn write<IO: Write + Seek>(
    &mut self,
    fs: &FileSystem<IO, TP, OCC>,
) -> Result<(), Error<IO::Error>> {
    // ✅ Validate position is still valid
    self.validate_position(fs)?;

    fs.disk
        .with(|disk| async {
            disk.seek(SeekFrom::Start(self.pos)).await?;
            self.data.serialize(&mut disk).await
        })
        .await
}
```

#### 5. Add Comprehensive Tests

Create `fatrs/tests/multi_file_corruption.rs`:
```rust
#[tokio::test]
async fn test_write_multiple_files_no_corruption() {
    // Test writing multiple files
    // Verify each file retains its own content
    // Test with explicit and implicit flushes
}

#[tokio::test]
async fn test_directory_entry_flush() {
    // Test that directory entries are flushed
    // Verify size is written to disk
}

#[tokio::test]
async fn test_concurrent_file_operations() {
    // Test multiple files open simultaneously
    // Verify no corruption
}
```

### Deliverables
- ✅ All four critical issues fixed
- ✅ New tests pass
- ✅ All existing tests still pass
- ✅ Ready for v0.4.1 release

---

## Track B: Hexagonal Architecture (Weeks 2-7)

### Phase 1: Define Domain Ports (Week 2)

#### 1.1 Create BlockStorage Port
**New file**: `fatrs/src/domain/ports/block_storage.rs`

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

#### 1.2 Define TimeProvider Port
**New file**: `fatrs/src/domain/ports/time_provider.rs`

Move existing `TimeProvider` trait here with documentation.

#### 1.3 Define OemConverter Port
**New file**: `fatrs/src/domain/ports/oem_converter.rs`

Move existing `OemCpConverter` trait here with documentation.

#### 1.4 Create Domain Error Type
**New file**: `fatrs/src/domain/error.rs`

```rust
pub enum DomainError<E> {
    Storage(E),
    InvalidCluster(u32),
    InvalidFileName(String),
    DirectoryFull,
    DiskFull,
    // ... other domain errors
}
```

### Phase 2: Extract Value Objects & Entities (Week 3)

#### 2.1 Value Objects
Create immutable, validated types:

- `domain/value_objects/cluster_number.rs`
- `domain/value_objects/sector_number.rs`
- `domain/value_objects/file_size.rs`
- `domain/value_objects/file_name.rs`

Example:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClusterNumber(u32);

impl ClusterNumber {
    pub fn new(value: u32) -> Result<Self, DomainError> {
        if value == 0 {
            return Err(DomainError::InvalidCluster(value));
        }
        Ok(Self(value))
    }

    pub fn get(&self) -> u32 {
        self.0
    }
}
```

#### 2.2 Entities
Create entities with lifecycle:

- `domain/entities/file_entry.rs`
- `domain/entities/dir_entry.rs`
- `domain/entities/fat_entry.rs`

Example:
```rust
pub struct FileEntry {
    name: FileName,
    size: FileSize,
    first_cluster: Option<ClusterNumber>,
    state: FileState,
}

enum FileState {
    Clean,
    Dirty,
    Deleted,
}
```

### Phase 3: Refactor Core Services (Weeks 4-5)

#### 3.1 Cluster Allocator Service
**New file**: `domain/services/cluster_allocator.rs`

Extract from `table.rs`:
- `alloc_cluster()`
- `free_cluster()`
- `count_free_clusters()`
- FAT12/16/32 logic

```rust
pub struct ClusterAllocator<S: BlockStorage> {
    storage: S,
    config: FatConfig,
}

impl<S: BlockStorage> ClusterAllocator<S> {
    pub async fn allocate(&mut self) -> Result<ClusterNumber, DomainError<S::Error>> {
        // Pure business logic using storage port
    }
}
```

#### 3.2 Directory Manager Service
**New file**: `domain/services/directory_manager.rs`

Extract from `dir.rs`:
- Directory iteration
- Entry creation/deletion
- LFN handling

#### 3.3 File Manager Service
**New file**: `domain/services/file_manager.rs`

Extract from `file.rs`:
- File I/O operations
- Seek logic
- Flush logic

#### 3.4 Filesystem Core Service
**New file**: `domain/services/filesystem_core.rs`

Extract from `fs.rs`:
- Mount logic
- Format logic
- Boot sector handling

### Phase 4: Create Adapters (Week 6)

#### 4.1 Embedded IO Adapter
**New file**: `adapters/embedded_io_adapter.rs`

```rust
pub struct EmbeddedIoAdapter<IO: Read + Write + Seek> {
    io: IO,
}

impl<IO: Read + Write + Seek> BlockStorage for EmbeddedIoAdapter<IO> {
    type Error = IO::Error;

    async fn read_blocks(&mut self, start: BlockAddress, dest: &mut [u8])
        -> Result<(), Self::Error> {
        // Translate BlockStorage semantics to embedded_io_async semantics
    }
}
```

#### 4.2 Tokio File Adapter
**New file**: `adapters/tokio_file_adapter.rs`

```rust
pub struct TokioFileAdapter {
    file: tokio::fs::File,
}

impl BlockStorage for TokioFileAdapter {
    type Error = std::io::Error;
    // ...
}
```

#### 4.3 Memory Adapter (for testing)
**New file**: `adapters/memory_adapter.rs`

```rust
pub struct MemoryAdapter {
    data: Vec<u8>,
}

impl BlockStorage for MemoryAdapter {
    type Error = std::io::Error;
    // ...
}
```

### Phase 5: Update FileSystem API (Week 7)

#### 5.1 Update FileSystem Struct
**File**: `fatrs/src/fs.rs`

```rust
// New API
pub struct FileSystem<S: BlockStorage, TP: TimeProvider, OCC: OemCpConverter> {
    storage: Shared<S>,
    time_provider: TP,
    oem_converter: OCC,
    // ... other fields
}

// Backward compatibility type alias
pub type FileSystemLegacy<IO, TP, OCC> =
    FileSystem<EmbeddedIoAdapter<IO>, TP, OCC>;
```

#### 5.2 Provide Convenient Constructors

```rust
impl<S: BlockStorage, TP, OCC> FileSystem<S, TP, OCC> {
    pub async fn new(storage: S, time_provider: TP, converter: OCC)
        -> Result<Self, Error<S::Error>> {
        // New constructor using ports
    }
}

// Convenience methods
impl FileSystem<EmbeddedIoAdapter<Vec<u8>>, DefaultTimeProvider, LossyOemCpConverter> {
    pub async fn create_in_memory(size: usize) -> Result<Self, Error<std::io::Error>> {
        // Helper for common case
    }
}
```

### Phase 6: Testing & Validation (Week 7)

#### 6.1 Mock Storage for Testing
**New file**: `tests/mocks/mock_storage.rs`

```rust
pub struct MockStorage {
    data: Vec<u8>,
    read_count: usize,
    write_count: usize,
}

impl BlockStorage for MockStorage {
    // Track operations for testing
}
```

#### 6.2 Pure Domain Tests

Create unit tests for each domain service:
- `tests/domain/cluster_allocator_tests.rs`
- `tests/domain/directory_manager_tests.rs`
- `tests/domain/file_manager_tests.rs`

#### 6.3 Integration Tests

Ensure all existing tests pass with new architecture:
```bash
cargo test --all-features
```

#### 6.4 Benchmark Suite

Verify zero-cost abstraction:
```bash
cargo bench
```

Compare before/after performance.

---

## Migration Path

### For Library Users

#### Old API (still works)
```rust
use fatrs::FileSystem;

let fs = FileSystem::new(
    storage,  // impl Read + Write + Seek
    time_provider,
    oem_converter,
).await?;
```

#### New API (preferred)
```rust
use fatrs::{FileSystem, adapters::EmbeddedIoAdapter};

let adapter = EmbeddedIoAdapter::new(storage);
let fs = FileSystem::new(
    adapter,  // impl BlockStorage
    time_provider,
    oem_converter,
).await?;
```

### Deprecation Timeline

- **v0.4.1**: Bug fixes only
- **v0.5.0**: New hexagonal architecture, old API still works
- **v0.6.0**: Soft deprecation of old API (warnings)
- **v0.7.0**: Hard deprecation (compilation errors on old API)

---

## Success Criteria

### Track A (Bug Fix)
- ✅ Multi-file write test passes
- ✅ Directory entry flush test passes
- ✅ No data corruption in any scenario
- ✅ All existing tests pass

### Track B (Refactoring)
- ✅ Clean three-layer architecture (domain/adapters/infrastructure)
- ✅ All domain logic testable with mock storage
- ✅ Zero performance regression (benchmarks)
- ✅ All existing tests pass with new architecture
- ✅ Backward compatibility maintained
- ✅ Documentation updated

---

## Release Plan

1. **v0.4.1** (Week 1) - Critical bug fix
   - File corruption bug fixed
   - New tests added
   - No breaking changes

2. **v0.5.0** (Week 7) - Hexagonal architecture
   - New domain/adapter structure
   - Old API still works (type aliases)
   - Performance validated
   - Documentation updated

3. **v0.6.0** (Future) - API refinement
   - Deprecation warnings on old API
   - New convenience methods
   - Additional adapters (if needed)

---

## Resources Needed

- **Time**: 7 weeks total (1 week bug fix + 6 weeks refactoring)
- **Testing**: Comprehensive test suite expansion
- **Documentation**: Architecture docs, migration guide
- **Benchmarking**: Performance validation

## Risk Mitigation

- **Backward Compatibility**: Type aliases maintain old API
- **Testing**: Extensive test coverage before/after
- **Performance**: Benchmark suite to catch regressions
- **Incremental**: Can pause refactoring if issues arise
