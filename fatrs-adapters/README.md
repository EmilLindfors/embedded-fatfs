# fatrs-adapters

Block device adapters for the fatrs ecosystem, providing both stack-allocated (no_std compatible) and heap-allocated variants.

## Overview

This crate consolidates the functionality of the former `fatrs-adapters-core` and `fatrs-adapters-alloc` crates into a single, unified adapter library with feature-gated functionality.

## Features

### Stack-Allocated Adapters (always available, no_std compatible)

- **`BufStream`**: Single-block buffering for byte-level Read/Write/Seek
- **`PageBuffer`**: Aggregates multiple blocks into larger pages (e.g., 8×512B → 4KB)
- **`PageStream`**: Byte-level access with page buffering
- **`StreamSlice`**: View into a portion of a stream

### Heap-Allocated Adapters (requires `alloc` feature)

- **`LargePageBuffer`**: Runtime-sized page buffer backed by Vec (128KB+ pages for SSDs)
- **`LargePageStream`**: Byte-level Read/Write/Seek over LargePageBuffer

### Shared Resource Abstraction

The `Shared<T>` type provides runtime-agnostic resource sharing with zero-overhead design:

- **`runtime-tokio`**: Uses `Arc<tokio::sync::Mutex<T>>` for tokio runtime
- **`runtime-generic`**: Uses `Arc<async_lock::Mutex<T>>` for portable async
- **`alloc` only**: Uses `Rc<RefCell<T>>` for single-threaded contexts
- **No features**: Direct ownership `T` - **zero overhead!**

## Feature Flags

- `std`: Enable std library support
- `alloc`: Enable heap-allocated adapters
- `log`: Enable logging via the `log` crate
- `defmt`: Enable logging via the `defmt` crate (embedded)
- `runtime-generic`: Use `async-lock` for synchronization primitives
- `runtime-tokio`: Use `tokio::sync` for synchronization primitives

## Examples

### Stack-Allocated 4KB Page Buffering

```rust
use fatrs_adapters::{PageBuffer, PageStream};

// SD card with 512-byte blocks
let sd_card: impl BlockDevice<512> = ...;

// Option 1: Use PageBuffer for page-level operations
let mut pages: PageBuffer<_, 8> = PageBuffer::new(sd_card);
pages.read_page(0).await?;

// Option 2: Use PageStream for byte-level access
let mut stream: PageStream<_, 8> = PageStream::new(sd_card);
stream.read(&mut buffer).await?;
```

### Heap-Allocated 128KB Pages for SSDs

```rust
use fatrs_adapters::{LargePageBuffer, presets};

let ssd: impl BlockDevice<512> = ...;

// Create 128KB page buffer optimized for SSDs
let mut buffer = LargePageBuffer::new(ssd, presets::PAGE_128K);
buffer.read_page(0).await?;
```

### Runtime-Agnostic Resource Sharing

```rust
use fatrs_adapters::Shared;

// Create a shared counter
let counter = Shared::new(0u32);
let clone = counter.clone();

// Acquire and modify
{
    let mut guard = counter.acquire().await;
    *guard += 1;
}

// Access from clone
{
    let guard = clone.acquire().await;
    assert_eq!(*guard, 1);
}
```

## Migration from v0.3

If you were using the separate `fatrs-adapters-core` and `fatrs-adapters-alloc` crates:

**Before:**
```toml
[dependencies]
fatrs-adapters-core = "0.3"
fatrs-adapters-alloc = "0.3"
```

**After:**
```toml
[dependencies]
fatrs-adapters = { version = "0.4", features = ["alloc"] }
```

**Code changes:**
```rust
// Before
use fatrs_adapters_core::PageStream;
use fatrs_adapters_alloc::LargePageStream;

// After
use fatrs_adapters::{PageStream, LargePageStream};
```

## License

MIT
