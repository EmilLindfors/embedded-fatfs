///! Test for generation counter detecting stale directory entries
///!
///! This test verifies that the generation counter properly detects when
///! directory clusters are reallocated, preventing corruption from writing
///! to stale directory entry positions.
///!
///! Note: These tests verify the generation counter mechanism itself.
///! Full integration testing with cluster deallocation happens in
///! multi_file_corruption.rs which exercises the complete file lifecycle.

use embedded_io_adapters::tokio_1::FromTokio;
use embedded_io_async::Write;
use fatrs::{FileSystem, FsOptions, FormatVolumeOptions};

async fn create_test_fs() -> FileSystem<FromTokio<tokio::fs::File>, fatrs::DefaultTimeProvider, fatrs::LossyOemCpConverter> {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let test_path = format!("target/test_stale_dir_{}.img", id);

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

#[tokio::test]
async fn test_generation_counter_starts_at_zero() {
    let fs = create_test_fs().await;

    // New filesystem should have generation 0
    // The generation counter API should exist and be accessible
    let generation = fs.cluster_generation();
    assert_eq!(generation, 0, "New filesystem should start with generation 0");

    // The actual increment behavior is tested via integration tests
    // in multi_file_corruption.rs which exercises real file operations
}

#[tokio::test]
async fn test_generation_doesnt_change_on_write() {
    let fs = create_test_fs().await;
    let root = fs.root_dir();

    let gen_before = fs.cluster_generation();

    // Create and write files - this allocates clusters but doesn't free them
    for i in 0..5 {
        let filename = format!("file{}.txt", i);
        let mut file = root.create_file(&filename).await.expect("Failed to create file");
        file.write_all(b"some content").await.expect("Failed to write");
        file.flush().await.expect("Failed to flush");
    }

    let gen_after = fs.cluster_generation();
    assert_eq!(gen_after, gen_before,
        "Generation should NOT increment when only allocating clusters");
}
