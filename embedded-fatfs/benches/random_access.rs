///! Random Access Benchmark
///!
///! Measures the latency of random seek and read operations.
///! This benchmark demonstrates the effectiveness of FAT caching.

use std::time::Instant;
use tokio::fs;
use embedded_io_async::{Read, Seek, SeekFrom};

#[tokio::main]
async fn main() {
    println!("===== Embedded-FatFS Random Access Benchmark =====\n");

    // Copy test image
    fs::copy("../resources/fat32.img", "target/bench_random.img").await
        .expect("Failed to copy test image");

    benchmark_random_access().await;

    // Cleanup
    let _ = fs::remove_file("target/bench_random.img").await;
}

async fn benchmark_random_access() {
    println!("--- Random Access Latency Benchmark ---");

    let img_file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("target/bench_random.img")
        .await
        .unwrap();

    let buf_stream = tokio::io::BufStream::new(img_file);
    let fs = embedded_fatfs::FileSystem::new(buf_stream, embedded_fatfs::FsOptions::new())
        .await
        .unwrap();

    // Create a 10MB test file
    let test_data = vec![0xCD; 1024 * 1024]; // 1MB chunks
    let mut file = fs.root_dir().create_file("random_test.bin").await.unwrap();

    for _ in 0..10 {
        file.write_all(&test_data).await.unwrap();
    }
    file.flush().await.unwrap();
    drop(file);

    // Now perform random reads
    let mut file = fs.root_dir().open_file("random_test.bin").await.unwrap();
    let mut buf = vec![0u8; 4096]; // 4KB reads

    let iterations = 100;
    let file_size = 10 * 1024 * 1024u64;

    let start = Instant::now();

    for i in 0..iterations {
        // Random offset (aligned to 4KB for consistency)
        let offset = ((i * 12345) % (file_size / 4096)) * 4096;

        file.seek(SeekFrom::Start(offset)).await.unwrap();
        file.read(&mut buf).await.unwrap();
    }

    let elapsed = start.elapsed();
    let avg_latency = elapsed / iterations;

    println!("  Iterations: {}", iterations);
    println!("  Total time: {:.3}s", elapsed.as_secs_f64());
    println!("  Avg latency: {:.2}ms", avg_latency.as_secs_f64() * 1000.0);
    println!("  Operations/sec: {:.0}", iterations as f64 / elapsed.as_secs_f64());
    println!();
}
