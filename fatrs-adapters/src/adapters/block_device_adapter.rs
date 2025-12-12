//! Adapter for connecting BlockDevice to the BlockStorage port.
//!
//! This adapter implements the hexagonal architecture pattern by translating
//! between the domain's `BlockStorage` port and the infrastructure's `BlockDevice`.

use crate::domain::{ports::BlockStorage, value_objects::BlockAddress};
use fatrs_block_device::BlockDevice;

#[cfg(feature = "alloc")]
use aligned::Aligned;

/// Adapter that implements BlockStorage using a BlockDevice.
///
/// This is the key adapter in our hexagonal architecture that connects
/// the domain layer to the actual storage infrastructure.
///
/// # Type Parameters
///
/// - `D`: The block device type (must implement `BlockDevice<BLOCK_SIZE>`)
/// - `BLOCK_SIZE`: The block size in bytes (must match the device's block size)
///
/// # Examples
///
/// ```ignore
/// use fatrs_adapters::adapters::BlockDeviceAdapter;
/// use fatrs_adapters::domain::PageBuffer;
///
/// let device = MyBlockDevice::new();
/// let adapter = BlockDeviceAdapter::new(device);
///
/// // Now use adapter with domain service
/// let mut buffer = PageBuffer::new(adapter, config);
/// ```
pub struct BlockDeviceAdapter<D: BlockDevice<BLOCK_SIZE>, const BLOCK_SIZE: usize> {
    device: D,
}

impl<D: BlockDevice<BLOCK_SIZE>, const BLOCK_SIZE: usize> BlockDeviceAdapter<D, BLOCK_SIZE> {
    /// Create a new adapter wrapping the given block device.
    pub fn new(device: D) -> Self {
        Self { device }
    }

    /// Get a reference to the underlying device.
    pub fn device(&self) -> &D {
        &self.device
    }

    /// Get a mutable reference to the underlying device.
    pub fn device_mut(&mut self) -> &mut D {
        &mut self.device
    }

    /// Consume the adapter and return the underlying device.
    pub fn into_inner(self) -> D {
        self.device
    }
}

impl<D: BlockDevice<BLOCK_SIZE>, const BLOCK_SIZE: usize> BlockStorage for BlockDeviceAdapter<D, BLOCK_SIZE>
where
    D: Send + Sync,
    D::Error: core::error::Error + Send + Sync + 'static,
{
    type Error = D::Error;

    async fn read_blocks(
        &self,
        start: BlockAddress,
        dest: &mut [u8],
    ) -> Result<(), Self::Error> {
        if dest.is_empty() {
            return Ok(());
        }

        let blocks_needed = (dest.len() + BLOCK_SIZE - 1) / BLOCK_SIZE;

        // For large reads, we need heap allocation
        #[cfg(feature = "alloc")]
        {
            extern crate alloc;

            // Create aligned buffer with zeroed blocks
            let mut buffer = alloc::vec![];
            for _ in 0..blocks_needed {
                buffer.push(Aligned::<D::Align, [u8; BLOCK_SIZE]>([0u8; BLOCK_SIZE]));
            }

            // Read from device
            self.device.read(start.value(), &mut buffer).await?;

            // Copy to destination
            let src_bytes = fatrs_block_device::blocks_to_slice(&buffer);
            let copy_len = dest.len().min(src_bytes.len());
            dest[..copy_len].copy_from_slice(&src_bytes[..copy_len]);

            Ok(())
        }

        #[cfg(not(feature = "alloc"))]
        {
            // Without alloc, we can't handle large reads
            let _ = start; // Suppress unused variable warning
            panic!("Cannot allocate buffer for {} blocks without alloc feature", blocks_needed);
        }
    }

    async fn write_blocks(
        &mut self,
        start: BlockAddress,
        src: &[u8],
    ) -> Result<(), Self::Error> {
        if src.is_empty() {
            return Ok(());
        }

        let blocks_needed = (src.len() + BLOCK_SIZE - 1) / BLOCK_SIZE;

        // For large writes, we need heap allocation
        #[cfg(feature = "alloc")]
        {
            extern crate alloc;

            // Create aligned buffer with zeroed blocks
            let mut buffer = alloc::vec![];
            for _ in 0..blocks_needed {
                buffer.push(Aligned::<D::Align, [u8; BLOCK_SIZE]>([0u8; BLOCK_SIZE]));
            }

            // Copy source to aligned buffer
            let dest_bytes = fatrs_block_device::blocks_to_slice_mut(&mut buffer);
            let copy_len = src.len().min(dest_bytes.len());
            dest_bytes[..copy_len].copy_from_slice(&src[..copy_len]);

            // Write to device
            self.device.write(start.value(), &buffer).await?;

            Ok(())
        }

        #[cfg(not(feature = "alloc"))]
        {
            let _ = start; // Suppress unused variable warning
            panic!("Cannot allocate buffer for {} blocks without alloc feature", blocks_needed);
        }
    }

    async fn size(&self) -> Result<u64, Self::Error> {
        self.device.size().await
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        self.device.sync().await
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use aligned::Aligned;
    use std::collections::HashMap;

    const BLOCK_SIZE: usize = 512;

    // Mock BlockDevice for testing
    pub(crate) struct MockBlockDevice<const BLOCK_SIZE: usize> {
        data: HashMap<u32, [u8; BLOCK_SIZE]>,
        size: u64,
    }

    impl<const BLOCK_SIZE: usize> MockBlockDevice<BLOCK_SIZE> {
        pub(crate) fn new(size: u64) -> Self {
            Self {
                data: HashMap::new(),
                size,
            }
        }
    }

    impl<const BLOCK_SIZE: usize> BlockDevice<BLOCK_SIZE> for MockBlockDevice<BLOCK_SIZE> {
        type Error = std::io::Error;
        type Align = aligned::A4;

        async fn read(
            &self,
            block_address: u32,
            data: &mut [Aligned<Self::Align, [u8; BLOCK_SIZE]>],
        ) -> Result<(), Self::Error> {
            for (i, block) in data.iter_mut().enumerate() {
                let addr = block_address + i as u32;
                if let Some(stored) = self.data.get(&addr) {
                    block.copy_from_slice(stored);
                } else {
                    // Return zeros for unwritten blocks
                    block.fill(0);
                }
            }
            Ok(())
        }

        async fn write(
            &mut self,
            block_address: u32,
            data: &[Aligned<Self::Align, [u8; BLOCK_SIZE]>],
        ) -> Result<(), Self::Error> {
            for (i, block) in data.iter().enumerate() {
                let addr = block_address + i as u32;
                let mut stored = [0u8; BLOCK_SIZE];
                stored.copy_from_slice(&block[..]);
                self.data.insert(addr, stored);
            }
            Ok(())
        }

        async fn size(&self) -> Result<u64, Self::Error> {
            Ok(self.size)
        }

        async fn sync(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_adapter_read_write() {
        let device = MockBlockDevice::<BLOCK_SIZE>::new(1024 * 1024);
        let mut adapter = BlockDeviceAdapter::new(device);

        // Write some data
        let write_data = vec![42u8; 1024];
        adapter
            .write_blocks(BlockAddress::new(0), &write_data)
            .await
            .unwrap();

        // Read it back (read_blocks takes &self)
        let mut read_data = vec![0u8; 1024];
        adapter
            .read_blocks(BlockAddress::new(0), &mut read_data)
            .await
            .unwrap();

        assert_eq!(read_data, write_data);
    }

    #[tokio::test]
    async fn test_adapter_size() {
        let device = MockBlockDevice::<BLOCK_SIZE>::new(2048);
        let adapter = BlockDeviceAdapter::new(device);

        // size takes &self
        let size = adapter.size().await.unwrap();
        assert_eq!(size, 2048);
    }

    #[tokio::test]
    async fn test_adapter_partial_block_write() {
        let device = MockBlockDevice::<BLOCK_SIZE>::new(1024 * 1024);
        let mut adapter = BlockDeviceAdapter::new(device);

        // Write less than a full block
        let write_data = vec![99u8; 256];
        adapter
            .write_blocks(BlockAddress::new(0), &write_data)
            .await
            .unwrap();

        // Read back full block
        let mut read_data = vec![0u8; BLOCK_SIZE];
        adapter
            .read_blocks(BlockAddress::new(0), &mut read_data)
            .await
            .unwrap();

        // First 256 bytes should be 99, rest should be 0
        assert_eq!(&read_data[..256], &vec![99u8; 256][..]);
        assert_eq!(&read_data[256..], &vec![0u8; BLOCK_SIZE - 256][..]);
    }
}
