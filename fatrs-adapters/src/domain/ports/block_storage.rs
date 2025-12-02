//! BlockStorage port - Secondary (driven) port for block-level I/O.
//!
//! This port defines what the domain needs from the infrastructure layer
//! for block-level storage operations. Adapters implement this trait to
//! connect the domain to actual storage devices.

use crate::domain::value_objects::BlockAddress;
use core::error::Error;

/// Port for block-level storage operations.
///
/// This is a **secondary (driven) port** in hexagonal architecture terms.
/// The domain layer depends on this abstraction, and the adapter layer
/// provides concrete implementations.
///
/// # Hexagonal Architecture
///
/// ```text
/// ┌─────────────────────┐
/// │   Domain Layer      │
/// │  (PageBuffer)       │
/// └──────────┬──────────┘
///            │ depends on
///            ▼
/// ┌─────────────────────┐
/// │  BlockStorage Port  │  ◄── This trait
/// └──────────┬──────────┘
///            │ implemented by
///            ▼
/// ┌─────────────────────┐
/// │  Adapter Layer      │
/// │ (BlockDeviceAdapter)│
/// └─────────────────────┘
/// ```
///
/// # Examples
///
/// ```ignore
/// // Domain uses the port
/// async fn load_page<S: BlockStorage>(storage: &mut S, addr: BlockAddress) {
///     let mut buffer = vec![0u8; 4096];
///     storage.read_blocks(addr, &mut buffer).await?;
///     // ...
/// }
/// ```
#[allow(async_fn_in_trait)]
pub trait BlockStorage: Send + Sync {
    /// The error type for storage operations.
    ///
    /// This should be the underlying device error type (e.g., `std::io::Error`).
    type Error: Error + Send + Sync + 'static;

    /// Read blocks from storage starting at the given block address.
    ///
    /// # Arguments
    ///
    /// * `start` - The starting block address
    /// * `dest` - Destination buffer to read into
    ///
    /// # Behavior
    ///
    /// - Reads `dest.len()` bytes starting from `start * BLOCK_SIZE`
    /// - The number of blocks read is `(dest.len() + BLOCK_SIZE - 1) / BLOCK_SIZE`
    /// - If `dest.len()` is not a multiple of block size, only full blocks are read
    ///
    /// # Errors
    ///
    /// Returns an error if the read operation fails (e.g., I/O error, out of bounds).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let addr = BlockAddress::new(10);
    /// let mut buffer = vec![0u8; 4096];
    /// storage.read_blocks(addr, &mut buffer).await?;
    /// ```
    async fn read_blocks(
        &mut self,
        start: BlockAddress,
        dest: &mut [u8],
    ) -> Result<(), Self::Error>;

    /// Write blocks to storage starting at the given block address.
    ///
    /// # Arguments
    ///
    /// * `start` - The starting block address
    /// * `src` - Source buffer to write from
    ///
    /// # Behavior
    ///
    /// - Writes `src.len()` bytes starting at `start * BLOCK_SIZE`
    /// - The number of blocks written is `(src.len() + BLOCK_SIZE - 1) / BLOCK_SIZE`
    /// - If `src.len()` is not a multiple of block size, the last block is zero-padded
    ///
    /// # Errors
    ///
    /// Returns an error if the write operation fails (e.g., I/O error, read-only storage).
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let addr = BlockAddress::new(10);
    /// let buffer = vec![42u8; 4096];
    /// storage.write_blocks(addr, &buffer).await?;
    /// ```
    async fn write_blocks(&mut self, start: BlockAddress, src: &[u8])
        -> Result<(), Self::Error>;

    /// Get the total size of the storage device in bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the size cannot be determined.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let size = storage.size().await?;
    /// let num_pages = size / 4096;
    /// ```
    async fn size(&mut self) -> Result<u64, Self::Error>;

    /// Flush any cached writes to the underlying storage.
    ///
    /// This operation ensures that all buffered writes are committed to
    /// persistent storage. The default implementation is a no-op.
    ///
    /// # Errors
    ///
    /// Returns an error if the flush operation fails.
    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::fmt;

    // Mock storage for testing
    struct MockStorage {
        data: std::collections::HashMap<u32, Vec<u8>>,
        block_size: usize,
    }

    impl MockStorage {
        fn new(block_size: usize) -> Self {
            Self {
                data: std::collections::HashMap::new(),
                block_size,
            }
        }
    }

    #[derive(Debug)]
    struct MockError;

    impl fmt::Display for MockError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "Mock storage error")
        }
    }

    impl Error for MockError {}

    impl BlockStorage for MockStorage {
        type Error = MockError;

        async fn read_blocks(
            &mut self,
            start: BlockAddress,
            dest: &mut [u8],
        ) -> Result<(), Self::Error> {
            let blocks_needed = (dest.len() + self.block_size - 1) / self.block_size;

            for i in 0..blocks_needed {
                let block_num = start.value() + i as u32;
                let block_data = self.data.get(&block_num).cloned().unwrap_or_else(|| {
                    vec![0u8; self.block_size]
                });

                let dest_offset = i * self.block_size;
                let copy_len = (dest.len() - dest_offset).min(self.block_size);
                dest[dest_offset..dest_offset + copy_len].copy_from_slice(&block_data[..copy_len]);
            }

            Ok(())
        }

        async fn write_blocks(
            &mut self,
            start: BlockAddress,
            src: &[u8],
        ) -> Result<(), Self::Error> {
            let blocks_needed = (src.len() + self.block_size - 1) / self.block_size;

            for i in 0..blocks_needed {
                let block_num = start.value() + i as u32;
                let src_offset = i * self.block_size;
                let copy_len = (src.len() - src_offset).min(self.block_size);

                let mut block_data = vec![0u8; self.block_size];
                block_data[..copy_len].copy_from_slice(&src[src_offset..src_offset + copy_len]);

                self.data.insert(block_num, block_data);
            }

            Ok(())
        }

        async fn size(&mut self) -> Result<u64, Self::Error> {
            Ok(1024 * 1024) // 1MB
        }
    }

    #[tokio::test]
    async fn test_mock_storage_read_write() {
        let mut storage = MockStorage::new(512);

        // Write some data
        let write_data = vec![42u8; 1024];
        storage
            .write_blocks(BlockAddress::new(0), &write_data)
            .await
            .unwrap();

        // Read it back
        let mut read_data = vec![0u8; 1024];
        storage
            .read_blocks(BlockAddress::new(0), &mut read_data)
            .await
            .unwrap();

        assert_eq!(read_data, write_data);
    }

    #[tokio::test]
    async fn test_mock_storage_size() {
        let mut storage = MockStorage::new(512);
        let size = storage.size().await.unwrap();
        assert_eq!(size, 1024 * 1024);
    }
}
