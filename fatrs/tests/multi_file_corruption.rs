///! Tests for multi-file write corruption bug
///!
///! These tests verify that writing multiple files doesn't cause data corruption.
///! The bug being tested was: writing a second file would corrupt the first file
///! because directory entries weren't being flushed after each write.

use embedded_io_adapters::tokio_1::FromTokio;
use embedded_io_async::{Write, Seek, SeekFrom};
use fatrs::{FileSystem, FsOptions, FormatVolumeOptions};

async fn create_test_fs() -> FileSystem<FromTokio<tokio::fs::File>, fatrs::DefaultTimeProvider, fatrs::LossyOemCpConverter> {
    // Create a unique test image for this test
    let test_id = std::thread::current().id();
    let test_path = format!("target/test_multi_file_{:?}.img", test_id);

    // Create a 10MB image file
    let file = tokio::fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&test_path)
        .await
        .expect("Failed to create test image");

    // Set file size to 10MB
    file.set_len(10 * 1024 * 1024).await.expect("Failed to set file size");

    let mut device = FromTokio::new(file);

    // Format the filesystem
    fatrs::format_volume(&mut device, FormatVolumeOptions::new())
        .await
        .expect("Failed to format filesystem");

    // Now open the filesystem
    FileSystem::new(device, FsOptions::new())
        .await
        .expect("Failed to mount filesystem")
}

/// Helper to read all bytes from a file
async fn read_all<R: embedded_io_async::Read>(file: &mut R) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut temp = [0u8; 512];
    loop {
        let n = file.read(&mut temp).await.expect("Failed to read");
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&temp[..n]);
    }
    buf
}

#[tokio::test]
async fn test_write_two_files_no_corruption() {
    let fs = create_test_fs().await;
    let root = fs.root_dir();

    // Create and write first file
    let mut file_a = root.create_file("fileA.txt").await.expect("Failed to create fileA.txt");
    file_a.write_all(b"Content from file A").await.expect("Failed to write to fileA");
    file_a.flush().await.expect("Failed to flush fileA");
    drop(file_a);

    // Create and write second file
    let mut file_b = root.create_file("fileB.txt").await.expect("Failed to create fileB.txt");
    file_b.write_all(b"Content from file B").await.expect("Failed to write to fileB");
    file_b.flush().await.expect("Failed to flush fileB");
    drop(file_b);

    // Read first file - should get original content, not second file's content
    let mut file_a = root.open_file("fileA.txt").await.expect("Failed to open fileA.txt");
    let buf = read_all(&mut file_a).await;

    assert_eq!(&buf, b"Content from file A", "fileA.txt was corrupted!");

    // Read second file - should get its own content
    let mut file_b = root.open_file("fileB.txt").await.expect("Failed to open fileB.txt");
    let buf = read_all(&mut file_b).await;

    assert_eq!(&buf, b"Content from file B", "fileB.txt has wrong content!");
}

#[tokio::test]
async fn test_write_multiple_files_sequential() {
    let fs = create_test_fs().await;
    let root = fs.root_dir();

    // Write multiple files sequentially
    for i in 0..5 {
        let filename = format!("file{}.txt", i);
        let content = format!("Content for file {}", i);

        let mut file = root.create_file(&filename).await.expect("Failed to create file");
        file.write_all(content.as_bytes()).await.expect("Failed to write");
        file.flush().await.expect("Failed to flush");
        drop(file);
    }

    // Verify each file has correct content
    for i in 0..5 {
        let filename = format!("file{}.txt", i);
        let expected_content = format!("Content for file {}", i);

        let mut file = root.open_file(&filename).await.expect("Failed to open file");
        let buf = read_all(&mut file).await;

        assert_eq!(
            String::from_utf8_lossy(&buf),
            expected_content,
            "File {} was corrupted!",
            filename
        );
    }
}

#[tokio::test]
async fn test_write_without_explicit_flush() {
    // This test verifies that files must be flushed before dropping
    // In the fixed version, writes flush directory entries immediately,
    // but the file data itself still needs explicit flush

    let fs = create_test_fs().await;
    let root = fs.root_dir();

    // Write file without explicit flush (relies on drop)
    {
        let mut file = root.create_file("test.txt").await.expect("Failed to create");
        file.write_all(b"Test content").await.expect("Failed to write");
        // Note: No explicit flush here - but the directory entry WILL be flushed
        // because we fixed update_dir_entry_after_write to flush immediately
    } // file dropped here

    // Read it back
    let mut file = root.open_file("test.txt").await.expect("Failed to open");
    let buf = read_all(&mut file).await;

    // The directory entry should have correct size because we flush it immediately
    // But the file data might not be fully persisted without explicit flush
    // (depending on the storage implementation)
    assert_eq!(&buf, b"Test content", "File content incorrect");
}

#[tokio::test]
async fn test_overwrite_existing_file() {
    let fs = create_test_fs().await;
    let root = fs.root_dir();

    // Create initial file
    let mut file = root.create_file("overwrite.txt").await.expect("Failed to create");
    file.write_all(b"Initial content that is quite long").await.expect("Failed to write");
    file.flush().await.expect("Failed to flush");
    drop(file);

    // Overwrite with shorter content
    let mut file = root.open_file("overwrite.txt").await.expect("Failed to open");
    file.seek(SeekFrom::Start(0)).await.expect("Failed to seek");
    file.write_all(b"Short").await.expect("Failed to write");
    file.truncate().await.expect("Failed to truncate");
    file.flush().await.expect("Failed to flush");
    drop(file);

    // Verify new content
    let mut file = root.open_file("overwrite.txt").await.expect("Failed to open");
    let buf = read_all(&mut file).await;

    assert_eq!(&buf, b"Short", "File was not properly overwritten");
}

#[tokio::test]
async fn test_directory_entry_size_updated() {
    let fs = create_test_fs().await;
    let root = fs.root_dir();

    // Create file and write data
    let mut file = root.create_file("sized.txt").await.expect("Failed to create");
    file.write_all(b"Exactly 25 characters!!").await.expect("Failed to write");
    file.flush().await.expect("Failed to flush");
    drop(file);

    // Check directory entry reports correct size
    let mut iter = root.iter();
    let mut found = false;
    while let Some(entry) = iter.next().await {
        let entry = entry.expect("Failed to read entry");
        if entry.file_name() == "sized.txt" {
            assert_eq!(entry.len(), 23, "Directory entry size is wrong");
            found = true;
            break;
        }
    }

    assert!(found, "File not found in directory");
}

#[tokio::test]
async fn test_concurrent_file_handles() {
    // Test having multiple file handles open simultaneously
    let fs = create_test_fs().await;
    let root = fs.root_dir();

    // Create two files
    let mut file_a = root.create_file("concurrent_a.txt").await.expect("Failed to create A");
    let mut file_b = root.create_file("concurrent_b.txt").await.expect("Failed to create B");

    // Write to both (interleaved)
    file_a.write_all(b"AAA").await.expect("Failed to write A1");
    file_b.write_all(b"BBB").await.expect("Failed to write B1");
    file_a.write_all(b"AAA").await.expect("Failed to write A2");
    file_b.write_all(b"BBB").await.expect("Failed to write B2");

    // Flush both
    file_a.flush().await.expect("Failed to flush A");
    file_b.flush().await.expect("Failed to flush B");
    drop(file_a);
    drop(file_b);

    // Verify both files
    let mut file_a = root.open_file("concurrent_a.txt").await.expect("Failed to open A");
    let buf_a = read_all(&mut file_a).await;
    assert_eq!(&buf_a, b"AAAAAA", "File A corrupted");

    let mut file_b = root.open_file("concurrent_b.txt").await.expect("Failed to open B");
    let buf_b = read_all(&mut file_b).await;
    assert_eq!(&buf_b, b"BBBBBB", "File B corrupted");
}
