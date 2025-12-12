# Directory Entry Cache Corruption Bug

## Summary

When multiple files are created and written to in the same directory without explicit filesystem flushes between operations, directory entry size updates are lost.

## Reproduction

```rust
let root = fs.root_dir();
for i in 0..8 {
    let filename = format!("file{}.bin", i);
    let mut file = root.create_file(&filename).await.unwrap();
    let data = vec![i as u8; 1024];  // Write 1024 bytes (2 clusters @ 512 bytes)
    file.write_all(&data).await.unwrap();
    file.flush().await.unwrap();
}
fs.flush().await.unwrap();

// Re-open file0.bin - size is 512 instead of 1024!
let mut file = root.open_file("file0.bin").await.unwrap();
let size = file.seek(SeekFrom::End(0)).await.unwrap();
assert_eq!(size, 1024);  // FAILS: size is 512
```

## Root Cause

When creating file B in a directory that already contains file A:

1. File A is created, written to (1024 bytes), and flushed
2. File A's directory entry is updated with size=1024 and written to disk
3. File B creation reads the directory sector from disk
4. File B's entry is added to the sector
5. The sector is written back - **but it contains the stale size for file A**

The problem is that when reading the directory sector to find space for file B, the in-memory dirty state of file A's directory entry is not considered. The read from disk fetches the old size value.

## Workaround

Call `fs.flush().await` after each file creation/write cycle:

```rust
for i in 0..8 {
    let mut file = root.create_file(&filename).await.unwrap();
    file.write_all(&data).await.unwrap();
    file.flush().await.unwrap();
    drop(file);
    fs.flush().await.unwrap();  // Flush FS after each file
}
```

## Affected Test

`fatrs/tests/concurrent_access.rs::test_concurrent_file_creation_unique_names`

## Fix Required

The filesystem needs to either:
1. Track dirty directory sectors and merge updates before reading
2. Use a write-through cache for directory entries
3. Lock directory sectors during multi-file operations

This is a data integrity bug that can cause silent file size corruption.

## Related Issues

### StaleDirectoryEntry on Truncate/Rename

Tests `test_file_truncate`, `test_overwrite_file`, and `test_rename_file` fail with `StaleDirectoryEntry` error. This occurs when:
1. A file operation (truncate/rename) modifies the cluster chain
2. The filesystem's cluster generation counter is incremented
3. Subsequent directory entry flush fails because the entry's generation doesn't match

This is a protection mechanism gone wrong - the generation counter check is too aggressive.

### WriteZero on Large Writes

Test `test_write_large_file` fails with `WriteZero` when writing 1MB to a 10MB image. This may be related to cluster allocation exhaustion or write failures.

## Affected Tests

- `fatrs/tests/concurrent_access.rs::test_concurrent_file_creation_unique_names`
- `fatrs/tests/embassy_integration.rs::test_file_truncate`
- `fatrs/tests/embassy_integration.rs::test_overwrite_file`
- `fatrs/tests/embassy_integration.rs::test_rename_file`
- `fatrs/tests/embassy_integration.rs::test_write_large_file`
