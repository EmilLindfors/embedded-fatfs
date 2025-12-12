//! Edge case tests for fatrs
//!
//! These tests cover edge cases related to:
//! - Rename operations (LFN, nested dirs, edge cases)
//! - Truncate operations (multiple, boundary conditions)
//! - File write boundaries (cluster boundaries, overwrites)
//! - FAT cache eviction scenarios
//! - Disk full conditions

use std::fs::File;
use std::io::{Read as StdRead, Seek as StdSeek, Write as StdWrite};
use std::sync::{Arc, Mutex};

use embedded_io_async::{ErrorType, Read, Seek, SeekFrom, Write};
use fatrs::{FileSystem, FsOptions};

/// Test block device wrapper
#[derive(Clone)]
struct TestBlockDevice {
    inner: Arc<Mutex<File>>,
}

impl TestBlockDevice {
    fn new(file: File) -> Self {
        Self {
            inner: Arc::new(Mutex::new(file)),
        }
    }
}

impl ErrorType for TestBlockDevice {
    type Error = std::io::Error;
}

impl Read for TestBlockDevice {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        let mut file = self.inner.lock().unwrap();
        file.read(buf)
    }
}

impl Write for TestBlockDevice {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        let mut file = self.inner.lock().unwrap();
        file.write(buf)
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        let mut file = self.inner.lock().unwrap();
        file.flush()
    }
}

impl Seek for TestBlockDevice {
    async fn seek(&mut self, pos: SeekFrom) -> Result<u64, Self::Error> {
        let mut file = self.inner.lock().unwrap();
        let std_pos = match pos {
            SeekFrom::Start(n) => std::io::SeekFrom::Start(n),
            SeekFrom::End(n) => std::io::SeekFrom::End(n),
            SeekFrom::Current(n) => std::io::SeekFrom::Current(n),
        };
        file.seek(std_pos)
    }
}

fn create_test_image(path: &str, size_mb: u32) -> std::io::Result<()> {
    use std::process::Command;

    let _ = std::fs::create_dir_all("target");

    let file = File::create(path)?;
    file.set_len((size_mb as u64) * 1024 * 1024)?;
    drop(file);

    // Try mkfs.fat first, fall back to internal formatter
    let output = Command::new("mkfs.fat")
        .args(["-F", "32", "-n", "TEST", path])
        .output();

    if output.is_err() || !output.as_ref().unwrap().status.success() {
        let file = File::options().read(true).write(true).open(path)?;
        let mut device = TestBlockDevice::new(file);

        futures::executor::block_on(async {
            let options = fatrs::FormatVolumeOptions::new()
                .fat_type(fatrs::FatType::Fat32)
                .volume_label(*b"TEST       ");
            fatrs::format_volume(&mut device, options).await.unwrap();
        });
    }

    Ok(())
}

fn create_fat16_test_image(path: &str, size_mb: u32) -> std::io::Result<()> {
    use std::process::Command;

    let _ = std::fs::create_dir_all("target");

    let file = File::create(path)?;
    file.set_len((size_mb as u64) * 1024 * 1024)?;
    drop(file);

    // Try mkfs.fat with FAT16
    let output = Command::new("mkfs.fat")
        .args(["-F", "16", "-n", "TEST", path])
        .output();

    if output.is_err() || !output.as_ref().unwrap().status.success() {
        let file = File::options().read(true).write(true).open(path)?;
        let mut device = TestBlockDevice::new(file);

        futures::executor::block_on(async {
            let options = fatrs::FormatVolumeOptions::new()
                .fat_type(fatrs::FatType::Fat16)
                .volume_label(*b"TEST       ");
            fatrs::format_volume(&mut device, options).await.unwrap();
        });
    }

    Ok(())
}

fn cleanup_test_image(path: &str) {
    let _ = std::fs::remove_file(path);
}

// =============================================================================
// RENAME EDGE CASES
// =============================================================================

/// Test renaming a file to the same name (should be a no-op)
#[tokio::test]
async fn test_rename_to_same_name() {
    let path = "target/test_rename_same.img";
    create_test_image(path, 10).unwrap();

    let file = File::options().read(true).write(true).open(path).unwrap();
    let device = TestBlockDevice::new(file);

    let fs = FileSystem::new(device, FsOptions::new()).await.unwrap();
    let root = fs.root_dir();

    // Create file
    let mut file = root.create_file("test.txt").await.unwrap();
    file.write_all(b"content").await.unwrap();
    file.flush().await.unwrap();
    drop(file);

    // Rename to same name - should succeed (no-op)
    root.rename("test.txt", &root, "test.txt").await.unwrap();

    // Verify file still exists with correct content
    let mut file = root.open_file("test.txt").await.unwrap();
    let mut buf = vec![0u8; 10];
    let n = file.read(&mut buf).await.unwrap();
    assert_eq!(&buf[..n], b"content");

    cleanup_test_image(path);
}

/// Test renaming with long file names
#[tokio::test]
async fn test_rename_long_filename() {
    let path = "target/test_rename_lfn.img";
    create_test_image(path, 10).unwrap();

    let file = File::options().read(true).write(true).open(path).unwrap();
    let device = TestBlockDevice::new(file);

    let fs = FileSystem::new(device, FsOptions::new()).await.unwrap();
    let root = fs.root_dir();

    // Create file with long name
    let long_name = "this_is_a_very_long_filename_that_requires_lfn_entries.txt";
    let mut file = root.create_file(long_name).await.unwrap();
    file.write_all(b"long name content").await.unwrap();
    file.flush().await.unwrap();
    drop(file);

    // Rename to another long name
    let new_long_name = "another_extremely_long_filename_for_testing_purposes.dat";
    root.rename(long_name, &root, new_long_name).await.unwrap();

    // Verify old name is gone
    assert!(root.open_file(long_name).await.is_err());

    // Verify new name exists with correct content
    let mut file = root.open_file(new_long_name).await.unwrap();
    let mut buf = vec![0u8; 20];
    let n = file.read(&mut buf).await.unwrap();
    assert_eq!(&buf[..n], b"long name content");

    cleanup_test_image(path);
}

/// Test renaming to an existing file (should fail with AlreadyExists)
#[tokio::test]
async fn test_rename_to_existing_file() {
    let path = "target/test_rename_exists.img";
    create_test_image(path, 10).unwrap();

    let file = File::options().read(true).write(true).open(path).unwrap();
    let device = TestBlockDevice::new(file);

    let fs = FileSystem::new(device, FsOptions::new()).await.unwrap();
    let root = fs.root_dir();

    // Create two files
    let mut file1 = root.create_file("file1.txt").await.unwrap();
    file1.write_all(b"content1").await.unwrap();
    file1.flush().await.unwrap();
    drop(file1);

    let mut file2 = root.create_file("file2.txt").await.unwrap();
    file2.write_all(b"content2").await.unwrap();
    file2.flush().await.unwrap();
    drop(file2);

    // Try to rename file1 to file2 - should fail
    let result = root.rename("file1.txt", &root, "file2.txt").await;
    assert!(result.is_err());

    // Both files should still exist with original content
    let mut file1 = root.open_file("file1.txt").await.unwrap();
    let mut buf = vec![0u8; 10];
    let n = file1.read(&mut buf).await.unwrap();
    assert_eq!(&buf[..n], b"content1");

    let mut file2 = root.open_file("file2.txt").await.unwrap();
    let n = file2.read(&mut buf).await.unwrap();
    assert_eq!(&buf[..n], b"content2");

    cleanup_test_image(path);
}

/// Test rename in nested directories
#[tokio::test]
async fn test_rename_in_nested_directory() {
    let path = "target/test_rename_nested.img";
    create_test_image(path, 10).unwrap();

    let file = File::options().read(true).write(true).open(path).unwrap();
    let device = TestBlockDevice::new(file);

    let fs = FileSystem::new(device, FsOptions::new()).await.unwrap();
    let root = fs.root_dir();

    // Create nested directories
    root.create_dir("level1").await.unwrap();
    let level1 = root.open_dir("level1").await.unwrap();
    level1.create_dir("level2").await.unwrap();
    let level2 = level1.open_dir("level2").await.unwrap();

    // Create file in nested directory
    let mut file = level2.create_file("nested.txt").await.unwrap();
    file.write_all(b"nested content").await.unwrap();
    file.flush().await.unwrap();
    drop(file);

    // Rename file within same nested directory
    level2
        .rename("nested.txt", &level2, "renamed.txt")
        .await
        .unwrap();

    // Verify old name is gone
    assert!(level2.open_file("nested.txt").await.is_err());

    // Verify new name exists
    let mut file = level2.open_file("renamed.txt").await.unwrap();
    let mut buf = vec![0u8; 20];
    let n = file.read(&mut buf).await.unwrap();
    assert_eq!(&buf[..n], b"nested content");

    cleanup_test_image(path);
}

/// Test multiple sequential renames
#[tokio::test]
async fn test_multiple_sequential_renames() {
    let path = "target/test_multi_rename.img";
    create_test_image(path, 10).unwrap();

    let file = File::options().read(true).write(true).open(path).unwrap();
    let device = TestBlockDevice::new(file);

    let fs = FileSystem::new(device, FsOptions::new()).await.unwrap();
    let root = fs.root_dir();

    // Create file
    let mut file = root.create_file("original.txt").await.unwrap();
    file.write_all(b"test data").await.unwrap();
    file.flush().await.unwrap();
    drop(file);

    // Rename multiple times
    root.rename("original.txt", &root, "renamed1.txt")
        .await
        .unwrap();
    root.rename("renamed1.txt", &root, "renamed2.txt")
        .await
        .unwrap();
    root.rename("renamed2.txt", &root, "final.txt")
        .await
        .unwrap();

    // Verify only final name exists
    assert!(root.open_file("original.txt").await.is_err());
    assert!(root.open_file("renamed1.txt").await.is_err());
    assert!(root.open_file("renamed2.txt").await.is_err());

    let mut file = root.open_file("final.txt").await.unwrap();
    let mut buf = vec![0u8; 10];
    let n = file.read(&mut buf).await.unwrap();
    assert_eq!(&buf[..n], b"test data");

    cleanup_test_image(path);
}

/// Test rename on FAT16 (fixed-size root directory)
#[tokio::test]
async fn test_rename_fat16() {
    let path = "target/test_rename_fat16.img";
    create_fat16_test_image(path, 10).unwrap();

    let file = File::options().read(true).write(true).open(path).unwrap();
    let device = TestBlockDevice::new(file);

    let fs = FileSystem::new(device, FsOptions::new()).await.unwrap();
    let root = fs.root_dir();

    // Create and rename in FAT16 root directory
    let mut file = root.create_file("fat16test.txt").await.unwrap();
    file.write_all(b"fat16 content").await.unwrap();
    file.flush().await.unwrap();
    drop(file);

    root.rename("fat16test.txt", &root, "fat16new.txt")
        .await
        .unwrap();

    // Verify
    assert!(root.open_file("fat16test.txt").await.is_err());
    let mut file = root.open_file("fat16new.txt").await.unwrap();
    let mut buf = vec![0u8; 20];
    let n = file.read(&mut buf).await.unwrap();
    assert_eq!(&buf[..n], b"fat16 content");

    cleanup_test_image(path);
}

// =============================================================================
// TRUNCATE EDGE CASES
// =============================================================================

/// Test multiple truncates in sequence
/// Note: truncate() truncates to the CURRENT position, not to 0.
/// To clear a file, seek to 0 first.
#[tokio::test]
async fn test_multiple_truncates() {
    let path = "target/test_multi_truncate.img";
    create_test_image(path, 10).unwrap();

    let file = File::options().read(true).write(true).open(path).unwrap();
    let device = TestBlockDevice::new(file);

    let fs = FileSystem::new(device, FsOptions::new()).await.unwrap();
    let root = fs.root_dir();

    // Create file with data
    let mut file = root.create_file("truncate.txt").await.unwrap();
    file.write_all(b"initial content that is quite long")
        .await
        .unwrap();
    file.flush().await.unwrap();

    // Truncate to 0 (seek to 0 first, then truncate)
    file.seek(SeekFrom::Start(0)).await.unwrap();
    file.truncate().await.unwrap();
    file.write_all(b"second").await.unwrap();
    file.flush().await.unwrap();

    // Truncate again
    file.seek(SeekFrom::Start(0)).await.unwrap();
    file.truncate().await.unwrap();
    file.write_all(b"third").await.unwrap();
    file.flush().await.unwrap();

    // Final truncate and write
    file.seek(SeekFrom::Start(0)).await.unwrap();
    file.truncate().await.unwrap();
    file.write_all(b"final").await.unwrap();
    file.flush().await.unwrap();
    drop(file);

    // Verify final content
    let mut file = root.open_file("truncate.txt").await.unwrap();
    let mut buf = vec![0u8; 50];
    let n = file.read(&mut buf).await.unwrap();
    assert_eq!(&buf[..n], b"final");

    cleanup_test_image(path);
}

/// Test truncate on empty file
#[tokio::test]
async fn test_truncate_empty_file() {
    let path = "target/test_truncate_empty.img";
    create_test_image(path, 10).unwrap();

    let file = File::options().read(true).write(true).open(path).unwrap();
    let device = TestBlockDevice::new(file);

    let fs = FileSystem::new(device, FsOptions::new()).await.unwrap();
    let root = fs.root_dir();

    // Create empty file
    let mut file = root.create_file("empty.txt").await.unwrap();
    file.flush().await.unwrap();

    // Truncate empty file - should be no-op
    file.truncate().await.unwrap();
    file.flush().await.unwrap();

    // Write after truncate of empty file
    file.write_all(b"now has content").await.unwrap();
    file.flush().await.unwrap();
    drop(file);

    // Verify content
    let mut file = root.open_file("empty.txt").await.unwrap();
    let mut buf = vec![0u8; 20];
    let n = file.read(&mut buf).await.unwrap();
    assert_eq!(&buf[..n], b"now has content");

    cleanup_test_image(path);
}

/// Test truncate followed by large write
#[tokio::test]
async fn test_truncate_then_large_write() {
    let path = "target/test_truncate_large.img";
    create_test_image(path, 20).unwrap();

    let file = File::options().read(true).write(true).open(path).unwrap();
    let device = TestBlockDevice::new(file);

    let fs = FileSystem::new(device, FsOptions::new()).await.unwrap();
    let root = fs.root_dir();

    // Create file with some data
    let mut file = root.create_file("large.txt").await.unwrap();
    file.write_all(&vec![0xAA; 10000]).await.unwrap();
    file.flush().await.unwrap();

    // Truncate to 0 (seek to start, then truncate)
    file.seek(SeekFrom::Start(0)).await.unwrap();
    file.truncate().await.unwrap();

    // Write larger data
    let large_data = vec![0xBB; 100000];
    file.write_all(&large_data).await.unwrap();
    file.flush().await.unwrap();
    drop(file);

    // Verify
    let mut file = root.open_file("large.txt").await.unwrap();
    let size = file.seek(SeekFrom::End(0)).await.unwrap();
    assert_eq!(size, 100000);

    // Verify content
    file.seek(SeekFrom::Start(0)).await.unwrap();
    let mut buf = vec![0u8; 100000];
    let n = file.read(&mut buf).await.unwrap();
    assert_eq!(n, 100000);
    assert!(buf.iter().all(|&b| b == 0xBB));

    cleanup_test_image(path);
}

// =============================================================================
// FILE WRITE EDGE CASES
// =============================================================================

/// Test writing at exact cluster boundary
#[tokio::test]
async fn test_write_at_cluster_boundary() {
    let path = "target/test_cluster_boundary.img";
    create_test_image(path, 10).unwrap();

    let file = File::options().read(true).write(true).open(path).unwrap();
    let device = TestBlockDevice::new(file);

    let fs = FileSystem::new(device, FsOptions::new()).await.unwrap();
    let root = fs.root_dir();

    // Get cluster size
    let stats = fs.stats().await.unwrap();
    let cluster_size = stats.cluster_size() as usize;

    // Create file and write exactly one cluster
    let mut file = root.create_file("boundary.txt").await.unwrap();
    let data1 = vec![0xAA; cluster_size];
    file.write_all(&data1).await.unwrap();
    file.flush().await.unwrap();

    // Write exactly one more cluster
    let data2 = vec![0xBB; cluster_size];
    file.write_all(&data2).await.unwrap();
    file.flush().await.unwrap();
    drop(file);

    // Verify
    let mut file = root.open_file("boundary.txt").await.unwrap();
    let size = file.seek(SeekFrom::End(0)).await.unwrap();
    assert_eq!(size, (cluster_size * 2) as u64);

    // Verify content
    file.seek(SeekFrom::Start(0)).await.unwrap();
    let mut buf = vec![0u8; cluster_size * 2];
    let n = file.read(&mut buf).await.unwrap();
    assert_eq!(n, cluster_size * 2);
    assert!(buf[..cluster_size].iter().all(|&b| b == 0xAA));
    assert!(buf[cluster_size..].iter().all(|&b| b == 0xBB));

    cleanup_test_image(path);
}

/// Test overwrite in middle of file
#[tokio::test]
async fn test_overwrite_middle_of_file() {
    let path = "target/test_overwrite_middle.img";
    create_test_image(path, 10).unwrap();

    let file = File::options().read(true).write(true).open(path).unwrap();
    let device = TestBlockDevice::new(file);

    let fs = FileSystem::new(device, FsOptions::new()).await.unwrap();
    let root = fs.root_dir();

    // Create file with pattern
    let mut file = root.create_file("overwrite.txt").await.unwrap();
    file.write_all(b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")
        .await
        .unwrap();
    file.flush().await.unwrap();
    drop(file);

    // Open and overwrite middle
    let mut file = root.open_file("overwrite.txt").await.unwrap();
    file.seek(SeekFrom::Start(10)).await.unwrap();
    file.write_all(b"BBBBBBBBBB").await.unwrap();
    file.flush().await.unwrap();
    drop(file);

    // Verify
    let mut file = root.open_file("overwrite.txt").await.unwrap();
    let mut buf = vec![0u8; 50];
    let n = file.read(&mut buf).await.unwrap();
    assert_eq!(n, 40);
    assert_eq!(&buf[..10], b"AAAAAAAAAA");
    assert_eq!(&buf[10..20], b"BBBBBBBBBB");
    assert_eq!(&buf[20..40], b"AAAAAAAAAAAAAAAAAAAA");

    cleanup_test_image(path);
}

/// Test many small writes (tests buffering/caching)
#[tokio::test]
async fn test_many_small_writes() {
    let path = "target/test_small_writes.img";
    create_test_image(path, 10).unwrap();

    let file = File::options().read(true).write(true).open(path).unwrap();
    let device = TestBlockDevice::new(file);

    let fs = FileSystem::new(device, FsOptions::new()).await.unwrap();
    let root = fs.root_dir();

    let mut file = root.create_file("small.txt").await.unwrap();

    // Many small writes
    for i in 0..1000 {
        file.write_all(&[i as u8]).await.unwrap();
    }
    file.flush().await.unwrap();
    drop(file);

    // Verify
    let mut file = root.open_file("small.txt").await.unwrap();
    let size = file.seek(SeekFrom::End(0)).await.unwrap();
    assert_eq!(size, 1000);

    file.seek(SeekFrom::Start(0)).await.unwrap();
    let mut buf = vec![0u8; 1000];
    let n = file.read(&mut buf).await.unwrap();
    assert_eq!(n, 1000);
    for (i, &byte) in buf.iter().enumerate() {
        assert_eq!(byte, i as u8, "Mismatch at position {}", i);
    }

    cleanup_test_image(path);
}

// =============================================================================
// FAT CACHE EDGE CASES (multiple files triggering cache eviction)
// =============================================================================

/// Test creating many files to trigger FAT cache eviction
#[tokio::test]
async fn test_many_files_cache_eviction() {
    let path = "target/test_cache_eviction.img";
    create_test_image(path, 20).unwrap();

    let file = File::options().read(true).write(true).open(path).unwrap();
    let device = TestBlockDevice::new(file);

    let fs = FileSystem::new(device, FsOptions::new()).await.unwrap();
    let root = fs.root_dir();

    // Create many files to stress FAT cache
    let file_count = 50;
    let data_size = 4096; // 4KB per file

    for i in 0..file_count {
        let name = format!("file_{:03}.dat", i);
        let mut file = root.create_file(&name).await.unwrap();
        let data = vec![(i % 256) as u8; data_size];
        file.write_all(&data).await.unwrap();
        file.flush().await.unwrap();
    }

    // Flush filesystem
    fs.flush().await.unwrap();

    // Verify all files
    for i in 0..file_count {
        let name = format!("file_{:03}.dat", i);
        let mut file = root.open_file(&name).await.unwrap();
        let size = file.seek(SeekFrom::End(0)).await.unwrap();
        assert_eq!(size, data_size as u64, "File {} has wrong size", name);

        file.seek(SeekFrom::Start(0)).await.unwrap();
        let mut buf = vec![0u8; data_size];
        let n = file.read(&mut buf).await.unwrap();
        assert_eq!(n, data_size);
        assert!(
            buf.iter().all(|&b| b == (i % 256) as u8),
            "File {} has wrong content",
            name
        );
    }

    cleanup_test_image(path);
}

/// Test interleaved operations on multiple files
#[tokio::test]
async fn test_interleaved_file_operations() {
    let path = "target/test_interleaved.img";
    create_test_image(path, 20).unwrap();

    let file = File::options().read(true).write(true).open(path).unwrap();
    let device = TestBlockDevice::new(file);

    let fs = FileSystem::new(device, FsOptions::new()).await.unwrap();
    let root = fs.root_dir();

    // Create multiple files
    let mut file_a = root.create_file("file_a.txt").await.unwrap();
    let mut file_b = root.create_file("file_b.txt").await.unwrap();
    let mut file_c = root.create_file("file_c.txt").await.unwrap();

    // Interleaved writes
    for i in 0..100 {
        file_a.write_all(format!("A{}", i).as_bytes()).await.unwrap();
        file_b.write_all(format!("B{}", i).as_bytes()).await.unwrap();
        file_c.write_all(format!("C{}", i).as_bytes()).await.unwrap();
    }

    file_a.flush().await.unwrap();
    file_b.flush().await.unwrap();
    file_c.flush().await.unwrap();
    drop(file_a);
    drop(file_b);
    drop(file_c);

    // Verify each file has correct content
    let mut file_a = root.open_file("file_a.txt").await.unwrap();
    let mut buf = vec![0u8; 1000];
    let n = file_a.read(&mut buf).await.unwrap();
    let content_a = String::from_utf8_lossy(&buf[..n]);
    assert!(content_a.starts_with("A0A1A2"));

    let mut file_b = root.open_file("file_b.txt").await.unwrap();
    let n = file_b.read(&mut buf).await.unwrap();
    let content_b = String::from_utf8_lossy(&buf[..n]);
    assert!(content_b.starts_with("B0B1B2"));

    let mut file_c = root.open_file("file_c.txt").await.unwrap();
    let n = file_c.read(&mut buf).await.unwrap();
    let content_c = String::from_utf8_lossy(&buf[..n]);
    assert!(content_c.starts_with("C0C1C2"));

    cleanup_test_image(path);
}

// =============================================================================
// DISK FULL EDGE CASES
// =============================================================================

/// Test writing a large amount of data (stress test for cluster allocation)
#[tokio::test]
async fn test_write_large_amount() {
    let path = "target/test_large_write.img";
    // Create 5MB image
    create_test_image(path, 5).unwrap();

    let file = File::options().read(true).write(true).open(path).unwrap();
    let device = TestBlockDevice::new(file);

    let fs = FileSystem::new(device, FsOptions::new()).await.unwrap();
    let root = fs.root_dir();

    let mut file = root.create_file("large.dat").await.unwrap();

    // Write 2MB of data in chunks
    let chunk = vec![0xDD; 4096];
    let target_size = 2 * 1024 * 1024; // 2MB
    let mut total_written = 0u64;

    while total_written < target_size {
        file.write_all(&chunk).await.unwrap();
        total_written += chunk.len() as u64;
    }

    file.flush().await.unwrap();
    drop(file);

    // Verify file size
    let mut file = root.open_file("large.dat").await.unwrap();
    let size = file.seek(SeekFrom::End(0)).await.unwrap();
    assert_eq!(size, target_size);

    // Verify some content
    file.seek(SeekFrom::Start(0)).await.unwrap();
    let mut buf = vec![0u8; 4096];
    let n = file.read(&mut buf).await.unwrap();
    assert_eq!(n, 4096);
    assert!(buf.iter().all(|&b| b == 0xDD));

    cleanup_test_image(path);
}

/// Test creating many small files (stress test for directory entries and cluster allocation)
#[tokio::test]
async fn test_create_many_small_files() {
    let path = "target/test_many_files.img";
    // Create 10MB image - enough for many files
    create_test_image(path, 10).unwrap();

    let file = File::options().read(true).write(true).open(path).unwrap();
    let device = TestBlockDevice::new(file);

    let fs = FileSystem::new(device, FsOptions::new()).await.unwrap();
    let root = fs.root_dir();

    // Create 100 small files
    let file_count = 100;
    for i in 0..file_count {
        let name = format!("f{:05}.txt", i);
        let mut file = root.create_file(&name).await.unwrap();
        let content = format!("content{}", i);
        file.write_all(content.as_bytes()).await.unwrap();
        file.flush().await.unwrap();
    }

    // Verify all files exist and have correct content
    for i in 0..file_count {
        let name = format!("f{:05}.txt", i);
        let mut file = root.open_file(&name).await.unwrap();
        let mut buf = vec![0u8; 20];
        let n = file.read(&mut buf).await.unwrap();
        let expected = format!("content{}", i);
        assert_eq!(&buf[..n], expected.as_bytes(), "File {} content mismatch", name);
    }

    cleanup_test_image(path);
}

// =============================================================================
// SEEK EDGE CASES
// =============================================================================

/// Test seek to position 0
#[tokio::test]
async fn test_seek_to_zero() {
    let path = "target/test_seek_zero.img";
    create_test_image(path, 10).unwrap();

    let file = File::options().read(true).write(true).open(path).unwrap();
    let device = TestBlockDevice::new(file);

    let fs = FileSystem::new(device, FsOptions::new()).await.unwrap();
    let root = fs.root_dir();

    let mut file = root.create_file("seek.txt").await.unwrap();
    file.write_all(b"0123456789").await.unwrap();
    file.flush().await.unwrap();

    // Seek to various positions and back to 0
    file.seek(SeekFrom::Start(5)).await.unwrap();
    let pos = file.seek(SeekFrom::Start(0)).await.unwrap();
    assert_eq!(pos, 0);

    // Read from beginning
    let mut buf = vec![0u8; 10];
    let n = file.read(&mut buf).await.unwrap();
    assert_eq!(n, 10);
    assert_eq!(&buf, b"0123456789");

    cleanup_test_image(path);
}

/// Test SeekFrom::Current with negative offset
#[tokio::test]
async fn test_seek_current_negative() {
    let path = "target/test_seek_neg.img";
    create_test_image(path, 10).unwrap();

    let file = File::options().read(true).write(true).open(path).unwrap();
    let device = TestBlockDevice::new(file);

    let fs = FileSystem::new(device, FsOptions::new()).await.unwrap();
    let root = fs.root_dir();

    let mut file = root.create_file("seek_neg.txt").await.unwrap();
    file.write_all(b"ABCDEFGHIJ").await.unwrap();
    file.flush().await.unwrap();

    // Seek to end, then back
    file.seek(SeekFrom::End(0)).await.unwrap();
    let pos = file.seek(SeekFrom::Current(-5)).await.unwrap();
    assert_eq!(pos, 5);

    // Read from position 5
    let mut buf = vec![0u8; 5];
    let n = file.read(&mut buf).await.unwrap();
    assert_eq!(n, 5);
    assert_eq!(&buf, b"FGHIJ");

    cleanup_test_image(path);
}

/// Test SeekFrom::End with various offsets
#[tokio::test]
async fn test_seek_from_end() {
    let path = "target/test_seek_end.img";
    create_test_image(path, 10).unwrap();

    let file = File::options().read(true).write(true).open(path).unwrap();
    let device = TestBlockDevice::new(file);

    let fs = FileSystem::new(device, FsOptions::new()).await.unwrap();
    let root = fs.root_dir();

    let mut file = root.create_file("seek_end.txt").await.unwrap();
    file.write_all(b"0123456789").await.unwrap();
    file.flush().await.unwrap();

    // Seek from end with offset 0
    let pos = file.seek(SeekFrom::End(0)).await.unwrap();
    assert_eq!(pos, 10);

    // Seek from end with negative offset
    let pos = file.seek(SeekFrom::End(-3)).await.unwrap();
    assert_eq!(pos, 7);

    // Read from that position
    let mut buf = vec![0u8; 3];
    let n = file.read(&mut buf).await.unwrap();
    assert_eq!(n, 3);
    assert_eq!(&buf, b"789");

    cleanup_test_image(path);
}

// =============================================================================
// DELETE EDGE CASES
// =============================================================================

/// Test delete and recreate same filename
#[tokio::test]
async fn test_delete_and_recreate() {
    let path = "target/test_delete_recreate.img";
    create_test_image(path, 10).unwrap();

    let file = File::options().read(true).write(true).open(path).unwrap();
    let device = TestBlockDevice::new(file);

    let fs = FileSystem::new(device, FsOptions::new()).await.unwrap();
    let root = fs.root_dir();

    // Create, delete, recreate cycle multiple times
    for cycle in 0..5 {
        let mut file = root.create_file("cycle.txt").await.unwrap();
        let content = format!("cycle {}", cycle);
        file.write_all(content.as_bytes()).await.unwrap();
        file.flush().await.unwrap();
        drop(file);

        // Verify content
        let mut file = root.open_file("cycle.txt").await.unwrap();
        let mut buf = vec![0u8; 20];
        let n = file.read(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], content.as_bytes());
        drop(file);

        // Delete
        root.remove("cycle.txt").await.unwrap();
        assert!(root.open_file("cycle.txt").await.is_err());
    }

    cleanup_test_image(path);
}

/// Test delete file with long filename
#[tokio::test]
async fn test_delete_long_filename() {
    let path = "target/test_delete_lfn.img";
    create_test_image(path, 10).unwrap();

    let file = File::options().read(true).write(true).open(path).unwrap();
    let device = TestBlockDevice::new(file);

    let fs = FileSystem::new(device, FsOptions::new()).await.unwrap();
    let root = fs.root_dir();

    let long_name = "this_is_a_file_with_a_very_long_name_that_needs_lfn.txt";
    let mut file = root.create_file(long_name).await.unwrap();
    file.write_all(b"long name file").await.unwrap();
    file.flush().await.unwrap();
    drop(file);

    // Delete the file
    root.remove(long_name).await.unwrap();

    // Verify it's gone
    assert!(root.open_file(long_name).await.is_err());

    // Create new file - should reuse directory entries
    let mut file = root.create_file("short.txt").await.unwrap();
    file.write_all(b"short").await.unwrap();
    file.flush().await.unwrap();

    cleanup_test_image(path);
}
