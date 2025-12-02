# File Corruption Bug Analysis

## Overview

**CRITICAL BUG**: Writing a second file to a FAT image corrupts the first file with the second file's content.

## Root Cause

The bug is caused by **unflushed directory entries** - file metadata updates are made in-memory but never persisted to disk.

## Four Critical Issues

### 1. Drop Implementation Doesn't Force Flush

**Location**: `fatrs/src/file.rs:460-471`

```rust
impl<IO: Write + Read + Seek, TP: TimeProvider, OCC: OemCpConverter> Drop
    for File<'_, IO, TP, OCC>
{
    fn drop(&mut self) {
        if self.dir_entry.is_some() && self.is_dirty() {
            warn!("File is dirty on drop, but we can't flush it because drop can't be async");
        }
    }
}
```

**Problem**:
- When files are dropped without explicit flush, dirty directory entries are lost
- Drop only warns about dirty entries; it doesn't flush them
- Result: File size updates in memory are discarded

### 2. File Write Doesn't Flush Directory Entry

**Location**: `fatrs/src/file.rs:369-380`

```rust
async fn update_dir_entry_after_write(&mut self) -> Result<(), Self::Error> {
    if let Some(ref mut editor) = self.dir_entry {
        let new_size = self.offset.max(self.initial_size);
        editor.set_size(new_size);
        // ❌ MISSING: await self.flush_dir_entry()?
    }
    Ok(())
}
```

**Problem**:
- `update_dir_entry_after_write()` updates size in-memory only
- Missing: Flush after `set_size()`
- Result: Directory entry on disk shows size=0 even after writing data

### 3. Directory Entry Position Cached Forever

**Location**: `fatrs/src/dir_entry.rs:643-645`

```rust
pub(crate) fn editor(&self) -> DirEntryEditor {
    DirEntryEditor::new(self.entry.clone(), self.pos) // ❌ Position cached here
}
```

**Problem**:
- `DirEntryEditor` captures the disk position once at creation
- Position is never updated if the directory cluster reallocates
- Result: Stale positions point to wrong locations on disk

### 4. Stale Position Used Without Validation

**Location**: `fatrs/src/dir_entry.rs:541-550`

```rust
pub(crate) async fn write<IO: Write + Seek>(
    &mut self,
    fs: &FileSystem<IO, TP, OCC>,
) -> Result<(), Error<IO::Error>> {
    fs.disk
        .with(|disk| async {
            disk.seek(SeekFrom::Start(self.pos)).await?; // ❌ Stale position used
            self.data.serialize(&mut disk).await
        })
        .await
}
```

**Problem**:
- `DirEntryEditor::write()` writes to cached position without checking validity
- If directory was reallocated, writes go to wrong disk location
- Result: Directory entries written to wrong locations, corrupting other data

## How The Corruption Happens

1. **Create fileA.txt, write 15 bytes**
   - Directory entry created at position X
   - Data written to clusters
   - Size in memory = 15 bytes
   - Size on disk = 0 bytes (not flushed!)

2. **Create fileB.txt in same directory**
   - Directory might reallocate if it needs more space
   - fileA's cached position becomes stale
   - fileB created at new position Y

3. **Write fileB.txt, 15 bytes**
   - Data written to clusters
   - Size in memory = 15 bytes
   - Size on disk = 0 bytes (not flushed!)

4. **Drop both files**
   - Warning logged about dirty files
   - Neither file's directory entry is flushed
   - Both files show size=0 on disk

5. **Read fileA.txt**
   - Directory says size=0
   - File appears empty or returns wrong data
   - **DATA CORRUPTED!**

## Affected Code Locations

### HIGHEST PRIORITY
- `/fatrs/src/file.rs` lines 460-471 (Drop implementation)
- `/fatrs/src/file.rs` lines 369-380 (update_dir_entry_after_write)

### HIGH PRIORITY
- `/fatrs/src/dir_entry.rs` lines 541-550 (DirEntryEditor::write)
- `/fatrs/src/dir_entry.rs` lines 643-645 (editor creation)

### MEDIUM PRIORITY
- `/fatrs/src/dir.rs` lines 887-920 (write_entry - should ensure flushes)

## Essential Fixes Needed

1. **Force flush directory entries on file drop**
   - Store async runtime handle in File struct
   - Block in drop to flush dirty entries
   - Or require explicit close() method

2. **Flush directory entry size after each write**
   - Add `await self.flush_dir_entry()?` after `set_size()`
   - Or batch flushes with periodic sync

3. **Validate or refresh cached directory entry positions**
   - Add validation before writing
   - Or refresh position from parent directory
   - Or use generation counter to detect stale positions

4. **Ensure all pending directory entries are flushed before creating new ones**
   - Add flush protocol to directory operations
   - Ensure consistency before structural changes

## Test Case

```rust
#[test]
async fn test_multi_file_write() {
    let mut fs = create_test_fs();

    // Create and write first file
    let mut file_a = fs.root_dir().create_file("fileA.txt").await?;
    file_a.write_all(b"Content A").await?;
    file_a.flush().await?; // Explicit flush
    drop(file_a);

    // Create and write second file
    let mut file_b = fs.root_dir().create_file("fileB.txt").await?;
    file_b.write_all(b"Content B").await?;
    file_b.flush().await?; // Explicit flush
    drop(file_b);

    // Read first file - should get original content
    let mut file_a = fs.root_dir().open_file("fileA.txt").await?;
    let mut buf = Vec::new();
    file_a.read_to_end(&mut buf).await?;
    assert_eq!(&buf, b"Content A"); // ❌ FAILS: Gets "Content B" or empty
}
```

## Impact

**CRITICAL** - This bug affects all multi-file operations and causes silent data corruption. Must be fixed before any production use.
