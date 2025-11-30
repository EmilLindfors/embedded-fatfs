embedded-fatfs
===========

[![CI Status](https://github.com/mabezdev/embedded-fatfs/actions/workflows/ci.yml/badge.svg)](https://github.com/mabezdev/embedded-fatfs/actions/workflows/ci.yml)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE.txt)
[![crates.io](https://img.shields.io/crates/v/embedded-fatfs)](https://crates.io/crates/embedded-fatfs)
[![Documentation](https://docs.rs/embedded-fatfs/badge.svg)](https://docs.rs/embedded-fatfs)
![Minimum rustc version](https://img.shields.io/badge/rustc-1.75+-green.svg)

A FAT filesystem library implemented in Rust. Built on the shoulders of the amazing [rust-fatfs](https://github.com/rafalh/rust-fatfs) crate by [@rafalh](https://github.com/rafalh).

## Features
* async
* read/write to files using `embedded-io-async` Read/Write traits
* read directory contents
* create/remove file or directory
* rename/move file or directory
* read/write file timestamps (updated automatically if `chrono` feature is enabled)
* format volume
* FAT12, FAT16, FAT32 compatibility
* LFN (Long File Names) extension is supported
* `no_std` environment support

## Quick Start

### Tokio Example

```rust
use embedded_fatfs::{FileSystem, FsOptions};
use embedded_io_async::Write;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let img_file = tokio::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("fat32.img")
        .await?;

    // Don't use tokio::io::BufStream - it slows down performance!
    // The FAT cache handles buffering internally.
    let fs = FileSystem::new(img_file, FsOptions::new()).await?;

    let mut file = fs.root_dir().create_file("hello.txt").await?;
    file.write_all(b"Hello, embedded-fatfs!").await?;
    file.flush().await?;

    fs.flush().await?;
    Ok(())
}
```

### Embassy Example (Embedded)

```rust
use embassy_executor::Spawner;
use embedded_fatfs::{FileSystem, FsOptions};
use embedded_io_async::Write;
use block_device_adapters::BufStream;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let sd_card = init_sd_card().await;

    // Use BufStream from block-device-adapters for embedded systems
    let buf_stream = BufStream::<_, 512>::new(sd_card);
    let fs = FileSystem::new(buf_stream, FsOptions::new()).await.unwrap();

    let mut file = fs.root_dir().create_file("test.log").await.unwrap();
    file.write_all(b"Hello from embedded!").await.unwrap();
    file.flush().await.unwrap();

    fs.unmount().await.unwrap();
}
```

## Porting from rust-fatfs to embedded-fatfs

There a are a few key differences between the crates:

- embedded-fatfs is async, therefore your storage device must implement the [embedded-io-async](https://github.com/rust-embedded/embedded-hal/tree/master/embedded-io-async) traits.
- You must call `flush` on `File`s before they are dropped. See the CHANGELOG for details.
- **Performance tip:** Don't use `tokio::io::BufStream` - the built-in FAT cache and multi-cluster I/O optimizations handle buffering more efficiently.

`no_std` usage
------------

Add this to your `Cargo.toml`:

    [dependencies]
    embedded-fatfs = { version = "0.1", default-features = false }

Additional features:

* `lfn` - LFN (long file name) support
* `alloc` - use `alloc` crate for dynamic allocation. Needed for API which uses `String` type. You may have to provide
a memory allocator implementation.
* `unicode` - use Unicode-compatible case conversion in file names - you may want to have it disabled for lower memory
footprint

License
-------
The MIT license. See `LICENSE`.
