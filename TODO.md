# fatrs TODO & Roadmap

This document tracks planned features, optimizations, and improvements for fatrs (formerly embedded-fatfs).

---

## ✅ Completed (Phases 1-3)

### Phase 1: Foundation & Quick Wins
- [x] FAT Sector Cache (4KB-16KB configurable)
- [x] Basic benchmarking suite (sequential I/O)
- [x] Feature flags system
- [x] Cache statistics API

### Phase 2: Core Caching Infrastructure
- [x] Multi-Cluster Batched I/O
- [x] Directory Entry Cache
- [x] Enhanced FileContext with optimization fields
- [x] Random access benchmark
- [x] Comprehensive testing

### Phase 3: Advanced Optimizations
- [x] Free Cluster Bitmap
- [x] Cluster allocation benchmark
- [x] Configurable bitmap sizes (small/medium/large)

### Phase 4: Hardening & Safety
- [x] File Locking (shared/exclusive locks)
- [x] Transaction-safe writes (power-loss resilience)
- [x] Send/Sync support for multi-threaded executors

### Phase 5: Hexagonal Architecture
- [x] Split into separate crates (domain, ports, adapters)
- [x] BlockDevice trait abstraction
- [x] Stack-allocated adapters (fatrs-adapters-core)
- [x] Heap-allocated adapters (fatrs-adapters-alloc)
- [x] Platform-specific implementations (fatrs-block-platform)
- [x] FUSE filesystem support (fatrs-fuse)
- [x] Generation counter for stale directory entry detection ← **Completed!**
- [x] True stack/heap separation in adapters ← **Completed!**

---

## 🚧 In Progress

### Outstanding TODOs from Codebase Audit

#### High Priority
- [ ] **Parent inode tracking** (fatrs-fuse/src/lib.rs:394) - Track parent directory properly in FUSE layer
- [ ] **Physical drive mapping** (fatrs-block-platform/src/windows.rs:415) - Detect physical drive for Windows devices
- [ ] **macOS BlockDevice Implementation** (fatrs-block-platform/src/macos.rs) - Complete stub implementation
  - [ ] Implement `MacOSBlockDevice::open()` using diskutil and raw device access
  - [ ] Implement Clone, ErrorType, Read, Write, Seek, BlockDevice traits
  - [ ] Implement `list_disks()` using diskutil list or system APIs
  - [ ] Add tests for macOS disk access

#### Medium Priority
- [x] **Timestamp from TimeProvider** (fatrs/src/transaction.rs:387) - Get actual timestamp instead of hardcoded 0 ← **Completed!**
- [ ] **AsyncIterator for File Extents** (fatrs/src/file.rs:263) - Implement extents using AsyncIterator pattern
- [ ] **Directory Cache Placeholder** (fatrs/src/dir_cache.rs:16-18) - Replace `DirFileEntryData` placeholder with actual type

#### Low Priority
- [ ] **Reduce panic! usage** - Replace panic! with proper error handling in:
  - fatrs-adapters/src/adapters/block_device_adapter.rs:96,135 (alloc unavailable)
  - fatrs-adapters/src/domain/error.rs:96 (error conversion helper)
  - fatrs/src/file.rs:161,801 (invalid state panics)

#### Pre-existing Test Failures
- [ ] Fix `test_concurrent_file_creation_unique_names` - UnexpectedEof error
- [ ] Fix `test_delete_file` - InvalidInput on file removal
- [ ] Fix `test_rename_file` - Embassy integration test failure
- [ ] Fix `test_write_large_file` - Embassy integration test failure

### Cluster Chain Checkpoints
**Priority:** Medium
**Complexity:** Medium
**Expected Gain:** 100x faster seeking on large files
**Memory Cost:** ~64 bytes per file
**Status:** Feature flag exists, needs implementation

**Description:**
- Store periodic checkpoints (every Nth cluster) in FileContext
- Binary search through checkpoints for O(log n) seeking
- Currently: Seeking 1GB into file = ~262,000 cluster reads
- With checkpoints: ~8-16 cluster reads

**Implementation:**
- [ ] Add checkpoint recording during sequential reads/writes
- [ ] Implement binary search in `File::seek()`
- [ ] Benchmark large file seek performance
- [ ] Test with files >100MB

### Read-Ahead Prefetching
**Priority:** Low-Medium
**Complexity:** Medium
**Expected Gain:** 20-40% sequential read throughput
**Memory Cost:** 1-4 cluster buffers (~4KB-16KB)

**Description:**
- Detect sequential access patterns
- Asynchronously prefetch next cluster
- Cache in read-ahead buffer

**Implementation:**
- [ ] Add read-ahead buffer to FileContext
- [ ] Detect sequential access pattern
- [ ] Implement async prefetch (if supported by runtime)
- [ ] Invalidate on seek/write
- [ ] Benchmark throughput improvement

---

## 📋 Planned Features

### TRIM Support
**Priority:** Medium
**Complexity:** Low
**Use Case:** Flash storage longevity

- [ ] Extend BlockDevice trait with `trim()` method
- [ ] Notify storage of freed clusters
- [ ] Call on cluster chain free
- [ ] Feature flag: `trim-support`
- [ ] Tests: Verify TRIM commands sent

### Tiny Mode (FF_FS_TINY)
**Priority:** Low-Medium
**Complexity:** Medium
**Use Case:** Ultra-low-memory microcontrollers

- [ ] Share single sector buffer across all files
- [ ] Reduces RAM by 512B per file
- [ ] Feature flag: `tiny-mode`
- [ ] Trade-off: Slower file switching
- [ ] Target: <1KB total RAM usage

---

### Performance Improvements

#### RwLock for Read-Heavy Caches
**Priority:** Medium
**Complexity:** Low
**Impact:** 2-4x better read concurrency on multicore

- [ ] Change `fat_cache` from `Mutex` to `RwLock` in `fs.rs`
- [ ] Change `dir_cache` from `Mutex` to `RwLock` in `fs.rs`
- [ ] Update `fat_cache.rs` to use `read()`/`write()` pattern
- [ ] Update `dir_cache.rs` to use `read()`/`write()` pattern
- [ ] Update `table.rs` FAT access patterns
- [ ] Benchmark read concurrency improvement

### Documentation & Examples

#### Async Runtime Examples
**Priority:** Medium
**Complexity:** Documentation only

- [ ] Add Embassy multicore example (RP2350, ESP32-S3)
- [ ] Add Tokio multi-threaded example with `Arc`
- [ ] Add async-std example
- [ ] Document `StaticCell` pattern for no_std multicore
- [ ] Document BlockDevice implementations for different platforms

#### Platform-Specific Guides
**Priority:** Low
**Complexity:** Documentation only

- [ ] Windows disk access guide
- [ ] Linux block device guide
- [ ] macOS disk access guide
- [ ] Embedded SPI SD card guide
- [ ] Performance tuning guide for each platform

---

## 🔬 Research & Investigation

### exFAT Support
**Priority:** Low (unless >4GB files needed)
**Complexity:** Very High (~3-6 months)
**Status:** Research phase

**Benefits:**
- No 4GB file size limit
- Native cluster bitmap
- Better flash optimization

**Considerations:**
- Patent licensing in some jurisdictions
- Significant spec differences
- Possibly separate crate (`embedded-exfat`)

**Tasks:**
- [ ] Review exFAT specification
- [ ] Assess patent/licensing requirements
- [ ] Design API compatibility layer
- [ ] Prototype basic implementation

### Write Coalescing
**Priority:** Medium
**Complexity:** Medium
**Expected Gain:** Additional 2-4x flash wear reduction

- [ ] Buffer small writes in RAM
- [ ] Flush on cluster boundary or timeout
- [ ] Combine with multi-cluster I/O
- [ ] Feature flag: `write-coalescing`

### Lazy FAT Mirroring
**Priority:** Low
**Complexity:** Low
**Expected Gain:** Reduced write amplification

- [ ] Batch FAT mirror updates
- [ ] Write all mirrors in one operation
- [ ] Reduces redundant I/O

---

## 🐛 Known Issues & Improvements

### Critical Issues Fixed (v0.4.0)
- [x] **Generation Counter for Stale Directory Entries** - Prevents corruption from writing to reallocated clusters
- [x] **set_dirty_flag Re-enabled** - Fixed DiskSlice seek bug and flag clearing logic
- [x] **fatrs-block-platform Compilation** - Fixed sdspi, stream, and fmt module issues

### Code Quality
- [ ] Fix lifetime warning in `FileSystem::root_dir()`
- [ ] Remove dead code warnings (invalidate, mark_clean, etc.)
- [ ] Add `#[must_use]` annotations where appropriate
- [ ] Improve error messages
- [x] Generation counter implementation ← **Completed!**
- [x] Stale directory entry detection ← **Completed!**

### Testing
- [ ] Add property-based tests (proptest/quickcheck)
- [ ] Test on real SD cards (not just RAM images)
- [ ] Test on real eMMC
- [ ] Power-loss injection testing
- [ ] Fuzzing for robustness
- [x] Generation counter tests ← **Completed!**
- [ ] Fix pre-existing test failures (see Outstanding TODOs above)

### Documentation
- [ ] Add more inline code examples
- [ ] Create performance tuning guide
- [ ] Add embedded examples (ESP32, STM32, etc.)
- [ ] Video tutorial / blog post

### Benchmarks
- [ ] Real hardware benchmarks (not just simulated)
- [ ] Comparison with ChaN FatFs (via FFI)
- [ ] Comparison with Linux kernel FAT driver
- [ ] Memory profiling benchmarks

---

## 🎯 Performance Targets

### Current Status (with all optimizations)
| Metric | Target | Current | Status |
|--------|--------|---------|--------|
| Sequential Read | >3 MB/s | ~4 MB/s | ✅ Exceeded |
| Random Access | <20ms | ~10ms | ✅ Exceeded |
| Allocation (90% full) | <10ms | ~5ms | ✅ Exceeded |
| Cache Hit Rate | >80% | 99%+ | ✅ Exceeded |
| Flash Wear Reduction | 10x | 16x | ✅ Exceeded |

### Stretch Goals (Phase 3 complete)
- [ ] Sequential read: 5 MB/s (near raw storage)
- [ ] Random access: <5ms average
- [ ] Large file seek: <10ms (any offset)
- [ ] Allocation: <1ms (any fill level)

---

## 🌟 Nice-to-Have Features

### Advanced Features
- [ ] Compression support (transparent file compression)
- [ ] Encryption support (at-rest encryption)
- [ ] Deduplication (for firmware updates)
- [ ] Snapshots (filesystem-level snapshots)

### Developer Experience
- [ ] Better error messages with suggestions
- [ ] Performance profiling tools
- [ ] Configuration wizard for feature selection
- [ ] CI/CD performance regression tracking

### Platform Support
- [ ] WebAssembly support
- [ ] Formal verification (for safety-critical code)
- [ ] MISRA-C compliance checking

---

## 📦 Release Planning

### v0.2.0 (Next Release)
**Target:** Q1 2025
**Focus:** Phase 3 completion + documentation

- [ ] Complete cluster checkpoints
- [ ] Complete read-ahead prefetching
- [ ] Integrate directory cache
- [ ] Comprehensive documentation update
- [ ] Real hardware validation
- [ ] Performance comparison report

### v0.3.0 (Future)
**Target:** Q2 2025
**Focus:** Hardening & safety

- [ ] File locking
- [ ] Power-loss resilience
- [ ] TRIM support
- [ ] Extensive testing on real hardware

### v0.4.0 (Future)
**Target:** Q3 2025
**Focus:** Concurrent multicore access (no_std first)

- [x] Thread-safe status flags (`Cell` → `AtomicU8`) ← **Completed!**
- [x] File-level locking (shared/exclusive) ← **Completed!**
- [ ] RwLock for read-heavy caches
- [x] `FileSystem: Send + Sync` when `IO: Send` ← **Completed!**
- [ ] Embassy multicore examples (RP2350, ESP32-S3)
- [ ] Tokio multi-threaded examples
- [ ] Concurrent access benchmarks
- [ ] Deadlock prevention tests

### v1.0.0 (Stable)
**Target:** Q4 2025
**Focus:** Production-ready

- [ ] All Phase 1-5 features complete
- [ ] Zero known corruption bugs
- [ ] 3+ production deployments
- [ ] Complete documentation
- [ ] Performance within 10% of targets
- [ ] Concurrent access verified on multicore hardware

---

## 💪 How to Contribute

Interested in helping? Here are high-impact areas:

### High Priority
1. **Thread-Safe Status Flags** - Replace `Cell` with `AtomicU8` for `Sync`
2. **File-Level Locking** - Implement shared/exclusive locks
3. **Real Hardware Testing** - Test on actual SD cards, eMMC
4. **Cluster Checkpoints** - Implement O(log n) seeking

### Medium Priority
1. **RwLock for Caches** - Better read concurrency
2. **Directory Cache Integration** - Hook up existing cache
3. **Multicore Examples** - Embassy, Tokio, async-std
4. **Platform Testing** - ESP32, STM32, RP2350 (multicore)

### Low Priority
1. **Heapless File Locking** - no_std without alloc
2. **Write Coalescing** - Further flash wear reduction
3. **Tiny Mode** - Ultra-low-memory support
4. **exFAT Research** - Feasibility study

---

## 📊 Success Metrics

### Performance (v1.0 targets)
- [x] 5-10x improvement over baseline ← **Achieved!**
- [ ] Competitive with ChaN FatFs
- [ ] <100KB RAM for high-perf config
- [ ] <1KB RAM for tiny mode

### Quality
- [ ] Zero known corruption bugs
- [ ] 100% test coverage on core paths
- [ ] Power-loss resilience validated (10,000+ iterations)
- [ ] 3+ real hardware platforms tested

### Concurrent Access (v0.4 targets)
- [x] `FileSystem: Send + Sync` when `IO: Send` ← **Completed!**
- [x] File locking prevents concurrent write corruption ← **Completed!**
- [ ] RwLock improves read concurrency by 2-4x
- [ ] Zero deadlocks in stress tests (10,000+ iterations)
- [ ] Works on Embassy multicore (RP2350, ESP32-S3)
- [ ] Works with tokio multi-threaded runtime
- [ ] Concurrent reads scale linearly with cores

### Adoption
- [ ] >1000 crates.io downloads/month
- [ ] >500 GitHub stars
- [ ] 3+ production deployments
- [ ] Integration with Embassy/RTIC

---

## 📚 Research References

See [ARCHITECTURE.md](ARCHITECTURE.md#research-references) and `PERFORMANCE_ROADMAP.md` (in git history) for:
- ChaN FatFs application notes
- exFAT specification
- Academic papers on FAT optimization
- PX5 FILE system documentation
- Linux kernel FAT driver source

---

## 📝 Recent Updates

### v0.4.0 Codebase Audit & Fixes (2025-12-05)

**What was investigated:**
- Complete audit of TODOs, stubs, unused code, and compilation issues
- Found 11 TODO comments, 1 stub implementation (macOS), 447 unwrap/expect calls
- Identified critical stale directory entry corruption risk

**What was fixed:**
- ✅ **Generation Counter Implementation** - Prevents corruption from stale directory positions
  - Added `AtomicU64 cluster_generation` to FileSystem
  - Increments on every cluster deallocation
  - DirEntryEditor validates generation before writing
  - Returns `Error::StaleDirectoryEntry` if clusters were reallocated
  - **Both files and directories are protected** via DirEntryEditor
- ✅ **set_dirty_flag Re-enabled** - Was disabled due to corruption, now safe
  - Fixed DiskSlice::seek() returning relative vs absolute positions
  - Fixed `flags.dirty |= dirty` to `flags.dirty = dirty`
  - All corruption tests passing
- ✅ **fatrs-block-platform Compilation Fixes**
  - sdspi.rs: Removed invalid crate-level attribute, fixed fmt import
  - stream.rs: Added missing feature gate for embedded-io-async
  - lib.rs: Made fmt module available for sdspi feature
  - Cargo.toml: Fixed defmt feature to use `dep:defmt`

**Metrics:**
- Files changed: 8
- Lines added: +69
- Lines removed: -31
- Tests: 47 passing (39 core + 6 corruption + 2 generation)
- Code quality: No new warnings, all clippy checks pass

**What still needs work:**
See "Outstanding TODOs from Codebase Audit" section above for complete list.

### Timestamp from TimeProvider Implementation (2025-12-05)

**What was implemented:**
- Added `DateTime::to_unix_timestamp()` method to convert DOS datetime to Unix timestamp
- Updated `TransactionLog::begin_transaction()` to accept timestamp parameter
- Updated `FileSystem::with_transaction()` to get timestamp from TimeProvider
- Moved `with_transaction()` to new impl block with `TP: TimeProvider` bound

**Technical details:**
- Converts FAT DOS datetime (1980-2107) to Unix timestamp (seconds since 1970)
- Handles leap years correctly (tested with 2000-02-29)
- Transaction log now records actual creation time instead of 0
- Useful for debugging and forensics of power-loss recovery

**Metrics:**
- Files changed: 3 (fatrs/src/time.rs, transaction.rs, fs.rs)
- Tests added: 1 (`datetime_to_unix_timestamp` with 4 test cases)
- All 39 tests passing with transaction-safe feature
- Zero clippy warnings

### Ergonomic Transaction Log API (2025-12-05)

**What was implemented:**
- Added `TransactionLogConfig` struct for configuring transaction log placement
- Added `FsOptions::with_transaction_log()` for automatic placement
- Added `FsOptions::with_transaction_log_at()` for custom placement
- Updated `FileSystem::new()` to use configuration from options
- Added comprehensive documentation with examples

**API Examples:**
```rust
// Enable transaction log with automatic placement
let options = FsOptions::new().with_transaction_log();
let fs = FileSystem::new(disk, options).await?;

// Or specify custom log location
let options = FsOptions::new().with_transaction_log_at(100, 8);
let fs = FileSystem::new(disk, options).await?;

// Backwards compatible - works without explicit configuration
let options = FsOptions::new();
let fs = FileSystem::new(disk, options).await?; // Uses default placement
```

**Technical details:**
- `log_start_sector: 0` means automatic placement (before data area)
- Automatic placement uses `first_data_sector.saturating_sub(4)`
- Configuration is optional for backwards compatibility
- Transaction log always created when `transaction-safe` feature enabled

**Metrics:**
- Files changed: 1 (fatrs/src/fs.rs)
- New types: `TransactionLogConfig`
- New methods: `with_transaction_log()`, `with_transaction_log_at()`
- All 39 tests passing + 2 integration tests
- Zero clippy warnings

---

**Last Updated:** 2025-12-05
**Maintained By:** embedded-fatfs contributors
**License:** MIT

---

💡 **Have an idea?** Open an issue on GitHub!
🐛 **Found a bug?** Please report it!
⚡ **Want to contribute?** Pull requests welcome!
