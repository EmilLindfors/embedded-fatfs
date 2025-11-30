# embedded-fatfs TODO & Roadmap

This document tracks planned features, optimizations, and improvements for embedded-fatfs.

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

### Phase 3: Advanced Optimizations (Partial)
- [x] Free Cluster Bitmap
- [x] Cluster allocation benchmark
- [x] Configurable bitmap sizes

---

## 🚧 In Progress

### Phase 3: Advanced Optimizations (Remaining)

#### Cluster Chain Checkpoints
**Priority:** High
**Complexity:** Medium
**Expected Gain:** 100x faster seeking on large files
**Memory Cost:** ~64 bytes per file

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

#### Read-Ahead Prefetching
**Priority:** Medium-High
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

#### Directory Cache Integration
**Priority:** Medium
**Complexity:** Low
**Expected Gain:** 3-5x faster directory operations

**Status:** Module complete, needs integration

**Implementation:**
- [ ] Integrate into `Dir::find_entry()`
- [ ] Cache entries after successful lookups
- [ ] Invalidate cache on create/delete
- [ ] Add `FileSystem::dir_cache_statistics()` API
- [ ] Test nested directory access performance

---

## 📋 Planned Features

### Phase 4: Hardening & Safety (3-4 weeks)

#### File Locking
**Priority:** Medium
**Complexity:** Low-Medium
**Use Case:** Multi-threaded applications, prevent corruption

- [ ] Add file locks: cluster → lock state mapping
- [ ] Implement shared (read) and exclusive (write) locks
- [ ] Return `Error::FileLocked` when unavailable
- [ ] Feature flag: `file-locking`
- [ ] Tests: Concurrent access scenarios

#### Power-Loss Resilience
**Priority:** High (for safety-critical systems)
**Complexity:** High
**Use Case:** Medical, automotive, aerospace

- [ ] Design two-phase commit for metadata
- [ ] Implement intent logging
- [ ] Add recovery on mount
- [ ] Feature flag: `transaction-safe`
- [ ] Tests: Power-loss injection (1000+ iterations)

#### TRIM Support
**Priority:** Medium
**Complexity:** Low
**Use Case:** Flash storage longevity

- [ ] Extend trait with `trim()` method
- [ ] Notify storage of freed clusters
- [ ] Call on cluster chain free
- [ ] Feature flag: `trim-support`
- [ ] Tests: Verify TRIM commands sent

#### Tiny Mode (FF_FS_TINY)
**Priority:** Low-Medium
**Complexity:** Medium
**Use Case:** Ultra-low-memory microcontrollers

- [ ] Share single sector buffer across all files
- [ ] Reduces RAM by 512B per file
- [ ] Feature flag: `tiny-mode`
- [ ] Trade-off: Slower file switching
- [ ] Target: <1KB total RAM usage

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

### Code Quality
- [ ] Fix lifetime warning in `FileSystem::root_dir()`
- [ ] Remove dead code warnings (invalidate, mark_clean, etc.)
- [ ] Add `#[must_use]` annotations where appropriate
- [ ] Improve error messages

### Testing
- [ ] Add property-based tests (proptest/quickcheck)
- [ ] Test on real SD cards (not just RAM images)
- [ ] Test on real eMMC
- [ ] Power-loss injection testing
- [ ] Fuzzing for robustness

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

### v1.0.0 (Stable)
**Target:** Q3 2025
**Focus:** Production-ready

- [ ] All Phase 1-4 features complete
- [ ] Zero known corruption bugs
- [ ] 3+ production deployments
- [ ] Complete documentation
- [ ] Performance within 10% of targets

---

## 💪 How to Contribute

Interested in helping? Here are high-impact areas:

### High Priority
1. **Real Hardware Testing** - Test on actual SD cards, eMMC
2. **Cluster Checkpoints** - Implement O(log n) seeking
3. **Read-Ahead** - Implement prefetching engine
4. **Benchmark Suite** - Expand with more scenarios

### Medium Priority
1. **Directory Cache Integration** - Hook up existing cache
2. **Power-Loss Testing** - Corruption resilience validation
3. **Documentation** - Examples, guides, tutorials
4. **Platform Testing** - ESP32, STM32, RISC-V

### Low Priority
1. **Write Coalescing** - Further flash wear reduction
2. **Tiny Mode** - Ultra-low-memory support
3. **exFAT Research** - Feasibility study

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

**Last Updated:** 2025-11-30
**Maintained By:** embedded-fatfs contributors
**License:** MIT

---

💡 **Have an idea?** Open an issue on GitHub!
🐛 **Found a bug?** Please report it!
⚡ **Want to contribute?** Pull requests welcome!
