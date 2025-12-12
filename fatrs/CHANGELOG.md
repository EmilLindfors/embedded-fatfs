# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Comprehensive edge case tests** (`tests/edge_cases.rs`): Added 21 new tests covering:
  - Rename operations: same name, long filenames, existing files, nested directories, FAT16, sequential renames
  - Truncate operations: multiple truncates, empty files, truncate + large write
  - File write boundaries: cluster boundaries, middle-of-file overwrites, many small writes
  - FAT cache stress: many files triggering cache eviction, interleaved operations
  - Seek operations: seek to 0, negative offsets, SeekFrom::End
  - Delete operations: delete and recreate, long filename deletion

### Fixed

- **FAT cache writeback offset bug**: Fixed critical bug where the FAT cache stored absolute disk offsets but treated them as relative offsets during cache eviction writeback. This caused FAT entries to be written to incorrect disk locations, corrupting cluster chains when multiple files were created. This also caused `WriteZero` errors during large file writes. The fix ensures the cache consistently uses relative offsets, while `DiskSlice` handles translation to absolute positions. (`fat_cache.rs`, `fs.rs`)

- **StaleDirectoryEntry after truncate**: Fixed bug where truncating a file would increment the cluster generation counter (due to freeing clusters), causing subsequent writes to fail with `StaleDirectoryEntry`. Added `refresh_generation()` method to `DirEntryEditor` and call it after truncate operations. (`dir_entry.rs`, `file.rs`)

- **Rename fails with InvalidInput on FAT16**: Fixed bug in `rename_internal()` where it used absolute disk offsets directly for seeking within the directory stream. The `offset_range` in `DirEntry` contains absolute positions, but directory streams expect relative offsets. Added conversion from absolute to relative offset before seeking. (`dir.rs`)
